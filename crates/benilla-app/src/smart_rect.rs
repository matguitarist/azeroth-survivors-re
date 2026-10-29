//! SmartScreenRect, the client's per-frame anti-overlap placement solver
//! (`UIUtil\SmartScreenRect`). Each rect is relocated off the rects already claimed this frame in
//! its bucket, then claimed. 5875 has two buckets that never interact: nameplates (0) and combat
//! worldtext (1). No state crosses frames, so a pushed rect snaps back when its blocker goes.
//!
//! Our viewport is Y-down logical pixels; the reference's gx device space is Y-up, and every clamp
//! is symmetric, so the mirror is exact.
//! - Solve (`0x5097a0`), a bounded BFS: the seed is the input clamped to the bands (`0x509bf0`
//!   mode 0), whose region (`0x509d80`) fixes the four-direction [`ORDER`]. A push against the
//!   first strictly overlapping claimed rect (`0x509220`) lands edge-to-edge; an unmoved try is
//!   adopted at once, a moved one is queued. Dequeues are capped at
//!   `(floor(W/w)+1)(floor(H/h)+1)` (`0x509dd0`); exhaustion returns the input, drawn overlapping.
//! - Seat tail (`0x509520`, `0x509ec0`): clamp center-X and the top edge half-extent inside,
//!   rebuild top-anchored, normalize (`0x509e20`), claim (`0x509660`).
//! - The node-strategy skip at `0x509874` is dead in 5875 and `showsmartrects` has no readers, so
//!   neither is ported.

use std::collections::VecDeque;

use bevy::math::{Rect, Vec2};

/// The near-top band (`[0x8087cc]`), in gx units, where one unit is the screen diagonal.
const DDC_NEAR_TOP: f32 = 0.0375;
/// The near-bottom band (`[0x8087d0]`); the X bands (`[0x8087d4]`, `[0x8087d8]`) are 0.0.
const DDC_NEAR_BOTTOM: f32 = 0.01875;

/// The push order per region (`0x8087e0`): 0 up, 1 left, 2 right, 3 down (`0x808870` slots).
const ORDER: [[u8; 4]; 9] = [
    [0, 2, 3, 1], // 0 center:                UP RIGHT DOWN LEFT
    [1, 0, 2, 0], // 1 near bottom:           LEFT UP RIGHT UP
    [1, 3, 2, 3], // 2 near top:              LEFT DOWN RIGHT DOWN
    [0, 1, 3, 1], // 3 off right:             UP LEFT DOWN LEFT
    [0, 2, 3, 2], // 4 off left:              UP RIGHT DOWN RIGHT
    [0, 1, 0, 1], // 5 off right + near bottom
    [0, 2, 0, 2], // 6 off left  + near bottom
    [3, 1, 3, 1], // 7 off right + near top
    [3, 2, 3, 2], // 8 off left  + near top
];

/// One claim bucket, the rects placed this frame; the first claimed never moves.
#[derive(Default)]
pub(crate) struct SmartBucket {
    claimed: Vec<Rect>,
    grid: Grid,
}

impl SmartBucket {
    /// The per-frame reset (`0x509500`).
    pub(crate) fn clear(&mut self) {
        self.claimed.clear();
        self.grid.clear();
    }

    /// The seat without the claim. Deviation: the plate caller snaps the result to device
    /// pixels before [`Self::claim`], so its border art blits 1:1.
    pub(crate) fn resolve(&self, desired: Rect, viewport: Vec2) -> Rect {
        let (w, h) = (desired.width(), desired.height());
        let (half_w, half_h) = (w * 0.5, h * 0.5);
        let input = normalize(desired, viewport);
        let solved = self.solve(input, viewport);
        // `0x509720`'s center-X and top edge; `max()` keeps an over-wide rect's bounds ordered.
        let cx =
            ((solved.min.x + solved.max.x) * 0.5).clamp(half_w, (viewport.x - half_w).max(half_w));
        let top = solved
            .min
            .y
            .clamp(half_h, (viewport.y - half_h).max(half_h));
        normalize(Rect::new(cx - half_w, top, cx + half_w, top + h), viewport)
    }

    /// `0x509660`.
    pub(crate) fn claim(&mut self, rect: Rect) {
        self.grid.insert(rect, self.claimed.len() as u32);
        self.claimed.push(rect);
    }

    /// The first claimed rect strictly overlapping `r`, in claim order (`0x509220`'s scan).
    fn first_overlap(&self, r: Rect) -> Option<Rect> {
        self.grid
            .first(r, |i| overlaps(r, self.claimed[i as usize]))
            .map(|i| self.claimed[i as usize])
    }

