pub struct Monitor {
    pub zebra: bool,
    pub peaking: bool,
    pub false_color: bool,
    pub guides: bool,
    pub histogram: bool,
    pub bins: [u32; 64],
    pub threshold: u8,
}
impl Default for Monitor {
    fn default() -> Self {
        Self {
            zebra: false,
            peaking: false,
            false_color: false,
            guides: false,
            histogram: false,
            bins: [0; 64],
            threshold: 242,
        }
    }
}
impl Monitor {
    pub fn process(&mut self, pixels: &mut [u8], width: u32, height: u32) {
        if pixels.len() != width as usize * height as usize * 4 || width == 0 || height == 0 {
            return;
        }
        self.bins.fill(0);
        let luma = |p: &[u8]| {
            ((19 * u32::from(p[0]) + 183 * u32::from(p[1]) + 54 * u32::from(p[2])) >> 8) as u8
        };
        if self.histogram {
            for y in (0..height as usize).step_by(4) {
                for x in (0..width as usize).step_by(4) {
                    self.bins
                        [usize::from(luma(&pixels[(y * width as usize + x) * 4..][..4])) / 4] += 1;
                }
            }
        }
        if !self.zebra && !self.peaking && !self.false_color && !self.guides {
            return;
        }
        // Read each row before coloring it, so peaking measures the image rather than the overlay.
        let mut row = vec![0u8; width as usize];
        for y in 0..height as usize {
            for (x, v) in row.iter_mut().enumerate() {
                *v = luma(&pixels[(y * width as usize + x) * 4..][..4]);
            }
            for x in 0..width as usize {
                let p = &mut pixels[(y * width as usize + x) * 4..][..4];
                let v = row[x];
                if self.false_color {
                    let color = match v {
                        0..=8 => [100, 20, 85],
                        9..=32 => [180, 50, 30],
                        33..=102 => [v, v, v],
                        103..=118 => [45, 150, 45],
                        119..=134 => [130, 145, 210],
                        135..=229 => [v, v, v],
                        230..=246 => [40, 215, 235],
                        _ => [50, 45, 240],
                    };
                    p[..3].copy_from_slice(&color);
                }
                if self.zebra && v >= self.threshold && (x + y) % 16 < 5 {
                    p[..3].copy_from_slice(&[225, 225, 225]);
                }
                if self.peaking
                    && x > 0
                    && x + 1 < row.len()
                    && row[x - 1].abs_diff(row[x + 1]) >= 35
                {
                    p[..3].copy_from_slice(&[90, 90, 255]);
                }
                if self.guides
                    && (x == width as usize / 3
                        || x == width as usize * 2 / 3
                        || y == height as usize / 3
                        || y == height as usize * 2 / 3)
                {
                    p[..3].copy_from_slice(&[200, 200, 200]);
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monitoring_only_changes_the_preview_and_histogram_reads_original_luma() {
        let source = vec![255; 16 * 16 * 4];
        let mut preview = source.clone();
        let mut m = Monitor {
            zebra: true,
            histogram: true,
            threshold: 240,
            ..Default::default()
        };
        m.process(&mut preview, 16, 16);
        assert_ne!(preview, source);
        assert_eq!(m.bins[63], 16);
        assert_eq!(source, vec![255; 16 * 16 * 4]);
        assert!(preview.chunks_exact(4).all(|p| p[3] == 255));
        m.process(&mut [], 16, 16);
    }
}
