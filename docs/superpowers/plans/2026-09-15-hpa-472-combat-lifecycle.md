# HPA-472 Combat and Match Lifecycle Implementation Plan

**Goal:** Extend the current Grus economy/production loop into a complete combat match with authoritative damage/destruction, attack/attack-move controls, start/pause/result/restart lifecycle, and runtime verification.

**Architecture:** Bevy ECS stays authoritative. Add only `combat.rs` and `session.rs`; extend the existing catalogue, command dispatcher, stable indexes, reset seam and Godot bridge. Godot remains presentation/input/UI only. The planning draft is the implementation PR for HPA-472.

**Tech Stack:** Rust 1.95.0, Bevy 0.19.1, godot-bevy 0.12.0, godot-rust 0.5.5, Godot 4.6.2, GDScript, existing GitHub Actions CI.

**Spec:** `docs/superpowers/specs/2026-09-15-hpa-472-combat-lifecycle-design.md`

## Global constraints

- Exactly one Linear ticket and one GitHub PR. Continue implementation on this draft branch.
- Preserve the current Bevy-authoritative / Godot-presentation boundary.
- Preserve the >90% `grus-sim` production-code line coverage gate.
- Every implementation commit keeps formatting, Clippy and focused Rust tests green; keep runtime gates green at each logical integration point.
- Canonical final fixed-step order: commands -> combat -> movement -> economy -> construction -> production -> route feedback.
- Missing `MatchSession` means Playing for pure simulation/tests. Only the normal Godot skirmish setup/reset inserts Start; benchmark reset remains immediately Playing.
- Use one combat target-eligibility function. HPA-473 must be able to add visibility there instead of rewriting combat.
- Reuse `MoveOrder`, A*, stable IDs, `GridMap::set_blocked`, current indexes and the authored fixture. Unit pursuit may reuse `assign_move_toward()` directly; building pursuit must first select a walkable immediate-perimeter goal with `approach_slots()`.
- Do not pathfind every frame. Unit pursuit refreshes only when a target changes grid cell or its route ends; static building pursuit refreshes only when its route ends.
- Keep combat tuning in `catalog.rs`, including `ATTACK_MOVE_RADIUS`.
- Append new `RejectReason` variants only; the bridge exposes existing discriminants as numeric codes.
- No projectile entities/physics, armour/damage-type framework, abilities, formations, garrisons, extra victory modes, save/load, multiplayer, AI, fog, new art pipeline or generic service/event architecture.
- No image-generation ticket is needed: use primitive unit markers/health bars and presentation-only tracer/hit effects.

---

## Task 1: Add combat/session contracts and catalogue health

**Files:**
- Create: `crates/grus-sim/src/combat.rs`
- Create: `crates/grus-sim/src/session.rs`
- Modify: `crates/grus-sim/src/catalog.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/buildings.rs`
- Modify: `crates/grus-sim/src/fixture.rs`
- Modify: `crates/grus-sim/src/lib.rs`

- [ ] Add failing catalogue tests for military combat specs, the three counter relationships, villager noncombatant status, building health and the attack-move acquisition radius.
- [ ] Extend `UnitSpec` with `max_health` + `Option<CombatSpec>` and `BuildingSpec` with `max_health` using the design values.
- [ ] Add `pub const ATTACK_MOVE_RADIUS: f32 = 8.0` beside the existing gameplay constants; no combat tuning literal should live in `combat.rs` or GDScript.
- [ ] Add `Health`, `CombatTarget`, `CombatOrder`, `AttackCooldown`, `CombatEvent`, `CombatEvents` and the `MatchPhase` / `MatchResult` / `MatchSession` contracts.
- [ ] Keep the last pursued target cell on `CombatOrder`; do not add a separate `PursuitState` component.
- [ ] Extend `UnitCommandKind` with `AttackMove` and `PlayerCommand` with direct `Attack`.
- [ ] Append typed rejects: `NotCombatant`, `TargetMissing`, `InvalidTarget`, `SessionLocked`. Do not reorder existing variants or reuse `Locked`.
- [ ] Make `spawn_unit()` attach catalogue health/cooldown. Seeded and placed buildings attach catalogue health.
- [ ] Export only the contracts later tasks need through `lib.rs`; do not add placeholder systems.

**Focused verification:**

