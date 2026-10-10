//! The interface: menu, CPU test progress, benchmark HUD and results.
//! Every control works with a mouse, a keyboard or a gamepad, so it is
//! comfortable on a desktop and on a handheld in game mode.

pub mod cpu_screen;
pub mod hud;
pub mod loading;
pub mod menu;
pub mod results;
pub mod theme;

use crate::config::{Config, Preset};
use crate::state::{start_run, AppState, RunPlan};
use bevy::prelude::*;
use theme::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiAction {
    Preset(Preset),
    Run(RunPlan),
    RunAgain,
    Explore,
    Menu,
    Quit,
}

/// Something the user can focus and activate. `order` sets the
/// keyboard/gamepad traversal order.
#[derive(Component)]
pub struct Focusable {
    pub order: u32,
    pub action: UiAction,
    pub primary: bool,
}

#[derive(Resource, Default)]
pub struct Focus {
    pub order: Option<u32>,
    /// Time of the last stick-driven move, for auto-repeat pacing.
    last_move: f32,
}

#[derive(Event)]
pub struct ActionEvent(pub UiAction);

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Focus>()
            .add_event::<ActionEvent>()
            .add_systems(PreStartup, load_fonts)
            .add_systems(
                Update,
                (
                    pointer_input,
                    nav_input,
                    back_input,
                    style_focusables,
                    handle_actions,
                )
                    .chain(),
            )
            .add_plugins((
                menu::plugin,
                cpu_screen::plugin,
                hud::plugin,
                results::plugin,
                loading::plugin,
            ));
    }
}

