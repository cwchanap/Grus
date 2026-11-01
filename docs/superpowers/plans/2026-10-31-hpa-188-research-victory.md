# HPA-188 — Research timer and Victory: implementation plan

**Issue:** https://linear.app/cwchanap/issue/HPA-188/grus-survival-68-research-timer-and-victory  
**Design:** [HPA-188 research/Victory contract](../specs/2026-10-31-hpa-188-research-victory-design.md)  
**Parent:** [Winter Survival](../specs/2026-10-21-winter-survival-design.md)  
**Base:** main including merged HPA-181 / PR #14  
**Status:** Planning only; no research gameplay has been implemented, no tests or CI results are claimed. **This draft PR is also the implementation PR.** One ticket, one branch, one PR.

## Why HPA-188 now

Linear lists HPA-188 as Todo, blocked only by HPA-181, now Done. Its prerequisite PR #14 is merged. HPA-188 unblocks HPA-288 underground levels (whose Labs add to research). The remaining art, SFX and benchmark tasks are deliberately independent or downstream; do not absorb them here.

The repository already has all the useful seams: catalog.rs for specs/constants, seed_skirmish/reset for durable match state, apply_player_command + CommandResult for paid actions, buildings.rs for real placement and completion, combat.rs for tower fire, waves.rs for deterministic threat budget, tick.rs for system order, and the GrusBridge snapshots and battlefield_controller HUD. There is no need to replace these pieces.

## Implementation checkpoints in this **single PR**

These are local development stages, **not additional Linear tickets or PRs**. The current branch starts with this plan and the design document only; implementation changes, verification, review replies and fixes go on the same branch.

### 1. Add research state, maths and canonical tick hook

**Touch:** crates/grus-sim/src/{research.rs,lib.rs,catalog.rs,tick.rs,fixture.rs}; focused research tests and crates/grus-sim/tests/system_order.rs.

- Define ResearchState with clamped f64 progress; seed it to zero in seed_skirmish. Ensure normal runtime reset/restart and benchmark fixture re-seed rather than reuse previous points. No Godot-side mutable research field.
- Put target/rate/Analyze conversion/milestones in catalog.rs. Research rate uses a surface term, 2 × surface_efficiency(world), added to an underground term that HPA-288 will introduce later; do not implement a dummy Underground resource now. Display ETA uses the exact same source-rate function as advancement.
- Implement step_research(world, seconds): no mutation without active gameplay and a living completed Team-1 Research Center; cap at 9,000; call resolve_result(TeamId(1)) exactly on reaching target. For deterministic threshold tests, start near a boundary in pure Rust rather than waiting thousands of ticks.
- Insert research after production and before visibility in step_tick. It therefore reads heat computed earlier in this tick. Combat before research gives Research Center destruction precedence in a same-tick tie. Update system_order.rs and the tick diagrams in CLAUDE.md/README to match the actual code; do not create another copied full-tick sequence in tests.
- Rust tests: warm vs cold rates (including 0.25 floor), rate freeze across phases, zero/absent RC, exact threshold/cap, passive Victory, same-tick lethal-combat tie, and new/restarted fixture = zero.

### 2. Implement Analyze through normal command dispatch

**Touch:** crates/grus-sim/src/{commands.rs,research.rs,lib.rs}; new unit tests alongside the existing command/research tests; bridge call in checkpoint 4.

- Add PlayerCommand::Analyze { issuer, building }. Existing apply_player_command gameplay gate stays the only session gate.
- Verify an owned, completed Research Center using commandable_building_entity and existing RejectReason variants. Spend 10 Crystals atomically through the existing TeamEconomy/ResourceStockpile helper; then add 90 points through the **same** research progression/milestone/victory helper used by passive research. No side-effect before validation.
- Input always names a BuildingId. It is one immediate command, not a job in ProductionQueue. Multiple Analyze requests in a fixed command batch are processed sequentially, including the case where the first settles Result and subsequent ones must reject.
- Rust tests: success with exactly 10 Crystals, shortage/foreign/wrong-kind/incomplete/missing/session-locked failures with no deduction, repeated Analyze, 25%/50%/75% crossings, clamping at 100%, and immediate session Victory.

