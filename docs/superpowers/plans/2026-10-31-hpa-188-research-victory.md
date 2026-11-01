# HPA-188 — Research timer and Victory: implementation plan

**Issue:** https://linear.app/cwchanap/issue/HPA-188/grus-survival-68-research-timer-and-victory  
**Design:** [HPA-188 research/Victory contract](../specs/2026-10-31-hpa-188-research-victory-design.md)  
**Parent:** [Winter Survival](../specs/2026-10-21-winter-survival-design.md)  
**Base:** main including merged HPA-181 / PR #14  
**Status:** Implemented locally on this branch (all six checkpoints landed, commits `7fa5515`..`5d7dfef`); required Linux x86_64 CI still pending — local evidence below is macOS arm64, not CI. **This draft PR is also the implementation PR.** One ticket, one branch, one PR.

## Why HPA-188 now

Linear lists HPA-188 as In Progress; its sole blocker HPA-181 is Done. Its prerequisite PR #14 is merged. HPA-188 unblocks HPA-288 underground levels (whose Labs add to research). The remaining art, SFX and benchmark tasks are deliberately independent or downstream; do not absorb them here.

The repository already has all the useful seams: catalog.rs for specs/constants, seed_skirmish/reset for durable match state, apply_player_command + CommandResult for paid actions, buildings.rs for real placement and completion, combat.rs for tower fire, waves.rs for deterministic threat budget, tick.rs for system order, and the GrusBridge snapshots and battlefield_controller HUD. There is no need to replace these pieces.

## Implementation checkpoints in this **single PR**

These are local development stages, **not additional Linear tickets or PRs**. The current branch starts with this plan and the design document only; implementation changes, verification, review replies and fixes go on the same branch. Prefer **one readable commit per checkpoint** (plus small focused review-fix commits) within this same PR, as in HPA-181.

### 1. Add research state, maths and canonical tick hook

**Touch:** crates/grus-sim/src/{research.rs,lib.rs,catalog.rs,tick.rs,fixture.rs}; focused research tests and crates/grus-sim/tests/system_order.rs.

- Add `ResearchState { points: f64 }`, the neutral-default `research_points(world)` accessor, `research_rate(world)`, `add_research(world, points)`, `building_unlocked(world, kind)` and `step_research(world, seconds)` in `research.rs`. Seed default zero in `seed_skirmish`, and **remove ResearchState** from the bridge's explicit `clear_gameplay_world` resource list beside `HeatState` before reseeding on reset/restart/benchmark reset. No GDScript-side mutable research state or duplicated milestone flags.
- Put `RESEARCH_TARGET_POINTS` (9,000), `RESEARCH_BASE_PER_SECOND` (2), `ANALYZE_COST: Cost` (10 Crystals), `ANALYZE_RESEARCH_GAIN` (90), `RESEARCH_WAVE_BOOST_PERCENT` (75) and budget multiplier numerator/denominator (3/2) in `catalog.rs`. The `ANALYZE_COST` const uses explicit food/scrap/coal/crystals fields: `Default::default()` cannot initialize a const. Add `BuildingSpec.research_unlock_percent` with 0 / 25 / 50 and derive unlock points from target × percent / 100. Research rate uses a surface term, 2 × `surface_efficiency(world)`, added to an underground term HPA-288 will introduce later; do not implement a dummy Underground resource now. Display ETA uses the same source-rate function as advancement.
- Implement `step_research(world, seconds)` gated **only by `gameplay_active`**, accumulating the fixed-step delta as f64, capped at 9,000. The seeded Research Center is complete and unplaceable; its destruction already settles Result inside combat, so a second living/completed-core check is dead logic. Call `resolve_result(TeamId(1))` on reaching the target. Use a shared `add_research` helper for both passive growth and Analyze, and seed tests near thresholds rather than waiting thousands of ticks.
- Insert research after production and before visibility in step_tick. It therefore reads heat computed earlier in this tick. Combat before research gives Research Center destruction precedence in a same-tick tie. Update system_order.rs and the tick diagrams in CLAUDE.md/README to match the actual code; do not create another copied full-tick sequence in tests.
- Rust tests: warm vs cold rates (including 0.25 floor), freeze across Start/Paused/Result, exact threshold/cap, passive Victory, same-tick lethal-combat tie, and fresh/restarted fixture = zero. Omit impossible absent/incomplete Research Center tests for a guard that no longer exists.

