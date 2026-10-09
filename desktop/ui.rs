use super::*;
use gpui::{Anchor, AnyElement, Focusable, Pixels, Point, anchored, canvas, deferred, point, svg};
use std::{cell::Cell, rc::Rc};

pub(super) struct Menu {
    key: &'static str,
    options: Vec<(String, Value)>,
    position: Point<Pixels>,
    index: usize,
}
struct Tip(SharedString);
impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .bg(gpui::rgba(0x161616ed))
            .border_1()
            .border_color(rgb(LINE))
            .rounded(px(6.))
            .text_size(px(12.))
            .text_color(rgb(INK))
            .child(self.0.clone())
    }
}
fn icon(name: &str) -> impl IntoElement {
    svg()
        .path(format!("icons/{name}.svg"))
        .size(px(16.))
        .text_color(rgb(INK))
}
// Surface proportions and washes adapted from Zeron's surface_chrome and header_icon_button.
fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    enabled: bool,
) -> gpui_base::Button {
    let label = label.into();
    gpui_base::Button::new(id)
        .accessibility_label(label.clone())
        .selected(selected)
        .disabled(!enabled)
        .aria_selected(selected)
        .flex()
        .items_center()
        .justify_center()
        .h(px(28.))
        .px_2()
        .rounded(px(6.))
        .bg(gpui::hsla(0., 0., 1., if selected { 0.11 } else { 0.035 }))
        .text_color(rgb(if selected { ACCENT } else { INK }))
        .when(!enabled, |d| d.opacity(0.4))
        .when(enabled, |d| {
            d.cursor_pointer()
                .hover(|d| d.bg(gpui::hsla(0., 0., 1., 0.11)))
                .focus(|d| d.border_1().border_color(rgb(ACCENT)))
        })
        .when(!label.is_empty(), |d| d.child(label))
}
fn icon_button(
    id: impl Into<ElementId>,
    name: &str,
    label: &'static str,
    selected: bool,
    enabled: bool,
) -> gpui_base::Button {
    button(id, "", selected, enabled)
        .accessibility_label(label)
        .w(px(32.))
        .h(px(32.))
        .px_0()
        .child(
            svg()
                .path(format!("icons/{name}.svg"))
                .size(px(16.))
                .text_color(rgb(if selected { ACCENT } else { INK })),
        )
        .tooltip(move |_, cx| cx.new(|_| Tip(label.into())).into())
}
fn muted(text: impl Into<SharedString>) -> Div {
    div().text_color(rgb(MUTED)).child(text.into())
}
fn dock(id: &'static str) -> Stateful<Div> {
    div().id(id).flex().flex_col().min_w(px(0.)).flex_shrink_0()
}
fn has_mode(value: &Value, mode: u64) -> bool {
    value
        .as_array()
        .is_some_and(|a| a.iter().any(|v| v.as_u64() == Some(mode)))
}

