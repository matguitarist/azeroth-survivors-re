//! The world map, M as in the game: the zone around the battleground in its `Interface\WorldMap`
//! art, the twelve 256-px tiles of a 1002×668 page (`WorldMapFrame.xml`'s `WorldMapDetailTile1`
//! to `12`), over a paused battle. The battlefield's edge, the hero's arrow and what is left
//! standing and lying on it are drawn on top, as they stood when the map opened.
//!
//! The zone is the one the hero stands in, as the minimap's zone line reads it, and its
//! `WorldMapArea` row's rect places everything on the page. Every `WorldMapOverlay` of the zone is
//! drawn, as if all of it were explored.

use bevy::prelude::*;

use benilla_assets::{LockRecover, WorldAssets};
use benilla_formats::{
    load_world_map_area_catalog, load_world_map_overlay_catalog, WorldMapArea, WorldMapAreaCatalog,
    WorldMapOverlay, WorldMapOverlayCatalog,
};
use benilla_world::world_point::WorldPoint;

use super::bag::Pickup;
use super::enemies::Enemy;
use super::hero::Hero;
use super::hud::{centered_bundle, HudFont, PauseRoot, DIM_GREY, GOLD_TONE};
use super::panels::{dialog, PanelBuilt, PauseBox};
use super::{Phase, Selection};
use crate::area::AreaTableRes;
use crate::glue::art::GlueArt;
use crate::sound::GlueSound;

/// The page the art covers, in its own pixels: four tiles across, three down, the last row and
/// column cut short.
const PAGE: Vec2 = Vec2::new(1002.0, 668.0);
const TILE: f32 = 256.0;
/// The page on the 768-unit canvas.
const SCALE: f32 = 0.75;
const ARROW: f32 = 28.0;

/// Whether the map is open; it holds the battle paused while it is.
#[derive(Resource, Default)]
pub(super) struct MapOpen(pub(super) bool);

#[derive(Component)]
pub(super) struct MapBuilt;
/// The clipped page everything is drawn in, refilled at every opening.
#[derive(Component)]
pub(super) struct MapPage;
#[derive(Component)]
pub(super) struct MapTitle;

/// The map's dialog in the pause root, hidden until opened.
pub(super) fn build_map(
    mut commands: Commands,
    roots: Query<Entity, (With<PauseRoot>, With<PanelBuilt>, Without<MapBuilt>)>,
    art: Res<GlueArt>,
    font: Res<HudFont>,
) {
    let Ok(root) = roots.single() else { return };
    let f = &font.0;
    let size = PAGE * SCALE;
    commands
        .entity(root)
        .insert(MapBuilt)
        .with_children(|root| {
            let title = (MapTitle, centered_bundle(f, "", 14.0, GOLD_TONE));
            dialog(root, &art, size.x + 48.0, PauseBox::Map, title, |b| {
                b.spawn((
                    MapPage,
                    Node {
                        width: Val::Px(size.x),
                        height: Val::Px(size.y),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::BLACK),
                ));
                b.spawn(centered_bundle(f, "M or Escape to close", 11.0, DIM_GREY));
            });
        });
}

/// M opens it over the battle and shuts it again, as Escape does.
pub(super) fn map_input(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<State<Phase>>,
    mut next: ResMut<NextState<Phase>>,
    mut open: ResMut<MapOpen>,
    bag: Res<super::panels::BagOpen>,
    settings: Res<super::panels::SettingsOpen>,
    mut sounds: MessageWriter<GlueSound>,
) {
    match *phase.get() {
        Phase::Playing if keys.just_pressed(KeyCode::KeyM) => {
            sounds.write(GlueSound("igQuestLogOpen"));
            open.0 = true;
            next.set(Phase::Paused);
        }
        Phase::Paused if open.0 && !bag.0 && !settings.0 => {
            if keys.just_pressed(KeyCode::KeyM) || keys.just_pressed(KeyCode::Escape) {
                sounds.write(GlueSound("igQuestLogClose"));
                open.0 = false;
                next.set(Phase::Playing);
            }
        }
        Phase::Paused => {}
        _ => open.0 = false,
    }
}

