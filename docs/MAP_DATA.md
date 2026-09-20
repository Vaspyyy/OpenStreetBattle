# Map data

## Three supported entry paths

The bundled First Contact map remains synthetic, hand-authored geometry. It is a deterministic offline fixture, not a downloaded town.

Both executables also accept an existing small OSM/Overpass JSON export without network access:

```bash
cargo run --locked -p osb-client -- --map path/to/export.json
cargo run --locked -p osb-headless -- --map path/to/export.json --duration 8m
```

That legacy importer accepts simple ways with inline geometry or references to included nodes. OSM XML/PBF and arbitrary GeoJSON are not supported inputs.

The new World view supplies the third path: browse an optional native live basemap, select a bounded region, explicitly acquire/reuse authoritative geography, review warnings, and create a battle. Run `bash tools/run-live.sh` to enable MapLibre Native Vulkan. Full instructions and provider/cache constraints are in [REAL_WORLD.md](REAL_WORLD.md).

## Visual and authoritative pipelines

The World view renders OpenFreeMap vector tiles using MapLibre Native. It is presentation only. It does not determine collision, visibility or traversability, and may differ in date or supported detail from a battle snapshot. The current battle view draws authoritative geometry rather than that visual tile image.

`osb-geodata` separately compiles supported OpenStreetMap building ways and road polylines. It clips features to selected bounds before local projection, including disconnected pieces of concave footprints. Incomplete ways are skipped instead of inventing connections. Server error/partial-response remarks are rejected.

Each battle region is 100 to 2000 metres on either side, at most 4 km², within the local projection's latitude envelope and without crossing the date line. These limits bound this tactical slice, not the eventual campaign design. Source input is capped at 32 MiB; element, vertex, geometry and navigation limits also apply.

## What remains unknown

No interiors, windows, entrances, elevation, floor/roof heights, vegetation, water barriers, bridge-layer connectivity, multi-level roads or multipolygon relations are simulated yet. Buildings are opaque non-walkable 2D obstacles, not occupied 3D structures. A displayed highway is not proof of a complete routable transport network.

Missing or unsupported data is unknown, not verified clear ground. The new cache guarantees the identity of the simplified geometry used, not the physical completeness or realism of the terrain.

## Snapshot and cache contract

A geographic snapshot stores source.json, compiled.json, manifest.json and attribution.txt. IDs cover source bytes, bounds, compiler/schema version and endpoint. Source/compiled hashes and embedded provenance are checked on load. Validated writes use a staged directory and atomic rename; corrupt existing snapshots fail closed.

Exact cached region/endpoint reuse does not refresh OSM automatically. New battlefield installation requires an explicit review/replacement action. Downloads, parsing errors or cancellation do not replace the active battle.

Checkpoints and exported scenarios embed compiled geometry and its provenance. Loading a saved battle does not need a live provider or the original raw-source cache. The native basemap's separate opportunistic tile cache is not a promise of offline world browsing or whole-country downloads.

## Hosting and attribution

Keep visible and exported OpenStreetMap attribution and source metadata. Do not treat public community servers as the shipping game's geographic backend. The default public Overpass endpoint is restricted in the UI to explicitly acknowledged, occasional development requests. There is no automatic retry, background region prefetch or bulk downloader; configure an appropriate provider/self-hosted endpoint for sustained use.

The offline demo needs no external service. The repository embeds no API key. Enabling the live style contacts its style/tile/glyph/sprite providers. Dataset and service requirements must be reviewed again before distributing geography bundles or shipping a hosted service.
