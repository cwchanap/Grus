# HPA-474 MVP Delivery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the existing Grus skirmish as a measured, tuned and reproducibly packaged Linux x86_64 MVP without adding new gameplay systems.

**Architecture:** Keep the merged HPA-470–473 ownership boundaries intact. Tune only the existing catalogue/AI/map seams from recorded playtests, evolve the retained 200-unit benchmark into one mixed real-game workload, add one compact help label, and package the existing Godot+Rust application through one release-export script. Performance work is measurement-gated: profile first, then change only the measured owner.

**Tech Stack:** Rust 1.95 / Bevy 0.19.1 / godot-bevy 0.12.0 / godot-rust 0.5.5 / Godot 4.6.2 / GDScript / GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-16-hpa-474-mvp-delivery-design.md`

## Global Constraints

- Exactly one PR for HPA-474. This planning draft becomes the implementation PR.
- Bevy ECS remains the only mutable gameplay model; Godot remains input/presentation/UI.
- Keep the fixed 20 Hz simulation and existing command/system order.
- No new unit/building/faction/map/difficulty/victory system.
- Balance changes are limited to `catalog.rs`, existing AI thresholds in `ai.rs`, and the one authored layout in `fixture.rs` when playtest evidence requires them.
- Keep one retained benchmark scene/reset path; do not add a second performance framework.
- Acceptance performance is a 60-second **release-export** hardware sample at 1920×1080. GitHub Actions remains a software-rendered regression only and must not enforce 60 FPS.
- Keep the >90% `grus-sim` production-line coverage gate and all retained Godot/export/E2E checks green.
- Package only Linux x86_64. No installer, signing/notarization, store/release automation or multi-OS matrix.
- No new image art or SFX work is required.

## Review Focus

1. **Benchmark silently stops being representative:** 200 units exist but gathering/fog/AI/combat are absent. Task 1 tests the benchmark world resources/components and Task 2 asserts runtime visibility/workload progress before sampling.
2. **Benchmark becomes synthetic to preserve 200 survivors:** combat deaths are hidden by health resets/respawns. Task 2 records start/min/end live counts and never respawns during the sample.
3. **Balance change fixes one start and breaks the mirror:** Task 3 records both authored sides and reruns the affected matrix case after any late spatial/policy tuning.
4. **Release archive works only from a dirty developer tree:** Task 5 verifies a fresh checkout, release Rust library staging, exported directory, archive extraction and launch outside the editor.
5. **Average FPS hides tail stalls:** Task 6 records p95/p99/max frame time and requires profiling of a repeatable stall before any optimization commit.

---

### Task 1: Grow the retained benchmark into a mixed 100-vs-100 skirmish workload

**Files:**
- Modify: `crates/grus-sim/src/fixture.rs`
- Modify: `crates/grus-sim/src/fixture/tests.rs`
- Modify: `crates/grus-sim/src/lib.rs`
- Modify: `crates/grus-godot/src/lib.rs`

**Interfaces:**
- Consumes: `seed_skirmish(world, map, fixture)`, `spawn_unit(...)`, `VisibilityMap`, `refresh_visibility(...)`, `AiController::new(TeamId(2))`, `MatchSession { phase: MatchPhase::Playing }`, `apply_player_command(...)`.
- Produces: `pub fn seed_benchmark_skirmish(world: &mut World, map: &mut GridMap, fixture: &MapFixture)` and an updated `GrusBridge.benchmark_start_workload() -> bool` replacing `benchmark_move_all()`.

- [ ] **Step 1: Add a failing fixture regression for the exact mixed roster**

In `crates/grus-sim/src/fixture/tests.rs`, add `benchmark_skirmish_has_100_units_per_side_with_the_locked_mix`.

Assertions after `seed_benchmark_skirmish`:

```text
Team 1: 20 Villager, 30 Spearman, 30 Archer, 20 Cavalry
Team 2: 20 Villager, 30 Spearman, 30 Archer, 20 Cavalry
Total: 200 units
Buildings: 2 completed Town Centers
Standalone resources: 18
TeamEconomy: both teams present
```

Also assert every spawned unit position maps to an in-bounds cell and no unit starts inside an authored blocked terrain/building/resource cell.

- [ ] **Step 2: Run the focused fixture test and verify it fails**

Run:

```bash
cargo test -p grus-sim fixture::tests::benchmark_skirmish_has_100_units_per_side_with_the_locked_mix -- --exact
```

Expected: FAIL because `seed_benchmark_skirmish` does not exist.

- [ ] **Step 3: Implement `seed_benchmark_skirmish(...)` in `fixture.rs`**

Contract:

```rust
pub fn seed_benchmark_skirmish(
    world: &mut World,
    map: &mut GridMap,
    fixture: &MapFixture,
)
```

Implementation decisions:

- call `seed_skirmish` first so Town Centers/resources/economy use the normal constructors;
- keep the eight normal starting Villagers;
- add 96 units per team at deterministic walkable staging cells: 16 Villagers, 30 Spearmen, 30 Archers and 20 Cavalry;
- added Villagers receive `Carry::Empty`, `GatherProgress::default()` and `WorkerTask::Idle` like normal Villagers;
- use stable IDs 9..=104 for Team 1 additions and 105..=200 for Team 2 additions, then replace the unit counter in `IdAllocator` with 201 while preserving the building/resource counters produced by the skirmish seed;
- do not add benchmark-only health, speed, damage or immortality overrides.

Use two mirrored 12×8 walkable staging blocks outside the Town Center/resource rings and outside the central blockers. Keep the cells as one small authored helper beside the benchmark seed rather than adding a placement/search framework.

- [ ] **Step 4: Export the helper from `grus-sim` and run the fixture tests**

Run:

```bash
cargo test -p grus-sim fixture::tests --locked
```

Expected: PASS, including the existing map/start symmetry tests.

- [ ] **Step 5: Add a failing `grus-godot` benchmark-world setup test**

In the existing `#[cfg(test)]` module in `crates/grus-godot/src/lib.rs`, add `benchmark_world_enables_real_session_visibility_and_ai`.

