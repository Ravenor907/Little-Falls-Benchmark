//! Main menu: a glass panel on the left, the live waterfall on the right.

use super::theme::*;
use super::{button, UiAction};
use crate::config::{Config, Preset};
use crate::state::{despawn_screen, AppState, RunPlan, Screen, System};
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Menu), spawn)
        .add_systems(OnExit(AppState::Menu), despawn_screen)
        .add_systems(
            Update,
            (update_preset_note, menu_screenshot).run_if(in_state(AppState::Menu)),
        );
}

#[derive(Component)]
struct PresetNote;

fn preset_note(p: Preset) -> &'static str {
    match p {
        Preset::Low => "Integrated graphics and older handhelds",
        Preset::Medium => "Handhelds and entry level GPUs",
        Preset::High => "Standard test. Use this for the scoreboard",
        Preset::Ultra => "High end GPUs. Maximum foliage and effects",
    }
}

fn spawn(mut commands: Commands, fonts: Res<Fonts>, sys: Res<System>, config: Res<Config>) {
    let f = &*fonts;
    let s = &sys.0;
    commands
        .spawn((root(JustifyContent::FlexStart, AlignItems::Center), Screen))
        .with_children(|root| {
            root.spawn(panel(Val::Px(440.0))).with_children(|p| {
                // Title block.
                p.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|t| {
                    t.spawn(text("BENCHMARK", &f.semibold, 12.0, ACCENT));
                    t.spawn(text("Little Falls", &f.light, 46.0, TEXT));
                    t.spawn(text("Graphics, CPU and memory", &f.regular, 15.0, TEXT_DIM));
                });

                // This machine.
                p.spawn(card()).with_children(|c| {
                    spec_row(c, f, "GPU", &or_unknown(&s.gpu_name));
                    spec_row(
                        c,
                        f,
                        "CPU",
                        &format!("{}  ({} threads)", s.cpu_model, s.cpu_threads),
                    );
                    spec_row(c, f, "MEMORY", &format!("{:.1} GB", s.ram_mib as f64 / 1024.0));
                    spec_row(c, f, "SYSTEM", &s.os_name);
                });

                // Quality preset.
                p.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|q| {
                    q.spawn(text("QUALITY", &f.semibold, 12.0, TEXT_FAINT));
                    q.spawn(Node {
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|row| {
                        for (i, (label, preset)) in [
                            ("Low", Preset::Low),
                            ("Medium", Preset::Medium),
                            ("High", Preset::High),
                            ("Ultra", Preset::Ultra),
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            button(row, f, label, i as u32, UiAction::Preset(preset), false);
                        }
                    });
                    q.spawn((
                        text(preset_note(config.preset), &f.regular, 13.0, TEXT_DIM),
                        PresetNote,
                    ));
                });

                // Actions.
                p.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    margin: UiRect::top(Val::Px(6.0)),
                    ..default()
                })
                .with_children(|a| {
                    a.spawn(Node::default()).with_children(|r| {
                        button(r, f, "Run benchmark", 10, UiAction::Run(RunPlan::FULL), true);
                    });
                    a.spawn(Node {
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|r| {
                        button(r, f, "Graphics only", 11, UiAction::Run(RunPlan::GRAPHICS), false);
                        button(r, f, "CPU and memory", 12, UiAction::Run(RunPlan::CPU), false);
                    });
                    a.spawn(Node {
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|r| {
                        button(r, f, "Explore scene", 13, UiAction::Explore, false);
                        button(r, f, "Quit", 14, UiAction::Quit, false);
                    });
                });

                // Footer.
                p.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        padding: UiRect::top(Val::Px(12.0)),
                        border: UiRect::top(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor(HAIRLINE),
                ))
                .with_children(|ft| {
                    ft.spawn(text(
                        "A full run takes about two minutes.",
                        &f.regular,
                        12.0,
                        TEXT_DIM,
                    ));
                    ft.spawn(text(
                        format!("Version {}", crate::VERSION),
                        &f.regular,
                        12.0,
                        TEXT_FAINT,
                    ));
                });
            });
        });
}

fn or_unknown(s: &str) -> String {
    if s.is_empty() {
        "Detecting...".into()
    } else {
        s.to_string()
    }
}

fn update_preset_note(config: Res<Config>, mut q: Query<&mut Text, With<PresetNote>>) {
    if !config.is_changed() {
        return;
    }
    for mut t in &mut q {
        t.0 = preset_note(config.preset).to_string();
    }
}

/// `--menu-screenshot`: let the menu settle, capture, quit.
fn menu_screenshot(
    mut commands: Commands,
    config: Res<Config>,
    time: Res<Time<Real>>,
    mut shown_at: Local<Option<f32>>,
    mut taken: Local<bool>,
    mut exit: EventWriter<AppExit>,
) {
    let Some(path) = &config.menu_screenshot else { return };
    let now = time.elapsed_secs();
    let t = now - *shown_at.get_or_insert(now);
    if !*taken && t > 6.0 {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(path.clone()));
        *taken = true;
    }
    if t > 10.0 {
        exit.write(AppExit::Success);
    }
}
