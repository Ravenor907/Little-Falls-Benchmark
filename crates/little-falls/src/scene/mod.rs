//! The Little Falls scene: a creek dropping over mossy ledges in a
//! Northern California ravine, lit by late morning sun through the
//! canopy.

pub mod creek;
pub mod flora;
pub mod props;
pub mod water;
pub mod wildlife;

use crate::config::Config;
use crate::textures::Textures;
use bevy::pbr::{
    CascadeShadowConfigBuilder, DirectionalLightShadowMap, FogVolume, NotShadowCaster, VolumetricLight,
};
use bevy::prelude::*;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<water::LongExposure>()
            // Bright sky glimpsed through the trees.
            .insert_resource(ClearColor(Color::srgb(0.70, 0.80, 0.90)))
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    water::animate_water,
                    water::animate_spray,
                    water::animate_mist,
                    water::toggle_long_exposure,
                    wildlife::flap_wings,
                    wildlife::update_birds,
                    wildlife::update_butterflies,
                ),
            );
    }
}

/// Direction the sunlight travels: from high upstream, down toward the
/// viewer, so leaves glow and light shafts face the camera.
pub fn sun_direction() -> Vec3 {
    Vec3::new(0.35, -0.62, 0.70).normalize()
}

fn setup(
    mut commands: Commands,
    config: Res<Config>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let q = config.quality;
    let textures = Textures::create(&mut images);

    // Terrain.
    let ground_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(creek::terrain(q.terrain_cell).build())),
        MeshMaterial3d(ground_mat),
        Transform::IDENTITY,
    ));

    props::spawn(&mut commands, &mut meshes, &mut materials, q.terrain_cell <= 0.5);
    flora::spawn(&mut commands, &mut meshes, &mut materials, &textures, &q);
    water::spawn(&mut commands, &mut meshes, &mut materials, &textures, &q);
    wildlife::spawn(&mut commands, &mut meshes, &mut materials, &q);

    // Sun.
    commands.insert_resource(DirectionalLightShadowMap { size: q.shadow_map });
    let mut sun = commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.95, 0.85),
            illuminance: 12_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::IDENTITY.looking_to(sun_direction(), Vec3::Y),
        CascadeShadowConfigBuilder {
            num_cascades: q.shadow_cascades,
            first_cascade_far_bound: 10.0,
            maximum_distance: 140.0,
            ..default()
        }
        .build(),
    ));
    if q.volumetric_steps > 0 {
        sun.insert(VolumetricLight);
    }

    // Soft sky light filling the shadows with a cool tint.
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.75, 0.85, 1.0),
        brightness: 350.0,
        ..default()
    });

    // A volume of thin, humid air over the whole ravine. Forward
    // scattering makes the sun shafts bright when looking toward the sun.
    if q.volumetric_steps > 0 {
        commands.spawn((
            FogVolume {
                fog_color: Color::srgb(0.95, 0.97, 1.0),
                density_factor: 0.035,
                absorption: 0.25,
                scattering: 0.35,
                scattering_asymmetry: 0.75,
                ..default()
            },
            Transform::from_xyz(0.0, 18.0, (creek::Z_MIN + creek::Z_MAX) * 0.5).with_scale(Vec3::new(
                creek::X_EXTENT * 2.0,
                40.0,
                creek::Z_MAX - creek::Z_MIN,
            )),
            NotShadowCaster,
        ));
    }

    commands.insert_resource(textures);
}
