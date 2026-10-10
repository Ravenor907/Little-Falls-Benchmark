//! Runs the graphics test: warm-up, the timed flythrough, then scoring,
//! saving the results file and showing the results screen.

use crate::camera::{flythrough, MainCamera};
use crate::config::Config;
use crate::scene::water::LongExposure;
use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::PrimaryWindow;
use lf_core::score::{self, FrameStats};
use lf_core::sysinfo::SystemInfo;
use lf_core::Measurement;

/// Points along the flythrough where `--screenshots` captures a frame.
const SHOTS: [f32; 5] = [0.0, 0.2, 0.42, 0.62, 0.85];

#[derive(Resource)]
pub struct CpuResults(pub Vec<Measurement>);

#[derive(Resource)]
pub struct System(pub SystemInfo);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Warmup,
    Running,
    Done,
}

#[derive(Resource)]
pub struct Bench {
    phase: Phase,
    phase_start: f64,
    frame_times: Vec<f64>,
    fps_smooth: f64,
    shots_taken: usize,
}

#[derive(Component)]
struct Hud;

#[derive(Component)]
struct ResultsPanel;

pub struct BenchPlugin;

impl Plugin for BenchPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Bench {
            phase: Phase::Warmup,
            phase_start: 0.0,
            frame_times: Vec::with_capacity(20_000),
            fps_smooth: 0.0,
            shots_taken: 0,
        })
        .add_systems(Startup, spawn_hud)
        .add_systems(Update, (run_bench, update_hud, quit_keys));
    }
}

fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        Text::new("Little Falls"),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(14.0),
            left: Val::Px(16.0),
            ..default()
        },
        Hud,
    ));
}

