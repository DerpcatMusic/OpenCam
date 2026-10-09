mod assets;
mod auth;
mod benchmark;
mod discovery;
mod frost;
mod media;
mod monitor;
mod output;
mod processing;
mod protocol;
mod session;
mod ui;
mod webcam;

use anyhow::{Context as _, Result};
use gpui::{
    App, Bounds, Context, Div, ElementId, FocusHandle, KeyBinding, MouseButton, ObjectFit,
    RenderImage, SharedString, Stateful, Window, WindowBounds, WindowOptions, actions, div, img,
    prelude::*, px, relative, rgb, size,
};
use serde_json::{Value, json};
use std::{
    process::Command,
    sync::{Arc, atomic::Ordering, mpsc},
    time::{Duration, Instant},
};

const BG: u32 = 0x060606;
const SURFACE: u32 = 0x0d0d0d;
const LINE: u32 = 0x252525;
const INK: u32 = 0xe8e8e8;
const MUTED: u32 = 0xa8a8a8;
const ACCENT: u32 = 0xa5a0ff;

actions!(opencam, [NextFocus, PreviousFocus]);

#[derive(Clone)]
struct Slider {
    key: &'static str,
    label: &'static str,
    min: f64,
    max: f64,
    log: bool,
}
impl Slider {
    fn value(&self, fraction: f64) -> f64 {
        let fraction = fraction.clamp(0., 1.);
        if self.log && self.min > 0. {
            self.min * (self.max / self.min).powf(fraction)
        } else {
            self.min + fraction * (self.max - self.min)
        }
    }
    fn fraction(&self, value: f64) -> f64 {
        if self.max <= self.min {
            return 0.;
        }
        if self.log && self.min > 0. {
            ((value / self.min).ln() / (self.max / self.min).ln()).clamp(0., 1.)
        } else {
            ((value - self.min) / (self.max - self.min)).clamp(0., 1.)
        }
    }
}

struct Studio {
    pairing: String,
    root_focus: FocusHandle,
    catalog: Value,
    settings: Value,
    phone_revision: Option<u64>,
    metadata: Value,
    processing_report: Value,
    session: Option<session::Session>,
    shared: Arc<session::Shared>,
    preview: Option<Arc<RenderImage>>,
    status: String,
    error: bool,
    streaming: bool,
    tool: Option<u8>,
    monitor: monitor::Monitor,
    camera_status: String,
    connecting: bool,
    benchmark: Option<benchmark::Benchmark>,
    measurements: Vec<benchmark::Measurement>,
    last_stats: (Instant, session::Stats),
    fps: f64,
    mbps: f64,
    dirty: Option<Instant>,
    dragging: Option<&'static str>,
    utility_tx: mpsc::Sender<Value>,
    utility_rx: mpsc::Receiver<Value>,
    serial: Option<String>,
    usb_port: Option<u16>,
    connection_id: u64,
    menu: Option<ui::Menu>,
    menu_focus: FocusHandle,
    format_inputs: [gpui::Entity<gpui_base::input::InputState>; 3],
    _format_subscriptions: Vec<gpui::Subscription>,
    pairing_input: gpui::Entity<gpui_base::input::InputState>,
    password_input: gpui::Entity<gpui_base::input::InputState>,
    discovery: Option<discovery::Discovery>,
}

