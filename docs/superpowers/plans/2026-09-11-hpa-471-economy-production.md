# HPA-471 Worker Economy, Construction, and Army Production Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the HPA-470 battlefield into the complete HPA-471 gather/build/train/advance loop without creating a second gameplay state model.

**Architecture:** Bevy ECS remains authoritative. Four focused simulation modules (`catalog`, `economy`, `buildings`, `production`) extend the existing movement/command core; `PlayerCommand` grows only as each owning feature lands, and Godot remains a thin input/HUD/presentation layer. HPA-470 destination-slot reservation, `GridMap` revision-based occupancy, and one-way Bevy → Godot presentation are reused rather than replaced.

**Tech Stack:** Rust 1.89, Bevy 0.18.1 ECS/time, godot-bevy 0.11.0, godot-rust 0.4.5, Godot 4.6.2, GDScript, existing GitHub Actions CI.

**Spec:** `docs/superpowers/specs/2026-09-11-hpa-471-economy-production-design.md`

## Global Constraints

- One Linear ticket and one GitHub PR for all HPA-471 work; this draft remains the implementation PR.
- Bevy owns every mutable gameplay rule/state; Godot may cache only presentation and selection state.
- Keep the authoritative 20 Hz fixed simulation and interpolated Bevy → Godot transforms.
- Fixed tick order ends as `commands → movement → economy → construction → production`.
- All walkability mutations continue through `GridMap::set_blocked` / `set_blocked_rect`, except reset replacing the whole map resource with a fresh authored map.
- Move, Gather, construction approach, drop-off approach, and rally/spawn path assignment reuse HPA-470 destination-slot/reservation logic; do not add a second navigation subsystem.
- Replacement worker commands validate first and cancel the old worker task only for accepted units; rejected commands preserve the old task/order.
- Stockpile/source/carry amounts remain `u32`; fractional gather progress is `f32` per worker.
- Fixed content ceiling: Food/Wood/Gold, four unit kinds, seven building kinds, two ages, 100 population.
- Normal start: four villagers/team, 200 Food, 300 Wood, 100 Gold, one Town Center/team.
- Carry 10; gather 2.0/s in Age 1 and 2.2/s in Age 2.
- Runtime-trained units, placed buildings, and runtime resource sources must receive Godot views through the same attachment system as fixture entities.
- A Farm has both `Building` and `ResourceSource` but exactly one Godot building view; its `resource_id` is exposed on that building view for Gather targeting.
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

Add monotonic allocation methods that never emit zero. Extend `Unit` with `kind`. Update existing `spawn_unit` callers so the benchmark still seeds speed-12 units; catalogue villagers use speed 6.0 when the normal fixture lands.

- [ ] **Step 4: Pin exact normal-start cells**

In `fixture.rs` define:

```rust
pub struct TeamStart {
    pub team: TeamId,
    pub town_center_id: BuildingId,
    pub town_center_anchor: GridPos,
    pub villagers: [(UnitId, GridPos); 4],
}

pub struct ResourceSpawn {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub cell: GridPos,
    pub amount: u32,
}
```

Use these exact cells:

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

Assign deterministic authored IDs and initialize allocator counters above their maxima.

- [ ] **Step 5: Add fixture occupancy regressions**

```rust
#[test]
fn villager_starts_remain_walkable_after_town_centers_are_blocked() {
    let fixture = MapFixture::battlefield();
    let mut map = fixture.map.clone();
    for start in fixture.team_starts() {
        for cell in footprint_cells(start.town_center_anchor, 4, 4) {
            map.set_blocked(cell, true);
        }
    }
    assert!(fixture.team_starts().iter().flat_map(|s| s.villagers.iter()).all(|(_, cell)| map.is_walkable(*cell)));
}

#[test]
fn resource_cells_do_not_overlap_town_center_footprints() {
    let fixture = MapFixture::battlefield();
    let occupied = fixture.town_center_cells().collect::<std::collections::HashSet<_>>();
    assert!(fixture.resource_spawns().iter().all(|spawn| !occupied.contains(&spawn.cell)));
}
```

Also assert every authored resource is in bounds and both teams have identical safe-source counts.

