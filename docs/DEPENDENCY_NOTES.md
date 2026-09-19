# Native dependency verification

The foundation uses Bevy/bevy_ecs 0.19.1 and the upstream bevy_egui v0.42.0 source pinned to commit `cfc6f33d47ab21210abe42d3f7c3850c9162de82`.

The first native CI build with the crates.io `bevy_egui = 0.42.0` package failed in its renderer with an unavailable `PipelineCacheError::ImmediateSize` variant. The selected upstream tag source does not contain that match arm. Pinning the exact source revision avoids a floating branch and makes the difference explicit. This change still requires the native CI build and runtime smoke test to pass; a version compatibility table alone is not sufficient evidence.

Upstream source: https://github.com/vladbat00/bevy_egui/tree/cfc6f33d47ab21210abe42d3f7c3850c9162de82

No local upstream source patches or copied font assets are used. Native clipboard and URL opening features remain disabled so they cannot accidentally enable X11 integrations. Keep the headless dependency graph independent from these graphics dependencies.
