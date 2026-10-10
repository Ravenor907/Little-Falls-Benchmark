//! The graphics test: shader warm-up, then the timed flythrough. Frame
//! times are recorded every frame; when the run ends the app moves on to
//! the results screen.

use crate::camera::{flythrough, MainCamera};
use crate::config::Config;
use crate::scene::water::LongExposure;
use crate::state::{AppState, RunData};
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::PrimaryWindow;
use lf_core::score;

/// Points along the flythrough where `--screenshots` captures a frame.
const SHOTS: [f32; 5] = [0.0, 0.2, 0.42, 0.62, 0.85];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Warmup,
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
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Bench>()
        .add_systems(OnEnter(AppState::Graphics), enter)
        .add_systems(Update, run.run_if(in_state(AppState::Graphics)));
}

fn enter(mut bench: ResMut<Bench>, mut long: ResMut<LongExposure>) {
    *bench = Bench::default();
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
            // Timed from the first frame in this state, so a slow scene
            // build does not eat into the warm-up.
            if bench.phase_start == 0.0 {
                bench.phase_start = now.max(1e-6);
            }
            if now - bench.phase_start >= config.warmup {
                bench.phase = Phase::Running;
                bench.phase_start = now;
            }
            bench.progress = 0.0;
        }
        Phase::Running => {
            bench.frame_times.push(dt);
            let p = ((now - bench.phase_start) / config.duration as f64) as f32;
            bench.progress = p.min(1.0);
            // The final quarter shows the long exposure water.
            long.0 = p > 0.75;
            if p >= 1.0 {
                data.frames = Some(score::frame_stats(&bench.frame_times));
                data.resolution = windows
                    .single()
                    .map(|w| (w.physical_width(), w.physical_height()))
                    .unwrap_or((0, 0));
                next.set(AppState::Results);
            }
        }
    }

    if let Ok(mut tf) = cam.single_mut() {
        let (pos, look) = flythrough(bench.progress);
        *tf = Transform::from_translation(pos).looking_at(look, Vec3::Y);
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
