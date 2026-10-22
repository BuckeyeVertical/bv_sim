use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    path::Path,
    process::{Command, Output, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::{cli::BackendArgs, paths::Paths, scenario::Scenario};

pub const CONTAINER_NAME: &str = "bv-sim";
pub const MANAGED_LABEL: &str = "org.buckeyevertical.bv-sim";
const MANAGED_VALUE: &str = "managed-v1";
pub const DEFAULT_IMAGE: &str = concat!(
    "ghcr.io/buckeyevertical/bv-sim-backend:",
    env!("CARGO_PKG_VERSION")
);
pub const LOCAL_IMAGE: &str = concat!("bv-sim-backend:", env!("CARGO_PKG_VERSION"));

#[derive(Clone)]
pub struct Docker {
    program: OsString,
}

impl Docker {
    pub fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
        }
    }

    fn output(&self, args: &[impl AsRef<OsStr>]) -> Result<Output> {
        Command::new(&self.program).args(args).output().with_context(|| {
            format!("cannot execute {}. Install and start Docker Desktop, OrbStack, or Docker Engine", self.program.to_string_lossy())
        })
    }

    fn checked_output(&self, args: &[impl AsRef<OsStr>]) -> Result<Output> {
        let output = self.output(args)?;
        if !output.status.success() {
            bail!(
                "Docker command failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(output)
    }

    fn inherited(&self, args: &[impl AsRef<OsStr>]) -> Result<bool> {
        Ok(Command::new(&self.program)
            .args(args)
            .stdin(Stdio::null())
            .status()
            .context("failed to execute Docker")?
            .success())
    }

    pub fn require_running(&self) -> Result<()> {
        let output = self.output(&["info", "--format", "{{.OSType}}"])?;
        if !output.status.success() {
            bail!(
                "Docker is unavailable. Start Docker Desktop, OrbStack, or Docker Engine, then retry.\n{}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        if String::from_utf8_lossy(&output.stdout).trim() != "linux" {
            bail!("the simulator requires Docker configured for Linux containers");
        }
        Ok(())
    }

    pub fn inspect(&self, target: &str) -> Result<Option<Container>> {
        let output = self.output(&["container", "inspect", target])?;
        if !output.status.success() {
            // Establish that Docker is usable rather than calling every daemon
            // or permission error a missing container.
            self.require_running()?;
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("No such") || stderr.contains("not found") {
                return Ok(None);
            }
            bail!("cannot inspect container {target}: {}", stderr.trim());
        }
        let mut containers: Vec<Container> = serde_json::from_slice(&output.stdout)
            .context("Docker returned invalid container inspection data")?;
        if containers.len() != 1 {
            bail!("Docker returned an unexpected container count");
        }
        Ok(containers.pop())
    }

    pub fn prepare_image(
        &self,
        args: &BackendArgs,
        paths: &Paths,
        shutdown: &AtomicBool,
    ) -> Result<String> {
        let image = selected_image(args);
        if image.is_empty() || image.starts_with('-') || image.chars().any(char::is_whitespace) {
            bail!("--image / BV_SIM_IMAGE must contain one valid nonempty Docker image reference");
        }
        if args.build {
            let source = args.source_dir.as_deref().or(paths.discovered_source.as_deref())
                .context("--build requires a source checkout. Use --source-dir /path/to/bv_sim containing backend/Dockerfile")?;
            if !source.join("backend/Dockerfile").is_file() {
                bail!("{} does not contain backend/Dockerfile", source.display());
            }
            eprintln!("Building {image} from {}", source.display());
            if !self.inherited(&build_arguments(source, &image, args.platform.as_deref()))? {
                bail!("backend image build failed; review the Docker build output above");
            }
        } else if !self.output(&["image", "inspect", &image])?.status.success() {
            eprintln!("Downloading {image}");
            let mut pull = vec!["pull".to_owned()];
            if let Some(platform) = &args.platform {
                pull.extend(["--platform".to_owned(), platform.clone()]);
            }
            pull.push(image.clone());
            if !self.inherited(&pull)? {
                bail!(
                    "could not download {image}. This checkout does not publish an image automatically. From the source checkout use `bv-sim run {} --build`, or provide an available image with --image IMAGE / BV_SIM_IMAGE. For a private registry, sign in with `docker login` first",
                    "suas_2026"
                );
            }
        }
        if shutdown.load(Ordering::Relaxed) {
            bail!("startup interrupted");
        }
        Ok(image)
    }

    pub fn start(
        &self,
        args: &BackendArgs,
        scenario: &Scenario,
        image: &str,
    ) -> Result<OwnedContainer> {
        if let Some(existing) = self.inspect(CONTAINER_NAME)? {
            if existing.is_managed() {
                bail!(
                    "managed container {CONTAINER_NAME} already exists ({}). Use `bv-sim run {} --render-only` to connect, or `bv-sim stop` before starting another run",
                    existing.state.status,
                    scenario.name
                );
            }
            bail!(
                "container {CONTAINER_NAME} already exists and was not created by this launcher. Rename it before starting the simulator"
            );
        }
        // Own the container before starting it so failed port binding, startup
        // errors, and readiness failures all follow the same cleanup path.
        let output = self.checked_output(&create_arguments(args, scenario, image))?;
        let id = String::from_utf8(output.stdout)?.trim().to_owned();
        if id.is_empty() {
            bail!("Docker did not return a container ID");
        }
        let owned = OwnedContainer {
            docker: self.clone(),
            id,
            active: true,
        };
        self.checked_output(&["container", "start", &owned.id])?;
        Ok(owned)
    }

    pub fn wait_ready(&self, id: &str, timeout_seconds: u64, shutdown: &AtomicBool) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
        eprintln!("Waiting for PX4 and simulation state (up to {timeout_seconds}s)");
        loop {
            if shutdown.load(Ordering::Relaxed) {
                bail!("startup interrupted");
            }
            let container = self
                .inspect(id)?
                .context("backend exited and its container was removed")?;
            if !container.state.running {
                self.print_logs(id);
                bail!(
                    "backend stopped with exit code {}",
                    container.state.exit_code
                );
            }
            match container
                .state
                .health
                .as_ref()
                .map(|health| health.status.as_str())
            {
                Some("healthy") => {
                    eprintln!("Backend ready");
                    return Ok(());
                }
                Some("unhealthy") => {
                    self.print_logs(id);
                    bail!("backend health check failed. Inspect Docker logs before retrying");
                }
                None => bail!(
                    "backend image has no health check; rebuild this repository's backend or select a compatible versioned image"
                ),
                _ => {}
            }
            if Instant::now() >= deadline {
                self.print_logs(id);
                bail!(
                    "backend did not become ready in {timeout_seconds}s; inspect logs or increase --ready-timeout"
                );
            }
            thread::sleep(Duration::from_millis(500));
        }
    }

    fn print_logs(&self, id: &str) {
        if let Ok(output) = self.output(&["logs", "--tail", "60", id]) {
            eprintln!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    fn stop_id(&self, id: &str) -> Result<()> {
        let Some(container) = self.inspect(id)? else {
            return Ok(());
        };
        container.require_managed()?;
        if container.state.running {
            self.checked_output(&["container", "stop", "--time", "10", id])?;
        }
        // Containers started with --rm disappear after stop. Also clean up a
        // stopped managed container retained by a compatible manual launch.
        if self.inspect(id)?.is_some() {
            self.checked_output(&["container", "rm", id])?;
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        let Some(container) = self.inspect(CONTAINER_NAME)? else {
            println!("No managed backend is running");
            return Ok(());
        };
        container.require_managed()?;
        self.stop_id(&container.id)?;
        println!("Stopped {CONTAINER_NAME}");
        Ok(())
    }

    pub fn status(&self) -> Result<()> {
        match self.inspect(CONTAINER_NAME)? {
            Some(container) => {
                container.require_managed()?;
                println!(
                    "{CONTAINER_NAME}: {} (health: {})",
                    container.state.status,
                    container
                        .state
                        .health
                        .as_ref()
                        .map(|h| h.status.as_str())
                        .unwrap_or("not configured")
                );
            }
            None => println!("No managed backend is running"),
        }
        Ok(())
    }
}

fn selected_image(args: &BackendArgs) -> String {
    args.image.clone().unwrap_or_else(|| {
        if args.build {
            LOCAL_IMAGE
        } else {
            DEFAULT_IMAGE
        }
        .to_owned()
    })
}

fn build_arguments(source: &Path, image: &str, platform: Option<&str>) -> Vec<OsString> {
    let mut args = vec![
        "build".into(),
        "--file".into(),
        source.join("backend/Dockerfile").into_os_string(),
        "--tag".into(),
        image.into(),
    ];
    if let Some(platform) = platform {
        args.extend(["--platform".into(), platform.into()]);
    }
    args.push(source.as_os_str().to_owned());
    args
}

fn create_arguments(args: &BackendArgs, scenario: &Scenario, image: &str) -> Vec<String> {
    let mut command = vec![
        "container".to_owned(),
        "create".to_owned(),
        "--name".to_owned(),
        CONTAINER_NAME.to_owned(),
        "--label".to_owned(),
        format!("{MANAGED_LABEL}={MANAGED_VALUE}"),
        "--label".to_owned(),
        format!("org.buckeyevertical.bv-sim.scenario={}", scenario.name),
        "--publish".to_owned(),
        "127.0.0.1:7001:7001/udp".to_owned(),
        "--publish".to_owned(),
        "14580:14580/udp".to_owned(),
        "--publish".to_owned(),
        "18570:18570/udp".to_owned(),
    ];
    if let Some(platform) = &args.platform {
        command.extend(["--platform".to_owned(), platform.clone()]);
    }
    for (key, value) in [
        ("PX4_HOME_LAT", scenario.home.latitude.to_string()),
        ("PX4_HOME_LON", scenario.home.longitude.to_string()),
        ("PX4_HOME_ALT", scenario.home.altitude_m.to_string()),
        ("BV_PX4_NAV_DLL_ACT", "0".to_owned()),
        ("PX4_PARAM_MPC_ACC_HOR", "2".to_owned()),
        ("PX4_PARAM_SIM_BAT_MIN_PCT", "100".to_owned()),
        ("PX4_PARAM_MAV_0_BROADCAST", "1".to_owned()),
        ("PX4_PARAM_MAV_1_BROADCAST", "1".to_owned()),
    ] {
        command.extend(["--env".to_owned(), format!("{key}={value}")]);
    }
    command.push(image.to_owned());
    command
}

#[derive(Deserialize)]
pub struct Container {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Config")]
    config: ContainerConfig,
    #[serde(rename = "State")]
    pub state: ContainerState,
}

impl Container {
    fn is_managed(&self) -> bool {
        self.config
            .labels
            .as_ref()
            .and_then(|labels| labels.get(MANAGED_LABEL))
            .is_some_and(|value| value == MANAGED_VALUE)
    }

    fn require_managed(&self) -> Result<()> {
        if !self.is_managed() {
            bail!(
                "container {CONTAINER_NAME} was not created by this launcher; leaving it untouched"
            );
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct ContainerConfig {
    #[serde(rename = "Labels")]
    labels: Option<BTreeMap<String, String>>,
}

#[derive(Deserialize)]
pub struct ContainerState {
    #[serde(rename = "Running")]
    pub running: bool,
    #[serde(rename = "Status")]
    pub status: String,
    #[serde(rename = "ExitCode")]
    pub exit_code: i32,
    #[serde(rename = "Health")]
    pub health: Option<Health>,
}

#[derive(Deserialize)]
pub struct Health {
    #[serde(rename = "Status")]
    pub status: String,
}

pub struct OwnedContainer {
    docker: Docker,
    pub id: String,
    active: bool,
}

impl OwnedContainer {
    pub fn leave_running(mut self) {
        self.active = false;
    }
}

impl Drop for OwnedContainer {
    fn drop(&mut self) {
        if self.active {
            eprintln!("Stopping the backend started by this command");
            if let Err(error) = self.docker.stop_id(&self.id) {
                eprintln!(
                    "Could not stop backend: {error:#}. Run `bv-sim stop` when Docker is available."
                );
            }
        }
    }
}

pub fn install_shutdown_handler() -> Result<Arc<AtomicBool>> {
    let shutdown = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&shutdown);
    ctrlc::set_handler(move || signal.store(true, Ordering::Relaxed))
        .context("cannot install simulator shutdown handler")?;
    Ok(shutdown)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backend(build: bool) -> BackendArgs {
        BackendArgs {
            build,
            image: None,
            source_dir: None,
            platform: None,
            ready_timeout: 120,
        }
    }

    #[test]
    fn launch_uses_no_mounts_and_passes_scenario_origin_without_a_shell() {
        let scenario: Scenario =
            toml::from_str(include_str!("../../../scenarios/suas_2026.toml")).unwrap();
        let args = create_arguments(&backend(false), &scenario, "example:0.1.0");
        assert!(
            !args
                .iter()
                .any(|arg| ["--mount", "--volume", "-v"].contains(&arg.as_str()))
        );
        assert!(args.contains(&"PX4_HOME_LAT=36.214675".to_owned()));
        assert!(args.contains(&"PX4_HOME_LON=-96.00655556".to_owned()));
        assert!(args.contains(&"127.0.0.1:7001:7001/udp".to_owned()));
        assert!(!args.contains(&"--platform".to_owned()));
        assert_eq!(args.last().unwrap(), "example:0.1.0");
    }

    #[test]
    fn local_build_has_separate_default_tag_and_handles_spaces() {
        assert_eq!(selected_image(&backend(true)), LOCAL_IMAGE);
        assert_eq!(selected_image(&backend(false)), DEFAULT_IMAGE);
        let args = build_arguments(Path::new("/tmp/a checkout"), "image:1", Some("linux/amd64"));
        assert_eq!(
            args[2],
            Path::new("/tmp/a checkout").join("backend/Dockerfile")
        );
        assert_eq!(args.last().unwrap(), OsStr::new("/tmp/a checkout"));
    }

    #[test]
    fn only_exact_managed_label_authorizes_stopping() {
        for value in ["", "other", MANAGED_VALUE] {
            let payload = serde_json::json!({
                "Id": "container-id", "Config": { "Labels": { MANAGED_LABEL: value } },
                "State": { "Running": true, "Status": "running", "ExitCode": 0, "Health": { "Status": "healthy" } },
            });
            let container: Container = serde_json::from_value(payload).unwrap();
            assert_eq!(container.require_managed().is_ok(), value == MANAGED_VALUE);
        }
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_uses_owned_id_and_refuses_an_unmanaged_container() {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::AtomicUsize;
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "bv-sim-docker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let script = root.join("docker");
        let data = root.join("inspect.json");
        let stopped = root.join("stopped");
        let output = serde_json::json!([{
            "Id": "owned-container-id", "Config": { "Labels": { MANAGED_LABEL: MANAGED_VALUE } },
            "State": { "Running": true, "Status": "running", "ExitCode": 0, "Health": { "Status": "healthy" } },
        }]);
        std::fs::write(&data, output.to_string()).unwrap();
        // Fixture paths are generated under the system temporary directory.
        // Arguments supplied by the launcher never pass through a shell.
        let quote =
            |path: &Path| format!("'{}'", path.display().to_string().replace('\'', "'\\''"));
        std::fs::write(
            &script,
            format!(
                r#"#!/bin/sh
if [ "$1" = info ]; then printf 'linux\n'; exit 0; fi
if [ "$1" = container ] && [ "$2" = inspect ]; then
    if [ -f {data} ]; then cat {data}; exit 0; fi
    echo 'Error: No such container' >&2; exit 1
fi
if [ "$1" = container ] && [ "$2" = stop ]; then
    printf '%s\n' "$5" > {stopped}
    rm -f {data}
    exit 0
fi
exit 1
"#,
                data = quote(&data),
                stopped = quote(&stopped)
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let docker = Docker::new(script.as_os_str());
        let owned = OwnedContainer {
            docker: docker.clone(),
            id: "owned-container-id".to_owned(),
            active: true,
        };
        drop(owned);
        assert_eq!(
            std::fs::read_to_string(&stopped).unwrap(),
            "owned-container-id\n"
        );
        std::fs::remove_file(&stopped).unwrap();

        let unmanaged = serde_json::json!([{
            "Id": "someone-elses-id", "Config": { "Labels": null },
            "State": { "Running": true, "Status": "running", "ExitCode": 0 },
        }]);
        std::fs::write(&data, unmanaged.to_string()).unwrap();
        assert!(docker.stop().is_err());
        assert!(!stopped.exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
