//! Scanned art assets: finding them on disk, loading them during the
//! loading screen, giving every texture a proper mipmap chain, and turning
//! the HDRI into sky and environment lighting.
//!
//! If the asset folder is missing (for example a plain `cargo run` without
//! running `tools/fetch_assets.py`), the app falls back to the fully
//! procedural scene, so it always runs.

use crate::scene::materials::{windy, WindMaterial, WindParams};
use bevy::asset::LoadState;
use bevy::gltf::{Gltf, GltfMesh};
use bevy::image::{
    ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor,
};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
    TextureViewDimension,
};
use lf_image::ibl;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const MODELS: [&str; 12] = [
    "rock_moss_set_01",
    "rock_moss_set_02",
    "rock_09",
    "rock_face_02",
    "fern_02",
    "celandine_01",
    "tree_stump_01",
    "dead_tree_trunk_02",
    "pine_roots",
    "dry_branches_medium_01",
    "bark_debris_01",
    "fir_sapling",
];
pub const SURFACES: [&str; 5] = [
    "forest_leaves_02",
    "brown_mud_leaves_01",
    "dark_rock",
    "bark_brown_02",
    "pine_bark",
];
pub const HDRI: &str = "mossy_forest";

/// Finds the asset folder: next to an installed binary (AppImage and
/// Flatpak put it in ../share/little-falls/assets), beside the binary
/// (tarball), or at the repository root during development.
pub fn find_root() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(p) = std::env::var_os("LITTLE_FALLS_ASSETS") {
        candidates.push(PathBuf::from(p));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("../share/little-falls/assets"));
            candidates.push(dir.join("assets"));
        }
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets"));
    candidates.push(PathBuf::from("assets"));
    candidates
        .into_iter()
        .find(|p| p.join("models").is_dir())
        .and_then(|p| p.canonicalize().ok())
}

#[derive(Resource, Clone)]
pub struct AssetRoot(pub Option<PathBuf>);

/// One textured surface (albedo + normal map).
#[derive(Clone)]
pub struct Surface {
    pub albedo: Handle<Image>,
    pub normal: Handle<Image>,
}

/// A separable piece of a scanned model (one glTF mesh), possibly with
/// several primitives (for example branches and twigs).
#[derive(Clone)]
pub struct Piece {
    pub prims: Vec<(Handle<Mesh>, Handle<StandardMaterial>)>,
    /// Swaying versions of the materials, for foliage (parallel to `prims`).
    pub wind: Option<Vec<Handle<WindMaterial>>>,
    pub min: Vec3,
    pub max: Vec3,
}

impl Piece {
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }
}

/// Everything the scene needs from disk, once loaded.
#[derive(Resource, Clone)]
pub struct Library {
    pub models: std::collections::HashMap<&'static str, Vec<Piece>>,
    pub surfaces: std::collections::HashMap<&'static str, Surface>,
}

