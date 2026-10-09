use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Write;

pub const PORT: u16 = 4937;
pub const MAX_PACKET: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Pairing {
    pub address: String,
    pub token: String,
    pub pin: [u8; 32],
}

pub fn unhex<const N: usize>(s: &str) -> Result<[u8; N]> {
    ensure!(
        s.len() == N * 2 && s.is_ascii(),
        "Expected {} hexadecimal characters",
        N * 2
    );
    let mut result = [0; N];
    for (i, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
            .context("Invalid hexadecimal pairing value")?;
    }
    Ok(result)
}

impl Pairing {
    pub fn parse(link: &str) -> Result<Self> {
        let link = link
            .trim()
            .strip_prefix("opencam://")
            .context("Paste the complete opencam:// pairing link from your phone")?;
        let (address, query) = link
            .split_once('?')
            .context("Pairing link needs a token and certificate pin")?;
        ensure!(
            !address.is_empty() && !address.contains(['/', '@', '#', ' ', '\n']),
            "Invalid phone address"
        );
        let (_, port) = address
            .rsplit_once(':')
            .context("Phone address needs a port")?;
        ensure!(
            port.parse::<u16>().is_ok_and(|p| p != 0),
            "Invalid phone port"
        );
        let mut token = None;
        let mut pin = None;
        for field in query.split('&') {
            let (name, value) = field.split_once('=').context("Invalid pairing field")?;
            match name {
                "token" if token.is_none() => {
                    unhex::<16>(value)?;
                    token = Some(value.to_string());
                }
                "pin" if pin.is_none() => pin = Some(unhex::<32>(value)?),
                _ => bail!("Unknown or duplicate pairing field"),
            }
        }
        Ok(Self {
            address: address.into(),
            token: token.context("Pairing token missing")?,
            pin: pin.context("Certificate pin missing")?,
        })
    }
}

pub fn certificate_matches(certificate: &[u8], expected: &[u8; 32]) -> bool {
    let actual: [u8; 32] = Sha256::digest(certificate).into();
    actual == *expected
}

pub fn write_json(writer: &mut impl Write, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    ensure!(body.len() <= 65536, "Control packet is too large");
    writer.write_all(&[1])?;
    writer.write_all(&(body.len() as u32).to_be_bytes())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}

#[derive(Default)]
pub struct Packets {
    buffer: Vec<u8>,
}

impl Packets {
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<(u8, Vec<u8>)>> {
        self.buffer.extend_from_slice(bytes);
        let mut packets = vec![];
        let mut consumed = 0;
        while self.buffer.len() - consumed >= 5 {
            let header = &self.buffer[consumed..consumed + 5];
            let kind = header[0];
            let length = u32::from_be_bytes(header[1..5].try_into()?) as usize;
            ensure!(
                matches!(kind, 1 | 2) && length > 0 && length <= MAX_PACKET,
                "Invalid or oversized stream packet"
            );
            if self.buffer.len() - consumed < 5 + length {
                break;
            }
            packets.push((
                kind,
                self.buffer[consumed + 5..consumed + 5 + length].to_vec(),
            ));
            consumed += 5 + length;
        }
        self.buffer.drain(..consumed);
        ensure!(
            self.buffer.len() <= MAX_PACKET + 5,
            "Stream buffer exceeded its limit"
        );
        Ok(packets)
    }
}

pub fn normal_fps(camera: &Value, size: &Value) -> Vec<u64> {
    let max_fps = camera["frameDurations"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|d| d["size"] == *size)
        .and_then(|d| d["minFrameNs"].as_u64())
        .filter(|n| *n > 0)
        .map(|n| 1_000_000_000 / n)
        .unwrap_or(u64::MAX);
    let mut values = Vec::new();
    for range in camera["fpsRanges"].as_array().into_iter().flatten() {
        let min = range[0].as_u64().unwrap_or(1).max(1);
        let max = range[1].as_u64().unwrap_or(0).min(max_fps);
        if min > max {
            continue;
        }
        values.extend(min..=max.min(240));
    }
    values.sort_unstable();
    values.dedup();
    values
}

