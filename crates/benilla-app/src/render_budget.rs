//! The world's pixel budget on a large Retina screen, in Survivors and Classic alike. The world is
//! fill-bound at 5K (a Survivors crowd runs about 35 fps on an M4 at 5120×2880, 50–90 at half the
//! axes), so while `renderScale` is left at 1 the 3D renders at no more than [`BUDGET`] pixels and
//! MetalFX scales it up (`benilla_world::metalfx`); the interface stays native. Any other
//! `renderScale` is the player's and is kept.
//!
//! Under the budget, dynamic resolution (`dynamicResolution`, on by default) follows the GPU: the
//! whole-frame GPU time (`perf::gpu_meter`) against the frame's target, the `maxFPS` cap, else the
//! display's refresh under vertical sync, else 60. Over budget the world's share of its pixels
//! drops at once to what fits; with room to spare it climbs back a step at a time. Only GPU time
//! moves it, so a scene the CPU holds back keeps its resolution.

use std::sync::atomic::Ordering;

use bevy::prelude::*;
use bevy::window::{Monitor, PresentMode, PrimaryMonitor, PrimaryWindow};

use crate::world_backdrop::RenderScale;

/// 2560×1600 physical: a 13-inch MacBook's panel stays native, a 5K one renders at half its axes.
const BUDGET: f32 = 2560.0 * 1600.0;

/// The scale snaps down to twentieths, so a window drag rebuilds the world's target only now and
/// then.
const STEPS: f32 = 20.0;

/// Above this, the saving is not worth the resample: native.
const NATIVE_ABOVE: f32 = 0.9;

/// The least share of the budget dynamic resolution goes to, and the least scale overall.
const MIN_FACTOR: f32 = 0.5;
const MIN_SCALE: f32 = 0.35;

/// GPU time over this share of the frame lowers the scale; under the second, it may climb.
const OVER: f32 = 0.9;
const UNDER: f32 = 0.6;

/// Seconds of frames each judgment takes, and calm judgments in a row before a climb.
const WINDOW: f32 = 1.0;
const CALM_TO_CLIMB: u32 = 3;

/// Seconds after a change before frames count again.
const SETTLE: f32 = 0.5;

/// A drop that saves less than this share of the GPU time is undone: what costs is not the
/// world's pixels (the interface, the upscale), and fewer of them would only blur it.
const MIN_SAVING: f32 = 0.15;
/// Judgments after a futile drop before another is tried, doubled by each futile drop in a row up
/// to the cap, so a scene where resolution buys nothing is left alone.
const FUTILE_HOLD: u32 = 30;
const FUTILE_HOLD_MAX: u32 = 600;

/// Dynamic resolution's state: `factor` scales the pixel budget's scale, 1 for none.
#[derive(Resource)]
pub(crate) struct Dynamic {
    pub(crate) on: bool,
    pub(crate) factor: f32,
    samples: Vec<f32>,
    since: f32,
    judge: Judge,
}

/// The controller proper, one judgment per window of frames.
#[derive(Default)]
struct Judge {
    /// Calm windows in a row.
    calm: u32,
    /// The factor and the GPU time before the last drop, checked by the next judgment.
    dropped_from: Option<(f32, f32)>,
    /// Judgments left before a drop may be tried again.
    hold: u32,
    /// Futile drops in a row.
    futile: u32,
}

impl Default for Dynamic {
    fn default() -> Self {
        Self {
            on: true,
            factor: 1.0,
            samples: Vec::new(),
            since: 0.0,
            judge: Judge::default(),
        }
    }
}

pub(crate) fn plugin(app: &mut App) {
    crate::perf::gpu_meter(app);
    app.init_resource::<Dynamic>()
        .add_observer(on_cvar)
        .add_systems(Update, (steer, apply).chain());
}

fn on_cvar(ev: On<crate::cvars::CvarChanged>, mut dynamic: ResMut<Dynamic>) {
    if ev.is("dynamicResolution") {
        dynamic.on = ev.flag();
        if !dynamic.on {
            dynamic.factor = 1.0;
        }
    }
}

impl Judge {
    /// The next factor after a window whose median GPU frame took `gpu_ms` of a `budget_ms` frame.
    fn next(&mut self, factor: f32, gpu_ms: f32, budget_ms: f32) -> f32 {
        self.hold = self.hold.saturating_sub(1);
        if let Some((before, before_ms)) = self.dropped_from.take() {
            if gpu_ms > before_ms * (1.0 - MIN_SAVING) {
                self.hold = (FUTILE_HOLD << self.futile.min(5)).min(FUTILE_HOLD_MAX);
                self.futile += 1;
                self.calm = 0;
                return before;
            }
            self.futile = 0;
        }
        if gpu_ms > OVER * budget_ms {
            self.calm = 0;
            if self.hold > 0 || factor <= MIN_FACTOR {
                return factor;
            }
            // Pixels go as the square of the scale; aim a little under the line.
            let fit = factor * (0.8 * budget_ms / gpu_ms).sqrt();
            let next = ((fit * STEPS).floor() / STEPS).clamp(MIN_FACTOR, 1.0);
            if next < factor {
                self.dropped_from = Some((factor, gpu_ms));
            }
            next
        } else if gpu_ms < UNDER * budget_ms && factor < 1.0 {
            self.calm += 1;
            if self.calm < CALM_TO_CLIMB {
                return factor;
            }
            self.calm = 0;
            (((factor * STEPS).round() + 1.0) / STEPS).min(1.0)
        } else {
            self.calm = 0;
            factor
        }
    }
}