    /// The bounded BFS (`0x5097a0`).
    fn solve(&self, input: Rect, viewport: Vec2) -> Rect {
        let (seed, flags) = clamp_to_bands(input, viewport);
        let region = region_index(flags);
        // `0x509dd0`. A degenerate size saturates, but never overlaps, so adopts at once.
        let bound = ((viewport.x / input.width()).trunc() as i64 + 1)
            .saturating_mul((viewport.y / input.height()).trunc() as i64 + 1);
        let mut queue = VecDeque::from([seed]);
        for _ in 0..bound {
            let Some(node) = queue.pop_front() else {
                return input; // every branch went off-screen
            };
            // The per-try skip (`0x509bf0` mode 1) tests the node, so it is hoisted.
            if band_flags(node, viewport) != 0 {
                continue;
            }
            // Every try of a node pushes off the same blocker, so it is looked up once.
            let blocker = self.first_overlap(node);
            for strategy in ORDER[region as usize] {
                let candidate = push(blocker, node, strategy);
                if candidate == node {
                    return candidate; // unmoved: overlaps nothing
                }
                queue.push_back(candidate);
            }
        }
        input
    }
}

/// The on-screen clamp (`0x509e20`), physical bottom, top, left, right, so an oversize rect pins
/// top and right.
fn normalize(r: Rect, viewport: Vec2) -> Rect {
    let mut r = r;
    if r.max.y > viewport.y {
        let d = r.max.y - viewport.y;
        r.min.y -= d;
        r.max.y -= d;
    }
    if r.min.y < 0.0 {
        let d = -r.min.y;
        r.min.y += d;
        r.max.y += d;
    }
    if r.min.x < 0.0 {
        let d = -r.min.x;
        r.min.x += d;
        r.max.x += d;
    }
    if r.max.x > viewport.x {
        let d = r.max.x - viewport.x;
        r.min.x -= d;
        r.max.x -= d;
    }
    r
}

/// The region flags (`0x509bf0`): bit 0 off left, 1 off right, 2 near top, 3 near bottom.
fn band_flags(r: Rect, viewport: Vec2) -> u8 {
    let diag = viewport.length();
    let mut flags = 0;
    if r.min.x < 0.0 {
        flags |= 1;
    }
    if r.max.x > viewport.x {
        flags |= 2;
    }
    if r.min.y < DDC_NEAR_TOP * diag {
        flags |= 4;
    }
    if r.max.y > viewport.y - DDC_NEAR_BOTTOM * diag {
        flags |= 8;
    }
    flags
}

/// `0x509bf0` mode 0: the flags and the seed, each offending edge clamped to its band.
///
/// The edge is assigned the boundary and its opposite derived from the size, as the reference
/// does: translating by the gap can land one ULP short in `f32`, the seed then fails the strict
/// re-test in [`SmartBucket::solve`], and the plate pops to the seat clamp for a frame.
fn clamp_to_bands(r: Rect, viewport: Vec2) -> (Rect, u8) {
    let diag = viewport.length();
    let flags = band_flags(r, viewport);
    let (w, h) = (r.width(), r.height());
    let mut r = r;
    if flags & 1 != 0 {
        r.min.x = 0.0;
        r.max.x = w;
    }
    if flags & 2 != 0 {
        r.max.x = viewport.x;
        r.min.x = viewport.x - w;
    }
    if flags & 4 != 0 {
        r.min.y = DDC_NEAR_TOP * diag;
        r.max.y = r.min.y + h;
    }
    if flags & 8 != 0 {
        r.max.y = viewport.y - DDC_NEAR_BOTTOM * diag;
        r.min.y = r.max.y - h;
    }
    (r, flags)
}

/// The 3x3 region (`0x509d80`), for all 16 flag words: bit 0 dominates and ignores bit 1, then
/// bit 2 ignoring bit 3, then bit 1, then bit 3.
fn region_index(flags: u8) -> u8 {
    if flags & 1 != 0 {
        if flags & 4 != 0 {
            8
        } else if flags & 8 != 0 {
            6
        } else {
            4
        }
    } else if flags & 4 != 0 {
        if flags & 2 != 0 {
            7
        } else {
            2
        }
    } else if flags & 2 != 0 {
        if flags & 8 != 0 {
            5
        } else {
            3
        }
    } else if flags & 8 != 0 {
        1
    } else {
        0
    }
}

