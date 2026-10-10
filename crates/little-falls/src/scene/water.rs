//! Running water: one surface drawn by the custom water shader (ripples,
//! refraction, whitewater), spray particles at every plunge pool, and
//! drifting mist. Long exposure mode switches the shader to the silky look
//! of slow shutter waterfall photos.

use super::creek;
use super::materials::{WaterExt, WaterMaterial, WaterParams};
use super::SceneObject;
use crate::config::Quality;
use crate::geom;
use crate::noise::Rng;
use crate::textures::Textures;
use bevy::pbr::ExtendedMaterial;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct LongExposure(pub bool);

#[derive(Resource)]
pub struct WaterMaterials {
    pub surface: Handle<WaterMaterial>,
    pub spray: Handle<StandardMaterial>,
    applied_long_exposure: Option<bool>,
}

#[derive(Component)]
pub struct Spray {
    vel: Vec3,
    life: f32,
    max_life: f32,
    size: f32,
}

#[derive(Component)]
pub struct Mist {
    anchor: Vec3,
    phase: f32,
}

#[derive(Resource)]
pub struct SprayRng(Rng);

pub fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    water_materials: &mut Assets<WaterMaterial>,
    textures: &Textures,
    q: &Quality,
) {
    let surface = water_materials.add(ExtendedMaterial {
        base: StandardMaterial {
            // Tea-green creek water seen through a clear surface.
            base_color: Color::srgb(0.05, 0.09, 0.08),
            perceptual_roughness: 0.035,
            reflectance: 0.5,
            specular_transmission: 0.85,
            ior: 1.33,
            thickness: 0.35,
            attenuation_color: Color::srgb(0.55, 0.75, 0.62),
            attenuation_distance: 0.9,
            double_sided: true,
            cull_mode: None,
            ..default()
        },
        extension: WaterExt {
            params: WaterParams {
                long_exposure: 0.0,
                normal_strength: 1.0,
                flow_speed: 1.6,
                foam_strength: 1.35,
            },
            foam: textures.streaks.clone(),
        },
    });
    commands.spawn((
        SceneObject,
        Mesh3d(meshes.add(creek::water_ribbon(0.02, true).build())),
        MeshMaterial3d(surface.clone()),
        Transform::IDENTITY,
        NotShadowCaster,
    ));

    // Spray droplets.
    let spray = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.6),
        alpha_mode: AlphaMode::Blend,
        emissive: LinearRgba::rgb(0.15, 0.15, 0.16),
        perceptual_roughness: 0.2,
        ..default()
    });
    let drop = meshes.add(geom::icosphere(1).build());
    let mut rng = Rng::new(5);
    for _ in 0..q.spray_particles {
        let max_life = rng.range(0.6, 1.4);
        commands.spawn((
            SceneObject,
            Mesh3d(drop.clone()),
            MeshMaterial3d(spray.clone()),
            // Start hidden with staggered timers so the spray builds up
            // smoothly instead of every droplet launching at once.
            Transform::from_xyz(0.0, -100.0, 0.0).with_scale(Vec3::ZERO),
            Spray {
                vel: Vec3::ZERO,
                life: rng.range(0.0, max_life),
                max_life,
                size: rng.range(0.015, 0.045),
            },
            NotShadowCaster,
        ));
    }
    commands.insert_resource(SprayRng(rng));

    // Mist puffs hanging over the plunge pools.
    let mist_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.9, 0.95, 1.0, 0.09),
        base_color_texture: Some(textures.mist.clone()),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let quad = meshes.add(geom::quad().build());
    let pools = creek::plunge_points();
    let mut rng = Rng::new(9);
    for i in 0..q.mist_puffs {
        let p = pools[i % pools.len()];
        let anchor = p + Vec3::new(rng.range(-2.0, 2.0), rng.range(0.3, 1.8), rng.range(-0.5, 2.5));
        commands.spawn((
            SceneObject,
            Mesh3d(quad.clone()),
            MeshMaterial3d(mist_mat.clone()),
            Transform::from_translation(anchor).with_scale(Vec3::splat(rng.range(1.2, 2.6))),
            Mist {
                anchor,
                phase: rng.range(0.0, std::f32::consts::TAU),
            },
            NotShadowCaster,
        ));
    }

    commands.insert_resource(WaterMaterials {
        surface,
        spray,
        applied_long_exposure: None,
    });
}