/// The zone's page, the battlefield's edge, the hero and the field, drawn as the map opens.
#[allow(clippy::too_many_arguments)]
pub(super) fn fill_map(
    mut commands: Commands,
    open: Res<MapOpen>,
    selection: Res<Selection>,
    assets: Option<ResMut<WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
    pages: Query<Entity, With<MapPage>>,
    mut titles: Query<&mut Text, With<MapTitle>>,
    hero: Query<&Transform, With<Hero>>,
    enemies: Query<(&Transform, &Enemy)>,
    pickups: Query<(&Transform, &Pickup)>,
    world: WorldPoint,
    areas: Option<Res<AreaTableRes>>,
    mut catalog: Local<Option<Catalogs>>,
) {
    if !open.is_changed() || !open.0 {
        return;
    }
    let (Some(mut assets), Ok(page)) = (assets, pages.single()) else {
        return;
    };
    let catalog = catalog.get_or_insert_with(|| {
        let mut chain = assets.chain.lock_recover();
        Catalogs {
            areas: load_world_map_area_catalog(&mut chain)
                .inspect_err(|e| warn!("survivors: WorldMapArea.dbc: {e:#}"))
                .ok(),
            overlays: load_world_map_overlay_catalog(&mut chain)
                .inspect_err(|e| warn!("survivors: WorldMapOverlay.dbc: {e:#}"))
                .ok(),
        }
    });
    let def = selection.stage_def();
    for mut t in &mut titles {
        t.0 = def.name.to_string();
    }
    commands.entity(page).despawn_related::<Children>();
    // The zone the hero stands in: a subzone's parent, or the area itself.
    let standing = world.area().map(|leaf| {
        areas
            .as_ref()
            .and_then(|a| a.0.get(leaf))
            .map_or(
                leaf,
                |row| if row.zone_id == 0 { leaf } else { row.zone_id },
            )
    });
    let Some((id, zone)) = catalog
        .areas
        .as_ref()
        .and_then(|c| zone_for(c, def.map, standing, def.center))
    else {
        return;
    };
    let overlays = catalog
        .overlays
        .as_ref()
        .map_or(&[][..], |o| o.for_area(id));
    let assets = &mut *assets;
    let place = |p: Vec3| {
        let [x, y, _] = benilla_assets::coords::bevy_to_wow(p);
        project(zone, x, y) * PAGE * SCALE
    };

    commands.entity(page).with_children(|b| {
        let tile = TILE * SCALE;
        for i in 0..12 {
            let path = format!("Interface\\WorldMap\\{0}\\{0}{1}.blp", zone.name, i + 1);
            let Some(image) = assets.sprite_texture(&path, &mut images) else {
                continue;
            };
            let (col, row) = ((i % 4) as f32, (i / 4) as f32);
            b.spawn((
                ImageNode::new(image),
                at(Vec2::new(col, row) * tile, tile, tile),
            ));
        }

        for o in overlays {
            overlay(b, assets, &mut images, &zone.name, o);
        }

        // The leash: the ground the run is fought on.
        let center = place(super::hero::arena(def));
        let width = zone.loc_left - zone.loc_right;
        let height = zone.loc_top - zone.loc_bottom;
        let reach = Vec2::new(def.leash / width, def.leash / height) * PAGE * SCALE;
        b.spawn((
            Node {
                border: UiRect::all(Val::Px(2.0)),
                ..round(center - reach, reach.x * 2.0, reach.y * 2.0)
            },
            BorderColor::all(Color::srgba(1.0, 0.82, 0.0, 0.8)),
            BackgroundColor(Color::srgba(1.0, 0.82, 0.0, 0.06)),
        ));

        let dot = |b: &mut ChildSpawnerCommands, p: Vec2, size: f32, color: Color| {
            b.spawn((round(p - size / 2.0, size, size), BackgroundColor(color)));
        };
        for (tf, p) in &pickups {
            if let Some(color) = p.marker() {
                dot(b, place(tf.translation), 6.0, color);
            }
        }
        for (tf, e) in &enemies {
            if e.dead {
                continue;
            }
            let (size, color) = if e.boss {
                (10.0, Color::srgb(1.0, 0.5, 0.0))
            } else if e.rare {
                (8.0, Color::srgb(0.75, 0.4, 1.0))
            } else {
                (4.0, Color::srgb(0.9, 0.1, 0.1))
            };
            dot(b, place(tf.translation), size, color);
        }

        if let Ok(h) = hero.single() {
            let arrow = crate::minimap::blips::PLAYER_ARROW_TEXTURE;
            if let Some(image) = assets.sprite_texture(&format!("{arrow}.blp"), &mut images) {
                // North up and the yaw counter-clockwise from it, as on the minimap.
                let yaw = h.rotation.to_euler(EulerRot::YXZ).0;
                b.spawn((
                    ImageNode::new(image),
                    at(place(h.translation) - ARROW / 2.0, ARROW, ARROW),
                    UiTransform::from_rotation(Rot2::radians(-yaw)),
                ));
            }
        }
    });
}

