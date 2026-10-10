//! Trees and undergrowth: bigleaf maples arching over the creek, tall
//! Douglas firs on the ravine walls, round-leaved plants and ferns along
//! the water.

use super::creek::{self, center, ground, ground_normal, half_width};
use super::SceneObject;
use crate::config::Quality;
use crate::geom::{self, MeshData};
use crate::noise::Rng;
use crate::textures::Textures;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

/// Adds a quad spanning `right` (width) and `up` (length) around `center`.
fn quad_axes(m: &mut MeshData, center: Vec3, right: Vec3, up: Vec3, normal: Vec3, size: Vec2, col: [f32; 4]) {
    let hx = right * size.x * 0.5;
    let hy = up * size.y * 0.5;
    let i0 = m.vertex(center - hx - hy, normal, Vec2::new(0.0, 1.0), col);
    let i1 = m.vertex(center + hx - hy, normal, Vec2::new(1.0, 1.0), col);
    let i2 = m.vertex(center + hx + hy, normal, Vec2::new(1.0, 0.0), col);
    let i3 = m.vertex(center - hx + hy, normal, Vec2::new(0.0, 0.0), col);
    m.tri(i0, i1, i2);
    m.tri(i0, i2, i3);
}

/// Adds one leaf quad facing `normal`, spun by `spin` radians.
fn leaf(m: &mut MeshData, center: Vec3, normal: Vec3, spin: f32, size: Vec2, col: [f32; 4]) {
    let n = normal.normalize_or(Vec3::Y);
    let t0 = n.any_orthonormal_vector();
    let (s, c) = spin.sin_cos();
    let t = t0 * c + n.cross(t0) * s;
    let b = n.cross(t);
    quad_axes(m, center, t, b, n, size, col);
}

fn bark_color(rng: &mut Rng, mossy: f32) -> [f32; 4] {
    let g = rng.range(-0.02, 0.02);
    let bark = [0.20 + g, 0.17 + g, 0.13 + g];
    let moss = [0.20, 0.30, 0.08];
    [
        bark[0] * (1.0 - mossy) + moss[0] * mossy,
        bark[1] * (1.0 - mossy) + moss[1] * mossy,
        bark[2] * (1.0 - mossy) + moss[2] * mossy,
        1.0,
    ]
}

/// Returns (wood, leaves) for a bigleaf maple rooted at `root`, leaning
/// toward `lean`.
fn maple(rng: &mut Rng, root: Vec3, lean: Vec3, leaves: usize) -> (MeshData, MeshData) {
    let mut wood = MeshData::default();
    let mut foliage = MeshData::default();
    let height = rng.range(9.0, 14.0);
    let trunk_dir = (Vec3::Y + lean * 0.35).normalize();
    let top = root + trunk_dir * height;
    // Moss climbs the lower trunk of creekside maples.
    let trunk = geom::cylinder(0.38, 0.22, height, 12, 6).paint(bark_color(rng, 0.65));
    wood.append(&trunk, geom::segment(root - Vec3::Y * 0.3, top));

    let mut clusters = Vec::new();
    let branches = 5 + (rng.u64() % 3) as usize;
    for b in 0..branches {
        let t = rng.range(0.45, 0.9);
        let start = root + trunk_dir * height * t;
        let ang = b as f32 / branches as f32 * std::f32::consts::TAU + rng.range(-0.4, 0.4);
        let out = Vec3::new(ang.cos(), 0.0, ang.sin());
        let dir = (out + Vec3::Y * rng.range(0.5, 1.1) + lean * 0.5).normalize();
        let len = rng.range(3.0, 6.0);
        let end = start + dir * len;
        let branch = geom::cylinder(0.14, 0.05, len, 6, 2).paint(bark_color(rng, 0.3));
        wood.append(&branch, geom::segment(start, end));
        clusters.push(end);
    }
    clusters.push(top);

    for _ in 0..leaves {
        let c = clusters[(rng.u64() % clusters.len() as u64) as usize];
        let off = rng.unit_vec() * rng.f().powf(0.5);
        let pos = c + Vec3::new(off.x * 2.6, off.y * 1.4, off.z * 2.6);
        let normal = (rng.unit_vec() + Vec3::Y * 0.9 + (pos - c).normalize_or_zero() * 0.6).normalize();
        let s = rng.range(0.32, 0.5);
        // Spring maple greens: a mix of lime and deeper green.
        let lime = rng.f();
        let col = [0.38 + 0.22 * lime, 0.58 + 0.17 * lime, 0.10 + 0.04 * lime, 1.0];
        leaf(
            &mut foliage,
            pos,
            normal,
            rng.range(0.0, 6.28),
            Vec2::splat(s),
            col,
        );
    }
    (wood, foliage)
}

