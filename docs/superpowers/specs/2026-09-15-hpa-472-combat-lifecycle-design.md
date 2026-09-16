# HPA-472 Combat and Match Lifecycle Design

## Status

Reviewed planning baseline for HPA-472. This design and its implementation plan live on the same branch and draft PR that will carry the implementation; there is no follow-up implementation PR for this ticket.

The design extends the merged HPA-470/HPA-471 seams and the current CI foundation. It deliberately avoids a second gameplay state model, projectile physics, an ability/armour framework, a generic event bus, or a separate lifecycle framework.

The review pass locks four integration details before implementation: building pursuit must target walkable perimeter cells rather than blocked footprints; session absence means Playing for existing pure-sim tests while only the normal Godot skirmish starts at Start; destruction retasks every worker that references a destroyed building; and the canonical system-order test/production chain are updated together with combat.

## Outcome

Turn the existing gather/build/train loop into a complete combat match. Military units can attack, pursue and attack-move; workers can be raided; buildings and construction sites can be destroyed; destroying the opposing starting Town Center resolves the match. The runtime gains start, pause/resume, result, restart and quit without editor intervention.

Fog/visibility and the real economic opponent remain HPA-473 work. HPA-472 may use a passive/scripted opponent fixture and full information only for verification.

## Existing seams we keep

- Bevy ECS remains the only mutable gameplay model; Godot owns input, rendering, HUD and effects.
- The simulation remains fixed-step at 20 Hz.
- `PlayerCommand` / `CommandResult` / `RejectReason` remain the command contract.
- `UnitIndex`, `BuildingIndex`, `ResourceIndex`, stable IDs and the authored skirmish fixture remain the identity seams.
- `GridMap::set_blocked` remains the only live walkability mutation seam.
- `MoveOrder`, A*, reservation-aware destination assignment and `assign_move_toward()` remain the movement primitives. Unit pursuit may target an enemy unit cell directly; building pursuit first chooses a walkable immediate-perimeter cell.
- Population remains derived from live units and completed Town Center/House capacity.
- The bridge reset path remains the runtime restart seam.
- Existing CI keeps the >90% `grus-sim` line-coverage gate, Godot smokes, export validation, Bevy E2E boot test and 200-unit benchmark.

HPA-472 adds two focused simulation modules only: `combat.rs` and `session.rs`.

## Combat catalogue

Extend the compile-time catalogue instead of creating combat tuning elsewhere.

```rust
pub struct UnitSpec {
    // existing economy fields...
    pub max_health: u32,
    pub combat: Option<CombatSpec>,
}

pub struct CombatSpec {
    pub damage: u32,
    pub attack_range: f32,
    pub cooldown_seconds: f32,
    pub counter_target: UnitKind,
    pub counter_bonus: u32,
    pub ranged: bool,
}

pub struct BuildingSpec {
    // existing construction fields...
    pub max_health: u32,
}

pub const ATTACK_MOVE_RADIUS: f32 = 8.0;
```

Initial HPA-472 tuning is intentionally simple and remains balanceable later in HPA-474:

| Unit | HP | Base damage | Range | Cooldown | Counter |
| --- | ---: | ---: | ---: | ---: | --- |
| Villager | 50 | — | — | — | noncombatant |
| Spearman | 100 | 10 | 1.5 | 1.0 s | +10 vs Cavalry |
| Archer | 70 | 8 | 6.0 | 1.25 s | +8 vs Spearman |
| Cavalry | 140 | 12 | 1.5 | 1.0 s | +12 vs Archer |

Building HP starts at: Town Center 800, House 250, Storehouse 300, Farm 200, Barracks 400, Archery Range 400, Stable 400.

Counter bonuses apply only to the named unit kind. Buildings receive base damage only. Construction sites use the owning building's full HP from placement; construction progress does not heal or scale HP in this slice.

## Authoritative combat components

```rust
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct Health {
    pub current: u32,
    pub max: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CombatTarget {
    Unit(UnitId),
    Building(BuildingId),
}

#[derive(Component, Clone, Debug, PartialEq)]
pub enum CombatOrder {
    Attack {
        target: CombatTarget,
        last_target_cell: Option<GridPos>,
    },
    AttackMove {
        destination: GridPos,
        target: Option<CombatTarget>,
        last_target_cell: Option<GridPos>,
    },
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct AttackCooldown(pub f32);
```

