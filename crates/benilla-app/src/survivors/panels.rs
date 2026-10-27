//! The pause and results screens as the game's own dialogs: `GameMenuFrame`'s `UI-DialogBox`
//! backdrop under its `UI-DialogBox-Header` plaque, with the glue screens' red buttons
//! ([`crate::glue::widgets::glue_button`]), so they match the pick screen. Built into
//! [`PauseRoot`] and [`OverRoot`] once the glue art is in.

use bevy::prelude::*;

use super::hud::{
    centered_bundle, text_bundle, HudFont, OverLine, OverRoot, OverStat, OverStats, OverTitle,
    PauseRoot, UiAction, DIM_GREY, GOLD_TONE,
};
use super::Phase;
use crate::glue::art::GlueArt;
use crate::glue::backdrop::{backdrop_border, tiled_bg_node};
use crate::glue::widgets::{abs, glue_button, GlueBtnKind};
use crate::glue::GlueClicks;

/// `GameMenuFrame`'s width, widened to the glue buttons.
const PAUSE_W: f32 = 240.0;
/// Room for two `GlueDialogButtonTemplate`s with the dialog's 13 gap, and the stat table.
const OVER_W: f32 = 460.0;

/// On a root once its dialog is built.
#[derive(Component)]
pub(super) struct PanelBuilt;

/// The dialog frame: the background tiled at 32 inside (11,12,12,11), the 32-edge border over
/// it, and the header plaque (256×64 at TOP (0,12)) with its title 14 down in it.
fn dialog(
    root: &mut ChildSpawnerCommands,
    art: &GlueArt,
    w: f32,
    title: impl Bundle,
    body: impl FnOnce(&mut ChildSpawnerCommands),
) {
    root.spawn(Node {
        width: Val::Px(w),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        padding: UiRect::new(Val::ZERO, Val::ZERO, Val::Px(34.0), Val::Px(22.0)),
        row_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|b| {
        if let (Some(bg), Some(border)) = (&art.dialog_bg, &art.dialog_border) {
            // A dark fill under the translucent tiles, so the fight doesn't show through.
            b.spawn((
                tiled_bg_node(bg.clone(), 32.0, 1.0, Color::WHITE),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(11.0),
                    right: Val::Px(12.0),
                    top: Val::Px(12.0),
                    bottom: Val::Px(11.0),
                    ..default()
                },
            ));
            backdrop_border(b, border, 32.0, Color::WHITE);
        } else {
            b.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.05, 0.08, 0.95)),
            ));
        }
        let left = (w - 256.0) / 2.0;
        if let Some((header, _)) = &art.dialog_header {
            b.spawn((
                ImageNode::new(header.clone()),
                abs(1.0, left, -12.0, 256.0, 64.0),
            ));
        }
        b.spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(left),
            width: Val::Px(256.0),
            top: Val::Px(1.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|t| {
            t.spawn(title);
        });
        body(b);
    });
}

pub(super) fn build_panels(
    mut commands: Commands,
    pause: Query<Entity, (With<PauseRoot>, Without<PanelBuilt>)>,
    over: Query<Entity, (With<OverRoot>, Without<PanelBuilt>)>,
    assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut art: ResMut<GlueArt>,
    mut images: ResMut<Assets<Image>>,
    mut add_mats: ResMut<Assets<crate::glue::add_material::AddUiMaterial>>,
    font: Res<HudFont>,
) {
    if pause.is_empty() && over.is_empty() {
        return;
    }
    let Some(mut assets) = assets else { return };
    art.ensure_loaded(&mut assets, &mut images, &mut add_mats);
    let f = &font.0;
    let art = &*art;

    for root in &pause {
        commands
            .entity(root)
            .insert(PanelBuilt)
            .with_children(|root| {
                let title = centered_bundle(f, "Paused", 14.0, GOLD_TONE);
                dialog(root, art, PAUSE_W, title, |b| {
                    for (label, action) in [
                        ("Return to Game", UiAction::Resume),
                        ("Abandon Run", UiAction::MainMenu),
                        ("Exit Game", UiAction::Quit),
                    ] {
                        glue_button(
                            b,
                            art,
                            f,
                            action,
                            label,
                            180.0,
                            34.0,
                            GlueBtnKind::Small,
                            1.0,
                        );
                    }
                });
            });
    }

    for root in &over {
        commands
            .entity(root)
            .insert(PanelBuilt)
            .with_children(|root| {
                let title = (OverTitle, centered_bundle(f, "", 14.0, GOLD_TONE));
                dialog(root, art, OVER_W, title, |b| {
                    b.spawn(Node {
                        max_width: Val::Px(OVER_W - 60.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .with_children(|n| {
                        n.spawn((OverLine, centered_bundle(f, "", 14.0, Color::WHITE)));
                    });
                    // The run in numbers: GameFontNormal labels, GameFontHighlight values.
                    b.spawn(Node {
                        width: Val::Px(280.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        margin: UiRect::vertical(Val::Px(6.0)),
                        ..default()
                    })
                    .with_children(|table| {
                        for (i, label) in ["Survived", "Level", "Kills", "Damage", "Bosses"]
                            .into_iter()
                            .enumerate()
                        {
                            table
                                .spawn(Node {
                                    justify_content: JustifyContent::SpaceBetween,
                                    ..default()
                                })
                                .with_children(|row| {
                                    row.spawn(text_bundle(f, label, 14.0, GOLD_TONE));
                                    row.spawn((
                                        OverStat(i),
                                        text_bundle(f, "", 14.0, Color::WHITE),
                                    ));
                                });
                        }
                    });
                    b.spawn((OverStats, centered_bundle(f, "", 12.0, DIM_GREY)));
                    b.spawn(Node {
                        column_gap: Val::Px(13.0),
                        margin: UiRect::top(Val::Px(6.0)),
                        ..default()
                    })
                    .with_children(|row| {
                        for (label, action) in [
                            ("Play Again", UiAction::MainMenu),
                            ("Exit Game", UiAction::Quit),
                        ] {
                            glue_button(
                                row,
                                art,
                                f,
                                action,
                                label,
                                190.0,
                                38.0,
                                GlueBtnKind::Dialog,
                                1.0,
                            );
                        }
                    });
                });
            });
    }
}

/// The dialogs' buttons fire on the click, as the glue screens' do.
pub(super) fn buttons(
    clicks: Res<GlueClicks>,
    actions: Query<(Entity, &UiAction)>,
    phase: Res<State<Phase>>,
    mut next: ResMut<NextState<Phase>>,
    mut exit: MessageWriter<AppExit>,
) {
    for (e, action) in &actions {
        if !clicks.hit(e) {
            continue;
        }
        match (*action, *phase.get()) {
            (UiAction::Resume, Phase::Paused) => next.set(Phase::Playing),
            (UiAction::MainMenu, Phase::Paused | Phase::GameOver) => next.set(Phase::Menu),
            (UiAction::Quit, Phase::Paused | Phase::GameOver) => {
                exit.write(AppExit::Success);
            }
            _ => {}
        }
    }
}
