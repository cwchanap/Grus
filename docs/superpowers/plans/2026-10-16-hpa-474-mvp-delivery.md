# HPA-474 MVP Delivery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the existing Grus skirmish as a measured, tuned and reproducibly packaged Linux x86_64 MVP without adding new gameplay systems.

**Architecture:** Keep the merged HPA-470–473 ownership boundaries intact. Replace the old benchmark-only 200-Villager contract in place with one mixed real-game workload, expose one test/manual-only mirrored-start launch seam for the required right-side human playtests, tune only existing catalogue/AI/map values from recorded evidence, add one compact controls label, and package the current Godot+Rust application through one release-export script. Performance work stays measurement-gated: profile first, then change only the measured owner.

**Tech Stack:** Rust 1.95 / Bevy 0.19.1 / godot-bevy 0.12.0 / godot-rust 0.5.5 / Godot 4.6.2 / GDScript / GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-16-hpa-474-mvp-delivery-design.md`

## Global Constraints

- Exactly one PR for HPA-474. This planning draft becomes the implementation PR.
- Bevy ECS remains the only mutable gameplay model; Godot remains input/presentation/UI.
- Keep the fixed 20 Hz simulation and existing command/system order.
- No new unit/building/faction/map/difficulty/victory system.
- Balance changes are limited to `catalog.rs`, existing AI thresholds in `ai.rs`, and the one authored layout in `fixture.rs` when playtest evidence requires them.
- Keep one retained benchmark scene/reset path; delete the superseded `UnitSpawn` / `MapFixture::units_200()` fixture instead of preserving a second legacy benchmark seed.
- `GRUS_MIRRORED_START=1` is test/manual-only. It must not become a shipping side-selection menu.
- `README.md` remains the authoritative fixture/controls/benchmark record; update it and `AGENTS.md` in the same tasks that change the benchmark contract.
- Acceptance performance is a 60-second **release-export** hardware sample at 1920×1080. GitHub Actions remains a software-rendered regression only and must not enforce 60 FPS.
- Keep the >90% `grus-sim` production-line coverage gate. On the final revision, the real GitHub Actions `rust-build-lint`, `unit-tests`, and `e2e` jobs must all be green.
- Package only Linux x86_64. No installer, signing/notarization, store/release automation or multi-OS matrix.
- No new image art or SFX work is required.

## Review Focus

1. **Old benchmark contract survives beside the new one:** stale visibility-free/AI-free/session-free tests, comments, `units_200()`, README or AGENTS disagree with runtime. Task 1 replaces them atomically rather than adding compatibility code.
2. **Benchmark looks large but is not representative:** 200 units exist while gathering/fog/AI/combat are absent. Tasks 1–2 assert the exact mixed world and active workload before timing starts.
3. **Mirrored playtest changes shipping defaults:** the test/manual seam accidentally moves normal Team 1 or changes AI behavior. Task 3 locks default launch unchanged and separately tests Team-1-right / Team-2-left authored-plan behavior.
4. **Release archive works only from a dirty developer tree:** Task 5 verifies a fresh checkout, release library staging, archive extraction and launch outside the editor.
5. **A green local checklist hides a red real CI/e2e surface or tail stalls:** Task 6 requires all three final CI jobs green and records p95/p99/max plus the local 60-second hardware result.

---

### Task 1: Replace the old benchmark fixture contract with a mixed 100-vs-100 skirmish

**Files:**
- Modify: `crates/grus-sim/src/fixture.rs`
- Modify: `crates/grus-sim/src/fixture/tests.rs`
- Modify: `crates/grus-sim/src/lib.rs`
- Modify: `crates/grus-godot/src/lib.rs`
- Modify: `README.md`
- Modify: `AGENTS.md`

**Interfaces:**
- Consumes: `seed_skirmish(world, map, fixture)`, `spawn_unit(...)`, `VisibilityMap`, `refresh_visibility(...)`, `AiController::new(TeamId(2))`, `MatchSession { phase: MatchPhase::Playing }`, `apply_player_command(...)`.
- Produces: `pub fn seed_benchmark_skirmish(world: &mut World, map: &mut GridMap, fixture: &MapFixture)` and `GrusBridge.benchmark_start_workload() -> bool`; removes `UnitSpawn`, `MapFixture::units_200()` and `benchmark_move_all()`.

- [ ] **Step 1: Replace the old fixture test with a failing mixed-roster regression**

Delete `two_hundred_unit_fixture_has_stable_unique_walkable_spawns` together with the old helper it specifies. Add `benchmark_skirmish_has_100_units_per_side_with_the_locked_mix`.

After `seed_benchmark_skirmish`, assert:

```text
Team 1: 20 Villager, 30 Spearman, 30 Archer, 20 Cavalry
Team 2: 20 Villager, 30 Spearman, 30 Archer, 20 Cavalry
Total: 200 units
Buildings: 2 completed Town Centers
Standalone resources: 18
TeamEconomy: both teams present
```

Also assert every unit starts in bounds and outside blocked terrain/building/resource cells.

- [ ] **Step 2: Run the focused fixture test and verify it fails**

```bash
cargo test -p grus-sim fixture::tests::benchmark_skirmish_has_100_units_per_side_with_the_locked_mix --locked -- --exact
```

Expected: FAIL because `seed_benchmark_skirmish` does not exist yet.

- [ ] **Step 3: Implement the mixed benchmark seed and remove the obsolete seed**

Add:

```rust
pub fn seed_benchmark_skirmish(
    world: &mut World,
    map: &mut GridMap,
    fixture: &MapFixture,
)
```

Decisions:

- call `seed_skirmish` first;
- retain the eight normal starting Villagers;
- add 96 units per team: 16 Villagers, 30 Spearmen, 30 Archers, 20 Cavalry;
- added Villagers get `Carry::Empty`, `GatherProgress::default()` and `WorkerTask::Idle`;
- stable added IDs are 9..=104 for Team 1 and 105..=200 for Team 2; update the allocator's next unit ID to 201 while preserving building/resource counters;
- use one pair of mirrored deterministic staging blocks that are verified walkable by the test;
- no benchmark-only speed, health, damage, immortality or catalogue override.

Delete `UnitSpawn` and `MapFixture::units_200()` in this same step.

- [ ] **Step 4: Run the fixture module**

```bash
cargo test -p grus-sim fixture::tests --locked
```

Expected: PASS, including authored symmetry/reachability tests.

- [ ] **Step 5: Rewrite the three bridge tests that currently lock the old benchmark behavior**

In `crates/grus-godot/src/lib.rs`, change the old contracts in the same commit as `reset_benchmark_world`:

- `runtime_setup_and_resets_manage_the_visibility_resource`: after benchmark reset, require `VisibilityMap` present with revision >= 1 rather than absent;
- `the_team_two_ai_controller_follows_setup_reset_and_test_disable`: after benchmark reset, require a Team-2 `AiController` rather than no controller;
- replace `benchmark_world_moves_units_without_a_session` with `benchmark_world_runs_playing_mixed_workload`: require `MatchPhase::Playing`, 200 units / 2 buildings / 18 resources, and accepted workload commands resolved from live team/kind membership rather than IDs 1..=100.

Keep the unrelated assertion that a completely missing session reads as Playing where that compatibility seam is tested elsewhere; do not preserve “benchmark must have no session” as a fixture requirement.

- [ ] **Step 6: Run the focused bridge tests and verify the old reset fails them**

```bash
cargo test -p grus-godot runtime_setup_and_resets_manage_the_visibility_resource --lib --locked -- --exact
cargo test -p grus-godot the_team_two_ai_controller_follows_setup_reset_and_test_disable --lib --locked -- --exact
cargo test -p grus-godot benchmark_world_runs_playing_mixed_workload --lib --locked -- --exact
```

Expected: the rewritten benchmark assertions fail until Steps 7–8 land.

- [ ] **Step 7: Replace `reset_benchmark_world` with the mixed Playing/fog/AI contract**

After the existing clear seam:

1. construct the battlefield map;
2. call `seed_benchmark_skirmish`;
3. insert the map resource;
4. insert `MatchSession { phase: MatchPhase::Playing }`;
5. insert a fresh `VisibilityMap` and call the normal initial refresh;
6. insert `AiController::new(TeamId(2))`;
7. leave the normal fixed-step schedule unchanged.

Update nearby comments in `clear_gameplay_world`, reset helpers and tests that currently say the benchmark is visibility-free, AI-free or session-free.

- [ ] **Step 8: Replace `benchmark_move_all()` with `benchmark_start_workload()`**

```rust
#[func]
fn benchmark_start_workload(&self) -> bool
```

Resolve live units from `UnitIndex` by team/kind; never depend on the old numeric half split.

Apply ordinary commands:

- first 12 sorted Villagers per team: two Gather assignments per authored safe source (Team 1 resources 1..=6; Team 2 resources 7..=12);
- all 80 military units per team: AttackMove toward the opposing Town Center area.

Return true only when all required setup commands are accepted. Keep Team 2's real AI installed for later ticks.

- [ ] **Step 9: Update README and AGENTS with the contract change**

`README.md`:

- change Benchmark reset from “200-villager fixture” to the mixed 100-vs-100 Playing/fog/AI workload;
- keep the HPA-470 llvmpipe 5.2 FPS table as explicitly **historical HPA-470 baseline**, not the current benchmark composition;
- state that HPA-474 current evidence will live in `docs/hpa-474-mvp-evidence.md`.

`AGENTS.md`:

- replace the “benchmark = 200-villager fixture” reset invariant with the mixed `Playing + VisibilityMap + Team-2 AI` contract;
- replace/add the bridge benchmark call name with `benchmark_start_workload`.

- [ ] **Step 10: Run the simulation/bridge suites**

```bash
cargo test -p grus-sim --locked
cargo test -p grus-godot --lib --locked
```

Expected: PASS.

- [ ] **Step 11: Commit the atomic benchmark-contract replacement**

```bash
git add crates/grus-sim/src/fixture.rs crates/grus-sim/src/fixture/tests.rs crates/grus-sim/src/lib.rs crates/grus-godot/src/lib.rs README.md AGENTS.md
git commit -m "test: make benchmark workload representative"
```

### Task 2: Make the retained benchmark duration-based and keep CI cheap

**Files:**
- Modify: `godot/scripts/benchmark_200.gd`
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`