Build a `World`, call the benchmark reset helper, and assert:

```text
MatchPhase == Playing
VisibilityMap exists and has a nonzero/current revision after initial refresh
AiController exists for Team 2
UnitIndex has 200 entries
BuildingIndex has 2 entries
ResourceIndex has 18 entries
```

- [ ] **Step 6: Run the focused bridge test and verify it fails**

Run:

```bash
cargo test -p grus-godot benchmark_world_enables_real_session_visibility_and_ai --lib --locked -- --exact
```

Expected: FAIL because the current reset creates only a visibility-free 200-Villager movement world.

- [ ] **Step 7: Replace the old benchmark reset setup with the mixed skirmish**

Update `reset_benchmark_world(world: &mut World)` in `crates/grus-godot/src/lib.rs` to:

1. clear gameplay state through the existing reset seam;
2. insert the battlefield map;
3. call `seed_benchmark_skirmish`;
4. insert `MatchSession { phase: MatchPhase::Playing }`;
5. insert a fresh `VisibilityMap`, then call `refresh_visibility` once;
6. insert `AiController::new(TeamId(2))`; and
7. leave the normal fixed-step systems unchanged.

Do not create a benchmark-only schedule.

- [ ] **Step 8: Replace `benchmark_move_all()` with `benchmark_start_workload()`**

Signature:

```rust
#[func]
fn benchmark_start_workload(&self) -> bool
```

The function resolves live units by team/kind from `UnitIndex` rather than assuming the old 1..100 / 101..200 ID split. It then applies ordinary gameplay commands:

- first 12 sorted Villagers per team: two Gather assignments per authored safe source (Team 1 resources 1..=6, Team 2 resources 7..=12);
- all 80 military units per team: one AttackMove command toward the mirrored opposing Town Center area.

Return true only when every required setup command is accepted. Keep Team 2's real `AiController` installed; the AI continues making its own later decisions.

- [ ] **Step 9: Run the bridge and simulation regression suites**

Run:

```bash
cargo test -p grus-sim --locked
cargo test -p grus-godot --lib --locked
```

Expected: PASS.

- [ ] **Step 10: Commit the mixed benchmark world**

```bash
git add crates/grus-sim/src/fixture.rs crates/grus-sim/src/fixture/tests.rs crates/grus-sim/src/lib.rs crates/grus-godot/src/lib.rs
git commit -m "test: make benchmark workload representative"
```

