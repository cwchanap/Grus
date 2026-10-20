# HPA-474 MVP Delivery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the existing Grus skirmish as a measured, tuned and reproducibly packaged Linux x86_64 MVP without adding new gameplay systems.

**Architecture:** Keep the merged HPA-470–473 ownership boundaries intact. Replace the old 200-Villager benchmark contract in place with one session-free mixed workload owned by `grus-sim`, reuse one normal match-seed path for setup/restart/benchmark, expose only one manual mirrored-start switch for playtest evidence, tune only existing catalogue/AI/map values from observed runs, add one compact controls label, and package the application through one release-export script. Measure performance early and again on the final SHA; optimize only a measured owner.

**Tech Stack:** Rust 1.95 / Bevy 0.19.1 / godot-bevy 0.12.0 / godot-rust 0.5.5 / Godot 4.6.2 / GDScript / GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-16-hpa-474-mvp-delivery-design.md`

## Global Constraints

- Exactly one PR for HPA-474. This draft becomes the implementation PR.
- Bevy ECS remains the only mutable gameplay model; Godot remains input/presentation/UI.
- Keep the fixed 20 Hz simulation and existing command/system order.
- No new unit/building/faction/map/difficulty/victory system.
- Balance changes stay in `catalog.rs`, existing `ai.rs` policy thresholds, and the one authored `fixture.rs` layout only when playtest evidence requires them.
- Keep one retained benchmark scene/reset path; delete `UnitSpawn`, `MapFixture::units_200()` and `benchmark_move_all()` instead of preserving a legacy benchmark path.
- The benchmark stays **session-free**. Missing `MatchSession` already means Playing and intentionally prevents `resolve_result` from freezing the load test.
- Benchmark workload setup belongs in `grus-sim` and uses `apply_player_command`; do not add `benchmark_start_workload()` to the bridge.
- `GRUS_MIRRORED_START=1` is manual/test-only. It must not become a shipping side-selection menu.
- `docs/hpa-474-mvp-evidence.md` is the single home for current HPA-474 measurements. README describes the contract/commands and links there.
- Acceptance performance is a 60-second **release-export** hardware sample at 1920×1080. CI remains a software-rendered regression and must not enforce 60 FPS.
- Keep the >90% `grus-sim` production-line coverage gate. On the exact final SHA, `rust-build-lint`, `unit-tests`, and `e2e` must all be green.
- Package only Linux x86_64. No installer, signing/notarization, store/release automation or multi-OS matrix.
- No new image art or SFX work is required.

## Review Focus

1. **Benchmark ends the match and inflates FPS:** never insert a benchmark `MatchSession`; attack toward midfield, not Town Centers; keep a liveness checkpoint.
2. **200-unit headline hides a much smaller measured load:** report start/min/mean/end live units and do not respawn dead units.
3. **Benchmark gameplay leaks into the bridge:** IDs/composition/orders are built and accepted in pure `grus-sim`; the bridge only resets/reads.
4. **Mirrored evidence changes shipping identity:** Team 1 remains the local human, Team 2 remains the AI; only authored start/AI plan side swap.
5. **Final task discovers there is no hardware host:** Task 0 records the real Linux x86_64 GPU host before implementation.
6. **Release export leaves `project.godot` dirty on failure:** package script owns main-scene override with a restoring `trap`; `dist/` is ignored.
7. **Author self-test is mistaken for first-time usability:** packaged fresh-player result is recorded, or the missing evidence is listed as a limitation.
8. **Current benchmark numbers drift between docs:** performance numbers live only in the evidence file.

---

### Task 0: Lock the real Linux x86_64 reference host before code work

**Files:**
- Create: `docs/hpa-474-mvp-evidence.md`

- [ ] **Step 1: Create the evidence skeleton**

Create sections for:

```text
Reference host / revision
Playtest matrix
Restart + lifecycle outcomes
Tuning log
Early release performance
Final release performance
Package verification
Fresh-player usability
Asset attribution
Final CI
Remaining limitations
```

- [ ] **Step 2: Record the actual hardware-rendered Linux x86_64 host**

Record a stable machine label plus:

```bash
uname -a
lscpu | grep -E 'Architecture|CPU\(s\)|Model name'
godot --version
```

Also record the GPU/renderer/adapter once the benchmark prints it.

HPA-470 recorded Linux x86_64 as the supported export target but only a software-rendered llvmpipe CI machine. Do **not** treat that CI runner as the hardware reference.

- [ ] **Step 3: Stop early if the required host does not exist**

If no Linux x86_64 GPU host is available, record that blocking acceptance dependency in the evidence file. Do not silently change the ticket to macOS and do not postpone host discovery until Task 6.

- [ ] **Step 4: Commit the preflight record**

```bash
git add docs/hpa-474-mvp-evidence.md
git commit -m "docs: start HPA-474 delivery evidence"
```

### Task 1: Replace the old benchmark fixture with one shared-seed mixed workload

**Files:**
- Modify: `crates/grus-sim/src/fixture.rs`
- Modify: `crates/grus-sim/src/fixture/tests.rs`
- Modify: `crates/grus-sim/src/lib.rs`
- Modify: `crates/grus-sim/src/ai.rs`
- Modify: `crates/grus-sim/src/ai/tests.rs`
- Modify: `crates/grus-godot/src/lib.rs`
- Modify: `README.md`
- Modify: `AGENTS.md`

**Produces:**
- one shared bridge-internal `seed_match_world(world, mirrored)` seam;
- a pure-sim benchmark workload helper that extends a seeded match and applies accepted initial commands;
- removal of `UnitSpawn`, `MapFixture::units_200()` and `benchmark_move_all()`.

- [ ] **Step 1: Add failing simulation tests for the mixed roster and workload**

Replace `two_hundred_unit_fixture_has_stable_unique_walkable_spawns` with a test that starts from the normal skirmish, extends it with the benchmark workload and asserts:

```text
Team 1: 20 Villager, 30 Spearman, 30 Archer, 20 Cavalry
Team 2: 20 Villager, 30 Spearman, 30 Archer, 20 Cavalry
Total: 200 units
Buildings: 2 completed Town Centers
Authored standalone resources: 18
TeamEconomy: both teams present
All added units: in bounds + not on blocked authored cells
Initial Gather/AttackMove commands: accepted
```

Also add/extend a fixture test for default vs mirrored authored starts:

```text
default: Team 1 left, Team 2 right
mirrored: Team 1 right, Team 2 left
```

- [ ] **Step 2: Add the narrow AI authored-plan-side seam**

Keep `AiController::new(team)` unchanged for normal use. Add only one authored-plan selector field/constructor for the mirrored harness. `step_ai` uses that field for `MapFixture::team_plan(...)` while commands still use `controller.team` as issuer.

Test that a Team-2 controller configured for the left authored plan still emits Team-2 commands against left-side anchors/routes.

- [ ] **Step 3: Make normal skirmish seeding selectable without changing the default API**

Keep existing `seed_skirmish(world, map, fixture)` as the default non-mirrored entry point used by current tests.

Add one narrow bool-selected helper underneath/alongside it so the bridge can seed Team 1 at the authored right start and Team 2 at the authored left start. Do not add a generic scenario/faction abstraction.

- [ ] **Step 4: Implement the pure benchmark workload helper**

The helper assumes a normal non-mirrored skirmish is already seeded.

Add 96 units per team:

- 16 Villagers;
- 30 Spearmen;
- 30 Archers;
- 20 Cavalry.

For every added unit:

1. obtain the ID from `world.resource_mut::<IdAllocator>().allocate_unit()`;
2. use normal `unit_spec(kind).speed` and `spawn_unit(...)`;
3. insert the same `(Carry::Empty, GatherProgress::default(), WorkerTask::Idle)` bundle production gives trained units.

Use deterministic mirrored walkable staging cells. Do not hand-code `9..=104` / `105..=200` or manually set the allocator to 201.

After spawning, resolve unit membership by live team/kind and apply ordinary commands directly through `apply_player_command`:

- first 12 sorted Villagers per side: two Gather assignments per authored safe source;
- all military: AttackMove to one shared, authored walkable midfield corridor cell (for example the retained northern corridor target), **not** either Town Center.

Assert each deterministic setup `CommandResult` has no top-level reject and no rejected units.

Delete `UnitSpawn` / `MapFixture::units_200()` in this same step.

- [ ] **Step 5: Extract one bridge-internal normal match seed path**

Add conceptually:

```text
seed_match_world(world, mirrored)
```

It seeds map/skirmish, inserts the map, inserts/refreshes visibility, and inserts Team-2 AI with the authored plan side matching its physical start. It does **not** insert a session.

Use it from:

- `setup_fixture`: call helper, then insert `MatchSession { Start }`;
- `reset_fixture_world`: clear, call helper with the launch `mirrored` boolean (captured once from `GRUS_MIRRORED_START` at app build into `MirroredStart`, so a mirrored launch stays mirrored across every restart in the same process and cannot drift mid-session), then insert `MatchSession { Start }`; a regression test pins the mirrored layout across the reset seam;
- `reset_benchmark_world`: clear, call helper with `false`, extend with the pure benchmark workload, and insert **no session**.

Keep e2e selector reattachment in the existing normal reset location/behavior.

- [ ] **Step 6: Rewrite old benchmark bridge tests atomically**

Update the current locked contracts:

- benchmark reset now has `VisibilityMap` with a refreshed revision;
- benchmark reset now has Team-2 `AiController`;
- benchmark reset still has **no `MatchSession`** and therefore reads as Playing;
- benchmark world has 200 mixed units / 2 starting Town Centers / 18 authored sources;
- remove the numeric-ID-half movement test and `benchmark_move_all()` test path.

Update stale comments in reset/clear/session tests. Preserve the independent rule that missing session means Playing.

- [ ] **Step 7: Update README and AGENTS with the new contract**

README:

- benchmark reset = session-free mixed 100-vs-100 skirmish with fog + Team-2 AI;
- current HPA-474 measurements live in `docs/hpa-474-mvp-evidence.md`;
- remove the old HPA-470 numeric benchmark table rather than carrying two homes for benchmark numbers; PR #1/git history remain the historical record.

AGENTS:

- replace the “200-villager visibility-free benchmark” invariant;
- state benchmark reset is mixed + visibility + AI + **session-free**;
- remove `benchmark_move_all` from bridge command documentation.

- [ ] **Step 8: Verify Task 1**

```bash
cargo test -p grus-sim fixture::tests --locked
cargo test -p grus-sim ai::tests --locked
cargo test -p grus-sim --locked
cargo test -p grus-godot --lib --locked
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/grus-sim crates/grus-godot README.md AGENTS.md
git commit -m "test: make benchmark workload representative"
```

### Task 2: Make benchmark sampling duration-based, live, and cheap in CI

**Files:**
- Modify: `godot/scripts/benchmark_200.gd`
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`