/// A focusable button with a centered label.
pub fn button(
    parent: &mut ChildSpawnerCommands,
    fonts: &Fonts,
    label: &str,
    order: u32,
    action: UiAction,
    primary: bool,
) {
    parent
        .spawn((
            Button,
            Node {
                height: Val::Px(if primary { 52.0 } else { 42.0 }),
                flex_grow: 1.0,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(18.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(if primary { ACCENT } else { BUTTON }),
            BorderColor(Color::NONE),
            BorderRadius::all(Val::Px(RADIUS_SMALL)),
            Focusable {
                order,
                action,
                primary,
            },
        ))
        .with_children(|b| {
            let (font, size, color) = if primary {
                (&fonts.semibold, 17.0, Color::srgb(0.04, 0.08, 0.04))
            } else {
                (&fonts.regular, 15.0, TEXT)
            };
            b.spawn(text(label, font, size, color));
        });
}

fn pointer_input(
    q: Query<(&Interaction, &Focusable), Changed<Interaction>>,
    mut focus: ResMut<Focus>,
    mut events: EventWriter<ActionEvent>,
) {
    for (interaction, f) in &q {
        match interaction {
            Interaction::Pressed => {
                focus.order = Some(f.order);
                events.write(ActionEvent(f.action));
            }
            Interaction::Hovered => focus.order = Some(f.order),
            Interaction::None => {}
        }
    }
}

/// Arrow keys, Tab, D-pad and left stick move focus; Enter, Space and
/// the A button activate.
fn nav_input(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    items: Query<&Focusable>,
    mut focus: ResMut<Focus>,
    mut events: EventWriter<ActionEvent>,
) {
    let mut orders: Vec<u32> = items.iter().map(|f| f.order).collect();
    if orders.is_empty() {
        return;
    }
    orders.sort_unstable();

    let mut step: i32 = 0;
    let mut activate = false;
    if keys.any_just_pressed([KeyCode::ArrowDown, KeyCode::ArrowRight, KeyCode::Tab]) {
        step = 1;
    }
    if keys.any_just_pressed([KeyCode::ArrowUp, KeyCode::ArrowLeft]) {
        step = -1;
    }
    if keys.any_just_pressed([KeyCode::Enter, KeyCode::Space, KeyCode::NumpadEnter]) {
        activate = true;
    }
    let now = time.elapsed_secs();
    for g in &pads {
        if g.digital()
            .any_just_pressed([GamepadButton::DPadDown, GamepadButton::DPadRight])
        {
            step = 1;
        }
        if g.digital()
            .any_just_pressed([GamepadButton::DPadUp, GamepadButton::DPadLeft])
        {
            step = -1;
        }
        if g.just_pressed(GamepadButton::South) {
            activate = true;
        }
        // Left stick with a repeat delay.
        let s = g.left_stick();
        if now - focus.last_move > 0.28 {
            if s.y < -0.6 || s.x > 0.6 {
                step = 1;
                focus.last_move = now;
            } else if s.y > 0.6 || s.x < -0.6 {
                step = -1;
                focus.last_move = now;
            }
        }
    }

    // Default focus lands on the primary action.
    let current = focus.order.filter(|o| orders.contains(o)).unwrap_or_else(|| {
        items
            .iter()
            .find(|f| f.primary)
            .map(|f| f.order)
            .unwrap_or(orders[0])
    });
    let idx = orders.iter().position(|o| *o == current).unwrap_or(0) as i32;
    let next = (idx + step).rem_euclid(orders.len() as i32) as usize;
    focus.order = Some(orders[next]);

    if activate {
        if let Some(f) = items.iter().find(|f| Some(f.order) == focus.order) {
            events.write(ActionEvent(f.action));
        }
    }
}

/// Escape and the B button go back: to the menu from any screen, and
/// quit from the menu.
fn back_input(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    state: Res<State<AppState>>,
    mut events: EventWriter<ActionEvent>,
) {
    let back = keys.just_pressed(KeyCode::Escape) || pads.iter().any(|g| g.just_pressed(GamepadButton::East));
    if back {
        events.write(ActionEvent(match state.get() {
            AppState::Menu => UiAction::Quit,
            _ => UiAction::Menu,
        }));
    }
}

fn style_focusables(
    focus: Res<Focus>,
    config: Res<Config>,
    mut q: Query<(&Focusable, &Interaction, &mut BackgroundColor, &mut BorderColor)>,
) {
    for (f, interaction, mut bg, mut border) in &mut q {
        let focused = focus.order == Some(f.order);
        let selected = matches!(f.action, UiAction::Preset(p) if p == config.preset);
        let pressed = *interaction == Interaction::Pressed;
        bg.0 = if f.primary {
            if focused || pressed {
                Color::srgb(0.72, 0.93, 0.64)
            } else {
                ACCENT
            }
        } else if selected {
            ACCENT_DEEP
        } else if focused {
            BUTTON_HOVER
        } else {
            BUTTON
        };
        border.0 = if focused {
            Color::srgba(0.75, 0.95, 0.68, 0.9)
        } else if selected {
            Color::srgba(0.62, 0.86, 0.55, 0.45)
        } else {
            Color::NONE
        };
    }
}

fn handle_actions(
    mut events: EventReader<ActionEvent>,
    mut commands: Commands,
    mut config: ResMut<Config>,
    mut next: ResMut<NextState<AppState>>,
    plan: Option<Res<RunPlan>>,
    mut focus: ResMut<Focus>,
    mut exit: EventWriter<AppExit>,
) {
    for ActionEvent(action) in events.read() {
        match *action {
            UiAction::Preset(p) => {
                if config.preset != p {
                    config.preset = p;
                    config.quality = p.quality();
                }
            }
            UiAction::Run(p) => {
                focus.order = None;
                start_run(p, &mut commands, &mut next);
            }
            UiAction::RunAgain => {
                focus.order = None;
                let p = plan.as_deref().copied().unwrap_or(RunPlan::FULL);
                start_run(p, &mut commands, &mut next);
            }
            UiAction::Explore => {
                focus.order = None;
                next.set(AppState::Explore);
            }
            UiAction::Menu => {
                focus.order = None;
                next.set(AppState::Menu);
            }
            UiAction::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}
