//! Controllers. A pad's buttons press the keys the mode already reads, mapped per screen, so
//! every handler serves both; its sticks move the hero and swing the free camera. While the pad
//! is in use the cursor stops aiming: the hero faces where he runs and the class movement spell
//! goes straight ahead. Moving the mouse hands control back.
//!
//! The pick screen, the pause menu's buttons and Settings stay mouse-driven, as the glue screens
//! are; a pad still starts the run, pauses, picks the cards and plays again.

use bevy::input::gamepad::{Gamepad, GamepadButton, GamepadConnection, GamepadConnectionEvent};
use bevy::prelude::*;
use bevy::window::CursorMoved;

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
        (Phase::Menu | Phase::GameOver, B::South | B::Start) => &[KeyCode::Enter],
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
) {
    if cursor.read().count() > 0 {
        pad.active = false;
    }
    let Some(gamepad) = gamepads.iter().next() else {
        pad.left = Vec2::ZERO;
        pad.right = Vec2::ZERO;
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
    fn a_button_presses_what_its_screen_reads() {
        use GamepadButton as B;
        assert_eq!(keys_for(Phase::Playing, B::South), &[KeyCode::Space]);
        assert_eq!(keys_for(Phase::Menu, B::South), &[KeyCode::Enter]);
        assert_eq!(keys_for(Phase::LevelUp, B::North), &[KeyCode::Digit2]);
        assert_eq!(keys_for(Phase::Paused, B::East), &[KeyCode::Escape]);
        // The movement spell is not cast from the cards.
        assert!(keys_for(Phase::LevelUp, B::RightTrigger).is_empty());
    }
}
