//! Places the photo-scanned assets: boulders along the banks, rock faces
//! framing each ledge of the falls, the logjam, stumps, roots, ferns,
//! celandine, fir saplings and woody debris. Densities follow the preset.

use super::creek::{self, bed, center, ground, ground_normal, half_width};
use super::SceneObject;
use crate::assets::{Library, Piece};
use crate::config::Quality;
use crate::noise::Rng;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, TAU};

struct Placer<'a> {
    lib: &'a Library,
    materials: &'a mut Assets<StandardMaterial>,
    /// Darker, glossier copies of materials for surfaces near the water.
    wet: HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>,
    count: usize,
}

impl Placer<'_> {
    fn wet_variant(&mut self, m: &Handle<StandardMaterial>) -> Handle<StandardMaterial> {
        if let Some(h) = self.wet.get(&m.id()) {
            return h.clone();
        }
        let wet = self.materials.get(m).cloned().map(|mut mat| {
            let c = mat.base_color.to_linear();
            mat.base_color =
                Color::LinearRgba(LinearRgba::new(c.red * 0.6, c.green * 0.6, c.blue * 0.6, c.alpha));
            mat.perceptual_roughness *= 0.38;
            mat.reflectance = 0.6;
            mat
        });
        let h = match wet {
            Some(mat) => self.materials.add(mat),
            None => m.clone(),
        };
        self.wet.insert(m.id(), h.clone());
        h
    }

    fn spawn(&mut self, commands: &mut Commands, piece: &Piece, tf: Transform, wet: bool, shadows: bool) {
        for (mesh, mat) in &piece.prims {
            let mat = if wet { self.wet_variant(mat) } else { mat.clone() };
            let mut e = commands.spawn((SceneObject, Mesh3d(mesh.clone()), MeshMaterial3d(mat), tf));
            if !shadows {
                e.insert(NotShadowCaster);
            }
        }
        self.count += 1;
    }

    /// Sets `piece` on the ground at (x, z), sunk by `sink` of its height,
    /// tilted partway toward the ground normal.
    #[allow(clippy::too_many_arguments)]
    fn on_ground(
        &mut self,
        commands: &mut Commands,
        piece: &Piece,
        x: f32,
        z: f32,
        yaw: f32,
        scale: f32,
        sink: f32,
        tilt: f32,
        wet: bool,
        shadows: bool,
    ) {
        let n = ground_normal(x, z);
        let up = Vec3::Y.lerp(n, tilt).normalize();
        let rot = Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(yaw);
        let h = piece.size().y * scale;
        let y = ground(x, z) - piece.min.y * scale - sink * h;
        let tf = Transform::from_xyz(x, y, z)
            .with_rotation(rot)
            .with_scale(Vec3::splat(scale));
        self.spawn(commands, piece, tf, wet, shadows);
    }
}

fn pick<'a>(rng: &mut Rng, pieces: &'a [Piece]) -> Option<&'a Piece> {
    if pieces.is_empty() {
        None
    } else {
        Some(&pieces[(rng.u64() % pieces.len() as u64) as usize])
    }
}

/// Distance from the water's edge (negative in the channel).
fn from_water(x: f32, z: f32) -> f32 {
    (x - center(z)).abs() - half_width(z)
}

