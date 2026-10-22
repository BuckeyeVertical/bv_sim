//! The SUAS visual world and its geographic origin.

pub mod shared;
mod suas_2026;

use crate::geo::{self, GeoPoint};
use bevy::prelude::*;

#[derive(clap::Subcommand, Clone, Debug)]
pub enum WorldChoice {
    /// The SUAS 2026 competition field: boundaries, lap route, targets and trees.
    #[command(name = "suas_2026")]
    Suas2026,
}

impl WorldChoice {
    pub fn name(&self) -> &'static str {
        "suas_2026"
    }

    /// Geographic location corresponding to the local physics/visual origin.
    pub fn px4_home(&self) -> GeoPoint {
        geo::SUAS_2026_FIELD
    }
}

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum WorldLoad {
    #[default]
    Loading,
    Ready,
}

#[derive(Resource, Clone, Copy)]
pub struct DebugCameraStart(pub Transform);

pub struct WorldPlugin(pub WorldChoice);

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<WorldLoad>();
        match self.0 {
            WorldChoice::Suas2026 => {
                app.add_plugins(suas_2026::Suas2026Plugin);
            }
        }
    }
}

fn ready(mut next: ResMut<NextState<WorldLoad>>) {
    next.set(WorldLoad::Ready);
}
