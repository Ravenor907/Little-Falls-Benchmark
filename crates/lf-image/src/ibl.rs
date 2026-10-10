//! Image based lighting from an equirectangular HDR environment.
//!
//! Produces the three cubemaps a PBR renderer needs:
//! - skybox: the environment itself
//! - diffuse: cosine-convolved irradiance, via 3rd order spherical harmonics
//! - specular: GGX prefiltered radiance, one roughness per mip level
//!
//! Faces follow the Vulkan/D3D cubemap order and orientation:
//! +X, -X, +Y, -Y, +Z, -Z.

use std::f32::consts::PI;

pub type Rgb = [f32; 3];

/// A float RGB equirectangular image with a box-filtered pyramid so
/// rough reflections can sample pre-blurred levels.
pub struct Equirect {
    levels: Vec<(usize, usize, Vec<Rgb>)>,
    /// Rotation around the vertical axis, in radians.
    pub yaw: f32,
}

impl Equirect {
    pub fn new(width: usize, height: usize, pixels: Vec<Rgb>, yaw: f32) -> Self {
        assert_eq!(pixels.len(), width * height);
        let mut levels = vec![(width, height, pixels)];
        while levels.last().unwrap().0 > 8 {
            let (w, h, src) = levels.last().unwrap();
            let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
            let mut dst = vec![[0.0; 3]; nw * nh];
            for y in 0..nh {
                for x in 0..nw {
                    let mut a = [0.0; 3];
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let p = src[((y * 2 + dy).min(h - 1)) * w + (x * 2 + dx).min(w - 1)];
                        for c in 0..3 {
                            a[c] += p[c] * 0.25;
                        }
                    }
                    dst[y * nw + x] = a;
                }
            }
            levels.push((nw, nh, dst));
        }
        Self { levels, yaw }
    }

    pub fn width(&self) -> usize {
        self.levels[0].0
    }

    /// Bilinear sample in direction `d` at pyramid level `lod` (fractional).
    pub fn sample(&self, d: [f32; 3], lod: f32) -> Rgb {
        let lod = lod.clamp(0.0, (self.levels.len() - 1) as f32);
        let l0 = lod.floor() as usize;
        let l1 = (l0 + 1).min(self.levels.len() - 1);
        let t = lod - l0 as f32;
        let a = self.sample_level(d, l0);
        if t <= 0.0 || l1 == l0 {
            return a;
        }
        let b = self.sample_level(d, l1);
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    }

    fn sample_level(&self, d: [f32; 3], level: usize) -> Rgb {
        let (w, h, px) = &self.levels[level];
        let (w, h) = (*w, *h);
        let phi = d[0].atan2(-d[2]) + self.yaw;
        let theta = d[1].clamp(-1.0, 1.0).acos();
        let u = (phi / (2.0 * PI) + 0.5).rem_euclid(1.0) * w as f32 - 0.5;
        let v = (theta / PI * h as f32 - 0.5).clamp(0.0, (h - 1) as f32);
        let (x0, y0) = (u.floor(), v.floor());
        let (fx, fy) = (u - x0, v - y0);
        let xi = |x: f32| (x as isize).rem_euclid(w as isize) as usize;
        let (xa, xb) = (xi(x0), xi(x0 + 1.0));
        let (ya, yb) = (y0 as usize, (y0 as usize + 1).min(h - 1));
        let p = |x: usize, y: usize| px[y * w + x];
        let mut o = [0.0; 3];
        for c in 0..3 {
            let top = p(xa, ya)[c] * (1.0 - fx) + p(xb, ya)[c] * fx;
            let bot = p(xa, yb)[c] * (1.0 - fx) + p(xb, yb)[c] * fx;
            o[c] = top * (1.0 - fy) + bot * fy;
        }
        o
    }
}

/// World direction for texel (x, y) of cube face `face` with `size` texels.
pub fn face_dir(face: usize, x: usize, y: usize, size: usize) -> [f32; 3] {
    let u = 2.0 * (x as f32 + 0.5) / size as f32 - 1.0;
    let v = 2.0 * (y as f32 + 0.5) / size as f32 - 1.0;
    let d = match face {
        0 => [1.0, -v, -u],
        1 => [-1.0, -v, u],
        2 => [u, 1.0, v],
        3 => [u, -1.0, -v],
        4 => [u, -v, 1.0],
        _ => [-u, -v, -1.0],
    };
    normalize(d)
}

