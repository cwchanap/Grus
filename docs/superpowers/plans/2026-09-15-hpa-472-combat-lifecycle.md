# HPA-472 Combat and Match Lifecycle Implementation Plan

**Goal:** Extend the current Grus economy/production loop into a complete combat match with authoritative damage/destruction, attack/attack-move controls, Start/pause/result/restart lifecycle, and runtime verification.

**Architecture:** Bevy ECS stays authoritative. Add only `combat.rs` and `session.rs`; extend the existing catalogue, command dispatcher, activity-cancellation seam, stable indexes, reset seam and Godot bridge. Godot remains presentation/input/UI only. This draft PR is the implementation PR for HPA-472.

**Tech Stack:** Rust 1.95.0, Bevy 0.19.1, godot-bevy 0.12.0, godot-rust 0.5.5, Godot 4.6.2, GDScript, existing GitHub Actions CI.

**Spec:** `docs/superpowers/specs/2026-09-15-hpa-472-combat-lifecycle-design.md`

## Global constraints

- Exactly one Linear ticket and one GitHub PR; continue implementation on this branch.
- Preserve the Bevy-authoritative / Godot-presentation boundary.
- Preserve the >90% `grus-sim` production-code line coverage gate.
- Every implementation commit keeps formatting, Clippy and focused Rust tests green; keep runtime gates green at each integration point.
- Canonical final order: commands -> combat -> movement -> economy -> construction -> production -> route feedback.
- HPA-472 explicitly requires `Start / Playing / Paused / Result`; keep Start. Missing `MatchSession` still means Playing for pure-sim fixtures.
- Use one combat `target_eligible()` seam so HPA-473 can add visibility without rewriting combat.
- Reuse `MoveOrder`, A*, stable IDs, `GridMap::set_blocked`, indexes and authored fixture. Unit pursuit may use `assign_move_toward()`; building pursuit uses existing `approach_slots()` but sorts candidates by attacker distance at the combat call site.
- Keep all tuning in `catalog.rs`, including `ATTACK_MOVE_RADIUS`.
- One `cancel_unit_activity()` path owns retask cleanup. Do not add per-command `CombatOrder` removal.
- `CombatEvents` is current-tick-only: clear it at the start of `step_combat()` so headless journeys cannot accumulate events.
- Append `RejectReason` variants only; never reorder existing discriminants or reuse `Locked`.
- No projectile physics/entities, armour/damage-type framework, abilities, formations, garrisons, extra victory modes, persistence, multiplayer, AI/fog, new art pipeline or generic service/event architecture.
- No image-generation task: primitive role markers, health bars and presentation-only tracer/hit effects are enough.

---

## Task 1: Add combat/session contracts, catalogue tuning, and one cancellation seam

**Files:**
- Create: `crates/grus-sim/src/combat.rs`
- Create: `crates/grus-sim/src/session.rs`
- Modify: `crates/grus-sim/src/catalog.rs`
- Modify: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/buildings.rs`
- Modify: `crates/grus-sim/src/fixture.rs`
- Modify: `crates/grus-sim/src/lib.rs`

- [ ] Add catalogue tests for military combat specs, counter relationships, villager noncombatant status, building health and `ATTACK_MOVE_RADIUS`.
- [ ] Extend `UnitSpec` with `max_health` + `Option<CombatSpec>` and `BuildingSpec` with `max_health`.
- [ ] Use initial tuning from the spec, including Archer **+12 vs Spearman** so the counter leg is not dependent on an opening-shot timing edge.
- [ ] Add `ATTACK_MOVE_RADIUS: f32 = 8.0` beside existing gameplay constants; no radius literal in `combat.rs`/GDScript.
- [ ] Add `Health`, `CombatTarget`, `CombatOrder`, `AttackCooldown`, `CombatEvent`, `CombatEvents`, `MatchPhase`, `MatchResult`, and `MatchSession` contracts.
- [ ] Keep `last_target_cell` on `CombatOrder`; no `PursuitState` component.
- [ ] Extend `UnitCommandKind` with `AttackMove` and `PlayerCommand` with direct `Attack`.
- [ ] Append `NotCombatant`, `TargetMissing`, `InvalidTarget`, `SessionLocked` to `RejectReason`.
- [ ] Rename `cancel_worker_activity()` to `cancel_unit_activity()` and make it also remove `CombatOrder` while preserving its existing worker/Farm/build cleanup, GatherProgress reset, Carry preservation and MoveOrder removal. Update existing Move/Stop/Gather/Place/Resume call sites atomically.
- [ ] Make `spawn_unit()` attach catalogue Health/AttackCooldown. Seeded/placed buildings attach Health.
- [ ] Export only contracts later tasks need; no placeholder framework/systems.

**Focused verification:**

```bash
cargo test -p grus-sim catalog
cargo test -p grus-sim commands
cargo test -p grus-sim economy
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## Task 2: Implement attack, attack-move, unit destruction, pursuit, and combat ordering

