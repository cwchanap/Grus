# HPA-474 Balance, Performance and Desktop MVP Delivery Design

## Status

Planning baseline for HPA-474. This design and its implementation plan live on the same branch and draft PR that will carry the implementation; there is no follow-up implementation PR for this ticket.

HPA-470 through HPA-473 already provide the retained battlefield, economy, combat/lifecycle, fog/minimap and economic AI. HPA-474 is therefore a measured tuning-and-delivery pass, not another gameplay slice. It must not add a second state owner, a new gameplay framework, another benchmark system, a release-service pipeline, or new content merely to make the final ticket look substantial.

## Outcome

Ship the existing one-map human-vs-AI skirmish as a usable Linux x86_64 desktop MVP with three kinds of evidence:

1. four directional playtests show both an early-pressure opening and an economy-first/age-up opening can credibly win from either authored starting side, with typical unforced matches trending toward 15–25 minutes;
2. a representative 200-unit mixed workload is measured for 60 seconds in a release export on the recorded desktop target, reporting mean FPS plus p95/p99 frame times and any repeatable stalls; and
3. a fresh-checkout release package launches outside the editor and completes the normal start → economy → scout → combat → result → restart journey without debug intervention.

The ticket may tune existing values and fix defects proven by those runs. It does not create new strategic systems, new units/buildings, another difficulty mode, an installer, signing/notarization, or a multi-platform release matrix.

## Current baseline we keep

The code already has the correct ownership boundaries:

- Bevy ECS is the only mutable gameplay model; Godot owns input, rendering, HUD and effects.
- The simulation remains a 20 Hz fixed step.
- `catalog.rs` is the single gameplay catalogue for costs, build/train times, health, damage, speed and gather rates.
- `ai.rs` owns the small 1 Hz scripted opponent. Its tuning values are deliberately ordinary constants (`TARGET_WORKERS`, `POPULATION_HEADROOM`, `DEFENSE_RADIUS`, `ATTACK_THRESHOLD`, and related policy thresholds), not a generic tuning/config layer.
- `fixture.rs` owns the handcrafted mirrored map, resource placement and authored AI slots/routes.
- `PlayerCommand` / `CommandResult` / `RejectReason` remain the only gameplay-order path.
- `benchmark_200.tscn` / `benchmark_200.gd` and the existing benchmark reset bridge remain the one performance harness.
- The existing Linux export preset remains the one supported delivery target.

The current 200-unit benchmark is intentionally insufficient for HPA-474: it is a visibility-free debug/editor fixture containing only 200 fast Villagers moving across the map. It provides a useful HPA-470 movement regression, but it does not exercise gathering, combat, fog updates, AI, normal unit mixes, or a release export. HPA-474 evolves this retained harness instead of adding a parallel benchmark.

The current Start overlay also has no concise in-game control reference. README documentation is good developer documentation, but a first-time player should not need the repository open to discover selection, contextual right-click, attack-move, stop, pause, camera and minimap controls.

## Design rule: evidence before tuning

Do not choose new balance numbers from the planning document.

The existing HPA-472/HPA-473 values are the baseline. Each balance change must point to an observed playtest problem such as:

- early pressure cannot convert worker damage into a meaningful economic advantage;
- economy-first is automatically superior or automatically loses;
- the expansion is reached too late to matter or is never worth contesting;
- Town Center time-to-kill makes ordinary armies unable to finish a won match; or
- a shared AI threshold causes one authored start to stall while the mirrored start works.

Allowed tuning locations are deliberately narrow:

- `crates/grus-sim/src/catalog.rs` — shared costs, timings, health/damage, speed and gather rates;
- `crates/grus-sim/src/ai.rs` — existing opponent policy thresholds only; and
- `crates/grus-sim/src/fixture.rs` — the existing one-map resource/route/slot layout only when the playtest evidence is spatial.

Do not add a balance database, difficulty profile, runtime config file, behavior tree, utility scorer, upgrade tree, siege unit, second victory condition, or hidden AI bonus.

