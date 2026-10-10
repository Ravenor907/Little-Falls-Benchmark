//! Visual language for the interface: a dark, translucent "glass" panel
//! over the live forest scene, Inter type, and a soft moss green accent.

use bevy::prelude::*;

pub const TEXT: Color = Color::srgba(0.95, 0.97, 0.95, 0.94);
pub const TEXT_DIM: Color = Color::srgba(0.85, 0.90, 0.86, 0.58);
pub const TEXT_FAINT: Color = Color::srgba(0.85, 0.90, 0.86, 0.36);
pub const ACCENT: Color = Color::srgb(0.62, 0.86, 0.55);
pub const ACCENT_DEEP: Color = Color::srgb(0.20, 0.34, 0.20);
pub const PANEL: Color = Color::srgba(0.035, 0.05, 0.045, 0.74);
pub const CARD: Color = Color::srgba(1.0, 1.0, 1.0, 0.045);
pub const HAIRLINE: Color = Color::srgba(1.0, 1.0, 1.0, 0.09);
pub const BUTTON: Color = Color::srgba(1.0, 1.0, 1.0, 0.06);
pub const BUTTON_HOVER: Color = Color::srgba(1.0, 1.0, 1.0, 0.12);
pub const BACKDROP: Color = Color::srgb(0.02, 0.03, 0.028);

pub const RADIUS: f32 = 18.0;
pub const RADIUS_SMALL: f32 = 10.0;

#[derive(Resource, Clone)]
pub struct Fonts {
    pub light: Handle<Font>,
    pub regular: Handle<Font>,
    pub semibold: Handle<Font>,
}

impl Fonts {
    /// Fonts are compiled into the executable so the app is a single
    /// file with nothing to install.
    pub fn load(fonts: &mut Assets<Font>) -> Self {
        let f = |bytes: &'static [u8]| Font::try_from_bytes(bytes.to_vec()).expect("embedded font");
        Self {
            light: fonts.add(f(include_bytes!("../../assets/fonts/Inter-Light.ttf"))),
            regular: fonts.add(f(include_bytes!("../../assets/fonts/Inter-Regular.ttf"))),
            semibold: fonts.add(f(include_bytes!("../../assets/fonts/Inter-SemiBold.ttf"))),
        }
    }
}

pub fn load_fonts(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    commands.insert_resource(Fonts::load(&mut fonts));
}

/// Text bundle helper.
pub fn text(s: impl Into<String>, font: &Handle<Font>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}

/// The translucent card that holds each screen's content.
pub fn panel(width: Val) -> impl Bundle {
    (
        Node {
            width,
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(32.0)),
            row_gap: Val::Px(18.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor(HAIRLINE),
        BorderRadius::all(Val::Px(RADIUS)),
    )
}

/// A subtle inset card inside a panel.
pub fn card() -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(Val::Px(18.0), Val::Px(14.0)),
            row_gap: Val::Px(6.0),
            ..default()
        },
        BackgroundColor(CARD),
        BorderRadius::all(Val::Px(RADIUS_SMALL)),
    )
}

/// Full-window root node, used to position a screen's panel.
pub fn root(justify: JustifyContent, align: AlignItems) -> Node {
    Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        justify_content: justify,
        align_items: align,
        padding: UiRect::all(Val::Px(40.0)),
        ..default()
    }
}

/// A key/value row such as "GPU   <adapter name>".
pub fn spec_row(parent: &mut ChildSpawnerCommands, fonts: &Fonts, key: &str, value: &str) {
    parent
        .spawn(Node {
            column_gap: Val::Px(14.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((Node {
                width: Val::Px(64.0),
                flex_shrink: 0.0,
                ..default()
            },))
                .with_children(|k| {
                    k.spawn(text(key, &fonts.semibold, 12.0, TEXT_FAINT));
                });
            row.spawn(text(value, &fonts.regular, 14.0, TEXT));
        });
}
