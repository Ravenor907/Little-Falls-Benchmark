//! Deterministic value noise and a small RNG. Seeds are fixed so every
//! machine renders an identical forest.

fn hash(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6b343)
        ^ (y as u32).wrapping_mul(0xd8163841)
        ^ (z as u32).wrapping_mul(0xcb1ab31f)
        ^ seed.wrapping_mul(0x9e3779b9);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1e995);
    h ^= h >> 15;
    (h & 0x00ff_ffff) as f32 / 0x00ff_ffff as f32
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// 3D value noise in 0..1.
pub fn value3(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let (xi, yi, zi) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
    let (tx, ty, tz) = (
        smooth(x - x.floor()),
        smooth(y - y.floor()),
        smooth(z - z.floor()),
    );
    let c = |dx, dy, dz| hash(xi + dx, yi + dy, zi + dz, seed);
    let x00 = lerp(c(0, 0, 0), c(1, 0, 0), tx);
    let x10 = lerp(c(0, 1, 0), c(1, 1, 0), tx);
    let x01 = lerp(c(0, 0, 1), c(1, 0, 1), tx);
    let x11 = lerp(c(0, 1, 1), c(1, 1, 1), tx);
    lerp(lerp(x00, x10, ty), lerp(x01, x11, ty), tz)
}

pub fn value2(x: f32, y: f32, seed: u32) -> f32 {
    value3(x, y, 0.5, seed)
}

/// Fractal noise in roughly -1..1.
pub fn fbm2(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    let (mut a, mut f, mut sum, mut norm) = (1.0, 1.0, 0.0, 0.0);
    for o in 0..octaves {
        sum += a * (value2(x * f, y * f, seed + o) * 2.0 - 1.0);
        norm += a;
        a *= 0.5;
        f *= 2.03;
    }
    sum / norm
}

pub fn fbm3(x: f32, y: f32, z: f32, octaves: u32, seed: u32) -> f32 {
    let (mut a, mut f, mut sum, mut norm) = (1.0, 1.0, 0.0, 0.0);
    for o in 0..octaves {
        sum += a * (value3(x * f, y * f, z * f, seed + o) * 2.0 - 1.0);
        norm += a;
        a *= 0.5;
        f *= 2.03;
    }
    sum / norm
}

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn u64(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in 0..1.
    pub fn f(&mut self) -> f32 {
        (self.u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    pub fn unit_vec(&mut self) -> bevy::math::Vec3 {
        loop {
            let v = bevy::math::Vec3::new(
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
            );
            let l = v.length_squared();
            if l > 0.01 && l <= 1.0 {
                return v / l.sqrt();
            }
        }
    }
}
