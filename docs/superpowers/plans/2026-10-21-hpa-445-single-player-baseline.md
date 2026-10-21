# HPA-445 — Single-player baseline cutover: design and implementation plan

**Linear:** https://linear.app/cwchanap/issue/HPA-445/grus-survival-18-single-player-baseline-remove-pvp-ai-and-mirrored
**Base audited:** cwchanap/Grus main @ 60a9c0440a0ff85df1826e98036b4ec0c1ae709a (2026-10-20)
**Design:** ../specs/2026-10-21-winter-survival-design.md
**PR policy:** **Exactly one PR for HPA-445.** This planning draft must become the implementation PR; do not open a separate follow-up PR to perform the changes.
**Status:** Proposed implementation plan — **documentation only**, not yet implemented or tested.

## Goal and acceptance contract

Remove the economic AI opponent and mirrored-start launch mode, leaving a playable human Team 1 on the existing battlefield against a **passive, still-authored Team 2 base**. Preserve economy, fog, combat, victory, restart, and the session-free benchmark. Introduce the Winter Survival design spec so HPA-448 and later tickets have a checked-in contract.

Acceptance is the current Rust/Godot CI (format, warnings-as-errors Clippy, workspace tests, >=90% sim line coverage, bridge/reset/economy/combat lifecycle smoke scenes, current benchmark). The scouting-AI smoke and only that smoke is deleted along with its CI invocation.

## Baseline findings verified against main

1. grus-sim/src/lib.rs re-exports AiController / decision helpers, AiMapPlan, and seed_skirmish_mirrored. The entire economic AI lives in grus-sim/src/ai.rs and ai/tests.rs. These exports have no surviving runtime purpose after HPA-445.
2. grus-sim/src/fixture.rs holds both the two authored TeamStart rows and the optional mirrored seed; seed_skirmish currently delegates to seed_skirmish_mirrored(..., false). Its AiMapPlan / team_plan map is AI-specific; the normal authored map and benchmark staging logic are **not**. Some reflection helpers are also used by the benchmark: check call sites before pruning.
3. grus-godot/src/lib.rs currently captures GRUS_MIRRORED_START into a MirroredStart resource at app build, uses a bool-selected seed_match_world for boot/reset, inserts a Team-2 AiController, and runs advance_ai after advance_visibility in FixedUpdate. There are dedicated bridge tests for mirrored launch, AI reseed/decisions, and AI disablement.
4. disable_ai_for_test is a Start-only bridge method used by smoke_test.gd and combat_lifecycle_smoke_test.gd. Their scripted gameplay should naturally run without opponent AI after removal. No replacement test-only toggle is necessary.
5. system_order.rs contains an AI-specific fresh-fog decision test; other visibility/order tests are still relevant.
6. godot/scenes/scouting_ai_smoke_test.tscn, godot/scripts/scouting_ai_smoke_test.gd and its .gd.uid exist; CI has a named HPA-473 scouting-AI smoke step.
7. Benchmark mode is distinct from normal play: reset_benchmark_world seeds the default world, extends it with 96 scripted units **per team**, and omits MatchSession, allowing 200 units to move/fight regardless of AI. Preserve it, including the visibility/liveness metric names.
8. README.md and both AGENTS.md / CLAUDE.md describe AI and the mirrored launch. Preserve historical HPA-473 design documents as history, but update active developer guidance and current behavior descriptions.

## Recommended approach

**Delete the AI, do not disable it.** Eliminate the AI module and AI-only helpers, and use a single non-mirrored seed path for runtime, restart and benchmark. Keep Team 2's starting Town Center, its four Villagers, its economy resource and its visibility data. Team 2 simply receives no autonomous commands. No inert AI controller, mode flag, alternate scenario type, compatibility alias, new feature toggle, replacement test seam or special combat logic.

