use crate::session::{Session, Stats};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Measurement {
    pub codec: String,
    pub label: String,
    pub decoded_fps: f64,
    pub mbps: f64,
    pub estimated_age_ms: Option<f64>,
    pub error: Option<String>,
}

pub struct Benchmark {
    pub candidates: Vec<Value>,
    pub results: Vec<Measurement>,
    pub original: Value,
    index: usize,
    started: Instant,
    baseline: Option<(Instant, Stats)>,
    configured: bool,
    needs_start: bool,
}

impl Benchmark {
    pub fn new(candidates: Vec<Value>, settings: Value, session: &Session) -> Self {
        let benchmark = Self {
            candidates,
            results: vec![],
            original: settings,
            index: 0,
            started: Instant::now(),
            baseline: None,
            configured: false,
            needs_start: false,
        };
        benchmark.configure(session);
        benchmark
    }
    fn configure(&self, session: &Session) {
        if let Some(codec) = self.candidates.get(self.index) {
            let mut settings = self.original.clone();
            settings["codec"] = codec["name"].clone();
            session.send(json!({"type":"configure", "settings":settings}));
        }
    }
    pub fn event(&mut self, event: &Value) {
        match event["type"].as_str() {
            Some("configured")
                if self
                    .candidates
                    .get(self.index)
                    .is_some_and(|c| c["name"] == event["settings"]["codec"]) =>
            {
                self.configured = true;
                self.started = Instant::now();
            }
            Some("error") => {
                self.failed(event["message"].as_str().unwrap_or("Codec failed").into())
            }
            _ => {}
        }
    }
    fn failed(&mut self, message: String) {
        if let Some(codec) = self.candidates.get(self.index) {
            self.results.push(Measurement {
                codec: codec["name"].as_str().unwrap_or("").into(),
                label: codec["label"].as_str().unwrap_or("").into(),
                decoded_fps: 0.,
                mbps: 0.,
                estimated_age_ms: None,
                error: Some(message),
            });
        }
        self.index += 1;
        self.configured = false;
        self.baseline = None;
        self.needs_start = true;
    }
    pub fn tick(&mut self, stats: Stats, session: &Session) -> bool {
        if self.index >= self.candidates.len() {
            return true;
        }
        if !self.configured {
            if self.needs_start {
                self.needs_start = false;
                self.started = Instant::now();
                self.configure(session);
            } else if self.started.elapsed() >= Duration::from_secs(20) {
                self.failed("Codec did not start within 20 seconds".into());
            }
            return self.index >= self.candidates.len();
        }
        if self.started.elapsed() >= Duration::from_secs(2) && self.baseline.is_none() {
            self.baseline = Some((Instant::now(), stats.clone()));
        }
        if let Some((start, before)) = &self.baseline {
            if start.elapsed() >= Duration::from_secs(5) {
                let seconds = start.elapsed().as_secs_f64();
                let samples = stats.age_samples.saturating_sub(before.age_samples);
                let codec = &self.candidates[self.index];
                let frames = stats.frames.saturating_sub(before.frames);
                self.results.push(Measurement {
                    codec: codec["name"].as_str().unwrap_or("").into(),
                    label: codec["label"].as_str().unwrap_or("").into(),
                    decoded_fps: frames as f64 / seconds,
                    mbps: stats.bytes.saturating_sub(before.bytes) as f64 * 8. / seconds / 1e6,
                    estimated_age_ms: (samples > 0)
                        .then(|| (stats.age_sum_ms - before.age_sum_ms) / samples as f64),
                    error: (frames == 0).then(|| "No decoded frames received".into()),
                });
                self.index += 1;
                self.baseline = None;
                self.configured = false;
                self.started = Instant::now();
                self.configure(session);
            }
        }
        self.index >= self.candidates.len()
    }
    pub fn progress(&self) -> String {
        format!(
            "Testing codec {} of {} · {}",
            (self.index + 1).min(self.candidates.len()),
            self.candidates.len(),
            self.candidates
                .get(self.index)
                .and_then(|c| c["label"].as_str())
                .unwrap_or("complete")
        )
    }
    pub fn winner(&self) -> Option<&Measurement> {
        winner(&self.results, self.original["fps"].as_f64().unwrap_or(30.))
    }
}

pub fn winner(results: &[Measurement], target_fps: f64) -> Option<&Measurement> {
    let max_fps = results
        .iter()
        .filter(|r| r.error.is_none())
        .map(|r| r.decoded_fps)
        .fold(0., f64::max);
    let threshold = (target_fps * 0.9).min(max_fps * 0.95);
    results
        .iter()
        .filter(|r| r.error.is_none() && r.decoded_fps > 0. && r.decoded_fps >= threshold)
        .min_by(|a, b| {
            a.estimated_age_ms
                .unwrap_or(f64::INFINITY)
                .total_cmp(&b.estimated_age_ms.unwrap_or(f64::INFINITY))
                .then_with(|| b.decoded_fps.total_cmp(&a.decoded_fps))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prioritizes_latency_only_among_codecs_that_keep_up() {
        let result = |name: &str, fps, age| Measurement {
            codec: name.into(),
            label: name.into(),
            decoded_fps: fps,
            mbps: 8.,
            estimated_age_ms: Some(age),
            error: None,
        };
        let results = vec![
            result("fast", 30., 25.),
            result("slow", 12., 5.),
            result("laggy", 30., 80.),
        ];
        assert_eq!(winner(&results, 30.).unwrap().codec, "fast");
        assert!(winner(&[], 30.).is_none());
    }
}
