# Native dependency verification

Bevy/bevy_ecs 0.19.1 and bevy_egui 0.42.0 are the foundation combination.
The UI integration source is pinned to upstream revision
cfc6f33d47ab21210abe42d3f7c3850c9162de82 under vendor/bevy_egui.

The crates.io 0.42.0 package failed against Bevy 0.19.1 because its renderer
referenced an unavailable PipelineCacheError::ImmediateSize variant. The
selected upstream revision does not have that match arm.

Native CI then detected a second integration issue: upstream bevy_egui
enabled bevy_winit's default features, which include X11. The vendored copy
applies exactly one manifest change: default-features = false on that
dependency. The application explicitly enables Wayland. Rust and shader
source remain unchanged, the MIT license is retained, and source hashes
are recorded in vendor/bevy_egui/OSB_FILES.sha256.json.

Keep native clipboard and URL-opening features disabled unless separately
validated. They must not enable an unwanted X11 integration. The normal CI
checks the feature graph and actually launches under Wayland/software Vulkan.
This is not a substitute for hardware-driver and KDE desktop testing.

No standalone font assets are copied. Application source licensing remains
for the owner to decide; the vendored dependency retains its own license.