/// One try (`0x808870`, `0x509220`): push edge-to-edge off the first strict overlap
/// (`0x5091c0`, `0x509300`, `0x509360`, `0x5093c0`), or return the node unchanged.
fn push(blocker: Option<Rect>, r: Rect, strategy: u8) -> Rect {
    let Some(c) = blocker else {
        return r;
    };
    let mut r = r;
    match strategy {
        0 => {
            // Up.
            let d = r.max.y - c.min.y;
            r.min.y -= d;
            r.max.y -= d;
        }
        1 => {
            let d = r.max.x - c.min.x;
            r.min.x -= d;
            r.max.x -= d;
        }
        2 => {
            let d = c.max.x - r.min.x;
            r.min.x += d;
            r.max.x += d;
        }
        _ => {
            // Down.
            let d = c.max.y - r.min.y;
            r.min.y += d;
            r.max.y += d;
        }
    }
    r
}

/// The strict overlap (`0x509220`): touching edges do not overlap.
fn overlaps(r: Rect, c: Rect) -> bool {
    r.max.x > c.min.x && r.min.x < c.max.x && r.max.y > c.min.y && r.min.y < c.max.y
}

/// Deviation, for speed only: the claims binned by [`CELL`]-pixel cell, so a try tests its
/// neighbours instead of every claim. A survivors fight claims hundreds of damage numbers a frame
/// and each solve tries up to thousands of nodes. Rects past the grid fall in its edge cells and
/// overlapping rects always share a cell, so the lowest claim index found is the linear scan's.
#[derive(Default)]
struct Grid {
    /// `SIDE * SIDE` cells of claim indices, ascending; allocated at the first claim.
    cells: Vec<Vec<u32>>,
    /// The cells holding claims, emptied by [`Grid::clear`].
    used: Vec<usize>,
}

const CELL: f32 = 64.0;
const SIDE: usize = 64;

impl Grid {
    fn clear(&mut self) {
        for &cell in &self.used {
            self.cells[cell].clear();
        }
        self.used.clear();
    }

    fn insert(&mut self, r: Rect, index: u32) {
        if self.cells.is_empty() {
            self.cells = vec![Vec::new(); SIDE * SIDE];
        }
        for cell in cells(r) {
            if self.cells[cell].is_empty() {
                self.used.push(cell);
            }
            self.cells[cell].push(index);
        }
    }

    /// The lowest index under `r`'s cells that `hit` accepts.
    fn first(&self, r: Rect, hit: impl Fn(u32) -> bool) -> Option<u32> {
        if self.used.is_empty() {
            return None;
        }
        let mut best: Option<u32> = None;
        for cell in cells(r) {
            // Ascending per cell: the first hit is the cell's lowest, and past `best` none helps.
            for &i in &self.cells[cell] {
                if best.is_some_and(|b| i >= b) {
                    break;
                }
                if hit(i) {
                    best = Some(i);
                    break;
                }
            }
        }
        best
    }
}

