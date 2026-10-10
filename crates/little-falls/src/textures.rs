//! Procedurally painted textures: leaves, water streaks, mist.

use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

fn make(w: u32, h: u32, srgb: bool, f: impl Fn(f32, f32) -> [f32; 4]) -> Image {
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let c = f((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
            for v in c {
                data.push((v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            }
        }
    }
    let mut img = Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        if srgb {
            TextureFormat::Rgba8UnormSrgb
        } else {
            TextureFormat::Rgba8Unorm
        },
        Default::default(),
    );
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    img
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Value noise that tiles with integer period `p` on both axes.
fn periodic_noise(x: f32, y: f32, px: i32, py: i32, seed: u32) -> f32 {
    let h = |ix: i32, iy: i32| {
        let ix = ix.rem_euclid(px) as u32;
        let iy = iy.rem_euclid(py) as u32;
        let mut v = ix.wrapping_mul(0x8da6b343) ^ iy.wrapping_mul(0xd8163841) ^ seed.wrapping_mul(0x9e3779b9);
        v ^= v >> 13;
        v = v.wrapping_mul(0x5bd1e995);
        v ^= v >> 15;
        (v & 0xffff) as f32 / 65535.0
    };
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let a = h(xi, yi) + (h(xi + 1, yi) - h(xi, yi)) * sx;
    let b = h(xi, yi + 1) + (h(xi + 1, yi + 1) - h(xi, yi + 1)) * sx;
    a + (b - a) * sy
}

/// Bigleaf maple style leaf: five pointed lobes, bright veins. The color
/// is white-ish so vertex colors and material tint control the hue.
pub fn maple_leaf() -> Image {
    make(64, 64, true, |u, v| {
        let p = Vec2::new(u - 0.5, 0.55 - v) * 2.0;
        let r = p.length();
        let a = p.y.atan2(p.x);
        // Five lobes spread over the upper ~270 degrees.
        let lobes = (a * 2.5 + 1.57).cos().abs().powf(0.6);
        let edge = 0.55 + 0.4 * lobes;
        let inside = 1.0 - smoothstep(edge - 0.06, edge, r);
        let stem = (1.0 - smoothstep(0.02, 0.05, p.x.abs()))
            * smoothstep(0.0, -0.1, p.y)
            * (1.0 - smoothstep(0.7, 0.8, -p.y));
        let alpha = inside.max(stem);
        // Distance to the nearest of the radial veins (one per lobe).
        let k = ((a * 2.5 + 1.57) * 2.0 / std::f32::consts::PI).rem_euclid(1.0);
        let vein = 1.0 - smoothstep(0.0, 0.035, k.min(1.0 - k) * r);
        let shade = 0.82 + 0.18 * vein - 0.12 * r;
        [shade, shade, shade * 0.9, alpha]
    })
}

/// Rounded leaf for undergrowth (like the round-leaved plants at the
/// edge of the creek).
pub fn round_leaf() -> Image {
    make(64, 64, true, |u, v| {
        let p = Vec2::new(u - 0.5, v - 0.5) * 2.0;
        let r = p.length();
        let a = p.y.atan2(p.x);
        let scallop = 0.88 + 0.06 * (a * 9.0).cos();
        let alpha = 1.0 - smoothstep(scallop - 0.05, scallop, r);
        let vein = 1.0
            - smoothstep(
                0.0,
                0.03,
                ((a * 7.0 / std::f32::consts::TAU).fract() - 0.5).abs() * r,
            );
        let s = 0.78 + 0.15 * vein + 0.1 * (1.0 - r);
        [s, s, s * 0.85, alpha]
    })
}

/// Fern frond: a central rachis with paired leaflets.
pub fn fern() -> Image {
    make(32, 128, true, |u, v| {
        let x = (u - 0.5) * 2.0;
        // v = 0 is the frond tip, v = 1 the base.
        let taper = v * 0.9 + 0.1;
        let leaflet = ((v * 40.0).fract() - 0.5).abs() * 2.0;
        let width = taper * (0.55 + 0.45 * (1.0 - leaflet));
        let alpha = 1.0 - smoothstep(width - 0.08, width, x.abs());
        let rachis = 1.0 - smoothstep(0.03, 0.08, x.abs());
        let s = 0.75 + 0.2 * rachis;
        [s, s, s * 0.85, alpha.max(rachis)]
    })
}

/// Tileable white water streaks. `silky` produces the long exposure look
/// from slow shutter photos: soft, stretched and continuous.
pub fn water_streaks(silky: bool) -> Image {
    make(128, 256, true, move |u, v| {
        let (sx, sy) = if silky { (24.0, 2.0) } else { (32.0, 6.0) };
        let n1 = periodic_noise(u * sx, v * sy, sx as i32, sy as i32, 7);
        let n2 = periodic_noise(
            u * sx * 2.0,
            v * sy * 2.0,
            (sx * 2.0) as i32,
            (sy * 2.0) as i32,
            11,
        );
        let n3 = periodic_noise(u * 8.0, v * 4.0, 8, 4, 3);
        let n = n1 * 0.6 + n2 * 0.3 + n3 * 0.3;
        let alpha = if silky {
            smoothstep(0.25, 0.75, n) * 0.85 + 0.15
        } else {
            let bubbles = smoothstep(0.62, 0.7, periodic_noise(u * 48.0, v * 96.0, 48, 96, 19));
            (smoothstep(0.45, 0.8, n) + bubbles * 0.6).min(1.0)
        };
        [0.95, 0.97, 1.0, alpha]
    })
}

/// Soft round puff for mist and spray haze.
pub fn mist() -> Image {
    make(64, 64, true, |u, v| {
        let r = Vec2::new(u - 0.5, v - 0.5).length() * 2.0;
        let n = periodic_noise(u * 6.0, v * 6.0, 6, 6, 5);
        let a = (1.0 - smoothstep(0.2, 1.0, r)) * (0.6 + 0.4 * n);
        [1.0, 1.0, 1.0, a * a]
    })
}

/// Tileable fractal noise in 0..1 from periodic value noise octaves.
fn periodic_fbm(u: f32, v: f32, base: i32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut f) = (0.0, 1.0, 0.0, base);
    for o in 0..octaves {
        sum += amp * periodic_noise(u * f as f32, v * f as f32, f, f, seed + o);
        norm += amp;
        amp *= 0.5;
        f *= 2;
    }
    sum / norm
}

/// Forest floor detail: leaf litter, twigs and soil, in grayscale so the
/// terrain's vertex colors set the hue. Averages near 0.85.
pub fn ground_detail() -> Image {
    make(256, 256, true, |u, v| {
        let n = periodic_fbm(u, v, 8, 5, 21);
        let leaves = smoothstep(0.55, 0.62, periodic_noise(u * 40.0, v * 40.0, 40, 40, 33));
        let twig = 1.0
            - smoothstep(
                0.0,
                0.04,
                (periodic_noise(u * 6.0, v * 30.0, 6, 30, 41) - 0.5).abs(),
            );
        let g = 0.62 + 0.38 * n + 0.18 * leaves - 0.25 * twig;
        [g, g * 0.98, g * 0.95, 1.0]
    })
}

/// Granite-like speckle and cracks for boulders.
pub fn rock_detail() -> Image {
    make(256, 256, true, |u, v| {
        let n = periodic_fbm(u, v, 4, 6, 51);
        let speck = periodic_noise(u * 96.0, v * 96.0, 96, 96, 57);
        let crack = 1.0 - smoothstep(0.0, 0.025, (periodic_fbm(u, v, 3, 4, 61) - 0.5).abs());
        let g = 0.65 + 0.35 * n + 0.15 * (speck - 0.5) - 0.35 * crack;
        [g, g, g * 0.97, 1.0]
    })
}

/// Furrowed bark: vertical ridges with some horizontal breaks.
pub fn bark() -> Image {
    make(128, 256, true, |u, v| {
        let ridges = periodic_noise(u * 16.0, v * 2.0, 16, 2, 71);
        let breaks = periodic_noise(u * 8.0, v * 24.0, 8, 24, 73);
        let g = 0.55 + 0.45 * smoothstep(0.25, 0.75, ridges) - 0.15 * smoothstep(0.7, 0.9, breaks);
        [g, g * 0.97, g * 0.94, 1.0]
    })
}

#[derive(Resource)]
pub struct Textures {
    pub ground: Handle<Image>,
    pub rock: Handle<Image>,
    pub bark: Handle<Image>,
    pub maple_leaf: Handle<Image>,
    pub round_leaf: Handle<Image>,
    pub fern: Handle<Image>,
    pub streaks: Handle<Image>,
    pub silky: Handle<Image>,
    pub mist: Handle<Image>,
}

impl Textures {
    pub fn create(images: &mut Assets<Image>) -> Self {
        Self {
            ground: images.add(ground_detail()),
            rock: images.add(rock_detail()),
            bark: images.add(bark()),
            maple_leaf: images.add(maple_leaf()),
            round_leaf: images.add(round_leaf()),
            fern: images.add(fern()),
            streaks: images.add(water_streaks(false)),
            silky: images.add(water_streaks(true)),
            mist: images.add(mist()),
        }
    }
}