/// The DBCs the map reads, loaded at its first opening.
pub(super) struct Catalogs {
    areas: Option<WorldMapAreaCatalog>,
    overlays: Option<WorldMapOverlayCatalog>,
}

/// One explored area's art, cut into 256-px pieces as `WorldMapFrame_Update` cuts it: numbered
/// across then down, the last row and column only as wide as what is left, each in a file
/// rounded up to a power of two.
fn overlay(
    b: &mut ChildSpawnerCommands,
    assets: &mut WorldAssets,
    images: &mut Assets<Image>,
    zone: &str,
    o: &WorldMapOverlay,
) {
    if o.texture_name.is_empty() || o.texture_width == 0 || o.texture_height == 0 {
        return;
    }
    let (w, h) = (o.texture_width as f32, o.texture_height as f32);
    let across = (w / TILE).ceil() as u32;
    let down = (h / TILE).ceil() as u32;
    for j in 0..down {
        for k in 0..across {
            let path = format!(
                "Interface\\WorldMap\\{zone}\\{}{}.blp",
                o.texture_name,
                j * across + k + 1
            );
            let Some(image) = assets.sprite_texture(&path, images) else {
                continue;
            };
            let pw = (w - k as f32 * TILE).min(TILE);
            let ph = (h - j as f32 * TILE).min(TILE);
            let p = Vec2::new(
                o.offset_x as f32 + k as f32 * TILE,
                o.offset_y as f32 + j as f32 * TILE,
            );
            b.spawn((
                ImageNode {
                    rect: Some(Rect::new(0.0, 0.0, pw, ph)),
                    ..ImageNode::new(image)
                },
                at(p * SCALE, pw * SCALE, ph * SCALE),
            ));
        }
    }
}

/// An absolutely placed box on the page.
fn at(p: Vec2, w: f32, h: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(p.x),
        top: Val::Px(p.y),
        width: Val::Px(w),
        height: Val::Px(h),
        ..default()
    }
}

/// The same, an ellipse.
fn round(p: Vec2, w: f32, h: f32) -> Node {
    Node {
        border_radius: BorderRadius::MAX,
        ..at(p, w, h)
    }
}

/// Where WoW `(x, y)` falls on the zone's page, 0 to 1 across and down: x runs north (up the
/// page), y west (left).
fn project(zone: &WorldMapArea, x: f32, y: f32) -> Vec2 {
    Vec2::new(
        (zone.loc_left - y) / (zone.loc_left - zone.loc_right),
        (zone.loc_top - x) / (zone.loc_top - zone.loc_bottom),
    )
}

/// The page of the zone `standing` (an `AreaTable` id), else the smallest zone page of `map` that
/// holds `center` (WoW coordinates): the rects overlap, Mulgore's reaching into The Barrens.
fn zone_for(
    catalog: &WorldMapAreaCatalog,
    map: u32,
    standing: Option<u32>,
    center: [f32; 3],
) -> Option<(u32, &WorldMapArea)> {
    let pages = || {
        catalog
            .iter()
            .filter(move |(_, a)| a.map_id == map && a.area_id != 0)
    };
    if let Some(found) = pages().find(|(_, a)| Some(a.area_id) == standing) {
        return Some(found);
    }
    pages()
        .filter(|(_, a)| {
            let p = project(a, center[0], center[1]);
            (0.0..=1.0).contains(&p.x) && (0.0..=1.0).contains(&p.y)
        })
        .min_by(|(_, a), (_, b)| {
            let area = |z: &WorldMapArea| (z.loc_left - z.loc_right) * (z.loc_top - z.loc_bottom);
            area(a).total_cmp(&area(b))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(left: f32, right: f32, top: f32, bottom: f32) -> WorldMapArea {
        WorldMapArea {
            map_id: 1,
            area_id: 17,
            name: "Barrens".into(),
            loc_left: left,
            loc_right: right,
            loc_top: top,
            loc_bottom: bottom,
        }
    }

    #[test]
    fn north_is_up_and_west_is_left() {
        let z = zone(100.0, -100.0, 200.0, 0.0);
        assert_eq!(project(&z, 200.0, 100.0), Vec2::ZERO);
        assert_eq!(project(&z, 0.0, -100.0), Vec2::ONE);
        assert_eq!(project(&z, 100.0, 0.0), Vec2::splat(0.5));
    }
}
