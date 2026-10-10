//! A short fade from black whenever the app changes screen, so
//! transitions feel smooth instead of cutting.

use super::theme::BACKDROP;
use crate::state::AppState;
use bevy::picking::Pickable;
use bevy::prelude::*;

const FADE_SECONDS: f32 = 0.45;

#[derive(Component)]
struct FadeOverlay;

#[derive(Resource, Default)]
struct Fade {
    /// Seconds left in the current fade.
    left: f32,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Fade>()
        .add_systems(Startup, spawn)
        .add_systems(Update, (start_on_change, animate).chain());
}

fn spawn(mut commands: Commands) {
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            ..default()
        },
        BackgroundColor(BACKDROP),
        // Above everything, but never in the way of clicks.
        GlobalZIndex(100),
        Pickable::IGNORE,
        FadeOverlay,
    ));
}

fn start_on_change(state: Res<State<AppState>>, mut fade: ResMut<Fade>) {
    if state.is_changed() {
        fade.left = FADE_SECONDS;
    }
}

fn animate(
    time: Res<Time<Real>>,
    mut fade: ResMut<Fade>,
    mut q: Query<&mut BackgroundColor, With<FadeOverlay>>,
) {
    fade.left = (fade.left - time.delta_secs()).max(0.0);
    let t = fade.left / FADE_SECONDS;
    // Ease out: quick at first, settling gently.
    let alpha = t * t;
    for mut bg in &mut q {
        bg.0 = BACKDROP.with_alpha(alpha);
    }
}