**Interfaces:**
- Consumes: `GrusBridge.reset_benchmark_fixture()`, `GrusBridge.benchmark_start_workload()`, `GrusBridge.visibility_revision()` and live unit/building/resource views.
- Produces: one `GRUS_BENCHMARK_200` record with duration, frame-time distribution, start/min/end live units and renderer information.

- [ ] **Step 1: Update the scene's starting-shape/workload contract**

Before timing, require:

```text
200 unique unit views
2 building views
18 resource views
visibility_revision() >= 0
benchmark_start_workload() == true
```

After workload start, wait for at least one visibility revision advance before sampling so movement/fog are proven active.

- [ ] **Step 2: Replace the fixed 40-frame sample with a duration**

Add:

```gdscript
const DEFAULT_SAMPLE_SECONDS := 10.0
const WARMUP_SECONDS := 2.0
```

Read `GRUS_BENCHMARK_SECONDS`; default to 10 seconds and fail on a non-positive/non-finite override. Collect one frame-time sample per `process_frame` until real elapsed time reaches the requested duration.

- [ ] **Step 3: Emit the final benchmark marker fields**

Track live unit count throughout real combat. Emit at least:

```text
start_units=200
min_live_units=<n>
end_units=<n>
duration_s=<seconds>
frames=<n>
mean_ms=<n>
p50_ms=<n>
p95_ms=<n>
p99_ms=<n>
max_ms=<n>
mean_fps=<n>
visibility_revision=<n>
renderer=<...>
adapter=<...>
vendor=<...>
display=<...>
```

