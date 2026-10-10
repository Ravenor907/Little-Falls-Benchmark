//! Custom materials: extensions of Bevy's standard PBR material with our
//! own fragment shaders (terrain blending and running water). The WGSL is
//! compiled into the executable.

use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType};
use std::sync::OnceLock;

static TERRAIN_SHADER: OnceLock<Handle<Shader>> = OnceLock::new();
static WATER_SHADER: OnceLock<Handle<Shader>> = OnceLock::new();

pub type TerrainMaterial = ExtendedMaterial<StandardMaterial, TerrainExt>;
pub type WaterMaterial = ExtendedMaterial<StandardMaterial, WaterExt>;

#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct TerrainParams {
    pub mud_scale: f32,
    pub rock_scale: f32,
    pub macro_strength: f32,
    pub wet_roughness: f32,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct TerrainExt {
    #[uniform(100)]
    pub params: TerrainParams,
    #[texture(101)]
    #[sampler(102)]
    pub mud: Handle<Image>,
    #[texture(103)]
    #[sampler(104)]
    pub rock: Handle<Image>,
}

impl MaterialExtension for TerrainExt {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(TERRAIN_SHADER.get().expect("terrain shader registered").clone())
    }
}

#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct WaterParams {
    pub long_exposure: f32,
    pub normal_strength: f32,
    pub flow_speed: f32,
    pub foam_strength: f32,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct WaterExt {
    #[uniform(100)]
    pub params: WaterParams,
    #[texture(101)]
    #[sampler(102)]
    pub foam: Handle<Image>,
}

impl MaterialExtension for WaterExt {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(WATER_SHADER.get().expect("water shader registered").clone())
    }
}

pub struct MaterialsPlugin;

impl Plugin for MaterialsPlugin {
    fn build(&self, app: &mut App) {
        let mut shaders = app.world_mut().resource_mut::<Assets<Shader>>();
        let terrain = shaders.add(Shader::from_wgsl(
            include_str!("terrain.wgsl"),
            "little_falls/terrain.wgsl",
        ));
        let water = shaders.add(Shader::from_wgsl(
            include_str!("water.wgsl"),
            "little_falls/water.wgsl",
        ));
        let _ = TERRAIN_SHADER.set(terrain);
        let _ = WATER_SHADER.set(water);
        app.add_plugins((
            MaterialPlugin::<TerrainMaterial>::default(),
            MaterialPlugin::<WaterMaterial>::default(),
        ));
    }
}
