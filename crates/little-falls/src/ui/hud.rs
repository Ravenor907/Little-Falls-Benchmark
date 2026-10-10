//! Minimal overlay during the graphics test and explore mode: live FPS,
//! a thin progress line and status, kept out of the way of the scene.

use super::loading::{LoadingFill, LoadingOverlay};
use super::theme::*;
use crate::bench::{Bench, Phase};
use crate::config::Config;
use crate::scene::water::LongExposure;
use crate::state::{despawn_screen, AppState, Screen};
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Graphics), spawn_graphics_hud)
        .add_systems(OnExit(AppState::Graphics), despawn_screen)
        .add_systems(OnEnter(AppState::Explore), spawn_explore_hud)
        .add_systems(OnExit(AppState::Explore), despawn_screen)
        .add_systems(Update, update.run_if(in_state(AppState::Graphics)));
}

#[derive(Component)]
struct Fps;
#[derive(Component)]
struct Status;
#[derive(Component)]
struct ProgressFill;

fn chip() -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
            row_gap: Val::Px(2.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.03, 0.05, 0.04, 0.55)),
        BorderRadius::all(Val::Px(RADIUS_SMALL)),
    )
}

fn spawn_graphics_hud(mut commands: Commands, fonts: Res<Fonts>, config: Res<Config>) {
    let f = &*fonts;
    // Covers the shader sweep; removed when the camera settles.
    super::loading::spawn_overlay(&mut commands, f, "Preparing benchmark");
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(24.0)),
                ..default()
            },
            Screen,
        ))
        .with_children(|r| {
            r.spawn(Node::default()).with_children(|top| {
                top.spawn(chip()).with_children(|c| {
                    c.spawn((text("-- fps", &f.semibold, 22.0, TEXT), Fps));
                    c.spawn(text(
                        format!("{} preset", capitalize(config.preset.name())),
                        &f.regular,
                        12.0,
                        TEXT_DIM,
                    ));
                });
            });
            r.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|bottom| {
                bottom.spawn((text("", &f.regular, 14.0, TEXT_DIM), Status));
                bottom
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(3.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.12)),
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
                            ProgressFill,
                        ));
                    });
            });
        });
}

fn spawn_explore_hud(mut commands: Commands, fonts: Res<Fonts>) {
    let f = &*fonts;
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::FlexEnd,
                padding: UiRect::all(Val::Px(24.0)),
                ..default()
            },
            Screen,
        ))
        .with_children(|r| {
            r.spawn(chip()).with_children(|c| {
                c.spawn(text("Explore", &f.semibold, 15.0, TEXT));
                c.spawn(text(
                    "WASD move · Q/E down/up · Shift faster · right mouse look · L long exposure · Esc menu",
                    &f.regular,
                    12.0,
                    TEXT_DIM,
                ));
            });
        });
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[allow(clippy::type_complexity)]
fn update(
    mut commands: Commands,
    bench: Res<Bench>,
    long: Res<LongExposure>,
    mut texts: ParamSet<(Query<&mut Text, With<Fps>>, Query<&mut Text, With<Status>>)>,
    mut fill: Query<&mut Node, (With<ProgressFill>, Without<LoadingFill>)>,
    mut overlay_fill: Query<&mut Node, With<LoadingFill>>,
    overlays: Query<Entity, With<LoadingOverlay>>,
) {
    if bench.phase == Phase::Warmup {
        for mut n in &mut overlay_fill {
            n.width = Val::Percent(bench.sweep.progress() * 100.0);
        }
    } else {
        for e in &overlays {
            commands.entity(e).despawn();
        }
    }
    for mut t in &mut texts.p0() {
        t.0 = format!("{:.0} fps", bench.fps_smooth);
    }
    let status = match bench.phase {
        Phase::Warmup | Phase::Settle => String::new(),
        Phase::Running if long.0 => "Long exposure".to_string(),
        Phase::Running => String::new(),
    };
    for mut t in &mut texts.p1() {
        if t.0 != status {
            t.0 = status.clone();
        }
    }
    for mut n in &mut fill {
        n.width = Val::Percent(bench.progress * 100.0);
    }
}
