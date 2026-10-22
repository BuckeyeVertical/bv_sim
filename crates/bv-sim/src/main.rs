mod cli;
mod docker;
mod paths;
mod scenario;

use std::{net::ToSocketAddrs, process::ExitCode};

use anyhow::{Context, Result, bail};
use bevy::app::AppExit;
use bv_renderer::RendererConfig;
use clap::Parser;

use cli::{Cli, Command, RunArgs};
use docker::Docker;
use paths::Paths;
use scenario::Scenario;

fn main() -> ExitCode {
    match execute(Cli::parse()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("bv-sim: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn execute(cli: Cli) -> Result<ExitCode> {
    let paths = Paths::resolve(&cli)?;
    let docker = Docker::new(&cli.docker);
    match &cli.command {
        Command::Run(args) => run(&cli, &paths, &docker, args),
        Command::Start(args) => {
            let scenario = Scenario::load(&paths.scenarios, &args.world)?;
            docker.require_running()?;
            let shutdown = docker::install_shutdown_handler()?;
            let image = docker.prepare_image(&args.backend, &paths, &shutdown)?;
            let owned = docker.start(&args.backend, &scenario, &image)?;
            docker.wait_ready(&owned.id, args.backend.ready_timeout, &shutdown)?;
            owned.leave_running();
            println!(
                "Backend ready. Open Bevy with `bv-sim run {} --render-only`. Stop it with `bv-sim stop`.",
                scenario.name
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Stop => {
            docker.stop()?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Status => {
            docker.status()?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Doctor => {
            println!(
                "bv-sim {} ({} / {})",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH
            );
            println!(
                "assets:    {} ({})",
                paths.assets.display(),
                if paths.assets.is_dir() {
                    "found"
                } else {
                    "missing"
                }
            );
            println!(
                "scenarios: {} ({})",
                paths.scenarios.display(),
                if paths.scenarios.is_dir() {
                    "found"
                } else {
                    "missing"
                }
            );
            println!("Docker executable: {}", cli.docker);
            docker.require_running()?;
            println!("Docker: ready for Linux containers");
            println!(
                "Release image: {} (registry publication is a separate release step)",
                docker::DEFAULT_IMAGE
            );
            paths.require_assets()?;
            if !paths.scenarios.join("suas_2026.toml").is_file() {
                bail!("missing scenario suas_2026; install the complete release bundle");
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn run(cli: &Cli, paths: &Paths, docker: &Docker, args: &RunArgs) -> Result<ExitCode> {
    paths.require_assets()?;
    let scenario = Scenario::load(&paths.scenarios, &args.world)?;
    let (camera_config, jpeg_quality) = scenario.camera.renderer_config()?;
    for (description, address) in [
        ("state endpoint", &cli.state_endpoint),
        ("camera bind", &cli.camera_bind),
    ] {
        if address
            .to_socket_addrs()
            .with_context(|| format!("invalid {description} {address}"))?
            .next()
            .is_none()
        {
            bail!("invalid {description} {address}");
        }
    }
    if !args.render_only && args.screenshot.is_none() && cli.state_endpoint != "127.0.0.1:7001" {
        bail!(
            "a managed backend publishes at 127.0.0.1:7001; use --render-only for a custom --state-endpoint"
        );
    }
    let shutdown = docker::install_shutdown_handler()?;
    let owned = if args.render_only || args.screenshot.is_some() {
        None
    } else {
        docker.require_running()?;
        let image = docker.prepare_image(&args.backend, paths, &shutdown)?;
        let owned = docker.start(&args.backend, &scenario, &image)?;
        docker.wait_ready(&owned.id, args.backend.ready_timeout, &shutdown)?;
        Some(owned)
    };
    eprintln!(
        "Scenario {} / geographic origin {}, {}, {} m",
        scenario.name, scenario.home.latitude, scenario.home.longitude, scenario.home.altitude_m
    );
    eprintln!("Camera JPEG/TCP listening on {}", cli.camera_bind);
    let result = bv_renderer::run(RendererConfig {
        world: args.world.clone(),
        asset_dir: paths.assets.clone(),
        state_endpoint: cli.state_endpoint.clone(),
        camera_bind: cli.camera_bind.clone(),
        screenshot: args.screenshot.clone(),
        camera_config,
        jpeg_quality,
        shutdown: Some(shutdown),
    });
    drop(owned);
    Ok(match result {
        AppExit::Success => ExitCode::SUCCESS,
        AppExit::Error(code) => ExitCode::from(code.get()),
    })
}
