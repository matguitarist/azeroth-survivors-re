//! The pause and results screens as the game's own dialogs: `GameMenuFrame`'s `UI-DialogBox`
//! backdrop under its `UI-DialogBox-Header` plaque, with the glue screens' red buttons
//! ([`crate::glue::widgets::glue_button`]), so they match the pick screen. Built into
//! [`PauseRoot`] and [`OverRoot`] once the glue art is in. The pause menu's Settings swaps in a
//! second dialog, tabbed as 1.12 splits its Video and Sound options: each row writes the engine's
//! own CVar, which applies it and saves it with the config; the latched display rows are
//! committed at once, as the options' Okay does with `RestartGx()`. The Controls tab holds the
//! camera lock, saved beside the profile, and every control.

use bevy::prelude::*;

use super::hud::{
    centered_bundle, text_bundle, HudFont, OverLine, OverRoot, OverStat, OverStats, OverTitle,
    PauseRoot, UiAction, DIM_GREY, GOLD_TONE,
};
use super::Phase;
use crate::glue::art::GlueArt;
use crate::glue::backdrop::{backdrop_border, tiled_bg_node};
use crate::glue::widgets::{abs, dial_arrow, glue_button, GlueBtnKind, GlueDisabled};
use crate::glue::GlueClicks;

/// `GameMenuFrame`'s width, widened to the glue buttons.
const PAUSE_W: f32 = 240.0;
/// Room for two `GlueDialogButtonTemplate`s with the dialog's 13 gap, and the stat table.
const OVER_W: f32 = 460.0;

/// The Settings dialog's width.
const SETTINGS_W: f32 = 340.0;

/// On a root once its dialog is built.
#[derive(Component)]
pub(super) struct PanelBuilt;

/// Whether the pause menu shows Settings in place of its buttons.
#[derive(Resource, Default)]
pub(super) struct SettingsOpen(pub(super) bool);

/// Settings' tabs, and the one shown.
#[derive(Component, Resource, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(super) enum Tab {
    #[default]
    Video,
    Sound,
    Controls,
}

const TABS: [(Tab, &str); 3] = [
    (Tab::Video, "Video"),
    (Tab::Sound, "Sound"),
    (Tab::Controls, "Controls"),
];

/// The pause root's two dialogs.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum PauseBox {
    Menu,
    Settings,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Setting {
    /// One of [`VOLUMES`].
    Volume(usize),
    /// One of [`CHOICES`].
    Choice(usize),
    /// Fullscreen, windowed or maximized: `gxWindow` and `gxMaximize` together.
    Display,
    Camera,
}

/// A video row: its CVar and the stops it steps through, `(value, shown)`. A latched row is
/// committed the moment it moves.
struct Choice {
    label: &'static str,
    cvar: &'static str,
    stops: &'static [(&'static str, &'static str)],
    latched: bool,
}

const OFF_ON: &[(&str, &str)] = &[("0", "Off"), ("1", "On")];
const LOW_HIGH: &[(&str, &str)] = &[("0", "Low"), ("1", "Medium"), ("2", "High")];