### 2. Implement Analyze through normal command dispatch

**Touch:** crates/grus-sim/src/{commands.rs,research.rs,lib.rs}; new unit tests alongside the existing command/research tests; bridge call in checkpoint 4.

- Add `PlayerCommand::Analyze { issuer, building }`. The existing `apply_player_command` gameplay gate stays the only session gate; `ANALYZE_COST: Cost` goes directly through the existing stockpile's `try_spend`.
- Mirror the small `production.rs::validate_enqueue` validation order, without a new abstraction: `commandable_building_entity` → owned → completed → `ResearchCenter` kind. Reuse `BuildingMissing`, `NotOwned`, `Locked`, `WrongProducer` and `InsufficientResources`. Spend `ANALYZE_COST` (10 Crystals) atomically via `TeamEconomy` / `ResourceStockpile::try_spend`, then call **the same `add_research` helper** as passive research with `ANALYZE_RESEARCH_GAIN` (90). No side effect before validation, and no new reject codes.
- Input always names a BuildingId. It is one immediate command, not a job in ProductionQueue. Multiple Analyze requests in a fixed command batch are processed sequentially, including the case where the first settles Result and subsequent ones must reject.
- Rust tests: success with exactly 10 Crystals, shortage/foreign/wrong-kind/missing/session-locked failures with no deduction, repeated Analyze, 25%/50%/75% crossings, clamping at 100%, and immediate session Victory. Preserve the shared complete-building validation, but don't hand-spawn an impossible incomplete Research Center just to test it.

### 3. Add advanced tower catalogue and unlock validation

**Touch:** crates/grus-sim/src/{catalog.rs,buildings.rs,combat.rs if needed,lib.rs}; crates/grus-sim/src/catalog/tests.rs and buildings/combat tests.

- Append FlameTower and CrystalLance to `BuildingKind::ALL` and existing `BuildingSpec` matches. Set `research_unlock_percent: 25` / `50` for them and `0` for existing kinds. Preserve fixed costs, 2×2 footprints, ranges, counter targets and heat demands (3 and 4). Put missing first-pass HP/build time/damage/cadence numbers in the catalogue as **provisional** rather than scattering literals through systems.
- Extend the **existing** `validate_placement` Locked gate with `research.rs::building_unlocked(world, kind)`, comparing points against `RESEARCH_TARGET_POINTS × building_spec(kind).research_unlock_percent / 100`. Team 1 alone may build both advanced towers; kinds with 0 stay available. Preview and applied commands share this gate. Reject before spending currency or allocating occupancy/IDs. No per-kind unlock match in GDScript or a second authoritative unlock table.
- Generalize step_construction's current GunTurret-only AttackCooldown insertion to any newly completed building with BuildingSpec.combat.is_some(). The shared combat step already acquires targets, applies counter bonuses, scales completed tower cooldown with heat and awards kill Crystals. Do **not** add a second tower attack loop.
- Explicitly add `FlameTower` and `CrystalLance` to the existing `GunTurret` styling branch in `godot/scripts/building_view.gd::_style_kind`, and check placement footprint/metadata. Reuse cold tint, primitive tower mesh and current tracer feedback; don't create new art or audio.
- Rust tests: both unlock boundaries, correct reject/unchanged stockpile under lock, valid paid placement after unlock, construction completion grants attack capability, ranges and counters, fuel-dependent cold slowdown, correct Crystal kill credit and heat demand only after completion.

### 4. Apply 75% research boost at wave launch

**Touch:** crates/grus-sim/src/{waves.rs,research.rs,catalog.rs if needed}; waves tests, system_order.rs.

- Keep `wave_budget(wave)`'s base polynomial unchanged. Let `launch_wave` select an effective integer budget using `ResearchState` and catalogue `RESEARCH_WAVE_BOOST_PERCENT = 75`: either normal budget or floor(base × 3 / 2), with widened/saturating arithmetic. No separately stored escalation flag or repeated multiplier.
- **Change the existing signature** from `plan_roster(wave)` to `plan_roster(wave, budget)`; update its existing call sites (approximately 11 including tests) so unchanged callers pass `wave_budget(wave)` and `launch_wave` passes the launch-time effective budget. Reuse alien unlock waves, 150-alien limit and overflow-to-HP exactly as before. No second roster function, count/HP multiplier or retroactive re-roll.
- Test below/exactly/above 75%; one boosted launch versus later repeated launches (never x2.25), already-launched roster unchanged, first launch after crossing threshold, two/three-zone waves, cap/overflow and extreme-budget safety. Make one canonical-tick test show waves launched earlier in the tick do not get research earned at its end.

