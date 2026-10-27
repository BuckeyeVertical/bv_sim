# Physics backend

This image contains PX4 SITL, headless Gazebo Harmonic, the vehicle's physics
and sensors, a ground collision plane, and the binary UDP state bridge. It
needs no host mounts. Bevy owns all visual assets and camera rendering.

There are no scenery models, visual geometry, meshes, textures, or remote Fuel
downloads in the runtime. `backend/px4/prepare_runtime.py` exports only the four
SDF dependencies of `x500_gimbal`: `x500_gimbal`, `x500`, `x500_base`, and `gimbal`.
It removes visuals while preserving inertia, collision primitives, motor and
gimbal joints/controllers, sensor frames, and sensor plugins. Those definitions
are needed for PX4 to fly and for Bevy to receive the true gimbal-camera pose.
They are physics definitions, not rendering assets. The exporter refuses to
drop a mesh used by a collision rather than silently changing the dynamics.

The old scenery placeholders and offline-world patch are removed. The only
world shipped is our minimal `backend/worlds/default.sdf`; upstream sample
worlds and unrelated vehicle models are excluded from the runtime image.
Gazebo's packaged example worlds/thumbnails are removed too. Its scene metadata
service remains enabled because PX4 uses it to detect that the world is ready;
this service does not render camera images.

Build from the repository root:

```sh
docker build -f backend/Dockerfile -t bv-sim-backend:local .
```

To use this manually built image with the launcher, run
`cargo run -- run suas_2026 --image bv-sim-backend:local`.

The default build uses the host's native CPU architecture. Release CI can pass
`--platform linux/amd64` or `--platform linux/arm64` to `docker buildx build`.
Both architecture builds need validation before publishing a multi-platform tag.

PX4 and the state bridge share the pinned PX4/Gazebo build toolchain. FlatBuffers
25.2.10 headers and Tini 0.19.0 source archives are verified by SHA-256; Tini's
static executable is built with that toolchain. The runtime image receives only
the resulting executables, plugins, models, and worlds.

Run a freshly created container:

```sh
docker run --rm --name bv-sim \
  --network host \
  -e PX4_HOME_LAT=36.214675 \
  -e PX4_HOME_LON=-96.00655556 \
  -e PX4_HOME_ALT=0 \
  bv-sim-backend:local
```

`PX4_HOME_ALT` is the altitude of the geographic origin, in meters AMSL; it is
not a commanded flight height. The native launcher supplies the selected
scenario's coordinates. The backend always uses its bundled `default` physics
world; scenario TOML files only configure the scenario name, origin, and camera.

The launcher and manual command use Docker host networking. Linux Docker Engine
and OrbStack support this directly. On Docker Desktop 4.34 or later, enable
**Settings → Resources → Network → Enable host networking** first. Port
publishing is unnecessary in this mode. PX4 keeps its standard localhost
destinations: connect from the host with `udpin:0.0.0.0:14540` for offboard
control or `udpin:0.0.0.0:14550` for a GCS.

The state bridge binds `0.0.0.0:7001/udp`. A subscriber sends a binary Hello from
the socket on which it receives state. The bridge replies to that socket's
observed address, without a hardcoded host address.
Each state datagram contains complete vehicle (ID 1) and camera (ID 2) world poses.
The subscriber renews its lease with Hello or Heartbeat; leases expire after
three seconds, and the server admits at most eight subscribers. A Welcome echoes
the Hello's client ID and advertises the current run. Unknown/malformed control
packets and datagrams exceeding 1,200 bytes are ignored. A simulation clock reset
starts a new run identity and resets its sequence counter.

Docker health requires successful PX4 parameter configuration and a validated
Welcome plus complete state from `bv_sim_bridge --healthcheck`. A bound UDP
socket alone does not count as ready. The health probe also works while paused,
because subscriptions receive the latest state immediately. Start-period is
60 seconds, interval two seconds, timeout three seconds, and retries 15.

The entrypoint supervises Gazebo, the state bridge, PX4, and the startup
configuration job. A failure stops the container; Tini forwards signals and
reaps child processes. Runtime writable files are contained in the disposable
container. Use a fresh container for each new simulation run.

Optional bridge overrides are `BV_BRIDGE_ADDRESS`, `BV_BRIDGE_PORT`,
`BV_POSE_TOPIC`, `BV_VEHICLE_NAME`, and `BV_CAMERA_NAME`. If changing the bridge
port, also override Docker's health command and the client's endpoint. The numeric
protocol IDs remain stable when Gazebo entity names are overridden.

## Tests

The Docker build runs pose composition, binary-message verification, and UDP
subscription tests. Tests for the binary protocol and UDP socket implementation
can run natively without Gazebo:

```sh
cmake -S backend/bridge -B /tmp/bv-sim-bridge-tests \
  -DBV_WITH_GAZEBO=OFF -DCMAKE_BUILD_TYPE=Release
cmake --build /tmp/bv-sim-bridge-tests
ctest --test-dir /tmp/bv-sim-bridge-tests --output-on-failure
```

FlatBuffers headers are downloaded from the SHA-256-pinned 25.2.10 source
archive. To reuse an existing source checkout, set
`-DBV_FLATBUFFERS_SOURCE_DIR=/path/to/flatbuffers-25.2.10`. Generated C++ bindings
live under `protocol/generated/cpp`; ordinary builds do not require `flatc`.
The `state_fixture OUTPUT_FILE` test executable writes a deterministic C++ state
packet for cross-language verification.

The physics exporter tests run during the image build and can also run on any
Python host:

```sh
python3 -m unittest discover -s backend/px4 -p 'test_*.py'
```
