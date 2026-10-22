# Running BV Sim

From a checkout, run `cargo run -- run suas_2026 --build`. From an installed
release, run `bv-sim run suas_2026 --image IMAGE:VERSION` with an image that has
been published or built locally. The first form builds PX4 and Gazebo and can
take substantially longer than starting a downloaded image.

The installed executable resolves assets and scenarios relative to its own
installation; it does not depend on the terminal's current directory.

On Windows, download and extract the x86_64 zip and run
`powershell -NoProfile -ExecutionPolicy Bypass -File .\install.ps1` inside it.
The installation lives in `%LOCALAPPDATA%\Programs\BVSim`; its `bin` directory
is added to the user PATH. The source-checkout installer is
`packaging\install.ps1` after a native `cargo build --release -p bv-sim`.
Windows uses the native GPU renderer with Docker Desktop running Linux
containers; no Bevy renderer runs in WSL or inside Docker.

macOS/Linux archives include `install.sh`, which installs into
`~/.local/share/bv-sim/releases/<version>` and adds `~/.local/bin` to PATH.

| Setting | CLI / environment | Behavior |
|---|---|---|
| Complete runtime data | `--data-dir` / `BV_SIM_DATA_DIR` | Directory containing `assets/` and `scenarios/` |
| Visual assets | `--assets-dir` / `BV_SIM_ASSET_DIR` | Override only the asset directory |
| Scenario files | `--scenarios-dir` / `BV_SIM_SCENARIO_DIR` | Override only the scenarios directory |
| Backend image | `--image` / `BV_SIM_IMAGE` | Explicit local or registry image |
| Build checkout | `--source-dir` / `BV_SIM_SOURCE_DIR` | Source directory for `--build` |
| Docker platform | `--platform` / `BV_SIM_PLATFORM` | Optional `linux/amd64` or `linux/arm64`; native by default |
| Readiness timeout | `--ready-timeout` | Seconds before a startup failure, default 120 |
| State endpoint | `--state-endpoint` / `BV_SIM_STATE_ENDPOINT` | UDP endpoint; managed backend uses `127.0.0.1:7001` |
| Camera listener | `--camera-bind` / `BV_CAMERA_BIND` | JPEG/TCP bind address, default `127.0.0.1:7002` |
| Camera dimensions | `BV_CAMERA_WIDTH`, `BV_CAMERA_HEIGHT` | Override scenario dimensions |
| Camera field of view | `BV_CAMERA_HFOV_DEG` | Horizontal field of view in degrees |
| JPEG quality | `BV_CAMERA_JPEG_QUALITY` | Quality 1–100 |

The home altitude is the geographic-origin elevation in meters. It is not the
flight height above ground. The SUAS scenario retains the original zero
origin-elevation convention. Scenario coordinates must match the SUAS field's
authored visual origin; the launcher rejects mismatches.

`bv-sim start suas_2026` leaves a backend running. Use `bv-sim run suas_2026
--render-only`, then `bv-sim stop`. The launcher refuses to
replace an existing container or stop an unrelated container with the same
name. Managed runs do not mount host directories or retain PX4 parameters/logs
after their containers are removed.

The backend has no visible scenery or rendering meshes. It retains only the
vehicle's physical/sensor SDF definitions and the ground collision plane.

`bv-sim status` reports readiness; `docker logs bv-sim` shows a running backend's
logs. `bv-sim doctor` checks Docker and installed data. If an image is absent
from its registry, build from source with `--build` or configure an available
image. If Docker is stopped, start Docker Desktop/OrbStack/Engine and retry.
