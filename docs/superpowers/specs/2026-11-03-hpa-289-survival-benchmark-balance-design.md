# HPA-289 — Survival benchmark and first balance pass: design specification

Status: **Revised after PR #17 design review; planning only — implementation and measured evidence pending** · 2026-11-03  
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

## 1. Shared completed-building constructor (avoids inert towers)

A completed building authored with only `Building`, `ConstructionState`, `Footprint` and `Health` does **not** necessarily behave as a completed in-game building: `step_construction` grants `Dropoff` for Depots, `AttackCooldown` for combat buildings, and `ProductionQueue` for producers. `combat.rs` skips towers without `AttackCooldown`.

- Extract those completion-only component insertions into `grant_completion(world, entity, kind, team)` in `buildings.rs` and call it from the existing `step_construction` path (unchanged gameplay).
- Add one `pub fn spawn_completed_building(world, map, team, kind, anchor) -> BuildingId` in `buildings.rs`, **for authored fixtures/tests**, not as a gameplay command. Allocate using `IdAllocator`, read `building_spec` for dimensions/HP, assert all cells legal and unoccupied, use `set_gate` for Gate and `set_blocked` for other buildings, register `BuildingIndex` and invoke the same grant helper. No generic spawn framework.
- Use the shared helper from **both** the benchmark fixture and the integration balance guard. Converting the older unit-test fixture copies is not part of HPA-289. Assert authored combat buildings have `AttackCooldown` **and actually fire**.

## 2. Existing 200-unit benchmark, now with a fortified city

`fixture.rs::extend_with_benchmark_workload` already seeds **50 human units and 150 actual Team-2 aliens** through `step_waves`. Retain this, the 18 resources, real fog/20 Hz combat, and the synthetic `BENCHMARK_WAVE = 100` **for 150-live-alien cap stress only**. The inflated overflow HP at wave 100 is not representative of normal wave-20-to-25 balance.

Extend only the benchmark fixture (not `seed_skirmish` or normal match economy):

- Add **40 completed combat towers** in a fixed variety mix: **24 Gun Turrets, 10 Flame Towers, 6 Crystal Lances**; a closed Wall/Gate perimeter around the central Research Center; and **nine Coal Furnaces**. Give this benchmark-only economy at least **400 Coal**, which fuels nine Furnaces for more than 60 *simulated* seconds with catalogue burn rates. Place all authored coordinates/fuel in `fixture.rs`; never change normal-match starting resources to support a stress fixture.
- Place the fortress from the real `MapFixture` map after checking the central Research Center, four Workers, all 18 resources, `RUINS`, alien zones and walkable approach lanes. No blocked/overlapping footprints or inaccessible Workers. Stage 23 Guards and 23 Riflemen defensively rather than marching them north; keep four real accepted Gather orders. No periodic replay orders and no timed obstacle toggles.
- **Do not insert benchmark-only research points.** `building_unlocked` controls player placement, not whether a directly authored completed Flame Tower/Crystal Lance fires.
- Assert the **exact tower mix and Wall/Gate/Furnace count once in Rust**. Godot checks only the authoritative expected *total* building-view count and the Research Center identity, and prints the per-kind counts; CI checks the summary sentinel, not another set of per-kind literals.

Extend the **existing** `fixture/tests.rs::benchmark_workload_keeps_real_activity_for_the_sixty_second_sample` 1,200-tick test; no new duplicated 150-alien replan test:

- Verify all 40 tower entities have `AttackCooldown` and `CombatEvent.attacker == CombatTarget::Building(...)` occurs, including at least one tower shot during the **last simulation-time window**.
- After initial `step_heat` and again at simulated t=60, assert `surface_efficiency(world) == 1.0`. Keep enough benchmark-only Coal and surviving heat sources to avoid a nearly idle cold benchmark.
- Verify a Wall or Gate is destroyed during that 60-**simulated**-second run and `map.revision()` increments with the breach. Pin a **meaningful remaining-live-alien floor** at t=60 from the first measured deterministic trajectory, not just a nonzero check; do not invent the threshold before observing actual behavior.
- If geometry does not simultaneously permit late tower activity, a breach, substantial surviving aliens and full heat, revise **benchmark-only geometry/fuel with before/after test evidence**, not live gameplay values to force a synthetic result.

