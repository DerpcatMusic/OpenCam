use anyhow::{Context, Result, ensure};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};

const MAX_FRAME: usize = 160 * 1024 * 1024;
const MAX_DNG: u64 = 512 * 1024 * 1024;
pub fn frame_bytes(w: u32, h: u32, rgba: bool) -> Result<usize> {
    ensure!(
        w > 0 && h > 0 && w <= 8192 && h <= 8192 && w % 2 == 0 && h % 2 == 0,
        "Invalid uncompressed dimensions"
    );
    let pixels = (w as usize)
        .checked_mul(h as usize)
        .context("Frame size overflow")?;
    let bytes = if rgba {
        pixels.checked_mul(4)
    } else {
        pixels.checked_mul(3).map(|n| n / 2)
    }
    .context("Frame size overflow")?;
    ensure!(
        bytes <= MAX_FRAME,
        "Uncompressed frame exceeds memory limit"
    );
    Ok(bytes)
}

#[derive(Default)]
pub struct FrameChunks {
    expected: usize,
    pts: u64,
    data: Vec<u8>,
}
impl FrameChunks {
    pub fn configure(&mut self, message: &Value) -> Result<()> {
        self.data.clear();
        self.expected = match message["mime"].as_str() {
            Some("video/x-opencam-i420" | "video/x-opencam-rgba") => frame_bytes(
                u32::try_from(message["width"].as_u64().context("Missing frame width")?)?,
                u32::try_from(message["height"].as_u64().context("Missing frame height")?)?,
                message["mime"] == "video/x-opencam-rgba",
            )?,
            _ => 0,
        };
        Ok(())
    }
    pub fn push(&mut self, packet: &[u8]) -> Result<Option<Vec<u8>>> {
        ensure!(
            packet.len() > 20 && packet.len() <= 65536 + 20,
            "Invalid pixel chunk size"
        );
        let pts = u64::from_be_bytes(packet[..8].try_into()?);
        let total = u32::from_be_bytes(packet[8..12].try_into()?) as usize;
        let offset = u32::from_be_bytes(packet[12..16].try_into()?) as usize;
        ensure!(
            total > 0 && total == self.expected && offset + packet.len() - 20 <= total,
            "Pixel chunk exceeds configured frame"
        );
        if offset == 0 {
            self.pts = pts;
            self.data = Vec::with_capacity(total + 12);
            self.data.extend_from_slice(&pts.to_be_bytes());
            self.data.extend_from_slice(&packet[16..20]);
        }
        // A mode change may cancel an in-flight frame; resume only at its next boundary.
        if self.data.is_empty() {
            return Ok(None);
        }
        ensure!(
            self.pts == pts && self.data.len() == offset + 12,
            "Pixel chunks are out of order"
        );
        self.data.extend_from_slice(&packet[20..]);
        Ok((self.data.len() == total + 12).then(|| std::mem::take(&mut self.data)))
    }
}

