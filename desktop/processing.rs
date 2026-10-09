mod gpu;
mod ml;

use crate::session::{Frame, Shared};
use anyhow::{Result, ensure};
use rayon::prelude::*;
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

#[allow(dead_code)] // Shared with the headless probe, which has no studio settings editor.
pub const EFFECT_KEYS: &[&str] = &[
    "stretchX",
    "stretchY",
    "distortion",
    "bulge",
    "bulgeRadius",
    "bulgeX",
    "bulgeY",
    "backgroundBlur",
    "maskFps",
    "mlDelegate",
    "outputWidth",
    "outputHeight",
    "phoneRotation",
    "phoneMirror",
    "outputMode",
];
pub const DESKTOP_KEYS: &[&str] = &[
    "processingLocation",
    "desktopBackend",
    "desktopAdapter",
    "desktopMl",
];

#[derive(Clone, Debug)]
pub struct Options {
    pub desktop: bool,
    pub backend: String,
    pub adapter: String,
    pub ml: String,
    pub stretch: [f32; 2],
    pub distortion: f32,
    pub bulge: f32,
    pub radius: f32,
    pub center: [f32; 2],
    pub blur: f32,
    pub mask_fps: f32,
    pub width: u32,
    pub height: u32,
    pub rotation: u16,
    pub mirror: bool,
    pub fit: u32,
}
impl Default for Options {
    fn default() -> Self {
        Self::read(&json!({})).unwrap()
    }
}
impl Options {
    pub fn read(v: &Value) -> Result<Self> {
        let n = |key: &str, default: f32, min: f32, max: f32| -> Result<f32> {
            let x = match v.get(key) {
                Some(x) => x.as_f64().ok_or_else(|| anyhow::anyhow!("Invalid {key}"))? as f32,
                None => default,
            };
            ensure!(x.is_finite() && x >= min && x <= max, "Invalid {key}");
            Ok(x)
        };
        let text = |key: &str, default: &str, choices: &[&str]| -> Result<String> {
            let x = v
                .get(key)
                .map(|x| x.as_str())
                .unwrap_or(Some(default))
                .ok_or_else(|| anyhow::anyhow!("Invalid {key}"))?;
            ensure!(choices.contains(&x), "Invalid {key}");
            Ok(x.into())
        };
        let width = n("outputWidth", 0., 0., 8192.)?;
        let height = n("outputHeight", 0., 0., 8192.)?;
        ensure!(
            width.fract() == 0. && height.fract() == 0.,
            "Output dimensions must be integers"
        );
        if width != 0. || height != 0. {
            crate::output::Output {
                width: width as u32,
                height: height as u32,
                crop: false,
            }
            .geometry(2, 2)?;
        }
        let rotation = n("phoneRotation", 0., 0., 270.)?;
        ensure!(
            [0., 90., 180., 270.].contains(&rotation),
            "Invalid rotation"
        );
        let fit = n("outputMode", 0., 0., 2.)?;
        ensure!(fit.fract() == 0., "Invalid output mode");
        Ok(Self {
            desktop: text("processingLocation", "phone", &["phone", "desktop"])? == "desktop",
            backend: text("desktopBackend", "auto", &["auto", "gpu", "cpu"])?,
            adapter: v["desktopAdapter"]
                .as_str()
                .unwrap_or("auto")
                .chars()
                .take(256)
                .collect(),
            ml: text(
                "desktopMl",
                "auto",
                &[
                    "auto", "cpu", "cuda", "rocm", "migraphx", "openvino", "directml", "coreml",
                ],
            )?,
            stretch: [n("stretchX", 1., 0.25, 4.)?, n("stretchY", 1., 0.25, 4.)?],
            distortion: n("distortion", 0., -0.8, 0.8)?,
            bulge: n("bulge", 0., -0.8, 0.8)?,
            radius: n("bulgeRadius", 0.4, 0.05, 1.)?,
            center: [n("bulgeX", 0.5, 0., 1.)?, n("bulgeY", 0.5, 0., 1.)?],
            blur: n("backgroundBlur", 0., 0., 32.)?,
            mask_fps: n("maskFps", 10., 2., 30.)?,
            width: width as u32,
            height: height as u32,
            rotation: rotation as u16,
            mirror: v["phoneMirror"].as_bool().unwrap_or(false),
            fit: fit as u32,
        })
    }
    pub fn dimensions(&self, w: u32, h: u32) -> (u32, u32) {
        if self.width != 0 {
            (self.width, self.height)
        } else if self.rotation % 180 == 90 {
            (h, w)
        } else {
            (w, h)
        }
    }
    fn scale(&self, w: u32, h: u32) -> [f32; 2] {
        if self.fit == 2 {
            return [1., 1.];
        }
        let (sw, sh) = if self.rotation % 180 == 90 {
            (h, w)
        } else {
            (w, h)
        };
        let (tw, th) = self.dimensions(w, h);
        let ratio = (tw as f32 / th as f32) / (sw as f32 / sh as f32);
        if (ratio > 1.) == (self.fit == 0) {
            [ratio, 1.]
        } else {
            [1., 1. / ratio]
        }
    }
    fn identity(&self, w: u32, h: u32) -> bool {
        self.dimensions(w, h) == (w, h)
            && self.rotation == 0
            && !self.mirror
            && self.stretch == [1., 1.]
            && self.distortion == 0.
            && self.bulge == 0.
            && self.blur == 0.
    }
}

