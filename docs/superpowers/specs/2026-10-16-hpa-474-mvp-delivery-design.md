# HPA-474 Balance, Performance and Desktop MVP Delivery Design

## Status

Planning baseline for HPA-474. This design and its implementation plan live on the same branch and draft PR that will carry the implementation; there is no follow-up implementation PR for this ticket.

HPA-470 through HPA-473 already provide the retained battlefield, economy, combat/lifecycle, fog/minimap and economic AI. HPA-474 is therefore a measured tuning-and-delivery pass, not another gameplay slice. It must not add a second state owner, a new gameplay framework, another benchmark system, a release-service pipeline, or new content merely to make the final ticket look substantial.

Two planning reviews are incorporated. The first made deferred contracts explicit. The second removes unnecessary bridge logic, fixes a benchmark-lifecycle flaw that could inflate FPS after the match ended, moves the first hardware measurement earlier, and maps the remaining ticket acceptance items directly to evidence.

## Outcome

Ship the existing one-map human-vs-AI skirmish as a usable Linux x86_64 desktop MVP with three kinds of evidence:

1. four directional playtests show both an early-pressure opening and an economy-first/age-up opening can credibly win from either authored starting side, with typical unforced matches trending toward 15–25 minutes;
2. a representative 200-unit mixed workload is measured for 60 seconds in a release export on an actual hardware-rendered Linux x86_64 reference host, reporting mean FPS plus p95/p99 frame times, live-unit load and liveness; and
3. a fresh-checkout release package launches outside the editor and completes the normal start → economy → scout → combat → result → restart journey without debug intervention.

The ticket may tune existing values and fix defects proven by those runs. It does not create new strategic systems, new units/buildings, another difficulty mode, an installer, signing/notarization, or a multi-platform release matrix.

## Current baseline we keep

The code already has the correct ownership boundaries:

- Bevy ECS is the only mutable gameplay model; Godot owns input, rendering, HUD and effects.
- The simulation remains a 20 Hz fixed step.
- `catalog.rs` is the single gameplay catalogue for costs, build/train times, health, damage, speed and gather rates.
- `ai.rs` owns the small 1 Hz scripted opponent. Its tuning values remain ordinary constants, not a generic tuning/config layer.
- `fixture.rs` owns the handcrafted mirrored map, resource placement and authored AI slots/routes.
- `PlayerCommand` / `CommandResult` / `RejectReason` remain the only gameplay-order path.
- `benchmark_200.tscn` / `benchmark_200.gd` and the existing benchmark reset bridge remain the one performance harness.
- Linux x86_64 remains the one supported delivery target established by HPA-470.

The current benchmark is intentionally insufficient for HPA-474: it is a visibility-free, AI-free, session-free editor/debug fixture containing 200 fast Villagers, with `MapFixture::units_200()` and bridge tests explicitly locking that old contract. HPA-474 replaces that retained benchmark contract in place with a representative mixed workload. The old `UnitSpawn` / `units_200()` helper and tests are deleted when the new seed lands; they do not remain as dead parallel fixture code.

`README.md` is the authoritative feature/baseline record and `AGENTS.md` documents the reset/working contracts. The benchmark contract changes are incomplete until those files stop describing the old 200-Villager fixture.

The current Start overlay also has no concise in-game control reference. README documentation is good developer documentation, but a first-time player should not need the repository open to discover selection, contextual right-click, attack-move, stop, pause, camera and minimap controls.

## Reference-host precondition

HPA-470 recorded **Linux x86_64** as the supported desktop/export target and recorded an Ubuntu/Xvfb/llvmpipe CI benchmark. That benchmark is explicitly a software-rendered regression baseline, not representative GPU hardware. No specific hardware-rendered Linux machine is recorded in the repository today.

HPA-474 must not silently reinterpret the CI runner as the 60 FPS reference host, and it must not silently change the product target to macOS merely because a developer workstation may be macOS.

Before implementation starts, record one actual Linux x86_64 GPU host in `docs/hpa-474-mvp-evidence.md` with a stable human-readable machine label plus OS, CPU, GPU/adapter, renderer and Godot version. The same host is used for the early and final 60-second samples and for the extracted release-package journey.

If no such host is available, record that as a blocking acceptance dependency before Task 1. Do not defer discovery until the final task and do not claim HPA-474 performance/package acceptance on the llvmpipe CI machine.

