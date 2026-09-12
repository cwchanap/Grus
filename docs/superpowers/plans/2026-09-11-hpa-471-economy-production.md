# HPA-471 Worker Economy, Construction, and Army Production Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the HPA-470 battlefield into the complete HPA-471 gather/build/train/advance loop without creating a second gameplay state model.

**Architecture:** Bevy ECS remains authoritative. Four focused simulation modules (`catalog`, `economy`, `buildings`, `production`) extend the existing movement/command core; one `PlayerCommand` dispatcher owns validation/application, and Godot remains a thin input/HUD/presentation layer. HPA-470 destination-slot reservation, `GridMap` revision-based occupancy, and one-way Bevy → Godot presentation are reused rather than replaced.

**Tech Stack:** Rust 1.89, Bevy 0.18.1 ECS/time, godot-bevy 0.11.0, godot-rust 0.4.5, Godot 4.6.2, GDScript, existing GitHub Actions CI.

**Spec:** `docs/superpowers/specs/2026-09-11-hpa-471-economy-production-design.md`

## Global Constraints

- One Linear ticket and one GitHub PR for all HPA-471 work; this draft remains the implementation PR.
- Bevy owns every mutable gameplay rule/state; Godot may cache only presentation and selection state.
- Keep the authoritative 20 Hz fixed simulation and interpolated Bevy → Godot transforms.
- Fixed tick order ends as `commands → movement → economy → construction → production`.
- All walkability mutations continue through `GridMap::set_blocked` / `set_blocked_rect` or replacement of the entire map resource during reset.
- Move, Gather, and construction approach share HPA-470 destination-slot reservation; do not add a second navigation subsystem.
- Replacement worker commands validate first and cancel the old worker task only for accepted units; rejected commands preserve the old task/order.
- Stockpile/source/carry amounts remain `u32`; fractional gather progress is `f32` per worker.
- Fixed content ceiling: Food/Wood/Gold, four unit kinds, seven building kinds, two ages, 100 population.
- Normal start: four villagers/team, 200 Food, 300 Wood, 100 Gold, one Town Center/team.
- Carry 10; gather 2.0/s in Age 1 and 2.2/s in Age 2.
- Runtime-trained units, placed buildings, and completed Farms must receive Godot views through the same attachment system as fixture entities.
- The 200-unit speed-12 fixture remains benchmark-only.
- No combat, fog, AI, destruction/refunds, save/load, content editor, hot reload, generic research tree, generic service/repository layer, or new rendering framework.

---

### Task 1: Lock typed content, stable IDs, and exact authored fixture coordinates

**Files:**
- Create: `crates/grus-sim/src/catalog.rs`
- Modify: `crates/grus-sim/src/ids.rs`
- Modify: `crates/grus-sim/src/movement.rs`
- Modify: `crates/grus-sim/src/fixture.rs`
- Modify: `crates/grus-sim/src/lib.rs`

**Interfaces:**
- Produces `ResourceKind`, `UnitKind`, `BuildingKind`, `Age`, `Cost`, `UnitSpec`, `BuildingSpec`, `unit_spec()`, `building_spec()`.
- Produces `BuildingId`, `ResourceId`, and `IdAllocator { next_unit, next_building, next_resource }`.
- Extends `Unit` with `kind: UnitKind`.
- Produces deterministic `TeamStart` and `ResourceSpawn` descriptors plus retained `MapFixture::units_200()`.

- [ ] **Step 1: Add failing catalogue tests**

```rust
#[test]
fn stable_and_cavalry_require_age_two() {
    assert_eq!(building_spec(BuildingKind::Stable).required_age, Age::Age2);
    assert_eq!(unit_spec(UnitKind::Cavalry).required_age, Age::Age2);
}

#[test]
fn population_values_match_mvp_contract() {
    assert_eq!(building_spec(BuildingKind::TownCenter).population_capacity, 10);
    assert_eq!(building_spec(BuildingKind::House).population_capacity, 10);
    assert_eq!(MAX_POPULATION, 100);
}
```

Run `cargo test -p grus-sim catalog`; expect compile failure because the catalogue does not exist yet.

