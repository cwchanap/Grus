# HPA-449 — Endless alien waves: implementation plan

**Issue:** https://linear.app/cwchanap/issue/HPA-449/grus-survival-38-wave-director-endless-alien-waves  
**Scoped spec:** [HPA-449 design](../specs/2026-10-24-hpa-449-wave-director-design.md)  
**Parent spec:** [Winter Survival](../specs/2026-10-21-winter-survival-design.md)  
**Verified base:** `main` `8b4c70c05458f133d7a541cd656c277d7f4ba02d`, includes merged HPA-448 PR #9.  
**Status:** documentation-only planning baseline; implementation and tests **not performed**. The **same draft PR/branch** is the implementation vehicle; never open a follow-up implementation PR for this ticket.

## Selection and dependencies

HPA-445 (1/8) and HPA-448 (2/8) are Done. Linear HPA-449 (3/8) is Todo and its sole blocker is completed HPA-448; it blocks HPA-450 and HPA-82. Later simulation tasks (walls/gates, heat, research, underground and late balance) remain out of scope. HPA-290/HPA-291/HPA-82/HPA-44 are separate art tasks and HPA-269 is a separate SFX task: use primitive role meshes and the existing combat effect in this code PR.

The existing project-wide Linear project description still describes the completed PvP MVP; follow the committed Winter Survival spec and the current HPA-449 Linear issue for this task.

## Existing code reused (avoid duplicate ownership)

| Responsibility | Reuse / change |
| --- | --- |
| Simulation clock/lifecycle | `session.rs::gameplay_active`; extend chained `FixedUpdate` in `crates/grus-godot/src/lib.rs`. No timer in GDScript. |
| Unit identity/spawn | `ids.rs::IdAllocator`, `commands.rs::spawn_unit`, `UnitIndex`, `SimPosition`, `Health`. No Alien entity hierarchy. |
| Movement/targeting | Existing `PlayerCommand::Units(AttackMove)`, path/occupancy, `combat.rs::step_combat`, eight-cell acquisition and visibility. |
| Crystal currency | `economy.rs::TeamEconomy/ResourceStockpile.crystals`, credit at combat's *actual kill* seam, not animation callbacks. |
| Balance | Extend `catalog.rs::UnitSpec` / `CombatSpec` and named wave constants. Avoid hard-coded values in `waves.rs` or Godot. |
| UI / fog | `GrusBridge` read-only snapshots, `battlefield_controller.gd` HUD, `minimap.gd` overlays; keep enemy markers vision-gated. |
| Reset | `seed_match_world` and `clear_gameplay_world`; no alternate restart policy. |
| Bench / tests | Evolve `fixture.rs` load helper, `benchmark_200.gd`, existing smokes and CI. Do not keep a second PvP test fixture. |

## Implementation steps — one PR, reviewable commits

These are **steps/commits within one ticket**, not subtasks requiring separate PRs. Prefer green functional checkpoints; at minimum, do not merge while any cross-layer fixture or enum cutover is incomplete.

### 1. Typed alien catalogue and deterministic wave planning

**Files:** `crates/grus-sim/src/catalog.rs`, `catalog/tests.rs`, new `waves.rs` and `waves/tests.rs`, `lib.rs`; adjust production/bridge exhaustiveness.

- Add `UnitKind::{Swarmer,Spitter,Brute,Behemoth}`, alien HP/speed/combat/threat/Crystal-reward/unlock specs, and the scoped `UnitKind::HUMAN` subset for human train/bridge buttons. Ensure no alien gets a producer, queue or population use in the normal match. Preserve `UnitKind::ALL` for exhaustive sim kind expectations. Keep Guard-vs-Swarmer and Rifleman-vs-Spitter bonuses in catalogue, with initial numbers documented there only.
- Add typed wave parameters in `catalog.rs` with the 300/100/30 schedule, 150 cap and formula terms; implement pure budget, unlock/roster and zone-rotation helpers with stable integer rounding, and an ECS `WaveState` resource. All w>=1 schedules are computed from one next-wave index; avoid a generalized queue.
- Test small and large wave budgets, availability at 1/4/8/10, bosses at 10/20, 9→10 and 19→20 zone changes, deterministic replay, zero/partial budget behavior and one-before/one-after timing boundaries. Protect against overflow and HP-multiplier divide-by-zero.

### 2. Cut over to a one-base fixture and run actual alien orders

