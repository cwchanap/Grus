# HPA-289 — Survival benchmark and first balance pass: design specification

Status: **Proposed for review (planning only; no implementation or benchmark results yet)** · 2026-11-03  
Issue: [HPA-289](https://linear.app/cwchanap/issue/HPA-289/grus-survival-88-wave-benchmark-and-first-balance-pass)  
Parent: [Winter Survival design contract, §9](2026-10-21-winter-survival-design.md)  
Prerequisite: [HPA-288 / merged PR #16](https://github.com/cwchanap/Grus/pull/16)

## Outcome and delivery

Complete the final Winter Survival engineering slice: a reproducible 1080p fortified-city load test, an automated deterministic **survive wave 10 / fall before wave 40** regression, and an evidence-led first tuning pass toward a **38–45 minute** victory (approximately waves 20–25). Match duration is a balance target, not an enforced timer.

**One ticket, one PR:** this planning draft becomes the same implementation PR, including code, tests, performance evidence, gameplay tuning, README and reviews. Artwork, icons and SFX stay in their existing separate Linear tasks. No additional architecture or features.

## Current main branch: what already exists

- `crates/grus-sim/src/fixture.rs::extend_with_benchmark_workload` already creates **50 human units and 150 actual aliens** from the shared `step_waves` director. It uses the synthetic `BENCHMARK_WAVE = 100` to guarantee the 150-alien cap. The benchmark therefore **does not still use** the old mixed 100-vs-100 PvP workload.
- `godot/scenes/benchmark_200.tscn` and `godot/scripts/benchmark_200.gd` already enforce the 1920×1080 viewport, 200 unique unit views, fog progress, frame-time percentiles, time-weighted live/visible counts and contested-window movement/combat activity. There are **no scripted periodic command replays**.
- The major missing workload dimension is the **city**: the benchmark currently expects exactly one building view, the Research Center. It has no completed walls, gates, towers or representative surface heat demand; 46 defenders are sent toward the northern field instead of defending a fortification.
- `.github/workflows/ci.yml` runs the existing scene with `GRUS_BENCHMARK_SECONDS=10`, non-e2e extension, software GL, exit-code and `GRUS_BENCHMARK_200` sentinel checks. README documents a 60-second hardware acceptance run, but an actual hardware-rendered Linux x86_64 reference host has **not been established**.
- `catalog.rs` is the single source of tunable gameplay parameters. `step_tick` owns the existing 20 Hz order (waves → underground → heat → combat → movement → economy → construction → production → research → visibility). Wave routing already uses map revisions, gates and reachable-building fallback.

Keep those working contracts and extend them. In particular, retain the 200-unit scene/marker names because they still accurately describe the unit census; no artificial backward-compatibility layer is needed.

## 1. One authored fortified-city benchmark fixture

Extend the existing **benchmark-only** fixture in `fixture.rs` after `seed_skirmish`, without modifying the player-facing initial match.

- Keep **50 humans** (4 Workers plus 23 Guards and 23 Riflemen) and **150 Team-2 aliens** created by the real `step_waves` path. Keep the synthetic wave index clearly labeled **load stress, not a wave-100 balance forecast**: overflow HP makes this deliberately harder than normal wave-20-to-25 play.
- Add a deterministic completed defense centered on the existing Research Center: **40 combat towers**, a closed wall perimeter with at least one **Gate**, and enough Coal Furnaces to avoid a cold-imposed near-idle load. Initial tower-mix proposal: **24 Gun Turrets / 10 Flame Towers / 6 Crystal Lances**. These are workload counts, not gameplay balance constants.
- For that proposed mix, tower demand is `24×2 + 10×3 + 6×4 = 102` heat, plus Research Center demand 10, for **112** total. **Nine** active Furnaces add `9×12` to the Research Center's 15 supply (123 total). Set **benchmark-only** Coal to at least **400** so all nine can run beyond 60 simulated seconds at `0.5 Coal/s` each. If the mix changes during fixture validation, recalculate furnaces and Coal from `building_spec` / `FURNACE_COAL_PER_SECOND`, not copied heat literals. Do **not** alter normal match starting stockpiles.
- Directly author complete benchmark buildings using the existing `Building`, `ConstructionState`, `BuildingIndex`, `Health`, `IdAllocator`, catalogue footprint and `GridMap` blocked/gate APIs (a small local fixture helper is sufficient). No new building spawn system, benchmark-only tower AI, gameplay command or second map. If needed, benchmark-only research may start at the 50% unlock threshold so the advanced tower mix is coherent; this is scenario setup, **not** a shortened player progression path.
- Use fixed, reviewable positions around the city, chosen after checking the actual `RUINS`, 18 resource cells, central Research Center, spawn cells and approach lanes. No overlapping footprints, unwalkable unit spawns, unreachable Gather targets, or stranded Team-1 defenders. Stage Guards/Riflemen around/within the fortification and give them accepted defensive orders; remove the current northbound attack-move distraction. Do not issue repeating timed orders in the measured loop.
- Walls and Gate must force aliens to interact with the existing reachable-structure fallback. Let real combat damage and building destruction update occupancy. Include a deterministic **150-alien wall/gate destruction → map revision → route replan** Rust scenario so this pressure is verified, rather than inventing a benchmark-only command to toggle obstacles during frame sampling.
- Keep the authored 18 resource nodes, actual workers/gathering, visibility, 20 Hz sim, current combat/cold mechanics and normal entity cleanup. All gameplay numeric tuning remains in `catalog.rs`; fixed benchmark counts and coordinates are fixture/test data.

**Proof:** Rust fixture assertions for the exact roster, 40 complete combat towers and prescribed mix, one core, wall/gate/furnace census, unique IDs, legal occupancy and paths, sufficient active heat/fuel and actual accepted orders. A replan test must observe an occupancy revision when a reachable gate or wall is destroyed with aliens active. Avoid a timing assertion on CI hardware.

## 2. Update the existing Godot benchmark contract in place

Update `benchmark_200.gd` to wait for the new **building** census (core + 40 towers + authored walls, gate and furnaces), while retaining the already working 200-unit/fog readiness and 1080p checks. Check the core by its identity, not by assuming it is the only building. Report the start tower/wall/gate census alongside the existing `GRUS_BENCHMARK_200 start_units=200` marker, and keep all existing frame-time / time-weighted live and visible unit fields.

- Sample for **real elapsed time**, `GRUS_BENCHMARK_SECONDS=10` in CI and **60** on a qualified hardware-rendered Linux x86_64 reference host. Keep valid-positive-finite duration validation and the current liveness rules (movement or combat while both sides still have units).
- The load is allowed to attrit naturally: no instant respawns, artificial invulnerability, forced freeze, scripted command replay, or fake 200-visible-units assertion. Report observed population trajectory and active towers so a benchmark that became empty is not misread as healthy throughput.
- Preserve software GL CI as a **contract check**, not an FPS target. Update the job display name and its sentinel/census checks; keep the existing Godot scene path and regression log. Recheck the packaged `GRUS_MAIN_SCENE` runbook.
- Record `map.revision` changes and gate/wall breach observations in the Rust deterministic stress case. If 60-second rendered runs show no breach, say so explicitly and adjust the authored load only with clear before/after evidence. Do not build a new profiling framework or expose a mutable Godot debug control.

## 3. Deterministic balance guard

Add **one integration journey test** (e.g. `crates/grus-sim/tests/survival_balance.rs`) that uses the actual `MapFixture`, `seed_skirmish`, `step_tick` and wave director at `SIM_STEP_SECONDS`. Create a **fixed, plausible reference defense** from ordinary catalogue buildings (some walls, a Gate, a small turret force and its heat/fuel), with **no player input** after setup, no invulnerability, no enemy spawn hacks, no experimental stat overrides and no underground Labs.

- Start a real **Playing** session and let waves 1 onward arrive on the normal 300s + 100s cadence. Run at the same 20 Hz steps as production. Do not simulate faster by making a giant `seconds` delta that skips combat/pathing ticks.
- At the **wave-11 deadline** (after wave 10's full interval), assert the Research Center is alive and the session has not ended in Defeat.
- Continue until natural **Defeat**, assert it happens **before wave 40 launches**, and fail if an unexpected Victory, stuck match, missing wave progress or early Defeat occurs. Stop as soon as the outcome is known; no arbitrary 40-wave runtime if it falls earlier.
- Keep this authored reference setup stable during balance iteration, so the guard cannot be made green simply by weakening the test fixture. The test checks only a useful pressure envelope; **it cannot prove a human can win in 38–45 minutes**. Measure test runtime on CI and keep it in the regular test/coverage lane.

## 4. Evidence-led first balance pass

Tune the existing catalogue values, not algorithms, to make a real player route plausible: early resource/Worker/Guard/Rifleman survival; Wall/Gate/Coal Furnace upkeep; staged Gun/Flame/Crystal tower effectiveness; Crystal/Analyze conversion; underground room yield and depth affordability; research gain, wave budget and alien durability.

- First record baseline simulated outcomes and actual player timings before changing numbers. Keep an annotated before/after table of **every changed `catalog.rs` field**, rationale and consequence. Preserve the existing wave/heat/research formulas and documented unlock semantics unless a measured result justifies tuning their catalogue coefficients.
- Ask the owner to play at least one normal run, noting waves reached, defeat or research victory time, bottlenecks, tower/heat/underground choices, and the felt difficulty of the first 10 waves and the wave-20/25 region. A deterministic reference defense is **not** a substitute for an interactive economy/tech/Analyze playtest.
- Prefer a small number of clearly motivated parameter edits. Any numerical gameplay change lives **only in `catalog.rs`**; tests and evidence may change their expected values, but modules/bridge/GDScript must not embed new tuning literals. No new units, game mechanics, difficulty modes, AI, automation, art or SFX.

## 5. Acceptance, evidence and limits

- Rust format, Clippy with warnings denied, workspace tests, **>=90% `grus-sim` line coverage**, benchmark fixture/replan tests and wave-10/40 guard pass.
- Existing Godot smoke scenes, packaged Linux export / `bevy-e2e` and a **10-second software-rendered CI benchmark** pass with the new fortress census and full end marker.
- Update `README.md` and a new `docs/hpa-289-survival-balance-evidence.md` with exact commands, commits, host/renderer, measured 10s / any actual 60s frame times (mean, p50, p95, p99, max, FPS), live/visible population, heat, tower census, replan findings, tuning and owner playtest notes. Preserve historical results but label the new workload **non-comparable** to the no-fortress benchmark.
- **External dependency:** the documented hardware-rendered Linux x86_64 reference host is currently unavailable. Do not assert 60-second reference-host acceptance until a real host exists and the packaged release benchmark has actually run; clearly mark it **blocked**, rather than substituting llvmpipe or macOS headless numbers. Do not fabricate a human playtest or mark checklist gates complete before results exist.

## Out of scope

New engine abstractions, a second benchmark framework, new performance telemetry service, artificial player AI, changes to the normal map/starting layout, art/icons/SFX generation, rewriting the Rust/Godot bridge, multiplayer, saves, backward-compatibility shims and speculative optimization not supported by a measured bottleneck.

## Design review questions

1. Is **24/10/6 towers with nine fueled Furnaces** a useful fixed, synthetic late-defense stress mix, or should the mix be simpler? The total stays around 40 towers.
2. Do the authored fortress and 150-alien destruction/replan tests exercise the real shared navigation, without repeatedly issuing benchmark-only orders?
3. Is the fixed-defense survival envelope and full 20 Hz journey a good lightweight balance guard while interactive win-time validation remains a separate manual evidence gate?
4. Is keeping `benchmark_200` scene/marker naming (still 200 units) preferable to file churn?
