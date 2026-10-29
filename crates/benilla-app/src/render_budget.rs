//! The world's pixel budget on a large Retina screen, in Survivors and Classic alike. The world is
//! fill-bound at 5K (a Survivors crowd runs about 35 fps on an M4 at 5120×2880, 50–90 at half the
//! axes), so while `renderScale` is left at 1 the 3D renders at no more than [`BUDGET`] pixels and
//! MetalFX scales it up (`benilla_world::metalfx`); the interface stays native. Any other
//! `renderScale` is the player's and is kept.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::world_backdrop::RenderScale;

/// 2560×1600 physical: a 13-inch MacBook's panel stays native, a 5K one renders at half its axes.
const BUDGET: f32 = 2560.0 * 1600.0;

/// The scale snaps down to twentieths, so a window drag rebuilds the world's target only now and
/// then.
const STEPS: f32 = 20.0;

/// Above this, the saving is not worth the resample: native.
const NATIVE_ABOVE: f32 = 0.9;

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(Update, apply);
}

/// The scale for a window of `pixels` physical pixels.
fn budget_scale(pixels: f32) -> f32 {
    let s = (BUDGET / pixels.max(1.0)).sqrt();
    if s > NATIVE_ABOVE {
        1.0
    } else {
        // The nudge keeps an exact step (0.5 at 5K) from flooring a step lower on rounding.
        ((s * STEPS + 1e-3).floor() / STEPS).max(0.25)
    }
}

/// `auto` is the scale this system last set: while `renderScale` holds 1 or that value, it is
/// ours to set; a cvar change to anything else is the player's choice.
fn apply(
    window: Query<&Window, With<PrimaryWindow>>,
    mut scale: ResMut<RenderScale>,
    mut auto: Local<Option<f32>>,
) {
    let Ok(window) = window.single() else {
        return;
    };
    if scale.0 != 1.0 && Some(scale.0) != *auto {
        *auto = None;
        return;
    }
    let pixels = window.physical_width() as f32 * window.physical_height() as f32;
    let want = budget_scale(pixels);
    if scale.0 != want {
        info!(
            "render budget: world renders at {:.0}% of the {}x{} window (pixel budget)",
            want * 100.0,
            window.physical_width(),
            window.physical_height()
        );
        scale.0 = want;
    }
    *auto = Some(want);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_5k_screen_renders_at_half_its_axes() {
        assert_eq!(budget_scale(5120.0 * 2880.0), 0.5);
    }

    #[test]
    fn a_laptop_panel_or_smaller_stays_native() {
        assert_eq!(budget_scale(2560.0 * 1664.0), 1.0);
        assert_eq!(budget_scale(1920.0 * 1080.0), 1.0);
    }

    #[test]
    fn a_4k_screen_snaps_to_a_step() {
        assert_eq!(budget_scale(3840.0 * 2160.0), 14.0 / 20.0);
    }
}
