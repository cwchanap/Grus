# HPA-289 — Survival benchmark and first balance pass: implementation plan

Status: **Revised after PR #17 design review; implementation and evidence pending** · 2026-11-03  
Issue: [HPA-289](https://linear.app/cwchanap/issue/HPA-289/grus-survival-88-wave-benchmark-and-first-balance-pass)  
Design: [HPA-289 design spec](../specs/2026-11-03-hpa-289-survival-benchmark-balance-design.md)  
Parent: [Winter Survival §9](../specs/2026-10-21-winter-survival-design.md)  
Baseline: `main` after merged [PR #16 / HPA-288](https://github.com/cwchanap/Grus/pull/16)

## Delivery contract

**One ticket = one PR.** This planning branch/PR must evolve into the complete HPA-289 implementation, not be followed by a second coding PR. Keep the diff limited to the shared completed-building helper, benchmark fixture, existing GDScript runner/CI, one release-only balance journey, existing catalogue, evidence and README. No generated art or SFX belongs here.

The current repository **already has** the real 50-human/150-alien director workload: do not redo the HPA-449 PvP conversion. The missing piece is a fortified city and a defensible tuning/evidence loop.

## Checkpoint 1 — Share real completed-building grants and authoring

**Touch:** `crates/grus-sim/src/buildings.rs` and targeted `buildings/tests.rs`. The approved design is in [the revised specification](../specs/2026-11-03-hpa-289-survival-benchmark-balance-design.md).

- [ ] Extract `step_construction` completion inserts to one small `grant_completion(world, entity, kind, team)`: Depot `Dropoff`, combat-building `AttackCooldown`, producer `ProductionQueue`. Reuse it in ordinary construction **without changing behavior**.
- [ ] Add `pub fn spawn_completed_building(world, map, team, kind, anchor) -> BuildingId` to `buildings.rs` for authored fixtures/tests: allocate ID, catalogue footprint + full HP, assert legal/unoccupied cells, choose `set_gate` vs `set_blocked`, insert `Building` and `BuildingIndex`, then call the **same grant function**. Do not expose it as a player command, bypass regular game placement, or refactor old hand-authored tests for aesthetics.
- [ ] Cover Gate team-specific passability, ordinary blocked footprints, unique IDs, grants and genuinely actionable tower cooldowns. A created tower lacking `AttackCooldown` must fail tests.

**Proof:** the existing construction completion tests still pass and a directly spawned completed Gun Turret has a ready cooldown and can fire. **This helper must land before the balance journey or fortress** to avoid duplicating an inert constructor.

## Checkpoint 2 — Build the fixed-defense balance guard first

**Touch:** add `crates/grus-sim/tests/survival_balance.rs`, `crates/grus-sim/src/catalog/tests.rs`, and one new explicit release test step in `.github/workflows/ci.yml`.

- [ ] Seed an ordinary `MapFixture` / `seed_skirmish` and explicit `MatchSession::Start` → `start_match`. Use the shared completed-building helper to author exactly the **fixed two-Gun-Turret, Wall/Gate reference defense** described in spec §4 (relative coordinates to `team_starts()[0].research_center_anchor`). Use the real `VisibilityMap`. Leave four Workers idle, with **no Furnaces, extra Coal, Labs or player input after setup**.
- [ ] Assert no footprint overlaps the core/Workers/safe resources or static terrain. Full surface efficiency must follow directly from 15 core heat supply versus 10 core + 4 turret demand; do not add a fuel economy to the fixed-defense guard.
- [ ] Run the real `step_tick` at `SIM_STEP_SECONDS` (20 Hz), check the Research Center and Playing session at the **wave-11 deadline** (wave 10 survived), then stop on natural Defeat **before wave 40 launches**. Fail on premature Defeat, unexpected Victory, missing wave progress or a stalemate; log real failure wave and simulation time. Do not advance with giant deltas.
- [ ] Mark only this long integration journey `#[ignore]` so normal `cargo llvm-cov` does not burn minutes in instrumented debug. Add a **separate CI step** after fast coverage in `unit-tests` running exactly: `cargo test -p grus-sim --release --locked --test survival_balance -- --ignored`. Keep actual fog; observe and report the wall-clock cost. There can be at most 84,000 fixed ticks until wave 40; do not silently skip the step.
- [ ] Add a cheap catalog regression in `catalog/tests.rs`: `f64::from(RESEARCH_TARGET_POINTS) / RESEARCH_BASE_PER_SECOND > wave_deadline(40)`. It guards the 4,500 s unattended Victory versus 4,200 s wave-40 deadline **before** the long integration test. No second long test is needed.
- [ ] Record the **pre-tune Defeat wave** as baseline. If the fixed reference layout itself is unsuitable, document **one** explicit baseline fixture correction before adjusting catalogue balance; afterward do not weaken the guard to chase a green run.

**Proof:** a repeatable real 20 Hz survival envelope and an actual release-mode CI exit, not a fabricated runtime/Defeat value.

## Checkpoint 3 — Fortify the existing 200-unit workload and pin its shape

**Touch:** `crates/grus-sim/src/fixture.rs` and `fixture/tests.rs`. **Do not** add a duplicate `occupancy_replan.rs` or `waves/tests.rs` scenario: the 150-alien revision / fallback tests already exist.

- [ ] Extend **benchmark-only** `extend_with_benchmark_workload` with exactly **40 combat towers** (Rust-authored 24 Gun/10 Flame/6 Crystal), a closed Wall/Gate perimeter and **nine fueled Furnaces** (benchmark-only Coal >=400). Use the **shared helper**. Keep 4 Workers/23 Guards/23 Riflemen, real wave-100-capped 150 aliens, 18 resources and the single central Research Center. No extra research override.
- [ ] Choose/validate positions against `MapFixture`'s actual `RUINS`, 18 resources, worker/core cells and incoming lanes. Put defenders inside/near a plausible fortified city, issue accepted defensive orders and retain Workers' accepted Gather orders; no replay bursts, respawn, artificial combat or alternative runtime seed.
- [ ] Assert tower-kind/wall/gate/furnace **exact counts once in Rust**, unique IDs, `AttackCooldown` on combat towers, legal footprints/spawn cells, valid orders, sufficient fuel and first-tick full heat efficiency. Remove older fixture tests' `only one building` expectation.
- [ ] Extend `benchmark_workload_keeps_real_activity_for_the_sixty_second_sample` (already 1,200 fixed ticks): detect `CombatEvent.attacker` as a **building**, assert a tower shot in its final simulated-time window, check `surface_efficiency == 1.0` at t=0/t=60, Wall/Gate count decreases with a corresponding map revision, and a meaningful live-alien floor at t=60. Select that floor **after** the first deterministic test run and record the observed count; don't invent a value before measuring.
- [ ] If the initial authored fortress fails its own pressure/late-tower/breach constraints, adjust *benchmark fixture geometry or fuel* with evidence; do not modify production balance numbers to force a synthetic benchmark pass. The pre-existing 150-alien tests for wall closures, fallback suppression and ring-revision path counts remain in place.

**Proof:** one nonduplicated simulation stress journey with real towers, combat activity, heat, breaches/replanning pressure and surviving aliens, plus all fixture contracts.

## Checkpoint 4 — Adapt only the existing Godot benchmark runner/CI

**Touch:** `godot/scripts/benchmark_200.gd` and the existing benchmark step in `.github/workflows/ci.yml`. Retain `godot/scenes/benchmark_200.tscn` unless its wiring actually needs adjustment.

- [ ] Replace `building_views.size() == 1` with the **total** expected fortress building-view census, while still pinning the sole Research Center's identity. Keep exact 200 unit IDs and 50/150 team counts, 1920×1080 viewport and existing fog/liveness readiness.
- [ ] Print observed per-kind counts and heat/fuel alongside the existing `GRUS_BENCHMARK_200 start_units=200` end marker; **do not** hardcode the 24/10/6 mix again in Godot. Keep existing frame-time p50/p95/p99/max, time-weighted live/visible stats and no periodic orders.
- [ ] Rename the CI benchmark display step to reflect the walled-city workload. Retain `GRUS_BENCHMARK_SECONDS=10`, exit code **and** marker check, software GL and log artifacts. **CI checks presence of the marker, not a second copy of tower counts**; no FPS floor.
- [ ] Re-run the real runner where supported and keep exact stdout/host information. Recheck packaged `GRUS_MAIN_SCENE=res://scenes/benchmark_200.tscn` instructions. Explain that 60 **wall-clock** rendered seconds do not necessarily equal 60 **simulated** seconds on a slow renderer.

**Proof:** CI benchmark and new fortress census pass with real observed metrics; all pre-existing Godot smoke, packaged Linux E2E and export paths remain intact.

## Checkpoint 5 — Small catalogue balance pass, human feedback, evidence

**Touch:** `crates/grus-sim/src/catalog.rs` plus relevant expectations, `README.md`, new `docs/hpa-289-survival-balance-evidence.md`; no algorithm rewrite.

- [ ] Write the **research arithmetic table first**: target 9,000; passive 2/s; each Lab +1/s; each Analyze +90 for 10 Crystals; no Labs 75 min, one instant Lab 50 min, two instant Labs 37.5 min, two Labs starting at 180 s ≈39 min (theoretical, **not** an affordability result). The 38–45 min goal needs an average 3.33–3.95 pts/s; real underground cost, heat, Crystals and timing determine feasibility.
- [ ] Record the unchanged/reference failure wave and 40-tower benchmark outcome. Get at least one **normal interactive owner playtest**: real Victory/Defeat time and wave, economy/heat/Lab/Analyze decisions, and pressure around waves 10 and 20–25. Distinguish observation from simulation conjecture.
- [ ] Tune only a small, observed subset of numerical **`catalog.rs`** fields (research/Lab/Analyze accessibility first for timing; tower, wave, economy for survivability). Keep an old/new/rationale/effect table. Repeat the explicit release-mode guard, fast regression/coverage and the fortress benchmark after changes. No new gameplay systems.
- [ ] Update README's reset/benchmark section: real fortified workload and synthetic wave-100 cap, 10s CI and 60s physical GPU expectations, historical one-core measurements **not comparable**. Evidence must contain actual commit/host/renderer, Rust/Godot commands, benchmark frame distribution/population/heat/tower shots/breach, balance guard result and owner playtest.
- [ ] **Merge this same single PR** once all engineering gates pass and **one owner playtest is recorded**, documenting follow-up balance issues instead of leaving an unbounded planning PR. Extra playtest iterations become **separate follow-up Linear tickets**.
- [ ] Record hardware-rendered 60s Linux x86_64 packaged benchmark as **BLOCKED / not run** while a suitable host does not exist. Do not claim performance acceptance on llvmpipe/macOS or hold otherwise verified engineering indefinitely on unavailable external hardware.

**Proof:** runnable CI, explicit measured evidence and one real playtest; no invented timings or benchmark results.

### Verification checklist (unchecked until executed)

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace --locked` (normal fast tests; long guard ignored)
- [ ] `cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --lcov --output-path lcov.info --locked --ignore-filename-regex 'tests\\.rs$'`
- [ ] `cargo test -p grus-sim --release --locked --test survival_balance -- --ignored`
- [ ] `cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1`
- [ ] 60 simulated seconds / fortress, actual tower-hit and replan/heat/alien-floor assertions
- [ ] All existing Godot smokes, Linux export/packaged E2E; exit 0 and `GRUS_BENCHMARK_200` marker at 10 wall-clock seconds
- [ ] One real owner normal-match playtest (including unfinished balance findings)
- [ ] Hardware-rendered Linux x86_64 60s release benchmark: **BLOCKED pending host** (not a passed gate)
- [ ] README, evidence and PR description reflect real results; no art/SFX or extra implementation PR

## Design review resolution

[GitHub review, 2026-11-03](https://github.com/cwchanap/Grus/pull/17#issuecomment-6620846291): all nine suggestions accepted: shared completion grants, release-only long guard before fortress, existing wall/replan tests reused, stronger deterministic fortress-shape assertions, fuel-free two-turret guard, no synthetic research override, up-front research arithmetic/passive-win guard, explicit merge bar, and tower mix asserted in Rust only. Implementation has **not** been performed by these documentation changes.
