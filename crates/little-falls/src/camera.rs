//! Camera setup, the scripted flythrough path and a free-fly explore
//! camera.

use crate::config::Config;
use crate::scene::creek::{bed, center};
use bevy::core_pipeline::bloom::Bloom;
use bevy::core_pipeline::motion_blur::MotionBlur;
use bevy::core_pipeline::smaa::Smaa;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::pbr::{
    DistanceFog, FogFalloff, ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel,
    VolumetricFog,
};
use bevy::prelude::*;

#[derive(Component)]
pub struct MainCamera;

pub fn spawn_camera(mut commands: Commands, config: Res<Config>) {
    let q = config.quality;
    let (pos, look) = flythrough(0.0);
    let mut cam = commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 60f32.to_radians(),
            ..default()
        }),
        Transform::from_translation(pos).looking_at(look, Vec3::Y),
        Tonemapping::TonyMcMapface,
        Bloom::NATURAL,
        // SSAO needs MSAA off; SMAA handles edges instead.
        Msaa::Off,
        Smaa::default(),
        // Blue-green distance haze of a humid forest.
        DistanceFog {
            color: Color::srgba(0.62, 0.72, 0.74, 1.0),
            directional_light_color: Color::srgba(1.0, 0.95, 0.8, 0.5),
            directional_light_exponent: 20.0,
            falloff: FogFalloff::Exponential { density: 0.012 },
        },
        MainCamera,
    ));
    if q.ssao > 0 {
        cam.insert(ScreenSpaceAmbientOcclusion {
            quality_level: match q.ssao {
                1 => ScreenSpaceAmbientOcclusionQualityLevel::Medium,
                2 => ScreenSpaceAmbientOcclusionQualityLevel::High,
                _ => ScreenSpaceAmbientOcclusionQualityLevel::Ultra,
            },
            ..default()
        });
    }
    if q.volumetric_steps > 0 {
        cam.insert(VolumetricFog {
            ambient_intensity: 0.0,
            step_count: q.volumetric_steps,
            ..default()
        });
    }
    if q.motion_blur {
        cam.insert(MotionBlur {
            shutter_angle: 0.5,
            samples: 4,
            ..default()
        });
    }
}

/// Key frames: (camera position, look target). The path opens on the
/// view of the reference photos, looking up the cascade from the plunge
/// pool, then climbs the falls, flies upstream under the canopy, rises
/// into the light shafts and returns to the pool.
fn keys() -> Vec<(Vec3, Vec3)> {
    let at = |x: f32, dy: f32, z: f32| Vec3::new(center(z) + x, bed(z) + dy, z);
    vec![
        (at(1.5, 1.6, 14.0), at(0.0, 2.0, -8.0)),
        (at(-1.5, 2.0, 9.0), at(0.0, 3.0, -10.0)),
        (at(3.0, 2.5, 2.0), at(-0.5, 2.0, -9.0)),
        (at(2.5, 2.0, -6.0), at(0.0, 1.0, -14.0)),
        (at(-1.0, 2.5, -13.0), at(0.0, 1.0, -26.0)),
        (at(0.5, 2.2, -24.0), at(0.0, 3.0, -40.0)),
        (at(-2.0, 6.0, -36.0), at(2.0, 10.0, -55.0)),
        (at(1.0, 12.0, -42.0), at(0.0, 4.0, -20.0)),
        (at(4.0, 10.0, -20.0), at(0.0, 0.0, -6.0)),
        (at(5.0, 8.0, -4.0), at(0.0, 0.0, 4.0)),
        (at(2.5, 3.0, 8.0), at(0.0, 2.0, -6.0)),
        (at(1.5, 1.6, 14.0), at(0.0, 2.0, -8.0)),
    ]
}

fn catmull(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

/// Camera position and look target for `t` in 0..=1.
pub fn flythrough(t: f32) -> (Vec3, Vec3) {
    let k = keys();
    let segs = (k.len() - 1) as f32;
    // Ease in and out so the run starts and ends gently.
    let e = t.clamp(0.0, 1.0);
    let e = e * e * (3.0 - 2.0 * e) * 0.3 + e * 0.7;
    let f = e * segs;
    let i = (f.floor() as usize).min(k.len() - 2);
    let u = f - i as f32;
    let g = |j: isize| k[(i as isize + j).clamp(0, k.len() as isize - 1) as usize];
    let pos = catmull(g(-1).0, g(0).0, g(1).0, g(2).0, u);
    let look = catmull(g(-1).1, g(0).1, g(1).1, g(2).1, u);
    (pos, look)
}

/// Free-fly camera for `--explore`: WASD to move, Q/E down/up, hold the
/// right mouse button to look around, Shift to go faster.
pub fn explore_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
) {
    let Ok(mut tf) = cam.single_mut() else { return };
    let dt = time.delta_secs();
    if mouse.pressed(MouseButton::Right) {
        let d = motion.delta * 0.003;
        let (yaw, pitch, _) = tf.rotation.to_euler(EulerRot::YXZ);
        tf.rotation = Quat::from_euler(EulerRot::YXZ, yaw - d.x, (pitch - d.y).clamp(-1.5, 1.5), 0.0);
    }
    let mut dir = Vec3::ZERO;
    let fwd = *tf.forward();
    let right = *tf.right();
    for (k, v) in [
        (KeyCode::KeyW, fwd),
        (KeyCode::KeyS, -fwd),
        (KeyCode::KeyD, right),
        (KeyCode::KeyA, -right),
        (KeyCode::KeyE, Vec3::Y),
        (KeyCode::KeyQ, -Vec3::Y),
    ] {
        if keys.pressed(k) {
            dir += v;
        }
    }
    let speed = if keys.pressed(KeyCode::ShiftLeft) {
        15.0
    } else {
        4.0
    };
    tf.translation += dir.normalize_or_zero() * speed * dt;
}