/// The 1.12 Video Options the engine applies, and its own render scale.
const CHOICES: [Choice; 9] = [
    Choice {
        label: "Vertical Sync",
        cvar: "gxVSync",
        stops: OFF_ON,
        latched: true,
    },
    Choice {
        label: "Anti-aliasing *",
        cvar: "gxMultisample",
        stops: &[("1", "Off"), ("2", "2x"), ("4", "4x"), ("8", "8x")],
        latched: true,
    },
    Choice {
        label: "Render Scale",
        cvar: "renderScale",
        stops: &[
            ("0.5", "50%"),
            ("0.75", "75%"),
            ("1", "100%"),
            ("1.5", "150%"),
            ("2", "200%"),
        ],
        latched: false,
    },
    Choice {
        label: "View Distance",
        cvar: "farclip",
        stops: &[
            ("177", "177 yd"),
            ("277", "277 yd"),
            ("350", "350 yd"),
            ("477", "477 yd"),
            ("577", "577 yd"),
            ("677", "677 yd"),
            ("777", "777 yd"),
        ],
        latched: false,
    },
    Choice {
        label: "Ground Clutter",
        cvar: "WorldDetail",
        stops: LOW_HIGH,
        latched: false,
    },
    Choice {
        label: "Spell Detail",
        cvar: "spellEffectLevel",
        stops: LOW_HIGH,
        latched: false,
    },
    Choice {
        label: "Weather",
        cvar: "weatherDensity",
        stops: &[("0", "Low"), ("1", "Medium"), ("2", "High"), ("3", "Ultra")],
        latched: false,
    },
    Choice {
        label: "Full-Screen Glow",
        cvar: "ffxGlow",
        stops: OFF_ON,
        latched: false,
    },
    Choice {
        label: "Texture Filtering *",
        cvar: "anisotropic",
        stops: &[
            ("1", "Bilinear"),
            ("2", "2x Aniso"),
            ("4", "4x Aniso"),
            ("8", "8x Aniso"),
            ("16", "16x Aniso"),
        ],
        latched: false,
    },
];

const DISPLAY_MODES: [&str; 3] = ["Fullscreen", "Windowed", "Maximized"];

/// The stop nearest a CVar's value.
fn stop_of(cvars: &crate::cvars::Cvars, c: &Choice) -> usize {
    let now = cvars.num(c.cvar).unwrap_or(0.0);
    (0..c.stops.len())
        .min_by(|&a, &b| {
            let d = |i: usize| (c.stops[i].0.parse::<f32>().unwrap_or(0.0) - now).abs();
            d(a).total_cmp(&d(b))
        })
        .unwrap_or(0)
}

fn display_of(cvars: &crate::cvars::Cvars) -> usize {
    match (cvars.flag("gxWindow"), cvars.flag("gxMaximize")) {
        (Some(true), Some(true)) => 2,
        (Some(true), _) => 1,
        _ => 0,
    }
}

/// A setting's arrow and its step.
#[derive(Component, Clone, Copy)]
pub(super) struct SettingStep(Setting, i8);

/// Where a setting stands, between its arrows.
#[derive(Component)]
pub(super) struct SettingValue(Setting);

/// The volumes, by the Sound options panel's names, and their CVars.
const VOLUMES: [(&str, &str); 4] = [
    ("Master Volume", "MasterVolume"),
    ("Sound Effects", "SoundVolume"),
    ("Music", "MusicVolume"),
    ("Ambience", "AmbienceVolume"),
];

/// The dialog frame: the background tiled at 32 inside (11,12,12,11), the 32-edge border over
/// it, and the header plaque (256×64 at TOP (0,12)) with its title 14 down in it.
fn dialog(
    root: &mut ChildSpawnerCommands,
    art: &GlueArt,
    w: f32,
    marker: impl Bundle,
    title: impl Bundle,
    body: impl FnOnce(&mut ChildSpawnerCommands),
) {
    root.spawn((
        marker,
        Node {
            width: Val::Px(w),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::new(Val::ZERO, Val::ZERO, Val::Px(34.0), Val::Px(22.0)),
            row_gap: Val::Px(8.0),
            ..default()
        },
    ))
    .with_children(|b| {
        if let (Some(bg), Some(border)) = (&art.dialog_bg, &art.dialog_border) {
            // A dark fill under the translucent tiles, so the fight doesn't show through.
            b.spawn((
                tiled_bg_node(bg.clone(), 32.0, 1.0, Color::WHITE),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(11.0),
                    right: Val::Px(12.0),
                    top: Val::Px(12.0),
                    bottom: Val::Px(11.0),
                    ..default()
                },
            ));
            backdrop_border(b, border, 32.0, Color::WHITE);
        } else {
            b.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.05, 0.08, 0.95)),
            ));
        }
        let left = (w - 256.0) / 2.0;
        if let Some((header, _)) = &art.dialog_header {
            b.spawn((
                ImageNode::new(header.clone()),
                abs(1.0, left, -12.0, 256.0, 64.0),
            ));
        }
        b.spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(left),
            width: Val::Px(256.0),
            top: Val::Px(1.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|t| {
            t.spawn(title);
        });
        body(b);
    });
}

