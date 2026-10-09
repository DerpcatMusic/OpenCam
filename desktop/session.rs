use crate::protocol::{self, Packets, Pairing};
use anyhow::{Context, Result, bail, ensure};
use ffmpeg_next as ffmpeg;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, ClientConnection, DigitallySignedStruct, SignatureScheme, StreamOwned};
use serde_json::{Value, json};
use std::{
    io::Read,
    net::{Shutdown, TcpStream, ToSocketAddrs},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

pub fn now_us() -> f64 {
    static ORIGIN: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    ORIGIN.get_or_init(Instant::now).elapsed().as_secs_f64() * 1e6
}

#[derive(Debug)]
struct PinnedCertificate {
    pin: [u8; 32],
    provider: Arc<rustls::crypto::CryptoProvider>,
}
impl ServerCertVerifier for PinnedCertificate {
    fn verify_server_cert(
        &self,
        end: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        if protocol::certificate_matches(end.as_ref(), &self.pin) {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(
                "Phone certificate does not match the pairing link".into(),
            ))
        }
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            signature,
            &self.provider.signature_verification_algorithms,
        )
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            signature,
            &self.provider.signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

pub fn connect(pair: &Pairing) -> Result<StreamOwned<ClientConnection, TcpStream>> {
    let address = pair
        .address
        .to_socket_addrs()?
        .next()
        .context("Phone address did not resolve")?;
    let socket = TcpStream::connect_timeout(&address, Duration::from_secs(5))
        .context("Phone unreachable. Enable OpenCam and check Wi-Fi or USB forwarding")?;
    socket.set_nodelay(true)?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    socket.set_write_timeout(Some(Duration::from_secs(3)))?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedCertificate {
            pin: pair.pin,
            provider,
        }))
        .with_no_client_auth();
    let connection = ClientConnection::new(
        Arc::new(config),
        ServerName::try_from("opencam")?.to_owned(),
    )?;
    let mut stream = StreamOwned::new(connection, socket);
    while stream.conn.is_handshaking() {
        stream.conn.complete_io(&mut stream.sock)?;
    }
    protocol::write_json(
        &mut stream,
        &json!({"type":"hello", "protocol":1, "token":pair.token}),
    )?;
    stream
        .sock
        .set_read_timeout(Some(Duration::from_millis(20)))?;
    Ok(stream)
}

#[derive(Default, Clone, Debug)]
pub struct Stats {
    pub frames: u64,
    pub bytes: u64,
    pub age_sum_ms: f64,
    pub age_samples: u64,
    pub last_age_ms: Option<f64>,
    pub last_frame_us: f64,
    pub rtt_ms: Option<f64>,
    pub clock_offset_us: Option<f64>,
}

#[derive(Clone)]
pub struct Frame {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub sequence: u64,
}

pub struct Shared {
    pub frame: Mutex<Option<Frame>>,
    pub stats: Mutex<Stats>,
    pub video_device: Option<String>,
    pub camera_enabled: AtomicBool,
    pub rotation: Mutex<u16>,
    pub mirror: AtomicBool,
    pub output: Mutex<crate::output::Output>,
    pub password: Mutex<String>,
    pub processing: Mutex<crate::processing::Options>,
    pub desktop_processing_report: Mutex<Value>,
}
impl Shared {
    pub fn new(video_device: Option<String>) -> Arc<Self> {
        Arc::new(Self {
            frame: Mutex::new(None),
            stats: Mutex::new(Stats::default()),
            camera_enabled: AtomicBool::new(video_device.is_some()),
            video_device,
            rotation: Mutex::new(0),
            mirror: AtomicBool::new(false),
            output: Mutex::new(crate::output::Output::default()),
            password: Mutex::new(String::new()),
            processing: Mutex::new(crate::processing::Options::default()),
            desktop_processing_report: Mutex::new(Value::Null),
        })
    }
}

struct Decoder {
    codec: ffmpeg::decoder::Video,
    scaler: Option<ffmpeg::software::scaling::Context>,
    shared: Arc<Shared>,
    width: u32,
    height: u32,
    rotation: u16,
    mirror: bool,
    scaled_width: u32,
    scaled_height: u32,
    output_width: u32,
    output_height: u32,
    timestamps_realtime: bool,
    sink: Option<crate::webcam::Webcam>,
    processor: Option<crate::processing::Worker>,
}