- [ ] **Step 1: Replace the old start-shape assumptions**

After `reset_benchmark_fixture()`:

- wait for exactly 200 unique unit views;
- require `building_views >= 2` (AI may place a foundation before all views attach);
- require at least the authored 18 resource views;
- capture the current visibility revision and wait until it advances before timing.

There is no `benchmark_start_workload()` call; Task 1 already seeded and accepted the workload.

- [ ] **Step 2: Convert fixed-frame timing to elapsed-time timing**

Use:

```gdscript
const DEFAULT_SAMPLE_SECONDS := 10.0
const WARMUP_SECONDS := 2.0
```

Read `GRUS_BENCHMARK_SECONDS`; default to 10 and reject non-positive/non-finite values. Collect one frame-time sample per `process_frame` until real elapsed time reaches the requested duration.

- [ ] **Step 3: Preserve and generalize the liveness checkpoint**

Do not delete the current movement checkpoint.

At a fixed interval:

- compare current live unit positions against the previous checkpoint for IDs that still exist;
- drain/count benchmark combat events through the existing bridge event read;
- fail only if the window shows neither meaningful unit movement nor combat activity.

This prevents a visually rendering but simulation-dead sample from passing.

- [ ] **Step 4: Record the actual live-unit load**

Record current live-unit and visible-in-tree unit counts once per liveness window — a per-frame `unit_views` group scan would land inside the measured frame times the benchmark exists to record. Live units are the `unit_views` group membership (fog-hidden enemies stay in the group); visible-in-tree units are the subset with `is_visible_in_tree()` — a visibility-flag render-load proxy, not a drawn count (cull-mask/layer filtering and frustum culling are not accounted for). Both means are elapsed-time weighted: each window's count is multiplied by its measured window duration, the products are summed, and the total is divided by the measured duration, so a trailing partial window counts only for the time it spanned. Emit:

```text
start_units=200
min_live_units=<n>
mean_live_units=<elapsed-time-weighted average>
end_units=<n>
min_visible_in_tree_units=<n>
mean_visible_in_tree_units=<elapsed-time-weighted average>
end_visible_in_tree_units=<n>
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

Real combat deaths are allowed. Do not respawn or reset health.

- [ ] **Step 5: Keep CI at 10 seconds with no FPS gate**

Set only the existing benchmark step:

```yaml
GRUS_BENCHMARK_SECONDS: "10"
```

Keep Xvfb, `LIBGL_ALWAYS_SOFTWARE=1` and existing teardown-139 handling. Grep the new marker using `start_units=200`; do not enforce 60 FPS in CI.

- [ ] **Step 6: Update README benchmark commands**

Document the 10-second CI regression vs 60-second hardware acceptance and point current numbers to `docs/hpa-474-mvp-evidence.md`.

- [ ] **Step 7: Verify Task 2**

```bash
godot --headless --path godot res://scenes/reset_test.tscn
godot --headless --path godot res://scenes/scouting_ai_smoke_test.tscn
GRUS_BENCHMARK_SECONDS=3 godot --path godot --rendering-method gl_compatibility res://scenes/benchmark_200.tscn
```

Expected: retained smoke markers plus one live benchmark marker.

- [ ] **Step 8: Commit**

```bash
git add godot/scripts/benchmark_200.gd .github/workflows/ci.yml README.md
git commit -m "perf: measure mixed benchmark workload"
```

### Task 3: Add safe release packaging and take the early 60-second hardware sample

**Files:**
- Create: `scripts/package_linux_x86_64.sh`
- Modify: `.gitignore`
- Modify: `README.md`
- Modify: `docs/hpa-474-mvp-evidence.md`
- Conditional modify: only the measured performance owner + its existing test module if the early sample demonstrates a bottleneck

- [ ] **Step 1: Write the one release package script**

Required base behavior:

```bash
#!/usr/bin/env bash
set -euo pipefail
```

Then:

1. resolve repo root;
2. build `cargo build --release -p grus-godot --locked`;
3. stage `target/release/libgrus_godot.so` into `godot/bin/libgrus_godot.so`;
4. run Godot headless import;
5. clean/recreate `dist/grus-linux-x86_64/`;
6. export `Linux x86_64` with `--export-release`;
7. verify exported executable exists/is executable;
8. archive the full directory as `dist/grus-linux-x86_64.tar.gz`.

Add optional `GRUS_MAIN_SCENE`. When set:

- save the current `godot/project.godot` to a temporary file;
- install a shell `trap` before editing;
- temporarily replace the main-scene entry;
- restore `project.godot` on success, error or interruption.

The script must not download/install toolchains.

- [ ] **Step 2: Ignore generated release output**

Add:

```gitignore
dist/
```

- [ ] **Step 3: Export the benchmark as a release build**

On the Task-0 reference host:

```bash
GRUS_MAIN_SCENE=res://scenes/benchmark_200.tscn \
  ./scripts/package_linux_x86_64.sh