## Design rule: evidence before tuning

Do not choose new balance numbers from the planning document.

The existing HPA-472/HPA-473 values are the baseline. Each balance change must point to an observed playtest problem such as:

- early pressure cannot convert worker damage into a meaningful economic advantage;
- economy-first is automatically superior or automatically loses;
- the expansion is reached too late to matter or is never worth contesting;
- Town Center time-to-kill makes ordinary armies unable to finish a won match; or
- a shared AI threshold causes one authored start to stall while the mirrored start works.

Allowed tuning locations are deliberately narrow:

- `crates/grus-sim/src/catalog.rs` — shared costs, timings, health/damage, speed and gather rates;
- `crates/grus-sim/src/ai.rs` — existing opponent policy thresholds only; and
- `crates/grus-sim/src/fixture.rs` — the existing one-map resource/route/slot layout only when the playtest evidence is spatial.

Do not add a balance database, difficulty profile, runtime config file, behavior tree, utility scorer, upgrade tree, siege unit, second victory condition, or hidden AI bonus.

## One shared normal-match seed seam

Today `setup_fixture` and `reset_fixture_world` duplicate the normal seed/map/fog/AI sequence. HPA-474 needs the same base state for the benchmark and for mirrored manual playtests, so keeping three copies would create unnecessary drift.

Add one bridge-internal helper conceptually equivalent to:

```text
seed_match_world(world, mirrored)
```

It owns only normal world construction:

1. build the retained `MapFixture` and map;
2. seed the normal skirmish, using the default or mirrored authored starts;
3. insert the map;
4. insert/initially refresh `VisibilityMap`; and
5. insert the Team-2 `AiController`, configured with the authored plan side that matches its physical start.

It deliberately does **not** insert `MatchSession`.

Normal setup/restart call this helper, then insert `MatchSession { Start }`. The benchmark calls the same helper with `mirrored=false` and remains session-free. This reuses one setup path without creating a scenario framework.

The environment variable itself is read outside the pure simulation. Pure helpers/tests receive a boolean. This avoids process-global environment mutation inside ordinary simulation tests.

## Four-run playtest matrix and mirrored launch seam

The required directional evidence is one run for each pair:

| Start side | Opening |
| --- | --- |
| left / Team 1 authored start | early pressure |
| left / Team 1 authored start | economy-first / age-up |
| right / mirrored authored start | early pressure |
| right / mirrored authored start | economy-first / age-up |

The normal shipping runtime remains human Team 1 on the left versus Team 2 AI on the right. The two right-side human runs use one explicit **test/manual-only** launch switch:

```bash
GRUS_MIRRORED_START=1 godot --path godot
```

The mirrored setup contract is exact:

- Team 1 is seeded at the authored right start and remains the bridge/local-human team;
- Team 2 is seeded at the authored left start and remains the AI team;
- the battlefield/resources stay the same symmetric authored map;
- Team 2 still issues commands as Team 2, but its controller reads the authored left-side `AiMapPlan`; and
- restart preserves the launch choice because the same boolean seed choice is reused.

Keep the implementation narrow. The default `seed_skirmish()` contract remains available to existing pure simulation tests. Add only the mirrored start selection needed by the shared seed helper. `AiController` gains at most one authored-plan-side field/constructor seam; no generic faction/side system is introduced.

Run the two left-side playtests in one process, using Restart between matches. Run the two right-side playtests in one mirrored process, again using Restart between matches. After each restart, record that resources, entities, queues, selections, fog and AI state returned cleanly.

The final evidence must cover both Victory and Defeat. If the four strategy runs happen to produce only one outcome, add one explicit outcome-path check by intentionally allowing the other side to win; this is lifecycle evidence, not a fifth balance claim.

For every balance run record revision, start side, opening, duration, result, first expansion use, worker-raid outcome, whether ordinary military could finish the Town Center, restart cleanliness and any tuning applied afterward. The four runs are directional evidence, not a competitive-balance claim.

If the last run causes a tuning change that materially affects an earlier matrix case, rerun only the affected case. Automated tests pin invariants, not emergent match duration.

## Minimal usability pass

Add one static, compact controls/help label to the existing non-Playing session panel. It should cover only the commands a first-time player needs to start playing:

