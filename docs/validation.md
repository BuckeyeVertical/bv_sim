# Local validation

Verified on 2026-10-21 with a macOS Apple Silicon host and a native Linux arm64
Docker backend:

- 63 Rust tests passed across the protocol, renderer, and launcher.
- Five backend CTest tests passed during the full Docker image build, including
  Gazebo pose composition and both independently generated language fixtures.
- Backend protocol/socket tests passed with AddressSanitizer and
  UndefinedBehaviorSanitizer.
- Twelve Python camera-contract tests and the installer test passed.
- Generated bindings exactly matched the schema and pinned FlatBuffers compiler.
- `bv-sim run suas_2026 --build` built the image, waited for health, and opened
  the native renderer. Docker inspection confirmed zero mounts and the SUAS
  origin passed to PX4.
- Five 1920×1080 JPEG camera frames passed payload, intrinsics, run identity,
  sequence, and simulation-time validation through the real UDP/render/TCP path.
- Ctrl+C exited successfully and removed the managed container. The existing
  `bv-simulator` and `bv-mission` containers remained stopped.
- The SUAS observer window rendered correctly. Offscreen screenshot capture
  also produced a valid image while running in the background.
- The native bundle was installed into a temporary home containing spaces and
  an apostrophe. After deleting its extracted download, the installed PATH
  command found its own assets and scenarios from an unrelated working directory.

The local image is `bv-sim-backend:0.1.0`. No registry image or GitHub release
was published. Linux x86_64 and Windows x86_64 native builds/tests are configured
in CI; those native targets have not been executed locally. The ROS mission stack itself was not
started during these checks; the existing camera wire contract was verified.

## Windows and backend cleanup

- A real Windows x86_64 PE executable was cross-compiled locally with the GNU
  target and packaged into `dist/bv-sim-0.1.0-x86_64-pc-windows-gnu.zip`. Its
  direct DLL imports contain Windows system/UCRT libraries, with no MinGW runtime
  DLL to bundle. This local artifact is a development build; release CI builds
  the optimized MSVC executable with the CRT linked statically.
- The PowerShell installer was exercised locally in file-only mode through
  PowerShell 7: paths with spaces, apostrophes, and brackets, deletion of the
  extracted download, repeat installation, and removal of stale release data
  passed. Native Windows CI additionally executes the installed `.exe`, checks
  its resolved asset paths, and verifies/restores the user PATH.
- Windows rendering, Docker Desktop networking, and Ctrl+C behavior have not
  been executed on a Windows machine locally. Native Windows CI has not been
  triggered from this local repository.
- Three physics-export tests passed. The exported runtime contains exactly four
  SDF vehicle/gimbal definitions and one ground-only world, with no visual
  elements or mesh assets. Required inertia, collisions, joints, sensors, and
  controllers are preserved. Scenery placeholders and the offline patch are gone.
  The unused Compose alternative and duplicate release metadata are removed;
  the launcher, `VERSION`, and CI workflow matrices define startup and releases.
- The rebuilt Linux backend passed all five C++ tests, reached healthy status
  with zero mounts, and sent complete state through Docker's published UDP port.
- PX4 performed an actual takeoff from about -0.013 m to over 1.68 m. The native
  renderer delivered 120 valid 1920×1080 JPEG frames during the climb, with
  increasing simulation timestamps and changing image content.
