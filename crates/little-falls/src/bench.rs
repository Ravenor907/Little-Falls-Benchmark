//! The graphics test: a shader sweep (see `warmup`), a short settle at the
//! start of the path, then the timed flythrough. Frame times are recorded
//! every frame; when the run ends the app moves on to the results screen.

use crate::camera::{flythrough, MainCamera};
use crate::config::Config;
use crate::scene::water::LongExposure;
use crate::state::{AppState, RunData};
use crate::warmup::{PipelineStatus, Sweep};
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::PrimaryWindow;
use lf_core::score;

fn place_camera(cam: &mut Query<&mut Transform, With<MainCamera>>, progress: f32) {
    if let Ok(mut tf) = cam.single_mut() {
        let (pos, look) = flythrough(progress);
        *tf = Transform::from_translation(pos).looking_at(look, Vec3::Y);
    }
}

/// Points along the flythrough where `--screenshots` captures a frame.
const SHOTS: [f32; 5] = [0.0, 0.2, 0.42, 0.62, 0.85];

/// Seconds held still at the start of the path after the sweep, so the
/// first timed frames are not affected by the camera jump.
const SETTLE: f64 = 1.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    /// Sweeping the path while shaders compile (hidden by an overlay).
    Warmup,
    /// Holding at the start of the path.
    Settle,
    Running,
}

#[derive(Resource)]
pub struct Bench {
    pub phase: Phase,
    pub phase_start: f64,
    pub progress: f32,
    pub fps_smooth: f64,
    frame_times: Vec<f64>,
    shots_taken: usize,
    pub sweep: Sweep,
    /// Frames during the timed run in which a shader was still compiling.
    compile_frames: u32,
}

impl Default for Bench {
    fn default() -> Self {
        Self {
            phase: Phase::Warmup,
            phase_start: 0.0,
            progress: 0.0,
            fps_smooth: 0.0,
            frame_times: Vec::with_capacity(20_000),
            shots_taken: 0,
            sweep: Sweep::new(240),
            compile_frames: 0,
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Bench>()
        .add_systems(OnEnter(AppState::Graphics), enter)
        .add_systems(Update, run.run_if(in_state(AppState::Graphics)));
}

fn enter(mut bench: ResMut<Bench>, mut long: ResMut<LongExposure>, config: Res<Config>) {
    *bench = Bench::default();
    // Shaders were compiled at startup; this shorter sweep catches any
    // variants a preset change introduced.
    bench.sweep = Sweep::new((config.sweep_frames / 2).max(10));
    long.0 = false;
}

#[allow(clippy::too_many_arguments)]
fn run(
    mut commands: Commands,
    time: Res<Time<Real>>,
    config: Res<Config>,
    mut bench: ResMut<Bench>,
    mut long: ResMut<LongExposure>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut data: ResMut<RunData>,
    mut next: ResMut<NextState<AppState>>,
    status: Res<PipelineStatus>,
) {
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs_f64();
    if dt > 0.0 {
        bench.fps_smooth = if bench.fps_smooth == 0.0 {
            1.0 / dt
        } else {
            bench.fps_smooth * 0.95 + (1.0 / dt) * 0.05
        };
    }

    match bench.phase {
        Phase::Warmup => {
            if bench.phase_start == 0.0 {
                bench.phase_start = now.max(1e-6);
            }
            let mut sweep = bench.sweep;
            let swept = sweep.step(&status, &mut cam, &mut long);
            bench.sweep = sweep;
            bench.progress = 0.0;
            // Give up waiting on a compiler that never goes quiet, but say so.
            let timed_out = now - bench.phase_start > 180.0;
            if (swept && now - bench.phase_start >= config.warmup) || timed_out {
                if timed_out {
                    data.warnings
                        .push("Shaders were still compiling when the test started.".into());
                }
                long.0 = false;
                bench.phase = Phase::Settle;
                bench.phase_start = now;
            }
        }
        Phase::Settle => {
            bench.progress = 0.0;
            place_camera(&mut cam, 0.0);
            if now - bench.phase_start >= SETTLE {
                bench.phase = Phase::Running;
                bench.phase_start = now;
            }
        }
        Phase::Running => {
            bench.frame_times.push(dt);
            if status.waiting > 0 {
                bench.compile_frames += 1;
            }
            let p = ((now - bench.phase_start) / config.duration as f64) as f32;
            bench.progress = p.min(1.0);
            // The final quarter shows the long exposure water.
            long.0 = p > 0.75;
            place_camera(&mut cam, bench.progress);
            if p >= 1.0 {
                let stats = score::frame_stats(&bench.frame_times);
                if let Some(cap) = score::detect_frame_cap(&bench.frame_times) {
                    data.warnings.push(format!(
                        "Frame rate looks capped at {cap:.0} fps (vsync or a frame limiter). \
                         Turn off vsync or the gamescope frame limit for a valid score."
                    ));
                }
                if bench.compile_frames > 0 {
                    data.warnings.push(format!(
                        "Shaders compiled during {} frames of the test; stutter may lower the score.",
                        bench.compile_frames
                    ));
                }
                data.frames = Some(stats);
                data.resolution = windows
                    .single()
                    .map(|w| (w.physical_width(), w.physical_height()))
                    .unwrap_or((0, 0));
                next.set(AppState::Results);
            }
        }
    }

    if let Some(dir) = &config.screenshot_dir {
        let i = bench.shots_taken;
        if bench.phase == Phase::Running && i < SHOTS.len() && bench.progress >= SHOTS[i] {
            let _ = std::fs::create_dir_all(dir);
            let path = dir.join(format!("little-falls-{}.png", i + 1));
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
            bench.shots_taken += 1;
        }
    }
}