/// The frame time dynamic resolution holds to, in milliseconds.
fn target_ms(cap: f32, vsync: bool, refresh_hz: Option<f32>) -> f32 {
    let fps = if cap >= 10.0 {
        cap
    } else if vsync {
        refresh_hz.filter(|&hz| hz >= 30.0).unwrap_or(60.0)
    } else {
        60.0
    };
    1000.0 / fps
}

fn steer(
    time: Res<Time<Real>>,
    gpu: Option<Res<crate::perf::GpuMsShared>>,
    cap: Option<Res<crate::fps_cap::FpsCap>>,
    window: Query<&Window, With<PrimaryWindow>>,
    monitor: Query<&Monitor, With<PrimaryMonitor>>,
    mut dynamic: ResMut<Dynamic>,
) {
    let (Some(gpu), Ok(window)) = (gpu, window.single()) else {
        return;
    };
    if !dynamic.on {
        return;
    }
    let ns = gpu.0.load(Ordering::Relaxed);
    dynamic.since += time.delta_secs();
    if ns > 0 && dynamic.since >= 0.0 {
        dynamic.samples.push(ns as f32 / 1e6);
    }
    if dynamic.since < WINDOW || dynamic.samples.len() < 10 {
        return;
    }
    dynamic.since = 0.0;
    let mut samples = std::mem::take(&mut dynamic.samples);
    samples.sort_by(f32::total_cmp);
    let median = samples[samples.len() / 2];
    let vsync = matches!(
        window.present_mode,
        PresentMode::AutoVsync | PresentMode::Fifo | PresentMode::FifoRelaxed
    );
    let refresh = monitor
        .single()
        .ok()
        .and_then(|m| m.refresh_rate_millihertz)
        .map(|mhz| mhz as f32 / 1000.0);
    let budget = target_ms(cap.map_or(0.0, |c| c.focused), vsync, refresh);
    let Dynamic { factor, judge, .. } = &mut *dynamic;
    let next = judge.next(*factor, median, budget);
    if next != *factor {
        info!(
            "dynamic resolution: gpu {median:.1} ms of a {budget:.1} ms frame, share {:.0}% -> {:.0}%",
            *factor * 100.0,
            next * 100.0
        );
        *factor = next;
        // The world's target is rebuilt at the new size; its first frames are no measure.
        dynamic.since = -SETTLE;
    }
    samples.clear();
    dynamic.samples = samples;
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
    dynamic: Res<Dynamic>,
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
    let budget = budget_scale(pixels);
    let want = if dynamic.factor < 1.0 {
        (((budget * dynamic.factor) * STEPS + 1e-3).floor() / STEPS).max(MIN_SCALE)
    } else {
        budget
    };
    if scale.0 != want && want == budget {
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
    fn an_overloaded_gpu_drops_to_fit_and_climbs_back_slowly() {
        let mut j = Judge::default();
        // Twice the 16.7 ms budget: the share drops to about 0.63 of the axes.
        let f = j.next(1.0, 33.3, 16.7);
        assert_eq!(f, 0.6);
        assert_eq!(
            j.next(f, 14.0, 16.7),
            f,
            "it paid, and is inside the band: held"
        );
        for _ in 0..CALM_TO_CLIMB - 1 {
            assert_eq!(j.next(f, 5.0, 16.7), f, "calm, not yet");
        }
        assert_eq!(j.next(f, 5.0, 16.7), 0.65, "then one step");
    }

    #[test]
    fn a_drop_that_saves_nothing_is_undone_and_not_retried_soon() {
        let mut j = Judge::default();
        let f = j.next(1.0, 10.0, 8.3);
        assert!(f < 1.0);
        assert_eq!(j.next(f, 9.8, 8.3), 1.0, "no saving: back to full");
        for _ in 0..FUTILE_HOLD - 1 {
            assert_eq!(j.next(1.0, 10.0, 8.3), 1.0, "held off");
        }
        let f = j.next(1.0, 10.0, 8.3);
        assert!(f < 1.0, "then tried again");
        assert_eq!(j.next(f, 9.9, 8.3), 1.0);
        assert_eq!(
            j.hold,
            2 * FUTILE_HOLD,
            "a second futile drop waits twice as long"
        );
    }

    #[test]
    fn the_target_is_the_cap_else_the_display_else_60() {
        assert_eq!(target_ms(120.0, true, Some(60.0)), 1000.0 / 120.0);
        assert_eq!(target_ms(0.0, true, Some(120.0)), 1000.0 / 120.0);
        assert_eq!(target_ms(0.0, false, Some(120.0)), 1000.0 / 60.0);
    }

    #[test]
    fn a_4k_screen_snaps_to_a_step() {
        assert_eq!(budget_scale(3840.0 * 2160.0), 14.0 / 20.0);
    }
}