impl Decoder {
    fn start(message: &Value, shared: Arc<Shared>, events: mpsc::Sender<Value>) -> Result<Self> {
        let source_w = message["width"].as_u64().context("Missing stream width")?;
        let source_h = message["height"]
            .as_u64()
            .context("Missing stream height")?;
        ensure!(
            source_w <= 8192 && source_h <= 8192,
            "Unsafe source dimensions"
        );
        let phone_processed = message["phoneProcessed"].as_bool().unwrap_or(false);
        let processing = shared.processing.lock().unwrap().clone();
        let rotation = if phone_processed || processing.desktop {
            0
        } else {
            *shared.rotation.lock().unwrap()
        };
        let (w, h) = if rotation == 90 || rotation == 270 {
            (source_h as u32, source_w as u32)
        } else {
            (source_w as u32, source_h as u32)
        };
        let output = if processing.desktop {
            crate::output::Output::default()
        } else {
            *shared.output.lock().unwrap()
        };
        let (scaled_w, scaled_h, w, h) = output.geometry(w, h)?;
        let (scaled_width, scaled_height) = if rotation == 90 || rotation == 270 {
            (scaled_h, scaled_w)
        } else {
            (scaled_w, scaled_h)
        };
        let fps = message["fps"].as_u64().unwrap_or(30).clamp(1, 240) as u32;
        let timestamps_realtime = message["timestampsRealtime"].as_bool().unwrap_or(false);
        let length = (w as usize)
            .checked_mul(h as usize)
            .and_then(|n| n.checked_mul(4))
            .context("Frame dimensions overflow")?;
        ensure!(
            w > 0 && h > 0 && length <= 160 * 1024 * 1024,
            "Unsafe frame dimensions"
        );
        let id = match message["mime"].as_str() {
            Some("video/avc") => ffmpeg::codec::Id::H264,
            Some("video/hevc") => ffmpeg::codec::Id::HEVC,
            _ => bail!("Unsupported video codec"),
        };
        ffmpeg::init()?;
        let decoder = ffmpeg::decoder::find(id).context("FFmpeg was built without this decoder")?;
        let mut context = ffmpeg::codec::Context::new_with_codec(decoder);
        context.set_flags(ffmpeg::codec::Flags::LOW_DELAY);
        context.set_threading(ffmpeg::threading::Config::count(1));
        // Permit decoder padding while keeping returned visible dimensions exact.
        unsafe {
            (*context.as_mut_ptr()).max_pixels = allocation_pixels(source_w, source_h) as i64;
        }
        let codec = context.decoder().open_as(decoder)?.video()?;
        let sink = if shared.camera_enabled.load(Ordering::Relaxed) {
            let (sink_w, sink_h) = if processing.desktop {
                processing.dimensions(source_w as u32, source_h as u32)
            } else {
                (w, h)
            };
            match crate::webcam::Webcam::start(
                shared.video_device.as_deref().unwrap_or("auto"),
                sink_w,
                sink_h,
                fps,
                events.clone(),
            ) {
                Ok(sink) => {
                    let _ = events.send(json!({"type":"output_ready", "device":crate::webcam::device(shared.video_device.as_deref().unwrap_or("auto")).unwrap_or_default()}));
                    Some(sink)
                }
                Err(e) => {
                    let _ = events.send(json!({"type":"output_error", "message":e.to_string()}));
                    None
                }
            }
        } else {
            None
        };
        let mirror =
            !phone_processed && !processing.desktop && shared.mirror.load(Ordering::Relaxed);
        let (sink, processor) = if processing.desktop {
            (
                None,
                Some(crate::processing::Worker::start(shared.clone(), sink)),
            )
        } else {
            (sink, None)
        };
        Ok(Self {
            codec,
            scaler: None,
            shared,
            width: source_w as u32,
            height: source_h as u32,
            rotation,
            mirror,
            timestamps_realtime,
            scaled_width,
            scaled_height,
            output_width: w,
            output_height: h,
            sink,
            processor,
        })
    }
    fn push(&mut self, data: &[u8]) -> Result<()> {
        ensure!(data.len() > 12, "Truncated video packet");
        let pts = i64::try_from(u64::from_be_bytes(data[..8].try_into()?))?;
        let mut packet = ffmpeg::Packet::copy(&data[12..]);
        packet.set_pts(Some(pts));
        packet.set_dts(Some(pts));
        self.codec.send_packet(&packet)?;
        loop {
            let mut decoded = ffmpeg::frame::Video::empty();
            match self.codec.receive_frame(&mut decoded) {
                Ok(()) => {}
                Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => break,
                Err(ffmpeg::Error::Eof) => break,
                Err(error) => return Err(error.into()),
            }
            ensure!(
                decoded.width() == self.width && decoded.height() == self.height,
                "Encoded dimensions differ from the configured stream"
            );
            if self
                .scaler
                .as_ref()
                .is_none_or(|s| s.input().format != decoded.format())
            {
                self.scaler = Some(ffmpeg::software::scaling::Context::get(
                    decoded.format(),
                    self.width,
                    self.height,
                    ffmpeg::format::Pixel::BGRA,
                    self.scaled_width,
                    self.scaled_height,
                    ffmpeg::software::scaling::Flags::FAST_BILINEAR,
                )?);
            }
            let scaler = self.scaler.as_mut().unwrap();
            let space = match decoded.color_space() {
                ffmpeg::color::Space::BT709 => 1,
                ffmpeg::color::Space::BT2020NCL | ffmpeg::color::Space::BT2020CL => 9,
                ffmpeg::color::Space::Unspecified if self.height >= 720 => 1,
                _ => 5,
            };
            // Preserve the encoder's YUV matrix/range when converting to the GPU's BGRA format.
            unsafe {
                let coefficients = ffmpeg::ffi::sws_getCoefficients(space);
                let status = ffmpeg::ffi::sws_setColorspaceDetails(
                    scaler.as_mut_ptr(),
                    coefficients,
                    i32::from(decoded.color_range() == ffmpeg::color::Range::JPEG),
                    coefficients,
                    1,
                    0,
                    1 << 16,
                    1 << 16,
                );
                ensure!(status >= 0, "Could not preserve encoded color space");
            }
            let mut converted = ffmpeg::frame::Video::empty();
            scaler.run(&decoded, &mut converted)?;
            let mut pixels =
                Vec::with_capacity(self.scaled_width as usize * self.scaled_height as usize * 4);
            for row in converted
                .data(0)
                .chunks(converted.stride(0))
                .take(self.scaled_height as usize)
            {
                pixels.extend_from_slice(&row[..self.scaled_width as usize * 4]);
            }
            let (pixels, w, h) = transform(
                pixels,
                self.scaled_width,
                self.scaled_height,
                self.rotation,
                self.mirror,
            )?;
            let pixels =
                crate::output::compose(pixels, w, h, self.output_width, self.output_height)?;
            let (w, h) = (self.output_width, self.output_height);
            let mut stats = self.shared.stats.lock().unwrap();
            stats.frames += 1;
            let now = now_us();
            stats.last_frame_us = now;
            if let (true, Some(pts), Some(offset)) = (
                self.timestamps_realtime,
                decoded.pts(),
                stats.clock_offset_us,
            ) {
                let age = (now - (pts as f64 + offset)) / 1000.;
                if (0.0..10_000.).contains(&age) {
                    stats.last_age_ms = Some(age);
                    stats.age_sum_ms += age;
                    stats.age_samples += 1;
                }
            }
            let sequence = stats.frames;
            drop(stats);
            let frame = Frame {
                pixels,
                width: w,
                height: h,
                sequence,
            };
            if let Some(processor) = &self.processor {
                processor.offer(frame);
            } else {
                if let Some(sink) = &mut self.sink {
                    sink.offer(frame.pixels.clone());
                }
                *self.shared.frame.lock().unwrap() = Some(frame);
            }
        }
        Ok(())
    }
}