pub fn phone_settings(settings: &Value) -> Value {
    let mut v = settings.clone();
    if settings["processingLocation"] == "desktop" {
        for (key, value) in [
            ("stretchX", json!(1)),
            ("stretchY", json!(1)),
            ("distortion", json!(0)),
            ("bulge", json!(0)),
            ("backgroundBlur", json!(0)),
            ("outputWidth", json!(0)),
            ("outputHeight", json!(0)),
            ("phoneRotation", json!(0)),
            ("phoneMirror", json!(false)),
            ("outputMode", json!(0)),
        ] {
            v[key] = value;
        }
    }
    if let Some(map) = v.as_object_mut() {
        for key in DESKTOP_KEYS {
            map.remove(*key);
        }
    }
    v
}

fn sample(p: &[u8], w: u32, h: u32, uv: [f32; 2]) -> [f32; 4] {
    let x = (uv[0] * w as f32 - 0.5).clamp(0., w as f32 - 1.);
    let y = (uv[1] * h as f32 - 0.5).clamp(0., h as f32 - 1.);
    let (ix, iy) = (x.floor() as usize, y.floor() as usize);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let get = |x: usize, y: usize, c: usize| p[(y * w as usize + x) * 4 + c] as f32 / 255.;
    std::array::from_fn(|c| {
        let a = get(ix, iy, c) * (1. - fx) + get((ix + 1).min(w as usize - 1), iy, c) * fx;
        let b = get(ix, (iy + 1).min(h as usize - 1), c) * (1. - fx)
            + get(
                (ix + 1).min(w as usize - 1),
                (iy + 1).min(h as usize - 1),
                c,
            ) * fx;
        a * (1. - fy) + b * fy
    })
}
fn coordinate(o: &Options, w: u32, h: u32, x: u32, y: u32) -> Option<[f32; 2]> {
    let (tw, th) = o.dimensions(w, h);
    let scale = o.scale(w, h);
    let mut p = [
        (((x as f32 + 0.5) / tw as f32 - 0.5) * scale[0]) / o.stretch[0] + 0.5,
        (((y as f32 + 0.5) / th as f32 - 0.5) * scale[1]) / o.stretch[1] + 0.5,
    ];
    let r = [(p[0] - 0.5) * 2., (p[1] - 0.5) * 2.];
    let factor = 1. + o.distortion * (r[0] * r[0] + r[1] * r[1]);
    p = [0.5 + r[0] * factor * 0.5, 0.5 + r[1] * factor * 0.5];
    let d = [p[0] - o.center[0], p[1] - o.center[1]];
    let dist = (d[0] * d[0] + d[1] * d[1]).sqrt() / o.radius;
    if dist < 1. {
        let f = 1. - o.bulge * (1. - dist).powi(2);
        p = [o.center[0] + d[0] * f, o.center[1] + d[1] * f];
    }
    if p.iter().any(|v| *v < 0. || *v > 1.) {
        return None;
    }
    if o.mirror {
        p[0] = 1. - p[0];
    }
    Some(match o.rotation {
        90 => [p[1], 1. - p[0]],
        180 => [1. - p[0], 1. - p[1]],
        270 => [1. - p[1], p[0]],
        _ => p,
    })
}

