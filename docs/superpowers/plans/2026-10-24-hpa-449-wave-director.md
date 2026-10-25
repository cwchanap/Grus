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
| Research Center approach destinations | **EXTEND/REUSE** `commands.rs::approach_slots(map, footprint, used, count)` for immediate-perimeter candidates. Keep route reservations/validation in the existing AttackMove dispatcher; do not add another perimeter scanner. |
| Crystal currency | `economy.rs::TeamEconomy/ResourceStockpile.crystals`, credit at combat's *actual kill* seam, not animation callbacks. |
| Balance | Extend `catalog.rs::UnitSpec` / `CombatSpec` and named wave constants. Avoid hard-coded values in `waves.rs` or Godot. |
| UI / fog | `GrusBridge` read-only snapshots, `battlefield_controller.gd` HUD, `minimap.gd` overlays; keep enemy markers vision-gated. |
| Reset | `seed_match_world` and `clear_gameplay_world`; no alternate restart policy. |
| Bench / tests | Evolve `fixture.rs` load helper, `benchmark_200.gd`, existing smokes and CI. Do not keep a second PvP test fixture. |

## Implementation steps — one PR, reviewable commits

These are **reviewable stages inside this single PR**, not additional tickets/PRs. Sequence the independent features first, then do **one honest fixture-and-consumers cutover**, with no period in which the benchmark, reset, or combat smokes are quietly disabled or falsely called green. Current `seed_skirmish` can support pure `step_waves` Rust tests before the final Team-2 base is removed; the old fixture is **not** kept in the final game.

### 1. Typed catalogue, fixture zone metadata and pure roster planning (green on current fixture)

**Files:** `crates/grus-sim/src/catalog.rs` and tests, `fixture.rs` and tests (add only `ENTRY_ZONES`, do not remove Team-2 yet), `waves.rs` with pure tests, `lib.rs`; production and bridge enum exhaustiveness.

- Add `UnitKind::{Swarmer,Spitter,Brute,Behemoth}`, `UnitKind::HUMAN` and `UnitSpec::alien: Option<AlienSpec { threat, crystal_reward, first_wave }>` mirroring existing `combat: Option<CombatSpec>`. No alien can be produced through normal commands or the human catalogue UI. Test that `HUMAN` exactly equals `ALL` minus kinds having `alien: Some`.
- Keep all wave **tuning** in `catalog.rs`, including 300/100/30 seconds, 150 cap, and budget coefficients as **integers**. Implement `pool(w) = 8 + 4*w + (4*w*w)/10` using widened/saturating arithmetic, every-tenth Behemoth reserved first, half of the remaining pool as Swarmers, then alternate affordable unlocked Brute/Spitter, convert residue to Swarmers. Cap ordering is **Behemoth → Brute → Spitter → Swarmer**; test exact planned rosters for waves 1, 4, 8 and 10, launch/cap priority, availability and budget overflow cases.
- Keep coordinates only in `fixture.rs::ENTRY_ZONES: [(EntryZone, GridPos); 4]` beside `RUINS` (N 64,0 / E 127,48 / S 64,95 / W 0,48). Fixture test: all anchors walkable, and each has a path to a walkable `commands.rs::approach_slots` candidate of Team-1's core. Rotation tests cover 9→10 and 19→20; no copied GDScript coordinates.
- `WaveState` holds only `elapsed` and `next_wave`. Compute warning zones from `zones_for(next_wave)` and deadline, without storing a duplicate announcement. Test boundary freeze/advance at Start/Playing/Pause/Result and exactly-once timer edges; no new timer framework.

### 2. Crystal reward and counters, independent of fixture cutover (green on current fixture)

**Files:** `combat.rs`, `combat/tests.rs`, `catalog/tests.rs`, `economy.rs` only if a tiny credit helper is warranted.

- At the existing `strike` kill boundary, inspect `unit_spec(victim.kind).alien` before despawn and credit its `crystal_reward` to Team-1 stockpile **only** for a Team-1 killer and Team-2 alien victim; no `CombatEvents` or presentation-driven grants.
- Test exactly-once kill credit, no pre-death or human/team-mismatched award, accumulation, and combat counters (Guard→Swarmer, Rifleman→Spitter only). Spawn aliens via the existing `spawn_unit` within today's fixture for these tests. Do not yet remove Team-2's old Research Center.

### 3. Unwired pure-simulation wave director (green on current fixture)

**Files:** new `waves.rs` and `waves/tests.rs`, `commands.rs` for one targeted visibility/reuse change to `destination_slots` (make it `pub(crate)`), optional existing tests.