fn allocation_pixels(w: u64, h: u64) -> u64 {
    w.div_ceil(128) * 128 * h.div_ceil(128) * 128
}

#[cfg(test)]
mod allocation_tests {
    #[test]
    fn permits_decoder_padding_for_custom_output_but_stays_bounded() {
        assert_eq!(super::allocation_pixels(720, 720), 768 * 768);
        assert_eq!(super::allocation_pixels(1280, 720), 1280 * 768);
        assert_eq!(super::allocation_pixels(8192, 8192), 8192 * 8192);
    }
}
fn transform(
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    rotation: u16,
    mirror: bool,
) -> Result<(Vec<u8>, u32, u32)> {
    if rotation == 0 && !mirror {
        return Ok((pixels, width, height));
    }
    let image =
        image::RgbaImage::from_raw(width, height, pixels).context("Invalid pixel buffer")?;
    let mut image = match rotation {
        90 => image::imageops::rotate90(&image),
        180 => image::imageops::rotate180(&image),
        270 => image::imageops::rotate270(&image),
        _ => image,
    };
    if mirror {
        image::imageops::flip_horizontal_in_place(&mut image);
    }
    let (w, h) = image.dimensions();
    Ok((image.into_raw(), w, h))
}

pub struct Session {
    pub commands: mpsc::Sender<Value>,
    pub events: mpsc::Receiver<Value>,
    stopped: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    socket: Arc<Mutex<Option<TcpStream>>>,
}

