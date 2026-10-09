#[cfg(target_os = "linux")]
mod linux {
    use anyhow::{Context, Result, ensure};
    use std::{
        io::Write,
        process::{Child, Command, Stdio},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
        time::Duration,
    };

    pub fn device(requested: &str) -> Result<String> {
        let valid = |name: &str| {
            let p = std::path::Path::new("/sys/class/video4linux").join(name);
            let virtual_node =
                std::fs::canonicalize(&p).is_ok_and(|p| p.starts_with("/sys/devices/virtual"));
            let label = std::fs::read_to_string(p.join("name"))
                .unwrap_or_default()
                .to_lowercase();
            virtual_node
                && (label.contains("loopback")
                    || label.contains("opencam")
                    || label.contains("dummy video"))
        };
        if requested == "auto" {
            for entry in std::fs::read_dir("/sys/class/video4linux")
                .into_iter()
                .flatten()
                .flatten()
            {
                let name = entry.file_name().to_string_lossy().into_owned();
                if valid(&name) {
                    return Ok(format!("/dev/{name}"));
                }
            }
            anyhow::bail!(
                "Virtual-camera driver missing. Install v4l2loopback, then load it with card_label=OpenCam exclusive_caps=1."
            );
        }
        ensure!(
            requested.starts_with("/dev/video")
                && requested[10..].bytes().all(|b| b.is_ascii_digit())
                && requested.len() > 10,
            "Select a /dev/videoN loopback device"
        );
        ensure!(
            valid(&requested[5..]),
            "That device is not a virtual loopback camera"
        );
        Ok(requested.into())
    }
    pub struct Webcam {
        child: Child,
        frame: Arc<Mutex<Option<Vec<u8>>>>,
        stop: Arc<AtomicBool>,
        writer: Option<thread::JoinHandle<()>>,
    }
    impl Webcam {
        pub fn start(
            path: &str,
            width: u32,
            height: u32,
            fps: u32,
            events: mpsc::Sender<serde_json::Value>,
        ) -> Result<Self> {
            let path = device(path)?;
            ensure!(
                path.starts_with("/dev/video")
                    && path[10..].chars().all(|c| c.is_ascii_digit())
                    && path.len() > 10,
                "Select a /dev/videoN loopback device"
            );
            let mut child = Command::new("ffmpeg")
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-f",
                    "rawvideo",
                    "-pixel_format",
                    "bgra",
                    "-video_size",
                    &format!("{width}x{height}"),
                    "-framerate",
                    &fps.to_string(),
                    "-i",
                    "pipe:0",
                    "-pix_fmt",
                    "yuv420p",
                    "-f",
                    "v4l2",
                    &path,
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .spawn()?;
            let mut input = child
                .stdin
                .take()
                .context("Virtual webcam input unavailable")?;
            let frame = Arc::new(Mutex::new(None::<Vec<u8>>));
            let frames = frame.clone();
            let stop = Arc::new(AtomicBool::new(false));
            let stopped = stop.clone();
            let writer = thread::spawn(move || {
                while !stopped.load(Ordering::Relaxed) {
                    let pixels = frames.lock().unwrap().take();
                    if let Some(pixels) = pixels {
                        if input.write_all(&pixels).is_err() {
                            let _ = events.send(serde_json::json!({"type":"output_error", "message":"Virtual-camera writer stopped; check loopback permissions and format support"}));
                            break;
                        }
                    } else {
                        thread::sleep(Duration::from_millis(2));
                    }
                }
            });
            Ok(Self {
                child,
                frame,
                stop,
                writer: Some(writer),
            })
        }
        pub fn offer(&mut self, pixels: Vec<u8>) {
            *self.frame.lock().unwrap() = Some(pixels);
        }
    }
    impl Drop for Webcam {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            let _ = self.child.kill();
            let _ = self.child.wait();
            if let Some(writer) = self.writer.take() {
                let _ = writer.join();
            }
        }
    }
}
#[cfg(target_os = "linux")]
pub use linux::{Webcam, device};

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod native {
    use anyhow::{Result, bail};
    use std::{
        ffi::{CStr, c_char, c_void},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
        time::Duration,
    };
    unsafe extern "C" {
        fn opencam_camera_create(w: u32, h: u32, error: *mut c_char, cap: usize) -> *mut c_void;
        fn opencam_camera_send(camera: *mut c_void, data: *const u8, len: usize) -> bool;
        fn opencam_camera_destroy(camera: *mut c_void);
    }
    pub fn device(_: &str) -> Result<String> {
        Ok(if cfg!(target_os = "windows") {
            "Unity Video Capture"
        } else {
            "OBS Virtual Camera"
        }
        .into())
    }
    pub struct Webcam {
        frame: Arc<Mutex<Option<Vec<u8>>>>,
        stop: Arc<AtomicBool>,
        writer: Option<thread::JoinHandle<()>>,
    }
    impl Webcam {
        pub fn start(
            _: &str,
            w: u32,
            h: u32,
            _: u32,
            events: mpsc::Sender<serde_json::Value>,
        ) -> Result<Self> {
            let frame = Arc::new(Mutex::new(None::<Vec<u8>>));
            let frames = frame.clone();
            let stop = Arc::new(AtomicBool::new(false));
            let stopped = stop.clone();
            let (ready, result) = mpsc::channel();
            let writer = thread::spawn(move || {
                let mut error = [0 as c_char; 512];
                let camera =
                    unsafe { opencam_camera_create(w, h, error.as_mut_ptr(), error.len()) };
                if camera.is_null() {
                    let message = unsafe { CStr::from_ptr(error.as_ptr()) }
                        .to_string_lossy()
                        .into_owned();
                    let _ = ready.send(Err(if message.is_empty() {
                        "Virtual-camera driver failed to start".into()
                    } else {
                        message
                    }));
                    return;
                }
                let _ = ready.send(Ok(()));
                #[cfg(target_os = "macos")]
                let mut scaler = ffmpeg_next::software::scaling::Context::get(
                    ffmpeg_next::format::Pixel::BGRA,
                    w,
                    h,
                    ffmpeg_next::format::Pixel::UYVY422,
                    w,
                    h,
                    ffmpeg_next::software::scaling::Flags::FAST_BILINEAR,
                )
                .ok();
                while !stopped.load(Ordering::Relaxed) {
                    let pixels = frames.lock().unwrap().take();
                    if let Some(pixels) = pixels {
                        #[cfg(target_os = "macos")]
                        let pixels = {
                            let mut source = ffmpeg_next::frame::Video::new(
                                ffmpeg_next::format::Pixel::BGRA,
                                w,
                                h,
                            );
                            let stride = source.stride(0);
                            for (src, dst) in pixels
                                .chunks_exact(w as usize * 4)
                                .zip(source.data_mut(0).chunks_mut(stride))
                            {
                                dst[..src.len()].copy_from_slice(src);
                            }
                            let mut out = ffmpeg_next::frame::Video::empty();
                            if scaler
                                .as_mut()
                                .is_none_or(|s| s.run(&source, &mut out).is_err())
                            {
                                let _=events.send(serde_json::json!({"type":"output_error","message":"Virtual-camera color conversion failed"}));
                                break;
                            }
                            out.data(0)
                                .chunks(out.stride(0))
                                .take(h as usize)
                                .flat_map(|row| row[..w as usize * 2].iter().copied())
                                .collect::<Vec<_>>()
                        };
                        if !unsafe { opencam_camera_send(camera, pixels.as_ptr(), pixels.len()) } {
                            let _=events.send(serde_json::json!({"type":"output_error","message":"Virtual-camera driver rejected a frame"}));
                            break;
                        }
                    } else {
                        thread::sleep(Duration::from_millis(2));
                    }
                }
                unsafe { opencam_camera_destroy(camera) };
            });
            match result.recv() {
                Ok(Ok(())) => Ok(Self {
                    frame,
                    stop,
                    writer: Some(writer),
                }),
                outcome => {
                    let _ = writer.join();
                    bail!(
                        "{}",
                        outcome
                            .ok()
                            .and_then(Result::err)
                            .unwrap_or("Virtual-camera initialization failed".into())
                    )
                }
            }
        }
        pub fn offer(&mut self, pixels: Vec<u8>) {
            *self.frame.lock().unwrap() = Some(pixels);
        }
    }
    impl Drop for Webcam {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(writer) = self.writer.take() {
                let _ = writer.join();
            }
        }
    }
}
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub use native::{Webcam, device};

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(target_os = "linux")]
    fn reject_physical_and_invalid_paths() {
        for path in ["/dev/video", "/dev/video../x", "pipe:0", "/dev/video999999"] {
            assert!(super::device(path).is_err());
        }
    }
}
