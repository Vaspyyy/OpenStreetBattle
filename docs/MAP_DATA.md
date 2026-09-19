# Map data: current backend and future integration

## Current behavior

The default First Contact map is **synthetic, hand-authored geometry**. It is not downloaded from a real location. It gives the reference simulation a deterministic, offline fixture and avoids making CI depend on a public map server.

Both executables support an existing small OSM/Overpass JSON export:

```bash
cargo run --locked -p osb-client -- --map path/to/export.json
cargo run --locked -p osb-headless -- --map path/to/export.json --duration 8m
```

The observer also has an import control before Play. These paths do not issue network requests. Standard GeoJSON, OSM XML and PBF are not supported by this importer.

Supported JSON has an `elements` array containing simple `way` objects with either inline `geometry` coordinates or references to included `node` objects. Closed `building` ways become opaque, non-walkable polygon footprints. `highway` ways become displayed road polylines. Incomplete building ways are skipped rather than turned into arbitrary walls.

Current limits include a 32 MiB input cap, an element cap and bounded local geometry/navigation. Import a small area, not a country or planet extract. The importer reports warnings and preserves a source attribution string.

## What this does not model

No building interiors, windows, entrances, floor/roof heights, terrain elevation, vegetation, walls, rivers, bridge-layer connectivity or multipolygon relations are imported yet. A displayed road is not proof of a complete routable transport network. Buildings currently behave as opaque 2D obstacles, not occupied 3D structures.

Missing data means **unknown**, not verified open terrain. Do not label a generated battle physically realistic just because its street shapes came from a real map.

## Two independent pipelines

The planned visual pipeline may use MapLibre Native and an OSM-derived vector tile source such as OpenFreeMap. It is **not integrated into this commit's executable**. Vulkan texture interoperation, camera synchronization and offline caching need an isolated prototype before committing the simulation to that backend.

The simulation pipeline must separately compile authoritative, validated geometry. Display pixels and provider-dependent zoom simplifications cannot define where people can move or see. Source provenance, extraction bounds, version/checksum and warnings should accompany imported/cacheable geometry, and saves must preserve the world version used by their campaign.

Map refresh must not silently change an ongoing battle's obstacles or road connectivity. Any world update needs an explicit migration/rebuild boundary.

## Distribution and hosting

Retain source attribution in the visible map UI and exported data. Do not treat community map infrastructure as an unlimited game CDN or implement bulk downloads against it by default. Provider terms, attribution and data-license requirements must be checked for the actual selected dataset/service before shipping downloads, caches or regional bundles.

No external basemap is required for the current offline prototype. No API key is embedded in the repository.
