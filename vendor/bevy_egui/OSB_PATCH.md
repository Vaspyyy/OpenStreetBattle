# OpenStreetBattle dependency patch

Upstream: https://github.com/vladbat00/bevy_egui
Exact revision: cfc6f33d47ab21210abe42d3f7c3850c9162de82 (v0.42.0).
License: MIT; the upstream LICENSE is retained.

The only source change is in Cargo.toml: the bevy_winit dependency now has
default-features = false. Its upstream default enables X11, which violates
this application's native Wayland-only requirement. The application enables
Wayland explicitly through Bevy. No Rust or shader behavior was changed.

Included: manifest, license, README/changelog, rustfmt config, source and
example source. Upstream CI, image/demo assets and repository metadata are
not copied. No standalone font assets are copied. Examples are retained for
manifest completeness, not as an application feature.

OSB_FILES.sha256.json records included file hashes after the manifest patch.
Recheck the original revision and this one-line difference when updating.
This package is excluded from the application workspace. Run application
tests and the native no-X11/Wayland/Vulkan smoke check after any upgrade.