fn blurred(pixels: &[u8], w: u32, h: u32, radius: f32) -> Vec<u8> {
    let mut horizontal = vec![0; pixels.len()];
    let mut vertical = vec![0; pixels.len()];
    let pass = |input: &[u8], output: &mut [u8], axis: u8| {
        output
            .par_chunks_exact_mut(w as usize * 4)
            .enumerate()
            .for_each(|(y, row)| {
                for x in 0..w as usize {
                    let mut color = [0.; 4];
                    for (offset, weight) in [
                        (-3., 0.064759),
                        (-2., 0.120985),
                        (-1., 0.176033),
                        (0., 0.199471),
                        (1., 0.176033),
                        (2., 0.120985),
                        (3., 0.064759),
                    ] {
                        let step = offset * (radius / 3.).max(1.);
                        let c = sample(
                            input,
                            w,
                            h,
                            [
                                (x as f32 + 0.5 + if axis == 0 { step } else { 0. }) / w as f32,
                                (y as f32 + 0.5 + if axis == 1 { step } else { 0. }) / h as f32,
                            ],
                        );
                        for i in 0..4 {
                            color[i] += c[i] * weight / 0.923025;
                        }
                    }
                    for i in 0..3 {
                        row[x * 4 + i] = (color[i] * 255.).round().clamp(0., 255.) as u8;
                    }
                    row[x * 4 + 3] = 255;
                }
            });
    };
    pass(pixels, &mut horizontal, 0);
    pass(&horizontal, &mut vertical, 1);
    vertical
}
fn cpu(frame: &Frame, o: &Options, mask: Option<&[f32]>) -> Frame {
    let (w, h) = o.dimensions(frame.width, frame.height);
    if o.identity(frame.width, frame.height) {
        return frame.clone();
    }
    let blur = if o.blur > 0. {
        Some(blurred(&frame.pixels, frame.width, frame.height, o.blur))
    } else {
        None
    };
    let mut pixels = vec![0; w as usize * h as usize * 4];
    pixels
        .par_chunks_exact_mut(w as usize * 4)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w {
                let dst = &mut row[x as usize * 4..][..4];
                dst[3] = 255;
                if let Some(uv) = coordinate(o, frame.width, frame.height, x, y as u32) {
                    let mut c = sample(&frame.pixels, frame.width, frame.height, uv);
                    if let Some(blur) = &blur {
                        let blurred = sample(blur, frame.width, frame.height, uv);
                        let person = mask.map(|m| ml::sample_mask(m, uv)).unwrap_or(0.);
                        for i in 0..3 {
                            c[i] = blurred[i] * (1. - person) + c[i] * person;
                        }
                    }
                    for i in 0..3 {
                        dst[i] = (c[i] * 255.).round().clamp(0., 255.) as u8;
                    }
                }
            }
        });
    Frame {
        pixels,
        width: w,
        height: h,
        sequence: frame.sequence,
    }
}