impl Studio {
    fn slider(&self, slider: Slider, enabled: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let value = self.settings[slider.key].as_f64().unwrap_or(slider.min);
        let fraction = slider.fraction(value);
        let value_label = match slider.key {
            "exposureNs" => format!("{:.2} ms", value / 1e6),
            "iso" => format!("{value:.0}"),
            "ev" => format!("{value:+.0}"),
            "focus" => format!("{value:.2} D"),
            "zoom" | "stretchX" | "stretchY" => format!("{value:.2}×"),
            "maskFps" => format!("{value:.0} fps"),
            "bitrate" => format!("{:.1} Mbps", value / 1e6),
            _ => format!("{value:.2}"),
        };
        let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let measured = bounds.clone();
        let down_bounds = bounds.clone();
        let down = slider.clone();
        let movement = slider.clone();
        let keyboard = slider.clone();
        div()
            .mb_2()
            .when(!enabled, |d| d.opacity(0.4))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(slider.label)
                    .child(muted(value_label.clone())),
            )
            .child(
                div()
                    .id(slider.key)
                    .role(gpui::accesskit::Role::Slider)
                    .aria_label(format!("{}: {value_label}", slider.label))
                    .aria_numeric_value(value)
                    .aria_min_numeric_value(slider.min)
                    .aria_max_numeric_value(slider.max)
                    .tab_index(0)
                    .tab_stop(enabled)
                    .relative()
                    .h(px(20.))
                    .w_full()
                    .px_1()
                    .cursor_pointer()
                    .focus(|d| d.border_1().border_color(rgb(ACCENT)))
                    .child(
                        canvas(move |b, _, _| measured.set(b), |_, _, _, _| {})
                            .absolute()
                            .size_full(),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(0.))
                            .right(px(0.))
                            .top(px(9.))
                            .h(px(2.))
                            .bg(rgb(0x4b4d55)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(0.))
                            .top(px(9.))
                            .h(px(2.))
                            .w(relative(fraction as f32))
                            .bg(rgb(ACCENT)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(4.))
                            .left(relative(fraction as f32))
                            .ml(px(-4.))
                            .w(px(8.))
                            .h(px(12.))
                            .rounded_sm()
                            .bg(rgb(0xcdced4)),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, e: &gpui::MouseDownEvent, _, cx| {
                            if enabled {
                                let b = down_bounds.get();
                                this.dragging = Some(down.key);
                                this.slider_set(
                                    &down,
                                    f64::from(f32::from(e.position.x - b.origin.x))
                                        / f64::from(f32::from(b.size.width).max(1.)),
                                );
                                cx.notify();
                            }
                        }),
                    )
                    .on_mouse_move(cx.listener(move |this, e: &gpui::MouseMoveEvent, _, cx| {
                        if enabled && e.dragging() && this.dragging == Some(movement.key) {
                            let b = bounds.get();
                            this.slider_set(
                                &movement,
                                f64::from(f32::from(e.position.x - b.origin.x))
                                    / f64::from(f32::from(b.size.width).max(1.)),
                            );
                            cx.notify();
                        }
                    }))
                    .on_key_down(cx.listener(move |this, e: &gpui::KeyDownEvent, _, cx| {
                        let next = match e.keystroke.key.as_str() {
                            "left" | "down" => fraction - 0.01,
                            "right" | "up" => fraction + 0.01,
                            "home" => 0.,
                            "end" => 1.,
                            _ => return,
                        };
                        if enabled {
                            this.slider_set(&keyboard, next);
                            cx.notify();
                            cx.stop_propagation();
                        }
                    })),
            )
    }
    fn toggle_icon(
        &self,
        key: &'static str,
        name: &str,
        label: &'static str,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        icon_button(key, name, label, self.settings[key] == true, enabled)
            .aria_toggled(if self.settings[key] == true {
                gpui::accesskit::Toggled::True
            } else {
                gpui::accesskit::Toggled::False
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                if enabled {
                    this.toggle(key);
                    cx.notify();
                }
            }))
    }
    fn selector(
        &self,
        key: &'static str,
        label: &'static str,
        current: String,
        options: Vec<(String, Value)>,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let measured = bounds.clone();
        div().mb_2().child(muted(label)).child(
            button(
                format!("select-{key}"),
                "",
                self.menu.as_ref().is_some_and(|m| m.key == key),
                enabled,
            )
            .aria_label(format!("{label}: {current}"))
            .w_full()
            .relative()
            .mt_1()
            .justify_between()
            .child(current)
            .child(icon("chevron"))
            .child(
                canvas(move |b, _, _| measured.set(b), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                if enabled && !options.is_empty() {
                    let index = options
                        .iter()
                        .position(|(_, v)| {
                            if key == "size" {
                                v[0] == this.settings["width"] && v[1] == this.settings["height"]
                            } else {
                                if key == "outputSize" {
                                    let o = this.shared.output.lock().unwrap();
                                    v[0].as_u64() == Some(o.width as u64)
                                        && v[1].as_u64() == Some(o.height as u64)
                                } else {
                                    *v == this.settings[key]
                                }
                            }
                        })
                        .unwrap_or(0);
                    let b = bounds.get();
                    this.menu = Some(Menu {
                        key,
                        options: options.clone(),
                        position: point(b.origin.x, b.origin.y),
                        index,
                    });
                    window.focus(&this.menu_focus, cx);
                    cx.notify();
                }
            })),
        )
    }
    fn choose(
        &mut self,
        key: &'static str,
        value: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if key == "zebraLevel" {
            self.monitor.threshold = value.as_u64().unwrap_or(242).min(255) as u8;
            self.menu = None;
            return;
        }
        if key == "shutterAngle" {
            let ns = value.as_f64().unwrap_or(180.) / 360. * 1e9
                / self.settings["fps"].as_f64().unwrap_or(30.);
            let camera = self.camera();
            let min = camera["exposureNs"][0].as_f64().unwrap_or(1000.);
            let max = camera["exposureNs"][1]
                .as_f64()
                .unwrap_or(ns)
                .min(1e9 / self.settings["fps"].as_f64().unwrap_or(30.));
            self.settings["manual"] = json!(true);
            self.set("exposureNs", json!(ns.clamp(min, max).round() as u64));
            self.menu = None;
            return;
        }
        if key == "outputSize" {
            let output = crate::output::Output {
                width: value[0].as_u64().unwrap_or(0) as u32,
                height: value[1].as_u64().unwrap_or(0) as u32,
                crop: self.shared.output.lock().unwrap().crop,
            };
            self.set_output(output);
            self.format_inputs[1].update(cx, |state, cx| {
                state.set_value(
                    if output.width == 0 {
                        "Source".into()
                    } else {
                        format!("{}x{}", output.width, output.height)
                    },
                    window,
                    cx,
                )
            });
            self.menu = None;
            return;
        }
        if key == "rawSize" {
            self.settings["rawWidth"] = value[0].clone();
            self.settings["rawHeight"] = value[1].clone();
            self.dirty.get_or_insert(Instant::now());
            self.menu = None;
            return;
        }
        if key == "codec" {
            self.settings["codec"] = value.clone();
            if protocol::uncompressed(&value) && value == "opencam.i420" {
                self.settings["processingLocation"] = json!("desktop");
            }
            let camera = self.camera();
            let size = json!([self.settings["width"], self.settings["height"]]);
            if let Some(sizes) = camera["sizes"].as_array() {
                if !sizes.contains(&size) {
                    if let Some(size) = sizes.first() {
                        self.settings["width"] = size[0].clone();
                        self.settings["height"] = size[1].clone();
                    }
                }
            }
            if let Some(fps) = protocol::normal_fps(
                &camera,
                &json!([self.settings["width"], self.settings["height"]]),
            )
            .into_iter()
            .min_by_key(|f| f.abs_diff(self.settings["fps"].as_u64().unwrap_or(30)))
            {
                self.settings["fps"] = json!(fps);
            }
        }
        if key == "size" {
            self.settings["width"] = value[0].clone();
            self.settings["height"] = value[1].clone();
            let camera = self.camera();
            let normal = protocol::normal_fps(&camera, &value);
            if !normal.contains(&self.settings["fps"].as_u64().unwrap_or(30)) {
                if let Some(fps) = normal.into_iter().min_by_key(|f| f.abs_diff(30)) {
                    self.settings["fps"] = json!(fps);
                }
            }
        } else {
            self.settings[key] = value;
        }
        self.sync_processing();
        if matches!(
            key,
            "awb" | "noiseReduction" | "edgeMode" | "aberrationMode" | "lensCorrection"
        ) {
            self.dirty.get_or_insert(Instant::now());
        } else {
            if key == "fps"
                && !self.camera()["fpsRanges"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|r| {
                        r[0].as_f64().unwrap_or(0.) <= self.settings["fps"].as_f64().unwrap_or(30.)
                            && r[1].as_f64().unwrap_or(0.)
                                >= self.settings["fps"].as_f64().unwrap_or(30.)
                    })
            {
                self.settings["manual"] = json!(false);
                self.settings["focusAuto"] = json!(true);
                self.settings["awb"] = json!(1);
            }
            if self.streaming {
                self.start_stream();
            }
        }
        if key == "size" || key == "codec" {
            self.format_inputs[0].update(cx, |state, cx| {
                state.set_value(
                    format!("{}x{}", self.settings["width"], self.settings["height"]),
                    window,
                    cx,
                )
            });
        }
        if key == "fps" || key == "size" || key == "codec" {
            self.format_inputs[2].update(cx, |state, cx| {
                state.set_value(self.settings["fps"].to_string(), window, cx)
            });
        }
        self.menu = None;
    }
    fn set_output(&mut self, output: crate::output::Output) {
        let (mut w, mut h) = (
            self.settings["width"].as_u64().unwrap_or(1280) as u32,
            self.settings["height"].as_u64().unwrap_or(720) as u32,
        );
        if matches!(*self.shared.rotation.lock().unwrap(), 90 | 270) {
            std::mem::swap(&mut w, &mut h);
        }
        match output.geometry(w, h) {
            Ok(_) => {
                *self.shared.output.lock().unwrap() = output;
                if self.streaming {
                    self.start_stream();
                }
            }
            Err(e) => self.message(e.to_string(), true),
        }
    }
    pub(super) fn apply_format(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.connected() || self.benchmark.is_some() {
            return;
        }
        let text = self.format_inputs[index].read(cx).value().to_string();
        if index == 2 {
            let Ok(fps) = text.trim().parse::<u64>() else {
                self.message("Enter a whole-number frame rate", true);
                return;
            };
            let c = self.camera();
            let mode = json!([self.settings["width"], self.settings["height"]]);
            let high = c["highSpeed"].as_array().into_iter().flatten().any(|m| {
                m["size"] == mode
                    && m["fpsRanges"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|r| r[0].as_u64() == Some(fps) && r[1].as_u64() == Some(fps))
            });
            if !protocol::normal_fps(&c, &mode).contains(&fps) && !high {
                self.message("FPS is outside this sensor mode's advertised ranges", true);
                return;
            }
            self.choose("fps", json!(fps), window, cx);
        } else {
            match output::resolution(&text) {
                Ok((w, h)) if index == 0 => {
                    let size = json!([w, h]);
                    if !self.camera()["sizes"]
                        .as_array()
                        .is_some_and(|s| s.contains(&size))
                    {
                        self.message("Sensor does not advertise that size. Use Output resolution for custom dimensions.", true);
                        return;
                    }
                    self.choose("size", size, window, cx);
                }
                Ok((w, h)) => {
                    let crop = self.shared.output.lock().unwrap().crop;
                    self.set_output(output::Output {
                        width: w,
                        height: h,
                        crop,
                    });
                }
                Err(e) => self.message(e.to_string(), true),
            }
        }
    }
    fn edit_field(
        &self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        state: &gpui::Entity<gpui_base::input::InputState>,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_base::InputBase {
        let input = state.clone();
        let focused = state.read(cx).focus_handle(cx).is_focused(window);
        // The container and editor need separate focus handles for native key dispatch.
        let frame_focus = window
            .use_keyed_state(("input-frame-focus", state.entity_id()), cx, |_, cx| {
                cx.focus_handle()
            })
            .read(cx)
            .clone();
        gpui_base::InputBase::new(id)
            .accessibility_label(label)
            .focused(focused)
            .disabled(!enabled)
            .track_focus(&frame_focus)
            .flex()
            .items_center()
            .px_2()
            .rounded(px(6.))
            .bg(gpui::hsla(0., 0., 1., 0.055))
            .styles(|s| s.focused(|d| d.border_1().border_color(rgb(ACCENT))))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |_, _, window, cx| {
                    if enabled {
                        input.update(cx, |state, cx| state.focus(window, cx));
                    }
                }),
            )
            .child(gpui_base::Input::new(state))
    }
    fn format_combo(
        &self,
        index: usize,
        key: &'static str,
        label: &'static str,
        options: Vec<(String, Value)>,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let measured = bounds.clone();
        div().mb_3().child(muted(label)).child(
            gpui_base::Combobox::new(format!("combo-{key}"))
                .disabled(!enabled)
                .open(self.menu.as_ref().is_some_and(|m| m.key == key))
                .flex()
                .items_center()
                .mt_1()
                .gap_1()
                .relative()
                .child(
                    canvas(move |b, _, _| measured.set(b), |_, _, _, _| {})
                        .absolute()
                        .size_full(),
                )
                .child(
                    self.edit_field(
                        format!("edit-{key}"),
                        format!("{label}; type a value or open suggestions"),
                        &self.format_inputs[index],
                        enabled,
                        window,
                        cx,
                    )
                    .flex_1()
                    .min_w(px(0.))
                    .h(px(30.)),
                )
                .child(
                    icon_button(
                        format!("list-{key}"),
                        "chevron",
                        "Choose a value",
                        false,
                        enabled,
                    )
                    .w(px(24.))
                    .h(px(30.))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if enabled && !options.is_empty() {
                            let b = bounds.get();
                            this.menu = Some(Menu {
                                key,
                                options: options.clone(),
                                position: point(b.origin.x, b.origin.y),
                                index: 0,
                            });
                            window.focus(&this.menu_focus, cx);
                            cx.notify();
                        }
                    })),
                )
                .child(
                    icon_button(
                        format!("apply-{key}"),
                        "check",
                        "Apply typed value",
                        false,
                        enabled,
                    )
                    .w(px(24.))
                    .h(px(30.))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.apply_format(index, window, cx);
                        cx.notify();
                    })),
                ),
        )
    }
    fn stream_options(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let output = *self.shared.output.lock().unwrap();
        let ready = self.connected() && self.benchmark.is_none();
        let camera = self.camera();
        let sizes = camera["sizes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|s| (format!("{} × {}", s[0], s[1]), s.clone()))
            .collect();
        let mode = json!([self.settings["width"], self.settings["height"]]);
        let mut fps = protocol::normal_fps(&camera, &mode);
        for high in camera["highSpeed"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|h| h["size"] == mode)
        {
            for r in high["fpsRanges"].as_array().into_iter().flatten() {
                if r[0] == r[1] {
                    if let Some(f) = r[0].as_u64() {
                        fps.push(f);
                    }
                }
            }
        }
        fps.sort_unstable();
        fps.dedup();
        let codecs = self.catalog["codecs"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let codec = codecs.iter().find(|c| c["name"] == self.settings["codec"]);
        let min = codec
            .and_then(|c| c["bitrate"][0].as_f64())
            .unwrap_or(100_000.)
            .max(100_000.);
        let max = codec
            .and_then(|c| c["bitrate"][1].as_f64())
            .unwrap_or(50_000_000.)
            .min(200_000_000.)
            .max(min);
        div()
            .id("video-options")
            .p_3()
            .child(self.format_combo(0, "size", "Sensor resolution", sizes, ready, window, cx))
            .child(
                self.format_combo(
                    2,
                    "fps",
                    "Frame rate",
                    fps.into_iter()
                        .map(|f| (format!("{f} fps"), json!(f)))
                        .collect(),
                    ready,
                    window,
                    cx,
                ),
            )
            .child(
                self.format_combo(
                    1,
                    "outputSize",
                    "Output resolution",
                    [
                        ("Source", 0, 0),
                        ("1920 × 1080 · 16:9", 1920, 1080),
                        ("1440 × 1080 · 4:3", 1440, 1080),
                        ("1080 × 1080 · 1:1", 1080, 1080),
                        ("1080 × 1920 · 9:16", 1080, 1920),
                        ("1620 × 1080 · 3:2", 1620, 1080),
                    ]
                    .into_iter()
                    .map(|(label, w, h)| (label.into(), json!([w, h])))
                    .collect(),
                    ready,
                    window,
                    cx,
                ),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .mb_4()
                    .child(
                        button(
                            "fit-output",
                            "Fit",
                            self.settings["outputMode"] != 2 && !output.crop,
                            ready,
                        )
                        .flex_1()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if ready {
                                let mut o = *this.shared.output.lock().unwrap();
                                o.crop = false;
                                this.settings["outputMode"] = json!(0);
                                this.set_output(o);
                                cx.notify();
                            }
                        })),
                    )
                    .child(
                        button(
                            "crop-output",
                            "Crop",
                            self.settings["outputMode"] != 2 && output.crop,
                            ready,
                        )
                        .flex_1()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if ready {
                                let mut o = *this.shared.output.lock().unwrap();
                                o.crop = true;
                                this.settings["outputMode"] = json!(1);
                                this.set_output(o);
                                cx.notify();
                            }
                        })),
                    )
                    .child(
                        button(
                            "stretch-output",
                            "Stretch",
                            self.settings["outputMode"] == 2,
                            ready,
                        )
                        .flex_1()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if ready {
                                this.settings["outputMode"] = json!(2);
                                this.start_stream();
                                cx.notify();
                            }
                        })),
                    ),
            )
            .child(
                self.selector(
                    "codec",
                    "Transport format",
                    codec
                        .and_then(|c| c["label"].as_str())
                        .unwrap_or("—")
                        .into(),
                    codecs
                        .iter()
                        .map(|c| {
                            (
                                format!(
                                    "{} · {}",
                                    c["label"].as_str().unwrap_or(""),
                                    c["name"].as_str().unwrap_or("")
                                ),
                                c["name"].clone(),
                            )
                        })
                        .collect(),
                    ready,
                    cx,
                ),
            )
            .children((!protocol::uncompressed(&self.settings["codec"])).then(|| {
                self.slider(
                    Slider {
                        key: "bitrate",
                        label: "Bitrate",
                        min,
                        max,
                        log: true,
                    },
                    ready,
                    cx,
                )
            }))
            .children((camera["raw"] == true).then(|| {
                self.selector(
                    "rawSize",
                    "RAW resolution",
                    if self.settings["rawWidth"].as_u64().unwrap_or(0) > 0 {
                        format!(
                            "{} × {}",
                            self.settings["rawWidth"], self.settings["rawHeight"]
                        )
                    } else {
                        "Full sensor".into()
                    },
                    camera["rawSizes"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|size| (format!("{} × {}", size[0], size[1]), size.clone()))
                        .collect(),
                    ready,
                    cx,
                )
            }))
            .child(
                button("apply-stream", "Apply stream", false, ready)
                    .w_full()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if ready {
                            this.start_stream();
                            cx.notify();
                        }
                    })),
            )
            .children(self.measurements.iter().map(|r| {
                div()
                    .mt_2()
                    .flex()
                    .justify_between()
                    .child(r.label.clone())
                    .child(muted(if r.error.is_some() {
                        "Failed".into()
                    } else {
                        format!(
                            "{:.1} fps / {}",
                            r.decoded_fps,
                            r.estimated_age_ms
                                .map(|a| format!("~{a:.1} ms"))
                                .unwrap_or("—".into())
                        )
                    }))
            }))
    }
    fn camera_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = self.camera();
        let ready = self.connected() && self.benchmark.is_none();
        let normal = c["fpsRanges"].as_array().into_iter().flatten().any(|r| {
            r[0].as_f64().unwrap_or(0.) <= self.settings["fps"].as_f64().unwrap_or(30.)
                && r[1].as_f64().unwrap_or(0.) >= self.settings["fps"].as_f64().unwrap_or(30.)
        });
        let manual = ready && normal && c["manualSensor"] == true;
        let focus = ready && normal && c["manualFocus"] == true;
        let range = |name: &str, fallback: (f64, f64)| {
            (
                c[name][0].as_f64().unwrap_or(fallback.0),
                c[name][1].as_f64().unwrap_or(fallback.1),
            )
        };
        let (iso_min, iso_max) = range("iso", (100., 100.));
        let (e_min, e_max) = range("exposureNs", (1000., 33_333_333.));
        let (z_min, z_max) = range("zoom", (1., 1.));
        let (ev_min, ev_max) = range("ev", (0., 0.));
        let awb_names = [
            "Manual gains",
            "Auto",
            "Incandescent",
            "Fluorescent",
            "Warm fluorescent",
            "Daylight",
            "Cloudy",
            "Twilight",
            "Shade",
        ];
        let awb = c["awbModes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| m.as_u64())
            .map(|m| {
                (
                    awb_names.get(m as usize).unwrap_or(&"Mode").to_string(),
                    json!(m),
                )
            })
            .collect();
        dock("camera-dock").child(
            div()
                .id("camera-controls")
                .flex_shrink_0()
                .p_3()
                .flex()
                .flex_col()
                .gap_4()
                .when(self.tool == Some(2), |d| {
                    d.child(
                        div()
                            .flex_shrink_0()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .mb_3()
                                    .child(self.toggle_icon(
                                        "manual",
                                        "sun",
                                        "Manual exposure",
                                        manual,
                                        cx,
                                    ))
                                    .child(self.toggle_icon(
                                        "aeLock",
                                        "lock",
                                        "Lock exposure",
                                        ready
                                            && c["aeLock"] == true
                                            && self.settings["manual"] != true,
                                        cx,
                                    )),
                            )
                            .child(self.slider(
                                Slider {
                                    key: "iso",
                                    label: "ISO",
                                    min: iso_min,
                                    max: iso_max,
                                    log: true,
                                },
                                manual,
                                cx,
                            ))
                            .child(
                                self.slider(
                                    Slider {
                                        key: "exposureNs",
                                        label: "Shutter",
                                        min: e_min,
                                        max: e_max.min(
                                            1e9 / self.settings["fps"].as_f64().unwrap_or(30.),
                                        ),
                                        log: true,
                                    },
                                    manual,
                                    cx,
                                ),
                            )
                            .child(
                                self.selector(
                                    "shutterAngle",
                                    "Shutter angle",
                                    format!(
                                        "{:.1}°",
                                        self.settings["exposureNs"].as_f64().unwrap_or(0.)
                                            * self.settings["fps"].as_f64().unwrap_or(30.)
                                            / 1e9
                                            * 360.
                                    ),
                                    [45., 90., 144., 172.8, 180., 270., 360.]
                                        .into_iter()
                                        .map(|a| (format!("{a}°"), json!(a)))
                                        .collect(),
                                    manual,
                                    cx,
                                ),
                            )
                            .child(self.slider(
                                Slider {
                                    key: "ev",
                                    label: "Exposure",
                                    min: ev_min,
                                    max: ev_max,
                                    log: false,
                                },
                                ready && ev_max > ev_min && self.settings["manual"] != true,
                                cx,
                            )),
                    )
                })
                .when(self.tool == Some(3), |d| {
                    d.child(
                        div()
                            .flex_shrink_0()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .mb_3()
                                    .child(self.toggle_icon(
                                        "focusAuto",
                                        "focus",
                                        "Autofocus",
                                        focus,
                                        cx,
                                    ))
                                    .child(self.toggle_icon(
                                        "torch",
                                        "light",
                                        "Torch",
                                        ready && c["flash"] == true,
                                        cx,
                                    )),
                            )
                            .child(self.slider(
                                Slider {
                                    key: "focus",
                                    label: "Focus",
                                    min: 0.,
                                    max: c["focusMax"].as_f64().unwrap_or(0.),
                                    log: false,
                                },
                                focus,
                                cx,
                            ))
                            .child(self.slider(
                                Slider {
                                    key: "zoom",
                                    label: "Zoom",
                                    min: z_min,
                                    max: z_max,
                                    log: true,
                                },
                                ready && z_max > z_min,
                                cx,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .mt_2()
                                    .child(self.toggle_icon(
                                        "ois",
                                        "optical",
                                        "Optical stabilization",
                                        ready && has_mode(&c["oisModes"], 1),
                                        cx,
                                    ))
                                    .child(self.toggle_icon(
                                        "stabilization",
                                        "video",
                                        "Video stabilization",
                                        ready && has_mode(&c["stabilizationModes"], 1),
                                        cx,
                                    )),
                            ),
                    )
                })
                .when(self.tool == Some(4), |d| {
                    d.child(
                        div()
                            .flex_shrink_0()
                            .min_w(px(0.))
                            .child(div().flex().gap_2().mb_3().child(self.toggle_icon(
                                "awbLock",
                                "lock",
                                "Lock white balance",
                                ready && c["awbLock"] == true,
                                cx,
                            )))
                            .child(
                                self.selector(
                                    "awb",
                                    "White balance",
                                    self.settings["awb"]
                                        .as_u64()
                                        .and_then(|m| awb_names.get(m as usize))
                                        .unwrap_or(&"—")
                                        .to_string(),
                                    awb,
                                    ready && normal,
                                    cx,
                                ),
                            )
                            .when(self.settings["awb"] == 0, |d| {
                                d.children((0..4).map(|i| {
                                    let v = self.settings["gains"][i].as_f64().unwrap_or(1.);
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .mb_1()
                                        .child(format!("{} {v:.2}", ["R", "G₁", "G₂", "B"][i]))
                                        .child(
                                            div()
                                                .flex()
                                                .gap_1()
                                                .child(
                                                    icon_button(
                                                        format!("gain-down-{i}"),
                                                        "minus",
                                                        "Decrease white balance gain",
                                                        false,
                                                        ready,
                                                    )
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        if ready {
                                                            this.settings["gains"][i] =
                                                                json!((v - 0.1).max(1.));
                                                            this.dirty
                                                                .get_or_insert(Instant::now());
                                                            cx.notify();
                                                        }
                                                    })),
                                                )
                                                .child(
                                                    icon_button(
                                                        format!("gain-up-{i}"),
                                                        "plus",
                                                        "Increase white balance gain",
                                                        false,
                                                        ready,
                                                    )
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        if ready {
                                                            this.settings["gains"][i] =
                                                                json!((v + 0.1).min(8.));
                                                            this.dirty
                                                                .get_or_insert(Instant::now());
                                                            cx.notify();
                                                        }
                                                    })),
                                                ),
                                        )
                                }))
                            }),
                    )
                }),
        )
    }
    fn sources(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editable = self.connected() && self.benchmark.is_none();
        dock("sources-dock")
            .flex_shrink_0()
            .children(
                self.discovery
                    .as_ref()
                    .into_iter()
                    .flat_map(|d| d.phones.values())
                    .map(|phone| {
                        let link = phone.link.clone();
                        button(
                            format!("discovered-{}", phone.name),
                            "",
                            self.pairing == phone.link,
                            true,
                        )
                        .mx_2()
                        .mt_2()
                        .justify_start()
                        .gap_2()
                        .accessibility_label(format!("Connect to {}", phone.name))
                        .child(icon(if phone.protected { "lock" } else { "wifi" }))
                        .child(phone.name.clone())
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.pairing = link.clone();
                                this.pairing_input.update(cx, |state, cx| {
                                    state.set_value(link.clone(), window, cx)
                                });
                                this.connect(false);
                                cx.notify();
                            },
                        ))
                    }),
            )
            .child(div().mx_3().mt_3().child(muted("Pairing link")))
            .child(
                self.edit_field(
                    "pairing-field",
                    "Phone pairing link",
                    &self.pairing_input,
                    true,
                    window,
                    cx,
                )
                .mx_3()
                .mt_1()
                .mb_3()
                .h(px(28.)),
            )
            .child(div().mx_3().mb_1().child(muted("Password")))
            .child(
                self.edit_field(
                    "phone-password",
                    "Phone password",
                    &self.password_input,
                    true,
                    window,
                    cx,
                )
                .mx_3()
                .mb_3()
                .h(px(28.)),
            )
            .child(
                div().id("lens-list").flex_shrink_0().px_2().children(
                    self.catalog["cameras"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .enumerate()
                        .map(|(i, c)| {
                            let camera = c.clone();
                            let selected = self.settings["camera"] == c["id"];
                            button(format!("lens-{i}"), "", selected, editable)
                                .w_full()
                                .justify_start()
                                .gap_2()
                                .mb_1()
                                .border_0()
                                .aria_label(format!(
                                    "{}: camera {}",
                                    c["label"].as_str().unwrap_or("Camera"),
                                    c["id"].as_str().unwrap_or("")
                                ))
                                .child(icon("camera"))
                                .child(c["label"].as_str().unwrap_or("Camera").to_string())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if editable {
                                        this.camera_select(camera.clone());
                                        cx.notify();
                                    }
                                }))
                        }),
                ),
            )
    }
    fn monitor_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_3()
            .children(
                [
                    ("zebra", "Zebras", self.monitor.zebra, 0),
                    ("peaking", "Focus peaking", self.monitor.peaking, 1),
                    ("palette", "False color", self.monitor.false_color, 2),
                    ("grid", "Frame guides", self.monitor.guides, 3),
                    ("chart", "Histogram", self.monitor.histogram, 4),
                ]
                .into_iter()
                .map(|(glyph, label, active, key)| {
                    button(format!("monitor-{key}"), "", active, true)
                        .w_full()
                        .justify_start()
                        .gap_2()
                        .mb_2()
                        .accessibility_label(label)
                        .child(icon(glyph))
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let state = match key {
                                0 => &mut this.monitor.zebra,
                                1 => &mut this.monitor.peaking,
                                2 => &mut this.monitor.false_color,
                                3 => &mut this.monitor.guides,
                                _ => &mut this.monitor.histogram,
                            };
                            *state = !*state;
                            cx.notify();
                        }))
                }),
            )
            .child(
                self.selector(
                    "zebraLevel",
                    "Zebra threshold",
                    format!("{:.0}%", f64::from(self.monitor.threshold) * 100. / 255.),
                    [70u32, 80, 90, 95, 100]
                        .into_iter()
                        .map(|v| (format!("{v}%"), json!((v * 255 + 50) / 100)))
                        .collect(),
                    true,
                    cx,
                ),
            )
    }
    fn output_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.shared.camera_enabled.load(Ordering::Relaxed);
        div()
            .p_3()
            .child(
                button("camera-output", "Virtual camera", active, true)
                    .w_full()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.shared
                            .camera_enabled
                            .fetch_xor(true, Ordering::Relaxed);
                        if this.streaming {
                            this.start_stream();
                        }
                        cx.notify();
                    })),
            )
            .child(div().mt_3().text_color(rgb(MUTED)).child(if active {
                self.camera_status.clone()
            } else {
                "Output disabled".into()
            }))
    }
    fn tool_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("tools")
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .py_2()
            .w(px(44.))
            .flex_shrink_0()
            .children(
                [
                    (0, "network", "Phone and lenses"),
                    (1, "format", "Format and codec"),
                    (2, "sun", "Exposure and shutter"),
                    (3, "focus", "Focus, zoom and torch"),
                    (4, "palette", "White balance"),
                    (5, "eye", "Monitoring aids"),
                    (6, "monitor", "Virtual camera output"),
                    (7, "crop", "Stretch and lens effects"),
                    (8, "photo", "ML background blur"),
                    (9, "settings", "Camera ISP processing"),
                ]
                .into_iter()
                .map(|(tool, glyph, label)| {
                    icon_button(
                        format!("tool-{tool}"),
                        glyph,
                        label,
                        self.tool == Some(tool),
                        true,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tool = if this.tool == Some(tool) {
                            None
                        } else {
                            Some(tool)
                        };
                        this.menu = None;
                        cx.notify();
                    }))
                }),
            )
    }
    fn processing_controls(&self, tool: u8, cx: &mut Context<Self>) -> impl IntoElement {
        let ready = self.connected() && self.benchmark.is_none();
        let camera = self.camera();
        let ml = &self.processing_report["ml"];
        let desktop = self.settings["processingLocation"] == "desktop";
        div()
            .p_3()
            .when(tool == 7 || tool == 8, |d| {
                d.child(self.selector(
                    "processingLocation",
                    "Process on",
                    if desktop { "Desktop" } else { "Phone" }.into(),
                    vec![
                        ("Phone".into(), json!("phone")),
                        ("Desktop".into(), json!("desktop")),
                    ],
                    ready,
                    cx,
                ))
            })
            .when(tool == 7 && desktop, |d| {
                let mut adapters = vec![("Automatic GPU".into(), json!("auto"))];
                adapters.extend(
                    self.processing_report["adapters"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|a| a["name"].as_str().map(|n| (n.into(), json!(n)))),
                );
                d.child(
                    self.selector(
                        "desktopBackend",
                        "Effects engine",
                        match self.settings["desktopBackend"].as_str() {
                            Some("gpu") => "GPU",
                            Some("cpu") => "CPU",
                            _ => "Auto benchmark",
                        }
                        .into(),
                        vec![
                            ("Auto benchmark".into(), json!("auto")),
                            ("GPU".into(), json!("gpu")),
                            ("CPU".into(), json!("cpu")),
                        ],
                        ready,
                        cx,
                    ),
                )
                .child(
                    self.selector(
                        "desktopAdapter",
                        "GPU",
                        match self.settings["desktopAdapter"].as_str() {
                            None | Some("auto") => "Automatic GPU",
                            Some(name) => name,
                        }
                        .into(),
                        adapters,
                        ready,
                        cx,
                    ),
                )
                .child(muted(format!(
                    "{} · {}",
                    self.processing_report["backend"]
                        .as_str()
                        .unwrap_or("Starting"),
                    self.processing_report["frameMs"]
                        .as_f64()
                        .map(|v| format!("{v:.1} ms"))
                        .unwrap_or("—".into())
                )))
                .when(
                    self.processing_report["error"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty()),
                    |d| {
                        d.child(muted(
                            self.processing_report["error"].as_str().unwrap().to_owned(),
                        ))
                    },
                )
            })
            .when(tool == 7, |d| {
                d.children(
                    [
                        ("stretchX", "Horizontal stretch", 0.25, 4.),
                        ("stretchY", "Vertical stretch", 0.25, 4.),
                        ("distortion", "Barrel / pincushion", -0.8, 0.8),
                        ("bulge", "Local magnification", -0.8, 0.8),
                        ("bulgeRadius", "Radius", 0.05, 1.),
                        ("bulgeX", "Center X", 0., 1.),
                        ("bulgeY", "Center Y", 0., 1.),
                    ]
                    .into_iter()
                    .map(|(key, label, min, max)| {
                        self.slider(
                            Slider {
                                key,
                                label,
                                min,
                                max,
                                log: false,
                            },
                            ready,
                            cx,
                        )
                        .into_any_element()
                    }),
                )
                .child(
                    icon_button(
                        "reset-geometry",
                        "refresh",
                        "Reset stretch and lens effects",
                        false,
                        ready,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        for (key, value) in [
                            ("stretchX", 1.),
                            ("stretchY", 1.),
                            ("distortion", 0.),
                            ("bulge", 0.),
                            ("bulgeRadius", 0.4),
                            ("bulgeX", 0.5),
                            ("bulgeY", 0.5),
                        ] {
                            this.set(key, json!(value));
                        }
                        cx.notify();
                    })),
                )
            })
            .when(tool == 8, |d| {
                d.child(self.slider(
                    Slider {
                        key: "backgroundBlur",
                        label: "Background blur",
                        min: 0.,
                        max: 32.,
                        log: false,
                    },
                    ready,
                    cx,
                ))
                .child(self.slider(
                    Slider {
                        key: "maskFps",
                        label: "Mask rate",
                        min: 2.,
                        max: 30.,
                        log: false,
                    },
                    ready,
                    cx,
                ))
                .child(
                    self.selector(
                        if desktop { "desktopMl" } else { "mlDelegate" },
                        "Inference",
                        self.settings[if desktop { "desktopMl" } else { "mlDelegate" }]
                            .as_str()
                            .unwrap_or("auto")
                            .into(),
                        (if desktop {
                            vec![
                                ("Auto benchmark", "auto"),
                                ("CPU", "cpu"),
                                ("NVIDIA CUDA", "cuda"),
                                ("AMD MIGraphX", "migraphx"),
                                ("AMD ROCm", "rocm"),
                                ("Intel OpenVINO", "openvino"),
                                ("DirectML", "directml"),
                                ("Apple CoreML", "coreml"),
                            ]
                        } else {
                            vec![("Auto benchmark", "auto"), ("CPU", "cpu"), ("GPU", "gpu")]
                        })
                        .into_iter()
                        .map(|(a, b)| (a.into(), json!(b)))
                        .collect(),
                        ready,
                        cx,
                    ),
                )
                .child(muted(format!(
                    "{} · {}",
                    self.processing_report["mode"]
                        .as_str()
                        .unwrap_or("Camera → encoder"),
                    ml["delegate"].as_str().unwrap_or("ML idle")
                )))
                .when(ml["error"].as_str().is_some_and(|s| !s.is_empty()), |d| {
                    d.child(muted(ml["error"].as_str().unwrap().to_owned()))
                })
                .children(
                    [
                        ("Inference", "inferenceMs"),
                        ("CPU", "cpuMs"),
                        ("GPU", "gpuMs"),
                        ("Mask age", "maskAgeMs"),
                    ]
                    .into_iter()
                    .map(|(label, key)| {
                        div()
                            .mt_1()
                            .flex()
                            .justify_between()
                            .child(muted(label))
                            .child(
                                ml[key]
                                    .as_f64()
                                    .map(|v| format!("{v:.1} ms"))
                                    .unwrap_or("—".into()),
                            )
                    }),
                )
            })
            .when(tool == 9, |d| {
                d.children(
                    [
                        ("noiseReduction", "Noise reduction", "noiseModes"),
                        ("edgeMode", "Edge enhancement", "edgeModes"),
                        ("aberrationMode", "Chromatic aberration", "aberrationModes"),
                        ("lensCorrection", "Lens correction", "lensCorrectionModes"),
                    ]
                    .into_iter()
                    .map(|(key, label, modes)| {
                        let values = camera[modes].as_array().cloned().unwrap_or_default();
                        let name = |v: i64| match v {
                            0 => "Off",
                            1 => "Fast",
                            2 => "High quality",
                            3 if key == "noiseReduction" => "Minimal",
                            3 | 4 => "Zero shutter lag",
                            _ => "Camera default",
                        };
                        let mut options = vec![("Camera default".into(), json!(-1))];
                        options.extend(
                            values
                                .iter()
                                .map(|v| (name(v.as_i64().unwrap_or(-1)).into(), v.clone())),
                        );
                        self.selector(
                            key,
                            label,
                            name(self.settings[key].as_i64().unwrap_or(-1)).into(),
                            options,
                            ready && !values.is_empty(),
                            cx,
                        )
                        .into_any_element()
                    }),
                )
            })
    }
    fn measured(&self, key: &str) -> Option<f64> {
        let camera = self.settings["camera"].as_str().unwrap_or("");
        self.metadata["physicalCameras"][camera][key]
            .as_f64()
            .or_else(|| self.metadata[key].as_f64())
    }
    fn hud(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let iso = self.measured("android.sensor.sensitivity");
        let shutter = self.measured("android.sensor.exposureTime");
        let zoom = self
            .measured("android.control.zoomRatio")
            .unwrap_or_else(|| self.settings["zoom"].as_f64().unwrap_or(1.));
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                button(
                    "hud-iso",
                    format!(
                        "ISO {}",
                        iso.map(|v| format!("{v:.0}")).unwrap_or("—".into())
                    ),
                    self.tool == Some(2),
                    self.connected(),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.tool = Some(2);
                    cx.notify();
                })),
            )
            .child(
                button(
                    "hud-shutter",
                    shutter
                        .map(|v| format!("1/{:.0} s", 1e9 / v.max(1.)))
                        .unwrap_or("— s".into()),
                    self.tool == Some(2),
                    self.connected(),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.tool = Some(2);
                    cx.notify();
                })),
            )
            .child(
                button(
                    "hud-zoom",
                    format!("{zoom:.2}×"),
                    self.tool == Some(3),
                    self.connected(),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.tool = Some(3);
                    cx.notify();
                })),
            )
            .child(self.toggle_icon(
                "torch",
                "light",
                "Toggle torch; watch the live video",
                self.connected() && self.camera()["flash"] == true,
                cx,
            ))
    }
    fn histogram(&self) -> impl IntoElement {
        let bins = self.monitor.bins;
        let max = bins.iter().copied().max().unwrap_or(1).max(1) as f32;
        div()
            .absolute()
            .left(px(12.))
            .bottom(px(12.))
            .w(px(176.))
            .h(px(48.))
            .px_2()
            .py_1()
            .rounded(px(6.))
            .bg(gpui::rgba(0x080808bf))
            .flex()
            .items_end()
            .gap(px(1.))
            .children(
                bins.into_iter()
                    .map(|v| div().flex_1().h(relative(v as f32 / max)).bg(rgb(0xc8c8ce))),
            )
    }
    fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
            let text: String = text
                .chars()
                .filter(|c| c.is_ascii() && !c.is_control())
                .take(1024)
                .collect();
            self.pairing_input
                .update(cx, |state, cx| state.set_value(text, window, cx));
        }
    }
    fn preview(&self) -> AnyElement {
        if let Some(image) = &self.preview {
            img(image.clone())
                .object_fit(ObjectFit::Contain)
                .size_full()
                .into_any_element()
        } else {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(0x53555d))
                .child(
                    svg()
                        .path("icons/camera-off.svg")
                        .size(px(36.))
                        .text_color(rgb(0x53555d)),
                )
                .into_any_element()
        }
    }
    fn menu_element(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(menu) = &self.menu else {
            return div().into_any_element();
        };
        let key = menu.key;
        deferred(
            div()
                .absolute()
                .inset_0()
                .child(
                    div()
                        .id("dismiss-menu")
                        .absolute()
                        .size_full()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| {
                                this.menu = None;
                                window.focus(&this.root_focus, cx);
                                cx.notify();
                            }),
                        ),
                )
                .child(
                    anchored()
                        .anchor(Anchor::BottomLeft)
                        .position(menu.position)
                        .snap_to_window_with_margin(px(6.))
                        .child(super::frost::frosted(
                            12.,
                            super::frost::MENU_BLUR,
                            div()
                                .id("selection-menu")
                                .track_focus(&self.menu_focus)
                                .occlude()
                                .w(px(310.))
                                .max_h(px(260.))
                                .overflow_y_scroll()
                                .p_1()
                                .bg(gpui::rgba(0x161616eb))
                                .rounded(px(12.))
                                .shadow_lg()
                                .on_key_down(cx.listener(
                                    |this, e: &gpui::KeyDownEvent, window, cx| {
                                        if let Some(m) = &mut this.menu {
                                            match e.keystroke.key.as_str() {
                                                "up" => m.index = m.index.saturating_sub(1),
                                                "down" => {
                                                    m.index = (m.index + 1).min(m.options.len() - 1)
                                                }
                                                "enter" | "space" => {
                                                    let key = m.key;
                                                    let v = m.options[m.index].1.clone();
                                                    this.choose(key, v, window, cx);
                                                    window.focus(&this.root_focus, cx);
                                                }
                                                "escape" => {
                                                    this.menu = None;
                                                    window.focus(&this.root_focus, cx);
                                                }
                                                _ => return,
                                            }
                                            cx.stop_propagation();
                                            cx.notify();
                                        }
                                    },
                                ))
                                .children(menu.options.iter().enumerate().map(
                                    |(i, (label, value))| {
                                        let value = value.clone();
                                        button(
                                            format!("option-{i}"),
                                            label.clone(),
                                            i == menu.index,
                                            true,
                                        )
                                        .w_full()
                                        .justify_start()
                                        .border_0()
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.choose(key, value.clone(), window, cx);
                                                window.focus(&this.root_focus, cx);
                                                cx.notify();
                                            }),
                                        )
                                    },
                                )),
                        )),
                ),
        )
        .into_any_element()
    }
}