Do not respawn/reset health and do not fail merely because combat lowers `min_live_units` after a valid 200-unit start.

- [ ] **Step 4: Run a short local benchmark**

```bash
GRUS_BENCHMARK_SECONDS=3 godot --path godot --rendering-method gl_compatibility res://scenes/benchmark_200.tscn
```

Expected: exactly one `GRUS_BENCHMARK_200 start_units=200 ...` marker and exit 0, or the already-tolerated post-marker teardown 139.

- [ ] **Step 5: Keep CI on a 10-second software-rendered regression**

In only the existing benchmark step add:

```yaml
GRUS_BENCHMARK_SECONDS: "10"
```

Keep Xvfb, `LIBGL_ALWAYS_SOFTWARE=1`, timeout/139 handling; change the marker grep from `moved=200` to `start_units=200`. Do not add a 60 FPS CI gate.

- [ ] **Step 6: Update README's current sampling description**

Document 10-second CI regression vs `GRUS_BENCHMARK_SECONDS=60` hardware acceptance, while leaving the old HPA-470 table clearly labeled historical.

- [ ] **Step 7: Run benchmark + owning smokes**

```bash
godot --headless --path godot res://scenes/reset_test.tscn
godot --headless --path godot res://scenes/scouting_ai_smoke_test.tscn
GRUS_BENCHMARK_SECONDS=3 godot --path godot --rendering-method gl_compatibility res://scenes/benchmark_200.tscn
```