### 3. Add advanced tower catalogue and unlock validation

**Touch:** crates/grus-sim/src/{catalog.rs,buildings.rs,combat.rs if needed,lib.rs}; crates/grus-sim/src/catalog/tests.rs and buildings/combat tests.

- Append FlameTower and CrystalLance to BuildingKind::ALL and existing BuildingSpec match. Preserve the issue's fixed costs, 2×2 footprints, range, counter target and heat demands (3 and 4). Put all missing first-pass HP/build time/damage/cadence numbers in the catalogue as **provisional** rather than scattering literals through systems.
- Extend the **existing** validate_placement locked-kind gate so only Team 1 can build these kinds and only after 25%/50% respectively. The same gate covers placement preview and applied commands. Reject before money is spent and before new occupancy/IDs are allocated.
- Generalize step_construction's current GunTurret-only AttackCooldown insertion to any newly completed building with BuildingSpec.combat.is_some(). The shared combat step already acquires targets, applies counter bonuses, scales completed tower cooldown with heat and awards kill Crystals. Do **not** add a second tower attack loop.
- Check building_view primitive/metadata and collision footprint handling for the new kinds. Reuse cold tint and current tracer feedback; don't create new art or audio.
- Rust tests: both unlock boundaries, correct reject/unchanged stockpile under lock, valid paid placement after unlock, construction completion grants attack capability, ranges and counters, fuel-dependent cold slowdown, correct Crystal kill credit and heat demand only after completion.

### 4. Apply 75% research boost at wave launch

**Touch:** crates/grus-sim/src/{waves.rs,research.rs,catalog.rs if needed}; waves tests, system_order.rs.

- Keep wave_budget(wave)'s base polynomial unchanged. Let launch_wave select effective integer budget based on the **current** ResearchState at the time of launch; use floor(base × 3 / 2) with saturating/widened arithmetic at or above 6,750 points.
- Pass that integer budget to the existing roster purchaser. Reuse unlock waves, 150-alien limit and overflow-to-HP implementation exactly as before. No mutable wave multiplier or retroactive re-rolling of rosters.
- Test below/exactly/above 75%; one boosted launch versus later repeated launches (never x2.25), already-launched roster unchanged, first launch after crossing threshold, two/three-zone waves, cap/overflow and extreme-budget safety. Make one canonical-tick test show waves launched earlier in the tick do not get research earned at its end.

### 5. Expose minimal bridge and working HUD

**Touch:** crates/grus-godot/src/lib.rs, godot/scripts/battlefield_controller.gd, godot/scenes/main.tscn; existing bridge parser/snapshot tests and relevant Godot smoke assertions.

- Add GrusBridge.research_snapshot() as a thin Rust projection: points, target, fraction, surface + total rate, ETA and derived unlock flags plus Analyze price/reward. Add GrusBridge.analyze(id) as one queued command; existing command feedback receives accept/reject, with no new feedback channel.
- In battlefield_controller.gd, read research_snapshot once per HUD frame, populate a ProgressBar/ETA label below the WaveLabel, and update Analyze availability. Show Analyze with the selected, completed Research Center only; disable unless Playing and stockpile Crystals ≥10. The sim remains the final validator.
- Add two build buttons to the 3-column BuildGrid: its existing seventh Coal Furnace leaves exactly two free cells on the third row. Use catalogue-derived cost/time tooltips and disable each button until its corresponding Rust research unlock flag is true. Avoid resizing/replacing the command panel unless a 1280×720 rendered smoke actually demonstrates overlap.
- Reuse the existing “Victory!” session overlay with winner_team=1; never create a parallel win screen. Verify snapshot fields are numeric/stable and that non-Playing screens have sensible progress/ETA.
- Extend the catalogue parser/unit tests to include both new tower strings and reject unknown kinds, and check HUD geometry and real selection/input dispatch in Godot.

