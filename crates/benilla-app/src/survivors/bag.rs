//! The backpack and the coin purse. Creatures now and then drop coins and items, as in the
//! game: grey junk, potions, food and explosives to use, and rings, necks, cloaks and trinkets in
//! the five qualities, which the hero wears for the rest of the run. Coins and items lie on the
//! ground in a gold pile or a sack until the hero's pickup reach pulls them in.
//!
//! A run's coins, and everything still in the bag or worn when it ends, sold at the vendor's
//! price, go to the purse the profile keeps ([`super::profile`]). The bag lives in [`Run`], so a
//! new run starts with it empty.

use bevy::prelude::*;

use super::abilities::Hit;
use super::data::{Effect, Stat};
use super::enemies::{Enemy, Loot};
use super::hero::Hero;
use super::units::{self, flat, Fx, COLOR_SPELL};
use super::{Rng, Run, Stats};

/// The backpack's sixteen slots.
pub(super) const BAG_SLOTS: usize = 16;
/// The item bar's sockets: the first usable stacks in the bag, on keys 1 to 4.
pub(super) const QUICK_SLOTS: usize = 4;
/// The most of one usable item a slot holds.
const STACK: u32 = 10;

/// `GoldPilesmall01.mdx` and `Sack02.mdx`.
const COIN_DISPLAY: u32 = 2614;
const SACK_DISPLAY: u32 = 2890;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum Quality {
    Poor,
    Common,
    Uncommon,
    Rare,
    Epic,
}

