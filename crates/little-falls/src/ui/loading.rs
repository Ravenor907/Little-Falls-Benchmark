//! Loading overlay: shown at startup and before each graphics run while
//! the camera sweeps the scene behind it to compile every shader.

use super::theme::*;
use crate::camera::MainCamera;
use crate::config::Config;
use crate::scene::water::LongExposure;
use crate::state::{after_loading, despawn_screen, AppState, Screen};
use crate::warmup::{PipelineStatus, Sweep};
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

#[derive(Resource)]
struct StartupSweep(Sweep);

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

fn enter(mut commands: Commands, fonts: Res<Fonts>, config: Res<Config>) {
    spawn_overlay(&mut commands, &fonts, "Preparing shaders");
    commands.insert_resource(StartupSweep(Sweep::new(config.sweep_frames)));
}

#[allow(clippy::too_many_arguments)]
fn step(
    mut commands: Commands,
    config: Res<Config>,
    status: Res<PipelineStatus>,
    mut sweep: ResMut<StartupSweep>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
    mut long: ResMut<LongExposure>,
    mut fill: Query<&mut Node, With<LoadingFill>>,
    mut detail: Query<&mut Text, With<LoadingDetail>>,
    mut next: ResMut<NextState<AppState>>,
) {
    let done = sweep.0.step(&status, &mut cam, &mut long);
    for mut n in &mut fill {
        n.width = Val::Percent(sweep.0.progress() * 100.0);
    }
    let line = if status.waiting > 0 {
        format!("Compiling shaders   {} remaining", status.waiting)
    } else {
        "Preparing shaders".to_string()
    };
    for mut t in &mut detail {
        if t.0 != line {
            t.0 = line.clone();
        }
    }
    if done {
        long.0 = false;
        commands.remove_resource::<StartupSweep>();
        after_loading(&config, &mut commands, &mut next);
    }
}
