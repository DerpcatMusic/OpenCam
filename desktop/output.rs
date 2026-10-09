use anyhow::{Context, Result, ensure};
use serde::Serialize;

pub fn resolution(text: &str) -> Result<(u32, u32)> {
    if text.trim().eq_ignore_ascii_case("source") {
        return Ok((0, 0));
    }
    let normalized = text.replace(['X', '×'], "x");
    let (w, h) = normalized
        .split_once('x')
        .ok_or_else(|| anyhow::anyhow!("Enter a resolution such as 1920x1080"))?;
    let (w, h) = (
        w.trim()
            .parse()
            .context("Enter a resolution such as 1920x1080")?,
        h.trim()
            .parse()
            .context("Enter a resolution such as 1920x1080")?,
    );
    dimensions(w, h)?;
    Ok((w, h))
}

#[derive(Clone, Copy, Default, Serialize)]
pub struct Output {
    pub width: u32,
    pub height: u32,
    pub crop: bool,
}
impl Output {
    pub fn geometry(self, source_w: u32, source_h: u32) -> Result<(u32, u32, u32, u32)> {
        dimensions(source_w, source_h)?;
        if self.width == 0 && self.height == 0 {
            return Ok((source_w, source_h, source_w, source_h));
        }
        dimensions(self.width, self.height)?;
        ensure!(
            self.width % 2 == 0 && self.height % 2 == 0,
            "Output width and height must be even"
        );
        let scale = if self.crop {
            (self.width as f64 / source_w as f64).max(self.height as f64 / source_h as f64)
        } else {
            (self.width as f64 / source_w as f64).min(self.height as f64 / source_h as f64)
        };
        let round = |n: f64| if self.crop { n.ceil() } else { n.floor().max(1.) } as u32;
        let (w, h) = (
            round(source_w as f64 * scale),
            round(source_h as f64 * scale),
        );
        dimensions(w, h)?;
        Ok((w, h, self.width, self.height))
    }
}
fn dimensions(w: u32, h: u32) -> Result<()> {
    ensure!(
        w > 0 && h > 0 && w <= 8192 && h <= 8192 && w as u64 * h as u64 * 4 <= 160 * 1024 * 1024,
        "Dimensions exceed the 8192-pixel / 160 MiB frame limit"
    );
    Ok(())
}
// Scaling happens in libswscale; this only centers the crop or adds opaque bars.
pub fn compose(pixels: Vec<u8>, w: u32, h: u32, target_w: u32, target_h: u32) -> Result<Vec<u8>> {
    dimensions(w, h)?;
    dimensions(target_w, target_h)?;
    ensure!(
        pixels.len() == w as usize * h as usize * 4,
        "Invalid scaled frame length"
    );
    if w == target_w && h == target_h {
        return Ok(pixels);
    }
    let mut out = vec![0; target_w as usize * target_h as usize * 4];
    for pixel in out.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    let (copy_w, copy_h) = (w.min(target_w) as usize, h.min(target_h) as usize);
    let (src_x, src_y) = (
        (w.saturating_sub(target_w) / 2) as usize,
        (h.saturating_sub(target_h) / 2) as usize,
    );
    let (dst_x, dst_y) = (
        (target_w.saturating_sub(w) / 2) as usize,
        (target_h.saturating_sub(h) / 2) as usize,
    );
    for row in 0..copy_h {
        let src = ((src_y + row) * w as usize + src_x) * 4;
        let dst = ((dst_y + row) * target_w as usize + dst_x) * 4;
        out[dst..dst + copy_w * 4].copy_from_slice(&pixels[src..src + copy_w * 4]);
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editable_resolution_accepts_sizes_and_rejects_invalid_input() {
        assert_eq!(resolution(" 1440 × 1080 ").unwrap(), (1440, 1080));
        assert_eq!(resolution("1080X1920").unwrap(), (1080, 1920));
        assert_eq!(resolution("Source").unwrap(), (0, 0));
        for invalid in [
            "16:9",
            "-1x720",
            "0x720",
            "1280x720x2",
            "99999x2",
            "Source720x720",
        ] {
            assert!(resolution(invalid).is_err());
        }
    }
    #[test]
    fn preserves_aspect_in_square_and_portrait_outputs() {
        assert_eq!(
            Output {
                width: 720,
                height: 720,
                crop: false
            }
            .geometry(1280, 720)
            .unwrap(),
            (720, 405, 720, 720)
        );
        assert_eq!(
            Output {
                width: 720,
                height: 720,
                crop: true
            }
            .geometry(1280, 720)
            .unwrap(),
            (1280, 720, 720, 720)
        );
        assert_eq!(
            Output {
                width: 720,
                height: 1280,
                crop: false
            }
            .geometry(720, 1280)
            .unwrap(),
            (720, 1280, 720, 1280)
        );
        assert!(
            Output {
                width: 3,
                height: 4,
                crop: true
            }
            .geometry(8, 8)
            .is_err()
        );
        assert!(
            Output {
                width: 8192,
                height: 8192,
                crop: false
            }
            .geometry(1280, 720)
            .is_err()
        );
        assert!(
            Output {
                width: 0,
                height: 720,
                crop: false
            }
            .geometry(1280, 720)
            .is_err()
        );
    }
    #[test]
    fn centers_crop_and_letterbox_without_stretching() {
        let row: Vec<u8> = (0..4).flat_map(|v| [v, v, v, 255]).collect();
        let cropped = compose(row.clone(), 4, 1, 2, 1).unwrap();
        assert_eq!(&cropped[..4], &[1, 1, 1, 255]);
        assert_eq!(&cropped[4..], &[2, 2, 2, 255]);
        let padded = compose(row, 4, 1, 4, 3).unwrap();
        assert_eq!(&padded[..4], &[0, 0, 0, 255]);
        assert_eq!(&padded[20..24], &[1, 1, 1, 255]);
        assert!(compose(vec![0; 3], 1, 1, 1, 1).is_err());
    }
}
