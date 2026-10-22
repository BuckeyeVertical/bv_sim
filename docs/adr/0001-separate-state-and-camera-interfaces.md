# Separate simulation state and camera interfaces

The physics backend supplies authoritative state to the native renderer using binary UDP messages. The renderer supplies camera images through a separate camera-output interface consumed by vision software. State updates favor the newest complete snapshot, while camera delivery has its own framing, encoding, and compatibility requirements; keeping these interfaces separate also lets simulated camera output evolve toward the physical drone's video interface.
