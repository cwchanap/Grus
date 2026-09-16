# HPA-472 Combat and Match Lifecycle Design

## Status

Reviewed planning baseline for HPA-472. This design and its implementation plan live on the same branch and draft PR that will carry the implementation; there is no follow-up implementation PR for this ticket.

The design extends the merged HPA-470/HPA-471 seams and current CI foundation. It deliberately avoids a second gameplay state model, projectile physics, armour/damage frameworks, a generic event bus, or a separate lifecycle framework.

Two review passes are incorporated. The second review's proposal to remove `MatchPhase::Start` is intentionally **not** adopted because HPA-472 explicitly requires a `Start / Playing / Paused / Result` session flow. The remaining valid findings are folded in below: pause interpolation bookkeeping, stronger counter-contract tests/tuning, centralized activity cancellation, liveness-safe combat iteration, nearest building pursuit, executable tick-order coverage, bridge-kind consistency, bounded combat events, bounded journey tests, and earlier runtime smoke coverage.

## Outcome

Turn the existing gather/build/train loop into a complete combat match. Military units attack, pursue and attack-move; workers can be raided; buildings and construction sites can be destroyed; destroying the opposing starting Town Center resolves the match. The runtime supports Start, pause/resume, result, restart and quit without editor intervention.

Fog/visibility and the real economic opponent remain HPA-473 work. HPA-472 may use a passive opponent fixture and full information only for verification.

## Existing seams we keep

- Bevy ECS is the only mutable gameplay model; Godot owns input, rendering, HUD and effects.
- Simulation remains fixed-step at 20 Hz.
- `PlayerCommand` / `CommandResult` / append-only `RejectReason` remain the command contract.
- `UnitIndex`, `BuildingIndex`, `ResourceIndex`, stable IDs and the authored skirmish fixture remain identity seams.
- `GridMap::set_blocked` remains the live walkability mutation seam.
- `MoveOrder`, A*, reservation-aware destination assignment, `assign_move_toward()` and `approach_slots()` remain movement primitives.
- Population remains derived from live units and completed Town Center/House capacity.
- The existing clear + reseed bridge path remains the restart seam.
- Existing CI keeps the >90% `grus-sim` production-line coverage gate, Godot smokes, export validation, Bevy E2E boot and 200-unit benchmark.

HPA-472 adds only two focused simulation modules: `combat.rs` and `session.rs`.

## Combat catalogue

Extend the compile-time catalogue instead of scattering combat tuning.

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

Initial HPA-472 tuning is deliberately small and remains balanceable in HPA-474:

| Unit | HP | Base damage | Range | Cooldown | Counter |
| --- | ---: | ---: | ---: | ---: | --- |
| Villager | 50 | — | — | — | noncombatant |
| Spearman | 100 | 10 | 1.5 | 1.0 s | +10 vs Cavalry |
| Archer | 70 | 8 | 6.0 | 1.25 s | **+12 vs Spearman** |
| Cavalry | 140 | 12 | 1.5 | 1.0 s | +12 vs Archer |

The Archer bonus is intentionally +12 rather than +8 so the Archer > Spearman leg is carried by counter damage, not a fragile opening-shot timing window. The contract is verified by real symmetric attack-move duels for all three pairs, not only by asserting catalogue relationships.

Building HP: Town Center 800, House 250, Storehouse 300, Farm 200, Barracks 400, Archery Range 400, Stable 400.

Counter bonuses apply only to the named unit kind. Buildings receive base damage only. Construction sites use full owning-building HP from placement; construction progress does not scale or heal HP.

## Authoritative combat state

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

The cached pursued cell lives on `CombatOrder`; there is no separate pursuit component. `spawn_unit()` inserts catalogue health/cooldown state. Seeded and placed buildings insert catalogue health.

## One activity-cancellation seam

The existing `cancel_worker_activity()` is already the one route/task cancellation path and is called by Move, Stop, Gather and placement. HPA-472 promotes it to the general unit-retask seam:

```rust
cancel_unit_activity(world, entity)
```

It preserves Carry and performs the existing worker cleanup, resets gather progress, removes `MoveOrder`, and also removes `CombatOrder`.

Rules:

- accepted Move/Stop/Gather/Place/Resume automatically cancel combat intent through this one helper;
- Direct Attack and AttackMove validate first, then call the same helper before installing their combat order;
- rejected commands leave the old worker/movement/combat state untouched;
- no command site gets its own ad-hoc `CombatOrder` removal.

This keeps future retasking behavior symmetric without introducing another service/helper layer.

## Commands and eligibility

Add only HPA-472-owned commands:

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

Append `NotCombatant`, `TargetMissing`, `InvalidTarget`, and `SessionLocked` to `RejectReason`; never reorder existing discriminants and do not reuse `Locked`.

All direct attacks and attack-move acquisition call one combat-owned function:

```rust
fn target_eligible(world: &World, attacker_team: TeamId, target: CombatTarget) -> bool
```