These use existing wall/revision tests in `waves/tests.rs`: `preserved_core_route_falls_back_after_a_wall_closes_its_only_path_and_recovers_after_breach`, `sealed_150_aliens_suppress_static_fallback_probes_until_the_map_changes`, and `siege_path_calls_are_measured_for_150_aliens_on_a_wall_ring_revision_bump`. Do not add another copy in `tests/occupancy_replan.rs`.

## 3. Keep the Godot runner and CI simple

Reuse `godot/scenes/benchmark_200.tscn`, `benchmark_200.gd`, and the existing `GRUS_BENCHMARK_200 start_units=200` summary (the census still is 200). Change the old one-building readiness condition to the full fortress building-total check and unique Research Center identity; append observed per-kind building counts, heat and remaining live/visible counts to the marker, while retaining 1920×1080, fog readiness, time-weighted frame metrics and liveness. Don't duplicate Rust's 24/10/6 mix check in GDScript.

`GRUS_BENCHMARK_SECONDS` stays positive finite, **10 real seconds in software-GL CI** and **60 real seconds on a hardware-rendered Linux x86_64 release-export reference host**. Update only the CI step label; preserve its exit-0 and summary-marker gate, packaging scene path, and **no FPS floor**. No new benchmark harness, simulation speed hacks, debug toggle, respawn/heal or periodic commands.

The pure Rust test's **60 simulated seconds** do not prove that a slow renderer progressed 60 simulated seconds during its 60 **wall-clock** seconds. Record actual rendered-workload trajectory and say explicitly if no breach occurred in the measured real-time run.

## 4. Fixed-defense balance guard: defined setup, release-only CI

Add **one** `crates/grus-sim/tests/survival_balance.rs` integration journey using `MapFixture::battlefield`, `seed_skirmish`, the shared completed-building constructor, a real `MatchSession` / `start_match`, normal `VisibilityMap`, `step_tick` and `SIM_STEP_SECONDS` (20 Hz). No player commands after setup, artificial stats, Labs, Analyze or benchmark wave shortcut.

**Proposed reviewable reference defense**, separate from the 40-tower stress scenario:

- Origin is `MapFixture::team_starts()[0].research_center_anchor` (currently (62,46)). Place a closed 1×1 Wall ring on the **inclusive bounds x = anchor.x−4 .. anchor.x+7, y = anchor.y−3 .. anchor.y+7** (currently x58..69, y43..53), using one **Gate at (anchor.x+1, anchor.y+7)** (currently (63,53)). This avoids the current four Worker cells, core, nearby six resources and static `RUINS`.
- Exactly **two 2×2 Gun Turrets** inside at **(anchor.x−3, anchor.y+1)** and **(anchor.x+5, anchor.y+1)** (currently (59,47) and (67,47)). Leave the four starting Workers idle.
- **No Furnaces or test-only Coal.** Research Center supplies 15 heat, demands 10, and two turrets demand 2 each: full efficiency without refueling (Walls/Gate demand zero). Author this fixed setup once and prove it is legal. If baseline gameplay contradicts the intended wave-10-to-40 envelope, document a single explicit pre-tuning reference-fixture correction before freezing it; never weaken the guard silently to make a catalogue change pass.

The wave timetable is `300 + 100×(wave−1)` simulated seconds. At the **wave-11 deadline**, the Research Center must still live and the session remain Playing (survived all wave-10 time); a real Defeat must occur **before wave 40 launches**. Fail on unexpected Victory or stalled progress. Stop immediately on Defeat; don't jump simulation time with giant deltas.

