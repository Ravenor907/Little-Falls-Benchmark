//! Rocks and fallen logs.

use super::creek::{self, bed, center, ground, half_width};
use crate::geom::{self, MeshData};
use crate::noise::{fbm3, Rng};
use bevy::prelude::*;

/// A lumpy boulder with moss on its upper surfaces.
fn rock_mesh(seed: u32, detail: u32) -> MeshData {
    let mut m = geom::icosphere(detail);
    let s = seed as f32 * 7.31;
    for p in m.pos.iter_mut() {
        let v = Vec3::from(*p);
        let big = fbm3(v.x * 1.1 + s, v.y * 1.1, v.z * 1.1, 3, seed) * 0.35;
        let small = fbm3(v.x * 4.0, v.y * 4.0 + s, v.z * 4.0, 2, seed + 7) * 0.08;
        let mut q = v * (1.0 + big + small);
        // Flatten the base so rocks sit in the ground, and facet the top
        // a little like fractured granite.
        q.y = q.y.max(-0.35);
        if q.y > 0.5 {
            q.y = 0.5 + (q.y - 0.5) * 0.7;
        }
        *p = q.to_array();
    }
    m.recompute_normals();
    for i in 0..m.len() {
        let p = Vec3::from(m.pos[i]);
        let n = Vec3::from(m.nrm[i]);
        let moss_mask = fbm3(p.x * 2.0, p.y * 2.0, p.z * 2.0 + s, 3, seed + 3) * 0.5 + 0.5;
        let moss = ((n.y - 0.35) * 2.5).clamp(0.0, 1.0) * ((moss_mask - 0.35) * 3.0).clamp(0.0, 1.0);
        let grain = fbm3(p.x * 9.0, p.y * 9.0, p.z * 9.0, 2, seed + 5) * 0.03;
        let rock = [0.075 + grain, 0.078 + grain, 0.072 + grain];
        let green = [0.16, 0.28, 0.05];
        m.col[i] = [
            rock[0] * (1.0 - moss) + green[0] * moss,
            rock[1] * (1.0 - moss) + green[1] * moss,
            rock[2] * (1.0 - moss) + green[2] * moss,
            1.0,
        ];
    }
    m
}

fn log_mesh(len: f32, radius: f32, seed: u32) -> MeshData {
    let mut m = geom::cylinder(radius, radius * 0.85, len, 14, (len * 2.0) as u32 + 1);
    for i in 0..m.len() {
        let p = Vec3::from(m.pos[i]);
        let n = Vec3::from(m.nrm[i]);
        // Ridged bark.
        let ridge = ((p.x.atan2(p.z) * 18.0).sin() * 0.5 + 0.5) * 0.04 * radius;
        let q = p + n * ridge;
        m.pos[i] = q.to_array();
        let moss = (fbm3(p.x * 1.5, p.y * 0.8, p.z * 1.5, 3, seed) * 0.5 + 0.5 - 0.4).clamp(0.0, 0.6) * 1.6;
        let bark = [0.24, 0.20, 0.15];
        let green = [0.18, 0.30, 0.06];
        m.col[i] = [
            bark[0] * (1.0 - moss) + green[0] * moss,
            bark[1] * (1.0 - moss) + green[1] * moss,
            bark[2] * (1.0 - moss) + green[2] * moss,
            1.0,
        ];
    }
    m
}

