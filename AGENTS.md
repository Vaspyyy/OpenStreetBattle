# Working on OpenStreetBattle

This is a native Rust/Linux/Wayland/Vulkan spectator simulation, not a web game. Read README.md, docs/ARCHITECTURE.md and docs/ROADMAP.md before broad changes.

- Keep osb-sim and the headless dependency graph free of rendering, windowing, camera and networking dependencies. Run tools/check_boundaries.py.
- Persistent SoldierId is not a Bevy Entity. Death, culling, saves and future LOD must preserve identities and resource accounting.
- Use explicit simulation ticks and serialized randomness. No wall-clock or camera-driven game outcomes. No per-agent LLM calls. Optional language interpretation ends at validated structured intent.
- Respect the release ladder. Fields/stubs are not completed features. The current map backend is offline geometry, not MapLibre. Do not claim campaign scale, real-world realism or GPU-driver coverage without measurements/tests.
- Preserve the pinned toolchain, dependency compatibility notes and Cargo.lock. Test upgrades together, including the native client.
- Use validated, bounded inputs and versioned save schemas. Never silently discard a user's campaign or replace a checkpoint before validation.
- Run cargo fmt --all -- --check, core tests, strict workspace Clippy and the native build. Graphical changes additionally require the Wayland/Vulkan smoke test or an explicit explanation of untested behavior.
- Keep normal CI read-only. Do not weaken assertions, remove tests or suppress broad lint groups simply to make a build green.
- Do not choose a source license, add credentials, bundle fonts or redistribute third-party map datasets without the appropriate authorization.