```bash
cargo test -p grus-sim catalog
cargo test -p grus-sim combat
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## Task 2: Implement direct attack, attack-move, unit destruction and pursuit

**Files:**
- Modify: `crates/grus-sim/src/combat.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/map.rs`
- Modify: `crates/grus-sim/src/movement.rs` only if a narrow helper is needed
- Modify: `crates/grus-sim/tests/system_order.rs`
- Modify: `crates/grus-godot/src/lib.rs` for the production FixedUpdate chain only
- Add/modify: `crates/grus-sim/src/combat/tests.rs`
- Add/modify: `crates/grus-sim/src/commands/tests.rs`

- [ ] Write failing tests for direct target validation, villager rejection, enemy-only targeting, melee/ranged range, cooldown, all three counter bonuses and building base damage.
- [ ] Add `Footprint::closest_point(Vec2) -> Vec2` with tests proving a melee unit beside a 4x4 Town Center measures to the perimeter, not its center.
- [ ] Implement the single `target_eligible(world, attacker_team, target)` seam. Do not duplicate eligibility checks in attack-move acquisition.
- [ ] Implement deterministic target distance: point-to-point for units; `Footprint::closest_point()` for buildings/sites.
- [ ] Implement direct Attack acceptance. Validate first, then cancel old worker/movement/combat activity for accepted units.
- [ ] Implement AttackMove acceptance and `ATTACK_MOVE_RADIUS` acquisition; nearest target wins with stable-ID tie breaking.
- [ ] Implement `step_combat()` cooldown, target refresh, hit application and pursuit assignment.
- [ ] If already in range, do not path. Unit pursuit may call `assign_move_toward()` with the target unit's walkable cell.
- [ ] For building/site pursuit, seed the existing reservation view, choose a walkable immediate-perimeter goal with `approach_slots()`, then pass that walkable goal through the existing move assignment seam. Never pass a blocked footprint cell or `Footprint::center()` to A*.
- [ ] Track the last pursued target cell on `CombatOrder` so unit pursuit does not rerun A* every tick.
- [ ] Add `UnitIndex::remove()` and an atomic `destroy_unit()` hit-path helper: worker cleanup, index removal, despawn. Do not defer unit death to Task 3.
- [ ] Direct Attack ends on missing/dead target. AttackMove clears a dead unit target and resumes toward its stored destination through the same `step_combat()` implementation.
- [ ] Update Move and Stop so accepted commands clear combat intent; Stop still performs existing worker cleanup.
- [ ] Record hit/death data in `CombatEvents`; do not add Godot effects yet.
- [ ] Insert `step_combat()` before movement in both the Godot `FixedUpdate` chain and `crates/grus-sim/tests/system_order.rs` in this task. With no combat order it is a no-op, so the existing 900-tick Age 2 ordering assertion stays meaningful and green.

**Required tests:**

- melee does not hit outside range and hits once inside range;
- Archer hits at range and respects cooldown;
- Spearman > Cavalry, Archer > Spearman, Cavalry > Archer bonuses;
- unit pursuit creates/reuses movement and refreshes after target cell change;
- building pursuit ends on an immediate-perimeter cell and never targets a blocked footprint;
- Town Center melee range uses `closest_point`, not `center`;
- AttackMove acquires, atomically destroys a unit target, then continues toward its destination;
- Stop cancels pursuit/attack;
- normal Move replaces combat intent;
- buildings/construction sites accept base damage without yet requiring their destruction cleanup.

---

## Task 3: Make building destruction atomic and preserve economy/production invariants

**Files:**
- Modify: `crates/grus-sim/src/combat.rs`
- Modify: `crates/grus-sim/src/buildings.rs`
- Modify: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/production.rs` only where tests expose a real invariant gap
- Add/modify: focused module tests

- [ ] Add `BuildingIndex::remove()` following the existing `ResourceIndex::remove()` / new `UnitIndex::remove()` pattern.
- [ ] Write destruction tests before implementation for Farm worker idle, destroyed drop-off reroute, no-valid-dropoff idle, every builder task that references a destroyed site, freed footprint, discarded production queue and House capacity reduction.
- [ ] Implement one building-destruction helper. Capture footprint/Farm resource identity, free footprint cells with `GridMap::set_blocked`, remove `BuildingIndex`/Farm `ResourceIndex` entries, retask every affected worker, then despawn.
- [ ] Scan all live `WorkerTask` values that hold the destroyed `BuildingId`; do not clean only `Building.construction.active_builder` or `ResourceSource.assigned_worker`.
- [ ] `ToConstruction` and `Constructing` workers for the destroyed site go through existing worker cancellation and become Idle.
- [ ] Every `ToDropoff { dropoff: destroyed_id, ... }` worker immediately reroutes its preserved carry to another reachable same-team drop-off using a narrow economy helper around the existing `nearest_reachable_dropoff`; if none is reachable, use existing route-failure/cleanup behavior to become visibly Idle while preserving carry.
- [ ] When a Farm dies, its assigned worker becomes Idle immediately and preserves carry even if it was moving to the Farm, gathering, or returning a Farm-sourced load. Do not use standalone `deplete_source()` for a Farm's 2x2 building footprint.
- [ ] Verify the combat-before-economy order prevents a destroyed drop-off worker from reaching the stale slot and calling `deposit_carry`, and prevents a destroyed site's worker from transitioning `ToConstruction -> Constructing` in that tick.
- [ ] A destroyed producer drops its queue with no refund.
- [ ] Rely on derived `population_cap()` after House/TC removal; do not delete units above the new cap.
- [ ] Keep existing sequential spawn completion as the simultaneous-cap safety mechanism and add one regression test at the reduced cap.
- [ ] Ensure stale combat targets clear through the stable-index/eligibility seam rather than scanning every combat order during destruction.