pub struct RawFile {
    id: [u8; 16],
    bytes: u64,
    offset: u64,
    file: File,
    part: PathBuf,
    path: PathBuf,
    digest: Sha256,
    complete: bool,
}
impl RawFile {
    pub fn begin(event: &Value) -> Result<Self> {
        let text = event["id"].as_str().context("Missing RAW transfer ID")?;
        let id = crate::protocol::unhex::<16>(text)?;
        let bytes = event["bytes"].as_u64().context("Missing RAW length")?;
        ensure!(
            bytes > 0 && bytes <= MAX_DNG && event["format"] == "dng",
            "Unsafe RAW file descriptor"
        );
        // The phone's filename is never used as a path.
        let path = PathBuf::from(format!("OpenCam-{}.dng", hex(&id)));
        let part = path.with_extension("dng.part");
        ensure!(!path.exists(), "RAW destination already exists");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&part)?;
        Ok(Self {
            id,
            bytes,
            offset: 0,
            file,
            part,
            path,
            digest: Sha256::new(),
            complete: false,
        })
    }
    pub fn push(&mut self, packet: &[u8]) -> Result<()> {
        ensure!(
            packet.len() > 24 && packet.len() <= 65536 + 24 && packet[..16] == self.id,
            "Invalid RAW chunk"
        );
        let offset = u64::from_be_bytes(packet[16..24].try_into()?);
        let data = &packet[24..];
        ensure!(
            offset == self.offset && offset + data.len() as u64 <= self.bytes,
            "RAW chunks exceed descriptor or are out of order"
        );
        self.file.write_all(data)?;
        self.digest.update(data);
        self.offset += data.len() as u64;
        Ok(())
    }
    pub fn finish(&mut self, event: &Value) -> Result<String> {
        ensure!(
            crate::protocol::unhex::<16>(event["id"].as_str().context("Missing RAW transfer ID")?)?
                == self.id
                && self.offset == self.bytes,
            "Incomplete RAW transfer"
        );
        let expected = crate::protocol::unhex::<32>(
            event["sha256"].as_str().context("Missing RAW checksum")?,
        )?;
        ensure!(
            self.digest.clone().finalize().as_slice() == expected,
            "RAW checksum mismatch"
        );
        self.file.flush()?;
        self.file.sync_all()?;
        // A hard link commits without replacing an existing user file; both paths share a filesystem.
        std::fs::hard_link(&self.part, &self.path)?;
        std::fs::remove_file(&self.part)?;
        self.complete = true;
        Ok(self.path.to_string_lossy().into_owned())
    }
}
impl Drop for RawFile {
    fn drop(&mut self) {
        if !self.complete {
            let _ = std::fs::remove_file(&self.part);
        }
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chunk(pts: u64, total: u32, offset: u32, data: &[u8]) -> Vec<u8> {
        let mut v = pts.to_be_bytes().to_vec();
        v.extend(total.to_be_bytes());
        v.extend(offset.to_be_bytes());
        v.extend(0x10c10000u32.to_be_bytes());
        v.extend(data);
        v
    }
    #[test]
    fn pixel_chunks_are_exact_bounded_and_can_replace_canceled_frames() {
        let mut chunks = FrameChunks::default();
        chunks
            .configure(&serde_json::json!({"mime":"video/x-opencam-i420","width":2,"height":2}))
            .unwrap();
        assert!(chunks.push(&chunk(1, 6, 0, &[1, 2, 3])).unwrap().is_none());
        assert!(chunks.push(&chunk(2, 6, 0, &[7, 8])).unwrap().is_none());
        let frame = chunks
            .push(&chunk(2, 6, 2, &[9, 10, 11, 12]))
            .unwrap()
            .unwrap();
        assert_eq!(&frame[12..], &[7, 8, 9, 10, 11, 12]);
        assert!(chunks.push(&chunk(3, 100000000, 0, &[0])).is_err());
        chunks.push(&chunk(4, 6, 0, &[1])).unwrap();
        assert!(chunks.push(&chunk(4, 6, 2, &[2])).is_err());
        assert!(frame_bytes(8192, 8192, true).is_err());
        assert!(frame_bytes(5, 4, false).is_err());
    }
    #[test]
    fn raw_transfer_checks_hash_order_and_removes_partial_files() {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = seed.to_be_bytes();
        let text = hex(&id);
        let bytes = b"II*\0sensor data";
        let event = serde_json::json!({"id":text,"bytes":bytes.len(),"format":"dng"});
        let mut file = RawFile::begin(&event).unwrap();
        let part = file.part.clone();
        let mut packet = id.to_vec();
        packet.extend(0u64.to_be_bytes());
        packet.extend(bytes);
        file.push(&packet).unwrap();
        assert!(file.push(&packet).is_err());
        assert!(
            file.finish(&serde_json::json!({"id":text,"sha256":"00".repeat(32)}))
                .is_err()
        );
        let path = file
            .finish(&serde_json::json!({"id":text,"sha256":hex(&Sha256::digest(bytes))}))
            .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(!part.exists());
        std::fs::remove_file(path).unwrap();
        let file = RawFile::begin(&event).unwrap();
        let part = file.part.clone();
        drop(file);
        assert!(!part.exists());
        assert!(
            RawFile::begin(&serde_json::json!({"id":"../escape","bytes":100,"format":"dng"}))
                .is_err()
        );
    }
}
