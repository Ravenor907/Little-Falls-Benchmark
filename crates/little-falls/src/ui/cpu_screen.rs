//! CPU and memory test screen. The tests run on a background thread
//! while the scene is hidden and the app redraws only a few times a
//! second, so rendering barely touches the CPU being measured.

use super::theme::*;
use crate::config::Config;
use crate::scene::SceneVisible;
use crate::state::{despawn_screen, AppState, RunData, RunPlan, Screen};
use bevy::prelude::*;
use bevy::winit::{UpdateMode, WinitSettings};
use lf_core::{cpu, memory, Measurement};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::CpuTest), (enter, spawn_ui))
        .add_systems(OnExit(AppState::CpuTest), (exit, despawn_screen))
        .add_systems(Update, poll.run_if(in_state(AppState::CpuTest)));
}

#[derive(Default)]
struct Shared {
    label: String,
    fraction: f32,
    results: Option<Vec<Measurement>>,
}

#[derive(Resource)]
struct CpuJob {
    shared: Arc<Mutex<Shared>>,
    cancel: Arc<AtomicBool>,
}

/// Unwind payload used to stop the worker when the user cancels.
struct Cancelled;

#[derive(Component)]
struct BarFill;
#[derive(Component)]
struct StepLabel;
#[derive(Component)]
struct Percent;

fn enter(mut commands: Commands, config: Res<Config>, mut visible: ResMut<SceneVisible>) {
    visible.0 = false;
    commands.insert_resource(WinitSettings {
        focused_mode: UpdateMode::reactive(Duration::from_millis(100)),
        unfocused_mode: UpdateMode::reactive_low_power(Duration::from_millis(250)),
    });

    let shared = Arc::new(Mutex::new(Shared {
        label: "Preparing".into(),
        ..default()
    }));
    let cancel = Arc::new(AtomicBool::new(false));
    let quick = config.quick;
    {
        let shared = shared.clone();
        let cancel = cancel.clone();
        std::thread::Builder::new()
            .name("little-falls-cpu".into())
            .spawn(move || {
                // Let the UI settle before measuring.
                std::thread::sleep(Duration::from_millis(800));
                let report = |label: &str, f: f32, span: (f32, f32)| {
                    if cancel.load(Ordering::Relaxed) {
                        std::panic::resume_unwind(Box::new(Cancelled));
                    }
                    if let Ok(mut s) = shared.lock() {
                        s.label = label.to_string();
                        s.fraction = span.0 + (span.1 - span.0) * f;
                    }
                };
                let size = if quick { cpu::Size::Quick } else { cpu::Size::Full };
                let mut ms = cpu::run_all(size, &mut |l: &str, f: f32| report(l, f, (0.0, 0.8)));
                ms.extend(memory::run_all(quick, &mut |l: &str, f: f32| {
                    report(l, f, (0.8, 1.0))
                }));
                if let Ok(mut s) = shared.lock() {
                    s.fraction = 1.0;
                    s.results = Some(ms);
                }
            })
            .expect("spawn CPU test thread");
    }
    commands.insert_resource(CpuJob { shared, cancel });
}

fn exit(mut commands: Commands, job: Option<Res<CpuJob>>, mut visible: ResMut<SceneVisible>) {
    if let Some(job) = job {
        job.cancel.store(true, Ordering::Relaxed);
    }
    commands.remove_resource::<CpuJob>();
    commands.insert_resource(WinitSettings::game());
    visible.0 = true;
}

fn spawn_ui(mut commands: Commands, fonts: Res<Fonts>, plan: Option<Res<RunPlan>>) {
    let f = &*fonts;
    let two_step = plan.map(|p| p.graphics).unwrap_or(false);
    commands
        .spawn((
            root(JustifyContent::Center, AlignItems::Center),
            BackgroundColor(BACKDROP),
            Screen,
        ))
        .with_children(|r| {
            r.spawn(panel(Val::Px(520.0))).with_children(|p| {
                p.spawn(text(
                    if two_step { "STEP 1 OF 2" } else { "CPU AND MEMORY" },
                    &f.semibold,
                    12.0,
                    ACCENT,
                ));
                p.spawn(text("Testing CPU and memory", &f.light, 34.0, TEXT));

                p.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    margin: UiRect::vertical(Val::Px(8.0)),
                    ..default()
                })
                .with_children(|b| {
                    b.spawn(Node {
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((text("Preparing", &f.regular, 15.0, TEXT_DIM), StepLabel));
                        row.spawn((text("0%", &f.semibold, 15.0, TEXT), Percent));
                    });
                    // Progress track and fill.
                    b.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(6.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                        BorderRadius::all(Val::Px(3.0)),
                    ))
                    .with_children(|t| {
                        t.spawn((
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(ACCENT),
                            BorderRadius::all(Val::Px(3.0)),
                            BarFill,
                        ));
                    });
                });

                p.spawn(text(
                    "Graphics are paused during this step so they do not affect the result. \
                     Close other apps for the most accurate score.",
                    &f.regular,
                    13.0,
                    TEXT_DIM,
                ));
                p.spawn(text("Esc or B to cancel", &f.regular, 12.0, TEXT_FAINT));
            });
        });
}

#[allow(clippy::type_complexity)]
fn poll(
    job: Option<Res<CpuJob>>,
    plan: Option<Res<RunPlan>>,
    mut data: ResMut<RunData>,
    mut next: ResMut<NextState<AppState>>,
    mut fill: Query<&mut Node, With<BarFill>>,
    mut texts: ParamSet<(Query<&mut Text, With<StepLabel>>, Query<&mut Text, With<Percent>>)>,
) {
    let Some(job) = job else { return };
    let Ok(mut s) = job.shared.lock() else { return };
    for mut n in &mut fill {
        n.width = Val::Percent(s.fraction * 100.0);
    }
    for mut t in &mut texts.p0() {
        if t.0 != s.label {
            t.0 = s.label.clone();
        }
    }
    for mut t in &mut texts.p1() {
        t.0 = format!("{:.0}%", s.fraction * 100.0);
    }
    if let Some(results) = s.results.take() {
        // More than 15% between repeats means something else was using
        // the CPU, or it throttled.
        let worst = results.iter().map(|m| m.spread).fold(0.0, f64::max);
        if worst > 0.15 {
            data.warnings.push(format!(
                "CPU results varied by up to {:.0}% between repeats. Close other apps \
                 and make sure the system is not throttling for a steadier score.",
                worst * 100.0
            ));
        }
        data.cpu = results;
        let graphics = plan.map(|p| p.graphics).unwrap_or(false);
        next.set(if graphics {
            AppState::Graphics
        } else {
            AppState::Results
        });
    }
}
