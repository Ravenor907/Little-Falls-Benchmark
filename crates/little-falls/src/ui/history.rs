//! Past runs, newest first, read from the results folder.

use super::theme::*;
use super::{button, UiAction};
use crate::config::Config;
use crate::state::{despawn_screen, AppState, Screen};
use bevy::prelude::*;
use lf_core::results::{self, HistoryEntry};

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::History), spawn)
        .add_systems(OnExit(AppState::History), despawn_screen);
}

const MAX_ROWS: usize = 10;

fn spawn(mut commands: Commands, fonts: Res<Fonts>, config: Res<Config>) {
    let f = &*fonts;
    let dir = config.out_dir.clone().unwrap_or_else(results::default_dir);
    let entries = results::load_history(&dir);

    commands
        .spawn((root(JustifyContent::Center, AlignItems::Center), Screen))
        .with_children(|r| {
            r.spawn(panel(Val::Px(760.0))).with_children(|p| {
                p.spawn(text("HISTORY", &f.semibold, 12.0, ACCENT));
                p.spawn(text("Your runs", &f.light, 40.0, TEXT));

                if entries.is_empty() {
                    p.spawn(card()).with_children(|c| {
                        c.spawn(text(
                            "No results yet. Finished runs appear here.",
                            &f.regular,
                            15.0,
                            TEXT_DIM,
                        ));
                    });
                } else {
                    p.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|list| {
                        header_row(list, f);
                        for e in entries.iter().take(MAX_ROWS) {
                            entry_row(list, f, e);
                        }
                    });
                    if entries.len() > MAX_ROWS {
                        p.spawn(text(
                            format!("Showing the latest {MAX_ROWS} of {} runs.", entries.len()),
                            &f.regular,
                            12.0,
                            TEXT_FAINT,
                        ));
                    }
                }
                p.spawn(text(
                    format!("Saved in {}", dir.display()),
                    &f.regular,
                    12.0,
                    TEXT_FAINT,
                ));
                p.spawn(Node {
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|b| {
                    button(b, f, "Back", 0, UiAction::Menu, true);
                });
            });
        });
}

/// Column widths: date, overall, graphics, cpu, memory, test, gpu.
const COLS: [f32; 7] = [140.0, 70.0, 70.0, 60.0, 70.0, 130.0, 160.0];

fn cell(row: &mut ChildSpawnerCommands, w: f32, content: impl Bundle) {
    row.spawn(Node {
        width: Val::Px(w),
        flex_shrink: 0.0,
        overflow: Overflow::clip_x(),
        ..default()
    })
    .with_children(|c| {
        c.spawn(content);
    });
}

fn header_row(list: &mut ChildSpawnerCommands, f: &Fonts) {
    list.spawn(Node {
        padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
        ..default()
    })
    .with_children(|row| {
        for (w, label) in COLS.iter().zip([
            "DATE (UTC)",
            "OVERALL",
            "GRAPHICS",
            "CPU",
            "MEMORY",
            "TEST",
            "GPU",
        ]) {
            cell(row, *w, text(label, &f.semibold, 11.0, TEXT_FAINT));
        }
    });
}

fn entry_row(list: &mut ChildSpawnerCommands, f: &Fonts, e: &HistoryEntry) {
    let score = |v: f64| if v > 0.0 { format!("{v:.0}") } else { "–".into() };
    let test = if e.preset.is_empty() {
        "CPU and memory".to_string()
    } else {
        format!("{} {}p", e.preset, e.resolution.1)
    };
    list.spawn((
        Node {
            padding: UiRect::axes(Val::Px(12.0), Val::Px(9.0)),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(CARD),
        BorderRadius::all(Val::Px(8.0)),
    ))
    .with_children(|row| {
        cell(
            row,
            COLS[0],
            text(results::format_utc(e.timestamp), &f.regular, 13.0, TEXT_DIM),
        );
        cell(row, COLS[1], text(score(e.overall), &f.semibold, 16.0, TEXT));
        cell(row, COLS[2], text(score(e.graphics), &f.regular, 14.0, TEXT));
        cell(row, COLS[3], text(score(e.cpu), &f.regular, 14.0, TEXT));
        cell(row, COLS[4], text(score(e.memory), &f.regular, 14.0, TEXT));
        let flag = if e.warnings > 0 { "  !" } else { "" };
        cell(
            row,
            COLS[5],
            text(
                format!("{test}{flag}"),
                &f.regular,
                13.0,
                if e.warnings > 0 { WARN } else { TEXT_DIM },
            ),
        );
        cell(row, COLS[6], text(e.gpu.clone(), &f.regular, 12.0, TEXT_FAINT));
    });
}
