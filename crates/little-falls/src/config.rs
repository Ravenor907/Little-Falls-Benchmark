//! Command line options and quality presets.

use bevy::prelude::Resource;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Low,
    Medium,
    High,
    Ultra,
}

impl Preset {
    pub fn name(self) -> &'static str {
        match self {
            Preset::Low => "low",
            Preset::Medium => "medium",
            Preset::High => "high",
            Preset::Ultra => "ultra",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "low" => Preset::Low,
            "medium" | "med" => Preset::Medium,
            "high" => Preset::High,
            "ultra" => Preset::Ultra,
            _ => return None,
        })
    }

    pub fn quality(self) -> Quality {
        match self {
            Preset::Low => Quality {
                terrain_cell: 1.0,
                maples: 10,
                leaves_per_maple: 900,
                conifers: 50,
                undergrowth: 1500,
                spray_particles: 250,
                mist_puffs: 10,
                birds: 16,
                butterflies: 10,
                shadow_map: 1024,
                shadow_cascades: 2,
                ssao: 0,
                volumetric_steps: 0,
                motion_blur: false,
            },
            Preset::Medium => Quality {
                terrain_cell: 0.7,
                maples: 16,
                leaves_per_maple: 1600,
                conifers: 80,
                undergrowth: 3500,
                spray_particles: 600,
                mist_puffs: 18,
                birds: 24,
                butterflies: 16,
                shadow_map: 2048,
                shadow_cascades: 3,
                ssao: 1,
                volumetric_steps: 32,
                motion_blur: true,
            },
            Preset::High => Quality {
                terrain_cell: 0.5,
                maples: 22,
                leaves_per_maple: 2600,
                conifers: 120,
                undergrowth: 7000,
                spray_particles: 1200,
                mist_puffs: 28,
                birds: 32,
                butterflies: 24,
                shadow_map: 4096,
                shadow_cascades: 4,
                ssao: 2,
                volumetric_steps: 64,
                motion_blur: true,
            },
            Preset::Ultra => Quality {
                terrain_cell: 0.35,
                maples: 30,
                leaves_per_maple: 4200,
                conifers: 170,
                undergrowth: 14000,
                spray_particles: 2500,
                mist_puffs: 40,
                birds: 48,
                butterflies: 32,
                shadow_map: 4096,
                shadow_cascades: 4,
                ssao: 3,
                volumetric_steps: 128,
                motion_blur: true,
            },
        }
    }
}

/// Scene detail knobs driven by the preset.
#[derive(Clone, Copy, Debug)]
pub struct Quality {
    pub terrain_cell: f32,
    pub maples: usize,
    pub leaves_per_maple: usize,
    pub conifers: usize,
    pub undergrowth: usize,
    pub spray_particles: usize,
    pub mist_puffs: usize,
    pub birds: usize,
    pub butterflies: usize,
    pub shadow_map: usize,
    pub shadow_cascades: usize,
    /// 0 = off, 1 = medium, 2 = high, 3 = ultra.
    pub ssao: u8,
    /// 0 disables volumetric light shafts.
    pub volumetric_steps: u32,
    pub motion_blur: bool,
}

#[derive(Resource, Clone, Debug)]
pub struct Config {
    pub preset: Preset,
    pub quality: Quality,
    /// Seconds of recorded flythrough.
    pub duration: f32,
    pub windowed: Option<(u32, u32)>,
    /// Start this run immediately instead of showing the menu.
    pub auto_run: Option<crate::state::RunPlan>,
    /// Console-only CPU and memory run with no window (for scripts).
    pub headless_cpu: bool,
    pub quick: bool,
    pub exit_on_finish: bool,
    /// Free camera, no timing. For exploring the scene.
    pub explore: bool,
    pub nickname: String,
    pub out_dir: Option<std::path::PathBuf>,
    /// Seconds to hold still before timing (shader warm-up).
    pub warmup: f64,
    /// Save screenshots at fixed points of the flythrough (for CI and
    /// visual regression checks).
    pub screenshot_dir: Option<std::path::PathBuf>,
    /// Capture the menu after shaders settle, then quit (CI).
    pub menu_screenshot: Option<std::path::PathBuf>,
    pub no_motion_blur: bool,
}