pub fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    detail_high: bool,
) {
    let mut rng = Rng::new(31);
    let rock_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        // Wet rock is fairly glossy.
        perceptual_roughness: 0.38,
        reflectance: 0.45,
        ..default()
    });
    let variants: Vec<Handle<Mesh>> = (0..8)
        .map(|i| meshes.add(rock_mesh(i + 1, if detail_high { 3 } else { 2 }).build()))
        .collect();

    let place_rock = |commands: &mut Commands, rng: &mut Rng, pos: Vec3, size: Vec3| {
        let v = variants[(rng.u64() % variants.len() as u64) as usize].clone();
        commands.spawn((
            Mesh3d(v),
            MeshMaterial3d(rock_mat.clone()),
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_y(rng.range(0.0, std::f32::consts::TAU)))
                .with_scale(size),
        ));
    };

    // Boulders along both banks for the whole creek.
    let mut z = creek::Z_MIN;
    while z < creek::Z_MAX {
        z += rng.range(0.6, 2.2);
        for side in [-1.0f32, 1.0] {
            let x = center(z) + side * (half_width(z) + rng.range(-0.3, 1.2));
            let s = rng.range(0.4, 1.4);
            let pos = Vec3::new(x, ground(x, z) + 0.05 * s, z);
            place_rock(
                commands,
                &mut rng,
                pos,
                Vec3::new(s * rng.range(0.8, 1.5), s * rng.range(0.5, 0.9), s),
            );
        }
        // Occasional stones in the stream bed, breaking the surface.
        if rng.f() < 0.35 {
            let x = center(z) + rng.range(-1.0, 1.0) * half_width(z) * 0.7;
            let s = rng.range(0.25, 0.6);
            let pos = Vec3::new(x, bed(z) - 0.1, z);
            place_rock(commands, &mut rng, pos, Vec3::new(s, s * 0.6, s * 1.2));
        }
    }

    // Big dark boulders framing every ledge of the falls, plus a center
    // rock that splits the flow into two chutes.
    for (k, p) in creek::plunge_points().iter().enumerate() {
        let z_lip = p.z - 2.2;
        for side in [-1.0f32, 1.0] {
            let x = center(z_lip) + side * (half_width(z_lip) + 0.3);
            let s = rng.range(1.6, 2.4);
            place_rock(
                commands,
                &mut rng,
                Vec3::new(x, bed(z_lip) - 0.4, z_lip),
                Vec3::new(s, s * 1.1, s * 1.3),
            );
        }
        if k != 1 {
            let s = rng.range(0.7, 1.0);
            place_rock(
                commands,
                &mut rng,
                Vec3::new(p.x + 0.4, bed(p.z - 0.8), p.z - 0.8),
                Vec3::new(s * 0.9, s * 1.3, s),
            );
        }
        // Rubble in each plunge pool.
        for _ in 0..5 {
            let s = rng.range(0.25, 0.55);
            let pos = *p + Vec3::new(rng.range(-1.6, 1.6), -0.25, rng.range(0.3, 2.5));
            place_rock(commands, &mut rng, pos, Vec3::splat(s));
        }
    }

    // Logs. The logjam leans across the top of the falls like in the
    // reference photos; one mossy trunk slants down into the cascade.
    let top = creek::FALLS_TOP;
    let logs: Vec<(Vec3, Vec3, f32)> = vec![
        (
            Vec3::new(center(top) - 5.0, bed(top) + 2.4, top - 1.0),
            Vec3::new(center(top) + 6.5, bed(top) + 1.0, top + 2.2),
            0.42,
        ),
        (
            Vec3::new(center(top) - 4.0, bed(top) + 1.4, top + 1.8),
            Vec3::new(center(top) + 5.0, bed(top) + 2.0, top - 0.4),
            0.32,
        ),
        (
            Vec3::new(center(top + 1.0) + 1.2, bed(top + 1.0) + 1.6, top + 1.0),
            Vec3::new(center(top + 7.0) + 0.6, bed(top + 7.0) + 0.2, top + 7.0),
            0.3,
        ),
        // Fallen trunks along the banks.
        (
            Vec3::new(9.0, ground(9.0, -30.0) + 0.3, -30.0),
            Vec3::new(18.0, ground(18.0, -36.0) + 0.3, -36.0),
            0.5,
        ),
        (
            Vec3::new(-7.0, ground(-7.0, 16.0) + 0.3, 16.0),
            Vec3::new(-15.0, ground(-15.0, 10.0) + 0.4, 10.0),
            0.4,
        ),
        (
            Vec3::new(5.5, ground(5.5, 26.0) + 0.3, 26.0),
            Vec3::new(13.0, ground(13.0, 31.0) + 0.3, 31.0),
            0.35,
        ),
    ];
    let log_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        ..default()
    });
    for (i, (a, b, r)) in logs.into_iter().enumerate() {
        let len = a.distance(b);
        let mesh = meshes.add(log_mesh(len, r, 40 + i as u32).build());
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(log_mat.clone()),
            Transform::from_matrix(geom::segment(a, b)),
        ));
    }
}
