# Preserve mission compatibility and existing physics for the first port

The first port includes only the SUAS 2026 visual world and its required assets. Forest, satellite imagery, and the waypoint editor are outside this rewrite.

It retains the camera-frame v1 JPEG/TCP interface and existing vehicle/flat-ground physics while moving authoritative state to binary UDP. Gazebo carries only the required vehicle dynamics, sensors, gimbal definitions, and ground collision. All scenery and visual meshes belong to Bevy. Native releases target macOS Apple Silicon, Linux x86-64, and Windows x86-64. This establishes a working replacement for the current simulator without requiring mission-stack changes, adding native video-encoder dependencies, or implying that rendered obstacles already have collision geometry. Standard encoded video and matching terrain/obstacle collisions are separate follow-on work.
