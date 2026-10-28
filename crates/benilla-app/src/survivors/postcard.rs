//! Zone postcards: the loading screen's wide art, taken from the game's own picture of each zone.
//!
//! 1.12 has one 4:3 loading screen per continent, so every zone loaded behind the same low-resolution
//! Kalimdor or Eastern Kingdoms art. Instead, the first time a zone has stood settled behind the pick
//! screen for [`SETTLE`] seconds, one frame is drawn without the menu and the hero and captured: the
//! zone at the window's own size and shape, sky, ground and light as this machine renders them. It is
//! kept as `benilla-config/Postcards/<zone>.jpg` and shown edge to edge while that zone loads
//! ([`crate::loading_screen::LoadingScreen::set_wide_art`]), under the zone's name.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};

use super::hero::Hero;
use super::hud::{HudFont, MenuRoot};
use super::{data, Phase, Selection};
use crate::glue::art::GOLD;

/// Seconds a zone stands settled behind the pick screen before its postcard is taken.
const SETTLE: f32 = 4.0;
/// A postcard is kept at most this wide: sharp on a 1440p screen, a few hundred kilobytes.
const KEEP_W: u32 = 2560;
const QUALITY: u8 = 88;

#[derive(Resource, Default)]
pub(super) struct Postcards {
    /// Each zone's art once looked for, `None` when it has none yet.
    loaded: HashMap<usize, Option<Handle<Image>>>,
    /// Zones shot this session; a shot is taken once even if its file could not be written.
    taken: HashSet<usize>,
    /// Zones whose file was written since they were looked for, to look again.
    written: Arc<Mutex<Vec<usize>>>,
    /// The zone standing behind the menu, and how long it has stood settled.
    stage: usize,
    still: f32,
    /// The frame drawn for a shot is up: the menu and the hero come back next frame.
    hiding: bool,
}

/// The zone's name over the loading screen.
#[derive(Component)]
pub(super) struct PostcardTitle;
#[derive(Component)]
pub(super) struct PostcardTitleText;
#[derive(Component)]
pub(super) struct PostcardTitleLine;

fn path(map: usize) -> Option<PathBuf> {
    let name: String = data::MAPS[map]
        .name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    crate::local_state::home().map(|h| h.join("Postcards").join(format!("{name}.jpg")))
}

/// A zone's postcard from disk, if it has one.
fn load(map: usize, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    let bytes = std::fs::read(path(map)?).ok()?;
    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg)
        .inspect_err(|e| warn!("survivors postcard for {}: {e}", data::MAPS[map].name))
        .ok()?;
    Some(images.add(Image::from_dynamic(
        img,
        true,
        RenderAssetUsages::RENDER_WORLD,
    )))
}

