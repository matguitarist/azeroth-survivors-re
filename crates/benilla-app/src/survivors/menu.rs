//! The run's pick screen, laid out like the 1.12 character create and select glue screens and built
//! from their own widgets and art ([`crate::glue`]): the race, gender and class tower on the left,
//! the zone list in the character-select frame on the right, Enter World at the bottom.
//!
//! The survivors HUD lays out on the 768-unit canvas through `UiScale`, so every glue builder here
//! takes a unit scale.

use bevy::prelude::*;
use bevy::ui_render::ui_material::MaterialNode;

use super::hud::{looks_for, rgb, start_run, HudFont, MenuRoot};
use super::profile::{zone_order, Profile};
use super::units::Custom;
use super::{data, Phase, Rng, Run, Selection};
use crate::char_select::race_name;
use crate::glue::art::{class_tc, race_tc, tc_rect, GlueArt, DIM, GOLD, NAME_EDGE};
use crate::glue::backdrop::{backdrop_border, tiled_bg_node};
use crate::glue::widgets::{
    abs, dial_arrow, glue_button, icon_button, outlined_text, GlueBtnKind, GlueDisabled, GlueText,
    Hilight, HoverLabel, LockHighlight,
};
use crate::glue::GlueClicks;
use crate::glue_strings::GlueStrings;

/// The create screen's two race columns.
const ALLIANCE: [u8; 4] = [1, 3, 4, 7];
const HORDE: [u8; 4] = [2, 5, 6, 8];

/// `CharacterSelectCharacterFrame`'s border and fill tints.
const FRAME_BORDER: Color = Color::srgb(0.8, 0.8, 0.8);
const FRAME_FILL: Color = Color::srgba(0.09, 0.09, 0.09, 0.85);

/// The tower ends under the class grid, the five appearance dials and Randomize.
const TOWER_H: f32 = 690.0;
/// The dials: the create screen's `CharacterCustomizationFrameTemplate` rows at this scale, so
/// all five fit under the nine-class grid.
const DIAL_TOP: f32 = 520.0;
const DIAL_S: f32 = 0.75;

/// The zone rows: the character-select row, compacted so every zone fits the 642-tall frame.
const ROW_TOP: f32 = 44.0;
const ROW_PITCH: f32 = 36.0;
const ROW_H: f32 = 34.0;
const ROW_W: f32 = 228.0;

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum MenuAction {
    Race(u8),
    Sex(u8),
    Class(usize),
    Map(usize),
    /// A customization dial (0 skin, 1 face, 2 hair style, 3 hair color, 4 facial hair) and step.
    Dial(u8, i8),
    Randomize,
    Enter,
    Quit,
}

/// On [`MenuRoot`] once the glue art is in and the screen is built.
#[derive(Component)]
pub(super) struct MenuBuilt;

/// A race button's face, re-cropped to the picked gender.
#[derive(Component)]
pub(super) struct RaceFace(u8);

#[derive(Component)]
pub(super) struct PickName;

/// An armor slot's icon on the pick screen.
#[derive(Component)]
pub(super) struct ArmorSlot(super::gear::Slot);

/// The line under the armor icons: the collection and its bonus, or the piece hovered.
#[derive(Component)]
pub(super) struct ArmorLine;

/// A dial's label, "Skin Color 3/9".
#[derive(Component)]
pub(super) struct DialLabel(u8);

/// The zone list, rebuilt when the profile changes what is open.
#[derive(Component)]
pub(super) struct ZoneFrame;

/// The colour a locked zone's row is written in.
const LOCKED: Color = Color::srgb(0.5, 0.5, 0.5);
/// "Won", beside a zone already won.
const WON: Color = Color::srgb(0.25, 0.85, 0.25);

/// The saved pick, `benilla-config/survivors-pick.txt` (`localStorage` on the page):
/// `class race sex map` and the five dials, restored at the menu so a reload keeps the last hero
/// and zone.
fn pick_path() -> Option<std::path::PathBuf> {
    crate::local_state::home().map(|h| h.join("survivors-pick.txt"))
}

fn save_pick(sel: &Selection) {
    let (Some(class), Some(&(_, race, sex))) = (sel.class, sel.look.and_then(|i| sel.looks.get(i)))
    else {
        return;
    };
    if let Some(path) = pick_path() {
        let c = sel.custom;
        let _ = crate::local_state::write_atomic(
            &path,
            &format!(
                "{class} {race} {sex} {} {} {} {} {} {}",
                sel.map, c.skin, c.face, c.hair_style, c.hair_color, c.facial_hair
            ),
        );
    }
}