Keep the existing single Bevy World and bridge architecture. The only fixed-step change is removing advance_ai; keep advance_visibility and drain_route_reject_feedback in their current relative order. Do not conflate this ticket with the map recentering, new catalogue, alien waves or research.

## Implementation checklist (all in this same PR)

### 1. Cut AI from the simulation crate

- Delete crates/grus-sim/src/ai.rs and crates/grus-sim/src/ai/tests.rs.
- Remove pub mod ai and its public re-exports from crates/grus-sim/src/lib.rs.
- Remove AiMapPlan, MapFixture::team_plan, and their AI-only reflection/unit tests in fixture.rs and fixture/tests.rs. **Retain** MapFixture::team_starts, seed_skirmish, the authored resource/map data, and any point-reflection routines used by the 200-unit scripted benchmark.
- Inline the normal TeamStart ordering directly into seed_skirmish and delete seed_skirmish_mirrored / its bool switch. Do not change TeamId, stable ids or entity insertion order.
- Delete the AI fresh-fog integration test in crates/grus-sim/tests/system_order.rs and its AI-only imports. Preserve meaningful post-production visibility and route-feedback ordering coverage.
- Remove AI-only full-match journeys wherever they live; retain combat/session tests exercising a passive enemy target. Search for remaining AI names and judge each occurrence rather than deleting entire test modules.

### 2. Simplify the Godot bridge to one seed path

- Drop AiController, step_ai and seed_skirmish_mirrored imports, MirroredStart resource, mirrored_start_from_env, mirrored_choice, and every GRUS_MIRRORED_START read.
- Change seed_match_world(World) to always call seed_skirmish and refresh the initial visibility. Adapt setup_fixture, reset_fixture_world and reset_benchmark_world callers; keep a single ordinary start seed path and preserve a session-free benchmark.
- Remove the advance_ai function and system from FixedUpdate. Leave the existing command / combat / movement / economy / construction / production / visibility / route feedback chain intact.
- Delete disable_ai_for_test from GrusBridgeNode and disable_ai_in_world. Both are obsolete in a genuinely passive Team-2 fixture.
- Remove AiController cleanup from clear_gameplay_world, but keep all gameplay entity, indexes, visibility, session, event, pending command and feedback cleanup. Restart must still clear remembered fog and pending commands.
- Refactor bridge Rust tests that assumed AI setup, mirrored boot/restart or AI internal state into non-AI fixture/reset/benchmark assertions. Preserve the existing readiness, Start/Playing and fog tests; do not erase generic coverage to make the suite green.

### 3. Remove the legacy smoke and adapt remaining scenarios

- Delete godot/scenes/scouting_ai_smoke_test.tscn, godot/scripts/scouting_ai_smoke_test.gd, and godot/scripts/scouting_ai_smoke_test.gd.uid.
- Remove the named scouting AI smoke invocation, marker expectation and teardown handling from .github/workflows/ci.yml. **Do not remove** the independent fog behavior from normal scenes/smokes.
- Delete the disable_ai_for_test calls and obsolete surrounding comments in godot/scripts/smoke_test.gd and combat_lifecycle_smoke_test.gd. Keep their scripted choreography, attacks, outcome/restart proof and result-state behavior unchanged.
- Preserve reset_test.gd's exact 8-unit / 2-building / 18-resource census and stable IDs. The passive enemy remains available for conquest until HPA-449.
- Preserve godot/scripts/benchmark_200.gd and benchmark_200.tscn workload/metrics. Only update misleading prose or assumptions about AI-spawned extra foundations. The script's 200-unit reset should still show real movement/combat without advance_ai.

### 4. Documentation cutover

