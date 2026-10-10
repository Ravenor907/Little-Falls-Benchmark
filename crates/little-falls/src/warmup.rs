//! Shader readiness. Bevy compiles render pipelines in the background and
//! only the first time an object with a given material and mesh layout is
//! actually seen. Timing a run while that happens measures the shader
//! compiler, not the GPU. So before timing, the camera sweeps the whole
//! flythrough path (making every object visible once, in both water modes),
//! and the run only starts when no pipelines are left compiling.

use crate::camera::{flythrough, MainCamera};
use crate::scene::water::LongExposure;
use bevy::prelude::*;
use bevy::render::render_resource::PipelineCache;
use bevy::render::{ExtractSchedule, MainWorld, RenderApp};

/// Number of render pipelines still compiling, mirrored from the render
/// world every frame.
#[derive(Resource, Default, Clone, Copy)]
pub struct PipelineStatus {
    pub waiting: usize,
}

pub struct WarmupPlugin;

impl Plugin for WarmupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PipelineStatus>();
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(ExtractSchedule, mirror_pipeline_status);
        }
    }
}

fn mirror_pipeline_status(mut main_world: ResMut<MainWorld>, cache: Res<PipelineCache>) {
    if let Some(mut status) = main_world.get_resource_mut::<PipelineStatus>() {
        status.waiting = cache.waiting_pipelines().count();
    }
}

/// Drives one shader sweep. Progress is counted in frames, not seconds,
/// so slow GPUs still visit every part of the path.
#[derive(Clone, Copy, Debug)]
pub struct Sweep {
    pub frames: u32,
    pub frame: u32,
    /// Consecutive frames with nothing compiling.
    pub quiet_frames: u32,
}

impl Sweep {
    pub fn new(frames: u32) -> Self {
        Self {
            frames,
            frame: 0,
            quiet_frames: 0,
        }
    }

    /// Advances one frame: moves the camera along the path and tracks
    /// compile activity. Returns true once the path has been covered and
    /// compilation has been quiet for a moment.
    pub fn step(
        &mut self,
        status: &PipelineStatus,
        cam: &mut Query<&mut Transform, With<MainCamera>>,
        long: &mut LongExposure,
    ) -> bool {
        let p = (self.frame as f32 / self.frames as f32).min(1.0);
        if let Ok(mut tf) = cam.single_mut() {
            let (pos, look) = flythrough(p);
            *tf = Transform::from_translation(pos).looking_at(look, Vec3::Y);
        }
        // Both water looks use different textures; show each once.
        long.0 = p > 0.5 && p < 1.0;
        self.frame = self.frame.saturating_add(1);
        if status.waiting == 0 {
            self.quiet_frames += 1;
        } else {
            self.quiet_frames = 0;
        }
        self.frame >= self.frames && self.quiet_frames >= 30
    }

    /// 0..1 for progress bars: path coverage, held just short of full
    /// until compilation settles.
    pub fn progress(&self) -> f32 {
        let path = (self.frame as f32 / self.frames as f32).min(1.0);
        if path >= 1.0 && self.quiet_frames < 30 {
            0.97
        } else {
            path
        }
    }
}