pub struct Worker {
    slot: Arc<(Mutex<Option<(Frame, Instant)>>, Condvar)>,
    stop: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Worker {
    pub fn start(shared: Arc<Shared>, mut sink: Option<crate::webcam::Webcam>) -> Self {
        let slot = Arc::new((Mutex::new(None::<(Frame, Instant)>), Condvar::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicU64::new(0));
        let (queue, stopped, drops) = (slot.clone(), stop.clone(), dropped.clone());
        let worker = thread::spawn(move || {
            let mut gpu = None;
            let mut device_key = String::new();
            let mut chosen = String::new();
            let mut cpu_ms = None;
            let mut gpu_ms = None;
            let mut error = String::new();
            let mut segmenter = None;
            let mut last_mask = Instant::now() - Duration::from_secs(1);
            let mut report_time = Instant::now();
            let mut adapters = None;
            let mut processed = 0u64;
            let mut interval = Instant::now();
            loop {
                let mut lock = queue.0.lock().unwrap();
                while lock.is_none() && !stopped.load(Ordering::Relaxed) {
                    lock = queue.1.wait(lock).unwrap();
                }
                if stopped.load(Ordering::Relaxed) {
                    break;
                }
                let (frame, queued) = lock.take().unwrap();
                drop(lock);
                let queue_ms = queued.elapsed().as_secs_f64() * 1000.;
                let started = Instant::now();
                let o = shared.processing.lock().unwrap().clone();
                if o.backend != "cpu" && adapters.is_none() {
                    adapters = Some(gpu::adapters());
                }
                if o.blur > 0. {
                    if segmenter
                        .as_ref()
                        .is_none_or(|s: &ml::Worker| s.provider != o.ml)
                    {
                        segmenter = Some(ml::Worker::start(o.ml.clone()));
                    }
                    if last_mask.elapsed().as_secs_f32() >= 1. / o.mask_fps {
                        if segmenter.as_ref().unwrap().offer(&frame) {
                            last_mask = Instant::now();
                        }
                    }
                } else {
                    segmenter = None;
                }
                let mask = segmenter.as_ref().and_then(|s| s.mask());
                let output = if o.identity(frame.width, frame.height) {
                    frame.clone()
                } else {
                    let key = format!(
                        "{}|{}|{}x{}|{}x{}|{}",
                        o.backend,
                        o.adapter,
                        frame.width,
                        frame.height,
                        o.width,
                        o.height,
                        o.blur > 0.
                    );
                    if key != device_key {
                        device_key = key;
                        chosen = "CPU".into();
                        gpu = None;
                        cpu_ms = None;
                        gpu_ms = None;
                        error.clear();
                        if o.backend != "cpu" {
                            match gpu::Gpu::new(&o.adapter) {
                                Ok(g) => gpu = Some(g),
                                Err(e) => error = format!("GPU unavailable: {e:#}"),
                            }
                            if let Some(g) = &mut gpu {
                                let _ = g.process(&frame, &o, mask.as_deref());
                                let t = Instant::now();
                                match (0..3).try_for_each(|_| {
                                    g.process(&frame, &o, mask.as_deref()).map(|_| ())
                                }) {
                                    Ok(_) => {
                                        gpu_ms = Some(t.elapsed().as_secs_f64() * 1000. / 3.);
                                        chosen = g.name.clone();
                                    }
                                    Err(e) => {
                                        error = format!("GPU failed: {e:#}");
                                        gpu = None;
                                    }
                                }
                            }
                            if o.backend == "auto" {
                                let _ = cpu(&frame, &o, mask.as_deref());
                                let t = Instant::now();
                                for _ in 0..3 {
                                    let _ = cpu(&frame, &o, mask.as_deref());
                                }
                                let ms = t.elapsed().as_secs_f64() * 1000. / 3.;
                                cpu_ms = Some(ms);
                                if gpu_ms.is_none_or(|g| ms <= g) {
                                    chosen = "CPU".into();
                                }
                            }
                        }
                    }
                    if chosen != "CPU" {
                        match gpu.as_mut().unwrap().process(&frame, &o, mask.as_deref()) {
                            Ok(f) => f,
                            Err(e) => {
                                error = format!("GPU failed; using CPU: {e:#}");
                                chosen = "CPU".into();
                                cpu(&frame, &o, mask.as_deref())
                            }
                        }
                    } else {
                        cpu(&frame, &o, mask.as_deref())
                    }
                };
                let elapsed = started.elapsed().as_secs_f64() * 1000.;
                if let Some(sink) = &mut sink {
                    sink.offer(output.pixels.clone());
                }
                *shared.frame.lock().unwrap() = Some(output);
                processed += 1;
                if report_time.elapsed() >= Duration::from_millis(500) {
                    *shared.desktop_processing_report.lock().unwrap() = json!({"mode":"Desktop", "backend":if o.identity(frame.width,frame.height){"Bypass"}else{&chosen},"frameMs":elapsed,"queueMs":queue_ms,"outputFps":processed as f64/interval.elapsed().as_secs_f64(),"cpuMs":cpu_ms,"gpuMs":gpu_ms,"dropped":drops.load(Ordering::Relaxed),"error":error,"adapters":adapters.as_ref().unwrap_or(&json!([])),"ml":segmenter.as_ref().map(|s|s.report())});
                    processed = 0;
                    interval = Instant::now();
                    report_time = Instant::now();
                }
            }
        });
        Self {
            slot,
            stop,
            dropped,
            worker: Some(worker),
        }
    }
    pub fn offer(&self, frame: Frame) {
        if self
            .slot
            .0
            .lock()
            .unwrap()
            .replace((frame, Instant::now()))
            .is_some()
        {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
        self.slot.1.notify_one();
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let guard = self.slot.0.lock().unwrap();
        self.stop.store(true, Ordering::Relaxed);
        self.slot.1.notify_one();
        drop(guard);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[allow(dead_code)] // Invoked by the separate native diagnostic executable.
pub fn smoke() -> Result<Value> {
    let frame = Frame {
        pixels: (0..160 * 120)
            .flat_map(|i| {
                [
                    (i % 160) as u8,
                    (i / 160 * 2) as u8,
                    if i % 20 < 10 { 255 } else { 0 },
                    255,
                ]
            })
            .collect(),
        width: 160,
        height: 120,
        sequence: 1,
    };
    let o = Options::read(
        &json!({"processingLocation":"desktop","stretchX":1.8,"distortion":0.3,"bulge":0.4,"outputWidth":120,"outputHeight":120}),
    )?;
    let expected = cpu(&frame, &o, None);
    ensure!(expected.pixels != frame.pixels, "CPU warp unchanged");
    let mut g = gpu::Gpu::new("auto")?;
    let t = Instant::now();
    let actual = g.process(&frame, &o, None)?;
    ensure!(
        actual.width == 120 && actual.height == 120,
        "GPU dimensions"
    );
    let max_error = actual
        .pixels
        .iter()
        .zip(&expected.pixels)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    ensure!(max_error <= 2, "CPU/GPU differ by {max_error}");
    let warp_ms = t.elapsed().as_secs_f64() * 1000.;
    let mut b = o.clone();
    b.blur = 12.;
    let blurred = g.process(&frame, &b, None)?;
    ensure!(blurred.pixels != actual.pixels, "GPU blur unchanged");
    ensure!(
        blurred.pixels.chunks_exact(4).all(|p| p[3] == 255),
        "GPU alpha"
    );
    Ok(
        json!({"syntheticInput":true,"adapter":g.name,"gpuWarpMs":warp_ms,"cpuGpuMaxChannelDifference":max_error,"dimensions":[actual.width,actual.height],"segmentation":ml::smoke(&frame)?}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_effects_and_strips_desktop_controls_at_phone_boundary() {
        for bad in [
            json!({"stretchX":0}),
            json!({"outputWidth":721,"outputHeight":720}),
            json!({"phoneRotation":45}),
            json!({"desktopBackend":"cuda"}),
            json!({"backgroundBlur":33}),
        ] {
            assert!(Options::read(&bad).is_err());
        }
        let settings = json!({"processingLocation":"desktop","desktopBackend":"gpu","stretchX":2.,"backgroundBlur":12,"outputWidth":720,"outputHeight":720,"zoom":2.,"torch":true});
        let phone = phone_settings(&settings);
        assert_eq!(phone["stretchX"], 1);
        assert_eq!(phone["backgroundBlur"], 0);
        assert_eq!(phone["outputWidth"], 0);
        assert_eq!(phone["zoom"], 2.);
        assert_eq!(phone["torch"], true);
        assert!(phone.get("desktopBackend").is_none());
    }
    #[test]
    fn cpu_identity_rotation_and_letterbox_are_opaque() {
        let frame = Frame {
            pixels: vec![
                20, 40, 180, 255, 220, 20, 40, 255, 90, 160, 90, 255, 50, 60, 220, 255,
            ],
            width: 2,
            height: 2,
            sequence: 1,
        };
        assert_eq!(cpu(&frame, &Options::default(), None).pixels, frame.pixels);
        let rotated = cpu(
            &frame,
            &Options::read(&json!({"phoneRotation":90})).unwrap(),
            None,
        );
        assert_eq!(&rotated.pixels[..4], &frame.pixels[8..12]);
        let out = cpu(
            &frame,
            &Options::read(&json!({"outputWidth":8,"outputHeight":4})).unwrap(),
            None,
        );
        assert_eq!(&out.pixels[..4], &[0, 0, 0, 255]);
        assert!(out.pixels.chunks_exact(4).all(|p| p[3] == 255));
    }
}