pub fn default_settings(catalog: &Value, camera: &Value) -> Result<Value> {
    let sizes = camera["sizes"]
        .as_array()
        .context("Camera did not advertise encoding sizes")?;
    let size = sizes
        .iter()
        .filter(|s| s[0].as_u64().unwrap_or(0) <= 1920 && s[1].as_u64().unwrap_or(0) <= 1080)
        .min_by_key(|s| {
            s[0].as_i64().unwrap_or(0).abs_diff(1280) + s[1].as_i64().unwrap_or(0).abs_diff(720)
        })
        .or_else(|| sizes.first())
        .context("This camera has no encoder outputs")?;
    let codecs = catalog["codecs"]
        .as_array()
        .context("No hardware codecs advertised")?;
    let codec = codecs
        .iter()
        .find(|c| c["mime"] == "video/avc")
        .or_else(|| codecs.first())
        .context("No hardware video encoder is exposed")?;
    let iso = camera["iso"][0]
        .as_i64()
        .unwrap_or(100)
        .max(100)
        .min(camera["iso"][1].as_i64().unwrap_or(100));
    let exposure = camera["exposureNs"][0]
        .as_i64()
        .unwrap_or(1)
        .max(8_333_333)
        .min(camera["exposureNs"][1].as_i64().unwrap_or(8_333_333));
    let fps = normal_fps(camera, size)
        .into_iter()
        .min_by_key(|fps| fps.abs_diff(30))
        .context("This resolution has no advertised regular frame rate")?;
    let bitrate = 8_000_000i64.clamp(
        codec["bitrate"][0].as_i64().unwrap_or(1),
        codec["bitrate"][1].as_i64().unwrap_or(8_000_000),
    );
    Ok(
        serde_json::json!({"camera":camera["id"], "codec":codec["name"], "width":size[0], "height":size[1], "fps":fps,
        "bitrate":bitrate, "manual":false, "iso":iso, "exposureNs":exposure, "focusAuto":true, "focus":0,
        "zoom":1.0, "ev":0, "awb":1, "ois":false, "stabilization":false, "torch":false,
        "aeLock":false, "awbLock":false, "gains":[1.0,1.0,1.0,1.0],
        "stretchX":1.0,"stretchY":1.0,"distortion":0.0,"bulge":0.0,"bulgeRadius":0.4,"bulgeX":0.5,"bulgeY":0.5,
        "backgroundBlur":0.0,"maskFps":10,"mlDelegate":"auto","outputMode":0,
        "processingLocation":"phone","desktopBackend":"auto","desktopAdapter":"auto","desktopMl":"auto",
        "noiseReduction":-1,"edgeMode":-1,"aberrationMode":-1,"lensCorrection":-1}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fps_choices_respect_resolution_duration_and_disjoint_ranges() {
        let camera = serde_json::json!({"fpsRanges":[[15,30],[60,60]],"frameDurations":[{"size":[3840,2160],"minFrameNs":66_666_666}]});
        assert_eq!(
            normal_fps(&camera, &serde_json::json!([3840, 2160])),
            vec![15]
        );
        let normal = normal_fps(&camera, &serde_json::json!([1280, 720]));
        assert!(
            normal.contains(&27)
                && normal.contains(&30)
                && normal.contains(&60)
                && !normal.contains(&48)
        );
    }
    #[test]
    fn pairing_rejects_missing_duplicate_and_malformed_credentials() {
        let valid = format!(
            "opencam://192.168.1.10:4937?token={}&pin={}",
            "ab".repeat(16),
            "cd".repeat(32)
        );
        assert_eq!(Pairing::parse(&valid).unwrap().address, "192.168.1.10:4937");
        for bad in [
            valid.replace("4937", "0"),
            valid.replace("ab", "xy"),
            format!("{valid}&token={}", "ab".repeat(16)),
            valid.replace("?token=", "?wrong="),
        ] {
            assert!(Pairing::parse(&bad).is_err(), "accepted {bad}");
        }
    }
    #[test]
    fn fragmented_packets_and_multiple_packets_are_preserved() {
        let mut bytes = vec![];
        write_json(&mut bytes, &serde_json::json!({"type":"ping"})).unwrap();
        let first = bytes.clone();
        bytes.extend_from_slice(&first);
        let mut parser = Packets::default();
        let mut result = vec![];
        for chunk in bytes.chunks(3) {
            result.extend(parser.feed(chunk).unwrap());
        }
        assert_eq!(result.len(), 2);
        assert_eq!(
            serde_json::from_slice::<Value>(&result[1].1).unwrap()["type"],
            "ping"
        );
    }
    #[test]
    fn oversized_and_unknown_packets_are_rejected_before_allocation() {
        assert!(Packets::default().feed(&[2, 255, 255, 255, 255]).is_err());
        assert!(Packets::default().feed(&[9, 0, 0, 0, 1]).is_err());
    }
    #[test]
    fn certificate_pin_is_exact() {
        let pin: [u8; 32] = Sha256::digest(b"certificate").into();
        assert!(certificate_matches(b"certificate", &pin));
        assert!(!certificate_matches(b"different", &pin));
    }
}
