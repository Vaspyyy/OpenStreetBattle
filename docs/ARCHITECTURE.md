# Architecture

## Current dependency boundaries

The workspace uses Rust 2024. Graphical dependencies and their compatibility patches are recorded in the manifests, Cargo.lock and DEPENDENCY_NOTES.md. Do not upgrade them independently or replace the native client with a browser wrapper.

| Package | Current responsibility |
| --- | --- |
| osb-world | Validated bounded 2D geometry, geographic/local metre conversion, battle-region validation, simple OSM JSON import, obstacle LOS and navigation. |
| osb-geodata | Offline geometry compilation and clipping, content-addressed snapshots, integrity verification and provenance. No network or renderer. |
| osb-content | Validated generic equipment definitions. Initial loadout construction still expects baseline content IDs. |
| osb-planner | Structured intent and map validation. No network client or language-model integration. |
| osb-sim | GPU-independent reference simulation, ECS storage, identities, clock/RNG, formations, knowledge, behavior, combat, health, events and non-authoritative decision diagnostics. |
| osb-campaign | Transactional SQLite checkpoints containing versioned compressed snapshots. Persistence infrastructure, not a campaign simulator. |
| osb-map | Presentation-only tactical and global browsing cameras and coordinate transforms. |
| osb-map-native | Optional, isolated MapLibre Native Vulkan owner thread, bounded frame exchange and dedicated device ownership. Located under native/, outside the core workspace. |
| osb-headless | Offline scenario/snapshot loading, simulation, checkpoints, event export and performance probes. |
| osb-client | Bevy Wayland/Vulkan observer, egui interface, geometry drawing, optional World view and explicit bounded geographic acquisition. |

`tools/check_boundaries.py` checks the headless dependency graph. Rendering, windowing and network clients do not belong in that graph. The graphical world does not own the simulation's ECS world.

The simulation runs on the observer's main thread with a bounded frame work budget. A separate ongoing simulation worker is a future optimization. Geography acquisition/preparation and native basemap rendering have their own workers; neither updates an active battle. World browsing pauses and preserves the current battle. Installing a prepared scenario is an explicit replacement boundary.

## Identity and organization

`SoldierId`, `FormationId` and `FactionId` are serialized domain identities. Bevy `Entity` values are runtime storage handles only. Death retains the person. Render culling must never mutate the roster.

Assigned formation and current group are distinct. Initial organization is generated as platoons, squads and fireteams; hierarchy changes and leadership succession operate on those identities. This is not a complete army/corps/battalion editor.

Relationships are sparse references to persistent people, not a dense all-to-all matrix. The foundation seeds simple bonds and uses them in basic behavior. A full social/personality simulation remains 1.0 work.

## Time and reproducibility

The reference clock is an integer tick counter at 10 ticks per simulated second. The simulation never reads wall time or camera state. Display positions may be interpolated, but authoritative injuries, ammunition and decisions may not.

Randomness has explicit serialized state. Stable iteration and a single authoritative update sequence form the reference contract. Tests establish repeatability and exact checkpoint continuation on the tested build/platform. Floating-point geometry means cross-architecture bitwise determinism needs separate validation.

Speed controls request more simulation work, not skipped injury updates or discarded ticks. Maximum speed is workload-dependent.

## Perception, command, combat and diagnostics

For 1.0 the desired model is local LOS plus communication. Simple opaque building polygons currently obstruct sight and shots; nearby/radio sharing is deliberately limited. A shared report does not grant permission to shoot through a building.

Facing/FOV and stale intelligence belong to 2.0. Attention and recognition belong to 3.0. Do not implement omniscient command as a hidden shortcut.

Intent is structured data executed by code-driven formations and individuals. Commander logic is a small heuristic prototype, not an operational planning engine. Any future LLM adapter produces candidate intent only: validate it, show it to the author, then persist it. Model inference does not belong in simulation ticks.

Combat and trauma are coarse reference models. Wounds affect function and may bleed; treatment is not instant full recovery. Resources must be conserved. Numeric tuning is provisional, not medical or military validation.

Decision traces are emitted from the actual decision/navigation branches with sampled inputs, reasons and a bounded recent history. They consume no randomness and do not feed back into behavior. They are excluded from authoritative saves; trace on/off must leave fingerprints unchanged. Inspector explanations must not be invented from the final action after the fact.

## Geography and native rendering

Use a geographic origin and local metres for tactical calculations. Global browsing uses Web Mercator only for presentation. A regional frame is not a continent-sized campaign projection.

The optional live tile layer is independent from authoritative geometry. Provider pixels, zoom simplifications and missing features cannot define trustworthy collision/LOS. The current compiler clips supported building ways and road polylines to a validated region, records provenance and warns about unsupported data. See MAP_DATA.md and REAL_WORLD.md for exact limits.

A geography snapshot contains raw source, compiled geometry, manifest and attribution. Its identity covers source, region, endpoint and compiler/schema version. Loads verify hashes and provenance. Checkpoints embed compiled geometry: provider updates cannot move an ongoing battle's buildings.

Native MapLibre runs on its own owner thread and dedicated Vulkan device. Session/map/runtime destruction precedes device destruction, and worker shutdown is joined before application exit. The initial bridge is bounded RGBA readback/upload with camera reprojection, not zero-copy sharing with Bevy. Unsafe handle work remains isolated to the Vulkan ownership module, never the simulation.

## Persistence and events

Checkpoints store a complete versioned snapshot in SQLite, compressed with zstd and protected by a BLAKE3 digest. People, formations, map/content, clock, random state and recent events are included. Checkpoints are retained transactionally; incompatible schemas and invalid snapshots are rejected.

This is not an unlimited event-sourced database. In-memory history is bounded; the headless runner can stream JSON Lines. Replay UI, long-term archives, recovery tooling and schema migrations remain future work.

## Campaign-compatible foundations, not an implemented campaign

Persistent identity does not imply continuously updating every person tactically. Future abstraction may schedule groups and materialize detailed state while preserving identities, inventories, injuries and relationships.

Before calling LOD usable, verify conservation across transitions; time-dependent injuries, needs and hazards offscreen; camera-independent outcomes; consistent crossing/contact time between resolutions; save/resume during transitions and concurrent battles; and measured abstraction error/performance against the reference model.

The operational scheduler, logistics world, reinforcements and multi-battle campaign are not implemented yet.