### 5. Expose minimal bridge and working HUD

**Touch:** crates/grus-godot/src/lib.rs, godot/scripts/battlefield_controller.gd, godot/scenes/main.tscn; existing bridge parser/snapshot tests and relevant Godot smoke assertions.

- Extend `catalogue_snapshot()` (read once in `_ready`) with a static `research` dictionary: `target_points`, `analyze_cost` serialized by existing `cost_dict`, `analyze_gain_points`. Each building catalogue entry exposes derived `unlock_points` (0 for always available). Add only dynamic `GrusBridge.research_snapshot()` fields: `points`, `rate_per_second`, `eta_seconds`. The frontend derives percent and availability from these; it receives **no milestone flags, cost/gain copies or independent unlock state**. `GrusBridge.analyze(id)` queues one normal command using existing accept/reject feedback and revision.
- In `battlefield_controller.gd`, read `research_snapshot()` once per HUD frame and populate a compact ProgressBar/ETA label below WaveLabel using cached static target points. **Enable Analyze while Playing with the owned Research Center selected, regardless of Crystals**; do not pre-check affordability in GDScript. Rust rejects a 0-Crystal click with `InsufficientResources` and the normal command feedback. The sim remains final validator.
- Add two build buttons to the existing 3-column BuildGrid (Coal Furnace already occupies the seventh of nine cells). The existing generic `_ready` button loop binds catalogue costs, construction times and `unlock_points`, without per-kind conditions. On **each** HUD update, use `research_snapshot.points` to refresh locked-button disabled state; a one-time `_ready` check would never unlock buttons later. Rust handles rejected stale clicks. Avoid resizing/replacing the command panel unless 1280×720 rendered smoke shows actual overlap.
- Reuse the existing “Victory!” session overlay with winner_team=1; never create a parallel win screen. Verify snapshot fields are numeric/stable and that non-Playing screens have sensible progress/ETA.
- Extend the catalogue parser/unit tests to include both new tower strings and reject unknown kinds, and check HUD geometry and real selection/input dispatch in Godot.

### 6. Smoke tests, README and acceptance evidence

**Touch:** existing godot/scripts/{wave_smoke_test.gd,defenses_smoke_test.gd,reset_test.gd} or a **small** focused research smoke that reuses shared helpers, its corresponding scene and one CI invocation only if necessary; README.md, CLAUDE.md and any affected current bridge tests.

- **Godot boundary only:** start a fresh Playing match (0 Crystals), select the real Research Center and press its **enabled** Analyze button through input; assert `InsufficientResources` text and existing numeric reject code **8**, unchanged 0-Crystal stockpile and no 90-point research jump. Check ProgressBar and live ETA advance at 20×, generic locked-button/tooltips render from catalogue, and Restart resets research to zero. Reuse current smoke helpers or a small isolated journey. **Do not** require ten live alien kills, mutate test-only resources or duplicate the defense smoke. The successful 10-Crystal spend / +90-points bridge command's underlying sim path is covered deterministically in Rust; the Godot test asserts button→queued command→typed feedback.
- **Rust owns long-time/rare states:** 9,000-point Victory, exact milestone thresholds and the 75% wave budget are tested with controlled pure-sim state. A natural 9,000-point run is 4,500 simulated seconds; even at 20× that is 225 real seconds, incompatible with the existing 45–90-second smoke jobs. Do not add a long Godot timer test or fake the complete run with a GDScript cheat.
- Update README: research goal, ETA, Analyze conversion, milestone/build unlocks, x1.5 later-wave pressure, winning/losing, heat slowdown, and practical controls. Correct the HPA-449 text that currently says there is no Victory path. Update CLAUDE.md for the new canonical step sequence; keep the parent contract in sync if implementation decisions differ.
- Keep original e2e/export/benchmark workload running; don't change its meaning or relax gates simply to land the feature.

## Verification gates (record actual results on this PR)

