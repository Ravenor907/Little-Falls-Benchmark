//! The creek: its path, the ravine terrain around it and the water
//! surfaces. Water flows from upstream (-Z) toward the viewer (+Z) and
//! drops down three rock ledges in the middle of the scene.

use crate::geom::MeshData;
use crate::noise::fbm2;
use bevy::prelude::*;

pub const X_EXTENT: f32 = 48.0;
pub const Z_MIN: f32 = -75.0;
pub const Z_MAX: f32 = 60.0;

/// Start and end of the cascade section.
pub const FALLS_TOP: f32 = -14.0;
pub const FALLS_BOTTOM: f32 = 4.0;
pub const FALLS_DROP: f32 = 9.0;
pub const LEDGES: usize = 3;

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Height of the creek bed along the flow.
pub fn bed(z: f32) -> f32 {
    if z < FALLS_TOP {
        FALLS_DROP + (FALLS_TOP - z) * 0.07
    } else if z > FALLS_BOTTOM {
        -(z - FALLS_BOTTOM) * 0.045
    } else {
        // Three ledges: flat pools broken by steep drops.
        let t = (z - FALLS_TOP) / (FALLS_BOTTOM - FALLS_TOP) * LEDGES as f32;
        let k = t.floor().min(LEDGES as f32 - 1.0);
        let s = (k + smoothstep(0.45, 0.85, t - k)) / LEDGES as f32;
        FALLS_DROP * (1.0 - s)
    }
}

/// Steepness of the bed (rise over run, positive downhill).
pub fn slope(z: f32) -> f32 {
    (bed(z - 0.1) - bed(z + 0.1)) / 0.2
}

/// Z positions where each falling sheet of water lands.
pub fn plunge_points() -> Vec<Vec3> {
    (0..LEDGES)
        .map(|k| {
            let z = FALLS_TOP + (k as f32 + 0.9) / LEDGES as f32 * (FALLS_BOTTOM - FALLS_TOP);
            Vec3::new(center(z), bed(z) + 0.15, z)
        })
        .collect()
}

/// X position of the channel center: a gentle meander.
pub fn center(z: f32) -> f32 {
    2.2 * (z * 0.06).sin() + 1.1 * (z * 0.17 + 1.0).sin()
}

pub fn half_width(z: f32) -> f32 {
    let falls =
        smoothstep(FALLS_TOP - 6.0, FALLS_TOP, z) * (1.0 - smoothstep(FALLS_BOTTOM, FALLS_BOTTOM + 6.0, z));
    2.4 + 0.5 * (z * 0.13).sin() - 0.5 * falls
}

/// Ground height anywhere in the scene.
pub fn ground(x: f32, z: f32) -> f32 {
    let d = (x - center(z)).abs();
    let w = half_width(z);
    let b = bed(z);
    let rough = fbm2(x * 0.09, z * 0.09, 5, 1);
    let fine = fbm2(x * 0.5, z * 0.5, 3, 2);
    if d < w {
        // Channel bottom sits under the water surface.
        b - 0.35 * (1.0 - (d / w).powi(2)) + fine * 0.08
    } else {
        let e = d - w;
        // Rocky banks rising into a steep ravine.
        let bank = e.powf(1.35) * 0.32 + 1.4 * smoothstep(0.0, 1.5, e);
        let ravine = 14.0 * smoothstep(8.0, 40.0, d);
        b + bank + ravine + rough * 3.0 * smoothstep(0.0, 6.0, e) + fine * 0.25 * smoothstep(0.0, 2.0, e)
    }
}

pub fn ground_normal(x: f32, z: f32) -> Vec3 {
    let e = 0.15;
    let hx = ground(x + e, z) - ground(x - e, z);
    let hz = ground(x, z + e) - ground(x, z - e);
    Vec3::new(-hx, 2.0 * e, -hz).normalize()
}