impl Studio {
    fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        pairing: String,
        serial: Option<String>,
        video_device: Option<String>,
    ) -> Self {
        let (utility_tx, utility_rx) = mpsc::channel();
        let root_focus = cx.focus_handle();
        window.focus(&root_focus, cx);
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(8))
                    .await;
                if cx
                    .update(|window, cx| this.update(cx, |this, cx| this.tick(window, cx)))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let pairing: String = pairing
            .chars()
            .filter(|c| c.is_ascii() && !c.is_control())
            .take(1024)
            .collect();
        let format_inputs = ["1280x720", "Source", "30"].map(|value| {
            cx.new(|cx| {
                gpui_base::input::InputState::new(window, cx)
                    .default_value(value)
                    .validate(|text, _| {
                        text.len() <= 32
                            && text
                                .chars()
                                .all(|c| c.is_ascii_alphanumeric() || c == '×' || c == ' ')
                    })
            })
        });
        let mut format_subscriptions: Vec<_> = format_inputs
            .iter()
            .enumerate()
            .map(|(i, state)| {
                cx.subscribe_in(
                    state,
                    window,
                    move |this, state, event: &gpui_base::input::InputEvent, window, cx| {
                        if matches!(event, gpui_base::input::InputEvent::Focus) {
                            state.update(cx, |state, cx| state.select_all(window, cx));
                        }
                        if matches!(event, gpui_base::input::InputEvent::PressEnter { .. }) {
                            this.apply_format(i, window, cx);
                            cx.notify();
                        }
                    },
                )
            })
            .collect();
        let pairing_input = cx.new(|cx| {
            gpui_base::input::InputState::new(window, cx)
                .default_value(pairing.clone())
                .placeholder("Pairing link")
                .validate(|text, _| {
                    text.len() <= 1024 && text.is_ascii() && !text.chars().any(char::is_control)
                })
        });
        format_subscriptions.push(cx.subscribe(
            &pairing_input,
            |this, state, event: &gpui_base::input::InputEvent, cx| {
                if matches!(event, gpui_base::input::InputEvent::Change) {
                    this.pairing = state.read(cx).value().to_string();
                }
            },
        ));
        format_subscriptions.push(cx.subscribe_in(
            &pairing_input,
            window,
            |this, _, event: &gpui_base::input::InputEvent, _, cx| {
                if matches!(event, gpui_base::input::InputEvent::PressEnter { .. }) {
                    this.connect(false);
                    cx.notify();
                }
            },
        ));
        let password_input = cx.new(|cx| {
            gpui_base::input::InputState::new(window, cx)
                .masked(true)
                .placeholder("Password")
                .validate(|text, _| text.chars().count() <= 128)
        });
        format_subscriptions.push(cx.subscribe(
            &password_input,
            |this, state, event: &gpui_base::input::InputEvent, cx| {
                if matches!(event, gpui_base::input::InputEvent::Change) {
                    *this.shared.password.lock().unwrap() = state.read(cx).value().to_string();
                }
            },
        ));
        let discovery = discovery::Discovery::new();
        let discovery_error = discovery.as_ref().err().map(ToString::to_string);
        let mut studio = Self {
            pairing,
            root_focus,
            catalog: Value::Null,
            settings: Value::Null,
            metadata: Value::Null,
            processing_report: Value::Null,
            session: None,
            shared: session::Shared::new(video_device),
            preview: None,
            status: discovery_error.unwrap_or("Disconnected".into()),
            error: false,
            streaming: false,
            tool: Some(0),
            monitor: monitor::Monitor {
                threshold: 242,
                ..Default::default()
            },
            camera_status: "Camera output pending".into(),
            connecting: false,
            benchmark: None,
            measurements: vec![],
            last_stats: (Instant::now(), session::Stats::default()),
            fps: 0.,
            mbps: 0.,
            dirty: None,
            phone_revision: None,
            dragging: None,
            utility_tx,
            utility_rx,
            serial,
            usb_port: None,
            connection_id: 0,
            menu: None,
            menu_focus: cx.focus_handle(),
            format_inputs,
            _format_subscriptions: format_subscriptions,
            pairing_input,
            password_input,
            discovery: discovery.ok(),
        };
        if !studio.pairing.is_empty() {
            studio.connect(false);
        }
        studio
    }
    fn message(&mut self, text: impl Into<String>, error: bool) {
        self.status = text.into();
        self.error = error;
    }
    fn connected(&self) -> bool {
        self.session.is_some() && !self.catalog.is_null() && !self.connecting
    }
    fn camera(&self) -> Value {
        let camera = self.catalog["cameras"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| c["id"] == self.settings["camera"])
            .cloned()
            .unwrap_or(Value::Null);
        protocol::camera_mode(&camera, &self.settings["codec"])
    }
    fn send(&self, mut message: Value) {
        if matches!(message["type"].as_str(), Some("configure" | "controls")) {
            if let Some(revision) = self.phone_revision {
                message["baseRevision"] = json!(revision);
            }
        }
        if let Some(s) = &self.session {
            s.send(message);
        }
    }
    fn connect(&mut self, usb: bool) {
        if self.connecting {
            return;
        }
        let pairing = match protocol::Pairing::parse(&self.pairing) {
            Ok(pairing) => pairing,
            Err(e) => {
                self.message(e.to_string(), true);
                return;
            }
        };
        self.disconnect();
        self.connecting = true;
        self.message(
            if usb {
                "Opening USB connection…"
            } else {
                "Pairing over encrypted Wi-Fi…"
            },
            false,
        );
        if usb {
            let tx = self.utility_tx.clone();
            let serial = self.serial.clone();
            let connection_id = self.connection_id;
            let link = self.pairing.clone();
            std::thread::spawn(move || {
                let mut command = Command::new("adb");
                if let Some(serial) = &serial {
                    command.args(["-s", &serial]);
                }
                let result = command
                    .args(["forward", "tcp:0", &format!("tcp:{}", protocol::PORT)])
                    .output()
                    .context("Install Android platform-tools and put adb on PATH")
                    .and_then(|o| {
                        anyhow::ensure!(
                            o.status.success(),
                            "USB: {}",
                            String::from_utf8_lossy(&o.stderr)
                        );
                        Ok(String::from_utf8(o.stdout)?.trim().parse::<u16>()?)
                    });
                let event = match result {
                    Ok(port) => {
                        json!({"type":"usb_ready", "port":port, "connection":connection_id, "link":link})
                    }
                    Err(e) => {
                        json!({"type":"usb_error", "message":format!("{e:#}"), "connection":connection_id})
                    }
                };
                if let Err(e) = tx.send(event) {
                    if let Some(port) = e.0["port"].as_u64() {
                        let mut cleanup = Command::new("adb");
                        if let Some(serial) = &serial {
                            cleanup.args(["-s", serial]);
                        }
                        let _ = cleanup
                            .args(["forward", "--remove", &format!("tcp:{port}")])
                            .output();
                    }
                }
            });
        } else {
            self.start_session(pairing);
        }
    }
    fn start_session(&mut self, pairing: protocol::Pairing) {
        *self.shared.stats.lock().unwrap() = session::Stats::default();
        *self.shared.frame.lock().unwrap() = None;
        self.last_stats = (Instant::now(), session::Stats::default());
        self.session = Some(session::Session::start(pairing, self.shared.clone()));
    }
    fn disconnect(&mut self) {
        self.connection_id = self.connection_id.wrapping_add(1);
        self.menu = None;
        self.session.take();
        self.benchmark.take();
        self.catalog = Value::Null;
        self.metadata = Value::Null;
        self.processing_report = Value::Null;
        self.streaming = false;
        self.connecting = false;
        self.dirty = None;
        self.phone_revision = None;
        *self.shared.frame.lock().unwrap() = None;
        if let Some(port) = self.usb_port.take() {
            self.remove_forward(port);
        }
    }
    fn remove_forward(&self, port: u16) {
        let serial = self.serial.clone();
        std::thread::spawn(move || {
            let mut adb = Command::new("adb");
            if let Some(serial) = serial {
                adb.args(["-s", &serial]);
            }
            let _ = adb
                .args(["forward", "--remove", &format!("tcp:{port}")])
                .output();
        });
    }
    fn start_stream(&mut self) {
        if !self.connected() {
            return;
        }
        self.dirty = None;
        let output = *self.shared.output.lock().unwrap();
        self.settings["outputWidth"] = json!(output.width);
        self.settings["outputHeight"] = json!(output.height);
        self.settings["phoneRotation"] = json!(*self.shared.rotation.lock().unwrap());
        self.settings["phoneMirror"] = json!(self.shared.mirror.load(Ordering::Relaxed));
        if self.settings["outputMode"] != 2 {
            self.settings["outputMode"] = json!(if output.crop { 1 } else { 0 });
        }
        self.sync_processing();
        self.message("Starting camera…", false);
        self.send(json!({"type":"configure", "settings":self.settings}));
    }
    fn camera_select(&mut self, camera: Value) {
        if self.benchmark.is_some() {
            return;
        }
        match protocol::default_settings(&self.catalog, &camera) {
            Ok(mut settings) => {
                for key in processing::DESKTOP_KEYS
                    .iter()
                    .chain(processing::EFFECT_KEYS)
                {
                    if let Some(value) = self.settings.get(*key) {
                        settings[*key] = value.clone();
                    }
                }
                self.settings = settings;
                if self.streaming {
                    self.start_stream();
                }
            }
            Err(e) => self.message(e.to_string(), true),
        }
    }
    fn set(&mut self, key: &'static str, value: Value) {
        if self.benchmark.is_some() || !self.connected() {
            return;
        }
        self.settings[key] = value;
        self.sync_processing();
        if key != "bitrate" {
            self.dirty.get_or_insert(Instant::now());
        }
    }
    fn controls(&self) -> Value {
        let mut controls = self.settings.clone();
        if let Some(map) = controls.as_object_mut() {
            for key in [
                "camera",
                "codec",
                "width",
                "height",
                "fps",
                "bitrate",
                "outputWidth",
                "outputHeight",
                "phoneRotation",
                "phoneMirror",
                "outputMode",
            ] {
                map.remove(key);
            }
        }
        controls
    }
    fn sync_processing(&mut self) {
        match processing::Options::read(&self.settings) {
            Ok(options) => *self.shared.processing.lock().unwrap() = options,
            Err(e) => self.message(e.to_string(), true),
        }
    }
    fn merge_phone_settings(&mut self, settings: &Value) {
        processing::merge_phone_settings(&mut self.settings, settings);
    }
    fn sync_phone_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (index, value) in [
            (
                0,
                format!("{}x{}", self.settings["width"], self.settings["height"]),
            ),
            (2, self.settings["fps"].to_string()),
        ] {
            self.format_inputs[index].update(cx, |state, cx| state.set_value(value, window, cx));
        }
        if self.settings["processingLocation"] != "desktop" {
            let explicit = self.settings["outputWidth"].as_u64().unwrap_or(0) > 0
                || self.settings["outputHeight"].as_u64().unwrap_or(0) > 0;
            let text = if explicit {
                let options = processing::Options::read(&self.settings).unwrap_or_default();
                let (w, h) = options.dimensions(
                    self.settings["width"].as_u64().unwrap_or(0) as u32,
                    self.settings["height"].as_u64().unwrap_or(0) as u32,
                );
                format!("{w}x{h}")
            } else {
                "source".into()
            };
            if let Ok((w, h)) = output::resolution(&text) {
                *self.shared.output.lock().unwrap() = output::Output {
                    width: w,
                    height: h,
                    crop: self.settings["outputMode"] == 1,
                };
                self.format_inputs[1].update(cx, |state, cx| {
                    state.set_value(
                        if w == 0 && h == 0 {
                            "Source".into()
                        } else {
                            format!("{w}x{h}")
                        },
                        window,
                        cx,
                    )
                });
            }
            *self.shared.rotation.lock().unwrap() =
                self.settings["phoneRotation"].as_u64().unwrap_or(0) as u16;
            self.shared
                .mirror
                .store(self.settings["phoneMirror"] == true, Ordering::Relaxed);
        }
        self.sync_processing();
    }
    fn toggle(&mut self, key: &'static str) {
        let value = !self.settings[key].as_bool().unwrap_or(false);
        if key == "ois" && value {
            self.settings["stabilization"] = json!(false);
        }
        if key == "stabilization" && value {
            self.settings["ois"] = json!(false);
        }
        self.set(key, json!(value));
    }
    fn slider_set(&mut self, slider: &Slider, fraction: f64) {
        if self.benchmark.is_some() || !self.connected() {
            return;
        }
        let value = slider.value(fraction);
        if matches!(slider.key, "iso" | "exposureNs") {
            self.settings["manual"] = json!(true);
        }
        if slider.key == "focus" {
            self.settings["focusAuto"] = json!(false);
        }
        self.set(
            slider.key,
            if matches!(
                slider.key,
                "iso" | "exposureNs" | "ev" | "bitrate" | "maskFps"
            ) {
                json!(value.round() as i64)
            } else {
                json!(value)
            },
        );
    }
    fn benchmark_start(&mut self) {
        if !self.connected() || self.benchmark.is_some() {
            return;
        }
        let candidates = self.catalog["codecs"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if candidates.is_empty() {
            self.message("No hardware encoders exposed by the phone", true);
            return;
        }
        if let Some(session) = &self.session {
            self.benchmark = Some(benchmark::Benchmark::new(
                candidates,
                self.settings.clone(),
                session,
            ));
            self.message(
                "Benchmarking codecs at the selected size, FPS and bitrate…",
                false,
            );
        }
    }
    fn export(&mut self) {
        let result = serde_json::to_vec_pretty(&json!({"capabilities":self.catalog, "captureMetadata":self.metadata, "processing":self.processing_report,
            "settings":self.settings, "output":*self.shared.output.lock().unwrap(), "benchmarks":self.measurements,
            "measurementNote":"Frame age is estimated from clock synchronization. It excludes display scanout."}))
            .map_err(anyhow::Error::from).and_then(|bytes| { std::fs::write("opencam-camera-report.json", bytes)?; Ok(()) });
        match result {
            Ok(()) => self.message(
                "Saved opencam-camera-report.json in the launch folder",
                false,
            ),
            Err(e) => self.message(format!("Export failed: {e}"), true),
        }
    }
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut changed = self.discovery.as_mut().is_some_and(|d| d.poll());
        if changed && self.pairing.is_empty() {
            if let Some(phone) = self
                .discovery
                .as_ref()
                .and_then(|d| d.phones.values().next())
            {
                self.pairing = phone.link.clone();
                let link = self.pairing.clone();
                self.pairing_input
                    .update(cx, |state, cx| state.set_value(link, window, cx));
            }
        }
        while let Ok(event) = self.utility_rx.try_recv() {
            changed = true;
            if event["connection"].as_u64() != Some(self.connection_id) {
                if let Some(port) = event["port"].as_u64() {
                    self.remove_forward(port as u16);
                }
                continue;
            }
            if event["type"] == "usb_ready" {
                if let Ok(mut pair) = protocol::Pairing::parse(event["link"].as_str().unwrap_or(""))
                {
                    let port = event["port"].as_u64().unwrap_or(0) as u16;
                    self.usb_port = Some(port);
                    pair.address = format!("127.0.0.1:{port}");
                    self.start_session(pair);
                }
            } else {
                self.connecting = false;
                self.message(event["message"].as_str().unwrap_or("USB failed"), true);
            }
        }
        let events: Vec<Value> = self
            .session
            .as_ref()
            .map(|s| s.events.try_iter().take(100).collect())
            .unwrap_or_default();
        for event in events {
            changed = true;
            if let Some(benchmark) = &mut self.benchmark {
                benchmark.event(&event);
            }
            if let Some(revision) = event["revision"].as_u64() {
                self.phone_revision = Some(revision);
            }
            if event["origin"] == "phone" && event["settings"].is_object() {
                self.benchmark = None;
                let sent = processing::phone_settings(&self.settings);
                if self.settings["processingLocation"] == "desktop"
                    && processing::EFFECT_KEYS.iter().any(|key| {
                        event["settings"]
                            .get(*key)
                            .is_some_and(|value| value != &sent[*key])
                    })
                {
                    self.settings["processingLocation"] = json!("phone");
                }
            }
            match event["type"].as_str() {
                Some("capabilities") => {
                    let reconnect = !self.catalog.is_null();
                    self.catalog = event;
                    let camera = self.catalog["cameras"]
                        .as_array()
                        .and_then(|c| {
                            c.iter()
                                .find(|c| c["id"] == self.catalog["settings"]["camera"])
                                .or_else(|| c.iter().find(|c| c["facing"] == 1))
                                .or_else(|| c.first())
                        })
                        .cloned();
                    if let Some(camera) = camera {
                        if !reconnect {
                            self.camera_select(camera);
                        }
                        let phone = self.catalog["settings"].clone();
                        self.merge_phone_settings(&phone);
                        if self.settings["codec"] == "opencam.i420" {
                            self.settings["processingLocation"] = json!("desktop");
                        }
                        self.sync_phone_controls(window, cx);
                        self.format_inputs[0].update(cx, |state, cx| {
                            state.set_value(
                                format!("{}x{}", self.settings["width"], self.settings["height"]),
                                window,
                                cx,
                            )
                        });
                        self.format_inputs[2].update(cx, |state, cx| {
                            state.set_value(self.settings["fps"].to_string(), window, cx)
                        });
                    }
                    self.connecting = false;
                    self.message("Connected", false);
                    if !self.settings.is_null() {
                        self.tool = Some(1);
                        self.start_stream();
                    }
                }
                Some("configured") => {
                    self.streaming = true;
                    if self.benchmark.is_none() {
                        self.dirty = None;
                        self.merge_phone_settings(&event["settings"]);
                        self.sync_phone_controls(window, cx);
                        self.message("Camera live", false);
                    }
                }
                Some("controls" | "state") => {
                    if self.dirty.is_none()
                        || event["origin"] == "phone"
                        || event["conflict"] == true
                    {
                        self.dirty = None;
                        self.merge_phone_settings(&event["settings"]);
                        self.sync_phone_controls(window, cx);
                    }
                    if event["conflict"] == true {
                        self.message(
                            "Settings changed on the phone; try your adjustment again",
                            true,
                        );
                    }
                }
                Some("stopped") => {
                    self.streaming = false;
                    self.message(event["message"].as_str().unwrap_or("Camera stopped"), false);
                }
                Some("metadata") => self.metadata = event["values"].clone(),
                Some("processing") if self.settings["processingLocation"] != "desktop" => {
                    self.processing_report = event.clone()
                }
                Some("reconnecting") => {
                    self.connecting = true;
                    self.streaming = false;
                    self.dirty = None;
                    self.benchmark = None;
                    self.message(
                        format!("Reconnecting · attempt {}", event["attempt"]),
                        false,
                    );
                }
                Some("raw_transfer_error") => self.message(
                    event["message"]
                        .as_str()
                        .unwrap_or("DNG transfer failed; file remains on phone"),
                    true,
                ),
                Some("raw_downloaded") => {
                    self.message(event["message"].as_str().unwrap_or("DNG downloaded"), false)
                }
                Some("raw_saved") => self.message(
                    event["message"].as_str().unwrap_or("RAW saved on phone"),
                    false,
                ),
                Some("output_ready") => {
                    self.camera_status = event["device"]
                        .as_str()
                        .unwrap_or("Camera output active")
                        .to_owned()
                }
                Some("output_error") => {
                    self.camera_status = event["message"]
                        .as_str()
                        .unwrap_or("Camera output unavailable")
                        .to_owned();
                }
                Some("error") => {
                    if event["settings"].is_object() {
                        self.dirty = None;
                        self.merge_phone_settings(&event["settings"]);
                        self.sync_phone_controls(window, cx);
                    }
                    self.message(event["message"].as_str().unwrap_or("Camera error"), true)
                }
                Some("disconnected") => {
                    self.disconnect();
                    self.message(
                        event["message"].as_str().unwrap_or("Phone disconnected"),
                        true,
                    );
                }
                _ => {}
            }
        }
        let stats = self.shared.stats.lock().unwrap().clone();
        if self.last_stats.0.elapsed() >= Duration::from_secs(1) {
            let elapsed = self.last_stats.0.elapsed().as_secs_f64();
            self.fps = stats.frames.saturating_sub(self.last_stats.1.frames) as f64 / elapsed;
            self.mbps =
                stats.bytes.saturating_sub(self.last_stats.1.bytes) as f64 * 8. / elapsed / 1e6;
            self.last_stats = (Instant::now(), stats.clone());
            changed = true;
        }
        if let (Some(benchmark), Some(session)) = (&mut self.benchmark, &self.session) {
            if benchmark.tick(stats, session) {
                let benchmark = self.benchmark.take().unwrap();
                let chosen = benchmark.winner().map(|w| w.codec.clone());
                self.measurements = benchmark.results;
                self.settings = benchmark.original;
                if let Some(codec) = chosen {
                    self.settings["codec"] = json!(codec);
                    self.start_stream();
                    self.message(
                        "Benchmark complete. Applied the best measured codec.",
                        false,
                    );
                } else {
                    self.start_stream();
                    self.message(
                        "No codec completed the benchmark. Restored your settings.",
                        true,
                    );
                }
                changed = true;
            }
        }
        if self
            .dirty
            .is_some_and(|t| t.elapsed() >= Duration::from_millis(33))
        {
            self.dirty = None;
            if self.streaming {
                self.send(json!({"type":"controls", "settings":self.controls()}));
            }
        }
        if self.settings["processingLocation"] == "desktop" {
            let report = self
                .shared
                .desktop_processing_report
                .lock()
                .unwrap()
                .clone();
            if report != self.processing_report {
                self.processing_report = report;
                changed = true;
            }
        }
        let frame = self.shared.frame.lock().unwrap().take();
        if let Some(mut frame) = frame {
            self.monitor
                .process(&mut frame.pixels, frame.width, frame.height);
            let _sequence = frame.sequence;
            if let Some(old) = self.preview.take() {
                cx.drop_image(old, Some(window));
            }
            if let Some(buffer) =
                image::RgbaImage::from_raw(frame.width, frame.height, frame.pixels)
            {
                self.preview = Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])));
                changed = true;
            }
        } else if !self.streaming && self.preview.is_some() {
            if let Some(old) = self.preview.take() {
                cx.drop_image(old, Some(window));
            }
            changed = true;
        }
        if changed {
            cx.notify();
        }
    }
}

