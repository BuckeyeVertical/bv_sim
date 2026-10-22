# Camera compatibility tools

These tools preserve the mission software's `bv.camera_frame` version 1
interface: a newline-terminated JSON header followed by exactly `data_length`
JPEG bytes. This is independent of the binary UDP state connection.
They use only Python 3.10+ standard-library modules.

With the backend and renderer running:

```sh
python3 tools/camera/verify_camera_stream.py --frames 10 --output /tmp/onboard.jpg
python3 tools/camera/record_camera_stream.py /tmp/flight.bvcam --frames 300
```

After stopping the renderer, replay a recording to mission vision without
starting PX4, Gazebo, or Bevy:

```sh
python3 tools/camera/replay_camera_stream.py /tmp/flight.bvcam --loop
```

The default TCP endpoint is `127.0.0.1:7002`. Override `--host`/`--port` when
needed. A Dockerized mission container should connect to its host gateway.
Frame validation covers payload boundaries, sequence/time ordering, intrinsics,
and simulation resets. Image content may stay identical while the drone is
stationary; use `--require-change` when actually moving the simulated camera.

```sh
python3 -m unittest discover -s tools/camera -p 'test_*.py'
```

The old JSON/TCP **state** recorder and geometry verifier were intentionally not
copied, because their state receiver is incompatible with the binary UDP link.
The Rust `state_probe` example verifies the new state subscription directly.