impl Studio {
    fn action_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ready = self.connected() && self.benchmark.is_none();
        let connected = self.connected();
        div()
            .id("action-rail")
            .w(px(44.))
            .flex_shrink_0()
            .min_h(px(0.))
            .overflow_y_scroll()
            .py_2()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .rounded(px(12.))
            .bg(rgb(SURFACE))
            .child(
                icon_button("paste", "clipboard", "Paste pairing link", false, true).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.paste(window, cx);
                        cx.notify();
                    }),
                ),
            )
            .child(
                icon_button(
                    "wifi",
                    "wifi",
                    "Connect over Wi-Fi",
                    connected && self.usb_port.is_none(),
                    !self.connecting,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.connect(false);
                    cx.notify();
                })),
            )
            .child(
                icon_button(
                    "usb",
                    "usb",
                    "Connect over USB with ADB",
                    self.usb_port.is_some(),
                    !self.connecting,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.connect(true);
                    cx.notify();
                })),
            )
            .child(
                icon_button(
                    "disconnect",
                    "power",
                    "Disconnect",
                    false,
                    connected || self.connecting,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.disconnect();
                    this.message("Disconnected", false);
                    cx.notify();
                })),
            )
            .child(div().w(px(20.)).h(px(1.)).my_2().bg(rgb(LINE)))
            .child(
                icon_button(
                    "stream",
                    if self.streaming { "stop" } else { "play" },
                    if self.streaming {
                        "Stop streaming"
                    } else {
                        "Start streaming"
                    },
                    self.streaming,
                    ready,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if ready {
                        if this.streaming {
                            this.send(json!({"type":"stop"}));
                        } else {
                            this.start_stream();
                        }
                        cx.notify();
                    }
                })),
            )
            .child(
                icon_button("rotate", "rotate", "Rotate 90 degrees", false, ready).on_click(
                    cx.listener(move |this, _, _, cx| {
                        if ready {
                            let mut r = this.shared.rotation.lock().unwrap();
                            *r = (*r + 90) % 360;
                            drop(r);
                            if this.streaming {
                                this.start_stream();
                            }
                            cx.notify();
                        }
                    }),
                ),
            )
            .child(
                icon_button(
                    "mirror",
                    "flip",
                    "Mirror output",
                    self.shared.mirror.load(Ordering::Relaxed),
                    ready,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if ready {
                        this.shared.mirror.fetch_xor(true, Ordering::Relaxed);
                        if this.streaming {
                            this.start_stream();
                        }
                        cx.notify();
                    }
                })),
            )
            .child(div().w(px(20.)).h(px(1.)).my_2().bg(rgb(LINE)))
            .child(
                icon_button(
                    "benchmark",
                    "gauge",
                    "Benchmark all codecs and apply the best result",
                    false,
                    ready,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if ready {
                        this.benchmark_start();
                        cx.notify();
                    }
                })),
            )
            .child(
                icon_button("apply-bitrate", "refresh", "Apply bitrate", false, ready).on_click(
                    cx.listener(move |this, _, _, cx| {
                        if ready {
                            this.start_stream();
                            cx.notify();
                        }
                    }),
                ),
            )
            .child(
                icon_button(
                    "raw",
                    "photo",
                    "Capture sensor RAW DNG and download; pauses video briefly",
                    false,
                    ready && self.camera()["raw"] == true,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if ready && this.camera()["raw"] == true {
                        this.send(json!({"type":"raw"}));
                        cx.notify();
                    }
                })),
            )
            .child(
                icon_button(
                    "export",
                    "download",
                    "Export all capabilities and capture metadata",
                    false,
                    ready,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if ready {
                        this.export();
                        cx.notify();
                    }
                })),
            )
    }
}
impl Render for Studio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette_height = px((f32::from(window.viewport_size().height) - 108.).max(100.));
        let root = div()
            .id("opencam")
            .track_focus(&self.root_focus)
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .text_color(rgb(INK))
            .font_family("Geist")
            .text_size(px(12.))
            .on_action(cx.listener(|_: &mut Self, _: &NextFocus, window, cx| window.focus_next(cx)))
            .on_action(
                cx.listener(|_: &mut Self, _: &PreviousFocus, window, cx| window.focus_prev(cx)),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = None),
            )
            .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    this.menu = None;
                    cx.notify();
                }
            }));
        let stats = self.shared.stats.lock().unwrap().clone();
        let status = self
            .benchmark
            .as_ref()
            .map(|b| b.progress())
            .unwrap_or_else(|| self.status.clone());
        root.child(
            div()
                .h(px(38.))
                .flex_shrink_0()
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .border_b_1()
                .border_color(rgb(LINE))
                .child(
                    button(
                        "phone-name",
                        self.catalog["device"]
                            .as_str()
                            .unwrap_or("Connect phone")
                            .to_owned(),
                        self.tool == Some(0),
                        true,
                    )
                    .max_w(relative(0.45))
                    .overflow_hidden()
                    .text_ellipsis()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.tool = Some(0);
                        cx.notify();
                    })),
                )
                .child(self.hud(cx)),
        )
        .child(
            div()
                .flex_1()
                .min_h(px(0.))
                .relative()
                .flex()
                .child(self.action_rail(cx))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .relative()
                        .overflow_hidden()
                        .my_2()
                        .rounded(px(6.))
                        .bg(rgb(0))
                        .child(div().absolute().inset_0().child(self.preview()))
                        .when(self.monitor.histogram && self.preview.is_some(), |d| {
                            d.child(self.histogram())
                        }),
                )
                .child(self.tool_rail(cx))
                .when_some(self.tool, |d, tool| {
                    d.child(
                        div()
                            .absolute()
                            .top(px(16.))
                            .right(px(52.))
                            .w(px(264.))
                            .max_h(palette_height)
                            .child(super::frost::frosted(
                                12.,
                                super::frost::MENU_BLUR,
                                div()
                                    .id("tool-palette")
                                    .occlude()
                                    .max_h(palette_height)
                                    .overflow_y_scroll()
                                    .rounded(px(12.))
                                    .bg(gpui::rgba(0x111111f5))
                                    .shadow_lg()
                                    .when(tool == 0, |d| d.child(self.sources(window, cx)))
                                    .when(tool == 1, |d| d.child(self.stream_options(window, cx)))
                                    .when((2..=4).contains(&tool), |d| {
                                        d.child(self.camera_controls(cx))
                                    })
                                    .when(tool == 5, |d| d.child(self.monitor_controls(cx)))
                                    .when(tool == 6, |d| d.child(self.output_controls(cx)))
                                    .when(tool >= 7, |d| {
                                        d.child(self.processing_controls(tool, cx))
                                    }),
                            )),
                    )
                }),
        )
        .child(
            div()
                .h(px(28.))
                .flex_shrink_0()
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .id("status-text")
                        .min_w(px(0.))
                        .flex_1()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_color(rgb(if self.error { 0xf0a0a0 } else { MUTED }))
                        .child(status),
                )
                .child(
                    div()
                        .id("stream-stats")
                        .flex_shrink_0()
                        .ml_3()
                        .flex()
                        .gap_4()
                        .font_family("Geist Mono")
                        .text_size(px(11.))
                        .text_color(rgb(MUTED))
                        .tooltip(|_, cx| {
                            cx.new(|_| Tip("Sensor → decode age; excludes desktop effects and display scanout".into()))
                                .into()
                        })
                        .child(format!("{:.1} fps", if self.settings["processingLocation"]=="desktop" {self.processing_report["outputFps"].as_f64().unwrap_or(self.fps)}else{self.fps}))
                        .child(format!("{:.1} Mbps", self.mbps))
                        .child(
                            stats
                                .last_age_ms
                                .map(|a| format!("~{a:.1} ms"))
                                .unwrap_or("— ms".into()),
                        ),
                ),
        )
        .child(self.menu_element(cx))
    }
}
