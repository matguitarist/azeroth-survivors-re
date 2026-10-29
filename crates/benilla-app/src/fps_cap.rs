//! A frame-rate cap, in Survivors and Classic alike: `maxFPS` while the window has focus and
//! `maxFPSBk` while it does not, 0 for none, as the later clients spell them. With vertical sync
//! off it stops tearing-free speed from burning the battery on frames the display never shows; in
//! the background it keeps the game from drawing at full rate behind other work.
//!
//! The main app sleeps at the end of each frame until the frame's slot, so the pipelined render
//! world follows at the same pace. The sleep ends a little early and spins the rest: a plain sleep
//! overshoots by up to a millisecond on macOS, which at 120 fps is an eighth of the frame.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Fewer than this is no cap a player means: 0 and anything below are "none".
const MIN_FPS: f32 = 10.0;

/// How much of the wait is spun rather than slept.
const SPIN: Duration = Duration::from_micros(700);

#[derive(Resource, Debug, PartialEq)]
pub(crate) struct FpsCap {
    pub(crate) focused: f32,
    pub(crate) background: f32,
}

impl Default for FpsCap {
    fn default() -> Self {
        Self {
            focused: 0.0,
            background: 30.0,
        }
    }
}

/// Whether this is an instrumented run, read once.
struct BackgroundRun(bool);

impl Default for BackgroundRun {
    fn default() -> Self {
        Self(benilla_world::bgwin::background_run())
    }
}

/// The end of the last frame's slot.
#[derive(Default)]
struct Pace(Option<Instant>);

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<FpsCap>()
        .add_observer(on_cvar)
        .add_systems(Last, pace);
}

fn on_cvar(ev: On<crate::cvars::CvarChanged>, mut cap: ResMut<FpsCap>) {
    if ev.is("maxFPS") {
        cap.focused = ev.num();
    } else if ev.is("maxFPSBk") {
        cap.background = ev.num();
    }
}

/// The frame interval for `fps`, `None` for no cap.
fn interval(fps: f32) -> Option<Duration> {
    (fps >= MIN_FPS).then(|| Duration::from_secs_f64(1.0 / f64::from(fps)))
}

/// The next slot: one interval after the last, but never in the past, so a slow frame does not
/// earn a burst of fast ones after it.
fn next_slot(last: Option<Instant>, every: Duration, now: Instant) -> Instant {
    match last {
        Some(last) if last + every > now => last + every,
        _ => now,
    }
}

fn pace(
    window: Query<&Window, With<PrimaryWindow>>,
    cap: Res<FpsCap>,
    mut slot: Local<Pace>,
    background: Local<BackgroundRun>,
) {
    // An instrumented run opens unfocused behind your work (`bgwin`) and measures at full rate.
    let focused = window.single().map(|w| w.focused).unwrap_or(true) || background.0;
    let Some(every) = interval(if focused { cap.focused } else { cap.background }) else {
        slot.0 = None;
        return;
    };
    let now = Instant::now();
    let due = next_slot(slot.0, every, now);
    if let Some(wait) = due.checked_duration_since(now) {
        if wait > SPIN {
            std::thread::sleep(wait - SPIN);
        }
        while Instant::now() < due {
            std::hint::spin_loop();
        }
    }
    slot.0 = Some(due);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_and_tiny_caps_are_none() {
        assert_eq!(interval(0.0), None);
        assert_eq!(interval(5.0), None);
        assert_eq!(interval(60.0), Some(Duration::from_secs_f64(1.0 / 60.0)));
    }

    #[test]
    fn a_slow_frame_earns_no_burst() {
        let t0 = Instant::now();
        let every = Duration::from_millis(10);
        assert_eq!(
            next_slot(None, every, t0),
            t0,
            "the first frame waits for nothing"
        );
        assert_eq!(
            next_slot(Some(t0), every, t0 + Duration::from_millis(3)),
            t0 + every
        );
        let late = t0 + Duration::from_millis(25);
        assert_eq!(
            next_slot(Some(t0), every, late),
            late,
            "behind: go now, not three times fast"
        );
    }
}