fn normalize(d: [f32; 3]) -> [f32; 3] {
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    [d[0] / l, d[1] / l, d[2] / l]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// One cubemap: `faces[f][mip]` holds RGBA pixels (alpha = 1).
pub struct Cubemap {
    pub size: usize,
    pub mips: usize,
    pub faces: Vec<Vec<Vec<[f32; 4]>>>,
}

impl Cubemap {
    /// Layer-major layout (face 0 all mips, face 1 all mips, ...), packed
    /// as RGBA16F, which is what GPUs and Bevy expect for cube arrays.
    pub fn to_rgba16f(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for face in &self.faces {
            for mip in face {
                crate::half::pack_rgba16f(mip, &mut out);
            }
        }
        out
    }
}

/// Skybox: the environment resampled to a cube, single mip.
pub fn skybox(env: &Equirect, size: usize) -> Cubemap {
    // Pick the pyramid level whose texel density matches the face size.
    let lod = ((env.width() as f32 / (size as f32 * 4.0)).log2()).max(0.0);
    let faces = (0..6)
        .map(|f| {
            let mut px = Vec::with_capacity(size * size);
            for y in 0..size {
                for x in 0..size {
                    let c = env.sample(face_dir(f, x, y, size), lod);
                    px.push([c[0], c[1], c[2], 1.0]);
                }
            }
            vec![px]
        })
        .collect();
    Cubemap { size, mips: 1, faces }
}

/// Third order spherical harmonics projection of the environment.
pub fn sh9(env: &Equirect) -> [Rgb; 9] {
    // Integrate over a coarse level; irradiance is very low frequency.
    let level = env
        .levels
        .iter()
        .position(|l| l.0 <= 128)
        .unwrap_or(env.levels.len() - 1);
    let (w, h, _) = &env.levels[level];
    let (w, h) = (*w, *h);
    let mut sh = [[0.0f32; 3]; 9];
    for y in 0..h {
        let theta = (y as f32 + 0.5) / h as f32 * PI;
        let d_omega = (2.0 * PI / w as f32) * (PI / h as f32) * theta.sin();
        for x in 0..w {
            let phi = (x as f32 + 0.5) / w as f32 * 2.0 * PI - PI - env.yaw;
            // Inverse of the mapping in `sample_level`.
            let d = [theta.sin() * phi.sin(), theta.cos(), -theta.sin() * phi.cos()];
            let c = env.sample(d, level as f32);
            let b = sh_basis(d);
            for (i, bi) in b.iter().enumerate() {
                for k in 0..3 {
                    sh[i][k] += c[k] * bi * d_omega;
                }
            }
        }
    }
    sh
}

fn sh_basis(d: [f32; 3]) -> [f32; 9] {
    let [x, y, z] = d;
    [
        0.282_095,
        0.488_603 * y,
        0.488_603 * z,
        0.488_603 * x,
        1.092_548 * x * y,
        1.092_548 * y * z,
        0.315_392 * (3.0 * z * z - 1.0),
        1.092_548 * x * z,
        0.546_274 * (x * x - y * y),
    ]
}

/// Diffuse irradiance cubemap from SH, already divided by pi so it reads
/// as outgoing radiance from a white Lambertian surface (what PBR
/// renderers expect in the diffuse environment map).
pub fn diffuse(sh: &[Rgb; 9], size: usize) -> Cubemap {
    // Cosine lobe convolution weights per band, divided by pi.
    let a = [1.0, 2.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 0.25, 0.25, 0.25, 0.25, 0.25];
    let faces = (0..6)
        .map(|f| {
            let mut px = Vec::with_capacity(size * size);
            for y in 0..size {
                for x in 0..size {
                    let b = sh_basis(face_dir(f, x, y, size));
                    let mut c = [0.0f32; 3];
                    for i in 0..9 {
                        for k in 0..3 {
                            c[k] += sh[i][k] * a[i] * b[i];
                        }
                    }
                    px.push([c[0].max(0.0), c[1].max(0.0), c[2].max(0.0), 1.0]);
                }
            }
            vec![px]
        })
        .collect();
    Cubemap { size, mips: 1, faces }
}

fn hammersley(i: u32, n: u32) -> (f32, f32) {
    (i as f32 / n as f32, i.reverse_bits() as f32 * 2.328_306_4e-10)
}

/// GGX prefiltered specular cubemap; mip `m` holds roughness m / (mips-1).
pub fn specular(env: &Equirect, size: usize, mips: usize, samples: u32) -> Cubemap {
    let mut faces: Vec<Vec<Vec<[f32; 4]>>> = (0..6).map(|_| Vec::with_capacity(mips)).collect();
    let src_w = env.width() as f32;
    for m in 0..mips {
        let s = (size >> m).max(1);
        let rough = m as f32 / (mips - 1).max(1) as f32;
        let a = (rough * rough).max(1e-4);
        let base_lod = (src_w / (s as f32 * 4.0)).log2().max(0.0);
        for (f, face) in faces.iter_mut().enumerate() {
            let mut px = Vec::with_capacity(s * s);
            for y in 0..s {
                for x in 0..s {
                    let n = face_dir(f, x, y, s);
                    let c = if m == 0 {
                        env.sample(n, base_lod)
                    } else {
                        convolve_ggx(env, n, a, samples, base_lod)
                    };
                    px.push([c[0], c[1], c[2], 1.0]);
                }
            }
            face.push(px);
        }
    }
    Cubemap { size, mips, faces }
}

fn convolve_ggx(env: &Equirect, n: [f32; 3], a: f32, samples: u32, base_lod: f32) -> Rgb {
    // Tangent frame around n.
    let up = if n[1].abs() < 0.999 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let t = normalize(cross(up, n));
    let b = cross(n, t);
    let mut acc = [0.0f32; 3];
    let mut wsum = 0.0;
    // Wider lobes read from blurrier pyramid levels to avoid sparkle.
    let lod = base_lod + 0.5 * (samples as f32).log2().min(6.0) * a.sqrt();
    for i in 0..samples {
        let (u1, u2) = hammersley(i, samples);
        let phi = 2.0 * PI * u1;
        let cos_t = ((1.0 - u2) / (1.0 + (a * a - 1.0) * u2)).sqrt();
        let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
        let hl = [sin_t * phi.cos(), sin_t * phi.sin(), cos_t];
        let h = [
            t[0] * hl[0] + b[0] * hl[1] + n[0] * hl[2],
            t[1] * hl[0] + b[1] * hl[1] + n[1] * hl[2],
            t[2] * hl[0] + b[2] * hl[1] + n[2] * hl[2],
        ];
        // V = N assumption: L = reflect(-N, H).
        let ndh = dot(n, h);
        let l = [
            2.0 * ndh * h[0] - n[0],
            2.0 * ndh * h[1] - n[1],
            2.0 * ndh * h[2] - n[2],
        ];
        let ndl = dot(n, l);
        if ndl > 0.0 {
            let c = env.sample(l, lod);
            for k in 0..3 {
                acc[k] += c[k] * ndl;
            }
            wsum += ndl;
        }
    }
    if wsum > 0.0 {
        [acc[0] / wsum, acc[1] / wsum, acc[2] / wsum]
    } else {
        env.sample(n, base_lod)
    }
}

/// The brightest direction of the environment (roughly where the sun is),
/// searched on a coarse level. Useful to aim the scene's sun light.
pub fn brightest_direction(env: &Equirect) -> [f32; 3] {
    let level = env
        .levels
        .iter()
        .position(|l| l.0 <= 64)
        .unwrap_or(env.levels.len() - 1);
    let (w, h, px) = &env.levels[level];
    let (w, h) = (*w, *h);
    let (mut best, mut bi) = (-1.0f32, 0);
    for (i, p) in px.iter().enumerate() {
        let lum = 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
        if lum > best {
            best = lum;
            bi = i;
        }
    }
    let (x, y) = (bi % w, bi / w);
    let theta = (y as f32 + 0.5) / h as f32 * PI;
    let phi = (x as f32 + 0.5) / w as f32 * 2.0 * PI - PI - env.yaw;
    [theta.sin() * phi.sin(), theta.cos(), -theta.sin() * phi.cos()]
}

/// Parses a Radiance .hdr (RGBE, new-style RLE or flat) into RGB floats.
pub fn decode_hdr(bytes: &[u8]) -> Result<(usize, usize, Vec<Rgb>), String> {
    let mut pos = 0;
    let mut line = || -> Option<String> {
        let start = pos;
        while pos < bytes.len() && bytes[pos] != b'\n' {
            pos += 1;
        }
        let l = String::from_utf8_lossy(&bytes[start..pos]).to_string();
        pos += 1;
        (start < bytes.len()).then_some(l)
    };
    let first = line().ok_or("empty file")?;
    if !first.starts_with("#?") {
        return Err("not a Radiance HDR file".into());
    }
    loop {
        let l = line().ok_or("missing header end")?;
        if l.trim().is_empty() {
            break;
        }
        if l.starts_with("FORMAT=") && !l.contains("32-bit_rle_rgbe") {
            return Err(format!("unsupported format {l}"));
        }
    }
    let dims = line().ok_or("missing size line")?;
    let parts: Vec<&str> = dims.split_whitespace().collect();
    if parts.len() != 4 || parts[0] != "-Y" || parts[2] != "+X" {
        return Err(format!("unsupported orientation '{dims}'"));
    }
    let h: usize = parts[1].parse().map_err(|_| "bad height")?;
    let w: usize = parts[3].parse().map_err(|_| "bad width")?;
    let mut out = Vec::with_capacity(w * h);
    let mut scan = vec![[0u8; 4]; w];
    for _ in 0..h {
        let rle = (8..0x8000).contains(&w)
            && pos + 4 <= bytes.len()
            && bytes[pos] == 2
            && bytes[pos + 1] == 2
            && bytes[pos + 2] & 0x80 == 0;
        if rle {
            pos += 4;
            for c in 0..4 {
                let mut x = 0;
                while x < w {
                    let count = *bytes.get(pos).ok_or("truncated")? as usize;
                    pos += 1;
                    if count > 128 {
                        let n = count - 128;
                        let v = *bytes.get(pos).ok_or("truncated")?;
                        pos += 1;
                        for _ in 0..n.min(w - x) {
                            scan[x][c] = v;
                            x += 1;
                        }
                    } else {
                        for _ in 0..count.min(w - x) {
                            scan[x][c] = *bytes.get(pos).ok_or("truncated")?;
                            pos += 1;
                            x += 1;
                        }
                    }
                }
            }
        } else {
            for p in scan.iter_mut() {
                let s = bytes.get(pos..pos + 4).ok_or("truncated")?;
                p.copy_from_slice(s);
                pos += 4;
            }
        }
        for p in &scan {
            if p[3] == 0 {
                out.push([0.0; 3]);
            } else {
                let f = 2f32.powi(p[3] as i32 - 136);
                out.push([p[0] as f32 * f, p[1] as f32 * f, p[2] as f32 * f]);
            }
        }
    }
    Ok((w, h, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constant(c: Rgb) -> Equirect {
        Equirect::new(64, 32, vec![c; 64 * 32], 0.0)
    }

    #[test]
    fn faces_point_the_right_way() {
        let s = 8;
        let center = |f| face_dir(f, s / 2, s / 2, s);
        assert!(center(0)[0] > 0.9);
        assert!(center(1)[0] < -0.9);
        assert!(center(2)[1] > 0.9);
        assert!(center(3)[1] < -0.9);
        assert!(center(4)[2] > 0.9);
        assert!(center(5)[2] < -0.9);
    }

    #[test]
    fn uniform_environment_stays_uniform() {
        let env = constant([0.5, 1.0, 2.0]);
        let d = diffuse(&sh9(&env), 4);
        for face in &d.faces {
            for p in &face[0] {
                for (k, want) in [0.5, 1.0, 2.0].iter().enumerate() {
                    assert!((p[k] - want).abs() / want < 0.03, "{p:?}");
                }
            }
        }
        let s = specular(&env, 8, 4, 32);
        let p = s.faces[2][3][0];
        assert!((p[1] - 1.0).abs() < 0.02, "{p:?}");
    }

    #[test]
    fn finds_the_sun() {
        let mut px = vec![[0.1f32; 3]; 64 * 32];
        // A bright spot in the upper hemisphere.
        px[6 * 64 + 40] = [100.0; 3];
        let env = Equirect::new(64, 32, px, 0.0);
        let d = brightest_direction(&env);
        assert!(d[1] > 0.5, "{d:?}");
        // The skybox should see it in the same direction.
        let c = env.sample(d, 0.0);
        assert!(c[0] > 1.0, "{c:?}");
    }

    #[test]
    fn decodes_flat_hdr() {
        let mut f = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 1 +X 2\n".to_vec();
        f.extend_from_slice(&[128, 64, 32, 129, 0, 0, 0, 0]);
        let (w, h, px) = decode_hdr(&f).unwrap();
        assert_eq!((w, h), (2, 1));
        assert!((px[0][0] - 1.0).abs() < 1e-6);
        assert_eq!(px[1], [0.0; 3]);
    }
}