pub(super) fn build_panels(
    mut commands: Commands,
    pause: Query<Entity, (With<PauseRoot>, Without<PanelBuilt>)>,
    over: Query<Entity, (With<OverRoot>, Without<PanelBuilt>)>,
    assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut art: ResMut<GlueArt>,
    mut images: ResMut<Assets<Image>>,
    mut add_mats: ResMut<Assets<crate::glue::add_material::AddUiMaterial>>,
    font: Res<HudFont>,
) {
    if pause.is_empty() && over.is_empty() {
        return;
    }
    let Some(mut assets) = assets else { return };
    art.ensure_loaded(&mut assets, &mut images, &mut add_mats);
    let f = &font.0;
    let art = &*art;

    for root in &pause {
        commands
            .entity(root)
            .insert(PanelBuilt)
            .with_children(|root| {
                let title = centered_bundle(f, "Paused", 14.0, GOLD_TONE);
                dialog(root, art, PAUSE_W, PauseBox::Menu, title, |b| {
                    for (label, action) in [
                        ("Return to Game", UiAction::Resume),
                        ("Settings", UiAction::Settings),
                        ("Abandon Run", UiAction::MainMenu),
                        ("Exit Game", UiAction::Quit),
                    ] {
                        glue_button(
                            b,
                            art,
                            f,
                            action,
                            label,
                            180.0,
                            34.0,
                            GlueBtnKind::Small,
                            1.0,
                        );
                    }
                });
                let title = centered_bundle(f, "Settings", 14.0, GOLD_TONE);
                // Hidden by [`settings`] until opened; the pause root is hidden in play anyway.
                dialog(root, art, SETTINGS_W, PauseBox::Settings, title, |b| {
                    b.spawn(Node {
                        column_gap: Val::Px(4.0),
                        margin: UiRect::bottom(Val::Px(4.0)),
                        ..default()
                    })
                    .with_children(|row| {
                        for (i, (_, label)) in TABS.iter().enumerate() {
                            glue_button(
                                row,
                                art,
                                f,
                                UiAction::Tab(i as u8),
                                label,
                                96.0,
                                30.0,
                                GlueBtnKind::Small,
                                1.0,
                            );
                        }
                    });
                    let section = |tab: Tab| {
                        (
                            tab,
                            Node {
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                        )
                    };
                    b.spawn(section(Tab::Video)).with_children(|v| {
                        setting_row(v, art, f, Setting::Display, "Display");
                        for (i, c) in CHOICES.iter().enumerate() {
                            setting_row(v, art, f, Setting::Choice(i), c.label);
                        }
                        v.spawn(Node {
                            margin: UiRect::top(Val::Px(4.0)),
                            ..default()
                        })
                        .with_children(|n| {
                            n.spawn(centered_bundle(
                                f,
                                "* applies on the next launch",
                                11.0,
                                DIM_GREY,
                            ));
                        });
                    });
                    b.spawn(section(Tab::Sound)).with_children(|v| {
                        for (i, (label, _)) in VOLUMES.iter().enumerate() {
                            setting_row(v, art, f, Setting::Volume(i), label);
                        }
                    });
                    b.spawn(section(Tab::Controls)).with_children(|v| {
                        setting_row(v, art, f, Setting::Camera, "Camera (C)");
                        controls(v, f, "Keyboard and mouse", &CONTROLS);
                        controls(v, f, "Controller", &PAD_CONTROLS);
                    });
                    b.spawn(Node {
                        margin: UiRect::top(Val::Px(6.0)),
                        ..default()
                    })
                    .with_children(|r| {
                        glue_button(
                            r,
                            art,
                            f,
                            UiAction::Done,
                            "Done",
                            180.0,
                            34.0,
                            GlueBtnKind::Small,
                            1.0,
                        );
                    });
                });
            });
    }

    for root in &over {
        commands
            .entity(root)
            .insert(PanelBuilt)
            .with_children(|root| {
                let title = (OverTitle, centered_bundle(f, "", 14.0, GOLD_TONE));
                dialog(root, art, OVER_W, (), title, |b| {
                    b.spawn(Node {
                        max_width: Val::Px(OVER_W - 60.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .with_children(|n| {
                        n.spawn((OverLine, centered_bundle(f, "", 14.0, Color::WHITE)));
                    });
                    // The run in numbers: GameFontNormal labels, GameFontHighlight values.
                    b.spawn(Node {
                        width: Val::Px(280.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        margin: UiRect::vertical(Val::Px(6.0)),
                        ..default()
                    })
                    .with_children(|table| {
                        for (i, label) in ["Survived", "Level", "Kills", "Damage", "Bosses"]
                            .into_iter()
                            .enumerate()
                        {
                            table
                                .spawn(Node {
                                    justify_content: JustifyContent::SpaceBetween,
                                    ..default()
                                })
                                .with_children(|row| {
                                    row.spawn(text_bundle(f, label, 14.0, GOLD_TONE));
                                    row.spawn((
                                        OverStat(i),
                                        text_bundle(f, "", 14.0, Color::WHITE),
                                    ));
                                });
                        }
                    });
                    b.spawn((OverStats, centered_bundle(f, "", 12.0, DIM_GREY)));
                    b.spawn(Node {
                        column_gap: Val::Px(13.0),
                        margin: UiRect::top(Val::Px(6.0)),
                        ..default()
                    })
                    .with_children(|row| {
                        for (label, action) in [
                            ("Play Again", UiAction::MainMenu),
                            ("Exit Game", UiAction::Quit),
                        ] {
                            glue_button(
                                row,
                                art,
                                f,
                                action,
                                label,
                                190.0,
                                38.0,
                                GlueBtnKind::Dialog,
                                1.0,
                            );
                        }
                    });
                });
            });
    }
}

/// What each input does in a run, as the Key Bindings panel lists them.
const CONTROLS: [(&str, &str); 9] = [
    ("Move", "W A S D / Arrows"),
    ("Run forward", "Both mouse buttons"),
    ("Jump", "Space"),
    ("Class movement spell", "F"),
    ("Lock / free camera", "C"),
    ("Turn camera (free)", "Drag either button"),
    ("Zoom", "Mouse wheel"),
    ("Pick a level-up card", "1  2  3"),
    ("Pause", "Escape"),
];

/// The same on a controller ([`super::pad`]).
const PAD_CONTROLS: [(&str, &str); 7] = [
    ("Move", "Left stick"),
    ("Turn / zoom camera", "Right stick"),
    ("Jump", "A"),
    ("Class movement spell", "X / RB"),
    ("Lock / free camera", "Y"),
    ("Pick a level-up card", "X  Y  B"),
    ("Pause / play again", "Start"),
];

/// Controls under a GameFontNormal heading: each action in gold, its input in white.
fn controls(b: &mut ChildSpawnerCommands, f: &Handle<Font>, heading: &str, list: &[(&str, &str)]) {
    b.spawn(Node {
        margin: UiRect::top(Val::Px(8.0)),
        ..default()
    })
    .with_children(|h| {
        h.spawn(centered_bundle(f, heading, 14.0, GOLD_TONE));
    });
    b.spawn(Node {
        width: Val::Px(260.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(2.0),
        ..default()
    })
    .with_children(|table| {
        for &(action, input) in list {
            table
                .spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(text_bundle(f, action, 12.0, GOLD_TONE));
                    row.spawn(text_bundle(f, input, 12.0, Color::WHITE));
                });
        }
    });
}

/// A setting's row: its name, then the pick screen's dial arrows around where it stands.
fn setting_row(
    b: &mut ChildSpawnerCommands,
    art: &GlueArt,
    f: &Handle<Font>,
    setting: Setting,
    label: &str,
) {
    b.spawn(Node {
        width: Val::Px(282.0),
        height: Val::Px(30.0),
        ..default()
    })
    .with_children(|row| {
        row.spawn(Node {
            align_items: AlignItems::Center,
            ..abs(1.0, 0.0, 0.0, 130.0, 32.0)
        })
        .with_children(|n| {
            n.spawn(text_bundle(f, label, 13.0, GOLD_TONE));
        });
        dial_arrow(
            row,
            &art.arrow_left,
            f,
            SettingStep(setting, -1),
            130.0,
            "<",
            1.0,
        );
        row.spawn(Node {
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..abs(1.0, 158.0, 0.0, 92.0, 32.0)
        })
        .with_children(|n| {
            n.spawn((
                SettingValue(setting),
                centered_bundle(f, "", 13.0, Color::WHITE),
            ));
        });
        dial_arrow(
            row,
            &art.arrow_right,
            f,
            SettingStep(setting, 1),
            250.0,
            ">",
            1.0,
        );
    });
}

/// The settings' arrows, what they read, and which pause dialog shows.
#[allow(clippy::too_many_arguments)]
pub(super) fn settings(
    clicks: Res<GlueClicks>,
    open: Res<SettingsOpen>,
    tab: Res<Tab>,
    steps: Query<(Entity, &SettingStep)>,
    mut values: Query<(&SettingValue, &mut Text)>,
    mut boxes: Query<(&PauseBox, &mut Node), Without<Tab>>,
    mut sections: Query<(&Tab, &mut Node), Without<PauseBox>>,
    mut tab_buttons: Query<(&UiAction, &mut GlueDisabled)>,
    mut cvars: Option<ResMut<crate::cvars::Cvars>>,
    mut rig: ResMut<super::hero::CamRig>,
) {
    let shown = |node: &mut Node, show: bool| {
        let want = if show { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    };
    for (b, mut node) in &mut boxes {
        shown(&mut node, (*b == PauseBox::Settings) == open.0);
    }
    if !open.0 {
        return;
    }
    for (t, mut node) in &mut sections {
        shown(&mut node, *t == *tab);
    }
    // The tab on show reads as pressed in.
    for (action, mut disabled) in &mut tab_buttons {
        if let UiAction::Tab(i) = *action {
            let on = TABS.get(usize::from(i)).is_some_and(|t| t.0 == *tab);
            if disabled.0 != on {
                disabled.0 = on;
            }
        }
    }
    // A volume steps by a tenth, as the Sound panel's sliders do.
    let volume = |cvars: &crate::cvars::Cvars, i: usize| {
        cvars.num(VOLUMES[i].1).unwrap_or(1.0).clamp(0.0, 1.0)
    };
    for (e, step) in &steps {
        if !clicks.hit(e) {
            continue;
        }
        match step.0 {
            Setting::Volume(i) => {
                if let Some(cvars) = cvars.as_deref_mut() {
                    let tenths = (volume(cvars, i) * 10.0).round() as i32 + i32::from(step.1);
                    let v = tenths.clamp(0, 10) as f32 / 10.0;
                    cvars.set(VOLUMES[i].1, &format!("{v}"));
                }
            }
            Setting::Choice(i) => {
                if let Some(cvars) = cvars.as_deref_mut() {
                    let c = &CHOICES[i];
                    let at = stop_of(cvars, c) as i32 + i32::from(step.1);
                    let at = at.clamp(0, c.stops.len() as i32 - 1) as usize;
                    cvars.set(c.cvar, c.stops[at].0);
                    if c.latched {
                        cvars.commit_latched();
                    }
                }
            }
            Setting::Display => {
                if let Some(cvars) = cvars.as_deref_mut() {
                    let at = (display_of(cvars) as i32 + i32::from(step.1)).clamp(0, 2);
                    cvars.set("gxWindow", if at == 0 { "0" } else { "1" });
                    cvars.set("gxMaximize", if at == 2 { "1" } else { "0" });
                    cvars.commit_latched();
                }
            }
            Setting::Camera => rig.toggle_mode(),
        }
    }
    for (value, mut t) in &mut values {
        let want = match value.0 {
            Setting::Volume(i) => match cvars.as_deref() {
                Some(c) => format!("{:.0}%", volume(c, i) * 100.0),
                None => "—".to_string(),
            },
            Setting::Choice(i) => match cvars.as_deref() {
                Some(c) => CHOICES[i].stops[stop_of(c, &CHOICES[i])].1.to_string(),
                None => "—".to_string(),
            },
            Setting::Display => match cvars.as_deref() {
                Some(c) => DISPLAY_MODES[display_of(c)].to_string(),
                None => "—".to_string(),
            },
            Setting::Camera => match rig.mode {
                super::hero::CamMode::Fixed => "Locked",
                super::hero::CamMode::Orbit => "Free",
            }
            .to_string(),
        };
        if t.0 != want {
            t.0 = want;
        }
    }
}

/// The dialogs' buttons fire on the click, as the glue screens' do.
pub(super) fn buttons(
    clicks: Res<GlueClicks>,
    actions: Query<(Entity, &UiAction)>,
    phase: Res<State<Phase>>,
    mut next: ResMut<NextState<Phase>>,
    mut exit: MessageWriter<AppExit>,
    mut open: ResMut<SettingsOpen>,
    mut tab: ResMut<Tab>,
) {
    for (e, action) in &actions {
        if !clicks.hit(e) {
            continue;
        }
        match (*action, *phase.get()) {
            (UiAction::Resume, Phase::Paused) => next.set(Phase::Playing),
            (UiAction::Settings, Phase::Paused) => open.0 = true,
            (UiAction::Done, _) => open.0 = false,
            (UiAction::Tab(i), _) => {
                if let Some(&(t, _)) = TABS.get(usize::from(i)) {
                    *tab = t;
                }
            }
            (UiAction::MainMenu, Phase::Paused | Phase::GameOver) => next.set(Phase::Menu),
            (UiAction::Quit, Phase::Paused | Phase::GameOver) => {
                exit.write(AppExit::Success);
            }
            _ => {}
        }
    }
}

/// The camera lock, saved as `benilla-config/survivors-settings.txt` (`camera locked|free`), so it
/// holds across launches as the CVars do.
fn settings_path() -> Option<std::path::PathBuf> {
    crate::local_state::home().map(|h| h.join("survivors-settings.txt"))
}

/// Load the saved camera lock once, then save it whenever it moves (the C key or Settings).
pub(super) fn keep_camera(
    mut rig: ResMut<super::hero::CamRig>,
    mut seen: Local<Option<super::hero::CamMode>>,
) {
    use super::hero::CamMode;
    let Some(last) = *seen else {
        let saved = settings_path().and_then(|p| crate::local_state::read_to_string(&p).ok());
        let free = saved.is_some_and(|t| t.lines().any(|l| l.trim() == "camera free"));
        if free && rig.mode == CamMode::Fixed {
            rig.toggle_mode();
        }
        *seen = Some(rig.mode);
        return;
    };
    if last == rig.mode {
        return;
    }
    *seen = Some(rig.mode);
    let word = match rig.mode {
        CamMode::Fixed => "locked",
        CamMode::Orbit => "free",
    };
    if let Some(path) = settings_path() {
        let _ = crate::local_state::write_atomic(&path, &format!("camera {word}\n"));
    }
}
