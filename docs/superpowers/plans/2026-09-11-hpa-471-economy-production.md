# HPA-471 Worker Economy, Construction, and Army Production Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the HPA-470 battlefield into the complete HPA-471 gather/build/train/advance loop without creating a second gameplay state model.

**Architecture:** Bevy ECS remains authoritative. Four focused simulation modules (`catalog`, `economy`, `buildings`, `production`) extend the existing movement/command core. Buildings land before gathering so Town Centers and Storehouses are real geometry-owning entities from birth. Godot stays a thin input/HUD/presentation layer and switches from the 200-unit startup fixture to the economy skirmish only after the simulation and runtime-view seams are ready.

**Tech Stack:** Rust 1.89, Bevy 0.18.1, godot-bevy 0.11.0, godot-rust 0.4.5, Godot 4.6.2, GDScript, existing GitHub Actions CI.

**Spec:** `docs/superpowers/specs/2026-09-11-hpa-471-economy-production-design.md`

## Global Constraints

- One Linear ticket and one GitHub PR; this draft remains the implementation PR.
- Bevy owns all mutable gameplay state. Godot caches only presentation/selection state.
- Every implementation commit must keep `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p grus-sim`, and the currently enabled HPA-470 Godot smoke/reset/benchmark CI gates green.
- Do **not** switch normal Godot startup away from the 200-unit fixture until Task 6, where smoke/reset/benchmark assumptions change atomically.
- Final fixed tick order is `commands → movement → economy → construction → production`.
- All walkability changes use `GridMap::set_blocked` / `set_blocked_rect`, except reset replacing the map resource with a fresh authored map.
- Reuse HPA-470 reference-counted current/goal reservations and A*. Do not add a persistent reservation table or navigation service.
- Move keeps its existing walkable-target ring behavior. Gather/build/drop-off uses immediate-adjacent `approach_slots`; workers never gather/build from radius 2+.
- Replacement worker commands validate first; only accepted workers have old activity canceled. Rejected commands preserve old task/order.
- Carried resources are never discarded by retasking.
- One `RejectReason` and one `CommandResult` cover every command family; bridge tests assert reject codes/state, not English strings.
- One `Footprint` type owns spatial rectangle geometry. `Dropoff` does not duplicate anchor/width/height.
- Fixed content ceiling: Food/Wood/Gold, four unit kinds, seven building kinds, two ages, 100 population.
- Starting team state: four villagers, 200 Food, 300 Wood, 100 Gold, one Town Center.
- Carry 10; gather 2.0/s in Age 1 and 2.2/s in Age 2.
- Rally points and idle-worker navigation remain in scope because HPA-471 explicitly requires them.
- Farm is one entity with `Building + ResourceSource + Footprint` and exactly one Godot building view.
- The 200-unit speed-12 fixture remains benchmark-only after the Task 6 cutover.
- No combat, fog, AI, destruction/refunds, persistence, content editor, generic research tree, generic service/repository layer, or new rendering framework.

---

### Task 1: Lock shared contracts, typed catalogue, and authored fixture data

**Files:**
- Create: `crates/grus-sim/src/catalog.rs`
- Create: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/ids.rs`
- Modify: `crates/grus-sim/src/map.rs`
- Modify: `crates/grus-sim/src/movement.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/fixture.rs`
- Modify: `crates/grus-sim/src/lib.rs`
- Modify: `crates/grus-godot/src/lib.rs`

**Interfaces:**
- Produces catalogue enums/specs/costs and exact HPA-471 tuning.
- Produces `BuildingId`, `ResourceId`, `IdAllocator::new(next_unit, next_building, next_resource)`.
- Produces `Footprint::new(anchor, width, height)`, `cells() -> Vec<GridPos>`, `perimeter_cells() -> Vec<GridPos>`, and `is_immediately_adjacent(cell)`.
- Produces `Carry`, `GatherProgress`, `WorkerTask`, `ResourceStockpile`, `TeamState`, `TeamEconomy::default()`, `TeamEconomy::insert_team(team, stockpile, age)`, and `Dropoff` contract types; gather behavior comes later.
- Replaces `CommandOutcome` / `CommandRejectReason` with `CommandResult` / `RejectReason`.
- Wraps current Move/Stop in the first exhaustive `PlayerCommand::Units` without changing public Godot Move/Stop signatures.
- Produces `approach_slots()` immediate candidate generation while keeping Move's existing destination generation.
- Replaces old untyped fixture resource arrays with typed descriptors; keeps `left_spawn`, `right_spawn`, and `units_200()`.

- [ ] **Step 1: Add the static catalogue**

Define:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceKind { Food, Wood, Gold }

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UnitKind { Villager, Spearman, Archer, Cavalry }

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuildingKind {
    TownCenter,
    House,
    Storehouse,
    Farm,
    Barracks,
    ArcheryRange,
    Stable,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Age { Age1, Age2 }

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Cost { pub food: u32, pub wood: u32, pub gold: u32 }
```