```

Verify `git diff -- godot/project.godot` is empty after the script, including after a deliberate failed export invocation used to exercise the trap path.

- [ ] **Step 4: Run the early 60-second sample**

Run the exported benchmark executable at 1920×1080 with:

```bash
GRUS_BENCHMARK_SECONDS=60
```

Record the full marker plus revision/host data in `Early release performance`.

- [ ] **Step 5: Apply the performance gate now, not at the end**

If mean FPS >= 60 and no repeatable stall exists, record “early target met; no performance change” and continue.

If target/stall fails:

1. profile first on the same workload;
2. record the measured hotspot;
3. add the narrowest owning regression/measurement;
4. make the smallest change in the existing owner only;
5. rerun the same 60-second sample and keep the change only if it improves the demonstrated problem without correctness loss.

No flow fields, second renderer, multithreaded presentation or speculative rewrite.

- [ ] **Step 6: Commit packaging + early evidence (and measured fix only if needed)**

```bash
git add scripts/package_linux_x86_64.sh .gitignore README.md docs/hpa-474-mvp-evidence.md crates godot
git commit -m "perf: establish release benchmark baseline"
```

### Task 4: Wire the mirrored manual launch, run the four openings, and tune from evidence

**Files:**
- Modify: `crates/grus-godot/src/lib.rs`
- Modify: `docs/hpa-474-mvp-evidence.md`
- Conditional modify: `crates/grus-sim/src/catalog.rs`, `catalog/tests.rs`, `ai.rs`, `ai/tests.rs`, `fixture.rs`, `fixture/tests.rs`

- [ ] **Step 1: Wire `GRUS_MIRRORED_START` only at the runtime seed boundary**

Read `GRUS_MIRRORED_START` outside pure simulation and pass a boolean into the shared `seed_match_world` helper.

Normal/unset launch stays Team 1 left / Team 2 right.

`GRUS_MIRRORED_START=1` becomes Team 1 right / Team 2 left, with Team-2 AI reading the authored left-side plan while remaining issuer Team 2.

Do not expose a menu toggle.

- [ ] **Step 2: Test the bool-selected seed directly**

Avoid tests that mutate process-global environment. Test the shared helper/default-vs-mirrored behavior directly.

Run:

```bash
cargo test -p grus-godot --lib --locked
cargo test -p grus-sim ai::tests --locked
cargo test -p grus-sim fixture::tests --locked
```

- [ ] **Step 3: Run the left-side pair in one process**

Default launch:

1. early pressure;
2. reach Result;
3. Restart and record clean resources/entities/queues/selections/fog/AI state;
4. economy-first/age-up;
5. reach Result;
6. Restart and record cleanliness again.

Record duration/result/expansion/raid/TC-finish observations immediately.

- [ ] **Step 4: Run the right-side pair in one mirrored process**

```bash
GRUS_MIRRORED_START=1 godot --path godot
```

Repeat the same early-pressure → Restart → economy-first → Restart evidence.

- [ ] **Step 5: Ensure both lifecycle outcomes are evidenced**

If the four strategy runs produced both Victory and Defeat, record them.

If all four produced only one outcome, perform one explicit lifecycle-only check by intentionally allowing the other result, then Restart and record clean state. Do not treat this extra run as balance evidence.

- [ ] **Step 6: Add a narrow regression before every demonstrated balance fix**

Examples only when the matching problem occurred:

- catalogue values → existing catalogue/combat/economy behavior test;
- AI threshold → existing pure policy/bounded journey test;
- spatial resource/route change → fixture symmetry/reachability/visibility test.

Never assert that a match must last 15–25 minutes in an automated test.

- [ ] **Step 7: Tune only the proven owner and rerun only affected matrix cases**

Keep tuning in `catalog.rs`, existing `ai.rs` thresholds and/or `fixture.rs` only.

Append reruns to the evidence table; do not overwrite original observations.

- [ ] **Step 8: Run simulation coverage gate and commit**

```bash
cargo test -p grus-sim --locked
cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\.rs$'
```

If no balance change is justified, commit only the evidence and explicitly record “no tuning change required.”

```bash
git add docs/hpa-474-mvp-evidence.md crates/grus-sim crates/grus-godot/src/lib.rs
git commit -m "balance: validate final MVP openings"
```

### Task 5: Add the minimal controls aid and validate the normal extracted package

**Files:**
- Modify: `godot/scenes/main.tscn`
- Modify: `godot/scripts/smoke_test.gd`
- Modify: `README.md`
- Modify: `docs/hpa-474-mvp-evidence.md`
- Conditional modify: only the existing scene/script that owns a demonstrated usability defect

- [ ] **Step 1: Add a failing smoke assertion for first-run controls help**

Before Start, require visible `HUD/SessionPanel/ControlsLabel` containing at least `Right click`, `A`, `S`, `Esc`, and `Minimap`.

- [ ] **Step 2: Add one compact static label**

Use copy equivalent to:

```text
Select: left click / drag / Shift
Command: right click
Attack-move: A then right click
Stop: S
Pause/resume: Esc
Camera: wheel / middle-drag
Minimap: click to recenter
```

Keep it inside the existing session panel so it hides during Playing. No help framework or new state.

- [ ] **Step 3: Fix only usability defects already demonstrated by Task 4**

For each concrete issue, add a focused assertion in the owning existing smoke first, then make the smallest scene/script change. No unrelated polish.

- [ ] **Step 4: Run all retained Godot smokes**

```bash
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
godot --headless --path godot res://scenes/economy_smoke_test.tscn
godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn
godot --headless --path godot res://scenes/scouting_ai_smoke_test.tscn
```

- [ ] **Step 5: Build the normal package from a fresh checkout/worktree**

Run without `GRUS_MAIN_SCENE`:

```bash
./scripts/package_linux_x86_64.sh
```

Extract elsewhere and launch outside the editor on the reference host.

- [ ] **Step 6: Complete the packaged gameplay journey**

Verify:

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

Record the result.

- [ ] **Step 7: Collect first-time-player evidence or state the exact limitation**

Have someone who has not previously played Grus attempt the extracted build using only the in-game controls label and no developer walkthrough.

Record whether they can start, gather/build/train, scout, fight, interpret result/restart and quit.

If no fresh player is available, write that specific missing acceptance evidence under Remaining limitations; do not substitute the author’s own run.

- [ ] **Step 8: Record attribution and package hash**

If no third-party game assets are present, record:

```text
Asset attribution: none.
```

Then:

```bash
sha256sum dist/grus-linux-x86_64.tar.gz
```

- [ ] **Step 9: Update README run/build/controls instructions and commit**

README should point current performance numbers to the evidence file rather than duplicating them.

```bash
git add godot/scenes/main.tscn godot/scripts README.md docs/hpa-474-mvp-evidence.md
git commit -m "ui: finish desktop MVP usability"
```

### Task 6: Re-measure the final SHA and close CI/package evidence

**Files:**
- Modify: `docs/hpa-474-mvp-evidence.md`
- Modify: `README.md` only for final limitations/instructions if needed
- Conditional modify: only the profiler-identified owner + owning regression if final measurement reveals a new repeatable bottleneck

- [ ] **Step 1: Export the benchmark from the final candidate revision**

On the same Task-0 host:

```bash
GRUS_MAIN_SCENE=res://scenes/benchmark_200.tscn \
  ./scripts/package_linux_x86_64.sh