Expected: both smoke markers and the new benchmark marker.

- [ ] **Step 8: Commit**

```bash
git add godot/scripts/benchmark_200.gd .github/workflows/ci.yml README.md
git commit -m "perf: measure mixed benchmark workload"
```

### Task 3: Add the manual mirrored-start seam, run four openings, then tune only from evidence

**Files:**
- Modify: `crates/grus-sim/src/fixture.rs`
- Modify: `crates/grus-sim/src/fixture/tests.rs`
- Modify: `crates/grus-sim/src/ai.rs`
- Modify: `crates/grus-sim/src/ai/tests.rs`
- Modify: `crates/grus-godot/src/lib.rs`
- Create: `docs/hpa-474-mvp-evidence.md`
- Conditional modify: `crates/grus-sim/src/catalog.rs`, `catalog/tests.rs`, `ai.rs`, `ai/tests.rs`, `fixture.rs`, `fixture/tests.rs` only for evidence-backed tuning

**Interfaces:**
- Consumes: normal shipping Team-1-left / Team-2-right seed, the authored mirrored map/plans, current HPA-472 catalogue and HPA-473 AI.
- Produces: `GRUS_MIRRORED_START=1` test/manual runtime setup plus four-run directional evidence and only the smallest proven tuning.

- [ ] **Step 1: Add failing tests for the exact mirrored setup without changing defaults**

Add focused assertions for:

```text
default: Team 1 Town Center anchor == left authored start
         Team 2 Town Center anchor == right authored start
mirrored: Team 1 Town Center anchor == right authored start
          Team 2 Town Center anchor == left authored start
```

Add an AI policy regression that a Team-2 controller configured for the left authored side still issues `issuer: TeamId(2)` commands but reads the left-side build/scout anchors.

- [ ] **Step 2: Implement the smallest authored-side seam**

Keep `AiController::new(team)` as the normal constructor. Add only the state/constructor needed for the harness, e.g.:

```rust
pub fn new_for_authored_side(team: TeamId, authored_side: TeamId) -> Self
```

Store `authored_side` in the controller and use `MapFixture::team_plan(controller.authored_side)` where the policy currently derives the plan from `controller.team`. Normal `new(team)` sets `authored_side = team`, so shipping behavior is unchanged.

Add a narrow mirrored fixture seed/helper that assigns Team 1 entities/economy to the authored right start and Team 2 to the authored left start while keeping resource IDs/map contents unchanged. Do not add a generic scenario abstraction.

- [ ] **Step 3: Wire only the normal runtime setup/reset to the manual environment switch**

Add one internal helper:

```rust
fn mirrored_start_enabled() -> bool
```

It returns true only when `GRUS_MIRRORED_START=1`.

`reset_fixture_world` / first setup choose:

- default: current `seed_skirmish` + `AiController::new(TeamId(2))`;
- mirrored: mirrored seed + `AiController::new_for_authored_side(TeamId(2), TeamId(1))`.

