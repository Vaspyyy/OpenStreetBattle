# OpenStreetBattle

A Linux-native, Wayland-only, Vulkan-first **spectator warfare sandbox**. Set up forces and intent, press Play, and inspect the persistent people making decisions underneath the dots.

**Status: runnable 0.1 foundation, not the 1.0 release.** The current game is a bounded, offline battle prototype. The long-term goal is concurrent battles and tens of thousands of persistent individuals over simulated years.

## Run the native observer

Use a native Wayland session with a working Vulkan driver and Rust installed through rustup. The repository pins Rust 1.95.0; Cargo selects it through `rust-toolchain.toml`. Native build dependencies include a C/C++ toolchain, pkg-config, Wayland development files and libxkbcommon development files. SQLite is built from the bundled dependency.

```bash
 git clone https://github.com/Vaspyyy/OpenStreetBattle.git
 cd OpenStreetBattle
 cargo run --locked -p osb-client
```

For an existing checkout, pull the current `main` before running. The first build compiles Bevy and its dependencies. No API key, external map service, or paid model is needed to run the bundled scenario.

The window opens paused. Press **Space**, or click **Play**. The forces initially have to approach the objective; use **5x or 10x** to see contact sooner. Drag the map to pan, scroll to zoom, and click a person to inspect them.

| Control | Action |
| --- | --- |
| Space | Pause / resume |
| N | Advance one simulation tick while paused |
| 1 / 2 / 3 / 4 / 5 | 1x / 2x / 5x / 10x / maximum available speed |
| F5 / F9 | Save / load the manual checkpoint |
| Scenario setup | Edit seed, force sizes, starting positions and objectives before Play |
| Person inspector | Follow a soldier; inspect injuries, morale, equipment, relationships and contacts |

Default saves go to `$XDG_STATE_HOME/openstreetbattle/checkpoint.osb`, or `~/.local/state/openstreetbattle/checkpoint.osb` when XDG_STATE_HOME is unset. Saves are manual. Resetting a scenario discards unsaved progress.

## Run without a display or GPU

```bash
cargo run --locked -p osb-headless -- --duration 8m --seed 42 --save saves/demo.osb --events events.jsonl
cargo run --locked -p osb-headless -- --load saves/demo.osb --duration 1m
cargo run --locked -p osb-headless -- --write-demo scenario.json
```

The observer and headless runner use the same simulation, not separate approximations. A duration supplied with `--load` is additional simulated time. Use `cargo run --locked -p osb-headless -- --help` for all options.

## What exists now

- A native Bevy/egui observer with Vulkan-only renderer selection and no X11 window feature; pan/zoom, individual inspection, event log, pre-battle editing, time controls and checkpoint controls.
- A fixed 10 Hz, seeded reference simulation with stable soldier IDs, code-driven platoon/squad/fireteam organization, leader succession, local LOS/contact sharing, simple combat and suppression, morale, wounds, finite resources and basic social/medical behavior.
- Polygon-blocked 2D LOS and navigation around obstacles; geographic origins with local metre coordinates; an offline map camera and a deliberately synthetic demo map.
- Local OSM/Overpass JSON import for supported simple building ways and road polylines. Pass `--map path/to/export.json` to either executable, or use the observer's import control before starting.
- Validated JSON scenarios and generic equipment data; a validated intent boundary with **no live LLM adapter or API calls**.
- Versioned, compressed SQLite checkpoints with integrity verification, retained checkpoints, identity preservation and exact same-build save/resume regression tests.
- Headless dependency checks, behavioral tests, native Wayland/software-Vulkan smoke testing and a small performance probe.

These are prototype implementations, not a claim of validated battlefield realism. Read [architecture](docs/ARCHITECTURE.md) and [development / validation](docs/DEVELOPMENT.md) for the actual boundaries.

## Important limits

**The live MapLibre/OpenFreeMap basemap is not integrated.** The bundled map is hand-authored, not a fetched real town. Local OSM import is not a full terrain pipeline: no interiors, elevation, forests, rivers, bridge topology or multipolygon relations yet. Missing geography is unknown, not proof of open terrain. See [map data](docs/MAP_DATA.md).

There is no campaign mode, simulation LOD, vehicle/supply network, sophisticated operational planning, complete social model, or validated tens-of-thousands combat capacity yet. Grenades are currently carried data, not implemented attacks. Performance probes are not evidence of campaign-scale battle performance.

## Design rules

Simulation state belongs to the GPU-independent Rust core. A marker is not a person: removing a visual must never remove an identity, relationship, injury or inventory. AI decisions use perception and communicated knowledge, not hidden enemy state. Optional future language-model parsing stops at validated intent; no API calls control simulation ticks.

**Camera movement must not change who lives, dies or consumes supplies.** Future abstraction must preserve people and resources and be tested against the reference simulation. Saves, explicit randomness and repeatable headless tests come before scale claims.

## Agreed releases

| Release | Scope |
| --- | --- |
| 1.0 | Battle sandbox; individual people; local LOS and sharing; autonomous command and dynamic groups; tactical ammo/medicine; coarse trauma and relationship-sensitive casualty assistance; fictional contemporary equipment. |
| 2.0 | Facing / field of view; stale, incomplete and potentially wrong reports. |
| 2.5 | Official modern, Cold War and WW2 content, plus engine work wherever those eras genuinely need different mechanics. |
| 3.0 | Attention and richer perception; battlefield supply networks; casualty evacuation. |
| 3.5 | Persistent campaigns, simultaneous battles, encirclements, reinforcements, simulated weeks/years, validated multi-resolution simulation and tens of thousands of persistent people. |

See [the roadmap](docs/ROADMAP.md) for release gates rather than promises that a foundation field equals a finished feature.

KDE Plasma is the priority desktop environment, not a runtime dependency. CI uses a headless Wayland compositor and software Vulkan; that does not replace testing on real KDE/NVIDIA/AMD/Intel systems.

The owner has not selected a source-code license yet. External geographic data retains its own attribution and licensing requirements.
