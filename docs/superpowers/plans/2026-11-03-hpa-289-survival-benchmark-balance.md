# HPA-289 — Survival benchmark and first balance pass: implementation plan

Status: **Draft for review; implementation and evidence pending** · 2026-11-03  
Issue: [HPA-289](https://linear.app/cwchanap/issue/HPA-289/grus-survival-88-wave-benchmark-and-first-balance-pass)  
Design: [HPA-289 design spec](../specs/2026-11-03-hpa-289-survival-benchmark-balance-design.md)  
Parent: [Winter Survival §9](../specs/2026-10-21-winter-survival-design.md)  
Baseline: `main` after merged [PR #16 / HPA-288](https://github.com/cwchanap/Grus/pull/16)

## Delivery contract

**One ticket = one PR.** This planning branch/PR must evolve into the complete HPA-289 implementation, not be followed by a second coding PR. Keep the diff limited to benchmark fixture, existing GDScript runner/CI, one balance journey, existing catalogue, evidence and README. No generated art or SFX belongs here.

The current repository **already has** the real 50-human/150-alien director workload: do not redo the HPA-449 PvP conversion. The missing piece is a fortified city and a defensible tuning/evidence loop.

## Checkpoint 1 — Author the fortified benchmark fixture (Rust)

**Touch:** `crates/grus-sim/src/fixture.rs`, `crates/grus-sim/src/fixture/tests.rs`; reuse existing `map`, `buildings`, `waves`, `heat`, `combat` structs.

- [ ] Build a small benchmark-only authoring helper for **completed** buildings, taking kind + anchor, reading `building_spec`, registering `BuildingIndex` and allocating unique IDs, setting `Footprint`, `Health`, and `GridMap` blocked cells or `set_gate`. Assert conflicts/invalid coordinates instead of silently skipping them. Keep regular `seed_skirmish` and gameplay placement unchanged.
- [ ] Author one wall perimeter with a Gate, 40 combat towers (initial candidate 24 Gun / 10 Flame / 6 Crystal), and enough Coal Furnaces (nine for this mix) for full-speed fire at benchmark start. Choose fixed coordinates after checking the actual ruins, all 18 sources, Research Center, workforce, approach lanes and staged reinforcements. Set **benchmark-only** fuel with headroom for the 60-second test; do not modify `catalog.rs` to feed the benchmark.
- [ ] Stage the existing four Workers and 46 defenders safely in/around the fortified city; issue ordinary accepted Gather and defensive movement/combat orders. Preserve current 150-alien director spawn and 200-unit census. Keep `BENCHMARK_WAVE=100` clearly synthetic and keep the session-free active convention.
- [ ] Replace tests that assume **one** building with tests for one Research Center plus exact tower/wall/gate/furnace counts and fully valid non-overlapping occupancy. Assert no stranded Worker gathers/defenders, live surface heat/fuel, stable 200 unique unit IDs, 150 aliens, 50 humans, correct wave state and real combat activity.
- [ ] Extend existing `crates/grus-sim/tests/occupancy_replan.rs` (or a single adjacent targeted test) with 150 active aliens, a destructible wall/gate and real building destruction. Assert map revision increments, routes adapt/re-acquire and the sim remains active. Never assert elapsed milliseconds on CI and do not add a timed mutation bridge just for this.

**Proof:** `cargo test -p grus-sim --locked`; a 60-second **simulated** benchmark fixture activity check and a deterministic breach/replan assertion. This checkpoint must not alter normal game balance.

## Checkpoint 2 — Adapt the existing 1080p runner and CI

**Touch:** `godot/scripts/benchmark_200.gd`, `.github/workflows/ci.yml`; retain `godot/scenes/benchmark_200.tscn` unless a scene adjustment is essential.

- [ ] In `_run`, wait for **200 unique live unit views** and the **complete building census** (one core, 40 combat towers, walls, gates, furnaces). Drop the existing `building_views.size() == 1` assumption. Validate the expected tower kind totals and that the authored core still exists. Fail with actionable counts on mismatch.
- [ ] Preserve viewport size, `GRUS_BENCHMARK_SECONDS` strict parsing (default/CI 10; hardware 60), visibility gate, warmup, time-weighted live/visible counts, liveness checkpoints and no periodic replay. Append initial fortress counts/heat to the `GRUS_BENCHMARK_200` marker; keep the existing `start_units=200` contract and frame metrics.
- [ ] Update the CI benchmark step name and assert exit 0 + full sentinel + fortress census. Keep software `gl_compatibility` and log capture; **no machine-dependent CI FPS floor**. Check `GRUS_MAIN_SCENE` Linux packaging instructions still point to the same runner.
- [ ] Run a local 10-second benchmark where the available environment supports it; keep actual stdout and explicitly note hardware/renderer. If 60 seconds of rendered load do not breach a gate/wall, report that limitation instead of claiming reroute performance was sampled.

**Proof:** actual Godot 1080p benchmark marker with 200 units, fortress census, movement/combat liveness and meaningful live/visible samples; CI software-GL job exits successfully.

## Checkpoint 3 — Add the 10-to-40 wave balance regression

**Touch:** add `crates/grus-sim/tests/survival_balance.rs` (or similarly scoped integration test); reuse existing `seed_skirmish`, `step_tick`, `MatchSession` and fixture helpers where suitable.

- [ ] Seed an ordinary Playing survival world with a **small, fixed, authored** reference defense: completed walls/Gate, a few permitted towers, adequate supplied heat/fuel. Keep the Research Center as the objective. No player actions after setup, Lab acceleration, invulnerable entities, direct wave instantiation or test-only numeric stat overrides.
- [ ] Advance at **`SIM_STEP_SECONDS` (20 Hz)** through actual wave deadlines and combat/navigation. At the wave-11 deadline, assert the core still stands and the run remains active (survived all of wave 10's interval).
- [ ] Stop on real Defeat and assert Defeat occurs **before the wave-40 deadline**, with wave progress having advanced normally. Fail on early loss, unexpected Victory, route deadlock or a run that survives to wave 40. This should be **one** regular CI test; measure its runtime and avoid duplicating the same long simulation across several tests.
- [ ] Lock the reference setup once reviewed. Make first-pass tuning in the catalogue, not by editing this test's defenses to force a green check.

**Proof:** deterministic repeated result, bounded CI runtime, no special simulator/clock implementation.

## Checkpoint 4 — Profile first; tune only observed pain points

**Touch:** `crates/grus-sim/src/catalog.rs` and associated catalogue expectation tests; no gameplay formulas or frontend literals.

- [ ] Record **pre-tune baseline**: new benchmark frame distribution and population trajectory, fixed-defense failure wave, basic human skirmish progression with Food/Scrap/Coal/Crystals, heat availability, Lab/Analyze and actual research ETA. Identify which parameters make the experience too easy, too hard, too slow, or impossible.
- [ ] Iterate a **small set** of catalogue parameters: provisional Flame Tower / Crystal Lance HP, damage and cooldown; room prices/times/yields; economic starting costs; Research/Analyze rate or conversion; wave budget/alien HP/damage, only as justified by evidence. Keep per-change old/new values and rationale. Avoid unrelated refactors, new units or difficulty modes.
- [ ] Re-run the wave guard and benchmark after catalogue changes; make sure balance improvements have not hidden a 150-alien performance regression. If a measured hot path needs an optimization, make the smallest local fix in the existing module and add a regression test; do not introduce a profiling or navigation framework.
- [ ] Request **owner playtest** of a normal match and record when research Victory occurs, waves reached, defeat causes, build/research/underground choices and subjective difficulty. Target 38–45 minutes / waves 20–25 but do not claim success from the synthetic benchmark or automated fixed defense.

**Proof:** one comparison table in evidence with catalogue deltas and reproducible before/after observations. Any claim about human enjoyment or victory time requires a human result.

## Checkpoint 5 — Evidence, documentation, CI and review

**Touch:** `README.md`, new `docs/hpa-289-survival-balance-evidence.md`, and only the relevant lines of `.github/workflows/ci.yml`.

- [ ] Correct README's benchmark/reset descriptions to describe the fortified 200-unit load (not the prior one-core version). Keep legacy numbers clearly marked **historical / non-comparable**. Explain 40-tower/wall/gate workload, heat/fuel, synthetic wave-100 cap, 10s CI vs 60s hardware sampling and evidence limitations.
- [ ] Record **actual** machine/renderer, Godot/Rust revision, commands, start tower/unit census, build/world heat, 10s frame metrics (mean, p50/p95/p99/max, FPS), end/live/visible counts, liveness and wall/gate replan proof. Separate software-rendered llvmpipe CI from physical GPU results.
- [ ] If an actual hardware-rendered **Linux x86_64** host is available, run the **release-export** benchmark for 60s using the existing package script, and record its raw marker. If not available, mark hardware acceptance **BLOCKED**; never substitute a macOS headless or Linux llvmpipe number as hardware-rendered acceptance.
- [ ] Verify all existing Godot smokes, Linux package/export, exported-game E2E and the benchmark CI marker, plus the balance guard and 90% sim coverage. Check complete logs/exit statuses rather than inferring from a printed sentinel alone.
- [ ] Keep all implementation commits and review follow-ups on **this PR**. Update Linear HPA-289 with the PR link; mark Done only when required verified acceptance is actually satisfied or an explicit owner ruling handles the external hardware/playtest dependency.

### Commands/checklist (leave unchecked until actually run)

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace --locked`
- [ ] `cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --lcov --output-path lcov.info --locked --ignore-filename-regex 'tests\\.rs$'`
- [ ] `cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1`
- [ ] CI Godot 4.7.1 smokes, packaged Linux export, exported-game E2E, exit 0 + `GRUS_BENCHMARK_200` fortress marker at 10s
- [ ] 60s release-export GPU benchmark on qualified Linux host (or explicitly **BLOCKED**)
- [ ] Owner normal-playthrough timing/feel feedback (or explicitly **PENDING**)
- [ ] README, evidence, Linear link, no new art/SFX or second PR

## Review focus

1. Is the proposed 24/10/6 tower mix and nine-furnace heat provisioning representative without making the 150-alien benchmark artificially idle?
2. Are all placement coordinates/test assertions grounded in the actual authored `MapFixture`, not a copied Godot map or assumed open ring?
3. Does the separate breach/replan stress test cover dynamic occupancy without putting scripted route bursts back into the frame-timing loop?
4. Is the full-tick survive-wave-10 / lose-before-wave-40 test affordable in CI, with no false playtest equivalence?
5. Are performance and balance measurements clearly separated from unrun hardware and human-acceptance dependencies?
