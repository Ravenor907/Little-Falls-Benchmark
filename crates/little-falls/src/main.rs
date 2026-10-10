//! Little Falls: a forest waterfall benchmark for Linux gaming systems.
//!
//! A single self-contained executable: run it and pick a test from the
//! menu. A full run does the CPU and memory tests (with rendering paused),
//! then a timed flythrough of the waterfall scene, then shows the scores
//! and saves a results file.

// Mesh generation loops index several parallel vertex arrays, and Bevy
// systems naturally take many parameters.
#![allow(
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::type_complexity
)]

mod assets;
mod bench;
mod camera;
mod config;
mod geom;
mod noise;
mod scene;
mod state;
mod textures;
mod ui;
mod warmup;

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, WindowMode, WindowResolution};
use config::Config;
use lf_core::results::{self, RunInfo};
use lf_core::score::FrameStats;
use lf_core::sysinfo::SystemInfo;
use lf_core::{cpu, memory, Measurement};
use state::AppState;
use std::io::Write;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn unix_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Builds and saves the results file. Returns the report and a line
/// describing where it went.
pub fn save_report(
    config: &Config,
    sys: &SystemInfo,
    ms: &[Measurement],
    frames: Option<&FrameStats>,
    resolution: (u32, u32),
    warnings: &[String],
) -> (results::Report, String) {
    let now = unix_time();
    let run = RunInfo {
        app_version: VERSION,
        preset: if frames.is_some() {
            config.preset.name().into()
        } else {
            String::new()
        },
        resolution,
        unix_time: now,
        nickname: config.nickname.clone(),
    };
    let report = results::build(&run, sys, ms, frames, warnings);
    let dir = config.out_dir.clone().unwrap_or_else(results::default_dir);
    let line = match results::save(&report, &dir, now) {
        Ok(p) => format!("Results saved to {}", p.display()),
        Err(e) => format!("Could not save results: {e}"),
    };
    (report, line)
}

fn run_cpu_tests(quick: bool) -> Vec<Measurement> {
    let mut progress = |name: &str, f: f32| {
        print!("\r[{:>3.0}%] {:<48}", f * 100.0, name);
        let _ = std::io::stdout().flush();
    };
    let size = if quick { cpu::Size::Quick } else { cpu::Size::Full };
    let mut ms = cpu::run_all(size, &mut progress);
    ms.extend(memory::run_all(quick, &mut progress));
    println!();
    for m in &ms {
        println!(
            "  {:<32} {:>10.2} {:<16} {:>6.0} pts",
            m.name,
            m.value,
            m.unit,
            lf_core::score::points(m)
        );
    }
    ms
}

/// On a crash: keep the normal stderr output, save a crash log, and show
/// a desktop dialog when possible, since an app launched from a menu or
/// Steam has no visible terminal.
fn install_crash_handler() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        let msg = info.to_string();
        let dir = results::default_dir()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        let log = dir.join("crash.log");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            &log,
            format!(
                "Little Falls {VERSION} crashed\n\n{msg}\n\n{}\n",
                std::backtrace::Backtrace::force_capture()
            ),
        );
        let lower = msg.to_lowercase();
        let hint = if lower.contains("gpu") || lower.contains("adapter") || lower.contains("vulkan") {
            "\n\nNo usable Vulkan GPU driver was found. Install your distribution's Vulkan \
             driver (for AMD and Intel: mesa-vulkan-drivers) and try again."
        } else {
            ""
        };
        let text = format!(
            "Little Falls ran into a problem and had to close.{hint}\n\nDetails were saved to {}",
            log.display()
        );
        let shown = std::process::Command::new("kdialog")
            .args(["--title", "Little Falls", "--error", &text])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !shown {
            let _ = std::process::Command::new("zenity")
                .args([
                    "--error",
                    "--title=Little Falls",
                    "--no-wrap",
                    &format!("--text={text}"),
                ])
                .status();
        }
    }));
}

fn main() {
    install_crash_handler();
    let config = match Config::from_args() {
        Ok(c) => c,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("error: {e}\n");
            }
            print!("{}", config::HELP);
            std::process::exit(if e.is_empty() { 0 } else { 2 });
        }
    };

    let sys = SystemInfo::detect();
    println!("Little Falls {VERSION}");
    println!(
        "{} | {} threads | {} MiB | {}",
        sys.cpu_model, sys.cpu_threads, sys.ram_mib, sys.os_name
    );

    // Script mode: CPU and memory in the terminal, no window.
    if config.headless_cpu {
        println!("\nRunning CPU and memory tests...");
        let cpu_results = run_cpu_tests(config.quick);
        let (report, line) = save_report(&config, &sys, &cpu_results, None, (0, 0), &[]);
        println!("\nCPU:      {:>6.0}", report.scores.cpu);
        println!("Memory:   {:>6.0}", report.scores.memory);
        println!("Overall:  {:>6.0}", report.scores.overall);
        println!("{line}");
        return;
    }

    let window = Window {
        title: "Little Falls".into(),
        present_mode: PresentMode::AutoNoVsync,
        mode: if config.windowed.is_some() {
            WindowMode::Windowed
        } else {
            WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
        },
        resolution: match config.windowed {
            Some((w, h)) => WindowResolution::new(w as f32, h as f32),
            None => WindowResolution::default(),
        },
        ..default()
    };

    let asset_root = assets::find_root();
    match &asset_root {
        Some(p) => println!("Assets: {}", p.display()),
        None => println!("Assets: not found, using the procedural scene"),
    }
    let asset_plugin = AssetPlugin {
        file_path: asset_root
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "assets".into()),
        ..default()
    };

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(window),
                    ..default()
                })
                .set(asset_plugin),
        )
        .insert_resource(assets::AssetRoot(asset_root))
        .init_state::<AppState>()
        .insert_resource(config)
        .insert_resource(state::System(sys))
        .insert_resource(state::RunData::default())
        .add_plugins((
            scene::ScenePlugin,
            ui::UiPlugin,
            bench::plugin,
            warmup::WarmupPlugin,
        ))
        .add_systems(PreStartup, state::detect_gpu)
        .add_systems(
            Startup,
            (camera::spawn_camera, state::enter_loading, assets::start_loading),
        )
        .add_systems(
            Update,
            (
                assets::prepare_textures,
                camera::update_camera_quality,
                camera::ambient_camera.run_if(in_state(AppState::Menu)),
                camera::ambient_camera.run_if(in_state(AppState::Results)),
                camera::explore_camera.run_if(in_state(AppState::Explore)),
            ),
        )
        .run();
}
