//! SMAA on the world, the `smaa` CVar, in Survivors and Classic alike: edges smoothed in one
//! post pass on the world's own target, before MetalFX scales it and the interface goes on top.
//! It costs a fraction of 4× multisampling at 5K, can be switched live (multisampling waits for
//! the next launch), and also smooths alpha-tested leaves and grass, which multisampling leaves
//! jagged. `benilla_world::boot` leaves bevy's anti-aliasing out; this adds back SMAA alone.

use bevy::anti_alias::smaa::{Smaa, SmaaPlugin, SmaaPreset};
use bevy::prelude::*;
use benilla_world::view::WorldCamera;

/// The `smaa` CVar: 0 off, 1 on.
#[derive(Resource, Debug, PartialEq)]
pub(crate) struct SmaaSetting(pub(crate) bool);

impl Default for SmaaSetting {
    fn default() -> Self {
        Self(true)
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.add_plugins(SmaaPlugin)
        .init_resource::<SmaaSetting>()
        .add_observer(on_cvar)
        .add_systems(Update, apply);
}

fn on_cvar(ev: On<crate::cvars::CvarChanged>, mut setting: ResMut<SmaaSetting>) {
    if ev.is("smaa") {
        setting.0 = ev.flag();
    }
}

/// Every world camera, the ones spawned since included, has `Smaa` while the setting is on.
fn apply(
    mut commands: Commands,
    setting: Res<SmaaSetting>,
    cams: Query<(Entity, Has<Smaa>), With<WorldCamera>>,
) {
    for (cam, has) in &cams {
        if setting.0 && !has {
            commands.entity(cam).insert(Smaa {
                preset: SmaaPreset::High,
            });
        } else if !setting.0 && has {
            commands.entity(cam).remove::<Smaa>();
        }
    }
}
