# State protocol, version 2

`state_v2.fbs` is the source of truth. FlatBuffers generates the C++ and Rust
types in `generated/`; a normal backend or renderer build uses those checked-in
files and does not need `flatc`. The state connection contains binary packets.
The separate mission-camera interface keeps its existing JSON-header/JPEG format.

Each UDP datagram is one non-size-prefixed FlatBuffer with the `BVST` identifier,
protocol version 2, and a maximum length of 1,200 bytes. This size avoids IP
fragmentation on typical local Docker networks. Neither side splits a snapshot
into multiple packets or waits for retransmission. Invalid packets are discarded.

## Subscription

The renderer opens an ephemeral UDP socket and sends `Hello` to
`127.0.0.1:7001`. Docker forwards it to the backend. The backend replies to the
sender's observed IP and port, so neither component needs the host's LAN address.

1. `Hello`: a fresh nonzero 64-bit `client_id` identifies a handshake attempt.
2. `Welcome`: echoes that ID and provides a nonzero `run_id`. A renderer accepts
   it only while awaiting that handshake's reply.
3. `State`: complete current poses, run ID, sequence, and simulation time in
   nanoseconds. The renderer accepts increasing sequences for its accepted run.
4. `Heartbeat`: echoes the client and run IDs every 500 ms, renewing the lease.

The backend supports eight subscribers and expires leases after three seconds.
It sends the latest state immediately on subscription, even when paused. The
renderer retries the handshake after two seconds without state; a new run ID also
requires a fresh handshake. No queued old packet may switch an established run.
These IDs detect restarts and delayed packets; they are not authentication.

## State meaning

| Field | Meaning |
|---|---|
| `run_id` | Random nonzero identity per backend run; changes on backward simulation-clock reset |
| `sequence` | Increasing state counter within a run; receivers tolerate gaps |
| `sim_time_ns` | Gazebo simulation clock, not wall-clock receipt time |
| `entities` | Complete configured entity set; an absent entity is currently unavailable |
| Entity `id = 1` | Vehicle origin, currently Gazebo `x500_gimbal_0` |
| Entity `id = 2` | Onboard camera optical sensor's world pose, including its gimbal transform |

Positions are Gazebo world ENU meters: east, north, up. Orientations are XYZW
quaternions relative to the same world. The bridge composes the Gazebo pose graph
before publishing; camera pose is not guessed from the vehicle body. The renderer
converts both position and orientation into Bevy's axes. Each position must be
finite, each quaternion finite and nonzero, and IDs nonzero and unique. At most
16 entities are allowed, within the 1,200-byte budget.

State packets have zero `client_id`. Control packets have no entities, sequence,
or simulation time; Hello has zero `run_id`. The backend emits the configured
vehicle/camera set once both are available. Encoding an oversized snapshot fails
instead of silently publishing only part of it. New incompatible versions use
an explicit version change; append-only compatible schema fields still need
both language fixtures and consumers checked.

The onboard image uses the exact accepted camera pose and associated simulation
sequence/time. Observer-view smoothing must not affect image geometry. Camera
metadata represents `run_id` as a 16-character lowercase hexadecimal
`sim_stream_id`, keeping mission reset detection compatible.

## Development

Regenerate with a C++ compiler, CMake, and Python 3.9+:

```sh
python3 tools/generate_protocol.py
python3 tools/generate_protocol.py --check
```

The script downloads checksum-pinned FlatBuffers 25.2.10 to `.tools/`. To use an
existing compiler, add `--flatc /path/to/flatc`; its exact version is checked.
Rust's FlatBuffers dependency and backend CMake use the same version. Generated
Rust keeps the compiler's formatting with LF line endings on every host; exclude
it from manual formatting.

The fixtures in `fixtures/` were independently encoded by C++ and Rust. Both
test suites decode both fixtures. Regenerate them deliberately when changing
the schema or encoder:

```sh
cargo run -p bv-protocol --example write_fixture -- protocol/fixtures/state_v2_rust.bvstate
/path/to/backend-build/state_fixture protocol/fixtures/state_v2_cpp.bvstate
cargo test -p bv-protocol
```

Probe the real running backend without a renderer or camera dependencies:

```sh
cargo run -p bv-protocol --example state_probe
```
