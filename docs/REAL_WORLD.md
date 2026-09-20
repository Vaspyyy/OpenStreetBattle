# Real-world scenario slice

This is the next bounded sandbox slice, not the campaign or complete terrain model.

## Launch

The original offline observer remains the default:

```bash
cargo run --locked -p osb-client
```

For the native live basemap, on Linux x86_64 with a working Vulkan driver:

```bash
bash tools/run-live.sh
```

The helper downloads one checksum-pinned MapLibre Native Vulkan archive into the user's cache, verifies its source revision and platform, and supplies the native install prefix to Cargo. It does not require sudo or install a desktop service. Rust bindings and the C library are pinned to the same upstream revision. A changed or unavailable upstream artifact fails closed instead of silently upgrading.

Native compilation also requires a C/C++ toolchain, pkg-config, Wayland/libxkbcommon development files, libclang (for bindgen) and Vulkan development files. On Arch-based systems the relevant packages include `base-devel`, `pkgconf`, `wayland`, `libxkbcommon`, `clang` and `vulkan-headers`, plus the Vulkan loader and the appropriate installed GPU driver. On Ubuntu the CI uses `libwayland-dev libxkbcommon-dev libvulkan-dev libclang-dev` and Mesa software Vulkan for validation. Do not install a different hardware driver just to reproduce the CI environment.

## Use the World view

Open **World** in the header. The current battle is paused and retained. Enable the live basemap explicitly, pan/zoom, or enter latitude and longitude and select **Go to coordinates**. The first implementation deliberately has no public geocoding/autocomplete service.

Select a square at the view center, or enable the area-selection tool and drag. Each side must be 100 to 2000 metres, at most 4 km². Battle creation supports the core's local-projection envelope, latitudes within ±80°, without crossing the date line. Browsing itself uses north-up Web Mercator and is separate from these tactical limits.

Choose **Fetch / reuse this area**. This first checks the verified local cache, then issues at most one bounded Overpass request when needed. The bundled public endpoint is for occasional, explicitly acknowledged development use. Configure your own or an appropriate provider endpoint for sustained usage. There is no background geographic prefetch, bulk region downloader, or automatic retry loop.

Review the snapshot, source timestamp where supplied, geometry counts and limitations. Creating the new battle requires acknowledging replacement of the current unsaved battle. A download, parse or validation failure does not replace that battle. Cancellation cannot always interrupt an in-flight network operation immediately; the bounded request finishes or times out, and its result is not installed after cancellation.

The battle view draws authoritative imported geometry, not the live map image. Buildings drawn by the visual provider are not necessarily supported collision geometry. The observer continues to offer force placement, objectives, play/pause, soldier inspection and manual checkpoints.

## Data and offline behavior

A cached region contains `source.json`, `compiled.json`, `manifest.json`, and `attribution.txt`. Its content-addressed ID covers source bytes, selected bounds, compiler/schema version and endpoint. Source and compiled hashes are checked when loading. Writes are validated, staged and atomically renamed; corrupt existing snapshots are rejected, not silently replaced. Reusing the exact region/endpoint reuses the existing snapshot rather than automatically refreshing OSM data.

The cache defaults to `$XDG_CACHE_HOME/openstreetbattle/geography`, or `~/.cache/openstreetbattle/geography`. `OSB_GEO_CACHE` overrides this path. Use the cached-region list without any external request. There is no automatic eviction or download of a whole state/country. Per-import size/geometry limits are enforced; manage total cache disk usage manually in this slice.

Scenario/checkpoint state embeds compiled geometry and provenance. A saved battle does not require the live provider or raw-source cache to resume, and later OSM changes cannot move its buildings. The native basemap uses a separate opportunistic tile cache; this is not a guarantee of offline world browsing. Only a prepared/cached battle snapshot has the offline reproducibility guarantee.

The headless runner can create a battle from a verified cached snapshot:

```bash
cargo run --locked -p osb-headless -- \
  --geo-snapshot SNAPSHOT_ID \
  --geo-cache /path/to/geography \
  --duration 8m --save saves/geographic.osb
```

Normal checkpoint continuation is unchanged:

```bash
cargo run --locked -p osb-headless -- --load saves/geographic.osb --duration 1m
```

## Current terrain limits

The compiler supports simple closed building ways and road polylines. It clips both to selected bounds, including disconnected pieces of concave footprints. It rejects incomplete/error server responses and skips incomplete or unsupported features with warnings. It never joins missing road nodes by inventing a straight connection.

No building interiors, elevation, forests, water barriers, bridge topology, multi-level roads or multipolygon relations are simulated yet. Missing data is **unknown**, not evidence that terrain is clear. Routes and LOS currently use the simplified 2D geometry model. Do not treat this as a real-world tactical planning tool or a validated battlefield model.

## Decision inspection

The soldier inspector's **Why this decision?** section records the branch actually taken by the coded AI, sampled input values, formation directive and navigation result. It distinguishes an intended movement from failures such as no route, blocked segment or waiting to repath. It retains a bounded recent history per person.

Diagnostics neither consume randomness nor feed back into decisions. They are not authoritative save data, so their history starts fresh after loading. Turning tracing on/off must produce the same simulation state for the same scenario, seed and ticks.

## Native renderer boundary

`native/osb-map-native` is an optional client-only dependency. Its owner thread contains the MapLibre runtime and a dedicated Vulkan device. Rendered images use bounded RGBA readback/upload at up to ten updates per second, with immediate camera reprojection between updates. This is a correctness-first integration, not zero-copy GPU sharing or a promise of final map performance.

Unsafe handle work is isolated to the documented Vulkan ownership module. No Vulkan pointer is shared with Bevy, and simulation crates retain their unsafe-code prohibition. Native session/map/runtime destruction precedes Vulkan-device destruction.

## Attribution and services

Visual basemap: OpenFreeMap / OpenMapTiles / OpenStreetMap contributors. Authoritative geography: OpenStreetMap contributors, ODbL 1.0, with source and timestamps recorded in each snapshot. Provider imagery and simulation snapshots are acquired independently and may differ in date or detail.

Reference policies and documentation:
- https://openfreemap.org/quick_start/
- https://maplibre.org/maplibre-native-ffi/install/
- https://dev.overpass-api.de/overpass-doc/en/preface/commons.html
- https://www.openstreetmap.org/copyright

`OSB_MAP_STYLE` overrides the live style URL and `OSB_OVERPASS_ENDPOINT` overrides the geography endpoint. Enabling a style can contact that style's tile/glyph/sprite sources. Do not put credentials into shared scenario files or committed configuration.

## Validation modes

Normal CI checks core behavior, offline acquisition tests, native Wayland/Vulkan rendering, and a synthetic MapLibre GeoJSON fixture without public map requests. `--world-smoke --live-smoke` is a separate explicit network acceptance mode, not the default CI path. Software Vulkan smoke tests do not certify KDE/NVIDIA hardware, high-DPI interaction, or every UI gesture.