pub const HELP: &str = "\
Little Falls benchmark

Just run it: the menu does the rest. These options are for scripts.

USAGE: little-falls [OPTIONS]

OPTIONS:
  --preset <low|medium|high|ultra>  Graphics quality (default: high)
  --run <full|graphics|cpu>         Skip the menu and start this run
  --windowed <WIDTHxHEIGHT>         Run in a window instead of fullscreen
  --duration <seconds>              Length of the recorded flythrough (default: 60)
  --cpu-only                        CPU and memory tests in the terminal, no window
  --quick                           Short smoke-test workloads (scores not comparable)
  --explore                         Start in free-fly explore mode
  --exit-on-finish                  Quit as soon as results are saved
  --name <nickname>                 Name shown on the scoreboard
  --out <dir>                       Results folder (default: ~/.local/share/little-falls/results)
  --warmup <seconds>                Shader warm-up before timing starts (default: 5)
  --screenshots <dir>               Save PNG screenshots during the run
  -h, --help                        Show this help

CONTROLS:
  Arrows / D-pad    move between buttons
  Enter / A         select
  Esc / B           back
  L / Y             toggle long exposure water
";

impl Config {
    pub fn from_args() -> Result<Self, String> {
        let mut c = Config {
            preset: Preset::High,
            quality: Preset::High.quality(),
            duration: 60.0,
            windowed: None,
            auto_run: None,
            headless_cpu: false,
            quick: false,
            exit_on_finish: false,
            explore: false,
            nickname: String::new(),
            out_dir: None,
            warmup: 5.0,
            screenshot_dir: None,
            menu_screenshot: None,
            no_motion_blur: false,
        };
        let mut args = std::env::args().skip(1);
        while let Some(a) = args.next() {
            let mut val = |name: &str| args.next().ok_or(format!("{name} needs a value"));
            match a.as_str() {
                "-h" | "--help" => return Err(String::new()),
                "--preset" => {
                    let v = val("--preset")?;
                    c.preset = Preset::parse(&v).ok_or(format!("unknown preset '{v}'"))?;
                }
                "--duration" => {
                    let v = val("--duration")?;
                    c.duration = v
                        .parse::<f32>()
                        .ok()
                        .filter(|d| *d >= 5.0)
                        .ok_or("--duration must be a number >= 5")?;
                }
                "--windowed" => {
                    let v = val("--windowed")?;
                    let (w, h) = v.split_once(['x', 'X']).ok_or("use --windowed 1280x720")?;
                    let w = w.parse().map_err(|_| "bad width")?;
                    let h = h.parse().map_err(|_| "bad height")?;
                    c.windowed = Some((w, h));
                }
                "--cpu-only" => c.headless_cpu = true,
                "--run" => {
                    use crate::state::RunPlan;
                    c.auto_run = Some(match val("--run")?.as_str() {
                        "full" => RunPlan::FULL,
                        "graphics" => RunPlan::GRAPHICS,
                        "cpu" => RunPlan::CPU,
                        other => return Err(format!("unknown run '{other}' (full, graphics, cpu)")),
                    });
                }
                "--quick" => c.quick = true,
                "--explore" => c.explore = true,
                "--exit-on-finish" => c.exit_on_finish = true,
                "--name" => c.nickname = val("--name")?.chars().take(32).collect(),
                "--out" => c.out_dir = Some(val("--out")?.into()),
                "--warmup" => {
                    c.warmup = val("--warmup")?
                        .parse::<f64>()
                        .ok()
                        .filter(|w| *w >= 0.0)
                        .ok_or("--warmup must be a number >= 0")?;
                }
                "--screenshots" => c.screenshot_dir = Some(val("--screenshots")?.into()),
                // Motion blur scales with frame time, so at CI's ~1 fps it
                // smears everything; screenshots for review turn it off.
                "--no-motion-blur" => c.no_motion_blur = true,
                "--menu-screenshot" => c.menu_screenshot = Some(val("--menu-screenshot")?.into()),
                other => return Err(format!("unknown option '{other}'")),
            }
        }
        c.quality = c.preset.quality();
        if c.no_motion_blur {
            c.quality.motion_blur = false;
        }
        Ok(c)
    }
}