/// Restore the saved pick once the creature data can resolve its looks; a stale or missing file
/// leaves the default. A zone that isn't open (the default, an older pick) gives way to the first
/// starting zone.
pub(super) fn restore_pick(
    creatures: Option<Res<crate::entities::Creatures>>,
    char_create: Option<Res<crate::entities::CharCreate>>,
    profile: Res<Profile>,
    mut selection: ResMut<Selection>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let (Some(creatures), Some(char_create)) = (creatures.as_deref(), char_create.as_deref())
    else {
        return;
    };
    *done = true;
    restore_saved(creatures, char_create, &mut selection);
    if !profile.is_open(selection.map) {
        if let Some(z) = zone_order().first() {
            selection.map = z.map;
            selection.stage = z.map;
            selection.version += 1;
        }
    }
}

fn restore_saved(
    creatures: &crate::entities::Creatures,
    char_create: &crate::entities::CharCreate,
    selection: &mut Selection,
) {
    let Some(text) = pick_path().and_then(|p| crate::local_state::read_to_string(&p).ok()) else {
        return;
    };
    let v: Vec<usize> = text
        .split_whitespace()
        .filter_map(|t| t.parse().ok())
        .collect();
    let (&[class, race, sex, map], dials) = (v.split_at(v.len().min(4)).0, v.get(4..9)) else {
        return;
    };
    if let Some(&[skin, face, hair_style, hair_color, facial_hair]) = dials {
        selection.custom = Custom {
            skin: skin as u8,
            face: face as u8,
            hair_style: hair_style as u8,
            hair_color: hair_color as u8,
            facial_hair: facial_hair as u8,
        };
    }
    if map < data::MAPS.len() {
        selection.map = map;
        selection.stage = map;
        selection.version += 1;
    }
    if class < data::CLASSES.len() {
        let all = all_looks(creatures, Some(char_create));
        if let Some(l) = find(&all[class], Some(race as u8), Some(sex as u8)) {
            set(selection, &all, class, l);
        }
    }
}

/// Every class's looks, `(display, race, sex)`, indexed like [`data::CLASSES`].
fn all_looks(
    creatures: &crate::entities::Creatures,
    char_create: Option<&crate::entities::CharCreate>,
) -> Vec<Vec<(u32, u8, u8)>> {
    data::CLASSES
        .iter()
        .map(|c| looks_for(c, creatures, char_create))
        .collect()
}

fn find(looks: &[(u32, u8, u8)], race: Option<u8>, sex: Option<u8>) -> Option<usize> {
    looks
        .iter()
        .position(|&(_, r, s)| race.is_none_or(|x| x == r) && sex.is_none_or(|x| x == s))
}

fn set(sel: &mut Selection, all: &[Vec<(u32, u8, u8)>], class: usize, look: usize) {
    sel.class = Some(class);
    sel.looks = all[class].clone();
    sel.look = Some(look);
    sel.version += 1;
}

/// Build the screen into [`MenuRoot`] once the archives are readable, so the glue art is there.
pub(super) fn build_menu(
    mut commands: Commands,
    roots: Query<Entity, (With<MenuRoot>, Without<MenuBuilt>)>,
    assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut art: ResMut<GlueArt>,
    mut images: ResMut<Assets<Image>>,
    mut add_mats: ResMut<Assets<crate::glue::add_material::AddUiMaterial>>,
    font: Res<HudFont>,
    profile: Res<Profile>,
) {
    let Ok(root) = roots.single() else { return };
    let Some(mut assets) = assets else { return };
    art.ensure_loaded(&mut assets, &mut images, &mut add_mats);
    let f = &font.0;
    commands.entity(root).insert(MenuBuilt).with_children(|ui| {
        tower(ui, &art, f);
        // `CharacterCreateWoWLogo`, 256×128 at TOPLEFT (3,-7), after the tower so it draws over
        // the border art, as the create screen orders it.
        if let Some(logo) = &art.logo {
            ui.spawn((
                ImageNode::new(logo.clone()),
                abs(1.0, 3.0, 7.0, 256.0, 128.0),
            ));
        }
        zone_frame(ui, &art, f, &profile);

        armor_strip(ui, f);

        // `CharSelectCharacterName`, GlueFontNormalHuge at BOTTOM (0,100).
        ui.spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(100.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|c| {
            outlined_text(
                c,
                Node::default(),
                (),
                PickName,
                GlueText {
                    text: "",
                    size: 22.0,
                    color: GOLD,
                    wrap: false,
                },
                f,
                1.0,
            );
        });
        // `CharSelectEnterWorldButton`, 200×60 at BOTTOM (0,30).
        ui.spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(30.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|c| {
            glue_button(
                c,
                &art,
                f,
                MenuAction::Enter,
                "Enter World",
                200.0,
                60.0,
                GlueBtnKind::Normal,
                1.0,
            );
        });
        // `CharacterSelectBackButton`'s place, BOTTOMRIGHT (-24,30): the game has nothing behind
        // the pick screen to go back to, so it quits. A page has no quit.
        if cfg!(not(target_arch = "wasm32")) {
            ui.spawn(Node {
                position_type: PositionType::Absolute,
                right: Val::Px(24.0),
                bottom: Val::Px(30.0),
                ..default()
            })
            .with_children(|c| {
                glue_button(
                    c,
                    &art,
                    f,
                    MenuAction::Quit,
                    "Quit",
                    150.0,
                    38.0,
                    GlueBtnKind::Small,
                    1.0,
                );
            });
        }
    });
}