Team 1 remains the bridge/local human team. No button/menu/state is added. Restart rereads the same environment flag.

Keep direct environment parsing outside the pure sim; tests may exercise the bool-selected setup helper directly so parallel Rust tests do not mutate process-global env vars.

- [ ] **Step 4: Run the mirrored/default focused tests**

```bash
cargo test -p grus-sim fixture::tests --locked
cargo test -p grus-sim ai::tests --locked
cargo test -p grus-godot mirrored --lib --locked
```

Expected: default and mirrored contracts pass.

- [ ] **Step 5: Create the evidence record**

`docs/hpa-474-mvp-evidence.md` starts with:

```text
Revision / environment
Playtest matrix
Tuning log
Release performance
Package verification
Regression / final CI
Remaining limitations
```

Playtest columns: start side, opening, duration, result, first expansion use, worker-raid observation, Town Center finish observation, tuning after run.

- [ ] **Step 6: Run the four required human playtests**

Default launch:

```bash
godot --path godot
```

Run left early-pressure and left economy-first/age-up.

Mirrored launch:

```bash
GRUS_MIRRORED_START=1 godot --path godot
```

Run right early-pressure and right economy-first/age-up.

Record facts immediately after each run. Four runs are directional evidence, not a competitive-balance claim.

- [ ] **Step 7: For each demonstrated balance defect, write the narrow owning regression first**

Examples only when the matching symptom occurred:

- catalogue relationship → existing catalogue/combat/economy behavioural test;
- AI threshold → existing pure policy/bounded journey test;
- resource/route position → fixture symmetry/reachability/visibility test;
- worker-raid economics → extend `raid_reduces_income_until_paid_replacement_lands` only if current coverage does not express the observed issue.

Never test “match duration == 15–25 minutes.”

- [ ] **Step 8: Apply only proven tuning in existing owners**

No tuning/config framework. Modify only `catalog.rs`, current AI threshold constants, and/or `fixture.rs` map/resource/route values justified by the playtest record.

- [ ] **Step 9: Run focused tests after each tuning change and rerun only materially affected matrix cases**

```bash
cargo test -p grus-sim catalog::tests --locked
cargo test -p grus-sim ai::tests --locked
cargo test -p grus-sim fixture::tests --locked
```

Append reruns to the evidence table; preserve the original observation.

- [ ] **Step 10: Run the full simulation + coverage gate**

```bash
cargo test -p grus-sim --locked
cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\.rs$'
```

Expected: PASS / >= 90% lines.

- [ ] **Step 11: Commit the mirrored harness + measured balance evidence**

```bash
git add crates/grus-sim/src crates/grus-godot/src/lib.rs docs/hpa-474-mvp-evidence.md
git commit -m "balance: tune final MVP from playtests"
```

If no tuning is needed, commit the mirrored test harness + evidence and explicitly record “no balance-code change required.”

### Task 4: Add the minimal in-game controls aid and only demonstrated usability fixes

**Files:**
- Modify: `godot/scenes/main.tscn`
- Modify: `godot/scripts/smoke_test.gd`
- Conditional modify: only the existing scene/script that owns a concrete Task-3 playtest defect

**Interfaces:**
- Consumes: existing `SessionPanel` and runtime controls.
- Produces: one static `HUD/SessionPanel/ControlsLabel`; no help/menu framework.

- [ ] **Step 1: Add a failing smoke assertion**

Before Start, require visible `HUD/SessionPanel/ControlsLabel` containing `Right click`, `A`, `S`, `Esc`, and `Minimap`.

- [ ] **Step 2: Run the bridge smoke and verify it fails**

```bash
godot --headless --path godot res://scenes/smoke_test.tscn
```

- [ ] **Step 3: Add the compact static label**

Copy:

```text
Select: left click / drag / Shift
Command: right click
Attack-move: A then right click
Stop: S
Pause/resume: Esc
Camera: wheel / middle-drag
Minimap: click to recenter
```

Keep it inside the existing session panel so it naturally hides during Playing. No controller state.

- [ ] **Step 4: Verify 1280×720 layout + smoke**