This journey can run up to **84,000 fixed ticks** (wave-40 deadline = 4,200 simulated seconds). Do **not** run it in instrumented debug coverage: annotate `#[ignore]` and add a dedicated CI step:

`cargo test -p grus-sim --release --locked --test survival_balance -- --ignored`

Keep real fog in the release journey, log actual failure wave and measured test runtime, and leave the normal 90% coverage job for the fast tests. Also add a near-zero-cost pure catalogue test:

`f64::from(RESEARCH_TARGET_POINTS) / RESEARCH_BASE_PER_SECOND > wave_deadline(40)`

Today that is **4,500 s > 4,200 s**; otherwise an unattended defense could win by passive research before wave 40 and invalidate the guard. The wave guard checks survival pressure, **not** player victory time or engine frame rate.

## 5. Research arithmetic before catalogue tuning

At full surface efficiency the existing `RESEARCH_TARGET_POINTS` is **9,000**, `RESEARCH_BASE_PER_SECOND` is **2 points/s**, a completed Lab adds **1 point/s**, and each accepted Analyze converts **10 Crystals to 90 points**.

| Idealized case at full heat (no Analyze) | Points/s | Earliest theoretical time |
| --- | ---: | ---: |
| Zero Labs | 2 | 4,500 s / 75 min |
| One Lab active from start | 3 | 3,000 s / 50 min |
| Two Labs active from start | 4 | 2,250 s / 37.5 min |
| Two Labs active after 180 simulated seconds | 2 then 4 | `2,340 s / 39 min |

The **38–45 minute** goal is 2,280–2,700 s, corresponding to an *average* **3.33–3.95 points/s** including Analyze. The table is **arithmetic, not a feasibility claim** that two Labs can be completed at 180 s; real digging/resource cost, cold, crystal drops and Analyze timing decide that. Record an actual progression baseline before tuning.

Prioritize `catalog.rs` research/Lab/Analyze access and the player's path to victory; use tower, wave and economy constants for survival pressure. Make only small evidence-supported numerical changes in `catalog.rs` (tests may update expected values). Log each old value → new value, why, and observed effect. No algorithm rewrites or extra mechanics. A fixed-defense test does not replace an interactive playtest.

## 6. Acceptance and merge decision

- Engineering gates: format, Clippy, workspace tests, **>=90% grus-sim line coverage**, explicit **release-mode ignored** balance guard, 60-simulated-second fortified benchmark test, existing Godot smokes, Linux export/packaged E2E and **10-second llvmpipe CI fortress marker with exit 0**.
- Update `README.md` and `docs/hpa-289-survival-balance-evidence.md` with exact revision/host/renderer, actual benchmark timing (mean, p50/p95/p99/max/FPS), live/visible/tower census, heat and breach evidence, fixed-defense failure wave, catalogue before/after and **at least one recorded normal-match owner playtest** (win/defeat time, wave, resource/heat/underground/Analyze bottlenecks and difficulty). Historic one-core timings are **not comparable** with the new fortified workload.
- **Merge this single implementation PR** after the engineering gates are green and **one real owner playtest has been recorded**, with any unresolved balance notes documented. Later playtest iterations belong in **separate follow-up tickets/PRs**, not an indefinite extension of HPA-289.
- The hardware-rendered Linux x86_64 reference host is **not available**. Mark 60-second packaged-release hardware acceptance **BLOCKED / not run** in README/evidence; do **not** assert it passed, substitute macOS headless or llvmpipe measurements, or hold otherwise verified engineering indefinitely for this external dependency. If a qualified host becomes available before merge, record its actual result.
- Leave all unrun checks unchecked. Planning docs are not evidence of code implementation, CI, human playtesting or actual hardware performance.

## Out of scope

New benchmark framework, new systems/units/difficulty modes, test-only invulnerability or forced combat, periodic command replay, a duplicate 150-alien navigation test, new renderer or general architecture, changes to normal gameplay seed outside measured `catalog.rs` tuning, generated art/icons/SFX, multiplayer, saves, and backward-compatibility shims.