/// Applies the long exposure look when it is toggled. The flow itself is
/// animated in the shader from the global time, so nothing changes per
/// frame here.
pub fn animate_water(
    long: Res<LongExposure>,
    wm: Option<ResMut<WaterMaterials>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut water_materials: ResMut<Assets<WaterMaterial>>,
) {
    // The scene (and its water) may not be built yet.
    let Some(mut wm) = wm else { return };
    if wm.applied_long_exposure == Some(long.0) {
        return;
    }
    wm.applied_long_exposure = Some(long.0);
    if let Some(m) = water_materials.get_mut(&wm.surface) {
        m.extension.params.long_exposure = if long.0 { 1.0 } else { 0.0 };
    }
    if let Some(m) = materials.get_mut(&wm.spray) {
        // Droplets blur away in a long exposure.
        m.base_color = Color::srgba(1.0, 1.0, 1.0, if long.0 { 0.12 } else { 0.6 });
    }
}

pub fn animate_spray(
    time: Res<Time>,
    rng: Option<ResMut<SprayRng>>,
    mut q: Query<(&mut Transform, &mut Spray)>,
) {
    let Some(mut rng) = rng else { return };
    let dt = time.delta_secs().min(0.05);
    let pools = creek::plunge_points();
    for (mut tf, mut s) in &mut q {
        s.life -= dt;
        if s.life <= 0.0 {
            // Respawn at a random plunge point, bursting up and out.
            let p = pools[(rng.0.u64() % pools.len() as u64) as usize];
            tf.translation = p + Vec3::new(rng.0.range(-0.9, 0.9), 0.05, rng.0.range(-0.4, 0.4));
            s.vel = Vec3::new(
                rng.0.range(-1.2, 1.2),
                rng.0.range(1.0, 3.2),
                rng.0.range(0.2, 2.2),
            );
            s.life = s.max_life;
        } else if tf.scale == Vec3::ZERO {
            // Not launched yet (staggered start).
            continue;
        }
        s.vel.y -= 9.81 * dt;
        s.vel *= 1.0 - 0.6 * dt; // air drag
        tf.translation += s.vel * dt;
        let fade = (s.life / s.max_life).clamp(0.0, 1.0);
        tf.scale = Vec3::splat(s.size * (0.4 + 0.6 * fade));
    }
}

pub fn animate_mist(
    time: Res<Time>,
    cam: Query<&GlobalTransform, With<Camera3d>>,
    mut q: Query<(&mut Transform, &Mist)>,
) {
    let Ok(cam) = cam.single() else { return };
    let eye = cam.translation();
    let t = time.elapsed_secs();
    for (mut tf, m) in &mut q {
        tf.translation = m.anchor
            + Vec3::new(
                (t * 0.21 + m.phase).sin() * 0.6,
                (t * 0.13 + m.phase * 2.0).sin() * 0.3,
                (t * 0.17 + m.phase).cos() * 0.5,
            );
        let to_cam = (eye - tf.translation).normalize_or(Vec3::Z);
        tf.rotation = Quat::from_rotation_arc(Vec3::Z, to_cam) * Quat::from_rotation_z(m.phase + t * 0.05);
    }
}

pub fn toggle_long_exposure(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut long: ResMut<LongExposure>,
) {
    let pad = pads.iter().any(|g| g.just_pressed(GamepadButton::North));
    if keys.just_pressed(KeyCode::KeyL) || pad {
        long.0 = !long.0;
    }
}