For HPA-472 it checks: target exists, has live Health, and belongs to another team. HPA-473 adds current-visibility there rather than rewriting combat.

Attack-move searches within `ATTACK_MOVE_RADIUS`, chooses the nearest eligible target, and breaks ties by stable target identity. Units and buildings are eligible. There is no threat table or aggro framework.

## Range and pursuit

Range is measured from attacker position to the nearest point on the target:

- unit target: its `SimPosition`;
- building/site target: `Footprint::closest_point(attacker_position)`.

Add:

```rust
Footprint::closest_point(Vec2) -> Vec2
```

beside the existing center/perimeter helpers. `Footprint::center()` is never the melee building range metric.

Pursuit reuses movement but never paths to a blocked footprint:

- if already in attack range, do not assign movement;
- unit pursuit may call `assign_move_toward()` with the target unit's walkable cell;
- building/site pursuit calls `approach_slots()`, then **sorts those candidates by distance from the attacker at the combat call site** before selecting a pathable goal;
- do not change `approach_slots()` ordering globally because economy/construction already rely on it;
- refresh unit pursuit only when the target changes grid cell or the current route ends; static building pursuit refreshes only when its route ends.

## `step_combat()` iteration and liveness

`step_combat()` runs before movement. At entry it clears `CombatEvents`, making the buffer current-tick-only and bounded even in headless tests.

Each tick it:

1. decrements cooldowns;
2. snapshots attacker stable IDs in sorted `UnitId` order;
3. before each attacker acts, re-resolves that ID through `UnitIndex` and skips it if the entity was destroyed earlier in the same step;
4. validates/refreshes the current target;
5. acquires an attack-move target when needed;
6. attacks immediately when in range and cooldown-ready, otherwise refreshes pursuit;
7. atomically destroys a dead unit through `destroy_unit()`;
8. after target death, Direct Attack ends while AttackMove resumes toward its destination.

This snapshot + liveness re-check prevents a later attacker from touching an entity despawned by an earlier attacker. A regression uses two lethal low-health opponents: the lower stable-ID attacker kills first and the dead later attacker is skipped rather than acting or panicking.

Archers apply validated damage immediately; projectile visuals never control hit timing. No friendly fire.

When a Town Center kill resolves Result, `step_combat()` returns immediately. No later attacker in the same step may overwrite the first result; there is no draw state.

## Combat feedback

Use one tiny sim-owned drain buffer:

```rust
#[derive(Resource, Default)]
pub struct CombatEvents(pub Vec<CombatEvent>);
```

`step_combat()` clears it at the start of each fixed tick, then records only that tick's hit/death events. At normal runtime cadence Godot drains current-tick events during presentation Update. Headless/accelerated tests cannot accumulate thousands of stale events; accelerated presentation may intentionally drop intermediate cosmetic events.

Events carry attacker/target identity, damage, hit position, ranged/melee and killed flags. Godot uses them for a short archer tracer, hit/death flash and one small attack/hit sound cue. No projectile or audio framework.

## Destruction

`combat.rs` owns terminal destruction helpers. Narrow economy helpers may expose existing drop-off routing; do not duplicate routing logic in combat.

### Unit destruction

- call `cancel_unit_activity()` first so worker/Farm/build assignments are released;
- carried resources disappear with the dead worker;
- remove the `UnitId` from `UnitIndex`;
- despawn the entity;
- stale selections disappear when Godot reconciles live views;
- stale combat targets resolve missing through `target_eligible`.

### Building/site destruction

Capture footprint/resource identity, then perform one transaction:

- free every footprint cell through `GridMap::set_blocked(cell, false)`;
- remove from `BuildingIndex`;
- if a Farm/resource entity, remove its `ResourceId` from `ResourceIndex`;
- scan **every** live `WorkerTask` that references the destroyed `BuildingId`;
- `ToConstruction` / `Constructing` workers cancel to Idle;
- `ToDropoff { dropoff: destroyed_id, ... }` carrying workers immediately try another reachable same-team drop-off using the existing routing seam; otherwise Idle while preserving Carry;
- a destroyed Farm's assigned worker becomes Idle and keeps Carry whether moving to it, gathering, or returning a Farm-sourced load;
- production queue disappears with no refund;
- derived population capacity falls naturally; living units are never deleted;
- despawn the building.

Do not call standalone-resource `deplete_source()` for a Farm; a Farm is a 2x2 `Building + ResourceSource + Footprint` entity.

Combat-before-economy/construction is a correctness contract: a worker may not deposit at a just-destroyed drop-off slot or transition to Constructing for a destroyed site in the same tick. `system_order.rs` gets a dedicated same-tick destroyed-drop-off test that fails if combat is moved after economy.

## Match session

HPA-472 explicitly requires Start/Playing/Paused/Result, so keep all four phases:

```rust
pub enum MatchPhase {
    Start,
    Playing,
    Paused,
    Result(MatchResult),
}
```