impl Drop for Studio {
    fn drop(&mut self) {
        self.disconnect();
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let argument = |key: &str| {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let pair = argument("--pair").unwrap_or_default();
    let serial = argument("--serial");
    let video_device = Some(argument("--v4l2").unwrap_or("auto".into()));
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            gpui_base::init(cx);
            if let Err(e) = cx.text_system().add_fonts(vec![
                std::borrow::Cow::Borrowed(include_bytes!("fonts/Geist.ttf")),
                std::borrow::Cow::Borrowed(include_bytes!("fonts/Geist-Medium.ttf")),
                std::borrow::Cow::Borrowed(include_bytes!("fonts/Geist-SemiBold.ttf")),
                std::borrow::Cow::Borrowed(include_bytes!("fonts/GeistMono.ttf")),
            ]) {
                eprintln!("Could not load bundled fonts: {e}");
            }
            cx.bind_keys([
                KeyBinding::new("tab", NextFocus, None),
                KeyBinding::new("shift-tab", PreviousFocus, None),
            ]);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
            let result = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(800.), px(560.))),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("OpenCam".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| cx.new(|cx| Studio::new(window, cx, pair, serial, video_device)),
            );
            if let Err(e) = result {
                eprintln!("Could not open OpenCam: {e}");
                cx.quit();
            }
            cx.activate(true);
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slider_log_round_trip_and_endpoints() {
        let slider = Slider {
            key: "iso",
            label: "ISO",
            min: 50.,
            max: 6400.,
            log: true,
        };
        assert_eq!(slider.value(0.), 50.);
        assert_eq!(slider.value(1.), 6400.);
        assert!((slider.fraction(slider.value(0.37)) - 0.37).abs() < 1e-9);
    }
}
