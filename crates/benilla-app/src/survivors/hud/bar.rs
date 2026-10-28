//! The action and item bars: their sockets built once, their icons, cooldown sweeps and counts
//! kept current, and the recess's boss, damage and victory lines.

use super::*;

/// `ActionButtonTemplate`'s 36-unit button, with `UI-Quickslot2` (66×66 at CENTER (0,−1)) as
/// its frame.
pub(super) const SLOT: f32 = 36.0;

/// The item bar's bottom, over the experience bar (40 up, 13 tall).
const ITEM_BAR_BOTTOM: f32 = 57.0;

pub(super) fn quickslot_frame() -> impl Bundle {
    socket_art(UiArt::crop(
        "Interface\\Buttons\\UI-Quickslot2",
        [0.0, 1.0, 0.0, 1.0],
    ))
}

/// A 66-unit button art centred on a 36-unit slot, as `UI-Quickslot2` sits.
fn socket_art(art: UiArt) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px((SLOT - 66.0) / 2.0),
            top: Val::Px((SLOT - 66.0) / 2.0 + 1.0),
            width: Val::Px(66.0),
            height: Val::Px(66.0),
            ..default()
        },
        ImageNode {
            color: Color::NONE,
            ..default()
        },
        art,
        bevy::ui::FocusPolicy::Pass,
    )
}