pub fn spawn(commands: &mut Commands, lib: &Library, materials: &mut Assets<StandardMaterial>, q: &Quality) {
    let mut p = Placer {
        lib,
        materials,
        wet: HashMap::new(),
        count: 0,
    };
    let mut rng = Rng::new(2024);
    let rich = q.shadow_cascades > 2;

    let set1 = p.lib.pieces("rock_moss_set_01").to_vec();
    let set2 = p.lib.pieces("rock_moss_set_02").to_vec();
    let dark = p.lib.pieces("rock_09").to_vec();
    let face = p.lib.pieces("rock_face_02").to_vec();

    // Boulders lining both banks.
    let mut z = creek::Z_MIN;
    while z < creek::Z_MAX {
        z += rng.range(1.0, 2.6);
        for side in [-1.0f32, 1.0] {
            let big = rng.f() < 0.4;
            let (pool, scale) = if big {
                (&set1, rng.range(0.32, 0.7))
            } else {
                (&set2, rng.range(0.55, 1.15))
            };
            let Some(piece) = pick(&mut rng, pool) else {
                continue;
            };
            let x = center(z) + side * (half_width(z) + rng.range(-0.3, 1.5));
            let wet = from_water(x, z) < 0.8;
            p.on_ground(
                commands,
                piece,
                x,
                z,
                rng.range(0.0, TAU),
                scale,
                0.4,
                0.3,
                wet,
                true,
            );
        }
        // Dark stones breaking the surface of the stream.
        if rng.f() < 0.35 {
            if let Some(piece) = pick(&mut rng, &dark) {
                let x = center(z) + rng.range(-0.7, 0.7) * half_width(z);
                let tf = Transform::from_xyz(x, bed(z) - 0.15, z)
                    .with_rotation(Quat::from_rotation_y(rng.range(0.0, TAU)))
                    .with_scale(Vec3::splat(rng.range(10.0, 18.0)));
                p.spawn(commands, piece, tf, true, true);
            }
        }
    }

    // The falls: layered rock faces as the walls of every drop, dark wet
    // boulders on the lips, a splitter rock, and rubble in each pool.
    for (k, pp) in creek::plunge_points().iter().enumerate() {
        let z_lip = pp.z - 2.2;
        for side in [-1.0f32, 1.0] {
            if let Some(piece) = pick(&mut rng, &face) {
                let s = rng.range(0.9, 1.2);
                let x = center(z_lip) + side * (half_width(z_lip) + 1.3);
                let yaw = side * FRAC_PI_2 + rng.range(-0.25, 0.25);
                let tf = Transform::from_xyz(x, bed(z_lip) - 0.9, z_lip)
                    .with_rotation(Quat::from_rotation_y(yaw))
                    .with_scale(Vec3::splat(s));
                p.spawn(commands, piece, tf, true, true);
            }
            if let Some(piece) = pick(&mut rng, &dark) {
                let x = center(z_lip) + side * (half_width(z_lip) - 0.2);
                let tf = Transform::from_xyz(x, bed(z_lip) - 0.2, z_lip + 0.4)
                    .with_rotation(Quat::from_rotation_y(rng.range(0.0, TAU)))
                    .with_scale(Vec3::splat(rng.range(16.0, 24.0)));
                p.spawn(commands, piece, tf, true, true);
            }
        }
        if k != 1 {
            if let Some(piece) = pick(&mut rng, &dark) {
                let tf = Transform::from_xyz(pp.x + 0.4, bed(pp.z - 0.8) - 0.1, pp.z - 0.8)
                    .with_rotation(Quat::from_rotation_y(rng.range(0.0, TAU)))
                    .with_scale(Vec3::splat(14.0));
                p.spawn(commands, piece, tf, true, true);
            }
        }
        for _ in 0..4 {
            let pool = if rng.f() < 0.5 { &dark } else { &set2 };
            if let Some(piece) = pick(&mut rng, pool) {
                let s = if std::ptr::eq(pool, &dark) {
                    rng.range(6.0, 10.0)
                } else {
                    rng.range(0.25, 0.42)
                };
                let pos = *pp + Vec3::new(rng.range(-1.5, 1.5), -0.3, rng.range(0.8, 2.6));
                let tf = Transform::from_translation(pos)
                    .with_rotation(Quat::from_rotation_y(rng.range(0.0, TAU)))
                    .with_scale(Vec3::splat(s));
                p.spawn(commands, piece, tf, true, true);
            }
        }
    }

    // Logjam across the top of the falls, and one trunk leaning into it.
    if let Some(trunk) = p.lib.pieces("dead_tree_trunk_02").first().cloned() {
        let top = creek::FALLS_TOP;
        let logs = [
            (
                Vec3::new(center(top) + 0.5, bed(top) + 2.0, top + 0.4),
                Quat::from_rotation_y(0.25),
                2.3,
            ),
            (
                Vec3::new(center(top) - 0.3, bed(top) + 1.2, top + 1.8),
                Quat::from_rotation_y(-0.22) * Quat::from_rotation_x(0.08),
                2.0,
            ),
            (
                Vec3::new(center(top + 3.0) + 1.0, bed(top + 3.0) + 1.4, top + 3.2),
                Quat::from_rotation_y(FRAC_PI_2 + 0.3) * Quat::from_rotation_z(-0.42),
                1.6,
            ),
        ];
        for (pos, rot, s) in logs {
            let tf = Transform::from_translation(pos)
                .with_rotation(rot)
                .with_scale(Vec3::splat(s));
            p.spawn(commands, &trunk, tf, false, true);
        }
        // Fallen trunks on the banks.
        for (x, z) in [(9.0, -30.0), (-9.0, 14.0), (8.0, 28.0), (-11.0, -48.0)] {
            let s = rng.range(1.3, 1.7);
            p.on_ground(
                commands,
                &trunk,
                x,
                z,
                rng.range(0.0, TAU),
                s,
                0.15,
                0.8,
                false,
                true,
            );
        }
    }

    // Stumps, exposed roots along the water, and woody debris.
    let stumps = p.lib.pieces("tree_stump_01").to_vec();
    let roots = p.lib.pieces("pine_roots").to_vec();
    let branches = p.lib.pieces("dry_branches_medium_01").to_vec();
    let bark = p.lib.pieces("bark_debris_01").to_vec();
    for _ in 0..9 {
        if let Some(piece) = pick(&mut rng, &stumps) {
            let (x, z) = bank_point(&mut rng, 3.0, 14.0);
            p.on_ground(
                commands,
                piece,
                x,
                z,
                rng.range(0.0, TAU),
                rng.range(1.1, 1.6),
                0.1,
                0.4,
                false,
                true,
            );
        }
    }
    for _ in 0..12 {
        if let Some(piece) = pick(&mut rng, &roots) {
            let (x, z) = bank_point(&mut rng, 0.1, 1.2);
            let side = (x - center(z)).signum();
            // Roots reach toward the water.
            let yaw = if side > 0.0 { 0.0 } else { std::f32::consts::PI } + rng.range(-0.4, 0.4);
            p.on_ground(
                commands,
                piece,
                x,
                z,
                yaw,
                rng.range(1.4, 2.0),
                0.05,
                0.8,
                true,
                rich,
            );
        }
    }
    let debris = q.undergrowth / 80;
    for i in 0..debris * 2 {
        let pool = if i % 2 == 0 { &branches } else { &bark };
        if let Some(piece) = pick(&mut rng, pool) {
            let (x, z) = bank_point(&mut rng, 0.5, 12.0);
            p.on_ground(
                commands,
                piece,
                x,
                z,
                rng.range(0.0, TAU),
                rng.range(1.0, 1.6),
                0.0,
                1.0,
                false,
                false,
            );
        }
    }

    // Ferns on the banks, celandine at the water's edge, fir saplings.
    let ferns = p.lib.pieces("fern_02").to_vec();
    for _ in 0..q.undergrowth / 14 {
        if let Some(piece) = pick(&mut rng, &ferns) {
            let (x, z) = bank_point(&mut rng, 0.8, 11.0);
            if ground_normal(x, z).y < 0.6 {
                continue;
            }
            p.on_ground(
                commands,
                piece,
                x,
                z,
                rng.range(0.0, TAU),
                rng.range(1.3, 2.2),
                0.05,
                0.6,
                false,
                rich,
            );
        }
    }
    let celandine = p.lib.pieces("celandine_01").to_vec();
    for _ in 0..q.undergrowth / 10 {
        if let Some(piece) = pick(&mut rng, &celandine) {
            let (x, z) = bank_point(&mut rng, -0.1, 2.5);
            p.on_ground(
                commands,
                piece,
                x,
                z,
                rng.range(0.0, TAU),
                rng.range(1.6, 2.6),
                0.05,
                0.8,
                false,
                false,
            );
        }
    }
    let saplings = p.lib.pieces("fir_sapling").to_vec();
    for _ in 0..q.conifers / 2 {
        if let Some(piece) = pick(&mut rng, &saplings) {
            let (x, z) = bank_point(&mut rng, 2.0, 16.0);
            p.on_ground(
                commands,
                piece,
                x,
                z,
                rng.range(0.0, TAU),
                rng.range(1.5, 3.2),
                0.0,
                0.2,
                false,
                true,
            );
        }
    }

    info!("placed {} scanned objects", p.count);
}

/// A random point on either bank, `near`..`far` meters from the water,
/// denser close to the creek.
fn bank_point(rng: &mut Rng, near: f32, far: f32) -> (f32, f32) {
    let z = rng.range(creek::Z_MIN + 2.0, creek::Z_MAX - 2.0);
    let side = if rng.f() < 0.5 { -1.0 } else { 1.0 };
    let d = near + (far - near) * rng.f().powf(1.6);
    (center(z) + side * (half_width(z) + d), z)
}