- left click / drag / Shift for selection;
- right click for contextual command;
- `A` then right click for attack-move;
- `S` stop;
- `Esc` pause/resume;
- mouse wheel + middle drag for camera; and
- minimap click to recenter.

No tutorial campaign, modal-help framework, tooltip system, key-rebinding screen, settings menu or generated art is required.

The ticket acceptance says the existing flow should be understandable without developer explanation. The author’s own playtests cannot demonstrate that. The extracted packaged build should therefore be attempted by someone who has not previously played Grus, using only the in-game controls label and no verbal walkthrough. Record whether they can complete the required journey. If no fresh player is available, record this specific acceptance gap as a limitation rather than treating the author’s own run as equivalent evidence.

Additional HUD/control changes are allowed only when a playtest demonstrates a concrete readability or interaction defect. Every such fix gets a focused smoke/regression assertion in the existing scene/test that already owns the flow.

## Representative 200-unit benchmark

Keep the existing benchmark scene and reset seam, but replace the benchmark world’s old contract with the real-game workload.

### World shape

Start from the normal authored, fogged Team-1-left / Team-2-right match state, but with **no `MatchSession` resource**. Missing session already means Playing in the simulation and `resolve_result` is intentionally a no-op for session-free worlds, so a Town Center death cannot freeze the benchmark partway through a 60-second sample.

Extend that world to exactly 100 units per side:

- 20 Villagers;
- 30 Spearmen;
- 30 Archers; and
- 20 Cavalry.

The existing four starting Villagers per team count toward the 20. Add 96 units per team at deterministic walkable staging cells.

Allocate every added ID through `IdAllocator::allocate_unit()` rather than hard-coding numeric ranges or manually resetting the allocator counter. Spawn every added unit with the same post-spawn component bundle used by production (`Carry::Empty`, `GatherProgress::default()`, `WorkerTask::Idle`) regardless of kind. There is no benchmark-specific health/speed/damage override.

The old `UnitSpawn` / `MapFixture::units_200()` path is removed in the same change.

### Workload belongs in `grus-sim`

Do not add `GrusBridge.benchmark_start_workload()`.

A bridge `queue_command()` result means only “queued”; it cannot truthfully report that `apply_player_command` accepted the command on a later fixed tick. Choosing unit/source membership in a `#[func]` would also put benchmark gameplay logic in the presentation bridge.

Instead, the pure benchmark seed/extension helper in `grus-sim` issues its initial orders directly through `apply_player_command` and asserts every required `CommandResult` is accepted. Its fixture test therefore locks both the composition and accepted workload under the normal simulation coverage gate.

Initial workload:

- first 12 sorted Villagers per side receive Gather orders, two per authored safe source;
- military units receive ordinary AttackMove orders toward the same authored **midfield corridor target**, not toward either Town Center; and
- the real Team-2 AI remains active and continues making its normal 1 Hz decisions after setup.

Using a midfield clash keeps combat live without intentionally driving the benchmark into Town Center destruction. Even if a Town Center is later destroyed, the session-free world continues simulating.

### Runtime sampling contract

`benchmark_200.gd` becomes duration-based instead of the current 40-frame sample:

- short CI/default regression sample: 10 real seconds after warm-up;
- acceptance run: `GRUS_BENCHMARK_SECONDS=60`;
- start only after exactly 200 unit views are present;
- require at least two building views rather than exactly two, because the live Team-2 AI may place a foundation before Godot finishes attaching all 200 views;
- require at least the authored 18 resource views;
- capture the initial visibility revision and wait for a later revision before timing; no separate `revision >= 0` assertion is needed;
- preserve a periodic liveness checkpoint rather than deleting the existing one. Within the checkpoint window, fail only if there is neither meaningful unit movement nor any drained combat event; and
- never respawn units or reset health merely to preserve the starting count.

Report at least:

```text
start_units=200
min_live_units=<n>
mean_live_units=<time-weighted/frame-weighted average>
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

`mean_live_units` makes the actual measured render/simulation load visible when real combat reduces the starting 200-unit roster.

The CI Xvfb/llvmpipe run remains a regression signal only. It must not be compared to the 60 FPS hardware target.

## Measure the riskiest unknown early

Do not wait until the final task to discover whether the representative benchmark is far below target.

Immediately after the mixed benchmark and duration-based sampler are implemented, create/use the release-package script and run the first 60-second **release-export** sample on the recorded hardware host. This is an early diagnostic, not final acceptance.

If the target is missed or a repeatable stall is visible, record the hotspot investigation early. Do not perform speculative optimization; profile before changing code. Balance/usability work may continue independently, but the likely performance owner is no longer a late surprise.

The final task reruns the identical 60-second sample on the final SHA and decides whether any measured optimization is retained.

## Linux release package

Keep one supported target: Linux x86_64.

Add one small repository script, `scripts/package_linux_x86_64.sh`, that performs the release steps reproducibly:

1. `cargo build --release -p grus-godot --locked`;
2. stage `target/release/libgrus_godot.so` into `godot/bin/libgrus_godot.so`;
3. run the Godot headless import once;
4. export `Linux x86_64` with `--export-release` into `dist/grus-linux-x86_64/`; and
5. create `dist/grus-linux-x86_64.tar.gz` from that complete exported directory.

The script accepts an optional `GRUS_MAIN_SCENE`. When set, it temporarily rewrites the project main scene for the export and installs a shell `trap` that restores `godot/project.godot` on success, error or interruption. Task 3 uses this to export the benchmark scene without duplicating the fragile backup/sed/restore sequence in the plan.

Add `dist/` to `.gitignore`. Do not add installer/store/signing/notarization/release automation.

A fresh-checkout verification must run the script, extract the normal archive elsewhere, launch it outside the editor on the same Linux x86_64 host, and confirm the gameplay journey plus pause/resume, result/restart and quit.

## One home for current measurements

Do not duplicate current HPA-474 performance numbers in both README and the evidence document.

`docs/hpa-474-mvp-evidence.md` is the single home for current playtest/performance/package numbers. README describes the current benchmark contract, the commands to run it, and links to the evidence record. The old HPA-470 numeric table can be removed from README; git history and PR #1 preserve that historical result.

`AGENTS.md` must replace the old “200-villager, visibility-free benchmark reset” invariant with the new session-free mixed/fog/AI benchmark contract.

## Evidence record

Create one durable record at `docs/hpa-474-mvp-evidence.md` containing:

- reference host preflight and exact environment;
- four-run playtest table and tuning log;
- repeated-restart cleanliness and both lifecycle outcomes;
- final shared tuning values changed from HPA-473, or explicitly none;
- early and final 60-second release performance records with start/min/mean/end live units;
- any profile-driven optimization and before/after numbers;
- fresh-checkout package command + archive path/hash;
- packaged gameplay journey result;
- fresh-player usability result, or an explicit limitation if unavailable;
- asset attribution (`none` when no third-party assets apply);
- final CI run URL/SHA and three-job status; and
- honest remaining limitations.

The PR description may summarize this file, but the repository record is the source of truth for HPA-474 evidence.

## Testing and CI

Keep the current three-job CI shape and the 90% `grus-sim` production-line coverage gate.

Expected changes are narrow:

- fixture/simulation tests lock the exact 100-per-side mixed roster and accepted benchmark workload;
- bridge tests lock benchmark visibility + AI while preserving the **session-free** contract;
- old `units_200()` and `benchmark_move_all()` tests/code are removed, not shimmed;
- a focused mirrored-launch test locks default vs mirrored setup without a shipping side-selection UI;
- `benchmark_200.gd` proves runtime liveness and emits duration/live-load metrics;
- README/AGENTS update with the benchmark contract in the same slice;
- an existing Godot smoke asserts the compact controls help is visible before Start;
- any balance/usability/performance defect fixed by this ticket gets one focused regression in the existing owner; and
- existing bridge/reset/economy/combat/scouting smokes, export boot, Bevy E2E boot and Codecov remain green.

The final verification gate is the **actual CI surface** at the exact final revision: `rust-build-lint`, `unit-tests`, and `e2e` must all be green. Local-only acceptance evidence is the final 60-second hardware release sample plus the fresh extracted release-package journey.

Do not enforce 60 FPS in GitHub Actions.

## Out of scope

Multiplayer, persistence/save/load, more maps, factions, difficulties, campaign/tutorial content, extra units/buildings, siege, formations, terrain/elevation gameplay, replay, new victory conditions, balance framework/config service, behavior-tree/utility AI, pathfinding replacement, flow fields, multithreaded renderer/presentation, custom art/animation/SFX pipeline, installers, stores, signing/notarization, auto-update, cloud services and multi-OS release packaging remain outside the MVP.