/// The cells `r` covers. Its hull is taken, so an inverted rect bins like the linear scan tests it,
/// and a NaN edge maps to cell 0 (a NaN rect overlaps nothing either way).
fn cells(r: Rect) -> impl Iterator<Item = usize> {
    let span = |a: f32, b: f32| {
        let at = |v: f32| (v / CELL).floor().clamp(0.0, (SIDE - 1) as f32) as usize;
        at(a.min(b))..=at(a.max(b))
    };
    let (xs, ys) = (span(r.min.x, r.max.x), span(r.min.y, r.max.y));
    ys.flat_map(move |y| xs.clone().map(move |x| y * SIDE + x))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VP: Vec2 = Vec2::new(1024.0, 768.0);

    fn seat(bucket: &mut SmartBucket, desired: Rect) -> Rect {
        let r = bucket.resolve(desired, VP);
        bucket.claim(r);
        r
    }

    /// A 73x18 plate at 640x360 swept toward the band: the seed clamp must land exactly on it.
    #[test]
    fn a_plate_in_the_near_top_band_never_pops_to_the_seat_clamp() {
        const VP2: Vec2 = Vec2::new(640.0, 360.0);
        let band = DDC_NEAR_TOP * VP2.length();
        for step in 0..2800 {
            #[allow(clippy::cast_precision_loss)]
            let top = step as f32 * 0.01;
            let desired = Rect::new(172.0, top, 245.0, top + 18.0);
            let got = SmartBucket::default().resolve(desired, VP2);
            let want = if top < band { band } else { top };
            assert!(
                (got.min.y - want).abs() < 1e-3,
                "a plate seeded at y={top} seats at {} — the band edge is {band} \
                 (a 19px pop means the seed skipped itself)",
                got.min.y
            );
        }
    }

    #[test]
    fn free_rect_stays_put() {
        let mut b = SmartBucket::default();
        let r = Rect::new(500.0, 380.0, 540.0, 400.0);
        assert_eq!(seat(&mut b, r), r);
    }

    #[test]
    fn center_pushes_up_abutting() {
        let mut b = SmartBucket::default();
        let r = Rect::new(500.0, 380.0, 540.0, 400.0);
        seat(&mut b, r);
        let pushed = seat(&mut b, r);
        assert_eq!(pushed.max.y, 380.0, "bottom abuts the blocker's top");
        assert_eq!(pushed.min.y, 360.0, "size preserved");
        assert_eq!((pushed.min.x, pushed.max.x), (500.0, 540.0), "x untouched");
    }

    /// The up child still overlaps the second rect while the right child is free.
    #[test]
    fn third_goes_right_not_twice_up() {
        let mut b = SmartBucket::default();
        let r = Rect::new(500.0, 380.0, 540.0, 400.0);
        seat(&mut b, r);
        seat(&mut b, r);
        let third = seat(&mut b, r);
        assert_eq!(third.min.x, 540.0, "left edge abuts the blocker's right");
        assert_eq!((third.min.y, third.max.y), (380.0, 400.0), "one push only");
    }

    #[test]
    fn near_top_pushes_left() {
        let mut b = SmartBucket::default();
        // The band at 1024x768 is 0.0375 x 1280 = 48 px.
        let r = Rect::new(500.0, 10.0, 540.0, 30.0);
        let first = seat(&mut b, r);
        let pushed = seat(&mut b, r);
        assert_eq!(
            pushed.max.x, first.min.x,
            "right edge abuts the blocker's left"
        );
        assert_eq!(
            (pushed.min.y, pushed.max.y),
            (first.min.y, first.max.y),
            "y untouched — never pushed further toward the top"
        );
    }

    #[test]
    fn seed_clamps_to_band() {
        let b = SmartBucket::default();
        let r = Rect::new(500.0, 10.0, 540.0, 30.0);
        let seated = b.resolve(r, VP);
        assert_eq!(seated.min.y, 48.0, "top edge on the 0.0375·diag band");
    }

    #[test]
    fn exhaustion_keeps_original() {
        let mut b = SmartBucket::default();
        let r = Rect::new(0.0, 0.0, 1024.0, 768.0);
        seat(&mut b, r);
        let second = b.resolve(r, VP);
        assert_eq!(second.width(), 1024.0);
        assert_eq!(second.height(), 768.0);
    }

    #[test]
    fn snaps_back_when_blocker_gone() {
        let mut b = SmartBucket::default();
        let r = Rect::new(500.0, 380.0, 540.0, 400.0);
        seat(&mut b, r);
        assert_ne!(seat(&mut b, r), r, "pushed while blocked");
        b.clear();
        assert_eq!(seat(&mut b, r), r, "back at the anchor next frame");
    }

    /// All 16 flag words of `0x509d80`.
    #[test]
    fn region_index_table() {
        let expected = [0, 4, 3, 4, 2, 8, 7, 8, 1, 6, 5, 6, 2, 8, 7, 8];
        for (flags, &region) in expected.iter().enumerate() {
            assert_eq!(region_index(flags as u8), region, "flags {flags:#06b}");
        }
    }

    #[test]
    fn touching_edges_do_not_overlap() {
        let mut b = SmartBucket::default();
        seat(&mut b, Rect::new(500.0, 380.0, 540.0, 400.0));
        let below = Rect::new(500.0, 400.0, 540.0, 420.0);
        assert_eq!(seat(&mut b, below), below);
    }

    /// The grid finds the linear scan's blocker, past the grid, inverted and degenerate included.
    #[test]
    fn grid_matches_the_linear_scan() {
        let mut seed = 0x9e37_79b9_u32;
        let mut next = move |lo: f32, hi: f32| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            #[allow(clippy::cast_precision_loss)]
            let unit = seed as f32 / u32::MAX as f32;
            lo + (hi - lo) * unit
        };
        let rect = |next: &mut dyn FnMut(f32, f32) -> f32| {
            let (x, y) = (next(-500.0, 5000.0), next(-500.0, 5000.0));
            Rect::from_corners(
                Vec2::new(x, y),
                Vec2::new(x + next(-20.0, 300.0), y + next(-20.0, 150.0)),
            )
        };
        let mut b = SmartBucket::default();
        for round in 0..20 {
            b.clear();
            for _ in 0..(round * 25) {
                b.claim(rect(&mut next));
            }
            b.claim(Rect::new(-1e30, -1e30, f32::INFINITY, f32::INFINITY));
            for _ in 0..500 {
                let r = rect(&mut next);
                let linear = b.claimed.iter().copied().find(|&c| overlaps(r, c));
                assert_eq!(b.first_overlap(r), linear, "round {round}, {r:?}");
            }
        }
    }
}
