# HPA-472 Combat and Match Lifecycle Design

## Status

Planning baseline for HPA-472. This design and its implementation plan live on the same branch and draft PR that will carry the implementation; there is no follow-up implementation PR for this ticket.

The design extends the merged HPA-470/HPA-471 seams and the current CI foundation. It deliberately avoids a second gameplay state model, projectile physics, an ability/armour framework, a generic event bus, or a separate lifecycle framework.

## Outcome

Turn the existing gather/build/train loop into a complete combat match. Military units can attack, pursue and attack-move; workers can be raided; buildings and construction sites can be destroyed; destroying the opposing starting Town Center resolves the match. The runtime gains start, pause/resume, result, restart and quit without editor intervention.

Fog/visibility and the real economic opponent remain HPA-473 work. HPA-472 may use a passive/scripted opponent fixture and full information only for verification.

## Existing seams we keep

- Bevy ECS remains the only mutable gameplay model; Godot owns input, rendering, HUD and effects.
- The simulation remains fixed-step at 20 Hz.
- `PlayerCommand` / `CommandResult` / `RejectReason` remain the command contract.
- `UnitIndex`, `BuildingIndex`, `ResourceIndex`, stable IDs and the authored skirmish fixture remain the identity seams.
- `GridMap::set_blocked` remains the only live walkability mutation seam.
- `MoveOrder`, A*, reservation-aware destination assignment and `assign_move_toward()` remain the movement/pursuit primitives.
- Population remains derived from live units and completed Town Center/House capacity.
- The bridge reset path remains the runtime restart seam.
- Existing CI keeps the >90% `grus-sim` line-coverage gate, Godot smokes, export validation, Bevy E2E boot test and 200-unit benchmark.

HPA-472 adds two focused simulation modules only: `combat.rs` and `session.rs`.

## Combat catalogue

Extend the compile-time catalogue instead of creating combat data elsewhere.

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
    Attack { target: CombatTarget },
    AttackMove {
        destination: GridPos,
        target: Option<CombatTarget>,
    },
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct AttackCooldown(pub f32);

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct PursuitState {
    pub last_target_cell: GridPos,
}
```

No weapon inventory, armour classes, damage types, abilities, formation state or projectile entities are added.

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

Add `RejectReason::NotCombatant`, `TargetMissing`, `InvalidTarget`, and `SessionLocked`.

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

Attack-move acquisition uses a fixed 8-world-unit radius. Choose the nearest eligible target; ties break by stable target identity. Units and buildings are eligible. There is no threat table or aggro framework.

## Range, pursuit and attacks

Range is measured from attacker position to the nearest point on the target:

- unit targets use their `SimPosition`;
- building/site targets use the closest point on their `Footprint`, not the footprint center.

This lets melee units attack from the building perimeter without entering blocked cells.

`step_combat()` runs before movement. Each tick it:

1. decrements cooldowns;
2. validates/refreshes current targets;
3. acquires a target for attack-move when one is in radius;
4. if in range and cooldown is ready, applies one deterministic hit;
5. otherwise assigns/refreshes pursuit using the existing movement seam;
6. after a target dies, direct Attack ends while AttackMove resumes toward its stored destination.

Pursuit replans when the target enters a different grid cell or the current pursuit route ends; it does not run A* every render frame.

Attackers are processed in stable `UnitId` order. No friendly fire. Archers apply damage immediately after validation; their projectile is presentation-only.

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

`combat.rs` owns terminal destruction helpers so every kill performs the same cleanup.

### Unit destruction

Before despawn:

- if the unit is a worker, call the existing worker-cancellation seam to release active-builder and Farm reservations;
- carried resources are simply lost with the dead worker;
- remove the stable `UnitId` from `UnitIndex`;
- despawn the entity, which removes movement/combat state and its presentation;
- stale selections disappear when Godot next reconciles live view IDs;
- stale combat targets resolve missing through the same target-eligibility path.

### Building/site destruction

Before despawn:

- free every footprint cell through `GridMap::set_blocked(cell, false)`;
- remove the building from `BuildingIndex`;
- if it is a Farm/resource entity, remove its `ResourceId` from `ResourceIndex`;
- active builders targeting the destroyed site become idle;
- the Farm's assigned worker becomes idle and keeps any current carry;
- workers whose stored drop-off was destroyed immediately try another reachable same-team drop-off; if none is reachable, they become visibly idle and keep their carry;
- a production queue disappears with the building, with no refund;
- House/Town Center population capacity falls automatically because capacity is derived from live completed buildings; living units are never deleted.

The existing sequential population recheck in `step_production()` remains the simultaneous-spawn safety mechanism.

## Match session

`session.rs` owns one resource:

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

Rules:

- normal runtime seed starts in `Start` with all match entities present but simulation frozen;
- Start transitions to Playing;
- Pause/Resume changes only `MatchSession`; do not use `Engine.time_scale` for user pause because camera/UI and CI speed controls must remain responsive/independent;
- every gameplay fixed-step system exits immediately unless phase is Playing;
- `apply_pending_commands` drains/rejects gameplay orders as `SessionLocked` outside Playing;
- destroying a Town Center resolves Result immediately; later systems in that same fixed tick see Result and do not mutate;
- Restart uses the existing clear + reseed seam and returns to Start;
- Quit remains a Godot scene-tree action.

Resume does not catch up paused wall-clock time because fixed ticks continue to occur while gameplay advancement is gated.

No draw state or alternate victory condition is introduced. The MVP has one Town Center per team; the first resolved starting Town Center destruction decides the result.

## Fixed-step order

The canonical HPA-472 order becomes:

```text
pending gameplay commands
→ combat (targeting / attacks / destruction / result)
→ movement
→ economy
→ construction
→ production
→ route feedback
```

Every stage after combat checks `MatchSession::Playing`. This makes a Town Center kill terminal in the same tick and prevents killed workers/buildings from gathering, constructing or producing afterward.

## Godot bridge and UI

Add thin bridge methods only:

- `attack_units(ids, target_kind, target_id)`
- `attack_move_units(ids, target)`
- `session_snapshot()`
- `start_match()`
- `set_paused(bool)`
- `restart_match()`

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
- pursuit and target-cell replan;
- deterministic attack-move acquisition and continuation;
- Stop / Move cancellation;
- target death and building/site damage;
- worker carry loss on death;
- site builder cleanup;
- destroyed Farm worker idle;
- destroyed drop-off reroute/idle behavior;
- footprint freeing;
- discarded queues/no refund;
- House capacity reduction without deleting living units;
- simultaneous production still respects capacity;
- pause freezes movement/economy/construction/production/combat/age-up state across many ticks;
- resume advances once per fixed tick without catch-up;
- result rejects gameplay commands and freezes later systems;
- passive-opponent economy -> army -> Town Center destruction -> victory journey;
- separate defeat fixture.

### Godot/runtime

Add one `combat_lifecycle_smoke_test.gd` that uses real controller/bridge paths to exercise attack input, health reduction/death view cleanup, result overlay and restart. Update earlier economy/reset smokes to call Start where needed.

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