Visually check the Start panel does not overlap economy/command/minimap, then run the smoke and require `GRUS_GODOT_SMOKE_OK`.

- [ ] **Step 5: Fix only concrete usability defects already recorded in Task 3**

For each defect, add a focused assertion to its existing owning smoke before the smallest fix. No unrelated polish.

- [ ] **Step 6: Run all five Godot smokes**

```bash
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
godot --headless --path godot res://scenes/economy_smoke_test.tscn
godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn
godot --headless --path godot res://scenes/scouting_ai_smoke_test.tscn
```

- [ ] **Step 7: Commit**

```bash
git add godot/scenes/main.tscn godot/scripts
git commit -m "ui: add compact skirmish controls help"
```

### Task 5: Produce one reproducible Linux x86_64 release package

**Files:**
- Create: `scripts/package_linux_x86_64.sh`
- Modify: `README.md`
- Modify: `docs/hpa-474-mvp-evidence.md`
- Conditional modify: `godot/export_presets.cfg` only if the existing preset cannot place all export files under the requested directory

**Interfaces:**
- Consumes: existing `Linux x86_64` export preset and release GDExtension mapping.
- Produces: `dist/grus-linux-x86_64/` and `dist/grus-linux-x86_64.tar.gz`.

- [ ] **Step 1: Write `scripts/package_linux_x86_64.sh`**

Start with:

```bash
#!/usr/bin/env bash
set -euo pipefail
```

Then:

1. resolve repository root from script path;
2. remove/recreate `dist/grus-linux-x86_64/`;
3. `cargo build --release -p grus-godot --locked`;
4. stage `target/release/libgrus_godot.so` to `godot/bin/libgrus_godot.so`;
5. `godot --headless --path godot --editor --quit-after 120`;
6. `godot --headless --path godot --export-release "Linux x86_64" ../dist/grus-linux-x86_64/Grus.x86_64`;
7. verify executable exists/is executable;
8. archive the complete directory to `dist/grus-linux-x86_64.tar.gz`.

Do not download/install toolchains from the script.

- [ ] **Step 2: Run it from a clean checkout/worktree**

```bash
./scripts/package_linux_x86_64.sh
```

Expected: release build, import, export and archive all succeed.

- [ ] **Step 3: Extract outside the checkout and launch**

```bash
rm -rf /tmp/grus-package-check
mkdir -p /tmp/grus-package-check
tar -xzf dist/grus-linux-x86_64.tar.gz -C /tmp/grus-package-check
/tmp/grus-package-check/grus-linux-x86_64/Grus.x86_64
```

Expected: boots outside editor and loads Rust GDExtension.

- [ ] **Step 4: Complete the packaged journey**

Verify Start → pause/resume → gather/build/train → scout/fog/minimap → fight → Result → Restart → brief second Start → Quit. Record result.

- [ ] **Step 5: Record archive SHA and exact commands**

```bash
sha256sum dist/grus-linux-x86_64.tar.gz
```

Store hash, Godot version and revision in evidence.

- [ ] **Step 6: Update concise README release instructions**

Point to the packaging script, archive path and Linux x86_64 support boundary. Keep existing debug/dev instructions.

- [ ] **Step 7: Commit**

```bash
git add scripts/package_linux_x86_64.sh README.md docs/hpa-474-mvp-evidence.md godot/export_presets.cfg
git commit -m "build: package Linux desktop MVP"
```

### Task 6: Run the release profile, optimize only if measured, and close evidence on the actual CI surface

**Files:**
- Modify: `docs/hpa-474-mvp-evidence.md`
- Conditional modify: only existing file(s) identified by profiling
- Conditional test: owning Rust/Godot test for any optimization

**Interfaces:**
- Consumes: Tasks 1–2 mixed benchmark and Task 5 release path.
- Produces: final 60-second performance record, optional profile-driven fix, final three-job CI evidence, package verification and limitations.

- [ ] **Step 1: Build a release benchmark entry without permanently changing the main scene**

