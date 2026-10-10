//! App flow: menu, CPU and memory tests, graphics test, results, plus a
//! free-roam explore mode.

use crate::config::Config;
use bevy::prelude::*;
use lf_core::score::FrameStats;
use lf_core::sysinfo::SystemInfo;
use lf_core::Measurement;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppState {
    /// One frame while startup finishes (fonts, GPU info, scene).
    #[default]
    Boot,
    /// Loading screen while shaders compile.
    Loading,
    Menu,
    CpuTest,
    Graphics,
    Results,
    Explore,
}

/// Which parts of the benchmark the current run includes.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunPlan {
    pub cpu: bool,
    pub graphics: bool,
}

impl RunPlan {
    pub const FULL: Self = Self {
        cpu: true,
        graphics: true,
    };
    pub const GRAPHICS: Self = Self {
        cpu: false,
        graphics: true,
    };
    pub const CPU: Self = Self {
        cpu: true,
        graphics: false,
    };

    pub fn first_state(self) -> AppState {
        if self.cpu {
            AppState::CpuTest
        } else {
            AppState::Graphics
        }
    }
}

/// Everything collected during a run.
#[derive(Resource, Default)]
pub struct RunData {
    pub cpu: Vec<Measurement>,
    pub frames: Option<FrameStats>,
    pub resolution: (u32, u32),
    /// Problems that make the result less trustworthy, shown on the
    /// results screen and saved in the results file.
    pub warnings: Vec<String>,
}

/// System details, with the GPU filled in once the renderer is up.
#[derive(Resource, Clone)]
pub struct System(pub SystemInfo);

/// Marker for UI belonging to one screen; despawned when the state exits.
#[derive(Component)]
pub struct Screen;

pub fn despawn_screen(mut commands: Commands, q: Query<Entity, With<Screen>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

pub fn start_run(plan: RunPlan, commands: &mut Commands, next: &mut NextState<AppState>) {
    commands.insert_resource(plan);
    commands.insert_resource(RunData::default());
    next.set(plan.first_state());
}

/// Fills in GPU details from the Vulkan adapter.
pub fn detect_gpu(mut sys: ResMut<System>, adapter: Option<Res<bevy::render::renderer::RenderAdapterInfo>>) {
    if let Some(a) = adapter {
        sys.0.gpu_name = a.name.clone();
        sys.0.gpu_driver = format!("{} {}", a.driver, a.driver_info).trim().to_string();
        sys.0.gpu_backend = format!("{:?}", a.backend);
    }
}

pub fn enter_loading(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Loading);
}

/// Where to go once loading finishes. `--run` skips the menu (used by
/// scripts and CI).
pub fn after_loading(config: &Config, commands: &mut Commands, next: &mut NextState<AppState>) {
    if let Some(plan) = config.auto_run {
        start_run(plan, commands, next);
    } else if config.explore {
        next.set(AppState::Explore);
    } else {
        next.set(AppState::Menu);
    }
}