impl Library {
    pub fn pieces(&self, id: &str) -> &[Piece] {
        self.models.get(id).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

/// Handles while loading.
#[derive(Resource)]
pub struct Loading {
    gltfs: Vec<(&'static str, Handle<Gltf>)>,
    surfaces: Vec<(&'static str, Surface)>,
    ibl: Arc<Mutex<Option<Result<IblData, String>>>>,
    pub ibl_done: bool,
    pub failed: Vec<String>,
}

/// Processed environment lighting, produced on a worker thread.
pub struct IblData {
    skybox: ibl::Cubemap,
    diffuse: ibl::Cubemap,
    specular: ibl::Cubemap,
    /// Direction of the brightest region after rotation (the sun).
    pub sun_dir: Vec3,
}

#[derive(Resource, Clone)]
pub struct Environment {
    pub skybox: Handle<Image>,
    pub diffuse: Handle<Image>,
    pub specular: Handle<Image>,
}

/// Where the sun should sit in the sky: high and upstream, behind the
/// falls as seen from the plunge pool, so leaves glow and light shafts
/// face the camera. The HDRI is rotated so its bright region agrees.
pub fn desired_sun_azimuth() -> f32 {
    let d = -crate::scene::sun_direction();
    d.x.atan2(-d.z)
}

pub fn start_loading(mut commands: Commands, root: Res<AssetRoot>, server: Res<AssetServer>) {
    let Some(dir) = root.0.clone() else {
        return;
    };
    let gltfs = MODELS
        .iter()
        .map(|id| (*id, server.load::<Gltf>(format!("models/{id}/{id}.gltf"))))
        .collect();
    let surfaces = SURFACES
        .iter()
        .map(|id| {
            let albedo = server.load(format!("textures/{id}/diff.jpg"));
            let normal = server.load_with_settings(
                format!("textures/{id}/nor_gl.jpg"),
                |s: &mut ImageLoaderSettings| {
                    s.is_srgb = false;
                },
            );
            (*id, Surface { albedo, normal })
        })
        .collect();

    let ibl = Arc::new(Mutex::new(None));
    {
        let ibl = ibl.clone();
        let path = dir.join(format!("hdri/{HDRI}.hdr"));
        std::thread::Builder::new()
            .name("little-falls-ibl".into())
            .spawn(move || {
                let r = build_ibl(&path);
                if let Ok(mut slot) = ibl.lock() {
                    *slot = Some(r);
                }
            })
            .expect("spawn IBL thread");
    }
    commands.insert_resource(Loading {
        gltfs,
        surfaces,
        ibl,
        ibl_done: false,
        failed: Vec::new(),
    });
}

fn build_ibl(path: &Path) -> Result<IblData, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (w, h, px) = ibl::decode_hdr(&bytes)?;
    // Rotate so the HDRI's bright region sits where our sun is.
    let probe = ibl::Equirect::new(w, h, px.clone(), 0.0);
    let b = ibl::brightest_direction(&probe);
    let yaw = b[0].atan2(-b[2]) - desired_sun_azimuth();
    let env = ibl::Equirect::new(w, h, px, yaw);
    let sun = ibl::brightest_direction(&env);
    Ok(IblData {
        skybox: ibl::skybox(&env, 1024),
        diffuse: ibl::diffuse(&ibl::sh9(&env), 32),
        specular: ibl::specular(&env, 256, 9, 64),
        sun_dir: Vec3::from(sun),
    })
}

fn cube_image(c: &ibl::Cubemap) -> Image {
    let size = c.size as u32;
    Image {
        data: Some(c.to_rgba16f()),
        texture_descriptor: TextureDescriptor {
            label: None,
            size: Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 6,
            },
            mip_level_count: c.mips as u32,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        },
        texture_view_descriptor: Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::Cube),
            ..default()
        }),
        sampler: ImageSampler::linear(),
        ..default()
    }
}

/// Loading progress in 0..1 and whether everything is done.
pub fn progress(
    loading: &mut Loading,
    server: &AssetServer,
    images: &mut Assets<Image>,
    commands: &mut Commands,
) -> (f32, bool) {
    let mut done = 0usize;
    let mut total = 0usize;
    for (id, h) in &loading.gltfs {
        total += 1;
        match server.load_state(h) {
            LoadState::Failed(e) => {
                if !loading.failed.iter().any(|f| f.starts_with(id)) {
                    loading.failed.push(format!("{id}: {e}"));
                }
                done += 1;
            }
            _ if server.is_loaded_with_dependencies(h) => done += 1,
            _ => {}
        }
    }
    for (_, s) in &loading.surfaces {
        for h in [&s.albedo, &s.normal] {
            total += 1;
            if server.is_loaded_with_dependencies(h) || matches!(server.load_state(h), LoadState::Failed(_)) {
                done += 1;
            }
        }
    }
    total += 3; // environment lighting counts for a few files' worth
    if !loading.ibl_done {
        let result = loading.ibl.lock().ok().and_then(|mut s| s.take());
        if let Some(r) = result {
            loading.ibl_done = true;
            match r {
                Ok(data) => {
                    commands.insert_resource(Environment {
                        skybox: images.add(cube_image(&data.skybox)),
                        diffuse: images.add(cube_image(&data.diffuse)),
                        specular: images.add(cube_image(&data.specular)),
                    });
                    info!("environment ready; HDRI sun direction {:?}", data.sun_dir);
                }
                Err(e) => loading.failed.push(format!("environment: {e}")),
            }
        }
    }
    if loading.ibl_done {
        done += 3;
    }
    (done as f32 / total as f32, done == total)
}