**Files:**
- Modify: `crates/grus-sim/src/combat.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/map.rs`
- Modify: `crates/grus-sim/tests/system_order.rs`
- Modify: `crates/grus-godot/src/lib.rs` for the production FixedUpdate chain
- Add/modify: `crates/grus-sim/src/combat/tests.rs`
- Add/modify: `crates/grus-sim/src/commands/tests.rs`

- [ ] Write failing tests for target validation, villager rejection, enemy-only targeting, melee/ranged range, cooldown and building base damage.
- [ ] Add three **behavioral counter-duel tests** using symmetric AttackMove: Spearman survives vs Cavalry, Archer survives vs Spearman, Cavalry survives vs Archer. Assert gameplay outcome, not just catalogue `counter_target` values.
- [ ] Add `Footprint::closest_point(Vec2) -> Vec2` with a Town Center melee-range regression.
- [ ] Implement the single `target_eligible(world, attacker_team, target)` seam.
- [ ] Direct Attack validates first, then calls `cancel_unit_activity()` and installs combat intent. AttackMove does the same per accepted military unit.
- [ ] AttackMove acquisition uses `ATTACK_MOVE_RADIUS`, nearest target, stable-ID tie-break.
- [ ] At the top of every `step_combat()`, clear `CombatEvents`.
- [ ] Snapshot attacker stable IDs in ascending order. Before each acts, re-resolve it via `UnitIndex`; if an earlier hit destroyed it, skip it. Never keep an Entity handle and blindly mutate after another attacker may despawn it.
- [ ] Add a low-health cross-target regression: lower UnitId kills the later attacker first; the dead later attacker is skipped and cannot act/panic.
- [ ] If already in range, do not path.
- [ ] Unit pursuit may use `assign_move_toward()` against the target's walkable cell.
- [ ] Building/site pursuit calls `approach_slots()`, sorts candidates by distance to the attacker **only at the combat call site**, then uses a walkable candidate with the existing move assignment seam. Do not change economy/building approach ordering.
- [ ] Refresh unit pursuit only when target cell changes or route ends; static building pursuit only when route ends.
- [ ] Add `UnitIndex::remove()` and atomic `destroy_unit()`: `cancel_unit_activity`, index removal, despawn. Carry dies with worker.
- [ ] Direct Attack ends on missing/dead target; AttackMove clears dead target and resumes destination.
- [ ] Move/Stop/worker commands inherit combat cancellation through the shared helper; no command-site duplication.
- [ ] Record current-tick hit/death data in `CombatEvents`.
- [ ] Insert `step_combat()` before movement in both Godot `FixedUpdate` and the existing `system_order.rs` long-form test.

**Required tests / verification:**