/// `MainMenuBar`, 1024×53 at BOTTOM: `MainMenuBarTexture0`..`3` (256×43 each, at −384, −128, 128
/// and 384 from the centre), the gryphon end caps (128×128 at ±544), the twelve action buttons
/// from BOTTOMLEFT (8,4) six apart, and the backpack at BOTTOMRIGHT (−6,2). With no micro menu
/// or bags, their recess carries the run's pace and the four bag sockets (each 5 left of the
/// next, 37 wide) the armor it has dropped. Over the experience bar, as `MultiBarBottomLeft`
/// sits, a short bar holds the mobility spell and the items to use; the money frame faces it.
pub(super) fn action_bar(
    root: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    sweeps: &mut Assets<SweepUiMaterial>,
) {
    let art = |left: f32, w: f32, h: f32, path: &'static str, coords: [f32; 4]| {
        (
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                bottom: Val::Px(0.0),
                width: Val::Px(w),
                height: Val::Px(h),
                ..default()
            },
            ImageNode {
                color: Color::NONE,
                ..default()
            },
            UiArt::crop(path, coords),
        )
    };
    root.spawn((
        ActionBar,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            bottom: Val::Px(0.0),
            margin: UiRect::left(Val::Px(-512.0)),
            width: Val::Px(1024.0),
            height: Val::Px(53.0),
            ..default()
        },
    ))
    .with_children(|bar| {
        const BAR: &str = "Interface\\MainMenuBar\\UI-MainMenuBar-Dwarf";
        for (i, top) in [0.832_031_25, 0.582_031_25, 0.332_031_25, 0.082_031_25]
            .into_iter()
            .enumerate()
        {
            bar.spawn(art(
                256.0 * i as f32,
                256.0,
                43.0,
                BAR,
                [0.0, 1.0, top, top + 0.167_968_75],
            ));
        }
        const CAP: &str = "Interface\\MainMenuBar\\UI-MainMenuBar-EndCap-Dwarf";
        bar.spawn(art(
            512.0 - 544.0 - 64.0,
            128.0,
            128.0,
            CAP,
            [0.0, 1.0, 0.0, 1.0],
        ));
        bar.spawn(art(
            512.0 + 544.0 - 64.0,
            128.0,
            128.0,
            CAP,
            [1.0, 0.0, 0.0, 1.0],
        ));
        for i in 0..12 {
            bar.spawn((
                BarSlot(i),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(8.0 + i as f32 * (SLOT + 6.0)),
                    bottom: Val::Px(4.0),
                    width: Val::Px(SLOT),
                    height: Val::Px(SLOT),
                    ..default()
                },
            ));
        }
        bar.spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(552.0),
            bottom: Val::Px(4.0),
            width: Val::Px(211.0),
            height: Val::Px(34.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|strip| {
            strip.spawn((BarInfo(0), centered(f, "", 12.0, GOLD)));
            strip.spawn((BarInfo(1), centered(f, "", 11.0, Color::WHITE)));
        });
        for k in 0..4 {
            bar.spawn((
                LootSlot(k),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(939.5 - 42.0 * k as f32),
                    bottom: Val::Px(2.5),
                    width: Val::Px(SLOT),
                    height: Val::Px(SLOT),
                    ..default()
                },
            ));
        }
        for k in 0..super::super::bag::QUICK_SLOTS {
            let handle = sweeps.add(SweepUiMaterial { sweep: 0.0 });
            bar.spawn((
                QuickSlot(k),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(8.0 + (k + 1) as f32 * (SLOT + 6.0)),
                    bottom: Val::Px(ITEM_BAR_BOTTOM),
                    width: Val::Px(SLOT),
                    height: Val::Px(SLOT),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.02, 0.02, 0.85)),
                Interaction::default(),
                TipFor::Quick(k),
            ))
            .with_children(|slot| {
                // `UI-Quickslot`, an empty action button's socket, under the item.
                slot.spawn((
                    socket_art(UiArt::tinted(
                        "Interface\\Buttons\\UI-Quickslot",
                        Color::srgba(0.35, 0.35, 0.35, 0.8),
                    )),
                    ZIndex(-1),
                ));
                slot.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    MaterialNode(handle.clone()),
                    QuickSweep(k, handle),
                    ZIndex(1),
                ));
                slot.spawn((quickslot_frame(), ZIndex(2)));
                slot.spawn((hotkey(f, &(k + 1).to_string()), ZIndex(3)));
            });
        }
        bar.spawn((
            BackpackButton,
            Button,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(6.0),
                bottom: Val::Px(2.0),
                width: Val::Px(SLOT),
                height: Val::Px(SLOT),
                ..default()
            },
            Interaction::default(),
            TipFor::Backpack,
        ))
        .with_children(|slot| {
            slot.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ImageNode {
                    color: Color::NONE,
                    ..default()
                },
                UiArt::crop(
                    "Interface\\Buttons\\Button-Backpack-Up",
                    [0.0, 1.0, 0.0, 1.0],
                ),
            ));
            slot.spawn(quickslot_frame());
            slot.spawn(hotkey(f, "B"));
        });
        bar.spawn(Node {
            position_type: PositionType::Absolute,
            right: Val::Px(12.0),
            bottom: Val::Px(ITEM_BAR_BOTTOM + 4.0),
            column_gap: Val::Px(2.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            for k in 0..3u8 {
                let l = 0.25 * f32::from(k);
                row.spawn((MoneyText(k), text(f, String::new(), 13.0, Color::WHITE)));
                row.spawn((
                    MoneyCoin(k),
                    Node {
                        width: Val::Px(13.0),
                        height: Val::Px(13.0),
                        margin: UiRect::right(Val::Px(4.0)),
                        ..default()
                    },
                    ImageNode {
                        color: Color::NONE,
                        ..default()
                    },
                    UiArt::crop(
                        "Interface\\MoneyFrame\\UI-MoneyIcons",
                        [l, l + 0.25, 0.0, 1.0],
                    ),
                ));
            }
        });
        bar.spawn((
            MobilitySlot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                bottom: Val::Px(ITEM_BAR_BOTTOM),
                width: Val::Px(SLOT),
                height: Val::Px(SLOT),
                display: Display::None,
                ..default()
            },
            Interaction::default(),
            TipFor::Mobility,
        ))
        .with_children(|slot| {
            slot.spawn((
                MobilityIcon,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ImageNode {
                    color: Color::srgba(0.0, 0.0, 0.0, 0.6),
                    ..default()
                },
            ));
            let handle = sweeps.add(SweepUiMaterial { sweep: 0.0 });
            slot.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                MaterialNode(handle.clone()),
                MobilitySweep(handle),
            ));
            slot.spawn(quickslot_frame());
            slot.spawn(hotkey(f, "F"));
        });
    });
}

/// `ActionButtonTemplate`'s `HotKey`, NumberFontNormalSmallGray at TOPLEFT (−2,−2), 36×10,
/// justified right.
fn hotkey(f: &Handle<Font>, key: &str) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(2.0),
            top: Val::Px(2.0),
            ..default()
        },
        Text::new(key),
        TextFont {
            font: f.clone(),
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgb(0.6, 0.6, 0.6)),
        TextShadow {
            offset: Vec2::new(1.0, -1.0),
            color: Color::BLACK,
        },
    )
}

pub(super) fn spell_name(
    spells: Option<&crate::ui_action::Spells>,
    spell: u32,
    fallback: &str,
) -> String {
    spells
        .and_then(|s| s.catalog.get(spell))
        .map(|d| d.name.clone())
        .unwrap_or_else(|| fallback.replace('_', " "))
}

