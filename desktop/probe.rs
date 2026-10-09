mod auth;
mod benchmark;
mod output;
mod processing;
mod protocol;
mod session;
mod webcam;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--processing-smoke") {
        println!("{}", serde_json::to_string_pretty(&processing::smoke()?)?);
        return Ok(());
    }
    if args.len() < 2 || args[0] != "--pair" {
        bail!(
            "Usage: opencam-probe --processing-smoke | --pair 'opencam://…' [--benchmark | --capabilities] [--seconds N] [--usb] [--password-stdin] [--size WIDTHxHEIGHT] [--fps N] [--output WIDTHxHEIGHT] [--crop] [--process-on phone|desktop] [--backend auto|gpu|cpu] [--stretch FACTOR] [--background-blur RADIUS]"
        );
    }
    let mut pairing = protocol::Pairing::parse(&args[1])?;
    if args.iter().any(|a| a == "--usb") {
        pairing.address = format!("127.0.0.1:{}", protocol::PORT);
    }
    let duration = args
        .iter()
        .position(|a| a == "--seconds")
        .and_then(|i| args.get(i + 1))
        .map(|v| v.parse::<u64>())
        .transpose()?
        .unwrap_or(8);
    let shared = session::Shared::new(None);
    if let Some(spec) = args
        .iter()
        .position(|a| a == "--output")
        .and_then(|i| args.get(i + 1))
    {
        let (w, h) = output::resolution(spec)?;
        let output = output::Output {
            width: w,
            height: h,
            crop: args.iter().any(|a| a == "--crop"),
        };
        output.geometry(1280, 720)?;
        *shared.output.lock().unwrap() = output;
    }
    if args.iter().any(|a| a == "--password-stdin") {
        let mut password = String::new();
        std::io::stdin().read_line(&mut password)?;
        *shared.password.lock().unwrap() = password.trim_end_matches(['\r', '\n']).to_string();
    }
    let session = session::Session::start(pairing, shared.clone());
    let started = Instant::now();
    let mut configured = None;
    let mut benchmark: Option<benchmark::Benchmark> = None;
    let mut capturing = None;
    let mut reported_progress = String::new();
    loop {
        while let Ok(event) = session.events.try_recv() {
            if let Some(benchmark) = &mut benchmark {
                benchmark.event(&event);
            }
            match event["type"].as_str() {
                Some("capabilities") => {
                    if args.iter().any(|a| a == "--capabilities") {
                        println!("{}", serde_json::to_string_pretty(&event)?);
                        return Ok(());
                    }
                    let camera = event["cameras"]
                        .as_array()
                        .context("Missing cameras")?
                        .first()
                        .context("No exposed cameras")?;
                    let mut settings = protocol::default_settings(&event, camera)?;
                    if let Some(size) = args
                        .iter()
                        .position(|a| a == "--size")
                        .and_then(|i| args.get(i + 1))
                    {
                        let (w, h) = size.split_once('x').context("Size must be WIDTHxHEIGHT")?;
                        let (w, h) = (w.parse::<u32>()?, h.parse::<u32>()?);
                        anyhow::ensure!(
                            camera["sizes"]
                                .as_array()
                                .is_some_and(|a| a.contains(&json!([w, h]))),
                            "Sensor size is not advertised"
                        );
                        settings["width"] = json!(w);
                        settings["height"] = json!(h);
                    }
                    if let Some(fps) = args
                        .iter()
                        .position(|a| a == "--fps")
                        .and_then(|i| args.get(i + 1))
                    {
                        let fps = fps.parse::<u32>()?;
                        anyhow::ensure!(
                            protocol::normal_fps(
                                camera,
                                &json!([settings["width"], settings["height"]])
                            )
                            .contains(&(fps as u64)),
                            "FPS is not advertised"
                        );
                        settings["fps"] = json!(fps);
                    }
                    if args.iter().any(|a| a == "--benchmark") {
                        benchmark = Some(benchmark::Benchmark::new(
                            event["codecs"]
                                .as_array()
                                .context("Missing codecs")?
                                .clone(),
                            settings,
                            &session,
                        ));
                    } else {
                        if let Some(place) = args
                            .iter()
                            .position(|a| a == "--process-on")
                            .and_then(|i| args.get(i + 1))
                        {
                            anyhow::ensure!(
                                ["phone", "desktop"].contains(&place.as_str()),
                                "Process on must be phone or desktop"
                            );
                            settings["processingLocation"] = json!(place);
                            let output = *shared.output.lock().unwrap();
                            settings["outputWidth"] = json!(output.width);
                            settings["outputHeight"] = json!(output.height);
                            settings["outputMode"] = json!(u32::from(output.crop));
                            for (flag, key) in [
                                ("--stretch", "stretchX"),
                                ("--background-blur", "backgroundBlur"),
                            ] {
                                if let Some(value) = args
                                    .iter()
                                    .position(|a| a == flag)
                                    .and_then(|i| args.get(i + 1))
                                {
                                    settings[key] = json!(value.parse::<f64>()?);
                                }
                            }
                            if let Some(backend) = args
                                .iter()
                                .position(|a| a == "--backend")
                                .and_then(|i| args.get(i + 1))
                            {
                                settings["desktopBackend"] = json!(backend);
                            }
                            *shared.processing.lock().unwrap() =
                                processing::Options::read(&settings)?;
                        }
                        session.send(json!({"type":"configure", "settings":settings}));
                    }
                }
                Some("configured") => {
                    configured = Some(event.clone());
                    capturing = Some(Instant::now());
                }
                Some("disconnected") => bail!("{}", event["message"]),
                Some("error") if benchmark.is_none() => bail!("{}", event["message"]),
                _ => {}
            }
        }
        let stats = shared.stats.lock().unwrap().clone();
        if let Some(benchmark) = &mut benchmark {
            let progress = benchmark.progress();
            if progress != reported_progress {
                eprintln!("{progress}");
                reported_progress = progress;
            }
            if benchmark.tick(stats.clone(), &session) {
                let winner = benchmark.winner();
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"results":benchmark.results, "winner":winner, "note":"Frame age is a clock-synchronized estimate, not glass-to-glass latency"})
                    )?
                );
                return Ok(());
            }
        } else if capturing.is_some_and(|s| s.elapsed() >= Duration::from_secs(duration)) {
            if stats.frames == 0 {
                bail!("Stream configured but produced no decoded frames");
            }
            let frame = shared.frame.lock().unwrap();
            let dimensions = frame
                .as_ref()
                .map(|f| (f.width, f.height, f.sequence, f.pixels.len()));
            let output: Value = json!({"configured":configured, "decodedFrames":stats.frames, "videoBytes":stats.bytes,
                "lastEstimatedAgeMs":stats.last_age_ms, "bestRoundTripMs":stats.rtt_ms, "lastFrame":dimensions,"processing":*shared.desktop_processing_report.lock().unwrap()});
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }
        if started.elapsed() > Duration::from_secs(180) {
            bail!("Probe timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
