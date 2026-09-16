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
- All gameplay stages and gameplay-command application are gated by `MatchSession::Playing`.
- Use one combat target-eligibility function. HPA-473 must be able to add visibility there instead of rewriting combat.
- Reuse `MoveOrder`, `assign_move_toward()`, A*, stable IDs, `GridMap::set_blocked`, current indexes and the authored fixture.
- Do not pathfind every frame. Pursuit refreshes only when a target changes grid cell or its route ends.
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

- [ ] Add failing catalogue tests for military combat specs, the three counter relationships, villager noncombatant status, and building health.
- [ ] Extend `UnitSpec` with `max_health` + `Option<CombatSpec>` and `BuildingSpec` with `max_health` using the design values.
- [ ] Add `Health`, `CombatTarget`, `CombatOrder`, `AttackCooldown`, `PursuitState`, `CombatEvent`, `CombatEvents` and the `MatchPhase` / `MatchResult` / `MatchSession` contracts.
- [ ] Extend `UnitCommandKind` with `AttackMove` and `PlayerCommand` with direct `Attack`.
- [ ] Add typed rejects: `NotCombatant`, `TargetMissing`, `InvalidTarget`, `SessionLocked`.
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

## Task 2: Implement direct attack, attack-move, targeting and pursuit

**Files:**
- Modify: `crates/grus-sim/src/combat.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/movement.rs` only if a narrow helper is needed
- Add/modify: `crates/grus-sim/src/combat/tests.rs`
- Add/modify: `crates/grus-sim/src/commands/tests.rs`

- [ ] Write failing tests for direct target validation, villager rejection, enemy-only targeting, melee/ranged range, cooldown, all three counter bonuses and building base damage.
- [ ] Implement the single `target_eligible(world, attacker_team, target)` seam. Do not duplicate eligibility checks in attack-move acquisition.
- [ ] Implement deterministic target distance: point-to-point for units; nearest point on `Footprint` for buildings/sites.
- [ ] Implement direct Attack acceptance. Validate first, then cancel old worker/movement/combat activity for accepted units.
- [ ] Implement AttackMove acceptance and an 8-unit acquisition radius; nearest target wins with stable-ID tie breaking.
- [ ] Implement `step_combat()` cooldown, target refresh, hit application and pursuit assignment.
- [ ] Reuse `assign_move_toward()` for pursuit. Track target cell to avoid A* every tick.
- [ ] Direct Attack ends on missing/dead target. AttackMove clears the target and resumes toward its stored destination.
- [ ] Update Move and Stop so accepted commands clear combat intent; Stop still performs existing worker cleanup.
- [ ] Record hit data in `CombatEvents`; do not add Godot behavior yet.

**Required tests:**

- melee does not hit outside range and hits once inside range;
- Archer hits at range and respects cooldown;
- Spearman > Cavalry, Archer > Spearman, Cavalry > Archer bonuses;
- pursuit creates/reuses movement and refreshes after target cell change;
- AttackMove acquires then continues after target death;
- Stop cancels pursuit/attack;
- normal Move replaces combat intent;
- buildings/construction sites accept base damage.

---

## Task 3: Make destruction atomic and preserve economy/production invariants