- Implement `step_waves(world, map, delta_seconds)` in the sim without adding `advance_waves` to the Godot bridge yet. Test manually seeded `WaveState` against the **current** `seed_skirmish` with its temporary Team-2 base; keep all test setup explicitly scoped to pure simulation.
- Spawn near each fixture-authored zone anchor through **existing `commands.rs::destination_slots`** (deterministic expanding rings, used-set and walkability; no new border scanner). Filter live occupied cells, keep all spawn cells distinct, distribute heavy-first roster round-robin across selected zones, call `IdAllocator + spawn_unit`, and issue Team-2 `AttackMove` toward `approach_slots` via the existing dispatcher. Do **not** perform an additional per-spawn A*: AttackMove validation is authoritative; rejected orders enter the retry path. Test bad/reachable paths and same-tick orders.
- Enforce 150 **live** alien cap and fresh-wave-only overflow HP, including an all-slots-full zero-recipient launch. Implement a **single** throttled retry predicate for an alien with no `MoveOrder`, no current combat target (no `CombatOrder` or `AttackMove { target: None }`), and distance from the Research Center's `Footprint::closest_point` **> `ATTACK_MOVE_RADIUS`**. Derive retry once per simulated second from `WaveState.elapsed`, stable-ID order; keep existing moving/engaged aliens unchanged. Test rejected route, finished-out-of-range destination and lost route recovery, plus path-call delta using existing `#[cfg(test)] GridMap::path_call_count` to demonstrate **director** throttling only. Do not claim to cure `combat.rs::resume_destination` per-tick replans; walled-in rescue remains HPA-450.

### 4. Single integrated one-base cutover, bridge wiring and **all** fixture consumers (one green checkpoint)

**Files:** `crates/grus-sim/src/fixture.rs` and tests, `crates/grus-godot/src/lib.rs`, `crates/grus-sim/tests/system_order.rs`, `godot/scripts/{reset_test,smoke_test,economy_smoke_test,combat_lifecycle_smoke_test,benchmark_200}.gd`, `godot/scripts/smoke_helpers.gd`, other existing fixture consumers and CI scenes as needed.

- **Cut over together, not in 2a then much-later step 5.** Delete the Team-2 `TeamStart`, Research Center, workers and stockpile; preserve the four central human Workers and 18 resources. Convert all fixture-based Rust assertions, hard-coded IDs, fog/scouting paths and Godot startup/reset/smoke cases at the same time. Specifically remove/update the old benchmark loop over both teams and `MapFixture::safe_source_ids(TeamId(2))` (which would panic), plus `reset_test.gd`'s 8-unit/2-building census and `smoke_test.gd`'s passive enemy Research Center/worker assertions. No legacy enemy base and no skipped or neutered smokes.
- Wire `advance_waves` after `apply_pending_commands` and before combat in the existing 20 Hz `FixedUpdate` chain, using `Time<Fixed>`. Seed/reset `WaveState` through the existing session and clear paths; missing-session benchmark remains Playing. Validate one-base Defeat, normal input protection, zone timing and cross-system order.
- **Replace the PvP benchmark in the same cutover commit** with **150 actual aliens and ~50 defenders (200 unique live units)** using the same spawn and AttackMove internals and the session-free benchmark path. Keep one Research Center, the real resource views, fog semantics, deterministic seeds, valid movement/combat, 10s CI / 60s hardware timers, frame-stat fields and liveness assertions. Do not duplicate AI or preserve the old 100-vs-100 scenario.
- Rework the pre-existing combat lifecycle smoke into a real alien-caused **Defeat**, retaining its meaningful input/combat/Result/Restart checks. Adapt old scouting/fog checks to live alien units rather than nonexistent passive Team-2 structures. Preserve exit-code and timeout gates.
- **Protect the pre-wave contract:** add `SmokeHelpers.no_launched_wave_error(where)` (or an equivalently small shared assertion) reading `GrusBridge.wave_snapshot()` and returning `"smoke overran wave prep"` if wave 1 has launched. Call it at the **end of every pre-existing Playing smoke run** (and before any reset that would hide an overrun), not the dedicated wave/combat-Defeat scenarios that intentionally run through wave 1. If a legacy scenario is genuinely too slow to stay within 300 simulated seconds, shorten/split the test interaction honestly rather than adding a test-only prep-time override. The old smokes now visibly fail on unintended alien intrusion.

### 5. Wave snapshot, HUD/minimap and new real wave journey

