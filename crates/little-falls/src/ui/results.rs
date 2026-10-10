//! Results: overall score, a tile per category with a score bar,
//! and what to do next.

use super::theme::*;
use super::{button, UiAction};
use crate::config::Config;
use crate::state::{despawn_screen, AppState, RunData, Screen, System};
use bevy::prelude::*;
use lf_core::score::{self, FrameStats, Scores};
use lf_core::Measurement;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Results), enter)
        .add_systems(OnExit(AppState::Results), despawn_screen)
        .add_systems(Update, capture_and_exit.run_if(in_state(AppState::Results)));
}

fn enter(
    mut commands: Commands,
    fonts: Res<Fonts>,
    config: Res<Config>,
    sys: Res<System>,
    data: Res<RunData>,
    mut exit: EventWriter<AppExit>,
) {
    let mut ms: Vec<Measurement> = data.cpu.clone();
    if let Some(f) = &data.frames {
        ms.extend(score::graphics_measurements(f));
    }
    let (report, saved) =
        crate::save_report(&config, &sys.0, &ms, data.frames.as_ref(), data.resolution, &data.warnings);
    print_summary(&report.scores, data.frames.as_ref(), &saved);
    if config.exit_on_finish && config.screenshot_dir.is_none() {
        exit.write(AppExit::Success);
        return;
    }
    commands.insert_resource(ResultsShown(0.0));
    spawn_ui(
        &mut commands,
        &fonts,
        &config,
        &sys,
        &data,
        &ms,
        &report.scores,
        &saved,
    );
}

fn print_summary(s: &Scores, f: Option<&FrameStats>, saved: &str) {
    println!();
    if let Some(f) = f {
        println!(
            "Graphics: {:>6.0}  ({:.1} fps average, {:.1} fps 1% low)",
            s.graphics, f.avg_fps, f.low1_fps
        );
    }
    if s.cpu > 0.0 {
        println!("CPU:      {:>6.0}", s.cpu);
        println!("Memory:   {:>6.0}", s.memory);
    }
    println!("Overall:  {:>6.0}", s.overall);
    println!("{saved}");
}

fn value(ms: &[Measurement], id: &str) -> Option<f64> {
    ms.iter().find(|m| m.id == id).map(|m| m.value)
}