Expose `BuildingKind::ALL` and `UnitKind::ALL`. Implement `unit_spec()` / `building_spec()` with the exact values in the spec plus `AGE_TWO_COST`, `AGE_TWO_SECONDS`, `CARRY_LIMIT`, `BASE_GATHER_RATE`, `AGE_TWO_GATHER_RATE`, and `MAX_POPULATION`.

Add one catalogue-relationship test instead of restating individual table entries:

```rust
#[test]
fn only_stable_and_cavalry_require_age_two() {
    let gated_buildings = BuildingKind::ALL
        .into_iter()
        .filter(|kind| building_spec(*kind).required_age == Age::Age2)
        .collect::<Vec<_>>();
    let gated_units = UnitKind::ALL
        .into_iter()
        .filter(|kind| unit_spec(*kind).required_age == Age::Age2)
        .collect::<Vec<_>>();

    assert_eq!(gated_buildings, vec![BuildingKind::Stable]);
    assert_eq!(gated_units, vec![UnitKind::Cavalry]);
}
```

Run `cargo test -p grus-sim catalog` and make it pass.

- [ ] **Step 2: Add stable IDs, Footprint, and worker/economy contract types**

In `ids.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BuildingId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceId(pub u32);

#[derive(Debug, Resource)]
pub struct IdAllocator {
    pub next_unit: u32,
    pub next_building: u32,
    pub next_resource: u32,
}

impl IdAllocator {
    pub const fn new(next_unit: u32, next_building: u32, next_resource: u32) -> Self {
        Self { next_unit, next_building, next_resource }
    }
}
```

Add monotonic allocation methods that reject/skip zero and increment after returning an ID.

In `map.rs`:

```rust
#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub struct Footprint {
    pub anchor: GridPos,
    pub width: u8,
    pub height: u8,
}
```

`cells()` and `perimeter_cells()` return deterministic `Vec<GridPos>` values; `perimeter_cells()` excludes every footprint cell.