**Files:** `crates/grus-sim/src/fixture.rs` and tests, `waves.rs`, `commands.rs` only if a targeted reusable helper is proven needed, `crates/grus-godot/src/lib.rs`, `crates/grus-sim/tests/system_order.rs`.

- Remove the authored Team-2 Research Center, four passive Workers and unused Team-2 stockpile; keep a single Team-1 central Research Center and four human Workers. Adapt `TeamStart`/helpers and tests to the one-side seed. Keep the authored finite resources unless a spawn/reachability proof shows an actual conflict; never invent new deposits for Crystals.
- Insert `WaveState` with a 300-second prep countdown on every normal seed. Drop it during `clear_gameplay_world`. Insert `advance_waves` directly after pending commands, before combat. Its `Time<Fixed>` delta is the only time input, including accelerated smoke. Same-tick spawn reaches combat/movement as normal.
- On wave launch, choose unique, walkable border cells, create aliens via `IdAllocator + spawn_unit`, assign validated Team-2 AttackMove toward a free perimeter approach to the central Research Center. Fail loudly in tests if any authored zone cannot route. Cap *live* alien count at 150; apply the documented fresh-wave-only overflow HP multiplier, test both reduced-cap and completely full-cap outcomes.
- Validate that Team 2 remains noncommandable by Team 1, exposed targets still require actual visibility, and Team-2 killing the center reaches the existing Defeat result. Prove pause/start/result prevent advancing wave state or spawning. Prove Restart restores countdown, wave number, spawn IDs and empty crystal stock.

### 3. Crystal rewards and non-cheating combat integration

**Files:** `combat.rs`, `combat/tests.rs`, `economy.rs` only if a tiny reusable stockpile credit helper is warranted, `catalog/tests.rs`.

- At the existing `strike` kill case, snapshot victim `Unit.kind/team` before `destroy_unit`; if and only if Team 1 kills a Team-2 alien, add the catalogue Crystal reward to Team 1's `ResourceStockpile`. No award from `CombatEvents`/Godot, no extra inventory or floating pickups.
- Test one kill/one reward, multi-hit no premature credit, multiple kills accumulating exact rewards, enemy/human deaths granting none, full reset discarding rewards, and counter-specific damage against Swarmer/Spitter with no accidental human-vs-human bonus.
- Keep the existing combat event payload/renderer unless alien integration actually reveals a missing field. Liveness-safe iteration and Research Center first-destruction-wins stay unchanged.

### 4. Godot wave UI, visibility and real runtime journey

**Files:** `crates/grus-godot/src/lib.rs`, `godot/scripts/battlefield_controller.gd`, `godot/scripts/minimap.gd`, `godot/scripts/unit_view.gd`, `godot/scenes/main.tscn`, `godot/scripts/combat_lifecycle_smoke_test.gd`, new `godot/scripts/wave_smoke_test.gd` / `godot/scenes/wave_smoke_test.tscn`, smoke helpers as needed.

- Add `wave_snapshot()` read-only from the ECS state; serialize only number, next deadline/countdown, announced N/E/S/W names and live alien count. Add a small current-wave/countdown/incoming-zone HUD; draw short border pings in the existing minimap on warnings, **without exposing unscouted units or the selected zone before the 30-second warning**. Never drive spawning from UI or wall time.
- Give Swarmer, Spitter, Brute and Behemoth clearly different placeholder role silhouettes and ensure the existing fog-hidden enemy view behavior still gates clicking/minimap markers. Keep the existing health bars and hit FX. Make Start UI say a survival run is ready.
- Rewrite legacy combat smoke assumptions of 8 starting Workers, enemy worker ID 5, enemy Research Center ID 2, and Victory. Keep the useful command-input, damage, health, role, pause, Result gating, Restart and effect assertions, but now prove **undefended alien-induced Defeat** with a bounded 20× journey. Do not accidentally preserve a fake PvP base to make an old test pass.
- Add a focused 20× wave smoke: countdown does not advance in Start/Paused, warning appears 30 sim seconds before first spawn, first wave arrives at 300, enemies attack-move, a player kill raises Crystals by exactly the corresponding catalogue reward, zone ping does not reveal hidden enemies, and Restart clears state. Aim for <=90 seconds wall-clock per scene. Prefer driving normal building/training/attack methods; any white-box test setup must be explicitly labelled as such and never used by the playable match.

### 5. Replace benchmark workload and migrate all fixture consumers

**Files:** `fixture.rs`, `fixture/tests.rs`, `crates/grus-godot/src/lib.rs`, `godot/scripts/benchmark_200.gd`, `godot/scripts/reset_test.gd`, `godot/scripts/smoke_test.gd`, `godot/scripts/economy_smoke_test.gd`, `crates/grus-sim/tests/*`, CI/README as needed.