The last pursued target cell lives on the combat order; do not add a separate `PursuitState` component for one cached value. No weapon inventory, armour classes, damage types, abilities, formation state or projectile entities are added.

`spawn_unit()` inserts catalogue health/cooldown state. Seeded and placed buildings insert catalogue health. Production therefore automatically creates combat-ready units without another spawn path.

## Commands and cancellation

Add only the commands HPA-472 owns:

```rust
pub enum UnitCommandKind {
    Move { target: Vec2 },
    AttackMove { target: Vec2 },
    Stop,
}

pub enum PlayerCommand {
    // existing variants...
    Attack {
        issuer: TeamId,
        units: Vec<UnitId>,
        target: CombatTarget,
    },
}
```

Append `RejectReason::NotCombatant`, `TargetMissing`, `InvalidTarget`, and `SessionLocked` to the existing enum. The bridge exposes reject discriminants as integer codes, so existing variants are never reordered and `Locked` is not reused for session/combat failures.

Rules:

- Direct Attack validates target existence/enemy ownership through one combat-owned eligibility function, then accepts military units only.
- Attack-move accepts military units only and stores the destination cell.
- Move, Stop and accepted worker-task commands cancel existing combat orders.
- Direct Attack and AttackMove cancel accepted workers' prior worker activity only after combat validation succeeds.
- Stop clears `MoveOrder`, worker activity and `CombatOrder`.
- Mixed selections report per-unit rejects; a villager is never silently converted into a combat unit.

## One target-eligibility seam

All direct attacks and attack-move acquisition call one function in `combat.rs`:

```rust
fn target_eligible(world: &World, attacker_team: TeamId, target: CombatTarget) -> bool
```

For HPA-472 it checks: target exists, target has live `Health`, and target belongs to another team. HPA-473 extends this one seam with current-visibility rules; combat code and commands must not perform independent hidden-state checks elsewhere.

Attack-move acquisition uses `ATTACK_MOVE_RADIUS`. Choose the nearest eligible target; ties break by stable target identity. Units and buildings are eligible. There is no threat table or aggro framework.

## Range, pursuit and attacks

Range is measured from attacker position to the nearest point on the target:

- unit targets use their `SimPosition`;
- building/site targets use `Footprint::closest_point(attacker_position)`, not `Footprint::center()`.

Add `Footprint::closest_point(Vec2) -> Vec2` beside the existing `center()` / perimeter helpers. This lets melee units attack from the building perimeter without entering blocked cells; a 4x4 Town Center center is deliberately not the melee distance reference.

`step_combat()` runs before movement. Each tick it:

1. decrements cooldowns;
2. validates/refreshes current targets;
3. acquires a target for attack-move when one is in `ATTACK_MOVE_RADIUS`;
4. if in range and cooldown is ready, applies one deterministic hit;
5. otherwise assigns/refreshes pursuit;
6. after a target dies, direct Attack ends while AttackMove resumes toward its stored destination.

Pursuit rules reuse existing movement machinery without ever pathing to a blocked building footprint:

- if the attacker is already in attack range, do not assign a movement route;
- unit-vs-unit pursuit may call `assign_move_toward()` with the target unit's walkable cell;
- building/site pursuit derives currently reserved cells through the existing reservation seam, picks a walkable immediate-perimeter goal with `approach_slots()`, then calls `assign_move_toward()` using that walkable goal;
- pursuit refreshes only when a moving unit target changes grid cell or the existing pursuit route ends; static buildings do not trigger per-tick A*.

Attackers are processed in stable `UnitId` order. No friendly fire. Archers apply damage immediately after validation; their projectile is presentation-only.

Unit death is part of the hit path, not deferred to a later half-despawn stage. `destroy_unit()` removes the stable index entry and despawns atomically so AttackMove can observe target death and resume in the same combat implementation slice.

When a Town Center kill resolves `MatchPhase::Result`, `step_combat()` returns immediately. No later attacker in that combat step may destroy the other Town Center and overwrite the first result; the MVP has no draw state.

## Combat feedback

Use one small sim-owned queue, not a generic event subsystem:

```rust
#[derive(Resource, Default)]
pub struct CombatEvents(pub Vec<CombatEvent>);
```

Events contain attacker/target identity, damage, hit position, ranged/melee and killed flags. The Godot bridge drains them during presentation update.

Godot uses these events for:

- a short archer shot tracer plus hit flash; no projectile physics;
- melee/ranged hit feedback;
- death feedback before the entity view disappears;
- one basic attack/hit sound cue. Keep this to a tiny project-local cue; no audio pipeline.

## Destruction is one cleanup path

`combat.rs` owns terminal destruction helpers so every kill performs the same cleanup. Narrow economy helpers may be exposed where destruction needs existing drop-off routing; do not duplicate routing logic inside combat.

### Unit destruction

Before despawn:

- if the unit is a worker, call the existing worker-cancellation seam to release active-builder and Farm reservations;
- carried resources are simply lost with the dead worker;
- remove the stable `UnitId` from `UnitIndex`;
- despawn the entity, which removes movement/combat state and its presentation;
- stale selections disappear when Godot next reconciles live view IDs;
- stale combat targets resolve missing through the same target-eligibility path.

### Building/site destruction

Capture the building footprint/resource identity first, then perform one destruction transaction:

- free every footprint cell through `GridMap::set_blocked(cell, false)`;
- remove the building from `BuildingIndex`;
- if it is a Farm/resource entity, remove its `ResourceId` from `ResourceIndex`;
- scan every live worker whose `WorkerTask` references the destroyed `BuildingId` rather than cleaning only `active_builder`;
- `ToConstruction` / `Constructing` workers are canceled to Idle;
- every `ToDropoff { dropoff: destroyed_id, ... }` worker immediately reroutes its preserved carry to another reachable same-team drop-off through the existing economy routing seam; if none is reachable, it becomes visibly Idle and keeps the carry;
- a destroyed Farm's assigned worker becomes Idle immediately and keeps any current carry, regardless of whether it was moving to the Farm, gathering, or returning a Farm-sourced load;
- the production queue disappears with the building, with no refund;
- House/Town Center population capacity falls automatically because capacity is derived from live completed buildings; living units are never deleted;
- finally despawn the building entity.

Do not call the standalone-resource `deplete_source()` path for a Farm: Farms are 2x2 `Building + ResourceSource + Footprint` entities and use building destruction.

Because combat/destruction runs before economy/construction, no worker may remain with a stale `ToDropoff` and deposit at an empty slot, and no worker may transition `ToConstruction -> Constructing` for a site already destroyed in that tick.

The existing sequential population recheck in `step_production()` remains the simultaneous-spawn safety mechanism.

## Match session

`session.rs` owns one optional resource:

```rust
pub enum MatchPhase {
    Start,
    Playing,
    Paused,
    Result(MatchResult),
}

pub struct MatchResult {
    pub winner: TeamId,
    pub loser: TeamId,
}

#[derive(Resource)]
pub struct MatchSession {
    pub phase: MatchPhase,
}
```

Session presence is a runtime lifecycle concern, not a requirement of every pure simulation fixture. The gate contract is:

- missing `MatchSession` means Playing. Existing Rust tests/fixtures that call `seed_skirmish()` and `step_*()` remain valid without lifecycle setup;
- `seed_skirmish()` itself does not insert `MatchSession`;
- normal Godot `setup_fixture` / `reset_fixture_world` insert `MatchSession::Start` after seeding;
- `reset_benchmark_world` explicitly inserts Playing (or otherwise ensures no Start resource remains) so the 200-unit benchmark keeps moving without a Start click;
- Start transitions to Playing;
- Pause/Resume changes only `MatchSession`; do not use `Engine.time_scale` for user pause because camera/UI and CI speed controls must remain responsive/independent;
- every gameplay fixed-step system treats absent session as Playing and exits when a present session is Start/Paused/Result;
- `apply_pending_commands` uses the same rule and rejects gameplay orders as `SessionLocked` only when a present session is outside Playing;
- destroying a Town Center resolves Result immediately; `step_combat()` aborts the rest of that combat step and later fixed-step systems do not mutate;
- Restart uses the existing clear + reseed seam and returns the normal skirmish to Start;
- Quit remains a Godot scene-tree action.

Resume does not catch up paused wall-clock time because fixed ticks continue to occur while gameplay advancement is gated.

No draw state or alternate victory condition is introduced. The MVP has one Town Center per team; the first resolved starting Town Center destruction decides the result.

## Fixed-step order

The canonical HPA-472 order becomes:

```text
pending gameplay commands
-> combat (targeting / attacks / destruction / result)
-> movement
-> economy
-> construction
-> production
-> route feedback
```

The production `FixedUpdate` chain and `crates/grus-sim/tests/system_order.rs` are changed in the same combat integration task. The order test explicitly calls `step_combat()` before movement so the test remains an executable statement of production order.

Every stage after combat uses the same optional-session gate. This makes a Town Center kill terminal in the same tick and prevents killed workers/buildings from gathering, constructing or producing afterward.

## Godot bridge and UI

Add thin bridge methods only:

- `attack_units(ids, target_kind: i32, target_id)` where `target_kind` is the closed wire code `0 = unit`, `1 = building`;
- `attack_move_units(ids, target)`;
- `session_snapshot()`;
- `start_match()`;
- `set_paused(bool)`;
- `restart_match()`.

The integer target kind mirrors the existing integer reject-code boundary and avoids string parsing in controller/smoke code. Rust converts the closed code to `CombatTarget`; Godot still does not compute eligibility.

Extend building snapshots with health fields and add a small unit health query only where HUD tests need it. Dynamic world health bars are updated from `Changed<Health>` through the existing Godot-node handles instead of polling every unit from GDScript.

`battlefield_controller.gd` changes:

- right-click enemy unit/building -> contextual Attack for selected military units;
- ground right-click remains Move;
- `A` then ground click issues AttackMove;
- `S` remains Stop;
- `Esc` cancels placement first, otherwise toggles pause while Playing/Paused;
- gameplay orders are not issued in Start/Paused/Result.

`main.tscn` gains one small session overlay and a Pause button. Start shows Start; Paused shows Resume; Result shows Victory/Defeat + Restart/Quit.

Primitive presentation distinguishes combat roles without image assets: reuse the current low-poly unit scene and vary body scale/role marker by `unit_kind`; add simple health bars. No image-generation task is required for HPA-472.

## Verification strategy

### Rust simulation

Cover production behavior, not duplicated test algorithms:

- melee/ranged range and cooldown;
- all three counter bonuses;
- unit pursuit and target-cell replan;
- building pursuit reaches an immediate-perimeter cell and never paths to a blocked footprint;
- `Footprint::closest_point` drives building range, including Town Center melee range;
- deterministic attack-move acquisition and continuation after unit target destruction;
- Stop / Move cancellation;
- target death and building/site damage;
- worker carry loss on death;
- site builder cleanup;
- destroyed Farm worker idle;
- destroyed drop-off reroute/idle behavior, including a worker already traveling to that drop-off;
- footprint freeing;
- discarded queues/no refund;
- House capacity reduction without deleting living units;
- simultaneous production still respects capacity;
- missing session behaves as Playing for existing pure-sim tests;
- Start/Paused freeze movement/economy/construction/production/combat/age-up state across many ticks;
- benchmark reset remains Playing;
- resume advances once per fixed tick without catch-up;
- first Town Center destruction wins and aborts remaining attackers in that combat step;
- Result rejects gameplay commands and freezes later systems;
- passive-opponent economy -> army -> Town Center destruction -> victory journey;
- separate defeat fixture.

### Godot/runtime

Add one `combat_lifecycle_smoke_test.gd` that uses real controller/bridge paths to exercise Start, attack input, health reduction/death view cleanup, result overlay and restart.

When normal skirmish setup begins inserting Start, update the existing reset/economy/controller smokes to call `start_match()` in the same commit. Do not leave existing gates frozen until the final CI task. The 200-unit benchmark remains immediately Playing.

The existing Bevy E2E boot selector test remains a boot contract; do not expand it into full UI automation in this ticket unless the new smoke cannot cover a required seam.

### CI

Preserve:

- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-targets -- -D warnings`;
- >90% `grus-sim` gameplay line coverage;
- `grus-godot` tests with and without the E2E feature;
- current Godot smoke/reset/economy gates;
- Linux export/boot;
- Bevy E2E exported-game boot;
- 200-unit benchmark.

Add only the HPA-472 combat/lifecycle Godot smoke to the existing `e2e` job.

## Explicit non-goals

No fog/visibility implementation, economic AI, additional units/buildings, siege, armour system, projectile physics, formations, abilities, garrisons, Town Center weapon, save/load, multiplayer, replay, animation pipeline, custom asset pipeline, new image art, generic event framework, service/repository architecture, or final balance/performance tuning.