/// An item's icon, `Interface\Icons\<name>`.
pub(in crate::survivors) fn item_icon(
    assets: &mut benilla_assets::WorldAssets,
    images: &mut Assets<Image>,
    icon: &str,
) -> Option<Handle<Image>> {
    assets.sprite_texture(&format!("Interface\\Icons\\{icon}.blp"), images)
}

/// An item's icon and stack count, filling a 36-unit socket.
pub(in crate::survivors) fn item_face(
    b: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    icon: Option<Handle<Image>>,
    count: u32,
) {
    if let Some(icon) = icon {
        b.spawn((
            ImageNode::new(icon),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ));
    }
    if count > 1 {
        b.spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(2.0),
                bottom: Val::Px(2.0),
                ..default()
            },
            ZIndex(4),
        ))
        .with_children(|n| {
            n.spawn(text(f, count.to_string(), 12.0, Color::WHITE));
        });
    }
}

/// The item bar's icons when the bag changes, their cooldown sweeps, and the money frame.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(in crate::survivors) fn refresh_items(
    mut commands: Commands,
    run: Res<Run>,
    font: Res<HudFont>,
    slots: Query<(Entity, &QuickSlot)>,
    faces: Query<(Entity, &ChildOf), With<QuickFace>>,
    sweeps: Query<&QuickSweep>,
    mut mats: ResMut<Assets<SweepUiMaterial>>,
    mut money: ParamSet<(
        Query<(&MoneyText, &mut Text, &mut Node)>,
        Query<(&MoneyCoin, &mut Node)>,
    )>,
    mut assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut shown: Local<Option<u32>>,
) {
    let quick = run.bag.quick();
    for s in &sweeps {
        let f = quick.get(s.0).map_or(0.0, |&i| {
            let (left, full) = run.bag.cooldown(i);
            (left / full.max(0.01)).clamp(0.0, 1.0)
        });
        if let Some(m) = mats.get_mut(&s.1) {
            if m.sweep != f {
                m.sweep = f;
            }
        }
    }
    let c = run.bag.copper;
    let coins = [c / 10_000, c / 100 % 100, c % 100];
    // Leading empty coins hide, as the money frame's do; copper always shows.
    let first = coins.iter().position(|&n| n > 0).unwrap_or(2);
    let display = |k: u8| {
        if usize::from(k) >= first {
            Display::Flex
        } else {
            Display::None
        }
    };
    for (k, mut t, mut node) in &mut money.p0() {
        let want = coins[usize::from(k.0)].to_string();
        if t.0 != want {
            t.0 = want;
        }
        node.display = display(k.0);
    }
    for (k, mut node) in &mut money.p1() {
        node.display = display(k.0);
    }
    if *shown == Some(run.bag.version) {
        return;
    }
    let Some(assets) = assets.as_deref_mut() else {
        return;
    };
    *shown = Some(run.bag.version);
    for (e, _) in &faces {
        commands.entity(e).despawn();
    }
    for (slot, &QuickSlot(k)) in &slots {
        let Some((item, count)) = quick.get(k).map(|&i| &run.bag.slots[i]) else {
            continue;
        };
        let icon = item_icon(assets, &mut images, item.icon);
        let face = commands
            .spawn((
                QuickFace,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                bevy::ui::FocusPolicy::Pass,
            ))
            .with_children(|b| item_face(b, &font.0, icon, *count))
            .id();
        commands.entity(slot).add_child(face);
    }
}

#[derive(Component)]
pub(in crate::survivors) struct QuickFace;

/// Fill icon slots from `SpellIcon.dbc` once the client data and the asset reader are up.
pub(in crate::survivors) fn fill_icons(
    mut commands: Commands,
    mut slots: Query<(Entity, &mut ImageNode, &SpellIcon), Without<IconDone>>,
    spells: Option<Res<crate::ui_action::Spells>>,
    mut assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut cache: Local<HashMap<u32, Handle<Image>>>,
) {
    let (Some(spells), Some(assets)) = (spells.as_deref(), assets.as_deref_mut()) else {
        return;
    };
    for (e, mut img, icon) in &mut slots {
        let handle = match cache.get(&icon.0) {
            Some(h) => Some(h.clone()),
            None => {
                let path = spells
                    .catalog
                    .get(icon.0)
                    .and_then(|d| d.icon.clone())
                    .unwrap_or_else(|| "Interface\\Icons\\INV_Misc_QuestionMark".into());
                let h = assets.sprite_texture(&format!("{path}.blp"), &mut images);
                if let Some(h) = &h {
                    cache.insert(icon.0, h.clone());
                }
                h
            }
        };
        if let Some(h) = handle {
            img.image = h;
            img.color = Color::WHITE;
        }
        commands.entity(e).insert(IconDone);
    }
}

