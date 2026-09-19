# Foundation validation record

This is evidence for the 0.1 foundation, not certification of realism, campaign scale or every desktop configuration.

## Native runtime

The audited UI dependency patch was compiled, linted and launched in workflow run
[35469606514](https://github.com/Vaspyyy/OpenStreetBattle/actions/runs/35469606514).
That workflow generated source commit `f3482478d65bf8791dcc92a5b728c133cc1063aa`, then launched the generated build.

Environment: Ubuntu 24.04, headless Weston, Mesa software Vulkan. The feature graph rejected X11 activation. The runtime selected Vulkan, rendered the actual observer window, saved a 1440 x 900 screenshot and exited successfully after 100 frames with the simulation at tick 266 and all 32 persistent people present.

The screenshot was inspected: map, people, controls, person inspector and event log were visible. This is a runtime/rendering smoke test, not automated clicking through every UI control. The headless compositor reported a non-fatal XDG settings portal timeout. Actual KDE Plasma, hardware Vulkan drivers, fractional scaling and multi-monitor behavior still need desktop playtesting.

Artifact: `native-ui-patch-validation`, containing `client.png`, `client.log`, `weston.log` and `client-features.txt`.

## Core and combat

Run [35469374233](https://github.com/Vaspyyy/OpenStreetBattle/actions/runs/35469374233), core job, passed 56 tests, strict core Clippy, formatting, headless dependency checks and a headless combat/checkpoint smoke test. The later native patch workflow also passed the core suite and strict full-workspace Clippy.

The bundled First Contact scenario, seed 42, ran for 480 simulated seconds. It produced 2,055 shot events, 107 injury events, 17 treatment events, 11 deaths, 10 leadership-change events and two group-joining events. Injury events are not a count of distinct wounded people. These are observed results of the provisional model, not expected real-world casualty rates.

First gunfire occurred at tick 2896, about 4 minutes 50 seconds into the approach. The default scenario does not start with both forces already in firing positions. Use 5x or 10x to reach contact sooner.

At tick 4800, the checkpoint still contained all 32 unique soldier identities, including deceased people. The status categories accounted for the entire roster. The headless executable loaded the checkpoint and advanced another 100 ticks to 4900. Separate tests compare exact state after uninterrupted and save/resumed runs, including active combat.

Artifact: `headless-validation`, containing `headless.json`, `resumed.json`, `events.jsonl` and `smoke.osb`.

## Current limitations and follow-up

The UI is a prototype. Overlapping objectives can have overlapping labels, and map-label placement needs a proper collision/priority layout pass. The native runtime check establishes launch and drawing, not exhaustive interaction correctness.

The visible default map is synthetic. OSM JSON import is limited to the documented supported ways. No live MapLibre basemap, operational campaign, simulation LOD, full terrain pipeline, vehicle logistics or long-term injury recovery is implemented.

Do not use the small 32-person smoke run or no-contact storage probes to advertise tens-of-thousands active combat performance. Run current CI for the latest source rather than treating this historical validation record as proof for later changes.
