# Architecture

## Current dependency boundaries

The workspace uses Rust 2024. Exact graphical dependency choices are recorded in the manifests, Cargo.lock and DEPENDENCY_NOTES.md. Do not upgrade them independently or replace the native client with a browser wrapper.

| Package | Current responsibility |
| --- | --- |
| osb-world | Validated bounded 2D map geometry, geographic origin/local metre conversion, simple OSM JSON import, obstacle LOS and navigation. |
| osb-content | Validated generic equipment definitions. Initial loadout construction still expects the baseline content IDs. |
| osb-planner | Structured intent and validation against the map. No network client or language model integration. |
| osb-sim | GPU-independent reference simulation, ECS storage, identities, clock/RNG, formations, knowledge, behavior, combat, health and events. |
| osb-campaign | Transactional SQLite checkpoints containing versioned compressed simulation snapshots. This is persistence infrastructure, not a campaign simulator. |
| osb-map | Presentation-only map camera and coordinate transforms. No MapLibre backend yet. |
| osb-headless | CLI scenario loading, simulation, checkpointing, event export and performance probes. |
| osb-client | Bevy native window/rendering, egui interface and offline geometry drawing. Drives the same simulation as the headless executable. |

`tools/check_boundaries.py` checks the headless dependency graph. Rendering, windowing and network clients do not belong in the core graph.

The current simulation runs on the observer's main thread with a bounded frame work budget. A separate simulation worker is a future optimization, not an implemented feature. The graphical world does not own the simulation's ECS world.

## Identity and organization

`SoldierId`, `FormationId` and `FactionId` are serialized domain identities. Bevy `Entity` values are runtime storage handles only. Death retains the person. Render culling must never mutate the roster.

A soldier's assigned formation and current group are distinct. Initial organization is generated as platoons, squads and fireteams; hierarchy changes and leadership succession operate on those identities. This is not yet a complete army/corps/battalion order-of-battle editor.

Relationships are sparse references to persistent people, not a dense all-to-all matrix. The foundation seeds simple bonds and uses them in basic behavior. A full social/personality simulation remains 1.0 work.

## Time and reproducibility

The reference clock is an integer tick counter at 10 ticks per simulated second. The simulation never reads wall time or camera state. The client may interpolate display positions, but it may not interpolate authoritative injuries, ammunition or decisions.

Randomness has explicit serialized state. Stable iteration and a single authoritative update sequence are the reference contract. Tests establish repeatability and exact checkpoint continuation on the tested build/platform. Do not advertise universal cross-architecture bitwise determinism without testing it, especially while geometric calculations use floating point.

Speed controls ask the runner to do more simulation work. They do not skip injury updates or silently discard ticks to hit a requested speed. Maximum speed is workload-dependent.

## Perception, command and combat

For 1.0 the desired model is local LOS plus communication. In the current slice, simple opaque building polygons obstruct sight and shots; nearby/radio contact sharing is deliberately limited. A shared report does not grant the recipient permission to shoot through a building.

Facing/FOV and persistent stale intelligence belong to 2.0. Attention and recognition belong to 3.0. Do not implement omniscient command as a hidden shortcut while the UI calls soldiers locally informed.

Intent is structured data. Code-driven formation and individual behavior execute it. The current commander logic is a small heuristic prototype, not an operational planning engine. A future LLM adapter may produce candidate intent only. Validate it, show it to the scenario author, then persist it. No model inference belongs in a tick, perception update or soldier decision loop.

Combat and trauma are coarse reference models. Wounds affect function and may bleed; treatment is not an instant restoration to full health. Resources must be conserved. Numeric tuning is provisional and must not be presented as medical or military validation.

## Geography

Keep source coordinates and a geographic origin, but use local metre coordinates for bounded tactical calculations. Screen pixels and Bevy transforms are presentation only. A regional frame is not a global map projection suitable for arbitrary continent-sized campaigns.

The authoritative map is independent from the visual map. An eventual pretty tile layer cannot become the source of collision or LOS truth. Importing missing, incomplete or unsupported features must surface limitations rather than inventing reliable terrain.

## Persistence and events

Current checkpoints store a complete versioned snapshot in SQLite, compressed with zstd and protected by a BLAKE3 digest. The snapshot includes people, formation state, map/content, clock, random state and recent events. Saves retain checkpoints transactionally. Unknown/incompatible schemas and invalid snapshots are rejected.

This is not an unlimited event-sourced database. The in-memory event history is bounded. The headless runner can stream new events to JSON Lines. A replay UI, long-term event archive, recovery tooling and schema migrations are future work.

## Campaign-compatible foundations, not an implemented campaign

Persistent identity does not imply continuously updating every person at tactical frequency. Future operational abstraction may schedule groups and materialize detailed state, but it must preserve identities, inventories, injuries and meaningful relationships.

Required invariants before calling LOD usable:

1. People and resources are conserved across every transition.
2. Injuries, time-dependent needs and ongoing hazards do not freeze offscreen.
3. Camera movement does not reroll casualties or change the authoritative outcome.
4. Opposing groups crossing resolution boundaries share consistent time and contact state.
5. Save/resume works during transitions and simultaneous battles.
6. Error and performance are measured against the reference model on representative workloads.

These invariants guide the architecture now. The operational scheduler, logistics world, reinforcements and multi-battle campaign are not implemented yet.