### 6. Smoke tests, README and acceptance evidence

**Touch:** existing godot/scripts/{wave_smoke_test.gd,defenses_smoke_test.gd,reset_test.gd} or a **small** focused research smoke that reuses shared helpers, its corresponding scene and one CI invocation only if necessary; README.md, CLAUDE.md and any affected current bridge tests.

- **Godot boundary only:** verify a real frame-visible research bar and ETA advance under accelerated simulated time, the selected Research Center Analyze button consumes 10 Crystals gained via actual alien kills (not test-only injections) and adds 90 points, the existing command feedback is visible, new catalogue buttons/lock state render, and Restart returns to zero research. Prefer reusing the existing wave/defense journey and common helpers over copying a second full combat setup. Keep the result under the current smoke time budget.
- **Rust owns long-time/rare states:** 9,000-point Victory, exact milestone thresholds and the 75% wave budget are tested with controlled pure-sim state. A natural 9,000-point run is 4,500 simulated seconds; even at 20× that is 225 real seconds, incompatible with the existing 45–90-second smoke jobs. Do not add a long Godot timer test or fake the complete run with a GDScript cheat.
- Update README: research goal, ETA, Analyze conversion, milestone/build unlocks, x1.5 later-wave pressure, winning/losing, heat slowdown, and practical controls. Correct the HPA-449 text that currently says there is no Victory path. Update CLAUDE.md for the new canonical step sequence; keep the parent contract in sync if implementation decisions differ.
- Keep original e2e/export/benchmark workload running; don't change its meaning or relax gates simply to land the feature.

## Verification gates (record actual results on this PR)

- [ ] cargo fmt --all -- --check
- [ ] cargo clippy --workspace --all-targets -- -D warnings
- [ ] cargo test --workspace --locked
- [ ] cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1
- [ ] cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --lcov --output-path lcov.info --locked --ignore-filename-regex 'tests\\.rs$'
- [ ] Godot 4.7.1 import and existing bridge/reset/economy/combat/wave/defense smokes (including the new research boundary case), each with strict exit 0 and its sentinel
- [ ] Linux x86_64 smoke export, packaged smoke, e2e-feature export and exported-game bevy-e2e
- [ ] Existing 10-second CI benchmark survives and any relevant measured performance regression is explained
- [ ] 1280×720 HUD shows research without overlap or input leakage, and the research → Analyze → unlock → Victory mechanics all have appropriate simulation/bridge coverage
- [ ] The merged implementation eventually lives in **this same PR**, not a second one

## Risks / decisions to verify during implementation

1. **Defeat versus Victory ordering.** Combat precedes passive research in the canonical tick; a lethal hit on the Research Center wins the same-tick tie for aliens. Immediate Analyze command happens before the tick; an accepted conversion that finishes research settles Victory before later combat. Assert both behaviors explicitly.
2. **Research unlock bypass.** Disabling Godot buttons is insufficient: validate both previews and authoritative placement. A stale selection or queued click must not spend resources below the milestone.
3. **Wave multiplier and roster cap.** Multiply the budget before purchasing the roster, not spawned counts or HP afterward. Preserve existing 150-alien/overflow logic and integer rounding.
4. **Smoke Crystal solvency.** Existing wave smoke proves just one Swarmer Crystal; Analyze needs 10. Reuse a real combat/defense journey that can reliably earn at least 10 or add a bounded continuation with player-built defences. Don't invent starting Crystals or mutate the sim from the UI test.
5. **Long duration and float drift.** Use f64 accumulated progress and boundary tests, not wall-clock sleeps; keep the complete victory check in pure Rust.

## Explicitly excluded

HPA-288 underground rooms and Lab production; HPA-289 full balance and 150-alien final stress benchmark; HPA-291 structure art; HPA-44 HUD icons; HPA-269 audio. Those stay in their dedicated issues. No research tree/framework, alternate victory, extra map, save format, or compatibility layer.
