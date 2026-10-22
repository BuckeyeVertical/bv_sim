# BV Sim

PX4 and headless Gazebo run in a self-contained Docker image. A native Bevy
application draws the SUAS 2026 field and streams onboard-camera images
to the existing mission software. Runtime simulation containers use no host
file mounts. The waypoint editor is outside this rewrite.

This directory is designed to become a standalone repository. **No registry
image or native release has been published by creating these files.** The
working source-checkout path builds the backend locally:

```bash
cd bv_sim
cargo run -- run suas_2026 --build
```

Install and start Docker Desktop, OrbStack, or Docker Engine first. Source builds
also require Rust and the native dependencies required by Bevy. On Linux, use a
desktop session with working GPU drivers and install the window/audio development
packages listed in `.github/workflows/test.yaml`. The native release targets are
macOS Apple Silicon, Windows x86_64, and Linux x86_64 (Ubuntu 24.04 or a
compatible desktop environment with its runtime libraries). Windows uses a
native `.exe` renderer and Docker Desktop configured for Linux containers;
the physics backend remains Linux on every host. Backend builds use Docker's
native Linux architecture unless `BV_SIM_PLATFORM` is set.

The launcher reads `scenarios/suas_2026.toml`, passes its geographic origin to PX4
through environment variables, builds or obtains the matching image, waits for
PX4 and valid simulation state, then opens Bevy. Closing the window or pressing
Ctrl+C stops only the container created by that command. It uses the name
`bv-sim`, leaving the old `bv-simulator` and `bv-mission` containers alone.

Useful commands:

```bash
cargo run -- doctor
cargo run -- status
cargo run -- start suas_2026 --build
cargo run -- run suas_2026 --render-only
cargo run -- stop
cargo run -- run suas_2026 --screenshot /tmp/suas.png
```

`--render-only` and `--screenshot` work without Docker. An observer view can
render without flight state; onboard-camera frames require valid vehicle and
camera state.

## Installation and future releases

Release downloads contain the executable **and its assets**. Windows uses a
`.zip`; macOS and Linux use `.tar.gz`. Users do not need Rust or a source checkout.

On Windows, extract the zip, open PowerShell in the extracted folder, and run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\install.ps1
```

The installer copies the executable and its data to
`%LOCALAPPDATA%\Programs\BVSim`, adds its `bin` directory to the user PATH,
and requires no administrator access. The execution-policy option applies only
to this installer process. Open a new terminal, then run `bv-sim doctor` and
`bv-sim run suas_2026`. Docker Desktop must be running in Linux-container mode.
The published backend tag needs a `linux/amd64` image for Windows x86_64.
The included backend release workflow publishes both `linux/amd64` and
`linux/arm64` variants under the same tag.

On macOS/Linux, run `bash install.sh` inside the extracted release. To build and
install from a source checkout into a permanent user-local directory:

```bash
cargo build --release --package bv-sim
bash packaging/install.sh
```

After opening a new terminal, `bv-sim` is available through `PATH`. The installer
copies the executable, scenarios, and visual assets to
`~/.local/share/bv-sim/releases/<version>/` and places a launcher in
`~/.local/bin`. It adds that directory to zsh or bash configuration once.
Use `--no-path` to manage shell configuration yourself. Downloaded release
archives use the same installer and do not need Rust or a source checkout.

Once an image has actually been published, a release user can run:

```bash
bv-sim run suas_2026 --image ghcr.io/trevorclute/bv-sim-backend:0.1.0
```

The launcher downloads a missing image automatically. `BV_SIM_IMAGE` sets a
default registry image, while `--build` defaults to the local tag
`bv-sim-backend:0.1.0`. Registry errors explain how to build locally or supply an
available image. `latest` is deliberately not used for version matching.

## Interfaces

| Connection | Format | Default |
|---|---|---|
| Gazebo bridge → Bevy | Versioned FlatBuffers state over UDP | `127.0.0.1:7001` |
| Bevy → mission vision | Existing `bv.camera_frame` v1 JSON header + JPEG over TCP | `127.0.0.1:7002` |
| PX4 ↔ mission flight control | MAVLink UDP | Existing ports `14580`, `18570` |

State messages carry run IDs, sequence numbers, simulation timestamps, and the
vehicle/camera poses. Missing or older UDP updates do not block rendering.
Camera transport remains separate so the existing `BevyPipeline` receiver can
continue to work. H.264/RTSP is a future optional camera output, rather than a
new requirement to install FFmpeg or GStreamer for this release.

For a Dockerized mission consumer, configure `bevy_host: host.docker.internal`
and `bevy_port: 7002` as before. If the Docker host gateway cannot reach a
loopback listener, use `--camera-bind 0.0.0.0:7002` on a trusted local network.
Linux Docker Engine may also need `--add-host=host.docker.internal:host-gateway`
on the mission container. The camera protocol currently provides no
authentication. See [running](docs/running.md) for settings.

Gazebo's `default` world contains a ground collision plane and the vehicle's
flight physics, sensors, and gimbal. It contains no scenery, visual geometry,
or mesh files. The SUAS environment and visible aircraft live entirely in Bevy.
The small SDF vehicle definitions remain necessary for PX4's flight simulation;
they are exported from the pinned upstream models with all visuals removed.
Objects drawn by Bevy do not automatically have Gazebo collision geometry.

## Development and releases

```bash
cargo test --workspace --locked
python3 -m unittest discover -s tools/camera -p 'test_*.py'
python3 packaging/test_install.py
python3 -m unittest discover -s backend/px4 -p 'test_*.py'
docker build --target bridge-build --file backend/Dockerfile .
```

`backend/` contains the image and Gazebo bridge, `crates/bv-renderer/` contains
Bevy, `crates/bv-protocol/` handles binary packets, and `crates/bv-sim/` owns
startup and paths. `protocol/` defines the shared C++/Rust messages;
`scenarios/` defines startup settings; `assets/` contains runtime visual data.

See the [binary state contract](protocol/README.md) and
[camera contract](docs/camera_frame_v1.md) for message definitions. With the
backend running, `cargo run -p bv-protocol --example state_probe` checks UDP
without a GPU. The [camera tools](tools/camera/README.md) verify, record, and
replay frames independently of mission software.

The [local validation record](docs/validation.md) describes the tested startup,
camera, cleanup, and installer paths and the remaining platform checks.

The launcher's default backend image is
`ghcr.io/trevorclute/bv-sim-backend:<version>`. Keep `VERSION`, Rust package
versions, and image tags aligned. Set repository variables
`BV_SIM_PUBLISH_IMAGES=true`,
`BV_SIM_IMAGE_REPOSITORY=ghcr.io/trevorclute/bv-sim-backend`, and
`BV_SIM_REGISTRY_USERNAME=trevorclute` to enable versioned image publication.
When publishing from a repository owned by another account or organization
(such as `BuckeyeVertical/bv_sim`) into this personal namespace, add a repository
Actions secret named `GHCR_TOKEN` containing a personal access token (classic)
from `trevorclute` with `write:packages` permission. The workflow uses this secret
when present; otherwise it uses the repository's automatic `GITHUB_TOKEN`.
See [GitHub's registry authentication guidance](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry#authenticating-with-a-personal-access-token-classic).
`BV_SIM_PUBLISH_RELEASES=true` enables native archive publication.
Both publication steps are disabled until configured. Tags use `v<version>`.
The release workflow builds all three platforms and runs the Windows installer
against the native Windows executable. `packaging/bundle.py` packages a compiled
binary with its runtime files, for example:

```bash
python3 packaging/bundle.py target/release/bv-sim aarch64-apple-darwin dist
```