- [ ] **Step 6: Verify and commit**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p grus-sim
git add crates/grus-sim
git commit -m "feat: define HPA-471 catalogue and authored start"
```

---

### Task 2: Centralize Move/Gather commands, shared approach slots, finite gathering, and starting drop-offs

**Files:**
- Create: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/movement.rs`
- Modify: `crates/grus-sim/src/lib.rs`
- Modify: `crates/grus-godot/src/lib.rs`
- Create: `crates/grus-sim/tests/economy_flow.rs`
- Create: `crates/grus-sim/tests/worker_commands.rs`

**Interfaces:**
- Produces the first fully implemented `PlayerCommand` surface: `Units(UnitCommand)` and `Gather` only.
- Promotes the existing ownership lookup to `pub(crate) fn owned_unit_entity(...)`.
- Produces shared destination-slot reservation usable with blocked source/drop-off footprints.
- Produces `TeamEconomy`, `ResourceStockpile`, `ResourceSource`, `Carry`, `GatherProgress`, `WorkerTask`, `Dropoff`, `ResourceIndex`.
- Produces the initial `cancel_worker_activity()` and `step_economy()`.
- Seeds authored finite resources and starting Town Center `Dropoff` entities, but does **not** add the full `Building` component or Farm yet.

- [ ] **Step 1: Add the first exhaustive PlayerCommand enum**

```rust
pub enum PlayerCommand {
    Units(UnitCommand),
    Gather {
        issuer: TeamId,
        workers: Vec<UnitId>,
        source: ResourceId,
    },
}
```

`apply_player_command()` handles both variants exhaustively. Move current unit ownership validation into `owned_unit_entity()`. Keep Godot `move_units()` / `stop_units()` signatures unchanged; they queue `PlayerCommand::Units`.

Task 3 extends this enum with Place/Resume; Task 4 extends it with production/age/rally. No placeholder variants exist in intermediate commits.

- [ ] **Step 2: Generalize HPA-470 reservation logic for blocked targets/footprints**

Extract the current reference-counted reservation loop so callers can request candidate slots around a one-cell source or rectangular footprint while preserving current-cell/existing-goal reservations.

Keep Move's target walkability rule. Add concrete tests that:

- Move to a blocked source cell returns `Unreachable` and preserves the prior order;
- four villagers gathering the same blocked source receive four unique walkable `MoveOrder.goal` cells;
- rejected workers keep their old reservations so accepted siblings cannot steal their cell/goal.

