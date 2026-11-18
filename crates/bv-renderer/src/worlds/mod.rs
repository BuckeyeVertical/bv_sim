//! Visual worlds and their geographic origins.

mod glacier_2027;
pub mod shared;
mod suas_2026;

use crate::geo::{self, GeoPoint};
use bevy::prelude::*;

#[derive(clap::Subcommand, Clone, Debug)]
pub enum WorldChoice {
    /// The SUAS 2026 competition field: boundaries, lap route, targets and trees.
    #[command(name = "suas_2026")]
    Suas2026,
    /// A Glacier Ridge Metro Park meadow with a mannequin and suitcase.
    #[command(name = "glacier_2027")]
    Glacier2027,
}

impl WorldChoice {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Suas2026 => "suas_2026",
            Self::Glacier2027 => "glacier_2027",
        }
    }

    /// Geographic location corresponding to the local physics/visual origin.
    pub fn px4_home(&self) -> GeoPoint {
        match self {
            Self::Suas2026 => geo::SUAS_2026_FIELD,
            Self::Glacier2027 => glacier_2027::HOME,
        }
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
            WorldChoice::Glacier2027 => {
                app.add_plugins(glacier_2027::Glacier2027Plugin);
            }
        }
    }
}

fn ready(mut next: ResMut<NextState<WorldLoad>>) {
    next.set(WorldLoad::Ready);
}
