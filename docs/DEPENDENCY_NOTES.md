# Native dependency verification

## Bevy and egui

Bevy/bevy_ecs 0.19.1 and bevy_egui 0.42.0 are the foundation combination. The UI integration source is pinned to upstream revision `cfc6f33d47ab21210abe42d3f7c3850c9162de82` under vendor/bevy_egui.

The crates.io 0.42.0 package failed against Bevy 0.19.1 because its renderer referenced an unavailable PipelineCacheError::ImmediateSize variant. The selected upstream revision does not have that match arm.

Native CI also detected upstream bevy_egui enabling bevy_winit's X11 default. The vendored copy applies exactly one manifest change: default-features = false on that dependency. The application explicitly enables Wayland. Rust/shader source remains unchanged, the MIT license is retained, and source hashes are recorded in vendor/bevy_egui/OSB_FILES.sha256.json.

Keep native clipboard and URL-opening features disabled unless separately validated. They must not re-enable X11. CI checks the feature graph and launches under Wayland/software Vulkan; this does not replace KDE/hardware-driver testing.

## Optional native basemap

`native/osb-map-native` and the client's `live-map` feature use MapLibre Native FFI pinned to `6f7998eec595560c0359ed033519cbab1f7c9aeb`. The Rust binding and C artifact must match exactly.

`tools/setup-maplibre.sh` downloads release asset `574254252`, verifies SHA256 `a2467bd331b8432614c5424160af47b34451d05fc7735d1be8eb4572dbc0fb19`, locates its install prefix and verifies source revision, Vulkan backend, Linux x86_64 platform and static archive presence. It does not follow an unpinned snapshot update. A changed/unavailable artifact is an explicit installation failure.

MapLibre uses its own Vulkan device and owner thread. Bounded RGBA readback/upload avoids unvalidated borrowed-image synchronization with Bevy. Worker Drop joins owner-thread teardown rather than detaching it across application exit. Unsafe Vulkan handle setup/destruction is isolated in the native wrapper; the core remains GPU- and network-independent.

The live feature requires libclang and Vulkan development files in addition to the original native dependencies. The default offline client does not require the MapLibre prefix. See REAL_WORLD.md for launch and test commands.

No standalone font assets are copied or bundled in this repository. Application source licensing remains for the owner to decide; third-party dependencies retain their own licenses.