#[allow(clippy::too_many_arguments)]
fn run_bench(
    mut commands: Commands,
    time: Res<Time<Real>>,
    config: Res<Config>,
    mut bench: ResMut<Bench>,
    mut long: ResMut<LongExposure>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
    cpu: Option<Res<CpuResults>>,
    sys: Option<Res<System>>,
    adapter: Option<Res<RenderAdapterInfo>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut exit: EventWriter<AppExit>,
) {
    if config.explore {
        return;
    }
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs_f64();
    if dt > 0.0 {
        bench.fps_smooth = bench.fps_smooth * 0.95 + (1.0 / dt) * 0.05;
    }

    let progress = match bench.phase {
        Phase::Warmup => {
            // Count warm-up from the first rendered frame, not app start,
            // since building the scene takes a moment.
            if bench.phase_start == 0.0 {
                bench.phase_start = now.max(1e-6);
            }
            if now - bench.phase_start >= config.warmup {
                bench.phase = Phase::Running;
                bench.phase_start = now;
            }
            0.0
        }
        Phase::Running => {
            bench.frame_times.push(dt);
            let p = ((now - bench.phase_start) / config.duration as f64) as f32;
            // The final quarter shows the long exposure water.
            long.0 = p > 0.75;
            if p >= 1.0 {
                bench.phase = Phase::Done;
                finish(
                    &mut commands,
                    &config,
                    &bench,
                    cpu,
                    sys,
                    adapter,
                    &windows,
                    &mut exit,
                );
            }
            p.min(1.0)
        }
        Phase::Done => return,
    };

    if let Ok(mut tf) = cam.single_mut() {
        let (pos, look) = flythrough(progress);
        *tf = Transform::from_translation(pos).looking_at(look, Vec3::Y);
    }

    if let Some(dir) = &config.screenshot_dir {
        let i = bench.shots_taken;
        if bench.phase == Phase::Running && i < SHOTS.len() && progress >= SHOTS[i] {
            let _ = std::fs::create_dir_all(dir);
            let path = dir.join(format!("little-falls-{}.png", i + 1));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
            bench.shots_taken += 1;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn finish(
    commands: &mut Commands,
    config: &Config,
    bench: &Bench,
    cpu: Option<Res<CpuResults>>,
    sys: Option<Res<System>>,
    adapter: Option<Res<RenderAdapterInfo>>,
    windows: &Query<&Window, With<PrimaryWindow>>,
    exit: &mut EventWriter<AppExit>,
) {
    let stats = score::frame_stats(&bench.frame_times);
    let mut ms: Vec<Measurement> = cpu.map(|c| c.0.clone()).unwrap_or_default();
    ms.extend(score::graphics_measurements(&stats));

    let mut info = sys.map(|s| s.0.clone()).unwrap_or_else(SystemInfo::detect);
    if let Some(a) = adapter {
        info.gpu_name = a.name.clone();
        info.gpu_driver = format!("{} {}", a.driver, a.driver_info).trim().to_string();
        info.gpu_backend = format!("{:?}", a.backend);
    }
    let resolution = windows
        .single()
        .map(|w| (w.physical_width(), w.physical_height()))
        .unwrap_or((0, 0));

    let (report, saved) = crate::save_report(config, &info, &ms, Some(&stats), resolution);
    print_summary(&report.scores, &stats, &saved);

    if config.exit_on_finish {
        exit.write(AppExit::Success);
        return;
    }
    show_results(
        commands,
        config,
        &info,
        &report.scores,
        &stats,
        resolution,
        &saved,
    );
}

fn print_summary(s: &score::Scores, f: &FrameStats, saved: &str) {
    println!();
    println!(
        "Graphics: {:>6.0}  ({:.1} fps average, {:.1} fps 1% low)",
        s.graphics, f.avg_fps, f.low1_fps
    );
    if s.cpu > 0.0 {
        println!("CPU:      {:>6.0}", s.cpu);
        println!("Memory:   {:>6.0}", s.memory);
    }
    println!("Overall:  {:>6.0}", s.overall);
    println!("{saved}");
}

fn show_results(
    commands: &mut Commands,
    config: &Config,
    info: &SystemInfo,
    s: &score::Scores,
    f: &FrameStats,
    res: (u32, u32),
    saved: &str,
) {
    let line = |label: &str, v: f64| {
        if v > 0.0 {
            format!("{label:<10}{v:>7.0}\n")
        } else {
            format!("{label:<10}{:>7}\n", "skipped")
        }
    };
    let body = format!(
        "{}{}{}\n{:.1} fps average   {:.1} fps 1% low\n{} preset at {}x{}\n\n{}\n{}\n\n{}\n\nPress Esc or B to exit",
        line("Graphics", s.graphics),
        line("CPU", s.cpu),
        line("Memory", s.memory),
        f.avg_fps,
        f.low1_fps,
        config.preset.name(),
        res.0,
        res.1,
        info.gpu_name,
        info.cpu_model,
        saved,
    );
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ResultsPanel,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(32.0)),
                    row_gap: Val::Px(12.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.04, 0.03, 0.78)),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("LITTLE FALLS"),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.65, 0.85, 0.55)),
                ));
                panel.spawn((
                    Text::new(format!("{:.0}", s.overall)),
                    TextFont {
                        font_size: 72.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                panel.spawn((
                    Text::new(body),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
                ));
            });
        });
}

fn update_hud(
    config: Res<Config>,
    bench: Res<Bench>,
    time: Res<Time<Real>>,
    long: Res<crate::scene::water::LongExposure>,
    mut hud: Query<&mut Text, With<Hud>>,
) {
    let Ok(mut text) = hud.single_mut() else { return };
    let now = time.elapsed_secs_f64();
    let status = if config.explore {
        "Explore mode: WASD, Q/E, right mouse to look".to_string()
    } else {
        match bench.phase {
            Phase::Warmup => format!(
                "Warming up shaders... {:.0}",
                (config.warmup - (now - bench.phase_start)).max(0.0).ceil()
            ),
            Phase::Running => {
                let left = (config.duration as f64 - (now - bench.phase_start)).max(0.0);
                format!("Benchmark running   {:.0}s left", left.ceil())
            }
            Phase::Done => String::new(),
        }
    };
    let exposure = if long.0 { "\nLong exposure water" } else { "" };
    text.0 = if bench.phase == Phase::Done {
        String::new()
    } else {
        format!(
            "LITTLE FALLS   {} preset\n{:.0} fps\n{}{}",
            config.preset.name(),
            bench.fps_smooth,
            status,
            exposure
        )
    };
}

fn quit_keys(keys: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>, mut exit: EventWriter<AppExit>) {
    let pad = pads
        .iter()
        .any(|g| g.just_pressed(GamepadButton::East) || g.just_pressed(GamepadButton::Select));
    if keys.just_pressed(KeyCode::Escape) || pad {
        exit.write(AppExit::Success);
    }
}