/// `CharacterCreateConfigurationFrame`, 206×600 at TOPLEFT (28,-74): the race columns under the
/// faction banners, the gender pair, the class grid, the class text and Randomize.
fn tower(ui: &mut ChildSpawnerCommands, art: &GlueArt, f: &Handle<Font>) {
    let px = Val::Px;
    // Hovered, so a drag that starts on the panel doesn't turn the hero.
    ui.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(28.0),
            top: px(74.0),
            width: px(206.0),
            height: px(TOWER_H),
            ..default()
        },
        Interaction::default(),
    ))
    .with_children(|tower| {
        if let Some(bg) = &art.tower_bg {
            tower.spawn((
                ImageNode::new(bg.clone()),
                abs(1.0, -3.0, 0.0, 218.0, TOWER_H + 6.0),
            ));
        }
        // The create tower's top piece, then its foot piece cut to end where the class grid does.
        if let Some((border, size)) = &art.tower_border {
            for (top, height, tc) in [
                (0.0, 236.0, [0.0, 0.875, 0.0, 0.9375]),
                (
                    236.0,
                    TOWER_H - 236.0 + 6.0,
                    [
                        0.0,
                        0.875,
                        1.0 - (TOWER_H - 236.0 + 6.0) / 256.0 * 0.8203125 / (210.0 / 256.0),
                        1.0,
                    ],
                ),
            ] {
                tower.spawn((
                    ImageNode {
                        image: border.clone(),
                        rect: Some(tc_rect(*size, tc)),
                        ..default()
                    },
                    abs(1.0, -9.0, top, 224.0, height),
                ));
            }
        }
        if let Some(banners) = &art.banners {
            tower.spawn((
                ImageNode::new(banners.clone()),
                abs(1.0, -27.0, 60.0, 256.0, 259.0),
            ));
        }
        for (label, center) in [("Alliance", 51.0), ("Horde", 151.0)] {
            outlined_text(
                tower,
                Node {
                    justify_content: JustifyContent::Center,
                    ..abs(1.0, center - 60.0, 40.0, 120.0, 17.0)
                },
                (),
                (),
                GlueText {
                    text: label,
                    size: 15.0,
                    color: GOLD,
                    wrap: false,
                },
                f,
                1.0,
            );
        }

        for (faction, left) in [(ALLIANCE, 33.0), (HORDE, 127.0)] {
            tower
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    left: px(left),
                    top: px(68.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(5.0),
                    ..default()
                })
                .with_children(|col| {
                    for race in faction {
                        let face = art.races.as_ref().and_then(|(sheet, size)| {
                            race_tc(race, 0).map(|tc| (sheet.clone(), tc_rect(*size, tc)))
                        });
                        icon_button(
                            col,
                            f,
                            MenuAction::Race(race),
                            Some(RaceFace(race)),
                            face,
                            None::<()>,
                            race_name(race),
                            art,
                            1.0,
                        );
                    }
                });
        }

        tower
            .spawn(Node {
                position_type: PositionType::Absolute,
                left: px(53.0),
                top: px(303.0),
                flex_direction: FlexDirection::Row,
                column_gap: px(5.0),
                ..default()
            })
            .with_children(|g| {
                for (sex, label) in [(0u8, "Male"), (1u8, "Female")] {
                    let face = art.gender.as_ref().map(|(sheet, size)| {
                        let half = if sex == 0 {
                            [0.0, 0.5, 0.0, 1.0]
                        } else {
                            [0.5, 1.0, 0.0, 1.0]
                        };
                        (sheet.clone(), tc_rect(*size, half))
                    });
                    icon_button(
                        g,
                        f,
                        MenuAction::Sex(sex),
                        None::<()>,
                        face,
                        None::<()>,
                        label,
                        art,
                        1.0,
                    );
                }
            });

        tower
            .spawn(Node {
                position_type: PositionType::Absolute,
                // The gender pair's centre line: 53 + (2·48 + 5)/2 ≈ 103.
                left: px(103.0 - (3.0 * 48.0 + 2.0 * 4.0) / 2.0),
                width: px(3.0 * 48.0 + 2.0 * 4.0),
                top: px(368.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                column_gap: px(4.0),
                row_gap: px(4.0),
                ..default()
            })
            .with_children(|grid| {
                for (i, class) in data::CLASSES.iter().enumerate() {
                    let face = art.classes.as_ref().and_then(|(sheet, size)| {
                        class_tc(class.id).map(|tc| (sheet.clone(), tc_rect(*size, tc)))
                    });
                    icon_button(
                        grid,
                        f,
                        MenuAction::Class(i),
                        None::<()>,
                        face,
                        None::<()>,
                        class.name,
                        art,
                        1.0,
                    );
                }
            });

        // `CharacterCustomizationFrameTemplate` rows, centred under the class grid.
        tower
            .spawn(Node {
                position_type: PositionType::Absolute,
                left: px((206.0 - 198.0 * DIAL_S) / 2.0),
                top: px(DIAL_TOP),
                width: px(198.0 * DIAL_S),
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|dials| {
                for dial in 0..5u8 {
                    dial_row(dials, art, f, dial);
                }
            });

        tower
            .spawn(Node {
                position_type: PositionType::Absolute,
                left: px(30.0),
                top: px(TOWER_H - 46.0),
                ..default()
            })
            .with_children(|r| {
                glue_button(
                    r,
                    art,
                    f,
                    MenuAction::Randomize,
                    "Randomize",
                    146.0,
                    30.0,
                    GlueBtnKind::Small,
                    1.0,
                );
            });
    });
}