/// Terrain mesh with vertex colors for wet rock, moss and leaf litter.
/// Terrain mesh with vertex colors for the procedural look, plus layer
/// weights (mud, rock) per vertex for the scanned-texture terrain shader.
pub fn terrain(cell: f32) -> (MeshData, Vec<[f32; 2]>) {
    let nx = (2.0 * X_EXTENT / cell) as u32;
    let nz = ((Z_MAX - Z_MIN) / cell) as u32;
    let mut m = MeshData::default();
    let mut weights = Vec::new();
    for j in 0..=nz {
        for i in 0..=nx {
            let x = -X_EXTENT + i as f32 * cell;
            let z = Z_MIN + j as f32 * cell;
            let y = ground(x, z);
            let n = ground_normal(x, z);
            let d = (x - center(z)).abs() - half_width(z);
            let steep = 1.0 - n.y;
            let mottle = fbm2(x * 0.3, z * 0.3, 4, 9) * 0.5 + 0.5;
            let moss_n = fbm2(x * 0.15, z * 0.15, 3, 17) * 0.5 + 0.5;

            // Base palette (linear-ish sRGB values, converted by the GPU).
            let wet_rock = [0.07, 0.075, 0.07];
            let moss = [0.14 + 0.08 * mottle, 0.24 + 0.10 * mottle, 0.06];
            let litter = [0.26 + 0.08 * mottle, 0.18 + 0.05 * mottle, 0.11];
            let dirt = [0.22, 0.17, 0.12];

            // Near and below the waterline everything is dark wet rock.
            let wet = 1.0 - smoothstep(-0.2, 1.2, d);
            // Moss loves damp, steep, shaded rock close to the creek.
            let mossy = smoothstep(0.35, 0.65, moss_n) * (1.0 - smoothstep(4.0, 14.0, d)).max(0.25);
            let rocky = smoothstep(0.35, 0.6, steep);

            let mut c = [0.0f32; 3];
            for k in 0..3 {
                let ground_col = litter[k] * (1.0 - mossy) + moss[k] * mossy;
                let slope_col = dirt[k] * (1.0 - mossy) + moss[k] * mossy;
                let dry = ground_col * (1.0 - rocky) + (slope_col * 0.6 + wet_rock[k] * 0.4) * rocky;
                c[k] = dry * (1.0 - wet) + wet_rock[k] * wet;
            }
            let uv = Vec2::new(x * 0.4, z * 0.4);
            m.vertex(Vec3::new(x, y, z), n, uv, [c[0], c[1], c[2], 1.0]);
            let mud = 1.0 - smoothstep(-0.3, 2.2, d);
            let rock = smoothstep(0.42, 0.72, steep) * (0.6 + 0.4 * mottle);
            weights.push([mud, rock]);
        }
    }
    let row = nx + 1;
    for j in 0..nz {
        for i in 0..nx {
            let a = j * row + i;
            let b = a + row;
            m.tri(a, b, a + 1);
            m.tri(a + 1, b, b + 1);
        }
    }
    (m, weights)
}

/// Water ribbon following the channel. `lift` raises the layer (the foam
/// layer sits a few centimeters above the base water). The v texture
/// coordinate advances faster where the water is steep, so a uniform
/// texture scroll makes falls race and pools idle.
pub fn water_ribbon(lift: f32, foam: bool) -> MeshData {
    let mut m = MeshData::default();
    let across = 10u32;
    let step = 0.2;
    let n = ((Z_MAX - Z_MIN) / step) as u32;
    let mut v = 0.0f32;
    for j in 0..=n {
        let z = Z_MIN + j as f32 * step;
        let s = slope(z).max(0.0);
        let speed = 1.0 + 5.0 * s.min(2.0);
        if j > 0 {
            v -= step * speed * 0.25;
        }
        let c = center(z);
        let w = half_width(z) + 0.15;
        let b = bed(z);
        for i in 0..=across {
            let u = i as f32 / across as f32;
            let x = c - w + u * 2.0 * w;
            let edge = (u - 0.5).abs() * 2.0;
            // Water humps slightly in the middle and hugs the bed.
            let y = b + 0.12 + lift + 0.05 * (1.0 - edge * edge);
            let alpha = if foam {
                // Whitewater on the drops and in the churn just below
                // them; pools stay mostly clear, with a little foam
                // along the banks.
                let white = (s * 1.3).min(1.0);
                let churn = plunge_points()
                    .iter()
                    .map(|p| {
                        let d = z - p.z;
                        if (0.0..4.0).contains(&d) {
                            1.0 - d / 4.0
                        } else {
                            0.0
                        }
                    })
                    .fold(0.0f32, f32::max);
                let edges = smoothstep(0.8, 1.0, edge) * 0.3;
                (0.04 + white + churn * 0.8 + edges).min(1.0)
            } else {
                0.92 - 0.25 * smoothstep(0.8, 1.0, edge)
            };
            m.vertex(
                Vec3::new(x, y, z),
                Vec3::Y,
                Vec2::new(u * 1.5, v),
                [1.0, 1.0, 1.0, alpha],
            );
        }
    }
    let row = across + 1;
    for j in 0..n {
        for i in 0..across {
            let a = j * row + i;
            let b = a + row;
            m.tri(a, b, a + 1);
            m.tri(a + 1, b, b + 1);
        }
    }
    m.recompute_normals();
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bed_flows_downhill() {
        let mut prev = bed(Z_MIN);
        let mut z = Z_MIN;
        while z < Z_MAX {
            z += 0.25;
            let b = bed(z);
            assert!(b <= prev + 1e-4, "bed rises at z={z}");
            prev = b;
        }
        assert!(bed(FALLS_TOP) - bed(FALLS_BOTTOM) > FALLS_DROP * 0.9);
    }
}
