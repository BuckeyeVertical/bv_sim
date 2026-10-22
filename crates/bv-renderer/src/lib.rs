//! Native Bevy rendering and the simulator's onboard-camera interface.
//!
//! The launcher supplies the installed asset location and starts the backend;
//! this crate owns rendering, UDP state reception, and the legacy camera stream.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use bevy::{asset::AssetPlugin, prelude::*, winit::WinitSettings};

pub mod camera;
pub mod capture;
pub mod geo;
pub mod sim;
pub mod vehicle;
pub mod worlds;

pub use camera::DroneCameraConfig;
pub use worlds::WorldChoice;

#[derive(Debug, Clone)]
pub struct RendererConfig {
    pub world: WorldChoice,
    pub asset_dir: PathBuf,
    /// Backend's published UDP endpoint, normally `127.0.0.1:7001`.
    pub state_endpoint: String,
    /// JPEG/TCP v1 server; use a host-reachable address for Docker clients.
    pub camera_bind: String,
    pub screenshot: Option<PathBuf>,
    pub camera_config: DroneCameraConfig,
    pub jpeg_quality: u8,
    pub shutdown: Option<Arc<AtomicBool>>,
}

/// Launch the native renderer using locations resolved by the application.
/// No current-working-directory or source-checkout paths are used for assets.
pub fn run(config: RendererConfig) -> AppExit {
    let geography = geo::WorldGeography {
        home: config.world.px4_home(),
    };
    if let Err(error) = geography.validate() {
        eprintln!("invalid scenario geography: {error}");
        return AppExit::error();
    }
    let asset_dir = match config.asset_dir.canonicalize() {
        Ok(path) if path.is_dir() => path,
        _ => {
            eprintln!(
                "asset directory does not exist: {}",
                config.asset_dir.display()
            );
            return AppExit::error();
        }
    };

    let camera = camera::DroneCameraPlugin::new(config.camera_config);
    let frames = camera::CameraFrameServerPlugin::new(config.camera_bind, config.jpeg_quality);
    let receiver = sim::SimReceiverPlugin::new(config.state_endpoint);
    eprintln!("world        {}", config.world.name());
    eprintln!("assets       {}", asset_dir.display());
    eprintln!("camera       {}", camera.describe());
    eprintln!("frame server {}", frames.describe());
    eprintln!("sim state    UDP {}", receiver.describe());

    let mut app = App::new();
    // Camera output remains a live service when the observer loses focus.
    app.insert_resource(WinitSettings::continuous())
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: asset_dir.to_string_lossy().into_owned(),
            ..default()
        }))
        .add_plugins((camera, frames, receiver))
        .add_plugins((
            worlds::WorldPlugin(config.world),
            camera::DebugCameraPlugin,
            vehicle::VehiclePlugin,
        ));
    if let Some(shutdown) = config.shutdown {
        app.insert_resource(ShutdownSignal(shutdown))
            .add_systems(Update, exit_on_shutdown);
    }
    if let Some(path) = config.screenshot {
        app.add_plugins(capture::CapturePlugin { path });
    }
    app.run()
}

#[derive(Resource)]
struct ShutdownSignal(Arc<AtomicBool>);

fn exit_on_shutdown(signal: Res<ShutdownSignal>, mut exit: MessageWriter<AppExit>) {
    if signal.0.load(Ordering::Relaxed) {
        exit.write(AppExit::Success);
    }
}