/// The zone list in `CharacterSelectCharacterFrame` (260×642 at TOPRIGHT (-5,-15)), one
/// character-row per zone, lowest level first.
/// After a run that changed the profile, the zone list is built again, so a zone just opened
/// shows open.
pub(super) fn rebuild_zones(
    mut commands: Commands,
    roots: Query<Entity, With<MenuBuilt>>,
    frames: Query<Entity, With<ZoneFrame>>,
    art: Res<GlueArt>,
    font: Res<HudFont>,
    profile: Res<Profile>,
    mut seen: Local<Option<u32>>,
) {
    let Ok(root) = roots.single() else { return };
    let built = seen.replace(profile.version);
    if built.is_none_or(|v| v == profile.version) {
        return;
    }
    for e in &frames {
        commands.entity(e).despawn();
    }
    commands
        .entity(root)
        .with_children(|ui| zone_frame(ui, &art, &font.0, &profile));
}

/// The armor worn, slot by slot, over the hero's name.
fn armor_strip(ui: &mut ChildSpawnerCommands, f: &Handle<Font>) {
    let px = Val::Px;
    ui.spawn(Node {
        position_type: PositionType::Absolute,
        bottom: px(132.0),
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: px(4.0),
        ..default()
    })
    .with_children(|col| {
        col.spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: px(4.0),
            ..default()
        })
        .with_children(|row| {
            for slot in super::gear::Slot::ALL {
                row.spawn((
                    ArmorSlot(slot),
                    Interaction::default(),
                    ImageNode {
                        color: Color::srgba(0.0, 0.0, 0.0, 0.6),
                        ..default()
                    },
                    Node {
                        width: px(30.0),
                        height: px(30.0),
                        border: UiRect::all(px(1.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.6, 0.6, 0.6, 0.8)),
                ));
            }
        });
        outlined_text(
            col,
            Node::default(),
            (),
            ArmorLine,
            GlueText {
                text: "",
                size: 12.0,
                color: Color::WHITE,
                wrap: false,
            },
            f,
            1.0,
        );
    });
}