- Replace `extend_with_benchmark_workload`'s 100-vs-100 gather/attack-move PvP seed. Through the shared wave-spawn code, stage **150 aliens** with attack-move plus **50 human units total** (including the four seed Workers) defending the central base. Script human defender orders using the existing dispatcher. Keep the benchmark **session-free** and fogged. Validate 200 unique live IDs, exactly **one** Research Center, 18 finite resource views if unchanged, no rejected setup commands, and both view-visible and hidden live aliens.
- Keep `benchmark_200.gd` measured real duration, frame percentiles, live/visible-in-tree sample averages, movement/combat liveness checkpoints, 10s CI and 60s hardware path. Update the startup census/metadata and baseline description; retire "two Town Centers" and "both teams gather" assertions. Do not substitute static dummy enemies that make the liveness gate meaningless.
- Update reset, bridge, economy, fixture and system-order assertions for the single human start plus empty Team 2; preserve the parts of those tests that still cover meaningful controls/transaction invariants. Remove obsolete hard-coded passive-opponent waypoint tests rather than adapt them into fake success conditions.
- CI must call the new wave smoke and the migrated combat smoke with **exit code 0** plus markers. The benchmark test should remain fully gated, not commented out or temporarily waived.

### 6. Final integration, evidence and documentation

**Files:** `README.md`, new/updated docs from this PR, `.github/workflows/ci.yml` only where test scene changes require it.

- Verify every runtime mode: boot → Start → Playing → warning → one or more waves, pause/resume without time drift, Crystal reward after a legitimate kill, Research Center defeat, Result command rejection, Restart to fresh Start. Verify both unit targeting and minimap fog do not expose unknown enemy positions while an incoming **zone** is announced.
- Record actual 10s CI benchmark results under the new survival workload and, if hardware is available, a **new** 60s hardware run with machine/OS/renderer and build; do **not** carry HPA-448 FPS into this workload. Without hardware access, report that gate as unmeasured rather than inventing a number. Retain historical benchmark records as historical only.
- Update README economy/gameplay, wave behavior/controls, fixture census, smoke invocation and benchmark sections, with one link to the wave design. No unrelated architecture cleanup.

## Acceptance / final gates

- [ ] Format: `cargo fmt --all -- --check`.
- [ ] Compile and lint: `cargo clippy --workspace --all-targets -- -D warnings`; `cargo clippy -p grus-godot --all-targets --features e2e -- -D warnings`.
- [ ] Full Rust workspace tests and **>=90% `grus-sim` line coverage** with the existing cargo-llvm-cov gate; no test-time-only duplicated algorithms.
- [ ] Rust wave, catalogue, crystal reward, fixture, system-order and session regressions listed in the design.
- [ ] Real Godot bridge, fixture-reset, economy, combat-lifecycle **Defeat**, and new wave smokes; initial warning, attack, reward and Restart asserted against authoritative bridge snapshots. All pass within existing/<=90s scene timeouts and clean process exit.
- [ ] Linux Godot import, smoke export/boot, `bevy-e2e` exported-game test and strict CI build split still pass.
- [ ] Session-free 200-unit survival benchmark runs the full existing frame-stat/liveness contract at 10s CI; measurements labeled as the **new** workload.
- [ ] README/spec synchronized; PR remains **Draft** while documentation-only and is not merged before the implementation and gates are complete.

## Deliberate exclusions and choices for review

- **No new task split:** art and SFX already have independent existing Linear tickets. Within HPA-449 there is exactly one PR from design through implementation.
- **No substitute victory:** the old passive-base Victory smoke becomes an alien Defeat smoke. Research Victory belongs to HPA-188.
- **No walled-in workaround yet:** alien AttackMove uses presently reachable map perimeter; wall/gate behavior is HPA-450.
- **No separate benchmark engine:** the bench uses the same spawn/orders/visibility, not handcrafted benchmark-only alien entities or fake combat.
- **Cap-full edge:** when zero spawn slots exist, no newly spawned wave unit can receive an overflow HP multiplier; the director still advances its schedule. Explicitly test this limitation. Revisit only with evidence in later balance work.
- **No extra abstractions:** one resource, one module, existing fixed-step chain, typed catalogue, existing read-only bridge snapshot. No configurable strategies, RNG, timer service, persistence, replay, multiplayer, engine upgrades or compatibility layers.