/// Builds the `Library` once all handles are loaded.
pub fn finish(
    loading: &Loading,
    gltfs: &Assets<Gltf>,
    gltf_meshes: &Assets<GltfMesh>,
    meshes: &Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    wind_materials: &mut Assets<WindMaterial>,
) -> Library {
    let mut models = std::collections::HashMap::new();
    for (id, h) in &loading.gltfs {
        let Some(gltf) = gltfs.get(h) else { continue };
        let mut pieces = Vec::new();
        for gm in &gltf.meshes {
            let Some(gm) = gltf_meshes.get(gm) else { continue };
            let mut prims = Vec::new();
            let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for p in &gm.primitives {
                let mat = p.material.clone().unwrap_or_default();
                if let Some(m) = materials.get_mut(&mat) {
                    tune_material(id, m);
                }
                if let Some(mesh) = meshes.get(&p.mesh) {
                    if let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(pos)) =
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    {
                        for v in pos {
                            min = min.min(Vec3::from(*v));
                            max = max.max(Vec3::from(*v));
                        }
                    }
                }
                prims.push((p.mesh.clone(), mat));
            }
            if min.x > max.x {
                // Mesh data not kept on the CPU: assume a unit-sized piece.
                warn!("{id}: no vertex data for bounds");
                min = Vec3::new(-0.5, 0.0, -0.5);
                max = Vec3::new(0.5, 1.0, 0.5);
            }
            let foliage = matches!(*id, "fern_02" | "celandine_01" | "fir_sapling");
            let wind = foliage.then(|| {
                prims
                    .iter()
                    .map(|(_, m)| {
                        let base = materials.get(m).cloned().unwrap_or_default();
                        wind_materials.add(windy(base, WindParams::PLANTS))
                    })
                    .collect()
            });
            if !prims.is_empty() {
                pieces.push(Piece {
                    prims,
                    wind,
                    min,
                    max,
                });
            }
        }
        models.insert(*id, pieces);
    }
    let surfaces = loading.surfaces.iter().map(|(id, s)| (*id, s.clone())).collect();
    Library { models, surfaces }
}

/// Per-asset material adjustments for the forest's light.
fn tune_material(id: &str, m: &mut StandardMaterial) {
    match id {
        "fern_02" | "celandine_01" | "fir_sapling" => {
            // Thin leaves let sunlight through.
            m.diffuse_transmission = 0.35;
            m.double_sided = true;
            m.cull_mode = None;
        }
        _ => {}
    }
}

/// Gives every loaded 8-bit texture a full mipmap chain and anisotropic,
/// repeating sampling. Scanned assets ship as plain JPEGs; without mips,
/// distant surfaces shimmer and alias badly in motion.
pub fn prepare_textures(
    mut events: EventReader<AssetEvent<Image>>,
    mut images: ResMut<Assets<Image>>,
    config: Res<crate::config::Config>,
) {
    let ids: Vec<AssetId<Image>> = events
        .read()
        .filter_map(|e| match e {
            AssetEvent::Added { id } => Some(*id),
            _ => None,
        })
        .collect();
    for id in ids {
        let Some(img) = images.get_mut(id) else { continue };
        let d = &img.texture_descriptor;
        let srgb = match d.format {
            TextureFormat::Rgba8UnormSrgb => true,
            TextureFormat::Rgba8Unorm => false,
            _ => continue,
        };
        if d.mip_level_count > 1
            || d.dimension != TextureDimension::D2
            || d.size.depth_or_array_layers != 1
            || d.size.width < 64
        {
            continue;
        }
        let (w, h) = (d.size.width, d.size.height);
        let Some(data) = img.data.as_ref() else { continue };
        if data.len() != (w * h * 4) as usize {
            continue;
        }
        let (mut chain, mut levels) = lf_image::mips::generate_rgba8(data, w, h, srgb);
        let (mut w, mut h) = (w, h);
        // Low presets drop the largest levels to save video memory.
        for _ in 0..config.quality.texture_skip {
            if w <= 256 || levels <= 1 {
                break;
            }
            chain.drain(..(w * h * 4) as usize);
            w = (w / 2).max(1);
            h = (h / 2).max(1);
            levels -= 1;
        }
        img.data = Some(chain);
        img.texture_descriptor.size.width = w;
        img.texture_descriptor.size.height = h;
        img.texture_descriptor.mip_level_count = levels;
        img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            anisotropy_clamp: 16,
            ..default()
        });
    }
}