Reuse CI's safe pattern: back up `godot/project.godot`, temporarily swap `res://scenes/main.tscn` to `res://scenes/benchmark_200.tscn`, `--export-release` a temporary benchmark executable, and restore the file on success/failure. Use the release GDExtension staged by Task 5.

- [ ] **Step 2: Record reference host/revision**

```bash
git rev-parse HEAD
uname -a
lscpu | grep -E 'Architecture|CPU\(s\)|Model name'
godot --version
```

GPU/renderer/adapter come from the benchmark marker.

- [ ] **Step 3: Run the 60-second 1920×1080 release sample**

Set `GRUS_BENCHMARK_SECONDS=60`, run the release benchmark export on the reference desktop renderer, and record the full marker.

Required: duration, mean FPS/ms, p95, p99, max, start/min/end units, renderer/adapter/vendor.

- [ ] **Step 4: Apply the optimization gate**

If mean FPS >= 60 and no repeatable command/frame stall appears: record “target met; no performance code change” and skip to Step 8.

Otherwise continue.

- [ ] **Step 5: Profile before touching code**

Use the host's available Godot/Rust profiler. Record one repeatable hotspot/reproduction. Do not pick a fix from speculation.

- [ ] **Step 6: Add a focused regression/measurement, then make the smallest fix in the measured owner**

Keep existing interfaces. No flow fields, second renderer, parallel state owner or broad refactor.

- [ ] **Step 7: Rerun the identical 60-second release sample**

Record before/after side by side and keep the fix only if the same workload improves the demonstrated bottleneck without correctness regressions.

- [ ] **Step 8: Put the exact final revision through the real GitHub Actions gate**

Push the final candidate SHA and require these three workflow jobs on **that SHA** to pass:

```text
rust-build-lint
unit-tests
e2e
```

This is the final automated gate instead of duplicating an incomplete local command list. It necessarily covers the e2e-feature Clippy/unit tests, export smoke, exported-game Bevy E2E, Godot smokes, coverage and short software benchmark already owned by `.github/workflows/ci.yml`.

Record the CI run URL and exact SHA in `docs/hpa-474-mvp-evidence.md`.

- [ ] **Step 9: Rerun only the local acceptance checks CI cannot prove**

On the exact final SHA:

1. ensure the recorded 60-second hardware release sample is from that SHA (rerun if any performance-relevant code changed afterward);
2. run `./scripts/package_linux_x86_64.sh` from a clean checkout/worktree;
3. extract elsewhere and repeat the packaged journey;
4. record the final archive SHA256.

- [ ] **Step 10: Finish evidence and limitations**

Record:

- final four-run/rerun balance summary;
- tuning values changed, or explicitly none;
- final hardware benchmark marker;
- profile before/after if applicable;
- final CI SHA/run and all-three-job status;
- package hash/journey;
- honest remaining limitations, including the known Linux post-marker teardown-139 issue if still present.

- [ ] **Step 11: Commit final evidence/performance work**

```bash
git add docs/hpa-474-mvp-evidence.md README.md crates godot scripts .github AGENTS.md
git commit -m "docs: record Grus MVP delivery evidence"
```

If Step 11 creates a new SHA after the recorded CI run, repeat Step 8 for that exact final SHA before marking the PR ready.

## Self-review result

- **Spec coverage:** benchmark contract replacement, mirrored launch, four playtests, evidence-backed tuning, controls help, 60-second release profiling, profile-only optimization, clean packaging, packaged journey, actual CI surface and limitations all map to Tasks 1–6.
- **Step scan:** the plan no longer defers the mirrored launch mechanism or old benchmark callers/docs to implementation-time discovery. No task preselects balance numbers or speculative optimization.
- **Type consistency:** new retained APIs are `seed_benchmark_skirmish(...)`, `benchmark_start_workload()`, the manual-only mirrored fixture/setup seam, and an `AiController` authored-side constructor; old `units_200()` / `benchmark_move_all()` are deleted.
- **Review Focus:** old-contract drift, workload realism, mirrored-default safety, clean packaging and CI/tail-latency gaps each have an owning test/gate.
- **Proportion:** decisions that cannot be inferred safely are explicit; no generic benchmark, scenario, tuning, help or release framework is introduced.