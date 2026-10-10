//! Loading overlay: shown at startup and before each graphics run while
//! the camera sweeps the scene behind it to compile every shader.

use super::theme::*;
use crate::assets;
use crate::camera::MainCamera;
use crate::config::Config;
use crate::scene::materials::WindMaterial;
use crate::scene::water::LongExposure;
use crate::scene::{RebuildScene, SceneBuilt};
use crate::state::{after_loading, despawn_screen, AppState, Screen};
use crate::warmup::{PipelineStatus, Sweep};
use bevy::gltf::{Gltf, GltfMesh};
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Loading), enter)
        .add_systems(OnExit(AppState::Loading), despawn_screen)
        .add_systems(Update, step.run_if(in_state(AppState::Loading)));
}

#[derive(Component)]
pub struct LoadingOverlay;
#[derive(Component)]
pub struct LoadingFill;
#[derive(Component)]
pub struct LoadingDetail;

/// Startup proceeds in three steps behind the overlay.
#[derive(Resource)]
enum Startup {
    /// Reading scanned models, textures and the HDRI from disk.
    Assets { since: f32 },
    /// Asking for the scene build once the library resource exists
    /// (resources inserted this frame are only visible next frame).
    Request { procedural: bool },
    /// Waiting for the scene to be built from them.
    Building,
    /// Sweeping the camera so every shader compiles.
    Sweep(Sweep),
}

/// Full-screen overlay with the wordmark, a status line and a progress bar.
pub fn spawn_overlay(commands: &mut Commands, f: &Fonts, status: &str) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(BACKDROP),
            // Above every other screen.
            GlobalZIndex(10),
            LoadingOverlay,
            Screen,
        ))
        .with_children(|o| {
            o.spawn(text("Little Falls", &f.light, 54.0, TEXT));
            o.spawn((text(status, &f.regular, 15.0, TEXT_DIM), LoadingDetail));
            o.spawn((
                Node {
                    width: Val::Px(280.0),
                    height: Val::Px(3.0),
                    margin: UiRect::top(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.1)),
                BorderRadius::all(Val::Px(2.0)),
            ))
            .with_children(|t| {
                t.spawn((
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(ACCENT),
                    BorderRadius::all(Val::Px(2.0)),
                    LoadingFill,
                ));
            });
        })
        .id()
}

fn enter(mut commands: Commands, fonts: Res<Fonts>, time: Res<Time<Real>>) {
    spawn_overlay(&mut commands, &fonts, "Loading the forest");
    commands.insert_resource(Startup::Assets {
        since: time.elapsed_secs(),
    });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn step(
    mut commands: Commands,
    config: Res<Config>,
    time: Res<Time<Real>>,
    status: Res<PipelineStatus>,
    mut startup: ResMut<Startup>,
    loading: Option<ResMut<assets::Loading>>,
    server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    (gltfs, gltf_meshes, meshes): (Res<Assets<Gltf>>, Res<Assets<GltfMesh>>, Res<Assets<Mesh>>),
    (mut materials, mut wind_materials): (ResMut<Assets<StandardMaterial>>, ResMut<Assets<WindMaterial>>),
    (mut rebuild, built, lib): (
        EventWriter<RebuildScene>,
        Option<Res<SceneBuilt>>,
        Option<Res<assets::Library>>,
    ),
    mut cam: Query<&mut Transform, With<MainCamera>>,
    mut long: ResMut<LongExposure>,
    mut fill: Query<&mut Node, With<LoadingFill>>,
    mut detail: Query<&mut Text, With<LoadingDetail>>,
    mut next: ResMut<NextState<AppState>>,
) {
    let (progress, line, done) = match &mut *startup {
        Startup::Assets { since } => {
            let waited = time.elapsed_secs() - *since;
            match loading {
                Some(mut loading) => {
                    let (p, ready) = assets::progress(&mut loading, &server, &mut images, &mut commands);
                    // Never hang on a stuck file: build with what loaded.
                    if ready || waited > 120.0 {
                        for f in &loading.failed {
                            warn!("asset problem: {f}");
                        }
                        let lib = assets::finish(
                            &loading,
                            &gltfs,
                            &gltf_meshes,
                            &meshes,
                            &mut materials,
                            &mut wind_materials,
                        );
                        commands.insert_resource(lib);
                        *startup = Startup::Request { procedural: false };
                    }
                    (p * 0.5, format!("Loading the forest   {:.0}%", p * 100.0), false)
                }
                None => {
                    // No asset folder: procedural scene.
                    *startup = Startup::Request { procedural: true };
                    (0.5, "Building the forest".to_string(), false)
                }
            }
        }
        Startup::Request { procedural } => {
            if *procedural || lib.is_some() {
                rebuild.write(RebuildScene);
                *startup = Startup::Building;
            }
            (0.5, "Building the forest".to_string(), false)
        }
        Startup::Building => {
            if built.is_some() {
                *startup = Startup::Sweep(Sweep::new(config.sweep_frames));
            }
            (0.5, "Building the forest".to_string(), false)
        }
        Startup::Sweep(sweep) => {
            let done = sweep.step(&status, &mut cam, &mut long);
            let line = if status.waiting > 0 {
                format!("Compiling shaders   {} remaining", status.waiting)
            } else {
                "Preparing shaders".to_string()
            };
            (0.5 + 0.5 * sweep.progress(), line, done)
        }
    };
    for mut n in &mut fill {
        n.width = Val::Percent(progress * 100.0);
    }
    for mut t in &mut detail {
        if t.0 != line {
            t.0 = line.clone();
        }
    }
    if done {
        long.0 = false;
        commands.remove_resource::<Startup>();
        after_loading(&config, &mut commands, &mut next);
    }
}