impl Session {
    pub fn start(pair: Pairing, shared: Arc<Shared>) -> Self {
        let (commands, receiver) = mpsc::channel();
        let (sender, events) = mpsc::channel();
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let socket = Arc::new(Mutex::new(None));
        let active_socket = socket.clone();
        let worker = thread::spawn(move || {
            let result = run(pair, shared, receiver, sender.clone(), stop, active_socket);
            if let Err(error) = result {
                let _ = sender.send(json!({"type":"disconnected", "message":format!("{error:#}")}));
            }
        });
        Self {
            commands,
            events,
            stopped,
            worker: Some(worker),
            socket,
        }
    }
    pub fn send(&self, value: Value) {
        let _ = self.commands.send(value);
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Some(socket) = self.socket.lock().unwrap().take() {
            let _ = socket.shutdown(Shutdown::Both);
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        } else {
            // A canceled connection attempt exits before creating a decoder or webcam process.
            self.worker.take();
        }
    }
}

fn run(
    pair: Pairing,
    shared: Arc<Shared>,
    commands: mpsc::Receiver<Value>,
    events: mpsc::Sender<Value>,
    stop: Arc<AtomicBool>,
    active_socket: Arc<Mutex<Option<TcpStream>>>,
) -> Result<()> {
    let mut stream = connect(&pair)?;
    *active_socket.lock().unwrap() = Some(stream.sock.try_clone()?);
    let mut packets = Packets::default();
    let mut bytes = [0; 65536];
    let mut decoder: Option<Decoder> = None;
    let mut authenticated = false;
    let mut ping = Instant::now() - Duration::from_secs(1);
    let mut last_data = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        for mut command in commands.try_iter() {
            if matches!(command["type"].as_str(), Some("configure" | "controls")) {
                command["settings"] = crate::processing::phone_settings(&command["settings"]);
            }
            protocol::write_json(&mut stream, &command)?;
        }
        if authenticated && ping.elapsed() >= Duration::from_secs(1) {
            protocol::write_json(&mut stream, &json!({"type":"ping", "sent":now_us()}))?;
            ping = Instant::now();
        }
        match stream.read(&mut bytes) {
            Ok(0) => bail!("Phone closed the connection"),
            Ok(n) => {
                last_data = Instant::now();
                for (kind, payload) in packets.feed(&bytes[..n])? {
                    if kind == 2 {
                        shared.stats.lock().unwrap().bytes += payload.len() as u64;
                        if let Some(decoder) = &mut decoder {
                            decoder.push(&payload)?;
                        }
                    } else {
                        let message: Value = serde_json::from_slice(&payload)?;
                        match message["type"].as_str() {
                            Some("auth") => {
                                ensure!(
                                    message["iterations"].as_u64()
                                        == Some(crate::auth::ITERATIONS as u64),
                                    "Unsupported password challenge"
                                );
                                let salt = protocol::unhex::<16>(
                                    message["salt"].as_str().context("Invalid auth salt")?,
                                )?;
                                let challenge = protocol::unhex::<32>(
                                    message["challenge"]
                                        .as_str()
                                        .context("Invalid auth challenge")?,
                                )?;
                                let password = shared.password.lock().unwrap().clone();
                                let proof =
                                    crate::auth::proof(&password, &salt, &challenge, &pair.pin)?;
                                protocol::write_json(
                                    &mut stream,
                                    &json!({"type":"auth", "proof":proof}),
                                )?;
                            }
                            Some("capabilities") => authenticated = true,
                            Some("error") if !authenticated => bail!(
                                "{}",
                                message["message"]
                                    .as_str()
                                    .unwrap_or("Authentication failed")
                            ),
                            Some("configured") => {
                                decoder.take();
                                *shared.frame.lock().unwrap() = None;
                                shared.stats.lock().unwrap().last_age_ms = None;
                                decoder =
                                    Some(Decoder::start(&message, shared.clone(), events.clone())?);
                            }
                            Some("stopped") => {
                                decoder.take();
                                *shared.frame.lock().unwrap() = None;
                            }
                            Some("pong") => {
                                let sent =
                                    message["sent"].as_f64().context("Invalid pong timestamp")?;
                                let phone =
                                    message["phoneUs"].as_f64().context("Invalid phone clock")?;
                                let now = now_us();
                                let rtt = (now - sent) / 1000.;
                                let mut stats = shared.stats.lock().unwrap();
                                if rtt.is_finite()
                                    && rtt >= 0.
                                    && stats.rtt_ms.is_none_or(|best| rtt < best)
                                {
                                    stats.rtt_ms = Some(rtt);
                                    stats.clock_offset_us = Some((sent + now) / 2. - phone);
                                }
                            }
                            _ => {}
                        }
                        events.send(message)?;
                    }
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Err(error.into()),
        }
        ensure!(
            last_data.elapsed() < Duration::from_secs(15),
            "Phone stopped responding; reconnect"
        );
    }
    let _ = protocol::write_json(&mut stream, &json!({"type":"stop"}));
    let _ = stream.sock.shutdown(Shutdown::Both);
    Ok(())
}
