//! Mipmap chains for RGBA8 textures.

/// Number of levels in a full chain down to 1x1.
pub fn mip_count(w: u32, h: u32) -> u32 {
    32 - w.max(h).max(1).leading_zeros()
}

fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(l: f32) -> u8 {
    let c = if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// Builds the complete mip chain for a tightly packed RGBA8 image and
/// returns all levels concatenated (level 0 first), the layout GPUs and
/// Bevy expect. Color channels of sRGB images are averaged in linear
/// space so mips do not darken; alpha is always linear.
pub fn generate_rgba8(data: &[u8], w: u32, h: u32, srgb: bool) -> (Vec<u8>, u32) {
    assert_eq!(data.len(), (w * h * 4) as usize, "RGBA8 data size mismatch");
    let lut: Vec<f32> = (0..=255u8)
        .map(|c| if srgb { srgb_to_linear(c) } else { c as f32 / 255.0 })
        .collect();
    let encode = |v: f32| {
        if srgb {
            linear_to_srgb(v)
        } else {
            (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
        }
    };

    let levels = mip_count(w, h);
    let mut out = Vec::with_capacity(data.len() * 4 / 3 + 16);
    out.extend_from_slice(data);
    let (mut pw, mut ph) = (w as usize, h as usize);
    let mut prev_start = 0usize;
    for _ in 1..levels {
        let nw = (pw / 2).max(1);
        let nh = (ph / 2).max(1);
        let start = out.len();
        out.resize(start + nw * nh * 4, 0);
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0f32; 4];
                let mut n = 0.0;
                for dy in 0..2 {
                    for dx in 0..2 {
                        let sx = (x * 2 + dx).min(pw - 1);
                        let sy = (y * 2 + dy).min(ph - 1);
                        let i = prev_start + (sy * pw + sx) * 4;
                        for c in 0..3 {
                            acc[c] += lut[out[i + c] as usize];
                        }
                        acc[3] += out[i + 3] as f32 / 255.0;
                        n += 1.0;
                    }
                }
                let o = start + (y * nw + x) * 4;
                for c in 0..3 {
                    out[o + c] = encode(acc[c] / n);
                }
                out[o + 3] = ((acc[3] / n).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            }
        }
        prev_start = start;
        pw = nw;
        ph = nh;
    }
    (out, levels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts() {
        assert_eq!(mip_count(1, 1), 1);
        assert_eq!(mip_count(2048, 2048), 12);
        assert_eq!(mip_count(2048, 1024), 12);
        assert_eq!(mip_count(5, 3), 3);
    }

    #[test]
    fn chain_size_and_values() {
        let w = 8;
        let h = 4;
        let data: Vec<u8> = (0..w * h).flat_map(|_| [128u8, 64, 255, 200]).collect();
        let (out, levels) = generate_rgba8(&data, w, h, true);
        assert_eq!(levels, 4);
        // 8x4 + 4x2 + 2x1 + 1x1 texels.
        assert_eq!(out.len(), (32 + 8 + 2 + 1) * 4);
        // A flat color stays the same color at every level.
        assert_eq!(&out[out.len() - 4..], &[128, 64, 255, 200]);
    }

    #[test]
    fn srgb_average_is_gamma_correct() {
        // Black and white average to linear 0.5, which is sRGB ~188.
        let data = [0u8, 0, 0, 255, 255, 255, 255, 255];
        let (out, _) = generate_rgba8(&data, 2, 1, true);
        assert!((out[8] as i32 - 188).abs() <= 1, "{}", out[8]);
    }
}
