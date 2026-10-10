//! The Little Falls scene: a creek dropping over mossy ledges in a
//! Northern California ravine, lit by late morning sun through the
//! canopy.

pub mod creek;
pub mod flora;
pub mod materials;
pub mod props;
pub mod scanned;
pub mod water;
pub mod wildlife;

use crate::assets::{Environment, Library};
use crate::camera::MainCamera;
use crate::config::Config;
use crate::textures::Textures;
use bevy::core_pipeline::Skybox;
use bevy::pbr::environment_map::EnvironmentMapLight;
use bevy::pbr::{
    CascadeShadowConfigBuilder, DirectionalLightShadowMap, ExtendedMaterial, FogVolume, NotShadowCaster,
    VolumetricLight,
};
use bevy::prelude::*;
use materials::{TerrainExt, TerrainMaterial, TerrainParams, WaterMaterial};

/// Marks every entity that belongs to the scene, so the scene can be
/// hidden during CPU tests and rebuilt when the quality preset changes.
#[derive(Component)]
pub struct SceneObject;

/// Whether the 3D scene is drawn. Hidden while the CPU tests run.
#[derive(Resource)]
pub struct SceneVisible(pub bool);

/// Request a (re)build of the scene, for example once assets are loaded.
#[derive(Event)]
pub struct RebuildScene;

/// Present once the scene has been built at least once.
#[derive(Resource)]
pub struct SceneBuilt;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<water::LongExposure>()
            .add_plugins(materials::MaterialsPlugin)
            .add_event::<RebuildScene>()
            .insert_resource(SceneVisible(true))
            // Shown only until the sky is ready.
            .insert_resource(ClearColor(Color::srgb(0.70, 0.80, 0.90)))
            .add_systems(
                Update,
                (
                    rebuild,
                    apply_environment,
                    apply_visibility,
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

fn apply_visibility(visible: Res<SceneVisible>, mut q: Query<&mut Visibility, With<SceneObject>>) {
    if !visible.is_changed() {
        return;
    }
    let v = if visible.0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut vis in &mut q {
        *vis = v;
    }
}

#[allow(clippy::too_many_arguments)]
fn rebuild(
    mut commands: Commands,
    mut events: EventReader<RebuildScene>,
    config: Res<Config>,
    built: Option<Res<SceneBuilt>>,
    lib: Option<Res<Library>>,
    objects: Query<Entity, With<SceneObject>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut terrain_materials: ResMut<Assets<TerrainMaterial>>,
    mut water_materials: ResMut<Assets<WaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let requested = events.read().count() > 0;
    let preset_changed = built.is_some() && config.is_changed() && !config.is_added();
    if !requested && !preset_changed {
        return;
    }
    for e in &objects {
        commands.entity(e).despawn();
    }
    build(
        &mut commands,
        &config,
        lib.as_deref(),
        &mut meshes,
        &mut materials,
        &mut terrain_materials,
        &mut water_materials,
        &mut images,
    );
    commands.insert_resource(SceneBuilt);
}

/// Puts the HDRI sky and environment lighting on the camera once ready.
fn apply_environment(
    mut commands: Commands,
    env: Option<Res<Environment>>,
    cam: Query<Entity, With<MainCamera>>,
    ambient: Option<ResMut<AmbientLight>>,
) {
    let Some(env) = env else { return };
    if !env.is_added() {
        return;
    }
    for e in &cam {
        commands.entity(e).insert((
            Skybox {
                image: env.skybox.clone(),
                brightness: 1800.0,
                rotation: Quat::IDENTITY,
            },
            EnvironmentMapLight {
                diffuse_map: env.diffuse.clone(),
                specular_map: env.specular.clone(),
                intensity: 1300.0,
                rotation: Quat::IDENTITY,
                affects_lightmapped_mesh_diffuse: true,
            },
        ));
    }
    // The environment map now provides the sky light.
    if let Some(mut ambient) = ambient {
        ambient.brightness = 40.0;
    }
}

#[allow(clippy::too_many_arguments)]
fn build(
    commands: &mut Commands,
    config: &Config,
    lib: Option<&Library>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    terrain_materials: &mut Assets<TerrainMaterial>,
    water_materials: &mut Assets<WaterMaterial>,
    images: &mut Assets<Image>,
) {
    let q = config.quality;
    let textures = Textures::create(images);

    // Terrain: scanned surfaces blended by the terrain shader when
    // available, procedural vertex colors otherwise.
    let (terrain_data, weights) = creek::terrain(q.terrain_cell);
    let surfaces = lib.and_then(|l| {
        Some((
            l.surfaces.get("forest_leaves_02")?,
            l.surfaces.get("brown_mud_leaves_01")?,
            l.surfaces.get("dark_rock")?,
        ))
    });
    if let Some((moss, mud, rock)) = surfaces {
        let mut mesh = terrain_data.build();
        mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, weights);
        let _ = mesh.generate_tangents();
        let mat = terrain_materials.add(ExtendedMaterial {
            base: StandardMaterial {
                base_color_texture: Some(moss.albedo.clone()),
                normal_map_texture: Some(moss.normal.clone()),
                perceptual_roughness: 0.92,
                reflectance: 0.3,
                ..default()
            },
            extension: TerrainExt {
                params: TerrainParams {
                    mud_scale: 0.8,
                    rock_scale: 0.6,
                    macro_strength: 0.22,
                    wet_roughness: 0.35,
                },
                mud: mud.albedo.clone(),
                rock: rock.albedo.clone(),
            },
        });
        commands.spawn((
            SceneObject,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(mat),
            Transform::IDENTITY,
        ));
    } else {
        let ground_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(textures.ground.clone()),
            perceptual_roughness: 0.85,
            ..default()
        });
        commands.spawn((
            SceneObject,
            Mesh3d(meshes.add(terrain_data.build())),
            MeshMaterial3d(ground_mat),
            Transform::IDENTITY,
        ));
    }

    match lib {
        Some(l) => scanned::spawn(commands, l, materials, &q),
        None => props::spawn(commands, meshes, materials, &textures, q.terrain_cell <= 0.5),
    }
    flora::spawn(commands, meshes, materials, &textures, &q, lib);
    water::spawn(commands, meshes, materials, water_materials, &textures, &q);
    wildlife::spawn(commands, meshes, materials, &q);

    // Sun.
    commands.insert_resource(DirectionalLightShadowMap { size: q.shadow_map });
    let mut sun = commands.spawn((
        SceneObject,
        DirectionalLight {
            color: Color::srgb(1.0, 0.95, 0.85),
            illuminance: 15_000.0,
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

    // Soft sky light filling the shadows with a cool tint. With scanned
    // assets the HDRI environment map does most of this job.
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.75, 0.85, 1.0),
        brightness: if lib.is_some() { 40.0 } else { 220.0 },
        ..default()
    });

    // A volume of thin, humid air over the whole ravine. Forward
    // scattering makes the sun shafts bright when looking toward the sun.
    if q.volumetric_steps > 0 {
        commands.spawn((
            SceneObject,
            FogVolume {
                fog_color: Color::srgb(0.95, 0.97, 1.0),
                density_factor: 0.012,
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
