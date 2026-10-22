use std::path::PathBuf;

use bv_renderer::WorldChoice;
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, about = "PX4/Gazebo in Docker, Bevy on your computer")]
pub struct Cli {
    /// Directory containing the installed assets/ and scenarios/ directories.
    #[arg(long, global = true, env = "BV_SIM_DATA_DIR")]
    pub data_dir: Option<PathBuf>,
    #[arg(long, global = true, env = "BV_SIM_ASSET_DIR")]
    pub assets_dir: Option<PathBuf>,
    #[arg(long, global = true, env = "BV_SIM_SCENARIO_DIR")]
    pub scenarios_dir: Option<PathBuf>,
    /// Docker CLI executable. Normally resolved through PATH.
    #[arg(long, global = true, env = "BV_SIM_DOCKER", default_value = "docker")]
    pub docker: String,
    /// Published UDP endpoint used by the native state receiver.
    #[arg(
        long,
        global = true,
        env = "BV_SIM_STATE_ENDPOINT",
        default_value = "127.0.0.1:7001"
    )]
    pub state_endpoint: String,
    /// Bind address for the existing mission-compatible JPEG/TCP camera stream.
    #[arg(
        long,
        global = true,
        env = "BV_CAMERA_BIND",
        default_value = "127.0.0.1:7002"
    )]
    pub camera_bind: String,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start the backend, open Bevy, and stop the backend when the window closes.
    Run(RunArgs),
    /// Start the backend without opening Bevy. Stop it with `bv-sim stop`.
    Start(StartArgs),
    /// Stop only the backend created by this launcher.
    Stop,
    /// Show the managed backend's status and readiness.
    Status,
    /// Check Docker and locate installed data; no simulation is started.
    Doctor,
}

#[derive(Debug, Args)]
pub struct BackendArgs {
    /// Build backend/Dockerfile from a checkout instead of downloading an image.
    #[arg(long, global = true)]
    pub build: bool,
    /// Image to run, or tag to build. Default is the versioned release image.
    #[arg(long, global = true, env = "BV_SIM_IMAGE")]
    pub image: Option<String>,
    /// Source checkout containing backend/Dockerfile; used only with --build.
    #[arg(long, global = true, env = "BV_SIM_SOURCE_DIR")]
    pub source_dir: Option<PathBuf>,
    /// Optional Docker platform; unset uses Docker's native platform.
    #[arg(long, global = true, env = "BV_SIM_PLATFORM")]
    pub platform: Option<String>,
    /// Seconds to wait for PX4 and a valid state snapshot.
    #[arg(long, global = true, default_value_t = 120, value_parser = clap::value_parser!(u64).range(1..))]
    pub ready_timeout: u64,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    #[command(flatten)]
    pub backend: BackendArgs,
    /// Open the renderer without starting or stopping Docker.
    #[arg(long, global = true, conflicts_with = "build")]
    pub render_only: bool,
    /// Save a world screenshot and exit; does not start Docker.
    #[arg(long, global = true, value_name = "PNG", conflicts_with = "build")]
    pub screenshot: Option<PathBuf>,
    #[command(subcommand)]
    pub world: WorldChoice,
}

#[derive(Debug, Args)]
pub struct StartArgs {
    #[command(flatten)]
    pub backend: BackendArgs,
    #[command(subcommand)]
    pub world: WorldChoice,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_suas_and_flags_after_world() {
        let cli = Cli::try_parse_from(["bv-sim", "run", "suas_2026", "--render-only"])
            .expect("global run flags work after the world selection");
        assert!(matches!(
            cli.command,
            Command::Run(RunArgs {
                render_only: true,
                ..
            })
        ));
        let cli = Cli::try_parse_from(["bv-sim", "start", "suas_2026", "--build"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Start(StartArgs {
                backend: BackendArgs { build: true, .. },
                ..
            })
        ));
        assert!(Cli::try_parse_from(["bv-sim", "run", "unknown_world"]).is_err());
    }

    #[test]
    fn screenshot_never_builds_a_backend() {
        assert!(
            Cli::try_parse_from([
                "bv-sim",
                "run",
                "suas_2026",
                "--build",
                "--screenshot",
                "world.png",
            ])
            .is_err()
        );
    }
}