/// Douglas fir: straight trunk with stacked, drooping cones of needles.
fn fir(rng: &mut Rng) -> MeshData {
    let mut m = MeshData::default();
    let h = rng.range(26.0, 36.0);
    let trunk = geom::cylinder(0.55, 0.12, h, 10, 4).paint(bark_color(rng, 0.15));
    m.append(&trunk, Mat4::IDENTITY);
    let layers = 11;
    for i in 0..layers {
        let t = i as f32 / layers as f32;
        let y = h * (0.32 + 0.66 * t);
        let r = (1.0 - t).powf(0.9) * rng.range(3.2, 4.2) + 0.4;
        let ch = r * 0.9;
        let g = rng.range(-0.015, 0.015);
        let cone = geom::cone(r, ch, 12).paint([0.045 + g, 0.11 + g, 0.05, 1.0]);
        m.append(
            &cone,
            Mat4::from_rotation_translation(
                Quat::from_rotation_y(rng.range(0.0, 6.28)),
                Vec3::Y * (y - ch * 0.3),
            ),
        );
    }
    m
}

pub fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    textures: &Textures,
    q: &Quality,
) {
    let mut rng = Rng::new(77);
    let wood_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.9,
        ..default()
    });
    let maple_leaf_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(textures.maple_leaf.clone()),
        alpha_mode: AlphaMode::Mask(0.5),
        double_sided: true,
        cull_mode: None,
        perceptual_roughness: 0.6,
        // Light passing through the leaves gives the bright backlit glow
        // of maples in the reference photos.
        diffuse_transmission: 0.55,
        ..default()
    });
    let needles_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        ..default()
    });

    // Maples: on the banks, leaning over the water.
    let mut placed = 0;
    let mut tries = 0;
    while placed < q.maples && tries < q.maples * 50 {
        tries += 1;
        let z = rng.range(creek::Z_MIN + 10.0, creek::Z_MAX - 5.0);
        let side = if rng.f() < 0.5 { -1.0 } else { 1.0 };
        let x = center(z) + side * (half_width(z) + rng.range(1.5, 9.0));
        let root = Vec3::new(x, ground(x, z), z);
        if ground_normal(x, z).y < 0.55 {
            continue;
        }
        let lean = Vec3::new(-side, 0.0, rng.range(-0.3, 0.3)).normalize();
        let (wood, foliage) = maple(&mut rng, root, lean, q.leaves_per_maple);
        commands.spawn((
            SceneObject,
            Mesh3d(meshes.add(wood.build())),
            MeshMaterial3d(wood_mat.clone()),
            Transform::IDENTITY,
        ));
        commands.spawn((
            SceneObject,
            Mesh3d(meshes.add(foliage.build())),
            MeshMaterial3d(maple_leaf_mat.clone()),
            Transform::IDENTITY,
        ));
        placed += 1;
    }

    // Firs: a handful of shared variants, instanced up the ravine walls.
    let fir_variants: Vec<Handle<Mesh>> = (0..5).map(|_| meshes.add(fir(&mut rng).build())).collect();
    let mut placed = 0;
    let mut tries = 0;
    while placed < q.conifers && tries < q.conifers * 50 {
        tries += 1;
        let z = rng.range(creek::Z_MIN, creek::Z_MAX);
        let x = rng.range(-creek::X_EXTENT + 2.0, creek::X_EXTENT - 2.0);
        let d = (x - center(z)).abs();
        if d < 10.0 {
            continue;
        }
        let v = fir_variants[(rng.u64() % fir_variants.len() as u64) as usize].clone();
        let s = rng.range(0.8, 1.2);
        commands.spawn((
            SceneObject,
            Mesh3d(v),
            MeshMaterial3d(needles_mat.clone()),
            Transform::from_xyz(x, ground(x, z) - 0.5, z)
                .with_rotation(Quat::from_rotation_y(rng.range(0.0, 6.28)))
                .with_scale(Vec3::splat(s)),
        ));
        placed += 1;
    }

    spawn_undergrowth(commands, meshes, materials, textures, q, &mut rng);
}