```

Confirm `project.godot` restored cleanly.

- [ ] **Step 2: Run the identical final 60-second sample**

Use the same host, 1920×1080, release mode and `GRUS_BENCHMARK_SECONDS=60` workload as Task 3.

Record the complete marker and compare early vs final results side by side.

- [ ] **Step 3: Apply the final optimization gate**

If target is met and no repeatable stall exists, make no performance change.

If a new/repeatable failure exists, profile first and apply only the smallest measured-owner fix with one focused regression. Rerun the identical 60-second sample after the fix.

- [ ] **Step 4: Rebuild the normal final package and recheck extraction/boot**

Run:

```bash
./scripts/package_linux_x86_64.sh
sha256sum dist/grus-linux-x86_64.tar.gz
```

Update the final hash and packaged journey status if the final revision changed after Task 5.

- [ ] **Step 5: Finish the evidence record**

Record:

- exact final SHA;
- four required balance runs + any reruns;
- both lifecycle outcomes + repeated restart cleanliness;
- final tuning values, or none;
- early/final 60-second hardware results including mean live units;
- any profile-driven fix with before/after evidence;
- final package hash + fresh-checkout/extracted journey;
- fresh-player usability result or explicit gap;
- asset attribution;
- final CI run URL/status; and
- honest remaining limitations, including the known post-marker teardown-139 issue if it still exists.

- [ ] **Step 6: Commit and push final evidence**

```bash
git add docs/hpa-474-mvp-evidence.md README.md crates godot scripts .github .gitignore
git commit -m "docs: record Grus MVP delivery evidence"
git push
```

- [ ] **Step 7: Require the real final CI surface on the pushed head**

The evidence commit cannot cite its own run: it records the run on the last code/pin head, and pushing it re-runs the gate on the resulting commit. Require all three GitHub Actions jobs green on that pushed head — the exact final SHA:

```text
rust-build-lint
unit-tests
e2e
```

This is the automated release gate. Do not replace it with a shorter hand-maintained local command list. The evidence file names both the validated code SHA and the pushed head each recorded run covered, so a docs-only tail commit is explicit rather than untested.

## Self-review result

- **Benchmark lifecycle:** session-free by design; Town Center destruction cannot transition the benchmark to Result/freeze fixed systems.
- **Workload ownership:** pure `grus-sim`; bridge does not pretend queued commands are accepted gameplay commands.
- **ID/component reuse:** allocator-owned IDs and production-equivalent post-spawn components; no manual counter repair.
- **Runtime seed reuse:** one normal world seed path shared by setup/restart/benchmark; normal sessions are layered on top.
- **Performance evidence:** liveness + start/min/mean/end live load; one early and one final hardware sample on the same named host.
- **Acceptance mapping:** four directional runs, both outcomes, repeated restarts, controls, fresh-player evidence/limitation, package journey, attribution, final CI and current measurements all have explicit owners.
- **Scope:** no extra content, generic tuning framework, second benchmark, platform matrix or speculative optimization.