/// The loading screen shows the postcard of the zone the world is going to, and its name.
#[allow(clippy::type_complexity)]
pub(super) fn show_postcard(
    mut commands: Commands,
    selection: Res<Selection>,
    mut cards: ResMut<Postcards>,
    mut images: ResMut<Assets<Image>>,
    mut screen: ResMut<crate::loading_screen::LoadingScreen>,
    font: Res<HudFont>,
    mut title: Query<&mut Visibility, With<PostcardTitle>>,
    mut texts: ParamSet<(
        Query<&mut Text, With<PostcardTitleText>>,
        Query<&mut Text, With<PostcardTitleLine>>,
    )>,
) {
    let written: Vec<usize> = std::mem::take(&mut *cards.written.lock().unwrap());
    for map in written {
        cards.loaded.remove(&map);
    }
    let stage = selection.stage.min(data::MAPS.len() - 1);
    let art = cards
        .loaded
        .entry(stage)
        .or_insert_with(|| load(stage, &mut images))
        .clone();
    screen.set_wide_art(art);

    if title.is_empty() {
        commands
            .spawn((
                PostcardTitle,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Percent(7.0),
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                // Over the loading screen's root (1000).
                GlobalZIndex(1001),
                Visibility::Hidden,
                bevy::ui::FocusPolicy::Pass,
            ))
            .with_children(|t| {
                t.spawn((
                    PostcardTitleText,
                    super::hud::centered_bundle(&font.0, "", 34.0, GOLD),
                ));
                t.spawn((
                    PostcardTitleLine,
                    super::hud::centered_bundle(&font.0, "", 15.0, Color::WHITE),
                ));
            });
        return;
    }
    // Over a postcard only: the 1.12 art carries its own logo where the title would go.
    let up = screen.swallows_input() && cards.loaded.get(&stage).is_some_and(Option::is_some);
    for mut vis in &mut title {
        let want = if up {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
    let map = &data::MAPS[stage];
    for mut t in &mut texts.p0() {
        if t.0 != map.name {
            t.0 = map.name.to_string();
        }
    }
    for mut t in &mut texts.p1() {
        if t.0 != map.zone {
            t.0 = map.zone.to_string();
        }
    }
}

/// Take a zone's postcard once it has stood settled behind the pick screen: for one frame the menu
/// and the hero are hidden and the window is captured; a thread scales it and writes the file.
#[allow(clippy::too_many_arguments)]
pub(super) fn take_postcard(
    mut commands: Commands,
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    mut cards: ResMut<Postcards>,
    veil: Res<super::menu::StageVeil>,
    screen: Res<crate::loading_screen::LoadingScreen>,
    progress: Res<benilla_world::terrain_stream::WorldLoadProgress>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut menu: Query<&mut Visibility, With<MenuRoot>>,
    // Shrunk to nothing for the shot, not hidden: the body's model parts keep their own
    // visibility, and a move would move the camera framing it.
    mut heroes: Query<&mut Transform, With<Hero>>,
) {
    if cards.hiding {
        cards.hiding = false;
        for mut tf in &mut heroes {
            tf.scale = Vec3::ONE;
        }
        return;
    }
    if cards.stage != selection.stage {
        cards.stage = selection.stage;
        cards.still = 0.0;
    }
    let settled = *phase.get() == Phase::Menu
        && selection.map == selection.stage
        && veil.alpha() == 0.0
        && !screen.covering()
        && progress.is_ready()
        && progress.presentable()
        && !heroes.is_empty()
        && !mouse.pressed(MouseButton::Left);
    if !settled {
        cards.still = 0.0;
        return;
    }
    cards.still += time.delta_secs();
    let stage = cards.stage.min(data::MAPS.len() - 1);
    let have = cards.loaded.get(&stage).is_some_and(Option::is_some);
    if cards.still < SETTLE || have || cards.taken.contains(&stage) {
        return;
    }
    let Some(file) = path(stage) else { return };
    cards.taken.insert(stage);
    cards.hiding = true;
    for mut vis in &mut menu {
        *vis = Visibility::Hidden;
    }
    for mut tf in &mut heroes {
        tf.scale = Vec3::ZERO;
    }
    let written = cards.written.clone();
    commands
        .spawn(Screenshot::primary_window())
        .observe(move |shot: On<ScreenshotCaptured>| {
            let Ok(img) = shot.image.clone().try_into_dynamic() else {
                warn!("survivors postcard: the capture has no readable format");
                return;
            };
            let (file, written) = (file.clone(), written.clone());
            // Scaling and encoding a 5K frame takes a while; the game keeps running meanwhile.
            std::thread::spawn(move || {
                let img = if img.width() > KEEP_W {
                    let h = (u64::from(img.height()) * u64::from(KEEP_W) / u64::from(img.width()))
                        as u32;
                    img.resize_exact(KEEP_W, h, image::imageops::FilterType::Triangle)
                } else {
                    img
                };
                let mut out = Vec::new();
                let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, QUALITY);
                if let Err(e) = img.to_rgb8().write_with_encoder(encoder) {
                    warn!("survivors postcard: {e}");
                    return;
                }
                if let Some(dir) = file.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                match crate::local_state::write_atomic_bytes(&file, &out) {
                    Ok(()) => {
                        info!("survivors postcard: {}", file.display());
                        written.lock().unwrap().push(stage);
                    }
                    Err(e) => warn!("survivors postcard: {}: {e}", file.display()),
                }
            });
        });
}

/// The world loads straight into the picked character's zone, not the first zone and then theirs.
pub(super) fn boot_stage(profile: Res<super::profile::Profile>, mut selection: ResMut<Selection>) {
    if let Some(hero) = profile.hero() {
        let map = if profile.is_open(hero.map) {
            hero.map
        } else {
            super::profile::zone_order().first().map_or(0, |z| z.map)
        };
        selection.map = map;
        selection.stage = map;
        selection.version += 1;
    }
}
