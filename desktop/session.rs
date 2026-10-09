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
        Arc, Condvar, Mutex,
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

fn connect(
    pair: &Pairing,
    active: &Mutex<Option<TcpStream>>,
) -> Result<StreamOwned<ClientConnection, TcpStream>> {
    let address = pair
        .address
        .to_socket_addrs()?
        .next()
        .context("Phone address did not resolve")?;
    let socket = TcpStream::connect_timeout(&address, Duration::from_secs(2))
        .context("Phone unreachable. Enable OpenCam and check Wi-Fi or USB forwarding")?;
    *active.lock().unwrap() = Some(socket.try_clone()?);
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
    codec: Option<ffmpeg::decoder::Video>,
    raw: Option<ffmpeg::format::Pixel>,
    raw_frame: Option<ffmpeg::frame::Video>,
    converted: ffmpeg::frame::Video,
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
        ffmpeg::init()?;
        let raw = match message["mime"].as_str() {
            Some("video/x-opencam-i420") => Some(ffmpeg::format::Pixel::YUV420P),
            Some("video/x-opencam-rgba") => Some(ffmpeg::format::Pixel::RGBA),
            _ => None,
        };
        let codec = if raw.is_some() {
            None
        } else {
            let id = match message["mime"].as_str() {
                Some("video/avc") => ffmpeg::codec::Id::H264,
                Some("video/hevc") => ffmpeg::codec::Id::HEVC,
                _ => bail!("Unsupported video codec"),
            };
            let decoder =
                ffmpeg::decoder::find(id).context("FFmpeg was built without this decoder")?;
            let mut context = ffmpeg::codec::Context::new_with_codec(decoder);
            context.set_flags(ffmpeg::codec::Flags::LOW_DELAY);
            context.set_threading(ffmpeg::threading::Config::count(1));
            // Permit decoder padding while keeping returned visible dimensions exact.
            unsafe {
                (*context.as_mut_ptr()).max_pixels = allocation_pixels(source_w, source_h) as i64;
            }
            Some(context.decoder().open_as(decoder)?.video()?)
        };
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
            raw,
            raw_frame: None,
            converted: ffmpeg::frame::Video::empty(),
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
        if let Some(format) = self.raw {
            let expected = crate::media::frame_bytes(
                self.width,
                self.height,
                format == ffmpeg::format::Pixel::RGBA,
            )?;
            ensure!(
                data.len() == 12 + expected,
                "Uncompressed frame size mismatch"
            );
            let mut decoded = self
                .raw_frame
                .take()
                .unwrap_or_else(|| ffmpeg::frame::Video::new(format, self.width, self.height));
            let mut offset = 12;
            for plane in 0..decoded.planes() {
                let row_bytes = if format == ffmpeg::format::Pixel::RGBA {
                    self.width as usize * 4
                } else if plane == 0 {
                    self.width as usize
                } else {
                    self.width as usize / 2
                };
                let rows = if plane == 0 {
                    self.height as usize
                } else {
                    self.height as usize / 2
                };
                let stride = decoded.stride(plane);
                for row in decoded.data_mut(plane).chunks_mut(stride).take(rows) {
                    row[..row_bytes].copy_from_slice(&data[offset..offset + row_bytes]);
                    offset += row_bytes;
                }
            }
            decoded.set_pts(Some(pts));
            let data_space = u32::from_be_bytes(data[8..12].try_into()?);
            let standard = (data_space >> 16) & 63;
            decoded.set_color_space(match standard {
                1 => ffmpeg::color::Space::BT709,
                6 | 7 => ffmpeg::color::Space::BT2020NCL,
                _ => ffmpeg::color::Space::SMPTE170M,
            });
            // ponytail: Android 11/12 omit image dataspace; use BT.601 limited until a device-specific profile is supplied.
            decoded.set_color_range(
                if format == ffmpeg::format::Pixel::RGBA || (data_space >> 27) & 7 == 1 {
                    ffmpeg::color::Range::JPEG
                } else {
                    ffmpeg::color::Range::MPEG
                },
            );
            let result = self.present(&decoded);
            self.raw_frame = Some(decoded);
            return result;
        }
        let mut packet = ffmpeg::Packet::copy(&data[12..]);
        packet.set_pts(Some(pts));
        packet.set_dts(Some(pts));
        self.codec.as_mut().unwrap().send_packet(&packet)?;
        loop {
            let mut decoded = ffmpeg::frame::Video::empty();
            match self.codec.as_mut().unwrap().receive_frame(&mut decoded) {
                Ok(()) => self.present(&decoded)?,
                Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => break,
                Err(ffmpeg::Error::Eof) => break,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
    fn present(&mut self, decoded: &ffmpeg::frame::Video) -> Result<()> {
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
        scaler.run(decoded, &mut self.converted)?;
        let converted = &self.converted;
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
        let pixels = crate::output::compose(pixels, w, h, self.output_width, self.output_height)?;
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

#[derive(Default)]
struct DecodeQueue {
    frames: std::collections::VecDeque<Vec<u8>>,
    waiting_key: bool,
    flush: bool,
    stopped: bool,
}
impl DecodeQueue {
    fn offer(&mut self, data: Vec<u8>, independent: bool) -> bool {
        let key = independent
            || data
                .get(8..12)
                .is_some_and(|v| u32::from_be_bytes(v.try_into().unwrap()) & 1 != 0);
        let full = self.frames.len() >= 2;
        if full {
            self.frames.clear();
            self.waiting_key = !independent;
            self.flush = true;
        }
        if self.waiting_key && !key {
            return full;
        }
        self.waiting_key = false;
        self.frames.push_back(data);
        full && !independent
    }
}
struct DecodeWorker {
    queue: Arc<(Mutex<DecodeQueue>, Condvar)>,
    failed: Arc<AtomicBool>,
    independent: bool,
    worker: Option<thread::JoinHandle<()>>,
}
impl DecodeWorker {
    fn start(message: Value, shared: Arc<Shared>, events: mpsc::Sender<Value>) -> Self {
        let queue = Arc::new((Mutex::new(DecodeQueue::default()), Condvar::new()));
        let input = queue.clone();
        let failed = Arc::new(AtomicBool::new(false));
        let failure = failed.clone();
        let independent = message["mime"]
            .as_str()
            .is_some_and(|v| v.starts_with("video/x-opencam-"));
        let worker = thread::spawn(move || {
            let result = (|| -> Result<()> {
                let mut decoder = Decoder::start(&message, shared, events.clone())?;
                loop {
                    let (data, flush) = {
                        let (lock, ready) = &*input;
                        let mut q = lock.lock().unwrap();
                        while q.frames.is_empty() && !q.stopped {
                            q = ready.wait(q).unwrap();
                        }
                        if q.stopped {
                            break;
                        }
                        let flush = std::mem::take(&mut q.flush);
                        (q.frames.pop_front().unwrap(), flush)
                    };
                    if flush {
                        if let Some(codec) = &mut decoder.codec {
                            codec.flush();
                        }
                    }
                    decoder.push(&data)?;
                }
                Ok(())
            })();
            if let Err(error) = result {
                failure.store(true, Ordering::Relaxed);
                let _ = events
                    .send(json!({"type":"error","message":format!("Decode failed: {error:#}")}));
            }
        });
        Self {
            queue,
            failed,
            independent,
            worker: Some(worker),
        }
    }
    fn offer(&self, data: Vec<u8>) -> bool {
        let (lock, ready) = &*self.queue;
        let sync = lock.lock().unwrap().offer(data, self.independent);
        ready.notify_one();
        sync
    }
}
impl Drop for DecodeWorker {
    fn drop(&mut self) {
        let (lock, ready) = &*self.queue;
        lock.lock().unwrap().stopped = true;
        ready.notify_one();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn retry_delay(attempt: u32) -> Duration {
    Duration::from_millis((100u64 << attempt.min(5)).min(2000))
}
fn transient(error: &anyhow::Error, authenticated: bool) -> bool {
    error
        .chain()
        .filter_map(|e| e.downcast_ref::<std::io::Error>())
        .any(|e| {
            use std::io::ErrorKind::*;
            matches!(
                e.kind(),
                ConnectionRefused
                    | ConnectionAborted
                    | ConnectionReset
                    | NotConnected
                    | TimedOut
                    | BrokenPipe
            ) || (authenticated && e.kind() == UnexpectedEof)
        })
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
            let mut attempt = 0;
            loop {
                let mut authenticated = false;
                let result = run(
                    &pair,
                    shared.clone(),
                    &receiver,
                    sender.clone(),
                    stop.clone(),
                    active_socket.clone(),
                    &mut authenticated,
                );
                active_socket.lock().unwrap().take();
                if stop.load(Ordering::Relaxed) || result.is_ok() {
                    break;
                }
                let error = result.unwrap_err();
                if !transient(&error, authenticated) {
                    let _ =
                        sender.send(json!({"type":"disconnected", "message":format!("{error:#}")}));
                    break;
                }
                if authenticated {
                    attempt = 0;
                }
                let delay = retry_delay(attempt);
                attempt = attempt.saturating_add(1);
                let _ = sender.send(json!({"type":"reconnecting","attempt":attempt,"delayMs":delay.as_millis(),"message":format!("{error:#}")}));
                thread::park_timeout(delay);
                if stop.load(Ordering::Relaxed) {
                    break;
                }
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
        if let Some(worker) = &self.worker {
            worker.thread().unpark();
        }
        let socket = self.socket.lock().unwrap().take();
        if let Some(socket) = socket {
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
    pair: &Pairing,
    shared: Arc<Shared>,
    commands: &mpsc::Receiver<Value>,
    events: mpsc::Sender<Value>,
    stop: Arc<AtomicBool>,
    active_socket: Arc<Mutex<Option<TcpStream>>>,
    authenticated: &mut bool,
) -> Result<()> {
    let mut stream = connect(pair, &active_socket)?;
    *active_socket.lock().unwrap() = Some(stream.sock.try_clone()?);
    let mut packets = Packets::default();
    let mut bytes = [0; 65536];
    let mut decoder: Option<DecodeWorker> = None;
    let mut chunks = crate::media::FrameChunks::default();
    let mut raw_file: Option<crate::media::RawFile> = None;
    let mut ping = Instant::now() - Duration::from_secs(1);
    let mut last_data = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        for mut command in commands.try_iter() {
            if !*authenticated {
                continue;
            }
            if matches!(command["type"].as_str(), Some("configure" | "controls")) {
                command["settings"] = crate::processing::phone_settings(&command["settings"]);
            }
            protocol::write_json(&mut stream, &command)?;
        }
        if *authenticated && ping.elapsed() >= Duration::from_secs(1) {
            protocol::write_json(&mut stream, &json!({"type":"ping", "sent":now_us()}))?;
            ping = Instant::now();
        }
        match stream.read(&mut bytes) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Phone closed the connection",
                )
                .into());
            }
            Ok(n) => {
                last_data = Instant::now();
                for (kind, payload) in packets.feed(&bytes[..n])? {
                    if kind == 2 || kind == 3 {
                        ensure!(*authenticated, "Video arrived before authentication");
                        shared.stats.lock().unwrap().bytes += payload.len() as u64;
                        let frame = if kind == 3 {
                            chunks.push(&payload)?
                        } else {
                            Some(payload)
                        };
                        if let (Some(decoder), Some(frame)) = (&decoder, frame) {
                            if decoder.offer(frame) {
                                protocol::write_json(&mut stream, &json!({"type":"keyframe"}))?;
                            }
                        }
                    } else if kind == 4 {
                        ensure!(*authenticated, "RAW data arrived before authentication");
                        raw_file
                            .as_mut()
                            .context("RAW bytes without a descriptor")?
                            .push(&payload)?;
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
                            Some("capabilities") => {
                                *authenticated = true;
                                *shared.stats.lock().unwrap() = Stats::default();
                            }
                            Some("error") if !*authenticated => bail!(
                                "{}",
                                message["message"]
                                    .as_str()
                                    .unwrap_or("Authentication failed")
                            ),
                            Some("configured") => {
                                decoder.take();
                                *shared.frame.lock().unwrap() = None;
                                shared.stats.lock().unwrap().last_age_ms = None;
                                chunks.configure(&message)?;
                                decoder = Some(DecodeWorker::start(
                                    message.clone(),
                                    shared.clone(),
                                    events.clone(),
                                ));
                            }
                            Some("raw_file_begin") => {
                                raw_file = Some(crate::media::RawFile::begin(&message)?);
                            }
                            Some("raw_file_end") => {
                                let mut file = raw_file
                                    .take()
                                    .context("RAW completion without a descriptor")?;
                                let path = file.finish(&message)?;
                                let _ = events.send(json!({"type":"raw_downloaded","path":path,"message":format!("DNG saved to {path}")}));
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
        if decoder
            .as_ref()
            .is_some_and(|d| d.failed.load(Ordering::Relaxed))
        {
            bail!("Video decoder failed");
        }
        if last_data.elapsed() >= Duration::from_secs(3) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Phone heartbeat stopped",
            )
            .into());
        }
    }
    let _ = protocol::write_json(&mut stream, &json!({"type":"stop"}));
    let _ = stream.sock.shutdown(Shutdown::Both);
    Ok(())
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    #[test]
    fn uncompressed_pixels_preserve_colors_and_reuse_conversion_buffers() {
        let shared = Shared::new(None);
        let (events, _) = mpsc::channel();
        let mut decoder = Decoder::start(
            &json!({"width":2,"height":2,"fps":30,"mime":"video/x-opencam-i420"}),
            shared.clone(),
            events.clone(),
        )
        .unwrap();
        for (y, expected) in [(16u8, 0u8), (235, 255)] {
            let mut data = 1u64.to_be_bytes().to_vec();
            data.extend(0x10c10000u32.to_be_bytes());
            data.extend([y, y, y, y, 128, 128]);
            decoder.push(&data).unwrap();
            let frame = shared.frame.lock().unwrap();
            let pixels = &frame.as_ref().unwrap().pixels;
            // libswscale's scalar and SIMD paths differ by up to two levels at limited-range white.
            assert!(
                pixels
                    .chunks_exact(4)
                    .all(|p| p[..3].iter().all(|c| c.abs_diff(expected) <= 2) && p[3] == 255),
                "Expected neutral {expected}, got {pixels:?}"
            );
        }
        let mut decoder = Decoder::start(
            &json!({"width":2,"height":2,"fps":30,"mime":"video/x-opencam-rgba"}),
            shared.clone(),
            events,
        )
        .unwrap();
        let mut data = 1u64.to_be_bytes().to_vec();
        data.extend(0u32.to_be_bytes());
        data.extend([255, 0, 0, 255].repeat(4));
        decoder.push(&data).unwrap();
        assert!(
            shared
                .frame
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .pixels
                .chunks_exact(4)
                .all(|p| p == [0, 0, 255, 255])
        );
    }
    #[test]
    fn retries_are_fast_bounded_and_auth_failures_are_fatal() {
        assert_eq!(retry_delay(0), Duration::from_millis(100));
        assert_eq!(retry_delay(20), Duration::from_secs(2));
        assert!(!transient(&anyhow::anyhow!("Password incorrect"), true));
        let eof = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
        assert!(!transient(&eof, false));
        assert!(transient(&eof, true));
    }
    #[test]
    fn overflow_resumes_compressed_video_only_at_a_keyframe() {
        let frame = |key: u8| {
            let mut d = vec![0; 13];
            d[11] = key;
            d
        };
        let mut q = DecodeQueue::default();
        q.offer(frame(1), false);
        q.offer(frame(0), false);
        assert!(q.offer(frame(0), false));
        assert!(q.frames.is_empty());
        assert!(q.waiting_key);
        q.offer(frame(0), false);
        assert!(q.frames.is_empty());
        q.offer(frame(1), false);
        assert_eq!(q.frames.len(), 1);
        assert!(q.flush);
        let mut q = DecodeQueue::default();
        for _ in 0..20 {
            q.offer(frame(0), true);
        }
        assert!(q.frames.len() <= 2);
        assert!(!q.waiting_key);
    }
}