```bash
cargo test -p grus-sim combat
cargo test -p grus-sim commands
cargo test -p grus-sim --test system_order
cargo build -p grus-godot --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Required behavior includes closest-point building range, nearest-perimeter pursuit, counter-duel winners, pursuit reuse/replan, AttackMove continuation after atomic unit death, and liveness-safe same-step iteration.

---

## Task 3: Make building destruction atomic and prove combat-before-economy

**Files:**
- Modify: `crates/grus-sim/src/combat.rs`
- Modify: `crates/grus-sim/src/buildings.rs`
- Modify: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/production.rs` only if tests expose a real invariant gap
- Modify: `crates/grus-sim/tests/system_order.rs`
- Add/modify: focused module tests

- [ ] Add `BuildingIndex::remove()` following the existing ResourceIndex/new UnitIndex pattern.
- [ ] Write tests for Farm worker idle, destroyed drop-off reroute, no-valid-dropoff idle, every construction task referencing a destroyed site, freed footprint, discarded queue and House capacity reduction.
- [ ] Implement one building-destruction transaction: capture footprint/Farm resource identity, free cells, remove BuildingIndex/Farm ResourceIndex entries, retask affected workers, despawn.
- [ ] Scan all live `WorkerTask` values that hold the destroyed `BuildingId`; do not clean only `active_builder` or `assigned_worker`.
- [ ] `ToConstruction` / `Constructing` workers use existing cancellation and become Idle.
- [ ] `ToDropoff { dropoff: destroyed_id }` workers reroute preserved Carry immediately through a narrow helper over the existing nearest-reachable-dropoff logic; if no route, become visibly Idle while preserving Carry.
- [ ] A destroyed Farm idles its assigned worker with Carry preserved whether moving to it, gathering, or returning a Farm-sourced load. Never call standalone `deplete_source()` for a Farm.
- [ ] A destroyed producer loses queue with no refund.
- [ ] Derived `population_cap()` falls naturally; do not delete living units above cap. Retain sequential production completion and add reduced-cap regression.
- [ ] Stale combat targets disappear through stable indexes + `target_eligible`, not a global order scan.
- [ ] Add a **second `system_order.rs` test**: in the same tick, a carrying worker reaches a drop-off slot while combat destroys that drop-off. Run combat -> movement -> economy and assert stockpile does not receive the stale deposit. Structure it so moving combat after economy makes the test fail.

**Focused verification:**

```bash
cargo test -p grus-sim combat
cargo test -p grus-sim economy
cargo test -p grus-sim buildings
cargo test -p grus-sim production
cargo test -p grus-sim --test system_order
```

---

## Task 4: Add Start/Pause/Resume/Result/Restart gating and freeze interpolation correctly

**Files:**
- Modify: `crates/grus-sim/src/session.rs`
- Modify: `crates/grus-sim/src/combat.rs`
- Modify: `crates/grus-sim/src/movement.rs`
- Modify: `crates/grus-godot/src/lib.rs`
- Modify: normal-skirmish Godot smokes affected by Start
- Add/modify: Rust session tests and bridge tests

- [ ] Keep all four ticket-required phases: `Start`, `Playing`, `Paused`, `Result`.
- [ ] Missing `MatchSession` means Playing; keep `seed_skirmish()` session-free so old pure-sim tests need no lifecycle boilerplate.
- [ ] Normal Godot setup/reset inserts Start; benchmark reset inserts Playing or removes stale session.
- [ ] In the same commit, update affected normal-skirmish smokes to call `start_match()` before gameplay commands; do not defer and leave CI frozen.
- [ ] Resolve Result immediately on Town Center destruction. `step_combat()` returns immediately once Result is set, so first destruction wins.
- [ ] Gameplay command application rejects with `SessionLocked` for explicit Start/Paused/Result; absent session remains Playing.
- [ ] Combat/economy/construction/production perform no gameplay mutation outside Playing.
- [ ] **Movement special case:** when session is Start/Paused/Result, `step_movement()` must still set every live `SimPosition.previous = current` before returning without advancing movement. This prevents Godot interpolation from cycling forever between stale previous/current positions while `Engine.time_scale` remains active.
- [ ] Add a regression asserting `previous == current` after one paused fixed tick for an in-flight unit; keep the MoveOrder so Resume can continue normally.
- [ ] Add `session_snapshot()`, `start_match()`, `set_paused(bool)`, `restart_match()`.
- [ ] User pause never changes `Engine.time_scale`; existing sim-speed control stays for headless acceleration only.
- [ ] Restart uses existing clear + reseed and returns normal skirmish to Start.
- [ ] Reset cleanup removes combat/session transients without creating a second reset implementation.
- [ ] Repeat reset/start cycles in a bridge regression: initial resources/entities/indexes are restored once with no stale IDs/nodes.
- [ ] Add same-step dual-Town-Center threat regression: stable first result cannot be overwritten by a later attacker.

