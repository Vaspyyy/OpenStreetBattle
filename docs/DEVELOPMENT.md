# Development and validation

## Toolchain

Use rustup with the repository's `rust-toolchain.toml`; do not change the user's global default toolchain. Rust 1.95.0 and Rust 2024 are pinned for this foundation. Commit Cargo.lock for reproducible application builds.

Linux client dependencies: a native compiler/linker, pkg-config, Wayland and libxkbcommon development packages, and a working Vulkan loader/driver. A native Wayland session is mandatory for the graphical executable. No X11 fallback is compiled. KDE Plasma is the priority desktop, but the core does not depend on KDE.

On Ubuntu, the native development packages used by CI are `libwayland-dev` and `libxkbcommon-dev`. CI additionally installs Weston and Mesa Vulkan for the graphical smoke test. Keep the desktop's existing working GPU driver rather than replacing it as a routine build step.

## Commands

Run from the repository root:

```bash
cargo run --locked -p osb-client
cargo run --locked -p osb-headless -- --duration 8m --seed 42
cargo run --locked -p osb-headless -- scenarios/first_contact.json --duration 8m
cargo run --locked -p osb-headless -- --duration 8m --save saves/demo.osb --events events.jsonl
cargo run --locked -p osb-headless -- --load saves/demo.osb --duration 1m
```

The client starts paused. Scenario editing is available before the first tick. Reset begins a new run, not a tactical command to existing soldiers. F5/F9 save and restore the manual checkpoint. Treat simulation speed as best-effort; inspect tick timing rather than assuming 10x is sustainable.

A release build can improve runtime performance:

```bash
cargo run --release --locked -p osb-client
cargo run --release --locked -p osb-headless -- --benchmark
```

The benchmark is a dispersed/no-contact storage/update probe. It is not evidence for 10,000-person active combat or years of campaign operation. Report build profile, machine, workload, simulation duration, warm-up and memory before comparing measurements.

## Automated checks

```bash
cargo fmt --all -- --check
cargo test --locked --workspace --exclude osb-client
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked -p osb-client
python3 tools/check_boundaries.py
```

The normal CI workflow is read-only. It runs core tests, dependency-boundary checks, headless execution with save/load and event output, native compilation, an X11 feature check and a real client launch under headless Weston using software Vulkan. `tools/wayland_smoke.sh` captures a screenshot and logs in `artifacts/`.

A successful graphical CI run demonstrates launch/rendering on the tested software Vulkan stack. It does not establish NVIDIA driver, fractional scaling, multi-monitor, suspend/resume or interactive KDE correctness. Those require actual desktop playtesting.

Existing regression areas include seeded repeatability, exact resume during combat, stable identity/people counts, missing/invalid save rejection, checkpoint integrity, LOS through buildings, navigation around obstacles, information sharing without wall penetration, ammunition/medical resource conservation, leader succession and injury consequences. The bundled-demo regression ensures the example actually reaches gunfire.

## Deterministic changes

Preserve update ordering and explicit RNG state. Do not read wall time, input state, camera position or network responses inside `osb-sim`. Update golden expectations only after explaining the behavioral change, not to hide a regression. Cross-platform bitwise identity is not guaranteed merely because one CI platform is deterministic.

## Save and input care

Checkpoints are versioned compressed SQLite snapshots, not a serialization of the renderer. Loading must validate before replacing live state. Never silently reinterpret an incompatible save. CLI input sizes and map bounds are restricted; retain those limits when adding import formats.

No source-code license has been selected. Do not insert a license or redistribute third-party map datasets on the owner's behalf as part of an unrelated code change.