- The Winter Survival design specification was introduced in this PR as docs/superpowers/specs/2026-10-21-winter-survival-design.md, assembled from current Linear contracts because the external Claude artifact was inaccessible. Review differences against that artifact when available without expanding this ticket.
- README: remove the “economic AI” feature section, two-sided full-match journey claims, the mirrored-start feature and scouting-AI smoke commands. Say clearly that Team 2 is currently a passive target **until task 3**, while fog, minimap, combat, and benchmark remain operational. Avoid advertising alien waves as already implemented.
- Update current development guidance in AGENTS.md and CLAUDE.md, including fixed system order, setup/reset modes, and test command list. Do not rewrite historical design records solely to remove references to former features.
- No new art, SFX, setup libraries, runtime configuration, migration or compatibility shim.

## Verification matrix

| Area | Expected proof | Command / location |
| --- | --- | --- |
| Formatting | Clean Rust style | cargo fmt --all -- --check |
| Lints | No warnings in all targets, including e2e feature | cargo clippy --workspace --all-targets --locked -- -D warnings; cargo clippy -p grus-godot --all-targets --features e2e --locked -- -D warnings |
| Unit/integration | All existing non-AI simulation, lifecycle and bridge tests | cargo test --workspace --locked; cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1 |
| Coverage | >=90% grus-sim non-test line coverage | cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\\.rs$' |
| Standard fixture | Team 1 starts left; Team 2 starts right; 8 stable unit IDs, 2 centers, 18 resource nodes; Team 2 receives no autonomous orders after fixed steps | Fixture / bridge tests and reset_test.tscn |
| Fog | Own area Visible, enemy start Unexplored at boot; reset removes explored fog | Visibility tests plus reset and bridge smokes |
| Basic input/economy | Selection, routing, gather, build, train and UI still work | smoke_test.tscn; economy_smoke_test.tscn |
| Combat/session | Start lock, passive enemy damage, Victory/Result and Restart remain valid | combat_lifecycle_smoke_test.tscn |
| Benchmark | 200-unit scripted load still reaches existing liveness, visibility, frame and marker assertions without the AI | benchmark_200.tscn with GRUS_BENCHMARK_SECONDS=10 in CI |
| Git hygiene | No references to removed AI symbols or mirrored env switch in compiled/runtime files; no scouting CI step | rg across crates, godot/scripts, godot/scenes, .github/workflows, current README / developer guides |

For the Godot smokes: build grus-godot, copy the rebuilt GDExtension into godot/bin (use the host-specific .so/.dylib), run the Godot editor scan once on a clean clone, then execute the four retained headless scenes. CI remains the final integration check, including its packaged-export boot and benchmark where applicable.

## Failure modes and focused safeguards

- **Dangling AI call sites:** Rust compile and Godot scene parsing will reveal missed imports / removed bridge methods. Grep both languages and CI.
- **Fixture drift:** removing the mirrored wrapper must not reorder team/ID insertion or alter geometry. Assert known Team 1/Team 2 spawn anchors plus stable IDs and counts before and after reset.
- **Benchmark false pass/false fail:** do not remove its deterministic scripted workload, its combat-liveness checks or its visibility refresh just because AI disappears. Run the actual scene, not only Rust tests.
- **Weakening fog correctness:** remove only AI-dependent tests; keep human-side visibility and no-information-leak behavior. Enemy structures still exist and stay fogged until seen.
- **Overwide scope:** no rename to Research Center, no centered-city layout, no enemy-wave system and no change to catalogue balance; those belong to HPA-448 and HPA-449.
- **Coverage gate shifts:** deleting ai.rs changes the denominator and may expose under-covered remaining logic. Add targeted assertions only where a genuine non-AI behavior lacks coverage.

## Review checklist / ready to implement

- [ ] Validate file/symbol call sites against the latest main before code edits.
- [ ] Implement steps 1–4 as commits on this **same** PR branch.
- [ ] Run all Rust and Godot checks listed above and address regressions.
- [ ] Verify CI on the final pushed head and update README/PR evidence accordingly.
- [ ] Keep the PR draft for review until implementation and acceptance are complete.

**Not part of this draft:** source implementation, a claim that CI passed, or completion of HPA-445.