**Runtime verification:**

```bash
cargo test -p grus-sim
cargo test -p grus-godot --lib --locked
# stage extension
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
godot --headless --path godot res://scenes/economy_smoke_test.tscn
# run benchmark path and prove units still move without Start interaction
```

---

## Task 5: Add bounded passive-opponent victory/defeat journeys

**Files:**
- Add/modify: `crates/grus-sim/src/combat/tests.rs`
- Add/modify: `crates/grus-sim/src/session/tests.rs`
- Reuse: fixture/economy/buildings/production/commands

- [ ] Build a helper around the normal authored skirmish; no runtime AI/game mode/debug grant API.
- [ ] Add economy -> production -> army -> enemy Town Center -> victory journey using real commands/systems.
- [ ] Add separate defeat journey where team 2 destroys team 1 through the same combat systems.
- [ ] Implement journeys as **bounded event loops**, not hardcoded tick arithmetic:

```rust
let mut resolved_at = None;
for tick in 0..BUDGET {
    step_match(...);
    if is_result(&world) {
        resolved_at = Some(tick);
        break;
    }
}
assert!(resolved_at.is_some(), "match did not resolve inside budget");
```

- [ ] Assert Result freezes stockpiles, positions, construction, production, cooldowns and age-up progress across subsequent ticks.
- [ ] Assert commands after Result return/reveal `SessionLocked` and do not mutate.
- [ ] The helper may accelerate direct fixed-step calls but must assert outcomes/invariants rather than exact completion ticks.

---

## Task 6: Wire Godot combat/lifecycle incrementally with a growing smoke

**Files:**
- Modify: `crates/grus-godot/src/lib.rs`
- Modify: `godot/scripts/battlefield_controller.gd`
- Modify: `godot/scripts/unit_view.gd`
- Modify: `godot/scripts/building_view.gd`
- Modify: `godot/scenes/unit_view.tscn`
- Modify: `godot/scenes/building_view.tscn`
- Modify: `godot/scenes/main.tscn`
- Create first: `godot/scripts/combat_lifecycle_smoke_test.gd`
- Create first: `godot/scenes/combat_lifecycle_smoke_test.tscn`
- Create only if useful: one tiny transient combat-effect helper

