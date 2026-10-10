//! IEEE 754 binary16 conversion (round to nearest even).

pub fn f32_to_f16(v: f32) -> u16 {
    let x = v.to_bits();
    let sign = ((x >> 16) & 0x8000) as u16;
    let exp = ((x >> 23) & 0xff) as i32;
    let mant = x & 0x007f_ffff;

    if exp == 0xff {
        // Inf or NaN.
        return sign | 0x7c00 | if mant != 0 { 0x0200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00; // overflow to infinity
    }
    if e <= 0 {
        // Subnormal or zero.
        if e < -10 {
            return sign;
        }
        let m = mant | 0x0080_0000;
        let shift = (14 - e) as u32;
        let half = 1u32 << (shift - 1);
        let rounded = m + half - 1 + ((m >> shift) & 1);
        return sign | (rounded >> shift) as u16;
    }
    let rounded = mant + 0x0fff + ((mant >> 13) & 1);
    if rounded & 0x0080_0000 != 0 {
        // Mantissa overflowed into the exponent.
        let e = e + 1;
        if e >= 0x1f {
            return sign | 0x7c00;
        }
        return sign | ((e as u16) << 10);
    }
    sign | ((e as u16) << 10) | (rounded >> 13) as u16
}

pub fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h & 0x8000) as u32) << 16;
    let exp = ((h >> 10) & 0x1f) as u32;
    let mant = (h & 0x03ff) as u32;
    let bits = match exp {
        0 if mant == 0 => sign,
        0 => {
            // Subnormal: normalize.
            let mut e = 127 - 15 + 1;
            let mut m = mant;
            while m & 0x0400 == 0 {
                m <<= 1;
                e -= 1;
            }
            sign | (e << 23) | ((m & 0x03ff) << 13)
        }
        0x1f => sign | 0x7f80_0000 | (mant << 13),
        _ => sign | ((exp + 127 - 15) << 23) | (mant << 13),
    };
    f32::from_bits(bits)
}

/// Packs RGBA f32 pixels as little-endian RGBA16F bytes.
pub fn pack_rgba16f(px: &[[f32; 4]], out: &mut Vec<u8>) {
    out.reserve(px.len() * 8);
    for p in px {
        for c in p {
            out.extend_from_slice(&f32_to_f16(*c).to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values() {
        assert_eq!(f32_to_f16(0.0), 0);
        assert_eq!(f32_to_f16(1.0), 0x3c00);
        assert_eq!(f32_to_f16(-2.0), 0xc000);
        assert_eq!(f32_to_f16(65504.0), 0x7bff);
        assert_eq!(f32_to_f16(1e6), 0x7c00);
        assert_eq!(f32_to_f16(0.5), 0x3800);
    }

    #[test]
    fn roundtrip_is_close() {
        for &v in &[0.001f32, 0.1, 0.333, 1.5, 7.25, 100.0, 1234.5, 3.0e-5] {
            let r = f16_to_f32(f32_to_f16(v));
            assert!((r - v).abs() / v < 1e-3, "{v} -> {r}");
        }
    }
}