/// Round-leaved plants hugging the water and ferns on the damp banks,
/// merged into a few chunks along the creek so whole chunks can be culled.
fn spawn_undergrowth(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    textures: &Textures,
    q: &Quality,
    rng: &mut Rng,
) {
    const CHUNKS: usize = 8;
    let mut round: Vec<MeshData> = vec![MeshData::default(); CHUNKS];
    let mut ferns: Vec<MeshData> = vec![MeshData::default(); CHUNKS];
    let span = creek::Z_MAX - creek::Z_MIN;
    let mut leaves = 0;
    while leaves < q.undergrowth {
        let z = rng.range(creek::Z_MIN, creek::Z_MAX);
        let side = if rng.f() < 0.5 { -1.0 } else { 1.0 };
        // Mostly right at the water's edge, thinning out up the bank.
        let dist = rng.f().powf(2.0) * 10.0 + 0.2;
        let x = center(z) + side * (half_width(z) + dist);
        let base = Vec3::new(x, ground(x, z), z);
        let n = ground_normal(x, z);
        let chunk = (((z - creek::Z_MIN) / span) * CHUNKS as f32).clamp(0.0, CHUNKS as f32 - 1.0) as usize;
        if rng.f() < 0.55 {
            // Round-leaf plant: a rosette of leaves on short stems.
            let count = 5 + (rng.u64() % 5) as usize;
            let g = rng.f();
            let col = [0.30 + 0.15 * g, 0.55 + 0.15 * g, 0.12, 1.0];
            for k in 0..count {
                let a = k as f32 / count as f32 * 6.28 + rng.range(-0.3, 0.3);
                let r = rng.range(0.1, 0.3);
                let pos = base + Vec3::new(a.cos() * r, rng.range(0.1, 0.35), a.sin() * r);
                let tilt = (n * 1.5 + Vec3::new(a.cos(), 0.0, a.sin()) * 0.6).normalize();
                let s = rng.range(0.16, 0.3);
                leaf(
                    &mut round[chunk],
                    pos,
                    tilt,
                    rng.range(0.0, 6.28),
                    Vec2::splat(s),
                    col,
                );
            }
            leaves += count;
        } else if dist > 1.0 {
            // Sword fern: arching fronds radiating from a crown.
            let count = 7 + (rng.u64() % 6) as usize;
            for k in 0..count {
                let a = k as f32 / count as f32 * 6.28 + rng.range(-0.2, 0.2);
                let out = Vec3::new(a.cos(), 0.0, a.sin());
                let len = rng.range(0.6, 1.1);
                let pos = base + out * len * 0.35 + Vec3::Y * len * 0.3;
                let normal = (Vec3::Y * 0.8 - out * 0.5 + n * 0.3).normalize();
                // Long axis arches outward and up from the crown.
                let up = (out + Vec3::Y * 0.9).normalize();
                let right = up.cross(normal).normalize_or(Vec3::X);
                let normal = right.cross(up);
                quad_axes(
                    &mut ferns[chunk],
                    pos,
                    right,
                    up,
                    normal,
                    Vec2::new(0.22, len),
                    [0.12, 0.30, 0.07, 1.0],
                );
            }
            leaves += count;
        }
    }

    let round_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(textures.round_leaf.clone()),
        alpha_mode: AlphaMode::Mask(0.5),
        double_sided: true,
        cull_mode: None,
        perceptual_roughness: 0.45,
        diffuse_transmission: 0.35,
        ..default()
    });
    let fern_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(textures.fern.clone()),
        alpha_mode: AlphaMode::Mask(0.5),
        double_sided: true,
        cull_mode: None,
        perceptual_roughness: 0.7,
        diffuse_transmission: 0.3,
        ..default()
    });
    for m in round.into_iter().filter(|m| !m.is_empty()) {
        commands.spawn((
            SceneObject,
            Mesh3d(meshes.add(m.build())),
            MeshMaterial3d(round_mat.clone()),
            Transform::IDENTITY,
            NotShadowCaster,
        ));
    }
    for m in ferns.into_iter().filter(|m| !m.is_empty()) {
        commands.spawn((
            SceneObject,
            Mesh3d(meshes.add(m.build())),
            MeshMaterial3d(fern_mat.clone()),
            Transform::IDENTITY,
            NotShadowCaster,
        ));
    }
}