**Focused verification:**

```bash
cargo test -p grus-sim combat
cargo test -p grus-sim economy
cargo test -p grus-sim buildings
cargo test -p grus-sim production
cargo test -p grus-sim --test system_order
```

---

## Task 4: Add authoritative result, Start/Pause/Resume/Restart gating, and atomically adapt existing runtime gates

**Files:**
- Modify: `crates/grus-sim/src/session.rs`
- Modify: `crates/grus-sim/src/combat.rs`
- Modify: `crates/grus-godot/src/lib.rs`
- Modify: `godot/scripts/smoke_test.gd` where normal skirmish Start affects it
- Modify: `godot/scripts/reset_test.gd`
- Modify: `godot/scripts/economy_smoke_test.gd`
- Add/modify: Rust session tests and bridge library tests

- [ ] Write failing session tests for missing-session-as-Playing, explicit Start freeze, Playing advancement, Paused freeze, Resume without catch-up, Result freeze and command rejection outside Playing.
- [ ] Keep `seed_skirmish()` session-free. A missing `MatchSession` is treated as Playing by gameplay command/system gates so current pure-sim tests do not need lifecycle boilerplate.
- [ ] Normal Godot `setup_fixture` and `reset_fixture_world` insert `MatchSession::Start` after seeding.
- [ ] `reset_benchmark_world` explicitly inserts `MatchSession::Playing` (or removes the resource after clear) so the 200-unit benchmark remains immediately active.
- [ ] In this same commit, update every existing normal-skirmish smoke affected by Start to call `start_match()` before issuing gameplay commands. Do not defer these adaptations to Task 7 and leave intermediate CI red.
- [ ] Resolve `MatchPhase::Result` immediately when a Town Center is destroyed, recording winner/loser teams.
- [ ] When Result is set during `step_combat()`, return immediately from the whole combat step. The first Town Center destruction in stable attacker order decides; there is no same-tick draw/overwrite.
- [ ] Gate `apply_pending_commands` with `SessionLocked` only when an explicit session is Start/Paused/Result; absent session remains Playing.
- [ ] Gate combat/movement/economy/construction/production advance functions with the same optional-session rule; route feedback may still drain already-recorded feedback but must not mutate gameplay.
- [ ] Add bridge methods: `session_snapshot()`, `start_match()`, `set_paused(bool)` and `restart_match()`.
- [ ] Keep user pause independent from `Engine.time_scale`; the existing sim-speed control remains only for headless acceleration.
- [ ] `restart_match()` calls the existing clear + reseed path and returns the normal skirmish to Start.
- [ ] Expand reset cleanup for combat/session resources without introducing a second reset implementation.
- [ ] Add a bridge regression that repeats reset/start cycles and asserts initial resources/entity counts/indexes have no stale/duplicate entries.
- [ ] Add a same-tick regression with attackers able to threaten both Town Centers and assert the first resolved destruction cannot be overwritten.

**Runtime verification in this task:**

```bash
cargo test -p grus-sim
cargo test -p grus-godot --lib --locked
# stage extension
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
godot --headless --path godot res://scenes/economy_smoke_test.tscn
# run the existing benchmark scene/path and confirm units still move without a Start click
```

---

## Task 5: Add passive-opponent victory/defeat journeys in Rust

**Files:**
- Add/modify: `crates/grus-sim/src/combat/tests.rs`
- Add/modify: `crates/grus-sim/src/session/tests.rs`
- Reuse: `fixture`, `economy`, `buildings`, `production`, `commands`

- [ ] Build a test helper around the normal authored skirmish seed; do not add a runtime game mode or AI framework.
- [ ] Add an economy -> production -> army -> enemy Town Center destruction -> victory test using real command/system calls. The opponent remains passive.
- [ ] Add a separate defeat fixture where team 2 destroys team 1's Town Center through the same combat systems.
- [ ] Assert result freezes stockpiles, positions, construction, production progress, attack cooldowns and age-up progress across many subsequent ticks.
- [ ] Assert gameplay commands submitted after Result return/reveal `SessionLocked` and do not mutate state.

The test helper may accelerate fixed-step calls directly; it must not add resource cheats/debug-grant APIs to runtime code.

---