In `economy.rs`:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Carry {
    Empty,
    Holding { kind: ResourceKind, amount: std::num::NonZeroU32 },
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct GatherProgress(pub f32);

#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub enum WorkerTask {
    Idle,
    ToSource { source: ResourceId, slot: GridPos },
    Gathering { source: ResourceId },
    ToDropoff { source: ResourceId, dropoff: BuildingId, slot: GridPos },
    ToConstruction { building: BuildingId, slot: GridPos },
    Constructing { building: BuildingId },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceStockpile { pub food: u32, pub wood: u32, pub gold: u32 }

#[derive(Clone, Debug)]
pub struct TeamState {
    pub stockpile: ResourceStockpile,
    pub age: Age,
    pub age_up_started: bool,
}

#[derive(Debug, Default, Resource)]
pub struct TeamEconomy(pub std::collections::HashMap<TeamId, TeamState>);

impl TeamEconomy {
    pub fn insert_team(&mut self, team: TeamId, stockpile: ResourceStockpile, age: Age) {
        self.0.insert(team, TeamState { stockpile, age, age_up_started: false });
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct Dropoff { pub team: TeamId }
```

Extend `Unit` with `UnitKind` and update all current callers. Benchmark units stay speed 12.0.

- [ ] **Step 3: Unify command results before adding new commands**

In `commands.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RejectReason {
    UnknownUnit,
    NotOwned,
    NotVillager,
    Unreachable,
    Crowded,
    SourceMissing,
    BuildingMissing,
    Locked,
    InsufficientResources,
    OutOfBounds,
    Occupied,
    WrongProducer,
    FarmOccupied,
    PopulationFull,
    NoSpawnSpace,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandResult {
    pub accepted_units: Vec<UnitId>,
    pub rejected_units: Vec<(UnitId, RejectReason)>,
    pub reject: Option<RejectReason>,
}

pub enum PlayerCommand {
    Units(UnitCommand),
}
```

Implement:

```rust
pub fn apply_player_command(
    world: &mut World,
    map: &mut GridMap,
    command: PlayerCommand,
) -> CommandResult
```

Move/Stop preserve existing per-unit behavior. Promote the existing ownership lookup to `pub(crate) fn owned_unit_entity(...)`.

Update `grus-godot` so `move_units()` and `stop_units()` queue `PlayerCommand::Units`. Extend `CommandFeedback` with `last_reject_code`. Preserve readable text containing `unreachable` so the existing HPA-470 smoke does not change yet.

- [ ] **Step 4: Add immediate approach-slot candidates without changing Move semantics**

Keep the existing Move ring generator. Add:

```rust
pub(crate) fn approach_slots(
    map: &GridMap,
    footprint: Footprint,
    used: &std::collections::HashSet<GridPos>,
    count: usize,
) -> Vec<GridPos>
```

It returns at most `count` walkable, unreserved cells from the immediate perimeter and never scans a wider ring.

Test production behavior directly:

```rust
#[test]
fn approach_slots_stay_on_the_immediate_perimeter() {
    let mut map = GridMap::new(8, 8);
    let footprint = Footprint::new(GridPos::new(3, 3), 2, 2);
    for cell in footprint.cells() {
        map.set_blocked(cell, true);
    }

    let used = std::collections::HashSet::new();
    let slots = approach_slots(&map, footprint, &used, 8);

    assert!(!slots.is_empty());
    assert!(slots.iter().all(|slot| map.is_walkable(*slot)));
    assert!(slots.iter().all(|slot| footprint.is_immediately_adjacent(*slot)));
    assert!(slots.iter().all(|slot| !footprint.cells().contains(slot)));
}
```

- [ ] **Step 5: Replace the old resource marker tables with typed fixture data**

Define deterministic `TeamStart` and `ResourceSpawn` descriptors with the exact cells from the spec. Delete `starting_resources` and `expansion_resources`. Keep `left_spawn`, `right_spawn`, and `units_200()`.

Add fixture tests for in-bounds resources, no Town Center/resource overlap, mirrored safe-source counts, and villager walkability after Town Center blocking.

- [ ] **Step 6: Verify the unchanged HPA-470 runtime and commit**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p grus-sim
cargo build -p grus-godot
mkdir -p godot/bin
cp target/debug/libgrus_godot.so godot/bin/libgrus_godot.so
godot --headless --path godot --editor --quit-after 120
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
```

Startup remains the HPA-470 200-unit fixture.

```bash
git add crates/grus-sim crates/grus-godot
git commit -m "feat: lock HPA-471 domain and command contracts"
```

---

### Task 2: Add authoritative buildings, construction, and pure skirmish seeding

**Files:**
- Create: `crates/grus-sim/src/buildings.rs`
- Modify: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/fixture.rs`
- Modify: `crates/grus-sim/src/lib.rs`

**Interfaces:**
- Extends `PlayerCommand` with `PlaceBuilding` and `ResumeConstruction` in the same commit that implements them.
- Produces `Building`, `ConstructionState`, `BuildingIndex`, `validate_placement()`, `step_construction()`.
- Produces `seed_skirmish(world, map, fixture)` with real completed Town Center Buildings/Footprints and Villagers from birth.
- Starting Town Centers have `Dropoff { team }`; completed Storehouses gain the same marker.
- Does not switch Godot Startup.

- [ ] **Step 1: Add Building, ConstructionState, and BuildingIndex**

```rust
#[derive(Component, Debug)]
pub struct Building {
    pub id: BuildingId,
    pub team: TeamId,
    pub kind: BuildingKind,
    pub construction: ConstructionState,
}

#[derive(Clone, Copy, Debug)]
pub struct ConstructionState {
    pub progress_seconds: f32,
    pub complete: bool,
    pub active_builder: Option<UnitId>,
}
```

Each Building entity also owns one `Footprint`. `BuildingIndex` mirrors `UnitIndex` with `entity(id)` and `iter()`.

- [ ] **Step 2: Add placement commands and authoritative validation**

Extend `PlayerCommand`:

```rust
PlaceBuilding {
    issuer: TeamId,
    builder: UnitId,
    kind: BuildingKind,
    anchor: GridPos,
},
ResumeConstruction {
    issuer: TeamId,
    builder: UnitId,
    building: BuildingId,
},
```

Validation order: owned villager → unlocked/buildable → footprint in bounds → footprint walkable → affordable → reachable immediate approach slot.

Accepted placement only:

1. cancels old worker activity;
2. deducts cost once;
3. allocates `BuildingId`;
4. blocks every footprint cell;
5. spawns `Building + Footprint`;
6. installs `ToConstruction { building, slot }` and matching MoveOrder.

All failures use `RejectReason`; do not create `BuildRejectReason`.

- [ ] **Step 3: Add private test setup inside `buildings.rs`**

Inside `#[cfg(test)] mod tests`, define `setup_build_test()` using only public/production constructors introduced in Task 1:

```rust
fn setup_build_test() -> (World, GridMap, Entity) {
    let mut world = World::new();
    let mut economy = TeamEconomy::default();
    economy.insert_team(
        TeamId(1),
        ResourceStockpile { food: 500, wood: 500, gold: 500 },
        Age::Age1,
    );
    world.insert_resource(economy);
    world.insert_resource(IdAllocator::new(10, 10, 10));

    let map = GridMap::new(64, 64);
    let villager = spawn_unit(
        &mut world,
        UnitId(1),
        TeamId(1),
        UnitKind::Villager,
        Vec2::new(10.5, 10.5),
        unit_spec(UnitKind::Villager).speed,
    );
    world.entity_mut(villager).insert((
        Carry::Empty,
        GatherProgress::default(),
        WorkerTask::Idle,
    ));
    (world, map, villager)
}
```

Any additional helper such as `setup_active_builder()` stays private in this test module and must build state by calling `apply_player_command` / `step_movement` / `step_construction`, not by mutating construction progress behind the production API.

- [ ] **Step 4: Implement one-builder construction and validate-then-cancel retasking**

`step_construction()` transitions `ToConstruction → Constructing` only when the worker reaches its stored slot. One `active_builder` advances progress. Accepted Move/Stop/Place/Resume pauses/replaces work; rejected replacement commands preserve it.

Regression for rejection preservation:

```rust
#[test]
fn rejected_move_preserves_active_builder() {
    let (mut world, mut map, villager, building) = setup_active_builder();
    let old_task = world.get::<WorkerTask>(villager).unwrap().clone();
    let blocked = GridPos::new(20, 20);
    map.set_blocked(blocked, true);

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::Units(UnitCommand {
            issuer: TeamId(1),
            units: vec![UnitId(1)],
            kind: UnitCommandKind::Move { target: map.cell_center(blocked) },
        }),
    );

    assert_eq!(
        result.rejected_units,
        vec![(UnitId(1), RejectReason::Unreachable)]
    );
    assert_eq!(world.get::<WorkerTask>(villager), Some(&old_task));
    assert_eq!(
        world.get::<Building>(building).unwrap().construction.active_builder,
        Some(UnitId(1))
    );
}
```

Also test invalid placement never charges, accepted House deducts exactly 50 Wood and blocks four cells, two builders never double speed, and Resume keeps accumulated progress.

- [ ] **Step 5: Implement pure `seed_skirmish` with real Town Centers**

`seed_skirmish` creates:

```text
TeamEconomy for Team 1 and Team 2
2 completed Town Center Buildings + 4x4 Footprints + Dropoff markers
4 Villagers per team with Carry::Empty/GatherProgress(0)/WorkerTask::Idle
Town Center occupancy
UnitIndex + BuildingIndex + IdAllocator counters above authored maxima
```

No temporary Dropoff-only Town Center exists.

- [ ] **Step 6: Add Storehouse completion effect**

A Storehouse receives `Dropoff { team }` exactly once when construction completes. No routing geometry is copied into Dropoff.

- [ ] **Step 7: Verify unchanged Godot baseline and commit**

Run the Task 1 verification commands. Godot still boots 200 units.

```bash
git add crates/grus-sim
git commit -m "feat: add authoritative building construction"
```

---

### Task 3: Add finite resources, Carry-safe gathering, Storehouse delivery, and Farms

**Files:**
- Modify: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/buildings.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/fixture.rs`
- Modify: `crates/grus-sim/src/lib.rs`
- Create: `crates/grus-sim/tests/economy_flow.rs`

**Interfaces:**
- Extends `PlayerCommand` with `Gather` in the same commit that implements it.
- Produces `ResourceSource`, `ResourceIndex`, authored finite source seeding, Farm source creation, and `step_economy()`.
- Uses `Building + Footprint + Dropoff` for delivery geometry.
- Uses stored slots for positive arrival detection.

- [ ] **Step 1: Add ResourceSource and finite authored source seeding**

```rust
#[derive(Component, Debug)]
pub struct ResourceSource {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub remaining: Option<u32>,
    pub assigned_worker: Option<UnitId>,
}
```

Standalone authored sources use `ResourceSource + Footprint::new(cell, 1, 1)` and block that one cell. `ResourceIndex` follows the typed index pattern.

Extend `seed_skirmish()` to insert every typed `ResourceSpawn` from Task 1.

- [ ] **Step 2: Add Gather assignment with distinct immediate slots**

Extend `PlayerCommand`:

```rust
Gather {
    issuer: TeamId,
    workers: Vec<UnitId>,
    source: ResourceId,
},
```

Gather validates source, owned Villagers, Farm availability, and shared reservation state. It assigns unique immediate-perimeter slots. Normal Move to the blocked source center remains `Unreachable`.

Module tests set up four Villagers and a one-cell source, then assert:

```text
Move(source center) -> RejectReason::Unreachable
Gather(source) -> four accepted Villagers
all four ToSource.slot values are unique
all slots are walkable and immediately adjacent
```

The test setup is private to `economy.rs` and uses production APIs.

- [ ] **Step 3: Implement Carry-preserving retasking**

Rules:

```text
Move/Stop/Place/Resume preserve Carry.
Gather + Carry::Empty routes to the requested source.
Gather + Carry::Holding routes to a Dropoff first, then the requested source.
Carry never mixes kinds.
```

`cancel_worker_activity()` clears worker task, Farm assignment, active builder, GatherProgress, and old MoveOrder as appropriate; it never changes Carry.

Add a production-level test: worker holds 6 Wood, receives Gather on berries, routes to Town Center Dropoff, stockpile Wood increases by 6, Carry becomes Empty, then worker routes to the berry slot. No Food is added before berry gathering begins.

- [ ] **Step 4: Implement actual gather/carry/deposit stepping**

`step_economy()` checks the stored slot for arrival. While Gathering:

```rust
progress.0 += gather_rate_for_age(team_age) * SIM_STEP_SECONDS;
let whole = progress.0.floor() as u32;
let carry_space = CARRY_LIMIT - carry.amount_or_zero();
let source_available = source.remaining.unwrap_or(u32::MAX);
let transferred = whole.min(carry_space).min(source_available);
```

Add only `transferred` to Carry and subtract only `transferred as f32` from progress. Stockpile changes only when the worker reaches stored `ToDropoff.slot`.

The accumulator regression calls production code. In `economy.rs` tests, create one Villager already adjacent to a 600-Food source, issue Gather, advance to `Gathering`, run `step_economy` ten times, and assert `Carry::Holding { kind: Food, amount: 1 }` plus source remaining 599. Do not reimplement the accumulator in the test body.

- [ ] **Step 5: Implement depletion and Farm one-worker semantics**

Finite depletion:

```text
remove ResourceId from ResourceIndex
despawn source entity
unblock its 1x1 Footprint through GridMap::set_blocked
worker delivers final Carry
worker becomes Idle after deposit
```

Completed Farm adds a runtime ResourceId and renewable Food `ResourceSource { remaining: None, assigned_worker: None }` to the existing Farm Building entity. First worker reserves it; second worker rejects `FarmOccupied`; accepted retask/Stop releases assignment.

- [ ] **Step 6: Add one public-API cross-module integration test**

`tests/economy_flow.rs` performs:

```text
seed_skirmish
issue Gather on Tree
deposit at starting Town Center
place/complete Storehouse through commands/steps
issue another Tree Gather
observe reachable Storehouse delivery
fully deplete a small finite test source
observe final deposit and Idle
```

The integration test uses only exported simulation functions/types. Ownership and internal helper behavior stays in module-local tests.

- [ ] **Step 7: Verify unchanged Godot baseline and commit**

Run the Task 1 verification commands. Godot remains on the 200-unit startup fixture.

```bash
git add crates/grus-sim
git commit -m "feat: add worker gathering and dropoff loop"
```

---

### Task 4: Add production queues, derived population, rally points, and Age 2

**Files:**
- Create: `crates/grus-sim/src/production.rs`
- Modify: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/buildings.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/lib.rs`
- Create: `crates/grus-sim/tests/system_order.rs`

**Interfaces:**
- Extends `PlayerCommand` with `EnqueueUnit`, `EnqueueAgeUp`, `SetRally` in the same commit that implements them.
- Produces `ProductionQueue`, `ProductionJob`, `RallyPoint`, population helpers, spawn clearance, production stepping.
- Keeps Age 2's 2.2/s gather improvement.

- [ ] **Step 1: Add FIFO production state and producer compatibility**

Producer compatibility is fixed:

```text
Town Center -> Villager, Age 2
Barracks -> Spearman
Archery Range -> Archer
Stable -> Cavalry
```

Use only unified `RejectReason` values.

- [ ] **Step 2: Implement enqueue charging and one-time Age 2 lock**

Accepted enqueue deducts cost exactly once. Age 2 sets `age_up_started = true` on acceptance so a second Age 2 command rejects immediately.

Module tests create real `World`, `TeamEconomy`, and complete producer Buildings. Assert Villager deducts 50 Food once, Archer-from-Barracks returns `WrongProducer` without charge, Stable/Cavalry are `Locked` before Age 2, and waiting-at-100%-ready jobs never recharge.

- [ ] **Step 3: Implement derived population and deterministic same-tick completion**

`population_used()` counts live team Units. `population_cap()` sums completed Town Center/House catalogue capacity and clamps to 100.

Process ready producers sorted by `BuildingId`, recomputing population after each successful spawn.

Inside `production.rs` tests, define private `setup_two_ready_barracks_at_pop_9_of_10()` that creates the exact world and queues through production APIs. The regression asserts one spawn succeeds, population becomes 10, and exactly one queue stays blocked with `PopulationFull`.

- [ ] **Step 4: Implement spawn clearance and rally points**

Spawn only into a walkable, unoccupied immediate perimeter slot around the producer. If none exists, keep job ready and expose `NoSpawnSpace`. Allocate UnitId only on successful spawn.

`SetRally` stores a map target. Spawned units use normal Move assignment toward it. Failed rally routing does not undo the spawn.

- [ ] **Step 5: Implement Age 2 completion and 2.2/s gathering**

Age job costs 300 Food + 200 Gold, takes 45 s, shares Town Center FIFO, and completes once. Completion sets Age 2. `gather_rate_for_age(Age2)` returns 2.2/s beginning on the next economy step.

- [ ] **Step 6: Add one real system-order integration test**

`tests/system_order.rs` sets up an arrival plus an Age job that will complete during the same logical tick. Call exactly:

```rust
apply_queued_test_commands(&mut world, &mut map);
step_movement(&mut world, &map, SIM_STEP_SECONDS);
step_economy(&mut world, &mut map, SIM_STEP_SECONDS);
step_construction(&mut world, SIM_STEP_SECONDS);
step_production(&mut world, &mut map, SIM_STEP_SECONDS);
```

Assert the arrival is visible to economy/construction in that tick, while the already-run economy step still used Age 1 rate before production flips the age.

`apply_queued_test_commands` is defined in this integration test and loops over a local `Vec<PlayerCommand>` through public `apply_player_command`; it is not production API.

- [ ] **Step 7: Verify and commit**

Run the Task 1 verification commands.

```bash
git add crates/grus-sim
git commit -m "feat: add production population rally and age advancement"
```

---

### Task 5: Replace fixture-only unit scene insertion on the known-green 200-unit baseline

**Files:**
- Modify: `crates/grus-godot/src/lib.rs`
- Modify: `godot/scenes/unit_view.tscn`
- Modify: `godot/scripts/unit_view.gd`

**Interfaces:**
- Produces `attach_missing_gameplay_views`, initially exercised by Unit entities on the existing 200-unit startup fixture.
- Preserves HPA-470 unit presentation contracts.
- Does not switch startup/reset.

- [ ] **Step 1: Move Unit GodotScene attachment out of fixture seeding**

Add `GameplayViewRequested`. In `Update`, a Unit lacking the marker receives:

```rust
(
    Transform::from_xyz(position.current.x, 0.0, position.current.y),
    TransformSyncMetadata::default(),
    Node3DMarker,
    GodotScene::from_path("res://scenes/unit_view.tscn"),
    GameplayViewRequested,
)
```

Remove direct Unit `GodotScene` insertion from HPA-470 fixture seeding. Metadata initialization still waits for `GodotNodeHandle`.

- [ ] **Step 2: Preserve the exact HPA-470 unit presentation contract**

Keep:

```text
unit_views group
SelectionRing child
set_selected(bool)
unit_id metadata
team_id metadata
```

Add `unit_kind` metadata without renaming old keys.

- [ ] **Step 3: Run existing integration gates against the same 200-unit fixture**

```bash
cargo build -p grus-godot
mkdir -p godot/bin
cp target/debug/libgrus_godot.so godot/bin/libgrus_godot.so
godot --headless --path godot --editor --quit-after 120
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
```

CI must keep the 200-unit benchmark green before Task 6 begins.

- [ ] **Step 4: Commit**

```bash
git add crates/grus-godot godot/scenes/unit_view.tscn godot/scripts/unit_view.gd
git commit -m "refactor: attach unit views through runtime system"
```

---

### Task 6: Atomically cut Godot runtime to the economy skirmish

**Files:**
- Modify: `crates/grus-godot/src/lib.rs`
- Create: `godot/scenes/resource_view.tscn`
- Create: `godot/scripts/resource_view.gd`
- Create: `godot/scenes/building_view.tscn`
- Create: `godot/scripts/building_view.gd`
- Modify: `godot/scripts/unit_view.gd`
- Modify: `godot/scenes/battlefield.tscn`
- Modify: `godot/scripts/smoke_test.gd`
- Modify: `godot/scripts/reset_test.gd`
- Modify: `godot/scripts/benchmark_200.gd`

**Interfaces:**
- Normal Startup/reset seeds the complete skirmish.
- Benchmark reset seeds only `units_200()` on a fresh map.
- Full fixed chain is wired once.
- Runtime views cover Units, Buildings, and standalone ResourceSources; Farm gets one Building view.
- Bridge exposes final command methods, ECS snapshots, reject codes, and `set_sim_speed`.

- [ ] **Step 1: Add one clear-and-seed reset path**

Collect unique gameplay entities in a `HashSet<Entity>` from queries for `Unit`, `Building`, and `ResourceSource`, despawn each once, then reset:

```text
UnitIndex
BuildingIndex
ResourceIndex
TeamEconomy
IdAllocator
PendingCommands contents
CommandFeedback
GridMap -> fresh MapFixture::battlefield().map
```

Expose:

```text
reset_fixture() -> economy skirmish
reset_benchmark_fixture() -> only 200 speed-12 units
```

Normal Startup now calls the skirmish seed.

- [ ] **Step 2: Wire the final fixed chain**

```rust
.add_systems(
    FixedUpdate,
    (
        apply_pending_commands,
        advance_movement,
        advance_economy,
        advance_construction,
        advance_production,
    ).chain(),
)
```

- [ ] **Step 3: Extend runtime view attachment to Buildings/resources**

Attachment precedence:

```text
Unit                            -> unit_view.tscn
Building                        -> building_view.tscn
ResourceSource without Building -> resource_view.tscn
```

Metadata:

```text
Unit: unit_id unit_kind team_id
Building: building_id building_kind team_id
Standalone resource: resource_id resource_kind
Farm building: building_id building_kind team_id resource_id resource_kind
```

Use primitive meshes/materials only. Remove static base markers.

- [ ] **Step 4: Expose bridge write methods and read-only snapshots**

Bridge writes queue Gather, Place, Resume, Enqueue Unit, Enqueue Age, Set Rally.

Bridge snapshots contain:

```text
economy: food wood gold age population_used population_cap idle_workers idle_worker_ids last_reject_code
building: id kind complete construction_progress queue_label queue_progress blocked_reason rally_x rally_y
placement: valid anchor_x anchor_y width height reject_code
```

Readable feedback text remains available, but tests use state/codes.

- [ ] **Step 5: Add Bevy virtual-time smoke hook**

```rust
#[func]
fn set_sim_speed(&self, relative_speed: f64) -> bool {
    if !relative_speed.is_finite() || relative_speed <= 0.0 {
        return false;
    }
    let Some(mut app_node) = bevy_app_singleton() else { return false; };
    let mut app_node = app_node.bind_mut();
    let Some(app) = app_node.get_app_mut() else { return false; };
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .set_relative_speed_f64(relative_speed);
    true
}
```

Do not modify `max_delta`; Bevy 0.18.1 clamps raw real delta before multiplying by relative speed.

- [ ] **Step 6: Adapt all three HPA-470 Godot gates in this same commit**

`smoke_test.gd`:

```text
wait for 8 normal Unit views
Team 1 IDs 1..4; Team 2 starts at 5
preserve click/shift/box/control-group/move/stop/HUD/camera/unreachable tests
speed-6 one-second distance must be 4.0..8.0
max interpolated visual step must be < 0.25
```

`reset_test.gd`:

```text
reset normal fixture
8 unique Unit views
2 Town Center Building views
18 standalone Resource views
Team 1 stockpile 200/300/100
Team 1 population 4/10
no duplicate stable IDs
```

`benchmark_200.gd`:

```text
call reset_benchmark_fixture() before waiting for units
wait for exactly 200 unique UnitIds
assert building_views/resource_views are empty
keep existing 200-moving-unit timing contract
```

- [ ] **Step 7: Run pre-existing gates before starting UI work**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p grus-sim
cargo build -p grus-godot
mkdir -p godot/bin
cp target/debug/libgrus_godot.so godot/bin/libgrus_godot.so
godot --headless --path godot --editor --quit-after 120
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
```

CI must run export + benchmark on this exact cutover. Do not begin Task 7 with a red integration baseline.

- [ ] **Step 8: Commit**

```bash
git add crates/grus-godot godot
git commit -m "feat: cut runtime to HPA-471 skirmish state"
```

---

### Task 7: Add contextual economy/construction/production UI

**Files:**
- Modify: `godot/scenes/main.tscn`
- Modify: `godot/scripts/battlefield_controller.gd`

**Interfaces:**
- Existing controller remains interaction owner.
- All costs/unlocks/progress/reject state comes from bridge snapshots.
- Idle navigation is minimal click-to-cycle over bridge-provided stable IDs.

- [ ] **Step 1: Expand HUD with required HPA-471 state/actions**

Add:

```text
Food / Wood / Gold
population used/cap
Age
idle-worker count/navigation
selected entity
construction/queue progress
command feedback
House / Storehouse / Farm / Barracks / Archery Range / Stable
Villager / Spearman / Archer / Cavalry / Advance Age
```

Keep all controls under the existing HUD input shield.

- [ ] **Step 2: Extend selection without breaking box selection**

Keep `selected_ids` for Units and add one `selected_building_id`. Box selection remains Units-only. Building click clears Unit selection; Unit click clears Building selection.

- [ ] **Step 3: Resolve contextual right-click in one order**

```text
Villager(s) + standalone resource -> Gather(resource_id)
Villager(s) + completed Farm -> Gather(resource_id)
Villager + incomplete owned Building -> ResumeConstruction
selected production Building + ground -> SetRally
selected Units + ground -> Move
```

Military Units never gather/build.

- [ ] **Step 4: Implement one placement-preview mode**

A build button stores only a `BuildingKind` code. Mouse motion calls the authoritative preview and updates one translucent box. Valid left click sends Place with the lowest selected Villager ID. Escape/right-click exits without command or cost.

- [ ] **Step 5: Wire production/Age/rally and idle navigation**

Production buttons enqueue against the selected Building. Town Center exposes Advance Age with cost/lock state. Building panel displays queue progress, blocked reason, and rally target.

Idle-worker control reads Rust-sorted `idle_worker_ids` and cycles one stable ID per click. Godot stores only the cursor position for UI convenience.

- [ ] **Step 6: Verify existing smoke/reset plus interactive start**

Run smoke/reset and `godot --path godot`. Verify `200/300/100`, `4/10`, contextual resource/building clicks, production buttons, rally input, idle-worker navigation, and no HUD click leakage.

- [ ] **Step 7: Commit**

```bash
git add godot/scenes/main.tscn godot/scripts/battlefield_controller.gd
git commit -m "feat: add economy construction and production controls"
```

---

### Task 8: Add the solvent HPA-471 end-to-end smoke, CI timeout evidence, and docs

**Files:**
- Create: `godot/scripts/economy_smoke_test.gd`
- Create: `godot/scenes/economy_smoke_test.tscn`
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`

**Interfaces:**
- Smoke uses real selection/contextual/HUD paths plus `set_sim_speed(20.0)`.
- No debug grants or direct gameplay mutation.
- CI starts conservative and is tightened from CI evidence.

- [ ] **Step 1: Build a solvent UI-driven economy smoke**

Start with:

```gdscript
if not GrusBridge.set_sim_speed(20.0):
    _fail("failed to accelerate Bevy virtual time")
    return
```

Drive through the normal controller/UI:

1. assign Villagers to berries/trees/gold; prove stockpile changes only after deposit;
2. gather at least 315 additional Wood so total available Wood covers House + Storehouse + Farm + Barracks + Archery Range + Archer Wood + Stable (615 total spend against 300 starting);
3. gather enough Food/Gold for Spearman + Archer + Age 2 + Cavalry (480 Food, 260 Gold total spend);
4. build House and assert cap `10 → 20`;
5. build Storehouse and prove a later worker delivery reaches it;
6. build Farm, assign one worker, and assert a second worker returns the numeric FarmOccupied reject code;
7. build Barracks, train Spearman, assert a new Unit view;
8. build Archery Range, train Archer, assert view;
9. enqueue Age 2 and assert Age becomes 2 plus catalogue/economy snapshot shows the 2.2/s gather rate;
10. verify Stable unlocks, build it, train Cavalry, assert view;
11. set a rally point and assert the next trained Unit receives movement toward it;
12. click idle-worker navigation and assert it selects one ID currently returned by `idle_worker_ids`;
13. assert construction/queue progress and reject/blocked codes through snapshots.

Before success:

```gdscript
if not GrusBridge.set_sim_speed(1.0):
    _fail("failed to restore Bevy virtual time")
    return
```

Print a success marker with wall-clock elapsed milliseconds.

- [ ] **Step 2: Add CI with a conservative first timeout**

```yaml
- name: Run HPA-471 economy integration smoke
  run: timeout 300s godot --headless --path godot res://scenes/economy_smoke_test.tscn
```

Place it after reset smoke and before export/benchmark.

- [ ] **Step 3: Tighten timeout from the first successful CI run**

Read the CI success marker. Set the final timeout to at least `2 × observed CI elapsed + 10 seconds`, rounded up to a 15-second bucket. Make that timeout-only follow-up commit on this same PR. Do not derive it from a local workstation measurement.

- [ ] **Step 4: Update README**

Document current controls, economy architecture, rally/idle navigation, reset modes, build/import sequence, benchmark-only 200-unit fixture, and preserve the historical HPA-470 performance baseline.

- [ ] **Step 5: Run final gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p grus-sim
cargo build -p grus-godot
mkdir -p godot/bin
cp target/debug/libgrus_godot.so godot/bin/libgrus_godot.so
godot --headless --path godot --editor --quit-after 120
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
godot --headless --path godot res://scenes/economy_smoke_test.tscn
```

CI additionally exports the desktop build and runs the retained benchmark after explicit benchmark reset.

- [ ] **Step 6: Commit**

```bash
git add .github README.md godot/scenes/economy_smoke_test.tscn godot/scripts/economy_smoke_test.gd
git commit -m "test: verify complete HPA-471 economy loop"
```

---

## Plan Self-Review

- Every HPA-471 acceptance item remains in scope, including rally points, Age 2 gather improvement, and idle-worker navigation.
- Buildings precede gathering, so Town Centers and Storehouses own one real Footprint; Dropoff is only a marker.
- RejectReason / CommandResult land before feature-specific commands and bridge smokes assert codes/state.
- Worker approach tasks carry explicit adjacent slots; missing MoveOrder is never treated as arrival.
- Carry cannot represent `None + positive amount`, and retasking never destroys carried resources.
- Tests use module-local private helpers built on production APIs; the plan does not assume an undeclared public test harness.
- The gather accumulator regression calls `step_economy`; it does not duplicate the accumulator algorithm in test code.
- Old `starting_resources` / `expansion_resources` are deleted when typed resource descriptors land.
- Runtime stays on known-green 200-unit startup through Tasks 1–5. Task 5 validates the view-attachment seam there; Task 6 cuts startup/reset/benchmark atomically.
- Bevy `Time<Virtual>::max_delta` is not overridden; in Bevy 0.18.1 it clamps raw real delta before relative-speed multiplication.
- Economy smoke begins with a conservative CI timeout and tightens from CI evidence.
- No image generation or custom asset pipeline is required.
- Combat, fog, AI, destruction, persistence, generic frameworks, and final balance remain outside this PR.