/// The armor strip: each slot's worn piece (its item icon, the quality's border) or the paper
/// doll's empty slot, and the line under it.
#[allow(clippy::type_complexity)]
pub(super) fn refresh_armor(
    selection: Res<Selection>,
    profile: Res<Profile>,
    catalog: Res<super::gear::GearCatalog>,
    mut assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut slots: Query<(&ArmorSlot, &Interaction, &mut ImageNode, &mut BorderColor)>,
    mut line: Query<&mut Text, With<ArmorLine>>,
    mut shown: Local<Option<(Option<usize>, u32, usize)>>,
) {
    use super::gear::{sets, Slot};
    let Some(class) = selection.class_def() else {
        return;
    };
    let worn = profile.worn(class.id, &catalog);
    let piece = |slot: Slot| {
        worn.iter()
            .find(|(s, _)| *s == slot)
            .and_then(|&(s, set)| catalog.piece(class.id, set, s))
    };
    let key = (selection.class, profile.version, catalog.pieces.len());
    if *shown != Some(key) {
        if let Some(assets) = assets.as_deref_mut() {
            *shown = Some(key);
            for (slot, _, mut img, mut border) in &mut slots {
                let worn = piece(slot.0);
                let path = match worn.and_then(|p| p.icon.as_deref()) {
                    Some(icon) => format!("{icon}.blp"),
                    None => format!(
                        "Interface\\PaperDoll\\UI-PaperDoll-Slot-{}.blp",
                        slot.0.paper_doll()
                    ),
                };
                match assets.sprite_texture(&path, &mut images) {
                    Some(h) => {
                        img.image = h;
                        img.color = Color::WHITE;
                    }
                    None => img.color = Color::srgba(0.0, 0.0, 0.0, 0.6),
                }
                // Epic purple for a piece, grey for an empty slot.
                *border = BorderColor::all(if worn.is_some() {
                    Color::srgb(0.64, 0.21, 0.93)
                } else {
                    Color::srgba(0.6, 0.6, 0.6, 0.8)
                });
            }
        }
    }
    let hovered = slots
        .iter()
        .find(|(_, i, _, _)| **i != Interaction::None)
        .map(|(s, ..)| s.0);
    let want = match hovered {
        Some(slot) => match piece(slot) {
            Some(p) => format!("{} ({})", p.name, sets(class.id)[p.set].name),
            None => format!(
                "{}: no piece yet. Bosses drop {} armor.",
                slot.name(),
                class.name
            ),
        },
        None => {
            let owned = catalog
                .for_class(class.id)
                .iter()
                .filter(|p| profile.owns(class.id, p.set, p.slot))
                .count();
            let total = catalog.for_class(class.id).len();
            let (might, hp) = super::gear::bonus(class.id, &profile, &catalog);
            if owned == 0 {
                format!("Armor 0/{total}: bosses drop pieces for your class")
            } else {
                format!(
                    "Armor {owned}/{total}   +{:.0}% damage   +{:.0}% health",
                    might * 100.0,
                    hp * 100.0
                )
            }
        }
    };
    if let Ok(mut t) = line.single_mut() {
        if t.0 != want {
            t.0 = want;
        }
    }
}

/// One dial: the create screen's label frame and arrows (`dial_row` in the create screen).
fn dial_row(dials: &mut ChildSpawnerCommands, art: &GlueArt, f: &Handle<Font>, dial: u8) {
    let s = DIAL_S;
    let px = |v: f32| Val::Px(v * s);
    dials
        .spawn(Node {
            width: px(198.0),
            height: px(32.0),
            ..default()
        })
        .with_children(|row| {
            if let Some((frame, size)) = &art.label_frame {
                for (left, width, tc) in [
                    (-5.0, 25.0, [0.0, 0.1953125, 0.0, 1.0]),
                    (20.0, 109.0, [0.1953125, 0.8046875, 0.0, 1.0]),
                    (129.0, 25.0, [0.8046875, 1.0, 0.0, 1.0]),
                ] {
                    row.spawn((
                        ImageNode {
                            image: frame.clone(),
                            rect: Some(tc_rect(*size, tc)),
                            ..default()
                        },
                        abs(s, left, -16.0, width, 64.0),
                    ));
                }
            }
            outlined_text(
                row,
                Node {
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..abs(s, 20.0, 0.0, 109.0, 32.0)
                },
                (),
                DialLabel(dial),
                GlueText {
                    text: "",
                    size: 12.0,
                    color: Color::WHITE,
                    wrap: false,
                },
                f,
                s,
            );
            dial_arrow(
                row,
                &art.arrow_left,
                f,
                MenuAction::Dial(dial, -1),
                137.0,
                "<",
                s,
            );
            dial_arrow(
                row,
                &art.arrow_right,
                f,
                MenuAction::Dial(dial, 1),
                166.0,
                ">",
                s,
            );
        });
}

/// The look's dials held inside what the race and sex offer.
fn clamp_custom(sel: &mut Selection, cc: Option<&crate::entities::CharCreate>) {
    let Some((race, sex)) = sel
        .look
        .and_then(|i| sel.looks.get(i))
        .map(|&(_, r, s)| (r, s))
    else {
        return;
    };
    let Some(r) = cc.and_then(|c| c.0.ranges(race, sex)) else {
        return;
    };
    let c = &mut sel.custom;
    for (v, n) in [
        (&mut c.skin, r.skin),
        (&mut c.face, r.face),
        (&mut c.hair_style, r.hair_style),
        (&mut c.hair_color, r.hair_color),
        (&mut c.facial_hair, r.facial_hair),
    ] {
        if *v >= n.max(1) {
            *v = 0;
        }
    }
}

