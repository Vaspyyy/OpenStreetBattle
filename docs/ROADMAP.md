# Roadmap and release gates

Versions below are agreed feature targets, not dates or claims of completion. The repository is currently the 0.1 foundation.

## 0.1 foundation

The initial slice establishes a native observer, headless reference runner, persistent people, deterministic ticks, polygon LOS/navigation, contact sharing, basic autonomous behavior/combat, inspectable health/resources/relationships and transactional checkpointing.

Acceptance checks include a build of both executables, strict workspace lint/format checks, save/resume equivalence, conservation tests, a bundled scenario that actually reaches gunfire, and a real Wayland/Vulkan smoke run with screenshot capture. An offline geometry map is an explicit temporary backend.

## Before 1.0

The next work must turn a coherent prototype into a useful battle sandbox:

- Implement and validate the real geographic data pipeline and replaceable visual basemap. Test projection, missing data, disconnected geometry and cache/provenance handling.
- Improve commander and squad autonomy beyond the current heuristic slice. Show why an agent moves, waits, seeks cover, assists, regroups or surrenders.
- Complete the intended tactical resource, trauma and relationship-sensitive assistance systems, with behavior/conservation tests and a declared abstraction level.
- Make scenario construction and observation comfortable: clear validation errors, useful presets, readable scale-dependent display and inspectable events.
- Profile representative battles, not just idle storage. Set a measured supported battle-size target on named hardware before advertising capacity.

## 1.0: Battle Sandbox

A contained spectator battle, not an RTS. Individual people have stable identities, injuries, morale, relationships, belongings and changing group membership. Player-created intent goes through autonomous code-driven command from the first release.

Perception: local geometric LOS plus information sharing. Logistics: finite tactical ammunition and medical supplies with appropriate sharing/use. Injuries: coarse trauma, bleeding, functional impairment and stabilization, not a health bar. Social assistance responds to relationships/personality/role. Generic fictional contemporary forces are the reference equipment set.

A simple field or interface does not mean the corresponding 1.0 behavior is complete.

## 2.0: Facing and imperfect information

Add facing/FOV, stale and incomplete reports, and potentially incorrect intelligence. Retain observation time and provenance. Command must reason with its own information and uncertainty, not hidden global positions.

## 2.5: Eras

Official modern, Cold War and WW2 equipment/organization content. Keep the data format extensible, but do not pretend genuinely different mechanics, communications and organization can always be represented by changing weapon numbers.

## 3.0: Deep Battlefield

Attention, detection/recognition and richer perception. Full battlefield logistics: physical stocks, transport, routes and resource transfer. Casualty collection, transport and evacuation become an actual chain rather than a healing abstraction. Vehicles and other supporting systems should arrive as validated dependencies of this work, not unchecked fields.

## 3.5: Persistent Campaign

Concurrent battles and operational movement in a world that persists over simulated weeks, months and potentially years. Tens of thousands of named/identified people may be active in the campaign, even though only a relevant subset receives detailed tactical updates.

The key scenario is a large encircled force surviving with finite supplies, no automatic reinforcements and the possibility of eventual relief, surrender or destruction. Time continues elsewhere. Closing the view, changing sectors or saving the game must not reset the war.

Release gates include resource conservation across supply routes and LOD, persistent injuries and relationships, simultaneous-battle consistency, bounded runtime and memory/event growth, long-duration soak tests, validated abstraction error and resilient versioned saves. Idle-agent benchmarks do not meet this gate.