`MatchSession` remains optional so old pure-sim fixtures and the benchmark stay simple:

- missing session means Playing;
- `seed_skirmish()` does not insert a session;
- normal Godot setup/reset inserts Start;
- benchmark reset inserts Playing (or removes any stale session);
- Start -> Playing is explicit through `start_match()`;
- Pause/Resume changes only session state; never user-pause via `Engine.time_scale`;
- gameplay commands reject with `SessionLocked` when an explicit session is Start/Paused/Result;
- combat/economy/construction/production do no gameplay mutation outside Playing;
- Restart uses clear + reseed and returns the normal skirmish to Start;
- Quit remains a Godot scene-tree action.

### Pause/result interpolation bookkeeping

Godot renders unit transforms by lerping `SimPosition.previous -> current` using the continuously cycling physics interpolation fraction. Therefore a frozen unit must not retain `previous != current` indefinitely.

`step_movement()` owns the fix: when the optional-session gate says gameplay is frozen, it still snapshots each live unit's position and sets `previous = current` before returning without movement. This applies to Start, Paused and Result. A session regression asserts `previous == current` after one gated movement tick.

Resume does not catch up paused wall-clock time because fixed ticks continue while gameplay mutation is gated.

## Fixed-step order

Canonical order:

```text
pending gameplay commands
-> combat
-> movement
-> economy
-> construction
-> production
-> route feedback
```

The production FixedUpdate chain and `crates/grus-sim/tests/system_order.rs` change together. `system_order.rs` contains both:

1. the existing long-form economy/age ordering test with `step_combat()` inserted; and
2. a targeted same-tick destruction test where a carrying worker reaches a drop-off as combat destroys it, proving combat must run before economy because stockpile must not receive a deposit at the dead slot.

## Godot bridge and input

Follow the bridge's existing input-kind convention (`place_building(kind: GString)`, `enqueue_unit(kind: GString)`):

- `attack_units(ids, target_kind: GString, target_id)`;
- `attack_move_units(ids, target)`;
- `session_snapshot()`;
- `start_match()`;
- `set_paused(bool)`;
- `restart_match()`.

Add `parse_target_kind()` beside `parse_unit_kind()` / `parse_building_kind()` and accept the closed strings `"unit"` / `"building"`. Rust maps them to `CombatTarget`; invalid strings reject at the bridge. Numeric reject codes remain outputs only.

Right-click target picking reuses the existing `_nearest_view` path with an enemy-only filter and the existing distance tie-break shape rather than inventing a new picker. Ground right-click remains Move; `A` then ground issues AttackMove; `S` remains Stop; `Esc` cancels placement first then toggles pause while Playing/Paused. No gameplay orders leave Godot in Start/Paused/Result.

Health bars are driven from `Changed<Health>` through existing node handles, not a per-frame GDScript world scan. Primitive role markers/scales distinguish villager/spearman/archer/cavalry; no image-generation task.

`main.tscn` gains one small session overlay and Pause button: Start, Paused/Resume, Result with Victory/Defeat + Restart/Quit.

## Verification strategy

### Rust

- melee/ranged range and cooldown;
- three real symmetric attack-move counter duels: Spearman survives vs Cavalry, Archer survives vs Spearman, Cavalry survives vs Archer;
- unit pursuit/replan;
- building pursuit picks the nearest walkable immediate-perimeter goal;
- `Footprint::closest_point` controls building range;
- deterministic acquisition and AttackMove continuation after atomic target death;
- dead later attacker is skipped safely in the same combat step;
- Stop/Move/worker retasks all clear combat via `cancel_unit_activity()`;
- worker carry loss on death;
- Farm/site/drop-off destruction cleanup;
- footprint freeing, queue discard/no refund, derived cap reduction and simultaneous-spawn safety;
- system-order same-tick destroyed-drop-off regression;
- missing session = Playing;
- Start/Paused/Result freeze gameplay and collapse interpolation state;
- benchmark remains Playing;
- first Town Center destruction wins and aborts remaining combat actors;
- post-Result commands reject/freeze;
- victory and defeat journeys use bounded event loops, not fixed tick arithmetic.

### Godot/runtime

Create `combat_lifecycle_smoke_test.gd/.tscn` at the **start of the Godot integration task** and grow it as bridge input, target picking, health bars/effects, session overlay and restart behavior land. Task 6 keeps this smoke green after each integration slice; Task 7 promotes the finished smoke into CI and performs final regression/export verification.

When normal setup begins inserting Start, existing normal-skirmish smokes call `start_match()` in that same commit. Benchmark remains immediately Playing.

The Bevy E2E selector test stays a small boot contract; do not duplicate full UI automation there.

## Explicit non-goals

No HPA-473 fog/AI, extra units/buildings, siege, armour/damage framework, projectile physics, formations, abilities, garrisons, Town Center weapon, save/load, multiplayer, replay, custom animation/art pipeline, generic event/service architecture, or final balance/performance tuning.