- [ ] **Step 3: Add stockpile/source/worker/drop-off state with fractional progress**

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
pub struct Dropoff {
    pub team: TeamId,
    pub building: BuildingId,
    pub anchor: GridPos,
    pub width: u8,
    pub height: u8,
}
```

`ResourceSource.remaining` is `Some(u32)` for berries/trees/gold. Seed one starting Dropoff entity per Town Center using the authored `BuildingId`, anchor, and 4×4 geometry. Task 3 adds the full `Building` component to those same entities; it does not replace them.

- [ ] **Step 4: Implement exact 20 Hz fractional gathering**

For finite sources:

```rust
progress.0 += gather_rate * SIM_STEP_SECONDS;
let whole = progress.0.floor() as u32;
let source_remaining = source.remaining.expect("finite source");
let transferable = whole.min(CARRY_LIMIT - carry.amount).min(source_remaining);
carry.amount += transferable;
source.remaining = Some(source_remaining - transferable);
progress.0 -= transferable as f32;
```

When a worker cancels/leaves the gathering job, reset `GatherProgress(0.0)`.

Required test:

```rust
#[test]
fn two_resources_per_second_yields_one_whole_resource_after_ten_fixed_ticks() {
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

- [ ] **Step 5: Add initial validate-then-cancel worker replacement semantics**

In Task 2, `cancel_worker_activity()` handles the components that exist now:

```text
set WorkerTask::Idle
reset GatherProgress
remove the old MoveOrder when replacement/Stop requires it
```

Move and Gather validate route/source/ownership before invoking it for an accepted worker. Stop invokes it unconditionally for each owned worker. Task 3 extends this same helper to release Farm assignment and clear active construction builder.

Add fully implemented tests for these outcomes:

- accepted Move while Gathering leaves the worker `Idle` with the new MoveOrder;
- rejected Move to a blocked target leaves the previous Gather task/order unchanged;
- accepted Gather to a new source replaces the old Gather task and resets fractional progress;
- Stop clears WorkerTask, fractional progress, and MoveOrder.

- [ ] **Step 6: Implement finite Gather → Dropoff → repeat/Idle**

`Gather` validates owned villagers/source, reserves distinct source approach slots, then cancels and replaces only accepted workers.

`step_economy()`:

1. switches `ToSource → Gathering` when the worker reaches its reserved source slot;
2. accumulates/whole-transfers finite resources into Carry;
3. finds the nearest reachable same-team `Dropoff` footprint using the shared reservation helper;
4. routes `ToDropoff` and deposits the whole Carry only on arrival;
5. routes back if the source still exists;
6. after depletion, delivers any final Carry and settles Idle;
7. if a required later route cannot be found, settles Idle with typed feedback rather than retrying A* every tick.

When a finite source reaches zero:

```rust
resource_index.remove(source_id);
map.set_blocked(source_cell, false);
world.despawn(source_entity);
```

- [ ] **Step 7: Seed the normal finite economy start**

Normal setup inserts:

- `TeamEconomy` for both teams with `200/300/100`, Age 1;
- two Town Center Dropoff entities at the exact authored anchors;
- eight Villagers with `Carry`, `GatherProgress`, `WorkerTask::Idle`;
- all authored berries/trees/gold as blocked one-cell `ResourceSource` entities;
- `UnitIndex`, `ResourceIndex`, and allocator counters.

The full `Building`/`BuildingIndex` representation is deliberately deferred to Task 3.

- [ ] **Step 8: Lock the first part of fixed-step ordering**

Replace the old movement-only advancement with:

```rust
(apply_pending_commands, advance_movement, advance_economy).chain()
```

Task 3 appends construction; Task 4 appends production. Do not create a parallel update path.

- [ ] **Step 9: Verify and commit**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p grus-sim
git add crates/grus-sim crates/grus-godot
git commit -m "feat: add commands and finite worker economy"
```

---

### Task 3: Add buildings, construction, Storehouse drop-offs, Farm sources, and full reset semantics

**Files:**
- Create: `crates/grus-sim/src/buildings.rs`
- Modify: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/lib.rs`
- Modify: `crates/grus-godot/src/lib.rs`
- Modify: `godot/scripts/reset_test.gd`
- Create: `crates/grus-sim/tests/construction.rs`

**Interfaces:**
- Extends `PlayerCommand` with `PlaceBuilding` and `ResumeConstruction`; both are fully implemented in the same commit.
- Produces `Building`, `ConstructionState`, `BuildingIndex`.
- Produces `validate_placement()`, `place_building()`, `assign_builder()`, `step_construction()`.
- Upgrades the existing starting Town Center Dropoff entities with complete `Building` components.
- Completed Storehouses insert `Dropoff`; completed Farms allocate `ResourceId` and insert renewable one-worker `ResourceSource`.
- Extends `cancel_worker_activity()` with Farm/construction cleanup.
- Produces normal/benchmark reset through one clear-and-seed helper.

- [ ] **Step 1: Extend PlayerCommand exhaustively for construction**

```rust
pub enum PlayerCommand {
    Units(UnitCommand),
    Gather { issuer: TeamId, workers: Vec<UnitId>, source: ResourceId },
    PlaceBuilding { issuer: TeamId, builder: UnitId, kind: BuildingKind, anchor: GridPos },
    ResumeConstruction { issuer: TeamId, builder: UnitId, building: BuildingId },
}
```

Add matching dispatcher branches in the same change.

- [ ] **Step 2: Write placement/cost/occupancy tests**

Cover out-of-bounds, occupied footprint, insufficient resources, locked Stable, non-villager builder, unreachable approach, and one-time charging.

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

- [ ] **Step 3: Implement validate-then-cancel placement/construction**

Only after placement validation and a reachable reserved builder slot succeed:

1. call `cancel_worker_activity()` for the accepted builder;
2. deduct cost once;
3. allocate `BuildingId`;
4. block every footprint cell using `GridMap::set_blocked`;
5. spawn the incomplete Building;
6. assign builder MoveOrder/WorkerTask.

Rejected placement preserves the previous worker task/order.

`ResumeConstruction` similarly validates building ownership/completion/reachable slot before replacing the builder's previous work.

- [ ] **Step 4: Extend worker cancellation for Farm/construction state**

When the old task references a Farm assignment, clear that source's `assigned_worker` if it matches. When it references active construction, clear `ConstructionState.active_builder` if it matches. Then perform the existing Idle/progress/MoveOrder cleanup.

Tests must prove Move/Stop/Gather pause construction and release a Farm, while a rejected replacement command preserves both.

- [ ] **Step 5: Upgrade authored Town Center entities to full Buildings**

Reuse the exact two entities/`BuildingId`s created in Task 2. Insert completed `Building` plus `BuildingIndex` entries; do not spawn replacement Town Centers. Their 4×4 footprints are already blocked and their `Dropoff` components remain authoritative routing geometry.

- [ ] **Step 6: Implement one active builder and completion effects**

`step_construction()` advances one active builder only. Pausing leaves accumulated progress intact. On completion:

- House affects only derived population cap;
- Storehouse inserts `Dropoff { team, building, anchor, width: 2, height: 2 }` exactly once;
- Farm allocates a new `ResourceId` and inserts a renewable Food `ResourceSource { remaining: None, assigned_worker: None, ... }` on the same entity;
- Barracks/Archery Range/Stable become complete producers for Task 4.

Required tests: two builders never double speed; reassignment resumes progress; Storehouse becomes a drop-off; Farm gets a unique runtime `ResourceId` and only one worker can reserve it.

- [ ] **Step 7: Append construction to the fixed chain**

```rust
(
    apply_pending_commands,
    advance_movement,
    advance_economy,
    advance_construction,
).chain()
```

Do not change prior order.

- [ ] **Step 8: Replace unit-only reset with full unique-entity clear and fresh map**

Collect a `HashSet<Entity>` containing every entity with `Unit`, `Building`, or `ResourceSource`, then despawn each once so Farm entities are not double-despawned.

Remove/reinitialize:

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

- `reset_fixture()` → normal Town Centers/drop-offs/buildings/resources/eight villagers/economy;
- `reset_benchmark_fixture()` → only `units_200()` on a fresh battlefield map.

Update `reset_test.gd` for deterministic normal IDs/no duplicate views. Benchmark exact-200 coverage stays in benchmark smoke.

- [ ] **Step 9: Verify and commit**

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
- Extends `PlayerCommand` with `EnqueueUnit`, `EnqueueAgeUp`, and `SetRally`; all three are fully implemented in this task.
- Produces `ProductionQueue`, `ProductionJob`, `RallyPoint`.
- Produces `population_used()`, `population_cap()`, `enqueue_unit()`, `enqueue_age_up()`, `set_rally_point()`, `step_production()`.
- Runtime units use existing movement components and catalogue speeds.

- [ ] **Step 1: Extend PlayerCommand exhaustively for production**

Final enum:

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

Add all dispatcher branches in the same change.

- [ ] **Step 2: Add concrete queue charging/lock tests**

Construct real test worlds and pin these results:

```text
Villager from TC deducts 50 Food exactly once.
Archer from Barracks rejects WrongProducer and deducts nothing.
Stable placement/cavalry enqueue reject before Age 2.
Second Age 2 enqueue rejects as soon as the first is accepted.
A 100%-complete unit job remains queued without another charge when population or spawn is blocked.
```

- [ ] **Step 3: Implement derived population**

`population_used()` counts live team units. `population_cap()` sums completed Town Center/House capacity and clamps to 100. Do not store a mutable duplicate counter.

- [ ] **Step 4: Implement deterministic FIFO completion**

Front jobs advance at 20 Hz. Unit jobs at 100% wait when cap/spawn clearance fails. Sort ready producers by `BuildingId`; after each successful spawn recompute live population.

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

- [ ] **Step 5: Add spawn clearance and rally behavior**

Reserve an unblocked, unoccupied perimeter slot using the shared reservation helper. Allocate runtime `UnitId`. If a rally target exists, assign a normal Move after spawn; failed rally routing leaves the trained unit spawned and idle.

- [ ] **Step 6: Add Age 2 as a Town Center FIFO job**

Age-up costs 300 Food + 200 Gold, takes 45 seconds, and competes with Villager production. Completion changes age only in `step_production`; because production is last, the 2.2/s gather rate starts on the next economy tick.

- [ ] **Step 7: Finalize canonical fixed-update order**

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

Add an integration test proving an arrival can be consumed by economy/construction after movement in that tick and an age completion does not change the already-run economy step.

- [ ] **Step 8: Verify and commit**

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
- `PendingCommands` is now the final `Vec<PlayerCommand>`.
- Bridge write methods: existing Move/Stop plus Gather, Place, Resume, Enqueue Unit, Enqueue Age, Set Rally.
- Bridge reads: economy/building/catalogue/placement/idle-worker snapshots.
- Produces `set_sim_speed(relative_speed)` for integration tests.
- Produces one `attach_missing_gameplay_views` system; Farm is handled by the Building branch only.

- [ ] **Step 1: Keep Godot as queue/snapshot boundary**

Map integer kind codes to Rust enums in one function and reject unknown codes. `move_units()` / `stop_units()` keep their current GDScript signatures but queue `PlayerCommand::Units(...)`.

Stable snapshot keys:

```text
economy: food wood gold age population_used population_cap idle_workers
building: id kind complete construction_progress queue_label queue_progress blocked_reason rally_x rally_y
placement: valid anchor_x anchor_y width height reason
```

Compute snapshots from ECS each call; do not maintain mirrored stockpiles/queues in GDScript.

- [ ] **Step 2: Replace fixture-only scene insertion with one runtime attachment system**

Create `GameplayViewRequested` in `grus-godot`. In one Update system:

```text
Unit                                      -> unit_view.tscn
Building                                  -> building_view.tscn
ResourceSource AND NOT Building           -> resource_view.tscn
```

Insert the appropriate `GodotScene`, static/initial transform, sync metadata, and marker exactly once. Fixture seeding and runtime spawn code must not directly insert `GodotScene` anymore.

A metadata initializer waits for `GodotNodeHandle` and sets:

```text
unit:     unit_id unit_kind team_id
building: building_id building_kind team_id
resource: resource_id resource_kind
```

If a Building also has `ResourceSource` (Farm), add `resource_id`/`resource_kind` to **that same building node**. Do not attach `resource_view.tscn` to Farm.

Add integration coverage that a newly trained unit, newly placed building, and completed Farm all receive exactly one appropriate view.

- [ ] **Step 3: Add primitive presentation**

`resource_view.gd` and `building_view.gd` join their groups and style kind/team/progress with primitive meshes/materials. `unit_view.gd` adds kind cues while retaining team tint/selection ring. Remove old static base markers; authoritative Town Center views replace them.

- [ ] **Step 4: Add Bevy virtual-time speed bridge**

The pinned godot-bevy 0.11.0 runs Bevy `app.update()` from Godot `_process` and installs Bevy `TimePlugin`; do not use `Engine.time_scale` as the economy-smoke contract.

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
        .set_relative_speed(relative_speed as f32);
    true
}
```

No gameplay UI calls this method.

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
- Existing controller remains interaction owner; no new UI framework.
- Every cost/unlock/progress/blocked label comes from bridge snapshots.

- [ ] **Step 1: Expand HUD**

Add Food/Wood/Gold, population, age, idle workers, selected entity, queue/progress, command feedback, and action buttons for the fixed seven buildings/four units/Advance Age. Hide/disable irrelevant actions instead of adding separate screens.

- [ ] **Step 2: Extend selection without breaking HPA-470 behavior**

Keep `selected_ids` for units plus one `selected_building_id`. Box selection remains units only. Clicking a building clears unit selection; clicking units clears building selection.

- [ ] **Step 3: Resolve contextual right-click before ground Move**

Order:

```text
selected villager(s) + standalone resource view -> Gather(resource_id)
selected villager(s) + completed Farm building view -> Gather(resource_id metadata)
selected villager + incomplete owned building -> ResumeConstruction
selected production building + ground -> SetRally
selected unit(s) + ground -> Move
```

Military units never gather/build.

- [ ] **Step 4: Implement one placement mode**

A build button stores only the requested `BuildingKind` code. Mouse motion requests authoritative preview and updates one translucent box. Left click valid preview sends Place using the lowest selected villager ID. Escape/right-click cancels preview without command/cost.

- [ ] **Step 5: Wire production/age/idle-worker navigation**

Buttons send enqueue commands against the selected building. Idle-worker control cycles bridge-provided stable IDs; no Godot-maintained idle list becomes authoritative.

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

The old speed-12 one-second assertion (`8..16` world units) is invalid after normal units move at 6.0. Use a `4.0..8.0` one-second distance band and keep the visual-step assertion below the 0.3-unit fixed-tick step (target `< 0.25`).

- [ ] **Step 2: Verify both reset modes**

`reset_test.gd` calls normal reset and checks eight villager views, two Town Centers, authored standalone resource IDs/views, starting stockpiles, and no duplicates. `benchmark_200.gd` calls `reset_benchmark_fixture()` before waiting for exactly 200 unique UnitIds.

- [ ] **Step 3: Build a solvent economy smoke without debug grants**

At startup:

```gdscript
if not GrusBridge.set_sim_speed(20.0):
    _fail("failed to accelerate Bevy virtual time")
    return