fn zone_frame(ui: &mut ChildSpawnerCommands, art: &GlueArt, f: &Handle<Font>, profile: &Profile) {
    let px = Val::Px;
    ui.spawn((
        ZoneFrame,
        Interaction::default(),
        Node {
            position_type: PositionType::Absolute,
            right: px(5.0),
            top: px(15.0),
            width: px(260.0),
            height: px(642.0),
            ..default()
        },
    ))
    .with_children(|frame| {
        if let (Some(bg), Some(border)) = (&art.tooltip_bg, &art.name_border) {
            frame.spawn((
                tiled_bg_node(bg.clone(), NAME_EDGE, 1.0, FRAME_FILL),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(10.0),
                    right: px(5.0),
                    top: px(4.0),
                    bottom: px(9.0),
                    ..default()
                },
            ));
            backdrop_border(frame, border, NAME_EDGE, FRAME_BORDER);
        } else {
            frame.spawn((BackgroundColor(FRAME_FILL), crate::glue::widgets::overlay()));
        }
        outlined_text(
            frame,
            Node {
                position_type: PositionType::Absolute,
                top: px(12.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            (),
            (),
            GlueText {
                text: "Choose a Zone",
                size: 18.0,
                color: DIM,
                wrap: false,
            },
            f,
            1.0,
        );
        for (row, z) in zone_order().iter().enumerate() {
            let (i, lo, hi) = (z.map, z.lo, z.hi);
            let map = &data::MAPS[i];
            let lock = profile.lock(i);
            let node = Node {
                position_type: PositionType::Absolute,
                left: px(24.0),
                top: px(ROW_TOP + row as f32 * ROW_PITCH),
                width: px(ROW_W),
                height: px(ROW_H),
                ..default()
            };
            // A locked row is only text: no button, so no click and no highlight.
            let mut row = match lock {
                None => frame.spawn((MenuAction::Map(i), Button, LockHighlight::default(), node)),
                Some(_) => frame.spawn(node),
            };
            row.with_children(|b| {
                if let (None, Some(hi)) = (lock, &art.select_highlight) {
                    b.spawn((
                        Hilight,
                        Visibility::Hidden,
                        MaterialNode(hi.clone()),
                        abs(1.0, -20.0, -6.0, ROW_W + 20.0, ROW_H + 12.0),
                    ));
                }
                outlined_text(
                    b,
                    abs(1.0, 0.0, 2.0, ROW_W, 16.0),
                    (),
                    (),
                    GlueText {
                        text: map.name,
                        size: 15.0,
                        color: if lock.is_some() { LOCKED } else { GOLD },
                        wrap: false,
                    },
                    f,
                    1.0,
                );
                if profile.won(i) {
                    b.spawn(Node {
                        position_type: PositionType::Absolute,
                        right: px(4.0),
                        top: px(2.0),
                        ..default()
                    })
                    .with_children(|t| {
                        outlined_text(
                            t,
                            Node::default(),
                            (),
                            (),
                            GlueText {
                                text: "Won",
                                size: 12.0,
                                color: WON,
                                wrap: false,
                            },
                            f,
                            1.0,
                        );
                    });
                }
                let line = match lock {
                    None if lo == hi => format!("Level {lo}  {}", map.zone),
                    None => format!("Level {lo}-{hi}  {}", map.zone),
                    Some(Some(prev)) => format!("Locked: win {}", data::MAPS[prev].name),
                    Some(None) => "Locked: win a starting zone".to_string(),
                };
                outlined_text(
                    b,
                    abs(1.0, 0.0, 19.0, ROW_W, 14.0),
                    (),
                    (),
                    GlueText {
                        text: &line,
                        size: 12.0,
                        color: if lock.is_some() { LOCKED } else { Color::WHITE },
                        wrap: false,
                    },
                    f,
                    1.0,
                );
            });
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub(super) fn menu_clicks(
    clicks: Res<GlueClicks>,
    actions: Query<(Entity, &MenuAction)>,
    phase: Res<State<Phase>>,
    creatures: Option<Res<crate::entities::Creatures>>,
    char_create: Option<Res<crate::entities::CharCreate>>,
    profile: Res<Profile>,
    catalog: Res<super::gear::GearCatalog>,
    mut selection: ResMut<Selection>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    mut next: ResMut<NextState<Phase>>,
    mut exit: MessageWriter<AppExit>,
) {
    if *phase.get() != Phase::Menu {
        return;
    }
    if actions
        .iter()
        .any(|(e, a)| *a == MenuAction::Quit && clicks.hit(e))
    {
        exit.write(AppExit::Success);
        return;
    }
    let Some(creatures) = creatures.as_deref() else {
        return;
    };
    let Some(action) = actions
        .iter()
        .find(|(e, _)| clicks.hit(*e))
        .map(|(_, a)| *a)
    else {
        return;
    };
    let all = all_looks(creatures, char_create.as_deref());
    let cur = selection
        .look
        .and_then(|i| selection.looks.get(i))
        .map(|&(_, r, s)| (r, s));
    let (race, sex) = (cur.map(|c| c.0), cur.map_or(0, |c| c.1));
    match action {
        MenuAction::Map(i) => {
            if selection.map != i && profile.is_open(i) {
                selection.map = i;
                selection.version += 1;
            }
        }
        MenuAction::Enter => {
            if profile.is_open(selection.map) && start_run(&mut run, &mut selection) {
                (run.gear_might, run.gear_hp) =
                    super::gear::bonus(run.class.id, &profile, &catalog);
                run.hp = run.stats().max_hp;
                next.set(Phase::Playing);
            }
        }
        MenuAction::Randomize => {
            let picks: Vec<(usize, usize)> = all
                .iter()
                .enumerate()
                .flat_map(|(c, looks)| (0..looks.len()).map(move |l| (c, l)))
                .collect();
            if !picks.is_empty() {
                let (c, l) = picks[rng.index(picks.len())];
                set(&mut selection, &all, c, l);
            }
            let (_, race, sex) = selection.looks[selection.look.unwrap_or(0)];
            if let Some(r) = char_create.as_deref().and_then(|c| c.0.ranges(race, sex)) {
                let mut roll = |n: u8| rng.index(usize::from(n.max(1))) as u8;
                selection.custom = Custom {
                    skin: roll(r.skin),
                    face: roll(r.face),
                    hair_style: roll(r.hair_style),
                    hair_color: roll(r.hair_color),
                    facial_hair: roll(r.facial_hair),
                };
            }
        }
        MenuAction::Dial(dial, step) => {
            if let (Some((race, sex)), Some(cc)) = (cur, char_create.as_deref()) {
                if let Some(r) = cc.0.ranges(race, sex) {
                    let c = &mut selection.custom;
                    let (v, n) = match dial {
                        0 => (&mut c.skin, r.skin),
                        1 => (&mut c.face, r.face),
                        2 => (&mut c.hair_style, r.hair_style),
                        3 => (&mut c.hair_color, r.hair_color),
                        _ => (&mut c.facial_hair, r.facial_hair),
                    };
                    let n = i16::from(n.max(1));
                    *v = (i16::from(*v) + i16::from(step)).rem_euclid(n) as u8;
                    selection.version += 1;
                }
            }
        }
        // The class keeps the race and gender where it can.
        MenuAction::Class(c) => {
            let looks = &all[c];
            let l = find(looks, race, Some(sex))
                .or_else(|| find(looks, race, None))
                .or_else(|| find(looks, None, Some(sex)))
                .or_else(|| find(looks, None, None));
            if let Some(l) = l {
                set(&mut selection, &all, c, l);
            }
        }
        // The race keeps the class where it can, else takes the first class it has.
        MenuAction::Race(r) => {
            let classes = selection.class.into_iter().chain(0..all.len());
            for c in classes {
                let l = find(&all[c], Some(r), Some(sex)).or_else(|| find(&all[c], Some(r), None));
                if let Some(l) = l {
                    set(&mut selection, &all, c, l);
                    break;
                }
            }
        }
        MenuAction::Quit => {}
        MenuAction::Sex(s) => {
            if let Some(c) = selection.class {
                if let Some(l) = find(&all[c], race, Some(s)) {
                    set(&mut selection, &all, c, l);
                }
            }
        }
    }
    clamp_custom(&mut selection, char_create.as_deref());
    save_pick(&selection);
}

/// The screen's per-frame state: the locked highlights, hover labels, which races and classes
/// the pick allows, the gender on the race faces, the name line and Enter World.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh_menu_ui(
    selection: Res<Selection>,
    creatures: Option<Res<crate::entities::Creatures>>,
    art: Res<GlueArt>,
    mut buttons: Query<(
        &MenuAction,
        &Interaction,
        &Children,
        Option<&mut LockHighlight>,
        Option<&mut GlueDisabled>,
        &mut Node,
        &mut Visibility,
    )>,
    mut labels: Query<&mut Visibility, (With<HoverLabel>, Without<MenuAction>)>,
    mut faces: Query<(&RaceFace, &mut ImageNode)>,
    mut name: Query<(&mut Text, &mut TextColor), (With<PickName>, Without<DialLabel>)>,
    mut dial_labels: Query<(&DialLabel, &mut Text), Without<PickName>>,
    (char_create, strings): (
        Option<Res<crate::entities::CharCreate>>,
        Option<Res<GlueStrings>>,
    ),
    mut cache: Local<(Option<u32>, bool, Vec<Vec<(u32, u8, u8)>>)>,
) {
    let have = creatures.is_some() && char_create.is_some();
    if cache.0 != Some(selection.version) || cache.1 != have {
        *cache = (
            Some(selection.version),
            have,
            creatures
                .as_deref()
                .map(|c| all_looks(c, char_create.as_deref()))
                .unwrap_or_default(),
        );
    }
    let all = &cache.2;
    let cur = selection
        .look
        .and_then(|i| selection.looks.get(i))
        .map(|&(_, r, s)| (r, s));

    for (action, interaction, children, lock, disabled, mut node, mut vis) in &mut buttons {
        let selected = match *action {
            MenuAction::Race(r) => cur.is_some_and(|c| c.0 == r),
            MenuAction::Sex(s) => cur.is_some_and(|c| c.1 == s),
            MenuAction::Class(i) => selection.class == Some(i),
            MenuAction::Map(i) => selection.map == i,
            MenuAction::Dial(..) | MenuAction::Randomize | MenuAction::Enter | MenuAction::Quit => {
                false
            }
        };
        if let Some(mut lock) = lock {
            if lock.0 != selected {
                lock.0 = selected;
            }
        }
        if let Some(mut disabled) = disabled {
            let off = *action == MenuAction::Enter && cur.is_none();
            if disabled.0 != off {
                disabled.0 = off;
            }
        }
        match *action {
            // As on the create screen, a class the race cannot be leaves the grid.
            MenuAction::Class(i) => {
                let ok = cur.is_none_or(|(r, _)| {
                    all.get(i).is_some_and(|l| find(l, Some(r), None).is_some())
                });
                let display = if ok { Display::Flex } else { Display::None };
                if node.display != display {
                    node.display = display;
                }
            }
            MenuAction::Race(r) => {
                let ok = all.is_empty() || all.iter().any(|l| find(l, Some(r), None).is_some());
                let want = if ok {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *vis != want {
                    *vis = want;
                }
            }
            _ => {}
        }
        let show = selected || *interaction != Interaction::None || art.races.is_none();
        for child in children {
            if let Ok(mut label) = labels.get_mut(*child) {
                let want = if show {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *label != want {
                    *label = want;
                }
            }
        }
    }

    if let (Some((_, size)), Some((_, sex))) = (&art.races, cur) {
        for (face, mut img) in &mut faces {
            if let Some(tc) = race_tc(face.0, sex) {
                let rect = Some(tc_rect(*size, tc));
                if img.rect != rect {
                    img.rect = rect;
                }
            }
        }
    }

    // The dials, labelled as the create screen labels them, with where each stands.
    let empty = GlueStrings::default();
    let strings = strings.as_deref().unwrap_or(&empty);
    let cc = char_create.as_deref().map(|c| &c.0);
    for (dial, mut t) in &mut dial_labels {
        let want = match cur {
            Some((race, sex)) => {
                let r = cc.and_then(|c| c.ranges(race, sex));
                let hair = cc
                    .and_then(|c| c.hair_customization(race))
                    .unwrap_or("NORMAL");
                let facial = cc
                    .and_then(|c| c.facial_hair_customization(race, sex))
                    .unwrap_or("NORMAL");
                let c = selection.custom;
                let (label, v, n) = match dial.0 {
                    0 => (
                        strings
                            .text("CHAR_CUSTOMIZATION1_DESC", "Skin Color")
                            .to_string(),
                        c.skin,
                        r.map_or(1, |r| r.skin),
                    ),
                    1 => (
                        strings.text("CHAR_CUSTOMIZATION2_DESC", "Face").to_string(),
                        c.face,
                        r.map_or(1, |r| r.face),
                    ),
                    2 => (
                        strings
                            .text(&format!("HAIR_{hair}_STYLE"), "Hair Style")
                            .to_string(),
                        c.hair_style,
                        r.map_or(1, |r| r.hair_style),
                    ),
                    3 => (
                        strings
                            .text(&format!("HAIR_{hair}_COLOR"), "Hair Color")
                            .to_string(),
                        c.hair_color,
                        r.map_or(1, |r| r.hair_color),
                    ),
                    _ => (
                        strings
                            .text(&format!("FACIAL_HAIR_{facial}"), "Facial Hair")
                            .to_string(),
                        c.facial_hair,
                        r.map_or(1, |r| r.facial_hair),
                    ),
                };
                format!("{label} {}/{}", v + 1, n.max(1))
            }
            None => String::new(),
        };
        if t.0 != want {
            t.0 = want;
        }
    }

    let class = selection.class_def();
    if let Ok((mut t, mut color)) = name.single_mut() {
        let want = match (class, cur) {
            (Some(c), Some((r, _))) => format!("{} {}", race_name(r), c.name),
            _ => String::new(),
        };
        if t.0 != want {
            t.0 = want;
        }
        // In the class's own colour, as the game's raid frames and chat show it.
        let want = class.map_or(GOLD, |c| rgb(c.color));
        if color.0 != want {
            color.0 = want;
        }
    }
}
