//! Controllers. A pad's buttons press the keys the mode already reads, mapped per screen, so
//! every handler serves both; its sticks move the hero and swing the free camera. While the pad
//! is in use the cursor stops aiming: the hero faces where he runs and the class movement spell
//! goes straight ahead. Moving the mouse hands control back.
//!
//! Off the battlefield the D-pad or the left stick moves a focus across the screen's buttons,
//! the cards, the pick screen and the zone list, and A clicks it: [`focus`] stands in for the
//! cursor by setting the focused widget's [`Interaction`], so every hover look and click handler
//! serves the pad unchanged.

use bevy::input::gamepad::{Gamepad, GamepadButton, GamepadConnection, GamepadConnectionEvent};
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use bevy::window::CursorMoved;

use super::cards::Card;
use super::minimap::MapButton;
use super::Phase;

/// A stick's radial dead zone.
const DEAD: f32 = 0.2;

#[derive(Resource, Default)]
pub(super) struct Pad {
    /// The pad drove last; the mouse has not moved since.
    pub active: bool,
    /// The sticks past the dead zone, rescaled to 0..1, up positive.
    pub left: Vec2,
    pub right: Vec2,
    /// A focus step asked this frame, screen axes (y down); zero for none.
    nav: Vec2,
    /// A, held.
    confirm: bool,
}

/// A held direction steps again after this, then every [`REPEAT`].
const REPEAT_FIRST: f32 = 0.35;
const REPEAT: f32 = 0.14;

/// The widget the pad's focus is on.
#[derive(Resource, Default)]
pub(super) struct PadFocus(Option<Entity>);

/// Move the focus and stand it in for the cursor: after the UI's own hover pass, before the glue
/// clicks are read, so A's release on a button is a click as the mouse's is.
#[allow(clippy::type_complexity)]
pub(super) fn focus(
    phase: Res<State<Phase>>,
    pad: Res<Pad>,
    mut focus: ResMut<PadFocus>,
    mut widgets: Query<
        (
            Entity,
            &mut Interaction,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            Option<&crate::glue::widgets::GlueDisabled>,
            Has<Card>,
        ),
        (With<Button>, Without<MapButton>),
    >,
) {
    let cards = match phase.get() {
        Phase::LevelUp => true,
        Phase::Menu | Phase::Paused | Phase::GameOver => false,
        _ => {
            focus.0 = None;
            return;
        }
    };
    if !pad.active {
        return;
    }
    // What can take the focus: shown, laid out and enabled; on the level-up screen the cards.
    let spots: Vec<(Entity, Vec2)> = widgets
        .iter()
        .filter(|(_, _, node, _, vis, off, card)| {
            *card == cards
                && vis.get()
                && node.size().cmpgt(Vec2::ONE).all()
                && !off.is_some_and(|d| d.0)
        })
        .map(|(e, _, _, at, ..)| (e, at.translation))
        .collect();
    let here = focus
        .0
        .and_then(|e| spots.iter().find(|s| s.0 == e).copied());
    focus.0 = match here {
        Some(at) if pad.nav != Vec2::ZERO => step(at, pad.nav, &spots).or(Some(at.0)),
        Some(at) => Some(at.0),
        // The first step only shows the focus where it lands: the top, then the left.
        None => spots
            .iter()
            .min_by(|a, b| {
                let row = |p: Vec2| (p.y / 24.0).round();
                row(a.1).total_cmp(&row(b.1)).then(a.1.x.total_cmp(&b.1.x))
            })
            .map(|s| s.0),
    };
    let Some(e) = focus.0 else { return };
    if let Ok((_, mut interaction, ..)) = widgets.get_mut(e) {
        let want = if pad.confirm {
            Interaction::Pressed
        } else {
            Interaction::Hovered
        };
        interaction.set_if_neq(want);
    }
}

/// The spot a step in `dir` lands on: ahead of `from`, nearest, straying least off the line.
fn step(from: (Entity, Vec2), dir: Vec2, spots: &[(Entity, Vec2)]) -> Option<Entity> {
    spots
        .iter()
        .filter(|s| s.0 != from.0)
        .filter_map(|&(e, at)| {
            let v = at - from.1;
            let along = v.dot(dir);
            let off = (v - dir * along).length();
            (along > 1.0).then_some((e, along + off * 2.5))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|s| s.0)
}

/// Name each pad as it connects, with its ids, so an unmapped one can be told apart. A pad that
/// never shows here is not a HID device to the system: the Amazon Luna on a Mac's USB speaks
/// Xbox 360's vendor protocol, which macOS has no driver for; over Bluetooth it is a mapped HID
/// pad.
pub(super) fn log_pads(mut events: MessageReader<GamepadConnectionEvent>) {
    for e in events.read() {
        match &e.connection {
            GamepadConnection::Connected {
                name,
                vendor_id,
                product_id,
            } => info!(
                "gamepad connected: {name} (vendor {:04x}, product {:04x})",
                vendor_id.unwrap_or(0),
                product_id.unwrap_or(0)
            ),
            GamepadConnection::Disconnected => info!("gamepad disconnected"),
        }
    }
}

/// The keys a button presses on each screen.
fn keys_for(phase: Phase, button: GamepadButton) -> &'static [KeyCode] {
    use GamepadButton as B;
    match (phase, button) {
        // A clicks the focus ([`focus`]); Start still begins and plays again.
        (Phase::Menu | Phase::GameOver, B::Start) => &[KeyCode::Enter],
        (Phase::Menu, B::East) => &[KeyCode::Escape],
        (Phase::Playing, B::South) => &[KeyCode::Space],
        (Phase::Playing, B::West | B::RightTrigger) => &[super::mobility::KEY],
        (Phase::Playing, B::North) => &[KeyCode::KeyC],
        (Phase::Playing, B::Start) | (Phase::Paused, B::Start | B::East) => &[KeyCode::Escape],
        // The cards as the face buttons sit: left, top, right.
        (Phase::LevelUp, B::West) => &[KeyCode::Digit1],
        (Phase::LevelUp, B::North) => &[KeyCode::Digit2],
        (Phase::LevelUp, B::East) => &[KeyCode::Digit3],
        _ => &[],
    }
}

