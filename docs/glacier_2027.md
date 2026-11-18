# Glacier 2027

`glacier_2027` is a simulator world inspired by the open meadows, paved trails,
ponds and wooded edges of Glacier Ridge Metro Park near Dublin, Ohio.
The scene uses a flat physics surface and places the launch point at
`40.15861502, -83.19696844` with a simulated ground elevation of 0 m.
[Glacier Ridge Metro Park](https://www.metroparks.net/parks-and-trails/glacier-ridge/)

The search meadow contains a prone mannequin in approximately the same relative
position as the SUAS 2026 mannequin, 30% east and 35% north across the rectangle.
A suitcase sits 8 m east and 6 m north of it. The current autonomy detector and
mission recognize `person` and `tent`. The suitcase is a visual prop with the
current mission.

From the `bv_sim` checkout:

```sh
cargo run -- run glacier_2027
```

## Simulator mission coordinates

The four lap points in order form a **632 m** closed loop, 20% of the
**3,157 m** loop used by the SUAS 2026 simulator site. The existing mission
repeats the loop twice for about **1.26 km** before the search, excluding transit.
The search rectangle is **262 × 147 m**, about **3.86 ha**, matching the SUAS
2026 simulator search area. The rendered world has no waypoint or boundary
markings. These coordinates are for simulator use.

The autonomy mission accepts `(latitude, longitude)` pairs via
`WaypointSequence.at_alt`. The lap list begins at the southwest corner and
proceeds counterclockwise. The scan boundary is ordered southwest, southeast,
northeast, northwest. Omit the repeated starting point because the mission
closes the lap itself. The scan planner uses the mission's scan altitude of
48.768 m and 35% overlap to generate three rows.

```python
from dataclasses import replace

from bv_autonomy.missions.suas2026 import sites
from bv_autonomy.missions.suas2026.suas2026 import LAP_ALT
from bv_autonomy.types import WaypointSequence

glacier_2027 = replace(
    sites.GLACIER,
    laps=WaypointSequence.at_alt(LAP_ALT, [
        (40.15798549, -83.19808632),
        (40.15798549, -83.19596823),
        (40.15920587, -83.19596823),
        (40.15920587, -83.19808632),
    ]),
    scan_area=WaypointSequence.at_alt(0.0, [
        (40.15835871, -83.19768624),
        (40.15835871, -83.19460323),
        (40.15968072, -83.19460323),
        (40.15968072, -83.19768624),
    ]),
)
```

Use this site with `SimPlatform` for a fresh simulated mission. Saved mission
state contains the previous route, so start fresh when switching fields.
