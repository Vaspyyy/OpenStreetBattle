# OpenStreetBattle

A Linux-native, Wayland-only, Vulkan-first spectator warfare sandbox on geographic map data. The player creates forces and intent, then observes autonomous, code-driven people and commanders.

**Status: foundation development, not a 1.0 release.** The long-term goal is a persistent campaign containing tens of thousands of individually identified people over simulated years. The initial playable milestone is a bounded battle.

## Non-negotiable design rules

- Simulation state belongs to a GPU-independent Rust core, not the renderer.
- A soldier has a stable identity, relationships, injuries, knowledge, inventory and changing formation membership. Removing a visual marker never removes a person.
- Perception and communicated reports drive decisions. AI must not read hidden enemy state to plan or aim.
- Intent is interpreted by code-driven commanders. An optional language-model adapter may translate text into validated intent, but no API calls control simulation ticks.
- Camera position must not change simulation outcomes. Future simulation abstraction must preserve people and resources and be tested against detailed simulation.
- A fixed clock, explicit randomness, versioned saves and repeatable headless tests precede claims of campaign scale or realism.

## Agreed releases

| Release | Scope |
| --- | --- |
| 1.0 | Battle sandbox; individual people; local LOS and information sharing; autonomous command hierarchy and dynamic groups; tactical ammunition and medical supplies; coarse trauma model and relationship-sensitive casualty assistance; fictional contemporary equipment. |
| 2.0 | Facing/field of view; stale, incomplete and potentially wrong reports. |
| 2.5 | Official modern, Cold War and WW2 equipment/organization content, with engine changes where genuinely different mechanics require them. |
| 3.0 | Attention and richer perception; battlefield supply networks; casualty evacuation. |
| 3.5 | Persistent campaigns, concurrent battles, encirclements, reinforcements, weeks/years of simulated time, validated multi-resolution simulation and tens of thousands of persistent people. |

Wayland is the supported window system; KDE Plasma and NVIDIA/Vulkan are the priority test environment. KDE is not a runtime dependency. Map presentation and authoritative simulation geometry are separate systems.

The source-code license has not yet been selected by the owner. External geographic data retains its own attribution and licensing requirements.