/// The mobility button: shown once learned, its icon the class's spell, its sweep the cooldown.
#[allow(clippy::type_complexity)]
pub(in crate::survivors) fn refresh_mobility(
    run: Res<Run>,
    mut commands: Commands,
    mut slot: Query<&mut Node, With<MobilitySlot>>,
    icon: Query<(Entity, Option<&SpellIcon>), With<MobilityIcon>>,
    sweep: Query<&MobilitySweep>,
    mut mats: ResMut<Assets<SweepUiMaterial>>,
) {
    let def = run.mobility();
    if let Ok(mut node) = slot.single_mut() {
        let display = if def.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    let Some(def) = def else { return };
    // A new spell re-arms the icon fill ([`fill_icons`]).
    if let Ok((e, have)) = icon.single() {
        if have.is_none_or(|s| s.0 != def.spell) {
            commands
                .entity(e)
                .remove::<IconDone>()
                .insert(SpellIcon(def.spell));
        }
    }
    if let Ok(s) = sweep.single() {
        let f = (run.mobile.cd / def.cooldown.max(0.01)).clamp(0.0, 1.0);
        if let Some(m) = mats.get_mut(&s.0) {
            if m.sweep != f {
                m.sweep = f;
            }
        }
    }
}

/// Drive each ability icon's cooldown sweep from its remaining cooldown.
pub(in crate::survivors) fn update_cooldown_sweeps(
    run: Res<Run>,
    mut mats: ResMut<Assets<SweepUiMaterial>>,
    sweeps: Query<&AbilitySweep>,
) {
    let stats = run.stats();
    for s in &sweeps {
        let Some(o) = run.abilities.get(s.index) else {
            continue;
        };
        let total = o.cooldown(&stats).max(0.01);
        let f = (o.cd / total).clamp(0.0, 1.0);
        if let Some(m) = mats.get_mut(&s.handle) {
            m.sweep = f;
        }
    }
}

/// The bar's recess: the next boss and when, the damage per second and the time to victory; and
/// the bag sockets: the armor this run has dropped, its item icons in `UI-Quickslot2`.
#[allow(clippy::type_complexity)]
pub(in crate::survivors) fn refresh_bar_info(
    mut commands: Commands,
    run: Res<Run>,
    selection: Res<Selection>,
    mut lines: Query<(&BarInfo, &mut Text)>,
    slots: Query<(Entity, &LootSlot)>,
    mut assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut shown: Local<Option<usize>>,
) {
    let bosses = &data::roster(selection.map).bosses;
    let n = ((run.elapsed / data::BOSS_EVERY_SECS) as usize).min(bosses.len());
    let clock = |secs: f32| {
        let s = secs.max(0.0) as u32;
        format!("{}:{:02}", s / 60, s % 60)
    };
    let boss = match bosses.get(n) {
        Some(b) => format!(
            "{} in {}",
            b.name,
            clock((n + 1) as f32 * data::BOSS_EVERY_SECS - run.elapsed)
        ),
        None if run.boss_kills as usize >= bosses.len() => "Every boss is down".to_string(),
        None => "The last boss is here".to_string(),
    };
    let dps = if run.elapsed > 1.0 {
        run.damage_done / f64::from(run.elapsed)
    } else {
        0.0
    };
    let pace = format!(
        "{} DPS   Victory in {}",
        compact(dps),
        clock(data::VICTORY_SECS - run.elapsed)
    );
    for (line, mut t) in &mut lines {
        let want = if line.0 == 0 { &boss } else { &pace };
        if t.0 != *want {
            t.0 = want.clone();
        }
    }
    if *shown == Some(run.loot.len()) {
        return;
    }
    let Some(assets) = assets.as_deref_mut() else {
        return;
    };
    *shown = Some(run.loot.len());
    for (slot, &LootSlot(k)) in &slots {
        commands.entity(slot).despawn_related::<Children>();
        let Some(piece) = run.loot.get(k) else {
            continue;
        };
        let icon = piece
            .icon
            .as_deref()
            .and_then(|i| assets.sprite_texture(&format!("{i}.blp"), &mut images));
        commands.entity(slot).with_children(|slot| {
            slot.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                Interaction::default(),
                TipFor::Loot(k),
            ))
            .with_children(|b| {
                if let Some(icon) = icon {
                    b.spawn((
                        ImageNode::new(icon),
                        Node {
                            width: Val::Px(SLOT),
                            height: Val::Px(SLOT),
                            ..default()
                        },
                    ));
                }
                b.spawn(quickslot_frame());
            });
        });
    }
}