- [ ] **Start Task 6 by creating the combat lifecycle smoke scene/script.** Keep it running as each integration slice lands; Task 7 only promotes/finishes the already-working smoke.
- [ ] Add bridge call `attack_units(ids, target_kind: GString, target_id)`. Add `parse_target_kind()` beside the existing parse helpers; accept only `"unit"` / `"building"`. This matches existing input-kind convention rather than introducing magic integer inputs.
- [ ] Add `attack_move_units(ids, target)`.
- [ ] Reuse `_nearest_view` for enemy picking with an enemy-only filter and the existing closest-view tie-break shape; do not create a second picking subsystem.
- [ ] Right-click enemy unit/building -> Attack; ground right-click -> Move; `A` then ground -> AttackMove; `S` -> Stop.
- [ ] `Esc` cancels placement first, otherwise toggles Playing/Paused. Do not emit gameplay orders in Start/Paused/Result.
- [ ] Add health bars to unit/building views from `Changed<Health>` through existing node handles, not per-frame GDScript world polling.
- [ ] Add primitive role-marker/scale differences for the four unit kinds; no art assets.
- [ ] Drain current-tick `CombatEvents` into short tracer/hit/death effects; cosmetics never drive combat.
- [ ] Add one tiny attack/hit sound cue without an audio framework.
- [ ] Add session overlay + Pause button: Start, Paused/Resume, Victory/Defeat + Restart/Quit.
- [ ] Restart clears transient effects plus local selection/control-group state while bridge reseeds; no dead stable IDs remain in Godot state.
- [ ] Grow the smoke to exercise each landed slice: Start -> Attack bridge -> HP decreases -> death/view removal -> result overlay -> input rejection -> Restart -> fresh state, plus role readability assertions that do not depend on final art.

**Focused verification while Task 6 is in progress:**

```bash
cargo build -p grus-godot --locked
cargo test -p grus-godot --lib --locked
# stage extension and import
godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn
# keep prior smoke/reset/economy gates green as relevant
```

Do not wait until Task 7 to discover bridge/view/session integration failures.

---

## Task 7: Promote the lifecycle smoke to CI, document, and finish the PR

**Files:**
- Modify: `godot/scripts/combat_lifecycle_smoke_test.gd`
- Modify: `godot/scenes/combat_lifecycle_smoke_test.tscn`
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`

- [ ] Finish any remaining full-journey assertions in the smoke; do not create a second redundant runtime test.
- [ ] Add one HPA-472 smoke step to the existing `e2e` CI job with evidence-based timeout.
- [ ] Re-run existing smoke/reset/economy gates; do not weaken prior assertions.
- [ ] Keep Bevy E2E exported-game test as the small boot/selector contract; no redundant full UI automation.
- [ ] Update README controls/lifecycle/verification and note HPA-473 still owns fog/economic AI.
- [ ] Run full local-equivalent gates:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --locked
cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\\.rs$'
cargo test -p grus-godot --lib --locked
cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1
cargo build -p grus-godot --features e2e --locked
# stage extension, import Godot, run existing smoke/reset/economy + combat lifecycle smoke
```

- [ ] Confirm Linux export/boot, Bevy E2E boot and 200-unit benchmark pass in CI.
- [ ] Keep PR draft until every HPA-472 acceptance item has test/runtime evidence.

## Definition of done

- Military roles fight with the intended counter triangle; behavioral duel tests prove each counter survives its paired matchup.
- Contextual Attack, AttackMove and Stop work through the real input/bridge path.
- Building pursuit uses nearest walkable perimeter candidates; melee range uses `Footprint::closest_point`.
- `cancel_unit_activity()` is the single retask cleanup seam for worker, movement and combat intent.
- Combat iteration is liveness-safe under mid-step despawns and its event buffer is bounded to one tick.
- Workers/buildings/sites die with required economy, occupancy and queue cleanup, including workers already traveling to destroyed drop-offs/sites.
- `system_order.rs` proves combat-before-economy with the same-tick destroyed-drop-off case.
- Start, pause/resume, result, restart and quit work without editor intervention; missing session remains Playing for pure-sim fixtures.
- Frozen movement collapses `previous = current`, so pause/result/start do not visually oscillate while Godot interpolation keeps running.
- Destroying the sole enemy/player Town Center yields a stable first result and freezes later gameplay mutation.
- Victory/defeat journeys are bounded by outcome budgets, not brittle exact tick schedules.
- Restart produces one fresh match with no duplicate nodes/stale IDs.
- Existing CI/export/benchmark behavior remains green and `grus-sim` stays above 90% production-line coverage.
- No HPA-473 visibility/AI, new art, or unrelated framework work enters this PR.
