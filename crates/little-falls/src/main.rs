//! Little Falls: a forest waterfall benchmark for Linux gaming systems.
//!
//! Order of a full run:
//! 1. CPU and memory tests (console, before the window opens so the
//!    renderer does not compete for the CPU).
//! 2. Graphics test: a timed flythrough of the waterfall scene.
//! 3. Scores, a results JSON file, and the results screen.

mod bench;
mod camera;
mod config;
mod geom;
mod noise;
mod scene;
mod textures;

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, WindowMode, WindowResolution};
use config::Config;
use lf_core::results::{self, RunInfo};
use lf_core::score::FrameStats;
use lf_core::sysinfo::SystemInfo;
use lf_core::{cpu, memory, Measurement};
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
    let report = results::build(&run, sys, ms, frames);
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

fn main() {
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

    let cpu_results = if config.run_cpu {
        println!("\nRunning CPU and memory tests...");
        run_cpu_tests(config.quick)
    } else {
        Vec::new()
    };

    if !config.run_graphics {
        let (report, line) = save_report(&config, &sys, &cpu_results, None, (0, 0));
        println!("\nCPU:      {:>6.0}", report.scores.cpu);
        println!("Memory:   {:>6.0}", report.scores.memory);
        println!("Overall:  {:>6.0}", report.scores.overall);
        println!("{line}");
        return;
    }

    println!("\nStarting graphics test ({} preset)...", config.preset.name());
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

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(window),
        ..default()
    }))
    .insert_resource(config.clone())
    .insert_resource(bench::CpuResults(cpu_results))
    .insert_resource(bench::System(sys))
    .add_plugins((scene::ScenePlugin, bench::BenchPlugin))
    .add_systems(Startup, camera::spawn_camera);
    if config.explore {
        app.add_systems(Update, camera::explore_camera);
    }
    app.run();
}
