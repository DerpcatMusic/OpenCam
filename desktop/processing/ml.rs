use super::{Frame, sample};
use anyhow::{Context, Result, ensure};
use ort::{
    ep::{self, ExecutionProvider},
    session::Session,
    value::Tensor,
};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const W: usize = 256;
const H: usize = 144;
fn init() -> Result<()> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| {
        let name = if cfg!(target_os = "windows") {
            "onnxruntime.dll"
        } else if cfg!(target_os = "macos") {
            "libonnxruntime.dylib"
        } else {
            "libonnxruntime.so"
        };
        let path = std::env::var_os("ORT_DYLIB_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::env::current_exe()
                    .unwrap_or_default()
                    .with_file_name(name)
            });
        ort::init_from(&path)
            .map_err(|e| format!("ML runtime missing at {}: {e}", path.display()))?
            .with_name("OpenCam segmentation")
            .commit();
        Ok(())
    })
    .clone()
    .map_err(anyhow::Error::msg)
}
fn tensor(frame: &Frame) -> Vec<f32> {
    let mut data = vec![0.; 3 * W * H];
    for y in 0..H {
        for x in 0..W {
            let rgb = sample(
                &frame.pixels,
                frame.width,
                frame.height,
                [(x as f32 + 0.5) / W as f32, (y as f32 + 0.5) / H as f32],
            );
            for c in 0..3 {
                data[c * W * H + y * W + x] = rgb[2 - c];
            }
        }
    }
    data
}
fn provider(name: &str) -> Result<Option<ep::ExecutionProviderDispatch>> {
    macro_rules! ep {
        ($kind:ident) => {{
            let p = ep::$kind::default();
            ensure!(
                p.is_available()?,
                "{name} is not provided by this ONNX Runtime"
            );
            Some(p.build().error_on_failure())
        }};
    }
    Ok(match name {
        "cuda" => ep!(CUDA),
        "rocm" => ep!(ROCm),
        "migraphx" => ep!(MIGraphX),
        "openvino" => ep!(OpenVINO),
        "directml" => ep!(DirectML),
        "coreml" => ep!(CoreML),
        "cpu" => None,
        _ => anyhow::bail!("Unknown inference backend"),
    })
}
fn session(name: &str) -> Result<Session> {
    let mut builder = Session::builder()?
        .with_intra_threads(2)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .with_intra_op_spinning(false)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .with_inter_op_spinning(false)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    if let Some(provider) = provider(name)? {
        builder = builder
            .with_execution_providers([provider])
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }
    if name == "directml" {
        builder = builder
            .with_memory_pattern(false)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }
    Ok(builder.commit_from_memory(include_bytes!("../models/selfie-landscape.onnx"))?)
}
fn infer(session: &mut Session, input: &[f32]) -> Result<Vec<f32>> {
    let output = session.run(ort::inputs![Tensor::from_array((
        [1usize, 3, H, W],
        input.to_vec().into_boxed_slice()
    ))?])?;
    let (_, mask) = output[0].try_extract_tensor::<f32>()?;
    ensure!(
        mask.len() == W * H,
        "Segmentation returned unexpected dimensions"
    );
    ensure!(
        mask.iter().all(|v| v.is_finite()),
        "Segmentation returned invalid values"
    );
    Ok(mask.iter().map(|v| v.clamp(0., 1.)).collect())
}
struct Model {
    session: Session,
    selected: String,
    benchmarks: Value,
    note: String,
}
impl Model {
    fn new(wanted: &str, input: &[f32]) -> Result<Self> {
        init()?;
        let candidates: Vec<_> = if wanted == "auto" {
            vec![
                "cpu", "cuda", "migraphx", "rocm", "openvino", "directml", "coreml",
            ]
        } else if wanted == "cpu" {
            vec!["cpu"]
        } else {
            vec![wanted, "cpu"]
        };
        let mut best = None;
        let mut scores = Vec::new();
        let mut note = String::new();
        for name in candidates {
            match session(name).and_then(|mut session| {
                let _ = infer(&mut session, input)?;
                let t = Instant::now();
                for _ in 0..3 {
                    let _ = infer(&mut session, input)?;
                }
                Ok((session, t.elapsed().as_secs_f64() * 1000. / 3.))
            }) {
                Ok((session, ms)) => {
                    scores.push(json!({"provider":name,"ms":ms}));
                    if wanted != "auto" && name == wanted {
                        best = Some((session, ms, name.to_string()));
                        break;
                    }
                    if best.as_ref().is_none_or(|(_, best_ms, _)| ms < *best_ms) {
                        best = Some((session, ms, name.to_string()));
                    }
                }
                Err(e) => {
                    if name == wanted {
                        note = format!("{name} unavailable; CPU fallback: {e:#}");
                    }
                    scores.push(json!({"provider":name,"error":format!("{e:#}")}));
                }
            }
        }
        let (session, _, selected) = best.context("No usable segmentation provider")?;
        Ok(Self {
            session,
            selected,
            benchmarks: json!(scores),
            note,
        })
    }
}
struct State {
    mask: Option<(Vec<f32>, Instant)>,
    report: Value,
}
pub struct Worker {
    pub provider: String,
    sender: Option<mpsc::SyncSender<(Vec<f32>, Instant)>>,
    busy: Arc<AtomicBool>,
    state: Arc<Mutex<State>>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Worker {
    pub fn start(provider: String) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<(Vec<f32>, Instant)>(1);
        let busy = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(State {
            mask: None,
            report: json!({"delegate":"Starting"}),
        }));
        let (flag, output, wanted) = (busy.clone(), state.clone(), provider.clone());
        let worker = thread::spawn(move || {
            let mut model = None;
            let mut failed = false;
            while let Ok((input, captured)) = receiver.recv() {
                if model.is_none() && !failed {
                    match Model::new(&wanted, &input) {
                        Ok(m) => model = Some(m),
                        Err(e) => {
                            output.lock().unwrap().report =
                                json!({"delegate":"Unavailable","error":format!("{e:#}")});
                            failed = true;
                        }
                    }
                }
                if let Some(m) = &mut model {
                    let t = Instant::now();
                    match infer(&mut m.session, &input) {
                        Ok(mask) => {
                            let ms = t.elapsed().as_secs_f64() * 1000.;
                            let cpu_ms = m
                                .benchmarks
                                .as_array()
                                .and_then(|b| b.iter().find(|v| v["provider"] == "cpu"))
                                .and_then(|v| v["ms"].as_f64());
                            let gpu_ms = m
                                .benchmarks
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter(|v| v["provider"] != "cpu")
                                .filter_map(|v| v["ms"].as_f64())
                                .reduce(f64::min);
                            let mut s = output.lock().unwrap();
                            s.mask = Some((mask, captured));
                            s.report = json!({"delegate":m.selected,"inferenceMs":ms,"cpuMs":cpu_ms,"gpuMs":gpu_ms,"benchmarks":m.benchmarks,"error":m.note});
                        }
                        Err(e) => {
                            let mut s = output.lock().unwrap();
                            s.mask = None;
                            s.report = json!({"delegate":m.selected,"error":format!("{e:#}")});
                        }
                    }
                }
                flag.store(false, Ordering::Release);
            }
        });
        Self {
            provider,
            sender: Some(sender),
            busy,
            state,
            worker: Some(worker),
        }
    }
    pub fn offer(&self, frame: &Frame) -> bool {
        if self.busy.swap(true, Ordering::AcqRel) {
            return false;
        }
        if self
            .sender
            .as_ref()
            .unwrap()
            .try_send((tensor(frame), Instant::now()))
            .is_err()
        {
            self.busy.store(false, Ordering::Release);
            return false;
        }
        true
    }
    pub fn mask(&self) -> Option<Vec<f32>> {
        self.state
            .lock()
            .unwrap()
            .mask
            .as_ref()
            .filter(|(_, t)| t.elapsed() < Duration::from_millis(500))
            .map(|(m, _)| m.clone())
    }
    pub fn report(&self) -> Value {
        let s = self.state.lock().unwrap();
        let mut report = s.report.clone();
        report["maskAgeMs"] = s
            .mask
            .as_ref()
            .map(|(_, t)| json!(t.elapsed().as_secs_f64() * 1000.))
            .unwrap_or(Value::Null);
        report
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub fn sample_mask(mask: &[f32], uv: [f32; 2]) -> f32 {
    let x = (uv[0] * W as f32 - 0.5).clamp(0., W as f32 - 1.);
    let y = (uv[1] * H as f32 - 0.5).clamp(0., H as f32 - 1.);
    let (ix, iy) = (x.floor() as usize, y.floor() as usize);
    let (nx, ny) = ((ix + 1).min(W - 1), (iy + 1).min(H - 1));
    let (fx, fy) = (x.fract(), y.fract());
    let value = (mask[iy * W + ix] * (1. - fx) + mask[iy * W + nx] * fx) * (1. - fy)
        + (mask[ny * W + ix] * (1. - fx) + mask[ny * W + nx] * fx) * fy;
    let t = ((value - 0.25) / 0.5).clamp(0., 1.);
    t * t * (3. - 2. * t)
}
#[allow(dead_code)] // Invoked by the separate native diagnostic executable.
pub fn smoke(frame: &Frame) -> Result<Value> {
    let input = tensor(frame);
    let mut m = Model::new("auto", &input)?;
    let t = Instant::now();
    let mask = infer(&mut m.session, &input)?;
    Ok(
        json!({"selected":m.selected,"benchmarks":m.benchmarks,"maskPixels":mask.len(),"inferenceMs":t.elapsed().as_secs_f64()*1000.,"min":mask.iter().copied().fold(1.,f32::min),"max":mask.iter().copied().fold(0.,f32::max)}),
    )
}
