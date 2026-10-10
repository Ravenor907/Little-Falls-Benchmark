//! Small birds flocking through the canopy (boids, which also adds a
//! little CPU work each frame) and butterflies drifting in the sunbeams.

use super::creek::{self, ground};
use crate::config::Quality;
use crate::geom::{self, MeshData};
use crate::noise::{value3, Rng};
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

#[derive(Component)]
pub struct Bird {
    vel: Vec3,
}

#[derive(Component)]
pub struct Butterfly {
    vel: Vec3,
    seed: u32,
}

#[derive(Component)]
pub struct Wing {
    side: f32,
    rate: f32,
    amp: f32,
    phase: f32,
}

/// Flock bounds: the air above the creek, below the canopy tops.
const FLOCK_CENTER: Vec3 = Vec3::new(0.0, 13.0, -12.0);
const FLOCK_HALF: Vec3 = Vec3::new(12.0, 6.0, 32.0);

/// A wing hinged at x = 0 extending along +x, nose toward -z.
fn wing_mesh(span: f32, chord: f32, rounded: bool) -> MeshData {
    let mut m = MeshData::default();
    let n = Vec3::Y;
    let w = geom::WHITE;
    let root_f = m.vertex(Vec3::new(0.0, 0.0, -chord * 0.5), n, Vec2::new(0.0, 0.0), w);
    let root_b = m.vertex(Vec3::new(0.0, 0.0, chord * 0.5), n, Vec2::new(0.0, 1.0), w);
    if rounded {
        // Butterfly: broad forewing and hindwing lobes.
        let fore = m.vertex(Vec3::new(span, 0.0, -chord * 0.7), n, Vec2::new(1.0, 0.0), w);
        let mid = m.vertex(
            Vec3::new(span * 0.85, 0.0, chord * 0.1),
            n,
            Vec2::new(1.0, 0.5),
            w,
        );
        let hind = m.vertex(
            Vec3::new(span * 0.55, 0.0, chord * 0.9),
            n,
            Vec2::new(0.6, 1.0),
            w,
        );
        m.tri(root_f, fore, mid);
        m.tri(root_f, mid, root_b);
        m.tri(root_b, mid, hind);
    } else {
        // Bird: tapered, swept-back wing.
        let tip = m.vertex(Vec3::new(span, 0.0, chord * 0.4), n, Vec2::new(1.0, 0.5), w);
        m.tri(root_f, tip, root_b);
    }
    m
}

fn mirrored(m: &MeshData) -> MeshData {
    let mut out = m.clone();
    for p in out.pos.iter_mut() {
        p[0] = -p[0];
    }
    for t in out.idx.chunks_exact_mut(3) {
        t.swap(1, 2);
    }
    out
}

pub fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    q: &Quality,
) {
    let mut rng = Rng::new(123);

    // Birds: small brown songbirds.
    let body = meshes.add({
        let mut b = geom::icosphere(1);
        for p in b.pos.iter_mut() {
            *p = [p[0] * 0.045, p[1] * 0.04, p[2] * 0.09];
        }
        b.build()
    });
    let bw = wing_mesh(0.16, 0.08, false);
    let bird_wing_r = meshes.add(bw.clone().build());
    let bird_wing_l = meshes.add(mirrored(&bw).build());
    let bird_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.17, 0.12),
        perceptual_roughness: 0.8,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    for _ in 0..q.birds {
        let pos = FLOCK_CENTER
            + Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)) * FLOCK_HALF;
        let vel = Vec3::new(rng.range(-1.0, 1.0), 0.0, rng.range(-1.0, 1.0)).normalize_or(Vec3::Z) * 5.0;
        let phase = rng.range(0.0, 6.28);
        let rate = rng.range(11.0, 15.0);
        commands
            .spawn((
                Mesh3d(body.clone()),
                MeshMaterial3d(bird_mat.clone()),
                Transform::from_translation(pos),
                Bird { vel },
            ))
            .with_children(|p| {
                for (side, mesh) in [(1.0, &bird_wing_r), (-1.0, &bird_wing_l)] {
                    p.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(bird_mat.clone()),
                        Transform::from_xyz(side * 0.03, 0.01, 0.0),
                        Wing {
                            side,
                            rate,
                            amp: 0.9,
                            phase,
                        },
                    ));
                }
            });
    }

    // Butterflies: orange tortoiseshells and pale yellow swallowtails.
    let fw = wing_mesh(0.045, 0.04, true);
    let fly_wing_r = meshes.add(fw.clone().build());
    let fly_wing_l = meshes.add(mirrored(&fw).build());
    let colors = [
        Color::srgb(0.95, 0.45, 0.08),
        Color::srgb(0.98, 0.88, 0.35),
        Color::srgb(0.9, 0.9, 0.85),
    ];
    let fly_mats: Vec<_> = colors
        .iter()
        .map(|c| {
            materials.add(StandardMaterial {
                base_color: *c,
                perceptual_roughness: 0.7,
                double_sided: true,
                cull_mode: None,
                diffuse_transmission: 0.4,
                ..default()
            })
        })
        .collect();
    for i in 0..q.butterflies {
        let z = rng.range(-30.0, 30.0);
        let x = creek::center(z) + rng.range(-6.0, 6.0);
        let pos = Vec3::new(x, ground(x, z) + rng.range(0.6, 2.5), z);
        let mat = fly_mats[i % fly_mats.len()].clone();
        let phase = rng.range(0.0, 6.28);
        let rate = rng.range(8.0, 11.0);
        commands
            .spawn((
                Transform::from_translation(pos),
                Visibility::default(),
                Butterfly {
                    vel: Vec3::ZERO,
                    seed: i as u32 * 17 + 3,
                },
            ))
            .with_children(|p| {
                for (side, mesh) in [(1.0, &fly_wing_r), (-1.0, &fly_wing_l)] {
                    p.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(mat.clone()),
                        Transform::IDENTITY,
                        Wing {
                            side,
                            rate,
                            amp: 1.1,
                            phase,
                        },
                        NotShadowCaster,
                    ));
                }
            });
    }
}