- [ ] cargo fmt --all -- --check
- [ ] cargo clippy --workspace --all-targets -- -D warnings
- [ ] cargo test --workspace --locked
- [ ] cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1
- [ ] cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --lcov --output-path lcov.info --locked --ignore-filename-regex 'tests\.rs$'
- [ ] Godot 4.7.1 import and existing bridge/reset/economy/combat/wave/defense smokes (including the new research boundary case), each with strict exit 0 and its sentinel
- [ ] Linux x86_64 smoke export, packaged smoke, e2e-feature export and exported-game bevy-e2e
- [ ] Existing 10-second CI benchmark survives and any relevant measured performance regression is explained
- [ ] 1280×720 HUD shows research without overlap or input leakage; rejected Analyze path is exercised by real Godot input; accepted Analyze, unlock thresholds and Victory are covered by focused Rust tests
- [ ] The merged implementation eventually lives in **this same PR**, not a second one

### Local verification evidence — macOS arm64, 2026-11-01 (not Linux CI)

All six checkpoints are implemented on this branch. The results below were produced locally on **macOS arm64** with the repository-pinned **Rust 1.95.0** and **Godot 4.7.1**. They are evidence for review, **not** the required Linux x86_64 CI runs — every gate checkbox above remains open pending CI.

Controller-verified on macOS arm64 (Rust 1.95.0, Godot 4.7.1):

- `cargo fmt --all -- --check` — clean
- `cargo clippy --workspace --all-targets --locked -- -D warnings` — clean
- `cargo clippy -p grus-godot --all-targets --features e2e --locked -- -D warnings` — clean
- `cargo test --workspace --locked` — pass (28 bridge, 309 sim, 16 integration tests)
- `cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1` — 31 passed
- `cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --lcov --locked --ignore-filename-regex 'tests\.rs$'` — 96.43% lines (4,268/4,426), ≥90% gate met
- Godot editor import plus all six headless smokes (bridge `smoke_test`, `reset_test`, `economy_smoke_test`, `combat_lifecycle_smoke_test`, `wave_smoke_test`, `defenses_smoke_test`) — each strict exit 0 with its sentinel; the combat-lifecycle sentinel covers the zero-Crystal Analyze press (typed `InsufficientResources`, reject code 8) and the post-reset zero-research state
- Headless, non-e2e 200-unit 10 s benchmark (macOS arm64, `--headless` — not rendered, not Linux): exit 0; 796 frames; mean 12.566 ms; p95 37.334 ms; p99 42.909 ms; 79.6 fps; 2 ObjectDB leak warnings at exit

Implementer-reported on macOS arm64 (Apple M1 Pro): rendered **1280×720** bridge and combat-lifecycle smokes passed, plus the throttled combat smoke. Not independently re-verified by the controller.

Explicitly **NOT RUN** locally — no Linux toolchain target and no running Docker daemon on this host; these remain pending required CI and their checkboxes stay open:

- Linux x86_64 smoke export and packaged smoke
- e2e-feature export and exported-game bevy-e2e
- Linux CI benchmark run

## Risks / decisions to verify during implementation

1. **Defeat versus Victory ordering.** Combat precedes passive research in the canonical tick; a lethal hit on the Research Center wins the same-tick tie for aliens. Immediate Analyze command happens before the tick; an accepted conversion that finishes research settles Victory before later combat. Assert both behaviors explicitly.
2. **Research unlock bypass.** Disabling Godot buttons is insufficient: validate both previews and authoritative placement. A stale selection or queued click must not spend resources below the milestone.
3. **Wave multiplier and roster cap.** Multiply the budget before purchasing the roster, not spawned counts or HP afterward. Preserve existing 150-alien/overflow logic and integer rounding.
4. **Stable smoke boundary.** Starting stockpile intentionally has zero Crystals, which makes the real Analyze button's `InsufficientResources` feedback deterministic. Assert the existing reject code 8 and unchanged stockpile/research without depending on killing 10 of the first wave's 12 Swarmers. Keep successful spend/credit/threshold semantics in Rust tests.
5. **Long duration and float drift.** Use f64 accumulated progress and boundary tests, not wall-clock sleeps; keep the complete victory check in pure Rust.

## Explicitly excluded

HPA-288 underground rooms and Lab production; HPA-289 full balance and 150-alien final stress benchmark; HPA-291 structure art; HPA-44 HUD icons; HPA-269 audio. Those stay in their dedicated issues. No research tree/framework, alternate victory, extra map, save format, or compatibility layer.