fn dead_zone(v: Vec2) -> Vec2 {
    let len = v.length();
    if len < DEAD {
        Vec2::ZERO
    } else {
        v / len * ((len - DEAD) / (1.0 - DEAD)).min(1.0)
    }
}

/// Read the first pad, after the keyboard so its presses land on this frame's input.
pub(super) fn read_pad(
    gamepads: Query<&Gamepad>,
    phase: Res<State<Phase>>,
    mut pad: ResMut<Pad>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut cursor: MessageReader<CursorMoved>,
    mut held: Local<Vec<(GamepadButton, KeyCode)>>,
    mut repeat: Local<(Vec2, f32)>,
    time: Res<Time<Real>>,
) {
    if cursor.read().count() > 0 {
        pad.active = false;
    }
    pad.nav = Vec2::ZERO;
    let Some(gamepad) = gamepads.iter().next() else {
        pad.left = Vec2::ZERO;
        pad.right = Vec2::ZERO;
        pad.confirm = false;
        for (_, key) in held.drain(..) {
            keys.release(key);
        }
        return;
    };
    pad.left = dead_zone(gamepad.left_stick());
    pad.right = dead_zone(gamepad.right_stick());
    // Release what this pad pressed, on the screen it was pressed on.
    held.retain(|&(button, key)| {
        let down = gamepad.pressed(button);
        if !down {
            keys.release(key);
        }
        down
    });
    let mut used = pad.left != Vec2::ZERO || pad.right != Vec2::ZERO;
    pad.confirm = gamepad.pressed(GamepadButton::South);
    // The D-pad, or the left stick pushed well over, as one of four steps, repeating while held.
    use GamepadButton as B;
    let mut dir = Vec2::ZERO;
    for (button, d) in [
        (B::DPadLeft, Vec2::NEG_X),
        (B::DPadRight, Vec2::X),
        (B::DPadUp, Vec2::NEG_Y),
        (B::DPadDown, Vec2::Y),
    ] {
        if gamepad.pressed(button) {
            dir = d;
        }
    }
    let stick = gamepad.left_stick();
    if dir == Vec2::ZERO && stick.length() > 0.6 {
        dir = if stick.x.abs() > stick.y.abs() {
            Vec2::new(stick.x.signum(), 0.0)
        } else {
            Vec2::new(0.0, -stick.y.signum())
        };
    }
    let (last, wait) = &mut *repeat;
    if dir == Vec2::ZERO {
        *last = Vec2::ZERO;
    } else if dir != *last {
        *last = dir;
        *wait = REPEAT_FIRST;
        pad.nav = dir;
    } else {
        *wait -= time.delta_secs();
        if *wait <= 0.0 {
            *wait = REPEAT;
            pad.nav = dir;
        }
    }
    for &button in gamepad.get_just_pressed() {
        used = true;
        for &key in keys_for(*phase.get(), button) {
            keys.press(key);
            held.push((button, key));
        }
    }
    if used && !pad.active {
        pad.active = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dead_zone_swallows_drift_and_still_reaches_full_tilt() {
        assert_eq!(dead_zone(Vec2::new(0.1, -0.1)), Vec2::ZERO);
        assert!((dead_zone(Vec2::X).length() - 1.0).abs() < 1e-6);
        let half = dead_zone(Vec2::new(0.0, 0.6));
        assert!(half.x == 0.0 && half.y > 0.0 && half.y < 1.0);
    }

    #[test]
    fn a_step_lands_ahead_and_nearest_the_line() {
        let e = |i| Entity::from_raw_u32(i).unwrap();
        let spots = [
            (e(1), Vec2::new(0.0, 0.0)),
            (e(2), Vec2::new(100.0, 0.0)),
            (e(3), Vec2::new(60.0, 80.0)),
            (e(4), Vec2::new(-100.0, 0.0)),
        ];
        assert_eq!(step(spots[0], Vec2::X, &spots), Some(e(2)));
        assert_eq!(step(spots[0], Vec2::NEG_X, &spots), Some(e(4)));
        assert_eq!(step(spots[0], Vec2::Y, &spots), Some(e(3)));
        assert_eq!(step(spots[0], Vec2::NEG_Y, &spots), None);
    }

    #[test]
    fn a_button_presses_what_its_screen_reads() {
        use GamepadButton as B;
        assert_eq!(keys_for(Phase::Playing, B::South), &[KeyCode::Space]);
        assert_eq!(keys_for(Phase::Menu, B::Start), &[KeyCode::Enter]);
        // A clicks the focus there instead.
        assert!(keys_for(Phase::Menu, B::South).is_empty());
        assert_eq!(keys_for(Phase::LevelUp, B::North), &[KeyCode::Digit2]);
        assert_eq!(keys_for(Phase::Paused, B::East), &[KeyCode::Escape]);
        // The movement spell is not cast from the cards.
        assert!(keys_for(Phase::LevelUp, B::RightTrigger).is_empty());
    }
}