- [ ] **Step 2: Implement the static catalogue**

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceKind { Food, Wood, Gold }

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UnitKind { Villager, Spearman, Archer, Cavalry }

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuildingKind {
    TownCenter, House, Storehouse, Farm, Barracks, ArcheryRange, Stable,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Age { Age1, Age2 }

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Cost { pub food: u32, pub wood: u32, pub gold: u32 }
```

Use `match`-based `unit_spec()` / `building_spec()` and the exact spec values. Add `AGE_TWO_COST`, `AGE_TWO_SECONDS`, `CARRY_LIMIT`, `BASE_GATHER_RATE`, `AGE_TWO_GATHER_RATE`, and `MAX_POPULATION`.

- [ ] **Step 3: Extend IDs and units without adding a generic registry**

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
```

Add monotonic allocation methods that never emit zero. Extend `Unit` with `kind`. Update existing `spawn_unit` callers so the benchmark still seeds speed-12 units and normal catalogue villagers use speed 6.0 later.

- [ ] **Step 4: Pin exact normal-start cells**

In `fixture.rs` define:

```rust
pub struct TeamStart {
    pub team: TeamId,
    pub town_center_anchor: GridPos,
    pub villagers: [GridPos; 4],
}

pub struct ResourceSpawn {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub cell: GridPos,
    pub amount: u32,
}
```

Use these exact cells from the spec:

```text
T1 TC anchor: (12,46)    T2 TC anchor: (112,46)
T1 villagers: (11,45) (11,50) (16,45) (16,50)
T2 villagers: (116,45) (116,50) (111,45) (111,50)

T1 berries: (22,42) (22,54)
T1 trees:   (20,45) (20,48) (20,51)
T1 gold:    (25,48)
T2 berries: (105,42) (105,54)
T2 trees:   (107,45) (107,48) (107,51)
T2 gold:    (102,48)

Expansion SW: gold (45,16), trees (43,18) (47,18)
Expansion NE: gold (82,79), trees (84,77) (80,77)
```

- [ ] **Step 5: Add fixture occupancy regressions**

```rust
#[test]
fn villager_starts_remain_walkable_after_town_centers_are_blocked() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    for start in fixture.team_starts() {
        block_footprint_for_test(&mut map, start.town_center_anchor, (4, 4));
    }
    assert!(fixture.team_starts().iter().flat_map(|s| s.villagers).all(|cell| map.is_walkable(cell)));
}

#[test]
fn resource_cells_do_not_overlap_town_center_footprints() {
    let fixture = MapFixture::battlefield();
    let occupied = fixture.town_center_cells().collect::<std::collections::HashSet<_>>();
    assert!(fixture.resource_spawns().iter().all(|spawn| !occupied.contains(&spawn.cell)));
}
```

Also assert authored resources are in bounds and both teams have identical safe-source counts.

- [ ] **Step 6: Verify and commit**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p grus-sim
git add crates/grus-sim
git commit -m "feat: define HPA-471 catalogue and authored start"
```

---

### Task 2: Centralize player commands, shared approach slots, and finite gathering

**Files:**
- Create: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/movement.rs`
- Modify: `crates/grus-sim/src/lib.rs`
- Modify: `crates/grus-godot/src/lib.rs`
- Create: `crates/grus-sim/tests/economy_flow.rs`
- Create: `crates/grus-sim/tests/worker_commands.rs`

**Interfaces:**
- Produces `PlayerCommand` and `apply_player_command()`.
- Promotes the existing ownership lookup to a reusable internal `owned_unit_entity()` helper.
- Produces shared destination-slot reservation usable with a blocked center.
- Produces `TeamEconomy`, `ResourceStockpile`, `ResourceSource`, `Carry`, `GatherProgress`, `WorkerTask`, `Dropoff`, `ResourceIndex`.
- Produces `cancel_worker_activity()` and `step_economy()`.
- Does **not** implement Farm yet; Farm is added after buildings in Task 3.

- [ ] **Step 1: Introduce one command dispatcher while preserving Move/Stop public behavior**

```rust
pub enum PlayerCommand {
    Units(UnitCommand),
    Gather { issuer: TeamId, workers: Vec<UnitId>, source: ResourceId },
    PlaceBuilding { issuer: TeamId, builder: UnitId, kind: BuildingKind, anchor: GridPos },
    ResumeConstruction { issuer: TeamId, builder: UnitId, building: BuildingId },
    EnqueueUnit { issuer: TeamId, building: BuildingId, kind: UnitKind },
    EnqueueAgeUp { issuer: TeamId, town_center: BuildingId },
    SetRally { issuer: TeamId, building: BuildingId, target: Vec2 },
}
```

Move current unit ownership validation into `pub(crate) fn owned_unit_entity(...)`. `apply_player_command()` dispatches `PlayerCommand`; the Godot bridge only queues it and formats outcomes.

At this task only `Units` and `Gather` are implemented; later variants return a typed `NotImplementedForCurrentStage` only inside tests until their owning task lands, then that temporary test-only branch is removed before Task 2 commit. Do not merge a production placeholder path.

- [ ] **Step 2: Generalize HPA-470 slot reservation instead of adding one-worker routing**

Refactor the existing `destination_slots`/reference-counted reservation flow so it can generate ring candidates when the target center is blocked. Keep Move's explicit `target_is_walkable` guard.

Add these regressions:

```rust
#[test]
fn normal_move_to_blocked_source_is_unreachable() {
    // block source cell, issue Move to its center, assert Unreachable and old order preserved
}

#[test]
fn gather_assigns_distinct_slots_around_one_blocked_source() {
    // four villagers gather one blocked source; accepted MoveOrder goals are unique walkable perimeter cells
}
```

The group reservation loop must continue reserving current cells and existing goals so accepted siblings cannot stack.

- [ ] **Step 3: Add stockpile/source/worker/drop-off state with fractional gather progress**

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceStockpile { pub food: u32, pub wood: u32, pub gold: u32 }

#[derive(Component, Debug, Default)]
pub struct Carry { pub kind: Option<ResourceKind>, pub amount: u32 }

#[derive(Component, Debug, Default)]
pub struct GatherProgress(pub f32);

#[derive(Component, Debug)]
pub enum WorkerTask {
    Idle,
    ToSource(ResourceId),
    Gathering(ResourceId),
    ToDropoff { source: ResourceId, dropoff: BuildingId },
    ToConstruction(BuildingId),
    Constructing(BuildingId),
}

#[derive(Component, Debug)]
pub struct Dropoff { pub team: TeamId, pub building: BuildingId }
```

`ResourceSource.remaining` is `Some(u32)` for berries/trees/gold. Economy tests may spawn a minimal test-only entity with `Dropoff`; the real starting Town Center receives it in Task 3 when `Building` exists.

Gather accumulation is exact at 20 Hz:

```rust
progress.0 += gather_rate * SIM_STEP_SECONDS;
let whole = progress.0.floor() as u32;
let transferable = whole.min(carry_space).min(source_remaining);
carry.amount += transferable;
source.remaining -= transferable;
progress.0 -= transferable as f32;
```

When the worker leaves/cancels a gather job, reset `GatherProgress(0.0)`.

- [ ] **Step 4: Add validate-then-cancel worker replacement semantics**

`cancel_worker_activity(world, entity)` must:

1. inspect the old `WorkerTask`;
2. release an assigned Farm slot if one exists (Task 3 fills this branch);
3. clear an active construction builder if one exists (Task 3 fills this branch);
4. set `WorkerTask::Idle`;
5. reset `GatherProgress`;
6. remove old `MoveOrder` when replacing/stopping movement.

Do not cancel before validating the new command. Add tests:

```rust
#[test]
fn accepted_move_cancels_gather_task() { /* accepted route -> WorkerTask::Idle + new MoveOrder */ }

#[test]
fn rejected_move_preserves_existing_gather_task() { /* blocked target -> old task/order remain */ }

#[test]
fn stop_clears_worker_task_and_move_order() { /* explicit cancel */ }
```

Implement the test bodies with concrete spawned worker/source/drop-off entities; do not leave comment-only test shells in the branch.

- [ ] **Step 5: Implement finite gather/carry/deposit transitions**

`Gather` validates ownership/villager/source and reserves distinct source approach slots. For each accepted worker only: cancel old activity, install `ToSource`, and install the reserved `MoveOrder`.

`step_economy()` transitions `ToSource → Gathering → ToDropoff → repeat/Idle`, searches completed `Dropoff` components rather than building kinds, deposits only on arrival, and on finite depletion:

```rust
resource_index.remove(source_id);
map.set_blocked(source_cell, false);
world.despawn(source_entity);
```

If a required later route becomes unreachable, cancel the worker job and publish a typed rejection/feedback event rather than retrying pathfinding every tick.

- [ ] **Step 6: Lock the fixed-step order as new systems arrive**

Replace the old movement-only `advance_simulation()` integration with explicit systems ordered as:

```rust
(apply_pending_commands, advance_movement, advance_economy).chain()
```

Task 3 appends construction after economy; Task 4 appends production last. Do not create parallel fixed-update paths.

- [ ] **Step 7: Verify finite gathering and command semantics**

Required tests include:

```rust
#[test]
fn two_resources_per_second_accumulates_whole_units_at_twenty_hz() {
    let mut progress = 0.0_f32;
    let mut gathered = 0_u32;
    for _ in 0..10 {
        progress += BASE_GATHER_RATE * SIM_STEP_SECONDS;
        let whole = progress.floor() as u32;
        gathered += whole;
        progress -= whole as f32;
    }
    assert_eq!(gathered, 1);
}
```

Also prove no stockpile income before deposit, final load after depletion, and idle settlement.

- [ ] **Step 8: Verify and commit**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p grus-sim
git add crates/grus-sim crates/grus-godot
git commit -m "feat: add player commands and finite worker economy"
```

---

### Task 3: Add buildings, real drop-offs, Farm resources, construction, and full reset semantics

**Files:**
- Create: `crates/grus-sim/src/buildings.rs`
- Modify: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/lib.rs`
- Modify: `crates/grus-godot/src/lib.rs`
- Modify: `godot/scripts/reset_test.gd`
- Create: `crates/grus-sim/tests/construction.rs`

**Interfaces:**
- Produces `Building`, `ConstructionState`, `BuildingIndex`.
- Produces `validate_placement()`, `place_building()`, `assign_builder()`, `step_construction()`.
- Starting Town Centers and completed Storehouses insert `Dropoff`.
- Completed Farms allocate `ResourceId` and insert renewable `ResourceSource` with one-worker assignment.
- Produces `reset_normal_fixture()` and `reset_benchmark_fixture()` through one clear-and-seed helper.

- [ ] **Step 1: Write placement/cost/occupancy tests before implementation**

Cover out-of-bounds, occupied footprint, insufficient resources, locked Stable, non-villager builder, and unreachable perimeter. Pin one-time charging:

```rust
#[test]
fn invalid_placement_never_charges_resources() {
    let mut game = construction_world();
    let before = game.stockpile(TeamId(1));
    let result = game.place(UnitId(1), BuildingKind::House, GridPos::new(-1, 4));
    assert_eq!(result, Err(BuildRejectReason::OutOfBounds));
    assert_eq!(game.stockpile(TeamId(1)), before);
}

#[test]
fn accepted_house_charges_once_and_blocks_four_cells() {
    let mut game = construction_world();
    game.place(UnitId(1), BuildingKind::House, GridPos::new(30, 30)).unwrap();
    assert_eq!(game.stockpile(TeamId(1)).wood, 250);
    for cell in [GridPos::new(30,30), GridPos::new(31,30), GridPos::new(30,31), GridPos::new(31,31)] {
        assert!(!game.map.is_walkable(cell));
    }
}
```

- [ ] **Step 2: Implement building identity and validate-then-cancel placement**

`Building` contains `id`, `team`, `kind`, `anchor`, and construction state. Validation follows the spec order. Only after validation and a reachable reserved approach slot exist:

1. cancel the accepted builder's old worker activity;
2. deduct cost once;
3. allocate `BuildingId`;
4. block every footprint cell through `GridMap::set_blocked`;
5. spawn the under-construction entity;
6. assign the builder route/task.

Rejected placement preserves the previous worker job.

- [ ] **Step 3: Implement one active builder and shared cancellation**

`step_construction()` advances only while the assigned worker is adjacent and still constructing that `BuildingId`. `Move`, `Stop`, and accepted `Gather` call the same cancellation helper and clear `active_builder`; progress remains paused, not reset.

`ResumeConstruction` validates ownership/building/reachable approach first, then cancels the new worker's previous activity and replaces the old active builder. Add tests proving two workers never stack build speed and reassignment resumes existing progress.

- [ ] **Step 4: Bootstrap the exact starting Town Centers and attach generic drop-offs**

Seed complete Town Centers at `(12,46)` and `(112,46)` without charging cost, through the same `Building`/`BuildingIndex` representation. Block their 4×4 footprints and insert:

```rust
Dropoff { team, building: town_center_id }
```

Spawn the eight normal villagers only after those footprints are blocked and assert their authored cells remain walkable.

On Storehouse completion insert `Dropoff` exactly once.

- [ ] **Step 5: Add Farm only after building completion**

On Farm completion:

```rust
let resource_id = id_allocator.allocate_resource();
commands.entity(farm_entity).insert(ResourceSource {
    id: resource_id,
    kind: ResourceKind::Food,
    remaining: None,
    cell: farm.anchor,
    assigned_worker: None,
});
resource_index.insert(resource_id, farm_entity);
```

Do not create a Farm source before construction completes. `Gather` reserves the Farm's one worker; accepted Move/Stop/Gather-to-another-source releases it through `cancel_worker_activity()`. Add one-worker acceptance/rejection/release tests.

- [ ] **Step 6: Complete the fixed order with construction**

The integration chain becomes:

```rust
(
    apply_pending_commands,
    advance_movement,
    advance_economy,
    advance_construction,
).chain()
```

Do not change the earlier ordering.

- [ ] **Step 7: Replace unit-only reset with full gameplay clear-and-seed**

Before either reset mode, despawn every `Unit`, `Building`, and `ResourceSource`, then remove/reinitialize:

```text
UnitIndex
BuildingIndex
ResourceIndex
TeamEconomy
IdAllocator
PendingCommands contents
CommandFeedback text/revision
GridMap (replace with fresh MapFixture::battlefield().map)
```

Then seed exactly one mode:

- `reset_fixture()` → normal Town Centers/resources/economy/eight villagers;
- `reset_benchmark_fixture()` → only `units_200()` on a fresh battlefield map.

Update `reset_test.gd` to assert deterministic normal IDs and no duplicate views. The benchmark's exact 200-unit assertion remains in its own path.

- [ ] **Step 8: Verify and commit**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p grus-sim
cargo build -p grus-godot
git add crates/grus-sim crates/grus-godot godot/scripts/reset_test.gd
git commit -m "feat: add construction farms dropoffs and reset state"
```

---

### Task 4: Add production queues, population, rally points, and Age 2

**Files:**
- Create: `crates/grus-sim/src/production.rs`
- Modify: `crates/grus-sim/src/buildings.rs`
- Modify: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/lib.rs`
- Modify: `crates/grus-godot/src/lib.rs`
- Create: `crates/grus-sim/tests/production_queue.rs`

**Interfaces:**
- Produces `ProductionQueue`, `ProductionJob`, `RallyPoint`.
- Produces `population_used()`, `population_cap()`, `enqueue_unit()`, `enqueue_age_up()`, `set_rally_point()`, `step_production()`.
- New units use existing movement components and catalogue speeds.

- [ ] **Step 1: Add concrete queue charging/lock tests**

Pin these outcomes with fully constructed test worlds:

```text
Villager from TC deducts 50 Food once.
Archer from Barracks rejects WrongProducer and deducts nothing.
Stable placement/cavalry enqueue reject before Age 2.
Second Age 2 enqueue rejects as soon as the first is accepted.
A 100%-complete unit job stays queued without a second charge when population/spawn is blocked.
```

- [ ] **Step 2: Implement derived population**

`population_used()` counts live team units. `population_cap()` sums completed Town Center/House capacity and clamps to 100. Do not store a mutable duplicate counter.

- [ ] **Step 3: Implement deterministic FIFO completion**

Front jobs advance at 20 Hz. Unit jobs at 100% wait when cap/spawn clearance fails. Sort ready producers by `BuildingId` before processing. After each spawn, recompute live population before the next producer.

Required regression:

```rust
#[test]
fn two_ready_buildings_competing_for_last_slot_spawn_exactly_one() {
    let mut game = production_world_with_population(9, 10);
    game.finish_front_job(BuildingId(10));
    game.finish_front_job(BuildingId(20));
    game.step_production();
    assert_eq!(game.population_used(TeamId(1)), 10);
    assert_eq!(game.blocked_ready_jobs(), 1);
}
```

- [ ] **Step 4: Add spawn clearance and rally behavior**

Spawn on an unblocked, unoccupied perimeter cell. Allocate runtime `UnitId`. If a rally target exists, reuse the normal Move assignment after spawn; a failed rally route leaves the trained unit spawned and idle.

- [ ] **Step 5: Add Age 2 as a Town Center FIFO job**

Age-up costs 300 Food + 200 Gold, takes 45 seconds, and competes with Villager production. Completion changes the team age only in `step_production`; because production is last, the 2.2/s gather rate applies beginning with the next economy tick.

- [ ] **Step 6: Finalize the canonical fixed-update chain**

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

Add one integration test/system-order assertion that a movement arrival is visible to economy before construction/production and an age completion does not retroactively change the same tick's gather amount.

- [ ] **Step 7: Verify and commit**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p grus-sim
git add crates/grus-sim crates/grus-godot
git commit -m "feat: add production population rally and age advancement"
```

---

### Task 5: Add one runtime presentation path, bridge snapshots, and Bevy simulation-speed control

**Files:**
- Modify: `crates/grus-godot/src/lib.rs`
- Create: `godot/scenes/resource_view.tscn`
- Create: `godot/scripts/resource_view.gd`
- Create: `godot/scenes/building_view.tscn`
- Create: `godot/scripts/building_view.gd`
- Modify: `godot/scenes/unit_view.tscn`
- Modify: `godot/scripts/unit_view.gd`
- Modify: `godot/scenes/battlefield.tscn`

**Interfaces:**
- `PendingCommands` is `Vec<PlayerCommand>`.
- Bridge write methods: existing Move/Stop plus Gather, Place, Resume, Enqueue Unit, Enqueue Age, Set Rally.
- Bridge reads: economy/building/catalogue/placement/idle-worker snapshots.
- Produces `set_sim_speed(relative_speed)` for integration tests.
- Produces one `attach_missing_gameplay_views` system for Unit/Building/ResourceSource.

- [ ] **Step 1: Keep Godot as a queue/snapshot boundary**

Map integer kind codes to Rust enums in one function and reject unknown codes. `move_units()` / `stop_units()` keep their current GDScript signatures but queue `PlayerCommand::Units(...)`.

Snapshot keys stay stable:

```text
economy: food wood gold age population_used population_cap idle_workers
building: id kind complete construction_progress queue_label queue_progress blocked_reason rally_x rally_y
placement: valid anchor_x anchor_y width height reason
```

Compute snapshots directly from ECS each call.

- [ ] **Step 2: Replace fixture-only `GodotScene` attachment with one runtime system**

Create an integration-only marker such as `GameplayViewRequested`. Each `Update`, query gameplay entities missing that marker and attach the correct scene plus transform metadata:

```text
Unit           -> res://scenes/unit_view.tscn
Building       -> res://scenes/building_view.tscn
ResourceSource -> res://scenes/resource_view.tscn
```

Fixture seeding and runtime spawning must **not** separately insert `GodotScene`. All entity types use this one path. A later metadata initializer waits for `GodotNodeHandle` and sets:

```text
unit_id unit_kind team_id
building_id building_kind team_id
resource_id resource_kind
```

Add a Godot smoke assertion that one trained runtime unit and one placed runtime building both obtain views; this prevents Task 7 from discovering invisible ECS entities.

- [ ] **Step 3: Add primitive presentation only**

`resource_view.gd` and `building_view.gd` join `resource_views`/`building_views`; style kind/team/progress with primitive meshes/materials. `unit_view.gd` adds kind cues while retaining team tint and selection ring. Remove old static base markers because authoritative Town Center views replace them.

- [ ] **Step 4: Add Bevy virtual-time speed bridge for smoke tests**

The pinned godot-bevy 0.11.0 calls Bevy `app.update()` from Godot `_process` and installs `TimePlugin`; do not use `Engine.time_scale` as the economy-smoke acceleration contract.

Add:

```rust
#[func]
fn set_sim_speed(&self, relative_speed: f64) -> bool {
    if !relative_speed.is_finite() || relative_speed <= 0.0 {
        return false;
    }
    with_app_mut(|app| {
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_relative_speed(relative_speed as f32);
    })
    .is_some()
}
```

Use the existing singleton/app access pattern; if a mutable helper is extracted, keep it local to `grus-godot`. No gameplay UI calls this method.

- [ ] **Step 5: Build/import smoke**

```bash
cargo build -p grus-godot
mkdir -p godot/bin
cp target/debug/libgrus_godot.so godot/bin/libgrus_godot.so
godot --headless --path godot --editor --quit-after 120
```

Expected: import succeeds and no placeholder GDExtension classes appear.

- [ ] **Step 6: Commit**

```bash
git add crates/grus-godot godot/scenes godot/scripts
git commit -m "feat: attach runtime economy views and bridge state"
```

---

### Task 6: Add contextual gather/build/train UI with authoritative previews

**Files:**
- Modify: `godot/scenes/main.tscn`
- Modify: `godot/scripts/battlefield_controller.gd`

**Interfaces:**
- Existing controller remains the interaction owner; no new UI framework.
- Every cost/unlock/progress/blocked label comes from bridge snapshots.

- [ ] **Step 1: Expand the HUD**

Add Food/Wood/Gold, population, age, idle workers, selected entity, queue/progress, command feedback, and action buttons for the fixed seven buildings/four units/Advance Age. Hide or disable irrelevant actions instead of adding separate screens.

- [ ] **Step 2: Extend selection without breaking HPA-470 behavior**

Keep `selected_ids` for units plus one `selected_building_id`. Box selection remains units only. Clicking a building clears unit selection; clicking units clears building selection.

- [ ] **Step 3: Resolve contextual right-click before ground Move**

Order:

```text
selected villager(s) + resource view -> Gather
selected villager + incomplete owned building -> ResumeConstruction
selected production building + ground -> SetRally
selected unit(s) + ground -> Move
```

Military units never gather/build.

- [ ] **Step 4: Implement one placement mode**

A build button stores only the requested `BuildingKind` code. Mouse motion requests an authoritative preview and updates one translucent box. Left click valid preview sends Place using the lowest selected villager ID. Escape/right-click cancels preview without command/cost.

- [ ] **Step 5: Wire production/age/idle-worker navigation**

Buttons send enqueue commands against the selected building. The idle-worker control cycles bridge-provided stable IDs; no Godot-maintained idle list becomes authoritative.

- [ ] **Step 6: Manually verify normal start**

Run `godot --path godot` and verify exactly one Town Center/four villagers per team, visible safe resources, Team 1 `200/300/100`, population `4/10`, and no 200-unit fixture.

- [ ] **Step 7: Commit**

```bash
git add godot/scenes/main.tscn godot/scripts/battlefield_controller.gd
git commit -m "feat: add economy construction and production controls"
```

---

### Task 7: Preserve HPA-470 coverage and prove the complete HPA-471 loop

**Files:**
- Modify: `godot/scripts/smoke_test.gd`
- Modify: `godot/scripts/reset_test.gd`
- Modify: `godot/scripts/benchmark_200.gd`
- Create: `godot/scripts/economy_smoke_test.gd`
- Create: `godot/scenes/economy_smoke_test.tscn`
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`

**Interfaces:**
- Normal reset restores the economy start.
- Benchmark reset restores only the 200-unit movement fixture.
- Economy smoke uses real UI/contextual commands plus `GrusBridge.set_sim_speed(20.0)`; it never mutates gameplay state directly.

- [ ] **Step 1: Adapt HPA-470 smoke to speed-6 normal villagers**

Keep click/additive/box selection, control groups, move, stop, HUD shielding, unreachable feedback, camera pan/zoom, 20 Hz cadence, and interpolation.

The old speed-12 one-second assertion (`8..16` world units) is no longer valid. For a speed-6 normal villager, assert a measured one-second travel band centered on 6; use `4.0..8.0` to allow frame/headless scheduling variance while still detecting a 2×/0.5× cadence regression. Keep the interpolation max-step assertion scaled below the 0.3 fixed-tick movement step (for example `< 0.25`).

- [ ] **Step 2: Verify both reset modes**

`reset_test.gd` calls normal reset and checks eight villager views, two Town Centers, authored resource count/IDs, no duplicates, and starting stockpiles. `benchmark_200.gd` calls `reset_benchmark_fixture()` before waiting for exactly 200 unique UnitIds.

- [ ] **Step 3: Build an economy smoke that is solvent without debug grants**

At startup:

```gdscript
if not GrusBridge.set_sim_speed(20.0):
    _fail("failed to accelerate Bevy virtual time")
    return
```

Drive actual selection/right-click/buttons/placement and gather enough of all three resources. The starting 300 Wood is insufficient for the required loop, so the smoke must gather Wood before later buildings.

The smoke sequence is:

1. assign villagers to berries, trees, and gold; verify stockpile does not change until the first return/deposit;
2. continue gathering until at least the costs for House + Storehouse + Farm + Barracks + Archery Range + Archer Wood + Stable are available over the run;
3. build House and assert cap `10 → 20`;
4. build Storehouse and verify subsequent delivery can use it as a `Dropoff`;
5. build Farm, then assign one worker; verify a second worker receives Farm-occupied feedback;
6. build Barracks, enqueue/train Spearman, and assert a runtime `unit_view` appears;
7. build Archery Range, enqueue/train Archer, and assert its view appears;
8. gather enough Food/Gold, enqueue Age 2 in Town Center, assert Age becomes 2;
9. verify Stable unlocks, build it, train Cavalry, assert its view appears;
10. verify construction/queue progress and command feedback changed through normal snapshots.

Before successful exit call `GrusBridge.set_sim_speed(1.0)`.

The smoke may accelerate Bevy virtual time only. It must not grant stockpiles, force progress, mutate age, spawn entities directly, or bypass command validation.

- [ ] **Step 4: Measure real smoke duration before choosing CI timeout**

Build/stage the extension, then run:

```bash
/usr/bin/time -f 'elapsed=%e' godot --headless --path godot res://scenes/economy_smoke_test.tscn
```

Record the successful elapsed seconds in the PR verification notes. Configure the CI timeout to the next whole 15-second bucket that is at least `2 × measured elapsed + 10 seconds`. Example: a 19-second measured run requires at least 48 seconds, so use 60 seconds. Do not choose the timeout before this measurement.

- [ ] **Step 5: Add CI economy smoke with the measured timeout**

Place it after existing bridge/reset smoke and before export/benchmark. Keep export and the HPA-470 benchmark otherwise unchanged.

- [ ] **Step 6: Update README**

Document current controls, economy architecture, reset modes, build/import sequence, and that the retained performance baseline uses the benchmark-only 200-unit fixture. Preserve the pinned toolchain/export instructions and historical HPA-470 baseline numbers.

- [ ] **Step 7: Run the final gate**

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

Expected: every command exits 0; old movement smoke proves the speed-6 normal fixture still runs at 20 Hz with interpolation; economy smoke prints its success marker after the full solvent gather/build/train/advance loop.

- [ ] **Step 8: Commit final verification changes**

```bash
git add .github README.md godot
git commit -m "test: verify complete HPA-471 economy loop"
```

---

## Plan self-review

- Every HPA-471 acceptance item maps to one task in this plan.
- Review blockers are closed before implementation: Bevy virtual-time acceleration, runtime view attachment, fractional gather accounting, worker cancellation, shared approach-slot reservation, Farm/drop-off ordering, full reset, explicit fixed-tick order, and exact fixture coordinates.
- The plan reuses `UnitIndex`, destination-slot reservation, `GridMap::set_blocked`, `PendingCommands`, existing Godot views/controllers, reset smoke, and CI instead of introducing replacement frameworks.
- Costs, durations, footprints, age locks, population, speeds, and gather rates have one Rust source of truth.
- Normal gameplay never boots the 200-unit benchmark fixture; benchmark coverage remains explicit.
- No implementation step requires image generation or an asset-production pipeline.
- Combat, fog, AI, destruction, persistence, generic frameworks, and final balance remain outside this PR.