### Task 2: Make the benchmark duration-based and keep CI cheap

**Files:**
- Modify: `godot/scripts/benchmark_200.gd`
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: `GrusBridge.reset_benchmark_fixture()`, `GrusBridge.benchmark_start_workload()`, `GrusBridge.visibility_revision()` and live Godot `unit_views` / `building_views` / `resource_views`.
- Produces: one `GRUS_BENCHMARK_200` record with duration, frame-time distribution, start/min/end live units and renderer information.

- [ ] **Step 1: Update the benchmark scene contract before changing the sampling loop**

Replace the old shape assertions in `benchmark_200.gd` with:

```text
200 unique unit views
2 building views
18 resource views
visibility_revision() >= 0
benchmark_start_workload() == true
```

Wait through physics frames until at least one visibility revision advance occurs after the workload starts. This proves moving units/fog are active before timing begins.

- [ ] **Step 2: Replace the 40-frame sample with duration-based sampling**

Add:

```gdscript
const DEFAULT_SAMPLE_SECONDS := 10.0
const WARMUP_SECONDS := 2.0
```

Resolve sample duration from `OS.get_environment("GRUS_BENCHMARK_SECONDS")`; use 10 seconds when unset and require a positive finite value when set.

Collect one real frame-time sample per `process_frame` until elapsed wall time reaches the requested duration. Keep the existing percentile helper.

- [ ] **Step 3: Record live-unit floor and the full timing distribution**

At sample start assert exactly 200 units. During the loop track the minimum number of live unit views; at the end record the current count.

Emit one marker containing at least:

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

Do not fail merely because real combat lowers `min_live_units`; do fail if the sample begins with fewer than 200 or the workload never starts.

- [ ] **Step 4: Run a local short benchmark**

Run the existing Godot import/staging flow, then:

```bash
GRUS_BENCHMARK_SECONDS=3 godot --path godot --rendering-method gl_compatibility res://scenes/benchmark_200.tscn
```

Expected: exactly one `GRUS_BENCHMARK_200 start_units=200 ...` marker and exit 0 (or the already-tolerated post-marker teardown 139).

- [ ] **Step 5: Keep GitHub Actions on the short regression sample**

In `.github/workflows/ci.yml`, set only the existing benchmark step environment:

```yaml
GRUS_BENCHMARK_SECONDS: "10"
```

Keep `LIBGL_ALWAYS_SOFTWARE=1`, Xvfb and the existing 139 handling. Update the grep contract from the old `moved=200` token to `start_units=200`.

Do **not** add a 60 FPS CI assertion and do not make CI run the 60-second acceptance profile.

- [ ] **Step 6: Run the Godot benchmark plus relevant smoke set**

Run:

```bash
godot --headless --path godot res://scenes/reset_test.tscn
godot --headless --path godot res://scenes/scouting_ai_smoke_test.tscn
GRUS_BENCHMARK_SECONDS=3 godot --path godot --rendering-method gl_compatibility res://scenes/benchmark_200.tscn
```

Expected: reset/scouting markers plus the new benchmark marker.

- [ ] **Step 7: Commit the benchmark sampler**

```bash
git add godot/scripts/benchmark_200.gd .github/workflows/ci.yml
git commit -m "perf: measure mixed benchmark workload"
```

### Task 3: Playtest the four required openings and tune only from evidence

**Files:**
- Create: `docs/hpa-474-mvp-evidence.md`
- Conditional modify: `crates/grus-sim/src/catalog.rs`
- Conditional modify: `crates/grus-sim/src/catalog/tests.rs`
- Conditional modify: `crates/grus-sim/src/ai.rs`
- Conditional modify: `crates/grus-sim/src/ai/tests.rs`
- Conditional modify: `crates/grus-sim/src/fixture.rs`
- Conditional modify: `crates/grus-sim/src/fixture/tests.rs`

**Interfaces:**
- Consumes: the shipping skirmish, current HPA-472 catalogue values, current HPA-473 AI policy, and authored mirrored map.
- Produces: four-run directional evidence plus only the smallest catalogue/AI/map tuning proven necessary.

- [ ] **Step 1: Create the evidence record with empty measured fields**

`docs/hpa-474-mvp-evidence.md` starts with sections for:

```text
Revision / environment
Playtest matrix
Tuning log
Release performance
Package verification
Regression results
Remaining limitations
```

The playtest table columns are: start side, opening, duration, result, first expansion use, worker-raid observation, Town Center finish observation, tuning after run.

- [ ] **Step 2: Run the four required playtests**

Cover exactly once before adding optional reruns:

1. left start + early pressure;
2. left start + economy-first/age-up;
3. mirrored/right start + early pressure;
4. mirrored/right start + economy-first/age-up.

Record facts immediately after each run. Do not invent a pass/fail from feel; note the concrete symptom and time.

- [ ] **Step 3: For each demonstrated balance defect, first add the narrow regression that expresses the invariant**

Examples, chosen only when the matching defect actually occurred:

- catalogue change: extend the existing behavioural counter/health/economy test rather than asserting the entire catalogue table;
- AI threshold change: extend the existing pure policy or bounded journey test that owns the decision;
- resource/route move: extend fixture symmetry/reachability/visibility tests;
- worker-raid economics: extend `raid_reduces_income_until_paid_replacement_lands` only if the observed problem is not already covered.

Do **not** add a test that asserts “match duration is 15–25 minutes.”

- [ ] **Step 4: Apply only the proven tuning in the existing owner**

No new tuning/config layer. Keep changes in `catalog.rs`, `ai.rs`, and/or `fixture.rs` only.

- [ ] **Step 5: Run the owning focused tests after each tuning change**

Examples:

```bash
cargo test -p grus-sim catalog::tests --locked
cargo test -p grus-sim ai::tests --locked
cargo test -p grus-sim fixture::tests --locked
```

Expected: PASS.

- [ ] **Step 6: Rerun only a matrix case materially affected by a late tuning change**

Append it to the evidence table as a rerun; do not replace the original observation.

- [ ] **Step 7: Run the full simulation suite and 90% coverage gate**

```bash
cargo test -p grus-sim --locked
cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\.rs$'
```

Expected: PASS and line coverage >= 90%.

- [ ] **Step 8: Commit the evidence-backed balance slice**

```bash
git add docs/hpa-474-mvp-evidence.md crates/grus-sim/src
git commit -m "balance: tune final MVP from playtests"
```

If the four runs justify no balance-code changes, commit only the evidence file with the same intent and explicitly record “no tuning change required.”

### Task 4: Add the minimal in-game controls aid and fix only demonstrated usability defects

**Files:**
- Modify: `godot/scenes/main.tscn`
- Modify: `godot/scripts/smoke_test.gd`
- Conditional modify: the existing Godot script/scene that owns any concrete defect found during Task 3 playtests

**Interfaces:**
- Consumes: existing `SessionPanel` shown in Start/Paused/Result and existing runtime controls.
- Produces: one static `ControlsLabel` inside `SessionPanel`; no new menu/help framework.

- [ ] **Step 1: Add a failing smoke assertion for first-run controls help**

In `godot/scripts/smoke_test.gd`, before the smoke starts the match, assert the main scene contains a visible `HUD/SessionPanel/ControlsLabel` whose text contains the key tokens `Right click`, `A`, `S`, `Esc`, and `Minimap`.

- [ ] **Step 2: Run the bridge smoke and verify it fails**

```bash
godot --headless --path godot res://scenes/smoke_test.tscn
```

Expected: FAIL because `ControlsLabel` does not exist.

- [ ] **Step 3: Add the compact label to `main.tscn`**

Expand the existing session panel only as much as needed. Use static copy covering:

```text
Select: left click / drag / Shift
Command: right click
Attack-move: A then right click
Stop: S
Pause/resume: Esc
Camera: wheel / middle-drag
Minimap: click to recenter
```

Keep the label under the existing session panel so it disappears during Playing with the panel. No new controller state is needed.

- [ ] **Step 4: Run the smoke and visually check 1280×720 layout**

Run:

```bash
godot --path godot
```

Confirm the Start panel does not overlap the economy panel, command panel or minimap, then run:

```bash
godot --headless --path godot res://scenes/smoke_test.tscn
```

Expected: `GRUS_GODOT_SMOKE_OK`.

- [ ] **Step 5: Fix only concrete usability defects already recorded in Task 3**