## Task 6: Wire Godot combat input, health/effects and lifecycle UI

**Files:**
- Modify: `crates/grus-godot/src/lib.rs`
- Modify: `godot/scripts/battlefield_controller.gd`
- Modify: `godot/scripts/unit_view.gd`
- Modify: `godot/scripts/building_view.gd`
- Modify: `godot/scenes/unit_view.tscn`
- Modify: `godot/scenes/building_view.tscn`
- Modify: `godot/scenes/main.tscn`
- Create only if useful: one small presentation helper script/scene for transient combat effects

- [ ] Add bridge call `attack_units(ids, target_kind: i32, target_id)` with the closed codes `0 = unit`, `1 = building`; Rust maps the code to `CombatTarget`. Do not use a string `target_kind` wire.
- [ ] Add `attack_move_units(ids, target)` using the existing ground target shape.
- [ ] Right-click enemy unit/building issues contextual Attack for selected military units. Ground right-click remains Move.
- [ ] Add `A` then ground-click AttackMove. Keep `S` Stop and existing selection/control groups.
- [ ] `Esc` cancels active placement first; otherwise toggles pause only between Playing/Paused.
- [ ] Disable gameplay-order emission in Start/Paused/Result even though the sim also rejects it authoritatively.
- [ ] Add primitive health bars to unit/building views. Update them from `Changed<Health>` through existing Godot node handles, not a per-frame GDScript scan of all entities.
- [ ] Style unit kinds with primitive scale/role-marker differences so villager/spearman/archer/cavalry are readable without art assets.
- [ ] Drain `CombatEvents` into short tracer/hit/death effects. Archer tracer is presentation-only and never controls hit timing.
- [ ] Add one tiny attack/hit sound cue without an audio framework.
- [ ] Add `SessionOverlay` + Pause button: Start; Paused/Resume; Victory or Defeat + Restart/Quit.
- [ ] On restart, clear transient effect nodes and local selection/control-group state before/while the bridge reseeds; do not retain dead stable IDs in Godot state.

---

## Task 7: Add the HPA-472 lifecycle smoke, preserve older gates and finish the PR

**Files:**
- Create: `godot/scripts/combat_lifecycle_smoke_test.gd`
- Create: `godot/scenes/combat_lifecycle_smoke_test.tscn`
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`

- [ ] Add a headless combat/lifecycle smoke through real bridge/controller-facing APIs: Start -> issue attack -> observe HP decrease -> observe death/view removal -> destroy Town Center -> Result -> reject gameplay input -> Restart -> initial state restored.
- [ ] Ensure the smoke distinguishes the three military role presentations without requiring final art.
- [ ] Re-run the earlier smoke/reset/economy gates already adapted atomically in Task 4; do not weaken their prior assertions.
- [ ] Add one HPA-472 smoke step to the existing `e2e` CI job with an evidence-based timeout. Do not create another workflow/job unless the current job becomes materially unmaintainable.
- [ ] Keep the Bevy E2E exported-game test as the small boot/selector contract; do not grow it into redundant full-match UI automation.
- [ ] Update README controls/lifecycle/verification commands and note that fog/economic AI remain HPA-473.
- [ ] Run the full local-equivalent gate set:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --locked
cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\\.rs$'
cargo test -p grus-godot --lib --locked
cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1
cargo build -p grus-godot --features e2e --locked
# stage the extension, import Godot, then run existing smoke/reset/economy + new combat lifecycle smoke
```

- [ ] Confirm Linux export/boot, Bevy E2E boot and the 200-unit benchmark still pass in CI.
- [ ] Keep the PR draft while implementation is incomplete; move to review only after every HPA-472 acceptance item is backed by tests/runtime evidence.

## Definition of done

- Military roles fight with the intended counter triangle and readable feedback.
- Contextual attack, attack-move and stop behave through the real input/bridge path.
- Building pursuit reaches walkable perimeter cells; melee range uses the footprint's closest point.
- Workers/buildings/sites die with all required economy, occupancy and queue cleanup semantics, including workers already traveling to a destroyed drop-off/site.
- Destroying the sole enemy/player Town Center yields victory/defeat, aborts the remainder of that combat step, and freezes later gameplay mutation.
- Start, pause/resume, result, restart and quit are usable without editor intervention.
- Missing session remains Playing for pure-sim tests; normal Godot skirmish starts at Start; benchmark remains immediately Playing.
- Restart produces one fresh match with no duplicate nodes/stale IDs.
- The production FixedUpdate chain and `system_order.rs` encode the same commands -> combat -> movement -> economy -> construction -> production order.
- Existing economy/CI/export/benchmark behavior remains green.
- `grus-sim` remains above the 90% production-line coverage gate.
- No HPA-473 visibility/AI work, new art, or unrelated framework work is pulled into this PR.