**Files:**
- Modify: `crates/grus-sim/src/combat.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/buildings.rs`
- Modify: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/production.rs` only where tests expose a real invariant gap
- Add/modify: focused module tests

- [ ] Add `UnitIndex::remove()` and `BuildingIndex::remove()` following the existing `ResourceIndex::remove()` pattern.
- [ ] Write destruction tests before implementation for worker carry loss, Farm worker idle, destroyed drop-off reroute, no-valid-dropoff idle, active builder cleanup, freed footprint, discarded production queue and House capacity reduction.
- [ ] Implement one unit-destruction helper: release worker assignments first, remove index entry, then despawn. Do not refund Carry.
- [ ] Implement one building-destruction helper: free footprint, clean indexes/resources, retask affected workers, then despawn.
- [ ] When a Farm dies, cancel its assigned worker to Idle while preserving any carry.
- [ ] When a stored drop-off dies, reroute affected carrying workers immediately to another reachable same-team drop-off; otherwise full worker cleanup to visible Idle while preserving carry.
- [ ] When a construction site dies, its active builder becomes Idle.
- [ ] Rely on derived `population_cap()` after House/TC removal; do not delete units above the new cap.
- [ ] Keep existing sequential spawn completion as the simultaneous-cap safety mechanism and add one regression test at the reduced cap.
- [ ] Ensure stale combat targets clear through the stable-index/eligibility seam rather than scanning every order during destruction.

**Focused verification:**

```bash
cargo test -p grus-sim combat
cargo test -p grus-sim economy
cargo test -p grus-sim production
```

---

## Task 4: Add authoritative match result and simulation gating

**Files:**
- Modify: `crates/grus-sim/src/session.rs`
- Modify: `crates/grus-sim/src/combat.rs`
- Modify: `crates/grus-godot/src/lib.rs`
- Add/modify: Rust session tests and bridge library tests

- [ ] Write failing session tests for Start freeze, Playing advancement, Paused freeze, Resume without catch-up, Result freeze and command rejection outside Playing.
- [ ] Insert `MatchSession::Start` during normal skirmish setup/reset.
- [ ] Resolve `MatchPhase::Result` immediately when a Town Center is destroyed, recording winner/loser teams.
- [ ] Gate `apply_pending_commands` with `SessionLocked` when phase is not Playing.
- [ ] Gate combat/movement/economy/construction/production advance functions on Playing; route feedback may still drain already-recorded feedback but must not mutate gameplay.
- [ ] Add bridge methods: `session_snapshot()`, `start_match()`, `set_paused(bool)` and `restart_match()`.
- [ ] Keep user pause independent from `Engine.time_scale`; the existing sim-speed control remains only for headless acceleration.
- [ ] `restart_match()` calls the existing clear + reseed path and returns to Start.
- [ ] Expand reset cleanup for combat/session resources without introducing a second reset implementation.
- [ ] Add a bridge regression that repeats reset/start cycles and asserts initial resources/entity counts/indexes have no stale/duplicate entries.

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

- [ ] Add bridge calls for direct Attack and AttackMove using stable target IDs; Godot never computes damage/range/eligibility.
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

## Task 7: Add Godot lifecycle smoke, preserve older gates and finish the PR

**Files:**
- Create: `godot/scripts/combat_lifecycle_smoke_test.gd`
- Create: `godot/scenes/combat_lifecycle_smoke_test.tscn`
- Modify: earlier smoke scripts only where Start lifecycle requires it
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`

- [ ] Add a headless combat/lifecycle smoke through real bridge/controller-facing APIs: Start -> issue attack -> observe HP decrease -> observe death/view removal -> destroy Town Center -> Result -> reject gameplay input -> Restart -> initial state restored.
- [ ] Ensure the smoke distinguishes the three military role presentations without requiring final art.
- [ ] Update existing smoke/reset/economy tests to enter Playing explicitly when needed; do not weaken their prior assertions.
- [ ] Add one HPA-472 smoke step to the existing `e2e` CI job with an evidence-based timeout. Do not create another workflow/job unless the current job becomes materially unmaintainable.
- [ ] Keep the Bevy E2E exported-game test as the small boot/selector contract; do not grow it into redundant full-match UI automation.
- [ ] Update README controls/lifecycle/verification commands and note that fog/economic AI remain HPA-473.
- [ ] Run the full local-equivalent gate set:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --locked
cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\.rs$'
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
- Workers/buildings/sites die with all required economy, occupancy and queue cleanup semantics.
- Destroying the sole enemy/player Town Center yields victory/defeat and immediately freezes gameplay mutation.
- Start, pause/resume, result, restart and quit are usable without editor intervention.
- Restart produces one fresh match with no duplicate nodes/stale IDs.
- Existing economy/CI/export/benchmark behavior remains green.
- `grus-sim` remains above the 90% production-line coverage gate.
- No HPA-473 visibility/AI work, new art, or unrelated framework work is pulled into this PR.
