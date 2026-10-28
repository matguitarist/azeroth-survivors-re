//! Reroll and Banish under the level-up cards: the merchant's Fortune and Banishment, spent a
//! charge at a time. R and T press them, a pad's bumpers too.

use bevy::prelude::*;

use super::hud::{self, centered_bundle, HudFont, LevelUpRoot, LevelUpSub, Offer, DIM_GREY};
use super::{Phase, Rng, Run};
use crate::glue::art::GlueArt;
use crate::glue::widgets::{glue_button, GlueBtnKind, GlueDisabled};
use crate::glue::GlueClicks;
use crate::sound::GlueSound;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum LevelUpAction {
    Reroll,
    Banish,
}

#[derive(Component)]
pub(super) struct LevelUpBuilt;
/// The row, hidden in a run that has neither.
#[derive(Component)]
pub(super) struct ChargesRow;
#[derive(Component)]
pub(super) struct ChargesText;

const SUB: &str = "Choose your power";
const SUB_BANISH: &str = "Choose a new spell or aura to banish from this run";

/// The two buttons and the charges line, under the cards' hint once the glue art is in.
pub(super) fn build_levelup(
    mut commands: Commands,
    roots: Query<Entity, (With<LevelUpRoot>, Without<LevelUpBuilt>)>,
    built: Query<(), With<super::panels::PanelBuilt>>,
    art: Res<GlueArt>,
    font: Res<HudFont>,
) {
    // The pause panel is built from the same art; once it is, the art is in.
    if built.is_empty() {
        return;
    }
    let Ok(root) = roots.single() else { return };
    let f = &font.0;
    let art = &*art;
    commands
        .entity(root)
        .insert(LevelUpBuilt)
        .with_children(|root| {
            root.spawn((
                ChargesRow,
                // Out of the column's flow, at the foot of the screen: the cards fill the middle.
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(22.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(4.0),
                    ..default()
                },
            ))
            .with_children(|c| {
                c.spawn(Node {
                    column_gap: Val::Px(16.0),
                    ..default()
                })
                .with_children(|row| {
                    for (a, caption) in [
                        (LevelUpAction::Reroll, "Reroll"),
                        (LevelUpAction::Banish, "Banish"),
                    ] {
                        glue_button(
                            row,
                            art,
                            f,
                            a,
                            caption,
                            160.0,
                            40.0,
                            GlueBtnKind::Normal,
                            1.0,
                        );
                    }
                });
                c.spawn((ChargesText, centered_bundle(f, "", 13.0, DIM_GREY)));
            });
        });
}

/// The buttons and their keys, and the lines that show what is left and whether a banish waits.
#[allow(clippy::too_many_arguments)]
pub(super) fn levelup(
    clicks: Res<GlueClicks>,
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<State<Phase>>,
    mut run: ResMut<Run>,
    mut offer: ResMut<Offer>,
    mut rng: ResMut<Rng>,
    mut buttons: Query<(Entity, &LevelUpAction, &mut GlueDisabled)>,
    mut row: Query<&mut Node, With<ChargesRow>>,
    mut texts: ParamSet<(
        Query<&mut Text, With<ChargesText>>,
        Query<(&mut Text, &mut TextColor), With<LevelUpSub>>,
    )>,
    mut sounds: MessageWriter<GlueSound>,
) {
    if *phase.get() != Phase::LevelUp {
        return;
    }
    let mut pressed = None;
    for (e, &a, _) in &buttons {
        if clicks.hit(e) {
            pressed = Some(a);
        }
    }
    if keys.just_pressed(KeyCode::KeyR) {
        pressed = Some(LevelUpAction::Reroll);
    }
    if keys.just_pressed(KeyCode::KeyT) {
        pressed = Some(LevelUpAction::Banish);
    }
    match pressed {
        Some(LevelUpAction::Reroll) => {
            if hud::reroll(&mut run, &mut offer, &mut rng) {
                sounds.write(GlueSound("igMainMenuOption"));
            }
        }
        // Banish arms the next pick; pressed again, it stands down.
        Some(LevelUpAction::Banish) if run.banishes > 0 || run.banishing => {
            run.banishing = !run.banishing;
            sounds.write(GlueSound("igMainMenuOption"));
        }
        _ => {}
    }
    // Nothing on offer can go: there is nothing to arm.
    if run.banishing
        && !offer
            .choices()
            .iter()
            .any(|&c| hud::banish_key(c).is_some())
    {
        run.banishing = false;
    }

    let any = run.rerolls > 0 || run.banishes > 0 || !run.banished.is_empty();
    for mut node in &mut row {
        let want = if any { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    for (_, &a, mut off) in &mut buttons {
        let want = match a {
            LevelUpAction::Reroll => run.rerolls == 0,
            LevelUpAction::Banish => run.banishes == 0 && !run.banishing,
        };
        if off.0 != want {
            off.0 = want;
        }
    }
    let line = format!(
        "Rerolls {} (R), banishes {} (T); on a controller, the bumpers",
        run.rerolls, run.banishes
    );
    for mut t in &mut texts.p0() {
        if t.0 != line {
            t.0 = line.clone();
        }
    }
    let (sub, color) = if run.banishing {
        (SUB_BANISH, Color::srgb(1.0, 0.35, 0.3))
    } else {
        (SUB, Color::srgb(0.85, 0.78, 0.6))
    };
    for (mut t, mut c) in &mut texts.p1() {
        if t.0 != sub {
            t.0 = sub.to_string();
            c.0 = color;
        }
    }
}