```

Drive actual selection/right-click/buttons/placement. Required sequence:

1. assign villagers to berries, trees, and gold; verify stockpile changes only on return/deposit;
2. continue gathering enough Wood over the run to pay House + Storehouse + Farm + Barracks + Archery Range + Archer Wood + Stable (615 Wood total, so starting 300 is insufficient);
3. gather enough Food/Gold for Spearman + Archer + Age 2 + Cavalry (480 Food and 260 Gold total, so starts are also insufficient);
4. build House; assert cap `10 → 20`;
5. build Storehouse; verify a later delivery can choose it as Dropoff;
6. build Farm; gather it with one worker and verify a second worker receives Farm-occupied feedback;
7. build Barracks, train Spearman, assert runtime `unit_view` appears;
8. build Archery Range, train Archer, assert its runtime view appears;
9. enqueue Age 2 in Town Center, assert Age becomes 2;
10. verify Stable unlocks, build it, train Cavalry, assert runtime view appears;
11. verify construction/queue progress and command feedback through normal snapshots.

Before success exit:

```gdscript
if not GrusBridge.set_sim_speed(1.0):
    _fail("failed to restore Bevy virtual time")
    return
```

The smoke may accelerate Bevy virtual time only. It must not grant stockpiles, force progress, mutate age, spawn entities directly, or bypass command validation.

- [ ] **Step 4: Measure real smoke duration before choosing CI timeout**

```bash
/usr/bin/time -f 'elapsed=%e' godot --headless --path godot res://scenes/economy_smoke_test.tscn
```

Record successful elapsed seconds in PR verification notes. Set CI timeout to the next whole 15-second bucket that is at least `2 × measured elapsed + 10 seconds`. Example: measured 19 seconds → requirement 48 seconds → choose 60 seconds. Do not preselect 60 seconds before measurement.

- [ ] **Step 5: Add CI smoke with measured timeout**

Place it after bridge/reset smoke and before export/benchmark. Keep export and HPA-470 benchmark logic unchanged except benchmark fixture reset.

- [ ] **Step 6: Update README**

Document current controls, economy architecture, reset modes, build/import sequence, and benchmark-only 200-unit fixture. Preserve pinned toolchain/export instructions and historical HPA-470 baseline numbers.

- [ ] **Step 7: Run final gate**

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

Expected: every command exits 0; movement smoke proves speed-6 normal fixture still runs at 20 Hz with interpolation; economy smoke prints its success marker after the full solvent gather/build/train/advance loop.

- [ ] **Step 8: Commit final verification changes**

```bash
git add .github README.md godot
git commit -m "test: verify complete HPA-471 economy loop"
```

---

## Plan self-review

- Every HPA-471 acceptance item maps to one task above.
- Review blockers are closed before implementation: Bevy virtual-time acceleration, runtime view attachment, fractional gather accounting, worker cancellation, shared approach-slot reservation, Farm/drop-off ordering, full reset, explicit fixed-tick order, and exact fixture coordinates.
- `PlayerCommand` grows only with implemented behavior; no intermediate placeholder variants are required.
- Completed Farm gets one building view plus `resource_id` metadata, not overlapping building/resource views.
- The plan reuses `UnitIndex`, destination-slot reservation, `GridMap::set_blocked`, `PendingCommands`, existing Godot view/controller patterns, reset smoke, and CI instead of introducing replacement frameworks.
- Costs, durations, footprints, age locks, population, speeds, and gather rates have one Rust source of truth.
- Normal gameplay never boots the 200-unit benchmark fixture; benchmark coverage remains explicit.
- No implementation step requires image generation or an asset-production pipeline.
- Combat, fog, AI, destruction, persistence, generic frameworks, and final balance remain outside this PR.