For each defect, add a focused assertion to its owning existing smoke before the fix, then make the smallest scene/script change. Do not add unrelated polish.

- [ ] **Step 6: Run the complete Godot smoke set**

```bash
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
godot --headless --path godot res://scenes/economy_smoke_test.tscn
godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn
godot --headless --path godot res://scenes/scouting_ai_smoke_test.tscn
```

Expected: all five success markers.

- [ ] **Step 7: Commit the usability slice**

```bash
git add godot/scenes/main.tscn godot/scripts
git commit -m "ui: add compact skirmish controls help"
```

### Task 5: Make one reproducible Linux x86_64 release package

**Files:**
- Create: `scripts/package_linux_x86_64.sh`
- Modify: `README.md`
- Modify: `docs/hpa-474-mvp-evidence.md`
- Conditional modify: `godot/export_presets.cfg` only if the current preset fails to place all required export files under the requested output directory

**Interfaces:**
- Consumes: existing `Linux x86_64` export preset and `godot/grus.gdextension` release library mapping.
- Produces: `dist/grus-linux-x86_64/` plus `dist/grus-linux-x86_64.tar.gz`.

- [ ] **Step 1: Write `scripts/package_linux_x86_64.sh`**

Required behavior:

```bash
#!/usr/bin/env bash
set -euo pipefail
```

Then:

1. resolve repository root from the script location;
2. remove/recreate `dist/grus-linux-x86_64/`;
3. run `cargo build --release -p grus-godot --locked`;
4. copy `target/release/libgrus_godot.so` to `godot/bin/libgrus_godot.so`;
5. run `godot --headless --path godot --editor --quit-after 120`;
6. run `godot --headless --path godot --export-release "Linux x86_64" ../dist/grus-linux-x86_64/Grus.x86_64`;
7. verify the executable exists and is executable;
8. archive the entire directory as `dist/grus-linux-x86_64.tar.gz`.

Do not download toolchains or install system packages from this script.

- [ ] **Step 2: Run the package script from a clean checkout/worktree**

```bash
./scripts/package_linux_x86_64.sh
```

Expected: release Rust build succeeds, Godot import succeeds, export succeeds, directory + tar.gz exist.

- [ ] **Step 3: Extract the archive outside the checkout and launch it**

Example:

```bash
rm -rf /tmp/grus-package-check
mkdir -p /tmp/grus-package-check
tar -xzf dist/grus-linux-x86_64.tar.gz -C /tmp/grus-package-check
/tmp/grus-package-check/grus-linux-x86_64/Grus.x86_64
```

Expected: the app boots without editor/debug intervention and loads the Rust GDExtension.

- [ ] **Step 4: Complete the packaged gameplay journey**

On the extracted build verify:

```text
Start
pause/resume
select + gather
place/build
train
scout/fog/minimap
fight
Victory or Defeat
Restart returns clean Start state
start again briefly
Quit
```

Record result in `docs/hpa-474-mvp-evidence.md`.

- [ ] **Step 5: Record archive hash and exact fresh-checkout commands**

```bash
sha256sum dist/grus-linux-x86_64.tar.gz
```

Store the hash, Godot version and build revision in the evidence record.

- [ ] **Step 6: Update README release instructions**

Add one concise “Release package” section pointing to `./scripts/package_linux_x86_64.sh`, the output archive, and the supported Linux x86_64 boundary. Keep existing development/debug launch instructions.

- [ ] **Step 7: Commit packaging and docs**

```bash
git add scripts/package_linux_x86_64.sh README.md docs/hpa-474-mvp-evidence.md godot/export_presets.cfg
git commit -m "build: package Linux desktop MVP"
```

### Task 6: Run the 60-second release profile, optimize only if measured, and close the MVP evidence

**Files:**
- Modify: `docs/hpa-474-mvp-evidence.md`
- Conditional modify: only the existing file(s) identified by the profiler if a repeatable bottleneck is demonstrated
- Conditional test: the owning Rust/Godot test module for any optimization

**Interfaces:**
- Consumes: the mixed benchmark from Tasks 1–2 and the release build/package path from Task 5.
- Produces: the final 60-second performance record, any profile-driven minimal fix, final regression evidence and remaining limitations.