#[allow(clippy::too_many_arguments)]
fn spawn_ui(
    commands: &mut Commands,
    f: &Fonts,
    config: &Config,
    sys: &System,
    data: &RunData,
    ms: &[Measurement],
    s: &Scores,
    saved: &str,
) {
    let gfx_detail = data
        .frames
        .map(|fr| format!("{:.0} fps avg · {:.0} fps 1% low", fr.avg_fps, fr.low1_fps));
    let cpu_detail = value(ms, "cpu.multi.nbody").map(|_| format!("{} threads", sys.0.cpu_threads));
    let mem_detail = match (value(ms, "mem.read"), value(ms, "mem.latency")) {
        (Some(bw), Some(lat)) => Some(format!("{bw:.0} GB/s · {lat:.0} ns")),
        _ => None,
    };

    commands
        .spawn((root(JustifyContent::Center, AlignItems::Center), Screen))
        .with_children(|r| {
            r.spawn(panel(Val::Px(640.0))).with_children(|p| {
                p.spawn(text("RESULTS", &f.semibold, 12.0, ACCENT));

                // Overall score.
                p.spawn(Node {
                    align_items: AlignItems::FlexEnd,
                    column_gap: Val::Px(14.0),
                    ..default()
                })
                .with_children(|o| {
                    o.spawn(text(format!("{:.0}", s.overall), &f.light, 84.0, TEXT));
                    o.spawn(Node {
                        padding: UiRect::bottom(Val::Px(16.0)),
                        ..default()
                    })
                    .with_children(|l| {
                        l.spawn(text("overall score", &f.regular, 16.0, TEXT_DIM));
                    });
                });

                // Category tiles.
                p.spawn(Node {
                    column_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|row| {
                    tile(row, f, "Graphics", s.graphics, gfx_detail);
                    tile(row, f, "CPU", s.cpu, cpu_detail);
                    tile(row, f, "Memory", s.memory, mem_detail);
                });

                // Anything that makes this result less trustworthy.
                if !data.warnings.is_empty() {
                    p.spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
                            border: UiRect::left(Val::Px(3.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.95, 0.70, 0.25, 0.08)),
                        BorderColor(WARN),
                        BorderRadius::all(Val::Px(RADIUS_SMALL)),
                    ))
                    .with_children(|w| {
                        for msg in &data.warnings {
                            w.spawn(text(msg.clone(), &f.regular, 13.0, Color::srgb(0.98, 0.85, 0.6)));
                        }
                    });
                }

                // System and run details.
                p.spawn(card()).with_children(|c| {
                    spec_row(c, f, "GPU", &sys.0.gpu_name);
                    spec_row(c, f, "CPU", &sys.0.cpu_model);
                    if data.frames.is_some() {
                        spec_row(
                            c,
                            f,
                            "TEST",
                            &format!(
                                "{} preset at {}x{}",
                                config.preset.name(),
                                data.resolution.0,
                                data.resolution.1
                            ),
                        );
                    }
                    spec_row(c, f, "SYSTEM", &sys.0.os_name);
                });

                p.spawn(text(saved, &f.regular, 12.0, TEXT_FAINT));

                p.spawn(Node {
                    column_gap: Val::Px(8.0),
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|b| {
                    button(b, f, "Run again", 0, UiAction::RunAgain, true);
                    button(b, f, "Main menu", 1, UiAction::Menu, false);
                    button(b, f, "Quit", 2, UiAction::Quit, false);
                });
            });
        });
}

/// A score tile with a bar that fills over 0 to 2000 points.
fn tile(parent: &mut ChildSpawnerCommands, f: &Fonts, label: &str, score: f64, detail: Option<String>) {
    let ran = score > 0.0;
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(CARD),
            BorderRadius::all(Val::Px(RADIUS_SMALL)),
        ))
        .with_children(|t| {
            t.spawn(text(label.to_uppercase(), &f.semibold, 11.0, TEXT_FAINT));
            t.spawn(text(
                if ran {
                    format!("{score:.0}")
                } else {
                    "Skipped".into()
                },
                &f.semibold,
                if ran { 32.0 } else { 20.0 },
                if ran { TEXT } else { TEXT_FAINT },
            ));
            t.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(4.0),
                    margin: UiRect::vertical(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                BorderRadius::all(Val::Px(2.0)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    Node {
                        width: Val::Percent((score / 20.0).clamp(0.0, 100.0) as f32),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(ACCENT),
                    BorderRadius::all(Val::Px(2.0)),
                ));
            });
            if let Some(d) = detail {
                t.spawn(text(d, &f.regular, 12.0, TEXT_DIM));
            }
        });
}

#[derive(Resource)]
struct ResultsShown(f32);

/// With `--exit-on-finish --screenshots`, capture the results screen
/// before quitting.
fn capture_and_exit(
    mut commands: Commands,
    config: Res<Config>,
    time: Res<Time<Real>>,
    shown: Option<ResMut<ResultsShown>>,
    mut exit: EventWriter<AppExit>,
) {
    let (Some(mut shown), Some(dir)) = (shown, &config.screenshot_dir) else {
        return;
    };
    if !config.exit_on_finish {
        return;
    }
    let before = shown.0;
    shown.0 += time.delta_secs();
    if before < 2.0 && shown.0 >= 2.0 {
        let _ = std::fs::create_dir_all(dir);
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(
                dir.join("results.png"),
            ));
    }
    if shown.0 >= 5.0 {
        exit.write(AppExit::Success);
    }
}
