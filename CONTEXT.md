# BV Simulation

The simulated flight environment used to develop and evaluate the drone's mission and vision behavior.

## Language

**Scenario**:
A named flight environment with a geographic origin, vehicle, and onboard camera.
_Avoid_: World profile

**Physics world**:
The physical environment governing vehicle motion, simulated flight sensors, and collisions.

**Visual world**:
The visible environment observed through a simulated camera. Visible geometry can exist without corresponding physical collision geometry.

**Simulation run**:
One continuous simulation timeline. A restart or reset begins a new run.

**State snapshot**:
The authoritative positions and orientations of the configured simulated entities at one simulation time.

**Geographic origin**:
The location on Earth corresponding to the local world's origin. It is distinct from a vehicle's return-home position.

**Onboard camera**:
The simulated camera carried by the vehicle or its gimbal, whose images are available to vision software.

**Observer camera**:
The viewpoint used by a person to inspect the simulation.
_Avoid_: Drone camera

**Camera frame**:
One captured onboard-camera image and its associated capture information.

**Ground computer**:
The computer outside the aircraft that can run mission and vision software.