**Files:** `crates/grus-godot/src/lib.rs`, `godot/scripts/{battlefield_controller,minimap,unit_view}.gd`, `godot/scenes/main.tscn`, new `godot/scripts/wave_smoke_test.gd` and its scene, CI smoke registration.

- Add one read-only `wave_snapshot()` with wave number, countdown, live alien count and (only inside warning window) **announced zone names and their anchor cells from `fixture::ENTRY_ZONES`**. Godot renders those authoritative anchors as minimap pings; neither zone coordinates nor timers live in GDScript and no unseen enemy locations are exposed.
- Make small wave HUD, primitive distinguishable Swarmer/Spitter/Brute/Behemoth silhouettes, and a Start overlay that refers to survival rather than skirmish. Keep the existing fog-gated unit views and hit FX; no art/audio task creep.
- Add 20x wave smoke: Start/Pause freeze, 30s warning, 300s launch, accepted alien orders and meaningful movement, real Team-1 kill and Crystal receipt, no fog information leak, alien-induced Defeat and Restart. Run within <=90s wall-clock, require actual Godot exit code 0 plus success marker. Maintain the ordinary playable path as the source of gameplay writes, labelling any deliberate white-box test setup.

### 6. Final gates and evidence

**Files:** `README.md`, changed docs, CI configuration only where new scenes require it.

- Verify full Start → Playing → warning → wave → Crystal kill → alien-caused Defeat → Result → Restart; repeat with Pause/Resume and authoritative fog/ping data.
- Keep formatting, no-warning Clippy, whole-workspace Rust tests, >=90% `grus-sim` coverage, imported/exported Godot boot, existing migrated smokes, new wave smoke, E2E and 200-unit benchmark as **real gates**. Update README economy/survival controls, initial census and wave/benchmark descriptions.
- Record newly measured 10-second CI survival benchmark and a newly measured 60-second hardware run **only if hardware is available**, with hardware/renderer/build metadata. The HPA-448 100-vs-100 numbers are historical and **not comparable**. Report any unmeasured hardware gate without inventing values.

## Acceptance / final gates

- [ ] Format: `cargo fmt --all -- --check`.
- [ ] Compile and lint: `cargo clippy --workspace --all-targets -- -D warnings`; `cargo clippy -p grus-godot --all-targets --features e2e -- -D warnings`.
- [ ] Full Rust workspace tests and **>=90% `grus-sim` line coverage** with the existing cargo-llvm-cov gate; no test-time-only duplicated algorithms.
- [ ] Rust wave, exact roster/order (1/4/8/10), `HUMAN`/alien catalogue partition, fixture zone reachability, Crystal reward, system-order and session regressions listed in the design; one retry predicate for rejection/arrival/lost leg, with `path_call_count` proving only **director** throttling.
- [ ] Real Godot bridge, fixture-reset, economy, adapted scouting/fog smoke, combat-lifecycle **Defeat**, and new wave smokes. Every other pre-existing Playing smoke asserts via shared helpers that **no wave launched before completion** (including before resets); wave/Defeat scenarios are explicitly exempt. All pass within existing/<=90s scene timeouts and clean process exit.
- [ ] Linux Godot import, smoke export/boot, `bevy-e2e` exported-game test and strict CI build split still pass.
- [ ] Session-free 200-unit survival benchmark runs the full existing frame-stat/liveness contract at 10s CI; measurements labeled as the **new** workload.
- [ ] README/spec synchronized; PR remains **Draft** while documentation-only and is not merged before the implementation and gates are complete.

## Deliberate exclusions and choices for review

- **No new task split:** art and SFX already have independent existing Linear tickets. Within HPA-449 there is exactly one PR from design through implementation.
- **No substitute victory:** the old passive-base Victory smoke becomes an alien Defeat smoke. Research Victory belongs to HPA-188.
- **No walled-in workaround yet:** alien AttackMove uses presently reachable map perimeter; wall/gate behavior is HPA-450.
- **No separate benchmark engine:** the bench uses the same spawn/orders/visibility, not handcrafted benchmark-only alien entities or fake combat.
- **Cap-full edge:** when zero spawn slots exist, no newly spawned wave unit can receive an overflow HP multiplier; the director still advances its schedule. Explicitly test this limitation. Revisit only with evidence in later balance work.
- **No extra abstractions:** `WaveState { elapsed, next_wave }`, one `waves.rs` module, fixture-owned zone anchors, existing destination/approach-slot helpers, typed optional alien catalogue metadata and one read-only bridge snapshot. No second border scanner or A* spawn preflight, configurable strategies, RNG, timer service, persistence, replay, multiplayer, engine upgrades or compatibility layers.