- [ ] **Step 1: Build a release benchmark entry without changing the shipping main scene permanently**

Reuse the CI pattern: back up `godot/project.godot`, temporarily replace the main scene with `res://scenes/benchmark_200.tscn`, export with `--export-release` to a temporary benchmark executable, then restore `project.godot` even on failure.

Use the release GDExtension staged by the packaging script.

- [ ] **Step 2: Record the reference host before the run**

Capture into the evidence file:

```bash
git rev-parse HEAD
uname -a
lscpu | grep -E 'Architecture|CPU\(s\)|Model name'
godot --version
```

Also record the GPU/renderer/adapter printed by the benchmark marker.

- [ ] **Step 3: Run the acceptance sample for 60 seconds at 1920×1080**

Set:

```bash
GRUS_BENCHMARK_SECONDS=60
```

Run the release benchmark export at 1920×1080 on the reference desktop renderer. Record the full `GRUS_BENCHMARK_200` line in the evidence file.

Required fields: mean FPS, mean ms, p95, p99, max, start/min/end live units, renderer, adapter, vendor and duration.

- [ ] **Step 4: Decide the optimization gate from evidence**

If mean FPS is >= 60 and no repeatable command/frame stall is observed, record “target met; no performance code change” and skip to Step 8.

If the target is missed or a repeatable stall exists, continue.

- [ ] **Step 5: Profile before touching code**

Use the available Godot/Rust profiler appropriate to the host and identify one repeatable owner. Record the measured hotspot and reproduction in the evidence file.

Do not choose a fix from the design's candidate list without profile evidence.

- [ ] **Step 6: Add a focused regression/measurement around the identified owner, then make the smallest fix**

Examples of allowed owners include existing visibility refresh/snapshot, fog/minimap redraw, movement/pathing or view synchronization. Keep the current interface; do not create a second renderer, flow-field system or parallel state owner.

Run the owning test first to show the regression/measurement, implement the fix, and rerun it.

- [ ] **Step 7: Rerun the identical 60-second release sample**

Record before/after numbers side by side. Keep the fix only if the same workload improves the demonstrated bottleneck without breaking correctness.

- [ ] **Step 8: Run the final full regression gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\.rs$'

godot --headless --path godot --editor --quit-after 120
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
godot --headless --path godot res://scenes/economy_smoke_test.tscn
godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn
godot --headless --path godot res://scenes/scouting_ai_smoke_test.tscn
GRUS_BENCHMARK_SECONDS=10 godot --path godot --rendering-method gl_compatibility res://scenes/benchmark_200.tscn
```

Then rerun `./scripts/package_linux_x86_64.sh` once on the final revision and recheck the extracted build boots.

Expected: all Rust/coverage/Godot markers pass and the final package launches.

- [ ] **Step 9: Finish the evidence record and README limitations**

Record:

- final four-run/rerun balance summary;
- changed tuning values, or explicitly none;
- final hardware performance line;
- before/after profile result if applicable;
- final package hash;
- packaged journey result;
- full regression results; and
- honest remaining limitations (including the existing known Linux post-marker teardown-139 issue if still present).

- [ ] **Step 10: Commit final evidence/performance work**

```bash
git add docs/hpa-474-mvp-evidence.md README.md crates godot scripts .github
git commit -m "docs: record Grus MVP delivery evidence"
```

## Self-review result

- **Spec coverage:** playtest matrix, evidence-backed tuning, minimal controls help, mixed 200-unit workload, 60-second release profiling, measured-only optimization, fresh-checkout packaging, packaged journey, full regression and remaining limitations all map to Tasks 1–6.
- **Step scan:** no task preselects final balance numbers or a speculative optimization; those are intentionally evidence gates. Every code task names its owner, interface and verification command.
- **Type consistency:** the only new simulation API is `seed_benchmark_skirmish(...)`; the only renamed bridge benchmark call is `benchmark_start_workload()`. Later tasks use those exact names.
- **Review Focus:** all five failure modes are exercised by Task 1/2 assertions, Task 3 mirrored evidence, Task 5 clean package verification and Task 6 tail-latency profiling.
- **Proportion:** the plan stays implementation-oriented but does not transcribe the Rust/GDScript bodies; benchmark composition and package steps are explicit because they are decisions an implementer cannot infer safely.