### Four-run playtest matrix

The required directional evidence is one run for each pair:

| Start side | Opening |
| --- | --- |
| left / Team 1 authored start | early pressure |
| left / Team 1 authored start | economy-first / age-up |
| right / mirrored authored start | early pressure |
| right / mirrored authored start | economy-first / age-up |

Runtime remains human Team 1 vs AI Team 2; the mirrored-start runs use the existing authored symmetry/test seam rather than adding a user-facing side selector. A small test-only/manual launch hook may choose the mirrored start only if the current harness cannot already do so; it must not become a shipping menu option.

For every run record revision, start side, opening, duration, result, first expansion use, worker-raid outcome, whether ordinary military could finish the Town Center, and any tuning applied afterward. The four runs are directional evidence, not a competitive-balance claim.

If the last run causes a tuning change that materially affects an earlier matrix case, rerun only the affected case. Do not automatically turn four directional runs into a large balance-test matrix.

Automated tests should pin invariants, not emergent match duration. Existing counter-duel, worker-raid, full-match-both-sides, visibility and economy tests remain the regression surface; add a focused regression only when a tuning change creates a new invariant worth locking.

## Minimal usability pass

Add one static, compact controls/help label to the existing non-Playing session panel. It should cover only the commands a first-time player needs to start playing:

- left click / drag / Shift for selection;
- right click for contextual command;
- `A` then right click for attack-move;
- `S` stop;
- `Esc` pause/resume;
- mouse wheel + middle drag for camera; and
- minimap click to recenter.

No tutorial campaign, modal-help framework, tooltip system, key-rebinding screen, settings menu or generated art is required.

Additional HUD/control changes are allowed only when a required playtest demonstrates a concrete readability or interaction defect. Every such fix gets a focused smoke/regression assertion in the existing scene/test that already owns the flow.

## Representative 200-unit benchmark

Keep the existing benchmark scene and reset seam, but make the benchmark world look like the real game.

### World shape

The benchmark reset starts from the normal authored skirmish state so it keeps the two Town Centers, 18 finite resource sources, TeamEconomy, the real map and normal unit components. It then grows the roster to exactly 100 units per side:

- 20 Villagers;
- 30 Spearmen;
- 30 Archers; and
- 20 Cavalry.

The existing four starting Villagers per team count toward the 20; the benchmark helper adds 16 Villagers plus 80 military units per team at deterministic walkable staging cells. This produces 200 total live units without creating another map or alternate gameplay catalogue.

Benchmark-only seeding stays in the existing fixture/bridge test surface. It may spawn the extra units directly because the benchmark fixture is a load generator, but after setup the workload uses ordinary gameplay orders and the production fixed-step systems.

### Active workload

Before sampling:

- 12 Villagers per side receive normal Gather commands, two per authored safe source;
- military groups receive normal AttackMove orders toward the opposing side so movement, acquisition and combat are live;
- a fresh `VisibilityMap` is inserted and refreshed, so fog recomputation and presentation projection run normally;
- a real `AiController` remains active for Team 2, so its 1 Hz economy/scouting/defense policy runs against the same world; and
- the session is `Playing`.

The remaining Villagers are intentionally available to the AI/economy systems rather than every entity being forced into one synthetic order.

Because real combat can kill units, the report must record `start_units=200`, `min_live_units`, and `end_units`; it must not silently reset health or respawn units merely to claim that all 200 survived the 60-second sample. The target is a 200-unit starting workload with real combat, not an artificial immortal stress scene.

### Sampling contract

`benchmark_200.gd` becomes duration-based instead of the current 40-frame sample:

- short CI/default regression sample: 10 real seconds after warm-up;
- acceptance run: `GRUS_BENCHMARK_SECONDS=60`;
- report mean frame time/FPS, p50, p95, p99, max frame time, start/min/end live-unit counts, renderer/adapter/vendor and visibility revision progress;
- fail if the scene never reaches the expected 200-unit/2-building/18-resource starting shape or if the workload cannot start.

