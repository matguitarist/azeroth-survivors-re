//! What outlasts a run: each zone's record, and from it which zones are open. The starting zones
//! are open from the first boot; every other zone opens once the zone before it in the menu's
//! order (by level) has been won, the first of them once any starting zone has.
//!
//! Saved as `benilla-config/survivors-profile.txt` (`localStorage` on the page), one zone per
//! line, `name\twins\truns\tbest_secs\tbest_kills`, keyed by name so a reordered or extended
//! [`data::MAPS`] keeps every record.

use bevy::prelude::*;

use super::{data, Run, Selection};

/// A zone whose weakest creature is at most this level is a starting zone.
const STARTER_LEVEL: u32 = 6;

#[derive(Clone, Copy, Default)]
pub(super) struct ZoneRecord {
    pub wins: u32,
    pub runs: u32,
    pub best_secs: u32,
    pub best_kills: u32,
}

#[derive(Resource)]
pub(super) struct Profile {
    /// Indexed like [`data::MAPS`].
    pub zones: Vec<ZoneRecord>,
    /// Bumped on every change, so the menu rebuilds once.
    pub version: u32,
}

impl Default for Profile {
    fn default() -> Self {
        let mut p = Self {
            zones: vec![ZoneRecord::default(); data::MAPS.len()],
            version: 0,
        };
        if let Some(text) = path().and_then(|p| crate::local_state::read_to_string(&p).ok()) {
            p.parse(&text);
        }
        p
    }
}

fn path() -> Option<std::path::PathBuf> {
    crate::local_state::home().map(|h| h.join("survivors-profile.txt"))
}

impl Profile {
    fn parse(&mut self, text: &str) {
        for line in text.lines() {
            let mut f = line.split('\t');
            let Some(name) = f.next() else { continue };
            let Some(i) = data::MAPS.iter().position(|m| m.name == name) else {
                continue;
            };
            let mut n = || f.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            self.zones[i] = ZoneRecord {
                wins: n(),
                runs: n(),
                best_secs: n(),
                best_kills: n(),
            };
        }
    }

    fn save(&self) {
        let text: String = data::MAPS
            .iter()
            .zip(&self.zones)
            .filter(|(_, r)| r.runs > 0)
            .map(|(m, r)| {
                format!(
                    "{}\t{}\t{}\t{}\t{}\n",
                    m.name, r.wins, r.runs, r.best_secs, r.best_kills
                )
            })
            .collect();
        if let Some(path) = path() {
            let _ = crate::local_state::write_atomic(&path, &text);
        }
    }

    pub fn won(&self, map: usize) -> bool {
        self.zones.get(map).is_some_and(|r| r.wins > 0)
    }

    /// `None` when `map` is open, else the zone whose win opens it (`None` inside: any starting
    /// zone).
    pub fn lock(&self, map: usize) -> Option<Option<usize>> {
        let order = zone_order();
        let pos = order.iter().position(|z| z.map == map)?;
        if order[pos].starter {
            return None;
        }
        let prev = &order[pos - 1];
        let open = if prev.starter {
            order.iter().any(|z| z.starter && self.won(z.map))
        } else {
            self.won(prev.map)
        };
        (!open).then_some((!prev.starter).then_some(prev.map))
    }

    pub fn is_open(&self, map: usize) -> bool {
        self.lock(map).is_none()
    }

    /// Record a finished run; the zone it opened, if it opened one.
    pub fn record(&mut self, map: usize, run: &Run) -> Option<usize> {
        let before: Vec<bool> = (0..data::MAPS.len()).map(|m| self.is_open(m)).collect();
        let r = &mut self.zones[map];
        r.runs += 1;
        r.wins += u32::from(run.won);
        r.best_secs = r.best_secs.max(run.elapsed as u32);
        r.best_kills = r.best_kills.max(run.kills);
        self.version += 1;
        self.save();
        (0..data::MAPS.len()).find(|&m| !before[m] && self.is_open(m))
    }
}

/// A zone in the menu's order.
pub(super) struct Zone {
    pub map: usize,
    /// The weakest and strongest creature's level.
    pub lo: u32,
    pub hi: u32,
    pub starter: bool,
}

/// Every zone, by its weakest creature's level, then name: the menu's rows and the unlock chain.
pub(super) fn zone_order() -> &'static [Zone] {
    static ORDER: std::sync::LazyLock<Vec<Zone>> = std::sync::LazyLock::new(|| {
        let mut order: Vec<Zone> = (0..data::MAPS.len())
            .map(|map| {
                let levels = data::roster(map).enemies.iter().map(|e| e.level);
                let lo = levels.clone().min().unwrap_or(1);
                let hi = levels.max().unwrap_or(lo);
                Zone {
                    map,
                    lo,
                    hi,
                    starter: lo <= STARTER_LEVEL,
                }
            })
            .collect();
        order.sort_by_key(|z| (z.lo, data::MAPS[z.map].name));
        order
    });
    &ORDER
}

/// At the results: count the run toward its zone, and say so if it opened the next.
pub(super) fn record_run(
    mut profile: ResMut<Profile>,
    mut run: ResMut<Run>,
    selection: Res<Selection>,
) {
    let opened = profile.record(selection.stage, &run);
    run.opened = opened;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(p: &mut Profile, map: usize) -> Option<usize> {
        let run = Run {
            won: true,
            elapsed: data::VICTORY_SECS,
            ..Run::default()
        };
        p.record(map, &run)
    }

    #[test]
    fn starters_open_and_each_win_opens_the_next() {
        let mut p = Profile::default();
        let order = zone_order();
        let starters: Vec<usize> = order.iter().filter(|z| z.starter).map(|z| z.map).collect();
        let rest: Vec<usize> = order.iter().filter(|z| !z.starter).map(|z| z.map).collect();
        assert!(!starters.is_empty() && rest.len() >= 2);
        assert!(starters.iter().all(|&m| p.is_open(m)));
        assert_eq!(p.lock(rest[0]), Some(None));
        assert_eq!(p.lock(rest[1]), Some(Some(rest[0])));

        // A loss opens nothing; a starting zone's win opens the first of the rest.
        assert_eq!(p.record(starters[0], &Run::default()), None);
        assert_eq!(win(&mut p, starters[0]), Some(rest[0]));
        assert!(!p.is_open(rest[1]));
        assert_eq!(win(&mut p, rest[0]), Some(rest[1]));
        assert_eq!(win(&mut p, rest[0]), None);
    }

    #[test]
    fn records_round_trip_by_name() {
        let mut p = Profile::default();
        win(&mut p, 3);
        let text: String = data::MAPS
            .iter()
            .zip(&p.zones)
            .filter(|(_, r)| r.runs > 0)
            .map(|(m, r)| {
                format!(
                    "{}\t{}\t{}\t{}\t{}\n",
                    m.name, r.wins, r.runs, r.best_secs, r.best_kills
                )
            })
            .collect();
        let mut q = Profile::default();
        q.parse(&format!("No Such Zone\t9\t9\t9\t9\n{text}"));
        assert_eq!(q.zones[3].wins, 1);
        assert_eq!(q.zones[3].best_secs, data::VICTORY_SECS as u32);
        assert_eq!(q.zones.iter().map(|r| r.runs).sum::<u32>(), 1);
    }
}