pub fn flap_wings(time: Res<Time>, mut q: Query<(&mut Transform, &Wing)>) {
    let t = time.elapsed_secs();
    for (mut tf, w) in &mut q {
        let a = (t * w.rate + w.phase).sin() * w.amp * 0.5 + 0.1;
        tf.rotation = Quat::from_rotation_z(a * w.side);
    }
}

/// Classic boids: separation, alignment, cohesion, plus steering to
/// stay inside the ravine and above the ground.
pub fn update_birds(time: Res<Time>, mut q: Query<(&mut Transform, &mut Bird)>) {
    let dt = time.delta_secs().min(0.05);
    let snapshot: Vec<(Vec3, Vec3)> = q.iter().map(|(t, b)| (t.translation, b.vel)).collect();
    for (i, (mut tf, mut bird)) in q.iter_mut().enumerate() {
        let p = tf.translation;
        let (mut sep, mut ali, mut coh, mut n) = (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 0.0);
        for (j, (op, ov)) in snapshot.iter().enumerate() {
            if i == j {
                continue;
            }
            let d = *op - p;
            let dist = d.length();
            if dist < 4.0 {
                n += 1.0;
                ali += *ov;
                coh += *op;
                if dist < 0.8 {
                    sep -= d / (dist * dist + 0.01);
                }
            }
        }
        let mut steer = sep * 2.0;
        if n > 0.0 {
            steer += (ali / n - bird.vel) * 0.8 + (coh / n - p) * 0.5;
        }
        // Soft box bounds.
        let rel = (p - FLOCK_CENTER) / FLOCK_HALF;
        steer -= rel * rel.abs() * 6.0;
        let floor = ground(p.x, p.z) + 3.0;
        if p.y < floor {
            steer.y += (floor - p.y) * 4.0;
        }
        bird.vel += steer * dt;
        let speed = bird.vel.length().clamp(3.5, 7.5);
        bird.vel = bird.vel.normalize_or(Vec3::Z) * speed;
        tf.translation += bird.vel * dt;
        let v = bird.vel;
        tf.look_to(v, Vec3::Y);
    }
}

/// Butterflies wander along smooth noise fields and keep low over the
/// banks.
pub fn update_butterflies(time: Res<Time>, mut q: Query<(&mut Transform, &mut Butterfly)>) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    for (mut tf, mut b) in &mut q {
        let p = tf.translation;
        let s = b.seed;
        let wander = Vec3::new(
            value3(p.x * 0.3, t * 0.4, 0.0, s) * 2.0 - 1.0,
            value3(p.y * 0.3, t * 0.5, 1.0, s + 1) * 2.0 - 1.0,
            value3(p.z * 0.3, t * 0.4, 2.0, s + 2) * 2.0 - 1.0,
        );
        let h = p.y - ground(p.x, p.z);
        let mut steer = wander * 3.0;
        steer.y += (1.5 - h) * 1.5;
        // Stay near the creek.
        let dx = p.x - creek::center(p.z);
        steer.x -= dx.signum() * (dx.abs() - 7.0).max(0.0);
        steer.z -= (p.z / 32.0).powi(3) * 4.0;
        b.vel += steer * dt;
        b.vel = b.vel.clamp_length_max(1.4);
        tf.translation += b.vel * dt;
        // Erratic flutter on top of the smooth path.
        tf.translation.y += (t * 9.0 + s as f32).sin() * 0.004;
        let v = b.vel;
        if v.length_squared() > 1e-4 {
            tf.look_to(Vec3::new(v.x, 0.0, v.z).normalize_or(Vec3::Z), Vec3::Y);
        }
    }
}