The CI software-rendered Xvfb run remains a regression signal only. It must not be compared to the 60 FPS hardware target.

## Release profiling and optimization gate

The acceptance measurement uses a **release** Rust GDExtension and a Godot `--export-release` build on the recorded Linux x86_64 desktop target. The evidence record captures:

- commit SHA;
- OS/kernel;
- CPU;
- GPU/adapter + renderer;
- Godot version;
- Rust release build;
- viewport (1920×1080);
- sample duration;
- mean FPS / mean frame time;
- p95 / p99 / max frame times; and
- start/min/end live units plus any visible command stall notes.

Target at least 60 FPS average, but do not hide tail latency behind the mean.

There is deliberately no pre-planned optimization. If the release sample misses the target or shows a repeatable stall, profile first and make the smallest change in the existing owner. Likely owners may include `visibility.rs`, the bridge visibility snapshot/cache, `fog_of_war.gd`, `minimap.gd`, movement/pathing or view synchronization, but the profile decides. Do not introduce flow fields, multithreaded presentation, a second renderer, ECS mirroring, a custom profiler subsystem or speculative data-structure rewrites.

If the target is met and no repeatable stall appears, record that result and make no performance code change.

## Linux release package

Keep one supported target: Linux x86_64.

Add one small repository script, `scripts/package_linux_x86_64.sh`, that performs the already-documented manual steps reproducibly:

1. `cargo build --release -p grus-godot --locked`;
2. stage `target/release/libgrus_godot.so` into `godot/bin/libgrus_godot.so`;
3. run the Godot headless import once so the extension is registered;
4. export `Linux x86_64` with `--export-release` into `dist/grus-linux-x86_64/`; and
5. create `dist/grus-linux-x86_64.tar.gz` from that complete exported directory.

The package is a portable directory/archive, not an installer. The whole export directory is archived so the executable, PCK and exported native-library/assets stay together regardless of how Godot lays them out.

No GitHub Release automation, store publishing, signing, notarization, update service or multi-OS package matrix is added.

A fresh-checkout verification must run this script, extract the archive elsewhere, launch the executable outside the editor, and confirm the normal gameplay journey plus pause/resume, result/restart and quit.

## Evidence record

Create one durable record at `docs/hpa-474-mvp-evidence.md` rather than scattering benchmark and playtest numbers across comments.

It contains:

- four-run playtest table and tuning log;
- final shared tuning values that changed from the HPA-473 baseline;
- 60-second release performance record;
- any profile-driven optimization and before/after numbers;
- fresh-checkout package command + archive path/hash;
- packaged gameplay journey result;
- full regression commands/results; and
- honest remaining limitations.

The PR description may summarize this file, but the repository record is the source of truth for HPA-474 evidence.

## Testing and CI

Keep the current three-job CI shape and the 90% `grus-sim` production-line coverage gate.

Expected changes are narrow:

- fixture/bridge tests lock the 100-per-side mixed benchmark composition, visibility and AI setup;
- `benchmark_200.gd` proves the runtime starting shape and emits the duration-based metrics;
- an existing Godot smoke asserts the compact controls help is visible before Start;
- any balance/usability/performance defect fixed by this ticket gets one focused regression in the existing owning module/scene;
- the existing bridge/reset/economy/combat/scouting smokes, export boot, Bevy E2E boot and Codecov gates stay green.

Do not enforce 60 FPS in GitHub Actions. The current Xvfb/llvmpipe runner is intentionally software-rendered and unsuitable as the product FPS gate.

## Out of scope

Multiplayer, persistence/save/load, more maps, factions, difficulties, campaign/tutorial content, extra units/buildings, siege, formations, terrain/elevation gameplay, replay, new victory conditions, balance framework/config service, behavior-tree/utility AI, pathfinding replacement, flow fields, multithreaded renderer/presentation, custom art/animation/SFX pipeline, installers, stores, signing/notarization, auto-update, cloud services and multi-OS release packaging remain outside the MVP.