impl Quality {
    /// `ITEM_QUALITY_COLORS`.
    pub(super) fn color(self) -> Color {
        match self {
            Quality::Poor => Color::srgb_u8(157, 157, 157),
            Quality::Common => Color::WHITE,
            Quality::Uncommon => Color::srgb_u8(30, 255, 0),
            Quality::Rare => Color::srgb_u8(0, 112, 221),
            Quality::Epic => Color::srgb_u8(163, 53, 238),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum GearSlot {
    Neck,
    Back,
    Finger,
    Trinket,
}

impl GearSlot {
    pub(super) fn name(self) -> &'static str {
        match self {
            GearSlot::Neck => "Neck",
            GearSlot::Back => "Back",
            GearSlot::Finger => "Finger",
            GearSlot::Trinket => "Trinket",
        }
    }

    /// The paper doll's empty-socket art; 1.12's back slot shows the chest's.
    pub(super) fn empty_art(self) -> &'static str {
        match self {
            GearSlot::Neck => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Neck",
            GearSlot::Back => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Chest",
            GearSlot::Finger => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Finger",
            GearSlot::Trinket => "Interface\\PaperDoll\\UI-PaperDoll-Slot-Trinket",
        }
    }
}

/// The worn sockets, in the backpack's order.
pub(super) const WORN: [GearSlot; 6] = [
    GearSlot::Neck,
    GearSlot::Back,
    GearSlot::Finger,
    GearSlot::Finger,
    GearSlot::Trinket,
    GearSlot::Trinket,
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Use {
    /// This fraction of the hero's health at once.
    Heal(f32),
    /// This fraction of the hero's health each second, for seconds.
    Food(f32, f32),
    /// Movement speed up by this fraction, for seconds.
    Swift(f32, f32),
    /// Thrown at the nearest creature, or set off at the hero's feet.
    Bomb {
        damage: f32,
        radius: f32,
        stun: f32,
        on_self: bool,
    },
}

/// A usable item: the spell whose visual plays, and the creature levels that drop it.
pub(crate) struct UseDef {
    pub key: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub quality: Quality,
    pub spell: u32,
    pub effect: Use,
    pub cooldown: f32,
    levels: (u32, u32),
}

static USES: &[UseDef] = &[
    UseDef {
        key: "minor_heal",
        name: "Minor Healing Potion",
        icon: "INV_Potion_49",
        quality: Quality::Common,
        spell: 439,
        effect: Use::Heal(0.25),
        cooldown: 15.0,
        levels: (0, 24),
    },
    UseDef {
        key: "heal",
        name: "Healing Potion",
        icon: "INV_Potion_51",
        quality: Quality::Common,
        spell: 441,
        effect: Use::Heal(0.4),
        cooldown: 15.0,
        levels: (18, 44),
    },
    UseDef {
        key: "greater_heal",
        name: "Greater Healing Potion",
        icon: "INV_Potion_54",
        quality: Quality::Common,
        spell: 2024,
        effect: Use::Heal(0.6),
        cooldown: 15.0,
        levels: (38, 99),
    },
    UseDef {
        key: "bread",
        name: "Freshly Baked Bread",
        icon: "INV_Misc_Food_11",
        quality: Quality::Common,
        spell: 433,
        effect: Use::Food(0.04, 12.0),
        cooldown: 20.0,
        levels: (0, 99),
    },
    UseDef {
        key: "swift",
        name: "Swiftness Potion",
        icon: "INV_Potion_95",
        quality: Quality::Common,
        spell: 2379,
        effect: Use::Swift(0.4, 10.0),
        cooldown: 30.0,
        levels: (0, 99),
    },
    UseDef {
        key: "dynamite",
        name: "Rough Dynamite",
        icon: "INV_Misc_Bomb_05",
        quality: Quality::Common,
        spell: 4054,
        effect: Use::Bomb {
            damage: 45.0,
            radius: 5.0,
            stun: 0.0,
            on_self: false,
        },
        cooldown: 4.0,
        levels: (0, 99),
    },
    UseDef {
        key: "grenade",
        name: "Iron Grenade",
        icon: "INV_Misc_Bomb_08",
        quality: Quality::Common,
        spell: 4068,
        effect: Use::Bomb {
            damage: 60.0,
            radius: 5.0,
            stun: 2.0,
            on_self: false,
        },
        cooldown: 8.0,
        levels: (14, 99),
    },
    UseDef {
        key: "sapper",
        name: "Goblin Sapper Charge",
        icon: "INV_Gizmo_08",
        quality: Quality::Uncommon,
        spell: 13241,
        effect: Use::Bomb {
            damage: 150.0,
            radius: 9.0,
            stun: 0.0,
            on_self: true,
        },
        cooldown: 15.0,
        levels: (24, 99),
    },
];

/// Grey drops, only worth their price.
const JUNK: &[(&str, &str)] = &[
    ("Broken Fang", "INV_Misc_Bone_08"),
    ("Cracked Bone", "INV_Misc_Bone_02"),
    ("Chipped Claw", "INV_Misc_MonsterClaw_02"),
    ("Ruined Pelt", "INV_Misc_Pelt_Wolf_Ruin_01"),
    ("Scrap of Leather", "INV_Misc_LeatherScrap_02"),
    ("Dull Fang", "INV_Misc_MonsterFang_01"),
    ("Tattered Cloth", "INV_Fabric_Linen_01"),
];

/// Each gear slot's names and icons.
const BASES: [(GearSlot, &[(&str, &str)]); 4] = [
    (
        GearSlot::Finger,
        &[
            ("Band", "INV_Jewelry_Ring_01"),
            ("Ring", "INV_Jewelry_Ring_03"),
            ("Signet", "INV_Jewelry_Ring_05"),
            ("Loop", "INV_Jewelry_Ring_15"),
        ],
    ),
    (
        GearSlot::Neck,
        &[
            ("Pendant", "INV_Jewelry_Necklace_01"),
            ("Choker", "INV_Jewelry_Necklace_04"),
            ("Amulet", "INV_Jewelry_Amulet_01"),
            ("Medallion", "INV_Jewelry_Amulet_03"),
        ],
    ),
    (
        GearSlot::Back,
        &[
            ("Cloak", "INV_Misc_Cape_01"),
            ("Cape", "INV_Misc_Cape_02"),
            ("Shroud", "INV_Misc_Cape_06"),
            ("Drape", "INV_Misc_Cape_18"),
        ],
    ),
    (
        GearSlot::Trinket,
        &[
            ("Charm", "INV_Jewelry_Talisman_01"),
            ("Talisman", "INV_Jewelry_Talisman_04"),
            ("Idol", "INV_Misc_Idol_02"),
            ("Orb", "INV_Misc_Orb_01"),
        ],
    ),
];

/// The random suffixes and the stats they carry, as "of the Bear" carries strength and stamina.
const SUFFIXES: &[(&str, &[Stat])] = &[
    ("of the Bear", &[Stat::MaxHp, Stat::Might]),
    ("of the Tiger", &[Stat::Might, Stat::Crit]),
    ("of the Monkey", &[Stat::Speed, Stat::Crit]),
    ("of the Owl", &[Stat::Haste, Stat::Area]),
    ("of the Whale", &[Stat::MaxHp, Stat::Regen]),
    ("of the Gorilla", &[Stat::Might, Stat::Armor]),
    ("of the Falcon", &[Stat::Crit, Stat::Haste]),
    ("of the Wolf", &[Stat::Speed, Stat::Might]),
    ("of the Eagle", &[Stat::MaxHp, Stat::Area]),
    ("of the Sage", &[Stat::Growth, Stat::Magnet]),
];

/// White gear's one-stat names.
const PLAIN: &[(&str, Stat)] = &[
    ("Sturdy", Stat::MaxHp),
    ("Sharp", Stat::Might),
    ("Light", Stat::Speed),
    ("Soothing", Stat::Regen),
    ("Quick", Stat::Haste),
    ("Lucky", Stat::Crit),
    ("Thick", Stat::Armor),
];

const RARE_WORDS: &[&str] = &["Gleaming", "Runed", "Ancient", "Polished", "Engraved"];
const EPIC_WORDS: &[&str] = &["Dragonforged", "Titanic", "Eternal", "Shadowforged"];

/// One point of a stat on white gear.
fn unit(stat: Stat) -> f32 {
    match stat {
        Stat::Might => 0.04,
        Stat::MaxHp => 0.06,
        Stat::Speed => 0.03,
        Stat::Regen => 0.4,
        Stat::Haste => 0.03,
        Stat::Area => 0.04,
        Stat::Magnet => 0.15,
        Stat::Growth => 0.04,
        Stat::Armor => 0.5,
        Stat::Crit => 0.02,
    }
}

/// Add a stat to the hero's, as the passives and the worn gear do.
pub(super) fn add_stat(s: &mut Stats, stat: Stat, v: f32) {
    match stat {
        Stat::Might => s.might += v,
        Stat::MaxHp => s.max_hp *= 1.0 + v,
        Stat::Speed => s.speed *= 1.0 + v,
        Stat::Regen => s.regen += v,
        Stat::Haste => s.haste += v,
        Stat::Area => s.area += v,
        Stat::Magnet => s.magnet *= 1.0 + v,
        Stat::Growth => s.growth += v,
        Stat::Armor => s.armor += v,
        Stat::Crit => s.crit += v,
    }
}

/// A stat line as the tooltip prints it.
pub(super) fn stat_line(stat: Stat, v: f32) -> String {
    let pct = |name: &str| format!("+{:.0}% {name}", v * 100.0);
    match stat {
        Stat::Might => pct("Damage"),
        Stat::MaxHp => pct("Health"),
        Stat::Speed => pct("Movement Speed"),
        Stat::Regen => format!("+{v:.1} Health per second"),
        Stat::Haste => pct("Cooldown Speed"),
        Stat::Area => pct("Area"),
        Stat::Magnet => pct("Pickup Range"),
        Stat::Growth => pct("Experience"),
        Stat::Armor => format!("+{v:.1} Armor"),
        Stat::Crit => pct("Critical Strike"),
    }
}

#[derive(Clone)]
pub(crate) enum Kind {
    Junk,
    Use(&'static UseDef),
    Gear {
        slot: GearSlot,
        stats: Vec<(Stat, f32)>,
    },
}

#[derive(Clone)]
pub(crate) struct Item {
    pub name: String,
    pub icon: &'static str,
    pub quality: Quality,
    pub kind: Kind,
    /// The vendor's price, in copper.
    pub sell: u32,
}

impl Item {
    fn use_def(&self) -> Option<&'static UseDef> {
        match self.kind {
            Kind::Use(u) => Some(u),
            _ => None,
        }
    }

    fn score(&self) -> f32 {
        match &self.kind {
            Kind::Gear { stats, .. } => stats.iter().map(|&(s, v)| v / unit(s)).sum(),
            _ => 0.0,
        }
    }

    pub(super) fn gear_slot(&self) -> Option<GearSlot> {
        match self.kind {
            Kind::Gear { slot, .. } => Some(slot),
            _ => None,
        }
    }
}

/// What became of a picked-up item.
#[derive(PartialEq, Debug)]
pub(super) enum Stowed {
    Worn,
    Bagged,
    /// The bag was full: sold on the spot for this many copper.
    Sold(u32),
}

#[derive(Default)]
pub(crate) struct Bag {
    pub slots: Vec<(Item, u32)>,
    pub worn: [Option<Item>; 6],
    /// Coins picked up this run, in copper.
    pub copper: u64,
    /// Seconds left on each usable item's cooldown, by key.
    cds: Vec<(&'static str, f32, f32)>,
    /// Food's healing per second and its seconds left; a speed potion's bonus and seconds left.
    food: (f32, f32),
    swift: (f32, f32),
    /// Explosives used from the paused backpack, set off when play resumes.
    queued: Vec<&'static UseDef>,
    /// Bumped on every change, so the item bar and the backpack rebuild once.
    pub version: u32,
}

impl Bag {
    /// Put an item away: worn if it beats what its slot wears, else stacked or in a free slot.
    pub(super) fn add(&mut self, item: Item) -> Stowed {
        self.version += 1;
        if let Some(slot) = item.gear_slot() {
            if let Some(i) = self.weakest(slot, item.score()) {
                let old = self.worn[i].replace(item);
                return match old {
                    Some(old) => {
                        self.store(old);
                        Stowed::Worn
                    }
                    None => Stowed::Worn,
                };
            }
        }
        self.store(item)
    }

    fn store(&mut self, item: Item) -> Stowed {
        if let Some(u) = item.use_def() {
            if let Some((_, n)) = self
                .slots
                .iter_mut()
                .find(|(it, n)| it.use_def().is_some_and(|o| o.key == u.key) && *n < STACK)
            {
                *n += 1;
                return Stowed::Bagged;
            }
        }
        if self.slots.len() < BAG_SLOTS {
            self.slots.push((item, 1));
            Stowed::Bagged
        } else {
            self.copper += u64::from(item.sell);
            Stowed::Sold(item.sell)
        }
    }

    /// The worn socket for `slot` that a piece scoring `score` would replace: an empty one, else
    /// the weakest below it.
    fn weakest(&self, slot: GearSlot, score: f32) -> Option<usize> {
        let sockets = (0..WORN.len()).filter(|&i| WORN[i] == slot);
        let mut best: Option<(usize, f32)> = None;
        for i in sockets {
            let have = self.worn[i].as_ref().map_or(-1.0, Item::score);
            if have < score && best.is_none_or(|(_, b)| have < b) {
                best = Some((i, have));
            }
        }
        best.map(|(i, _)| i)
    }

    /// Wear the bag's `i`th item in place of the weakest of its slot's sockets.
    pub(super) fn equip(&mut self, i: usize) {
        let Some(slot) = self.slots.get(i).and_then(|(it, _)| it.gear_slot()) else {
            return;
        };
        let socket = (0..WORN.len())
            .filter(|&s| WORN[s] == slot)
            .min_by(|&a, &b| {
                let sc = |s: usize| self.worn[s].as_ref().map_or(-1.0, Item::score);
                sc(a).total_cmp(&sc(b))
            })
            .unwrap_or(0);
        let (item, _) = self.slots.remove(i);
        if let Some(old) = self.worn[socket].replace(item) {
            self.slots.insert(i, (old, 1));
        }
        self.version += 1;
    }

    /// Take off the `socket`th worn piece, if the bag has room.
    pub(super) fn unequip(&mut self, socket: usize) {
        if self.slots.len() >= BAG_SLOTS {
            return;
        }
        if let Some(item) = self.worn.get_mut(socket).and_then(Option::take) {
            self.slots.push((item, 1));
            self.version += 1;
        }
    }

    /// The bag slots on the item bar: the first [`QUICK_SLOTS`] usable stacks.
    pub(super) fn quick(&self) -> Vec<usize> {
        (0..self.slots.len())
            .filter(|&i| self.slots[i].0.use_def().is_some())
            .take(QUICK_SLOTS)
            .collect()
    }

    /// The bag slot's item cooldown as `(left, full)`.
    pub(super) fn cooldown(&self, i: usize) -> (f32, f32) {
        let Some(u) = self.slots.get(i).and_then(|(it, _)| it.use_def()) else {
            return (0.0, 1.0);
        };
        self.cds
            .iter()
            .find(|c| c.0 == u.key)
            .map_or((0.0, 1.0), |c| (c.1, c.2))
    }

    /// Everything the bag holds and the hero wears, at the vendor's price.
    pub(super) fn worth(&self) -> u64 {
        let bagged: u64 = self
            .slots
            .iter()
            .map(|(it, n)| u64::from(it.sell) * u64::from(*n))
            .sum();
        let worn: u64 = self
            .worn
            .iter()
            .flatten()
            .map(|it| u64::from(it.sell))
            .sum();
        bagged + worn
    }

    /// What the worn pieces and a running speed potion add.
    pub(super) fn apply(&self, s: &mut Stats) {
        for item in self.worn.iter().flatten() {
            if let Kind::Gear { stats, .. } = &item.kind {
                for &(stat, v) in stats {
                    add_stat(s, stat, v);
                }
            }
        }
        if self.swift.1 > 0.0 {
            s.speed *= 1.0 + self.swift.0;
        }
    }

    /// Use the bag's `i`th item. Potions and food work at once; an explosive needs the battle
    /// running, so from the paused backpack it waits for play to resume.
    pub(super) fn use_slot(
        &mut self,
        i: usize,
        run_hp: &mut f32,
        max_hp: f32,
        paused: bool,
    ) -> Option<&'static UseDef> {
        let u = self.slots.get(i)?.0.use_def()?;
        if self.cooldown(i).0 > 0.0 {
            return None;
        }
        match u.effect {
            Use::Heal(frac) => *run_hp = (*run_hp + frac * max_hp).min(max_hp),
            Use::Food(rate, secs) => self.food = (rate * max_hp, secs),
            Use::Swift(bonus, secs) => self.swift = (bonus, secs),
            Use::Bomb { .. } if paused => self.queued.push(u),
            Use::Bomb { .. } => {}
        }
        self.cds.retain(|c| c.0 != u.key);
        self.cds.push((u.key, u.cooldown, u.cooldown));
        let (_, n) = &mut self.slots[i];
        *n -= 1;
        if *n == 0 {
            self.slots.remove(i);
        }
        self.version += 1;
        Some(u)
    }
}

/// A creature level's coin drop, in copper: a level 10 creature drops about 60 copper, a level 60
/// one about 36 silver.
fn coins(level: u32, rng: &mut Rng) -> u32 {
    let l = level.max(1) as f32;
    (l * l * rng.range(0.5, 1.5) + l) as u32
}

fn pick<T: Copy>(list: &[T], rng: &mut Rng) -> T {
    list[rng.index(list.len())]
}

/// A random item a level `level` creature drops: bosses drop rare and epic gear, the great
/// beasts better than the crowd.
pub(super) fn roll_item(level: u32, boss: bool, big: bool, rng: &mut Rng) -> Item {
    let l = level.max(1);
    let r = rng.f32();
    let (junk, usable) = if boss {
        (0.0, 0.2)
    } else if big {
        (0.3, 0.65)
    } else {
        (0.5, 0.85)
    };
    if r < junk {
        let (name, icon) = pick(JUNK, rng);
        return Item {
            name: name.into(),
            icon,
            quality: Quality::Poor,
            kind: Kind::Junk,
            sell: l * (2 + rng.index(5) as u32) + 1,
        };
    }
    if r < usable {
        let fits: Vec<&'static UseDef> = USES
            .iter()
            .filter(|u| (u.levels.0..=u.levels.1).contains(&l))
            .collect();
        let u = fits[rng.index(fits.len())];
        return Item {
            name: u.name.into(),
            icon: u.icon,
            quality: u.quality,
            kind: Kind::Use(u),
            sell: l * 2 + 5,
        };
    }
    let q = rng.f32();
    let quality = if boss {
        if q < 0.4 {
            Quality::Epic
        } else {
            Quality::Rare
        }
    } else if big {
        match q {
            q if q < 0.4 => Quality::Common,
            q if q < 0.85 => Quality::Uncommon,
            _ => Quality::Rare,
        }
    } else {
        match q {
            q if q < 0.65 => Quality::Common,
            q if q < 0.95 => Quality::Uncommon,
            _ => Quality::Rare,
        }
    };
    gear(l, quality, rng)
}

fn gear(level: u32, quality: Quality, rng: &mut Rng) -> Item {
    let (slot, bases) = BASES[rng.index(BASES.len())];
    let (base, icon) = pick(bases, rng);
    // Higher creatures drop stronger pieces; a level 60 one's half again a level 1's.
    let scale = 1.0 + level as f32 / 120.0;
    let (name, stats): (String, Vec<Stat>) = match quality {
        Quality::Poor | Quality::Common => {
            let (word, stat) = pick(PLAIN, rng);
            (format!("{word} {base}"), vec![stat])
        }
        Quality::Uncommon => {
            let (suffix, stats) = pick(SUFFIXES, rng);
            (format!("{base} {suffix}"), stats.to_vec())
        }
        Quality::Rare | Quality::Epic => {
            let (suffix, stats) = pick(SUFFIXES, rng);
            let words = if quality == Quality::Epic {
                EPIC_WORDS
            } else {
                RARE_WORDS
            };
            let mut stats = stats.to_vec();
            if quality == Quality::Epic {
                let (_, extra) = pick(PLAIN, rng);
                if !stats.contains(&extra) {
                    stats.push(extra);
                }
            }
            (format!("{} {base} {suffix}", pick(words, rng)), stats)
        }
    };
    let mult = match quality {
        Quality::Poor | Quality::Common => 1.0,
        Quality::Uncommon => 1.4,
        Quality::Rare => 2.0,
        Quality::Epic => 3.0,
    };
    let price = match quality {
        Quality::Poor | Quality::Common => 1,
        Quality::Uncommon => 3,
        Quality::Rare => 8,
        Quality::Epic => 20,
    };
    Item {
        name,
        icon,
        quality,
        kind: Kind::Gear {
            slot,
            stats: stats
                .into_iter()
                .map(|s| (s, unit(s) * mult * scale * rng.range(0.85, 1.15)))
                .collect(),
        },
        sell: price * (level * 4 + 5),
    }
}

/// Copper as the money frame shows it: `1g 2s 3c`, leaving out the empty leading coins.
pub(super) fn money(copper: u64) -> String {
    let (g, s, c) = (copper / 10_000, copper / 100 % 100, copper % 100);
    match (g, s) {
        (0, 0) => format!("{c}c"),
        (0, _) => format!("{s}s {c}c"),
        _ => format!("{g}g {s}s {c}c"),
    }
}

/// What lies on the ground: coins or an item.
#[derive(Component)]
pub(super) struct Pickup {
    what: Option<Dropped>,
    pulled: bool,
    speed: f32,
}

enum Dropped {
    Coins(u32),
    Item(Item),
}

fn spawn_pickup(commands: &mut Commands, run: &mut Run, pos: Vec3, what: Dropped) {
    let (display, scale) = match what {
        Dropped::Coins(_) => (COIN_DISPLAY, 0.7),
        Dropped::Item(_) => (SACK_DISPLAY, 0.6),
    };
    let e = units::spawn_object(commands, run.guid(), display, scale, pos + Vec3::Y * 0.2);
    commands.entity(e).insert(Pickup {
        what: Some(what),
        pulled: false,
        speed: 6.0,
    });
}

/// Coins from one creature in forty, a great beast's more often and richer, a boss's a pile;
/// an item from one in a hundred and fifty, a great beast's one in twenty, a boss's two.
pub(super) fn drop_items(
    mut commands: Commands,
    mut drops: MessageReader<Loot>,
    mut rng: ResMut<Rng>,
    mut run: ResMut<Run>,
) {
    // `WOW_SURVIVORS_LOOT=rich`, for testing: twenty times the drops.
    let rich = crate::webenv::var("WOW_SURVIVORS_LOOT").is_some_and(|v| v == "rich");
    for d in drops.read() {
        let (coin_p, coin_x, item_p) = if d.boss {
            (1.0, 25, 1.0)
        } else if d.big {
            (0.12, 3, 0.05)
        } else {
            (0.025, 1, 0.007)
        };
        let (coin_p, item_p) = if rich {
            ((coin_p * 20.0f32).min(1.0), (item_p * 20.0f32).min(1.0))
        } else {
            (coin_p, item_p)
        };
        if rng.f32() < coin_p {
            let n = coins(d.level, &mut rng) * coin_x;
            spawn_pickup(
                &mut commands,
                &mut run,
                d.pos - Vec3::X * 0.8,
                Dropped::Coins(n),
            );
        }
        let items = if d.boss { 2 } else { 1 };
        for k in 0..items {
            if rng.f32() < item_p {
                let item = roll_item(d.level, d.boss, d.big, &mut rng);
                let at = d.pos + Vec3::new(0.0, 0.0, 0.9 + k as f32 * 0.9);
                spawn_pickup(&mut commands, &mut run, at, Dropped::Item(item));
            }
        }
    }
}

/// Pickups in reach fly to the hero, as the gems do.
pub(super) fn collect_pickups(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    hero: Query<(Entity, &Transform), (With<Hero>, Without<Pickup>)>,
    mut pickups: Query<(Entity, &mut Transform, &mut Pickup)>,
    mut fx: Fx,
) {
    let Ok((hero, h)) = hero.single() else { return };
    if run.dead_t.is_some() {
        return;
    }
    let dt = time.delta_secs();
    let magnet = run.stats().magnet;
    let target = h.translation + Vec3::Y;
    for (e, mut tf, mut p) in &mut pickups {
        let d = target - tf.translation;
        let dist = d.length();
        if !p.pulled && flat(d).length() <= magnet {
            p.pulled = true;
        }
        if !p.pulled {
            continue;
        }
        if dist < 0.9 {
            match p.what.take() {
                Some(Dropped::Coins(n)) => {
                    run.bag.copper += u64::from(n);
                    run.bag.version += 1;
                    fx.label(hero, format!("+{}", money(u64::from(n))), 0xFFFF_D100);
                }
                Some(Dropped::Item(item)) => {
                    let name = item.name.clone();
                    let quality = item.quality;
                    match run.bag.add(item) {
                        Stowed::Sold(c) => fx.label(
                            hero,
                            format!("Bag full: +{}", money(u64::from(c))),
                            0xFFFF_D100,
                        ),
                        Stowed::Worn if quality >= Quality::Uncommon => {
                            run.banner = Some((format!("Equipped {name}"), 2.5));
                        }
                        _ => {}
                    }
                    run.version += 1;
                }
                None => {}
            }
            commands.entity(e).despawn();
            continue;
        }
        p.speed += 30.0 * dt;
        tf.translation += d / dist * (p.speed * dt).min(dist);
    }
}

/// The item bar's keys, 1 to 4 (a pad's D-pad presses them), the explosives used from the paused
/// backpack, and the food and speed potions running.
#[allow(clippy::too_many_arguments)]
pub(super) fn use_items(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut run: ResMut<Run>,
    hero: Query<(Entity, &Transform), With<Hero>>,
    enemies: Query<(Entity, &Transform, &Enemy)>,
    mut fx: Fx,
    mut hits: MessageWriter<Hit>,
) {
    let dt = time.delta_secs();
    let max_hp = run.stats().max_hp;
    let run = &mut *run;
    let bag = &mut run.bag;
    for c in &mut bag.cds {
        c.1 -= dt;
    }
    bag.cds.retain(|c| c.1 > 0.0);
    if bag.food.1 > 0.0 {
        bag.food.1 -= dt;
        run.hp = (run.hp + bag.food.0 * dt).min(max_hp);
    }
    if bag.swift.1 > 0.0 {
        bag.swift.1 -= dt;
    }
    let Ok((hero, h)) = hero.single() else { return };
    let mut fired: Vec<&'static UseDef> = bag.queued.drain(..).collect();
    let quick = bag.quick();
    for (k, key) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ]
    .into_iter()
    .enumerate()
    {
        if !keys.just_pressed(key) || run.dead_t.is_some() {
            continue;
        }
        if let Some(&i) = quick.get(k) {
            if let Some(u) = run.bag.use_slot(i, &mut run.hp, max_hp, false) {
                fired.push(u);
            }
            break;
        }
    }
    if !fired.is_empty() {
        run.version += 1;
    }
    let might = run.stats().might * (1.0 + run.level as f32 * 0.15);
    for u in fired {
        let Use::Bomb {
            damage,
            radius,
            stun,
            on_self,
        } = u.effect
        else {
            fx.cast(hero, u.spell, &[hero], None);
            continue;
        };
        let at = h.translation;
        let dest = if on_self {
            at
        } else {
            enemies
                .iter()
                .filter(|(_, _, e)| !e.dead)
                .map(|(_, t, _)| t.translation)
                .filter(|p| flat(*p - at).length() < 18.0)
                .min_by(|a, b| flat(*a - at).length().total_cmp(&flat(*b - at).length()))
                .unwrap_or(at + h.forward().as_vec3() * 6.0)
        };
        let targets: Vec<Entity> = enemies
            .iter()
            .filter(|(_, t, e)| !e.dead && flat(t.translation - dest).length() <= radius)
            .map(|(e, ..)| e)
            .collect();
        fx.cast(hero, u.spell, &targets, Some(dest));
        for &t in &targets {
            hits.write(Hit {
                effect: if stun > 0.0 {
                    Effect::Stun(stun)
                } else {
                    Effect::None
                },
                color: COLOR_SPELL,
                ..Hit::direct(t, damage * might, false)
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(score: f32) -> Item {
        Item {
            name: format!("Ring {score}"),
            icon: "",
            quality: Quality::Common,
            kind: Kind::Gear {
                slot: GearSlot::Finger,
                stats: vec![(Stat::Might, unit(Stat::Might) * score)],
            },
            sell: 10,
        }
    }

    #[test]
    fn coins_read_as_the_money_frame_shows_them() {
        assert_eq!(money(7), "7c");
        assert_eq!(money(1_203), "12s 3c");
        assert_eq!(money(10_000), "1g 0s 0c");
        assert_eq!(money(25_150), "2g 51s 50c");
    }

    #[test]
    fn better_rings_are_worn_and_the_weaker_bagged() {
        let mut bag = Bag::default();
        assert_eq!(bag.add(ring(1.0)), Stowed::Worn);
        assert_eq!(bag.add(ring(2.0)), Stowed::Worn);
        // Both finger sockets full: a better ring replaces the weaker, which goes to the bag.
        assert_eq!(bag.add(ring(3.0)), Stowed::Worn);
        assert_eq!(bag.slots.len(), 1);
        assert_eq!(bag.slots[0].0.name, "Ring 1");
        // A weaker one only goes in the bag.
        assert_eq!(bag.add(ring(0.5)), Stowed::Bagged);
    }

    #[test]
    fn potions_stack_and_a_full_bag_sells() {
        let mut bag = Bag::default();
        let potion = Item {
            name: "Minor Healing Potion".into(),
            icon: "",
            quality: Quality::Common,
            kind: Kind::Use(&USES[0]),
            sell: 5,
        };
        for _ in 0..3 {
            bag.add(potion.clone());
        }
        assert_eq!(bag.slots.len(), 1);
        assert_eq!(bag.slots[0].1, 3);
        let mut hp = 10.0;
        assert!(bag.use_slot(0, &mut hp, 100.0, false).is_some());
        assert_eq!(hp, 35.0);
        assert_eq!(bag.slots[0].1, 2);
        // On cooldown now.
        assert!(bag.use_slot(0, &mut hp, 100.0, false).is_none());
        for k in 0..BAG_SLOTS {
            bag.store(Item {
                name: format!("Junk {k}"),
                icon: "",
                quality: Quality::Poor,
                kind: Kind::Junk,
                sell: 3,
            });
        }
        assert_eq!(bag.slots.len(), BAG_SLOTS);
        assert_eq!(bag.copper, 3);
    }

    #[test]
    fn every_level_has_something_to_use() {
        for l in [1, 20, 40, 60] {
            assert!(USES.iter().any(|u| (u.levels.0..=u.levels.1).contains(&l)));
        }
    }
}
