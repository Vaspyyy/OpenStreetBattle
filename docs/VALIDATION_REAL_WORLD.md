# Real-world slice validation

Evidence collected on 20 September 2026. This validates the current bounded prototype, not complete terrain, campaign scale or every desktop configuration. The earlier VALIDATION.md is the historical 0.1 record.

## Reference tests and native rendering

Run [35505171934](https://github.com/Vaspyyy/OpenStreetBattle/actions/runs/35505171934) applied source commit `251e26eb41368bf4881e0d0d08cf9ff5f89161a7` and passed 72 core tests, 5 client tests in both offline/live-feature configurations, strict Clippy and formatting, headless dependency boundaries and X11 exclusion.

It launched the actual native Wayland/Vulkan observer with an offline MapLibre GeoJSON fixture, then a live OpenFreeMap viewport, then the original battle. Both World runs kept the battle at tick zero with all 32 identities present. The live renderer returned a fully loaded frame and exited cleanly. A detached native-worker shutdown failure found in earlier testing was fixed by joining owner-thread teardown.

## Real geographic acquisition and battle

Run [35505946308](https://github.com/Vaspyyy/OpenStreetBattle/actions/runs/35505946308), source `d6f53919e31f4914582f421078041c3376e77cd7`, passed the explicit live acquisition test, graphical rendering of the resulting scenario and another live basemap render with the attribution contrast fix.

One small fixed Berlin region, south/west/north/east `52.524, 13.405, 52.528, 13.411`, produced 243 compiled building footprints and 323 road polyline parts. The source timestamp was `2026-09-20T10:45:11Z`. Seven building/multipolygon relations were not simulated and were reported as warnings, not silently represented as supported terrain.

Snapshot ID: `0cf9ad97baa8cc9e2575144fb0a2bf01fb5d5e67c0ffb6d134d98aca4ff2f5dd`.

The test built the same simulation used by the observer, advanced it, wrote a SQLite checkpoint, restored it and compared exact fingerprints after further continuation. All 32 persistent identities remained. The native observer subsequently loaded the exported real-geography scenario, rendered it and exited at tick 78 with 32 people. The World render kept tick zero and exited with OSB_BASEMAP_OK. Both 1440 x 900 screenshots were visually inspected.

Artifact `final-map-acceptance` contains source revision, tests/logs, world-live.png, real-battle.png, the scenario/checkpoint, source/compiled geographic snapshot, summary and attribution. The manual workflow uses absolute artifact paths because Cargo unit tests run from their package directory.

## Offline and failure paths

Normal tests cover exact cached-region reuse with no listening server, cancellation before connection, HTTP rate limits, partial/error server responses, corrupted snapshot rejection, geographic provenance across checkpoint/resume, concave clipping that must not invent a connecting obstacle, and missing road nodes that must not invent connections.

Decision diagnostics are emitted by actual behavior/navigation branches. Tests compare tracing enabled/disabled and prove that diagnostic history does not change the authoritative fingerprint. Trace history intentionally starts fresh after restore.

## Limits of this evidence

Environment: Ubuntu 24.04 CI, headless Weston, Mesa software Vulkan, unoptimized development builds. This is not KDE/NVIDIA hardware, high-DPI or exhaustive mouse/keyboard interaction validation. A non-fatal XDG settings portal timeout and a native-provider line-geometry warning appeared in logs without preventing successful renders.

The 32-person dense urban probe is not a throughput benchmark. CPU geometry/navigation costs, larger areas and larger forces still need profiling. The battle geometry renderer has rough road-label placement, including repeated/overlapping labels on clipped road pieces. The World basemap uses MapLibre's label layout instead.

No interiors, elevation, vegetation, water barriers, bridge topology or multipolygon simulation is claimed. Native basemap pixels remain separate from collision/LOS geometry. Normal CI never contacts public map/geodata providers for runtime tests; external acceptance is explicit and manual. Check the latest CI for subsequent source changes rather than treating these historical runs as universal proof.
