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
- Do **not** switch normal Godot startup away from the 200-unit fixture until Task 6, where smoke/reset/benchmark assumptions are changed atomically.
- Final fixed tick order is `commands → movement → economy → construction → production`.
- All walkability changes use `GridMap::set_blocked` / `set_blocked_rect`, except reset replacing the map resource with a fresh authored map.
- Reuse HPA-470 reference-counted current/goal reservations and A*. Do not add a persistent reservation table or navigation service.
- Move keeps its existing walkable-target ring behavior. Gather/build/drop-off uses immediate-adjacent `approach_slots`; workers never gather/build from radius 2+.
- Replacement worker commands validate first; only accepted workers have old activity canceled. Rejected commands preserve old task/order.
- Carried resources are never discarded by retasking.
- One `RejectReason` and one `CommandResult` cover every command family; bridge tests assert reject codes, not English strings.
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
- Produces `BuildingId`, `ResourceId`, `IdAllocator`.
- Produces `Footprint` and immediate-perimeter enumeration.
- Produces invariant-preserving `Carry`, `GatherProgress`, `WorkerTask`, `ResourceStockpile`, `TeamEconomy`, `Dropoff` types; behavior comes in later tasks.
- Replaces `CommandOutcome` / `CommandRejectReason` with `CommandResult` / `RejectReason`.
- Wraps current Move/Stop in the first exhaustive `PlayerCommand::Units` without changing public Godot Move/Stop signatures.
- Produces `approach_slots()` candidate generation while keeping Move's existing destination generation.
- Replaces old untyped fixture resource arrays with typed descriptors; keeps `left_spawn`, `right_spawn`, and `units_200()`.

- [ ] **Step 1: Add the static catalogue and pin only meaningful catalogue invariants**

Define:

```rust
pub enum ResourceKind { Food, Wood, Gold }
pub enum UnitKind { Villager, Spearman, Archer, Cavalry }
pub enum BuildingKind { TownCenter, House, Storehouse, Farm, Barracks, ArcheryRange, Stable }
pub enum Age { Age1, Age2 }

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Cost { pub food: u32, pub wood: u32, pub gold: u32 }
```

Use `match`-based `unit_spec()` / `building_spec()` with the spec values. Keep one non-tautological gating test:

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

- [ ] **Step 2: Add IDs, Footprint, and worker/economy contract types**

In `ids.rs`:

```rust
pub struct BuildingId(pub u32);
pub struct ResourceId(pub u32);

#[derive(Debug, Resource)]
pub struct IdAllocator {
    pub next_unit: u32,
    pub next_building: u32,
    pub next_resource: u32,
}
```

In `map.rs`:

```rust
#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub struct Footprint {
    pub anchor: GridPos,
    pub width: u8,
    pub height: u8,
}
```

Add `cells()` and `perimeter_cells()`; perimeter must never include footprint cells.

In `economy.rs` define:

```rust
pub enum Carry {
    Empty,
    Holding { kind: ResourceKind, amount: std::num::NonZeroU32 },
}

#[derive(Component, Debug, Default)]
pub struct GatherProgress(pub f32);

pub enum WorkerTask {
    Idle,
    ToSource { source: ResourceId, slot: GridPos },
    Gathering { source: ResourceId },
    ToDropoff { source: ResourceId, dropoff: BuildingId, slot: GridPos },
    ToConstruction { building: BuildingId, slot: GridPos },
    Constructing { building: BuildingId },
}

pub struct Dropoff { pub team: TeamId }
```

Also define `ResourceStockpile` and `TeamEconomy`, but do not add gathering behavior yet.

- [ ] **Step 3: Unify command results before adding new commands**

In `commands.rs` define:

```rust
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

Make `apply_player_command(&mut World, &mut GridMap, PlayerCommand) -> CommandResult` dispatch Move/Stop. Preserve the existing per-unit Move/Stop semantics.

Update `grus-godot` to queue `PlayerCommand::Units` while keeping `move_units()` / `stop_units()` GDScript signatures unchanged. Extend `CommandFeedback` with `last_reject_code`, but keep readable text including the word `unreachable` so the pre-cutover HPA-470 smoke remains green.

- [ ] **Step 4: Add immediate approach-slot candidate generation without changing Move**

Keep the current `destination_slots()` ring search for Move. Add:

```rust
pub(crate) fn approach_slots(
    map: &GridMap,
    footprint: Footprint,
    used: &std::collections::HashSet<GridPos>,
    count: usize,
) -> Vec<GridPos>
```

It returns only walkable, unreserved cells from `footprint.perimeter_cells()`, sorted deterministically by distance/tie-break. It never includes the blocked center/footprint and never scans a wider radius.

Tests:

```rust
#[test]
fn approach_slots_never_include_the_blocked_footprint() {
    let mut map = GridMap::new(8, 8);
    let footprint = Footprint { anchor: GridPos::new(3, 3), width: 2, height: 2 };
    for cell in footprint.cells() { map.set_blocked(cell, true); }
    let slots = approach_slots(&map, footprint, &Default::default(), 8);
    assert!(slots.iter().all(|slot| !footprint.cells().contains(slot)));
    assert!(slots.iter().all(|slot| footprint.is_immediately_adjacent(*slot)));
}
```

Do not route workers yet; this step only locks candidate semantics.

- [ ] **Step 5: Replace fixture resource tables with one typed source of truth**

Define `TeamStart` and `ResourceSpawn` with deterministic IDs/cells. Use exact coordinates from the spec. Delete `starting_resources` and `expansion_resources`; keep `left_spawn` / `right_spawn` for benchmark movement.

Add tests that Town Center footprints do not overlap resources, villager starts remain walkable after Town Center blocking, all resource cells are in bounds, and safe-source counts mirror across teams.

- [ ] **Step 6: Keep existing HPA-470 runtime green and commit**

Run:

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

Do not change Startup fixture semantics in this task.

Commit:

```bash
git add crates/grus-sim crates/grus-godot
git commit -m "feat: lock HPA-471 domain and command contracts"
```

---

### Task 2: Add authoritative buildings, construction, and the pure skirmish seed

**Files:**
- Create: `crates/grus-sim/src/buildings.rs`
- Modify: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/fixture.rs`
- Modify: `crates/grus-sim/src/lib.rs`

**Interfaces:**
- Extends `PlayerCommand` with `PlaceBuilding` and `ResumeConstruction` in the same commit that implements them.
- Produces `Building`, `ConstructionState`, `BuildingIndex`, placement validation, construction stepping.
- Produces the pure Rust `seed_skirmish()` path with real complete Town Center Buildings/Footprints and Villagers from birth.
- Starting Town Centers have `Dropoff { team }`; completed Storehouses gain the same marker.
- Does not switch Godot Startup; HPA-470 runtime remains 200 units.

- [ ] **Step 1: Add Building and construction state**

Define:

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

Every Building entity also owns one `Footprint`. Add `BuildingIndex` matching the existing `UnitIndex` pattern.

- [ ] **Step 2: Add local in-module test setup, not a new public test harness**

Inside `buildings.rs` `#[cfg(test)] mod tests`, define a private helper such as:

```rust
fn setup_build_test() -> (World, GridMap, Entity) {
    let mut world = World::new();
    world.insert_resource(TeamEconomy::single_test_team(
        TeamId(1),
        ResourceStockpile { food: 500, wood: 500, gold: 500 },
    ));
    world.insert_resource(IdAllocator::for_tests());
    let map = GridMap::new(64, 64);
    let villager = spawn_unit(
        &mut world,
        UnitId(1),
        TeamId(1),
        UnitKind::Villager,
        Vec2::new(10.5, 10.5),
        unit_spec(UnitKind::Villager).speed,
    );
    world.entity_mut(villager).insert((Carry::Empty, GatherProgress::default(), WorkerTask::Idle));
    (world, map, villager)
}
```

Keep this helper private to the test module.

- [ ] **Step 3: Implement placement validation and immediate occupancy**

Extend `PlayerCommand`:

```rust
PlaceBuilding { issuer: TeamId, builder: UnitId, kind: BuildingKind, anchor: GridPos },
ResumeConstruction { issuer: TeamId, builder: UnitId, building: BuildingId },
```

Validation order: owned villager → unlock/buildable → bounds → footprint walkable → affordability → reachable immediate approach slot.

Only accepted placement:

1. cancels the builder's old accepted activity;
2. deducts cost once;
3. allocates `BuildingId`;
4. blocks every footprint cell;
5. spawns `Building + Footprint`;
6. stores/routs the builder to `ToConstruction { building, slot }`.

Use `RejectReason` only; do not create `BuildRejectReason`.

- [ ] **Step 4: Implement construction and retasking semantics**

`step_construction()` begins work only after the worker reaches the stored adjacent slot and transitions to `Constructing`. One builder advances progress. Accepted Move/Stop/Place/Resume cancels/pause as appropriate; rejected replacement commands preserve the active builder.

Tests must directly exercise production code:

```rust
#[test]
fn rejected_replacement_preserves_active_builder() {
    let (mut world, mut map, villager) = setup_build_test();
    let building = place_test_house(&mut world, &mut map, villager, GridPos::new(12, 12));
    advance_builder_to_site(&mut world, &mut map, villager, building);
    let before = world.get::<WorkerTask>(villager).unwrap().clone();

    let result = apply_player_command(
        &mut world,
        &mut map,
        PlayerCommand::Units(UnitCommand::move_one(TeamId(1), UnitId(1), Vec2::new(20.5, 20.5))),
    );

    assert!(result.reject.is_none());
    assert_ne!(world.get::<WorkerTask>(villager).unwrap(), &before);
}
```

Also cover invalid placement no charge, accepted House exactly -50 Wood, footprint blocked, two builders not stacking speed, and Resume preserving accumulated progress.

- [ ] **Step 5: Build the pure skirmish seed with real Town Centers**

Add a pure simulation seeding function used by Rust tests and later Godot integration. It creates:

- both `TeamEconomy` entries;
- two completed `BuildingKind::TownCenter` entities with 4×4 Footprints and `Dropoff`;
- four Villagers/team with `Carry::Empty`, `GatherProgress(0)`, `WorkerTask::Idle`;
- Town Center footprint occupancy;
- `UnitIndex`, `BuildingIndex`, allocator counters.

No temporary Dropoff-only Town Center entity exists.

- [ ] **Step 6: Add Storehouse completion effect**

When a Storehouse reaches completion, insert `Dropoff { team }` once on the same Building entity. Economy later reads `Dropoff + Building + Footprint`.

- [ ] **Step 7: Verify pure simulation plus unchanged Godot baseline and commit**

Run the Task 1 gate. Godot still boots the 200-unit fixture.

Commit:

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
- Produces `ResourceSource`, `ResourceIndex`, finite source seeding, Farm source creation, `step_economy()`.
- Uses existing `Building + Footprint + Dropoff` for delivery geometry.
- Uses stored approach slots for positive arrival detection.

- [ ] **Step 1: Add finite ResourceSource and typed index**

Define:

```rust
#[derive(Component, Debug)]
pub struct ResourceSource {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub remaining: Option<u32>,
    pub assigned_worker: Option<UnitId>,
}
```

Standalone authored sources spawn as `ResourceSource + Footprint { width: 1, height: 1 }` and block their cells. `ResourceIndex` follows `UnitIndex`.

Extend `seed_skirmish()` so both teams and expansion nodes use the single typed `ResourceSpawn` table.

- [ ] **Step 2: Implement Gather assignment using shared reservations**

Add:

```rust
Gather { issuer: TeamId, workers: Vec<UnitId>, source: ResourceId }
```

Gather validates source, owned Villagers, Farm availability, and immediate approach slots. Run the same reference-counted reservation selection used by Move so accepted siblings get distinct goals and rejected siblings keep old reservations.

Required tests:

```rust
#[test]
fn move_to_blocked_source_rejects_but_gather_uses_adjacent_slots() {
    // Set up four villagers and a blocked one-cell Food source.
    // Move to the source center rejects Unreachable.
    // Gather accepts four villagers and every ToSource.slot is unique,
    // walkable, and immediately adjacent to the source footprint.
}
```

Implement the test with local setup helpers in `economy.rs`; do not create an undefined public `game` harness.

- [ ] **Step 3: Implement Carry-preserving retasking**

Rules:

```text
Move/Stop/Place/Resume: preserve Carry.
Gather + Carry::Empty: route directly to source.
Gather + Carry::Holding: route to nearest Dropoff first, then requested source.
Never mix resource kinds in one Carry.
```

`cancel_worker_activity()` clears task/Farm/builder/progress/MoveOrder but does not touch Carry.

Add a test that a worker carrying 6 Wood, then Gather-targeted to berries, deposits 6 Wood before beginning Food gathering; no resource disappears.

- [ ] **Step 4: Implement actual fixed-step gathering and deposit**

`step_economy()` uses the stored slot to detect arrival. While Gathering:

```rust
progress.0 += gather_rate_for_age(team_age) * SIM_STEP_SECONDS;
let whole = progress.0.floor() as u32;
let carry_space = CARRY_LIMIT - carry.amount_or_zero();
let available = source.remaining.unwrap_or(u32::MAX);
let transferred = whole.min(carry_space).min(available);
```

Transfer only whole units into Carry and subtract exactly the transferred whole count from progress. Deposit changes stockpile only after reaching stored `ToDropoff.slot`.

The primary accumulator test must exercise production code rather than copy it:

```rust
#[test]
fn ten_fixed_gather_ticks_produce_one_food_in_carry() {
    let mut setup = adjacent_food_worker_world(600);
    issue_gather(&mut setup.world, &mut setup.map, UnitId(1), ResourceId(1));
    settle_worker_at_source_slot(&mut setup.world, UnitId(1));
    for _ in 0..10 {
        step_economy(&mut setup.world, &mut setup.map, SIM_STEP_SECONDS);
    }

    assert_eq!(carry_of(&setup.world, UnitId(1)), CarryValue::Food(1));
    assert_eq!(remaining_of(&setup.world, ResourceId(1)), Some(599));
}
```

`CarryValue` may be a private test helper converting `Carry` to easy assertions; it is not production API.

- [ ] **Step 5: Handle depletion, unreachable follow-up, and Farm assignment**

Finite depletion removes the source from `ResourceIndex`, despawns it, and unblocks its 1×1 footprint. Worker delivers final load then becomes Idle.

Completed Farm adds a runtime `ResourceId` and `ResourceSource { kind: Food, remaining: None, assigned_worker: None }` to the existing Farm Building entity. A second worker rejects `FarmOccupied`; cancellation/retask releases the assignment.

- [ ] **Step 6: Add one real cross-module integration test**

`tests/economy_flow.rs` uses only public APIs to prove:

```text
seed skirmish → gather Wood → deposit at Town Center → place/complete Storehouse →
issue another gather → delivery can select Storehouse → finite depletion final load → Idle
```

Keep all lower-level ownership/cancellation/accumulator tests in module-local `#[cfg(test)]` blocks.

- [ ] **Step 7: Verify unchanged Godot baseline and commit**

Run the Task 1 gate. Godot still boots 200 units and does not yet expose Gather UI.

Commit:

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

**Interfaces:**
- Extends `PlayerCommand` with `EnqueueUnit`, `EnqueueAgeUp`, `SetRally` in the same commit that implements them.
- Produces `ProductionQueue`, `ProductionJob`, `RallyPoint`, population helpers, spawn clearance, production stepping.
- Keeps Age 2's 2.2/s gather improvement.

- [ ] **Step 1: Add production state and producer compatibility**

Define FIFO `ProductionQueue` on complete producers and catalogue-driven compatibility:

```text
Town Center -> Villager, Age 2
Barracks -> Spearman
Archery Range -> Archer
Stable -> Cavalry
```

Use `RejectReason::WrongProducer`, `Locked`, and `InsufficientResources`; do not add production-specific reject enums.

- [ ] **Step 2: Implement enqueue charging and one-time Age 2 lock**

Costs are deducted once when accepted. `TeamEconomy` marks age-up queued immediately, so a second Age 2 command rejects before the first finishes.

Module tests construct real World/Building/TeamEconomy state and assert stockpile changes; do not use undefined `game.enqueue()` helpers.

- [ ] **Step 3: Implement derived population and deterministic ready completion**

`population_used()` counts live team Units. `population_cap()` sums completed Town Center/House capacity and clamps to 100.

Ready producers are sorted ascending `BuildingId`. After each successful spawn, recompute population before processing the next producer.

Regression:

```rust
#[test]
fn only_one_of_two_ready_jobs_consumes_the_last_slot() {
    let (mut world, mut map, first, second) = setup_two_ready_barracks_at_pop_9_of_10();
    step_production(&mut world, &mut map, SIM_STEP_SECONDS);
    assert_eq!(population_used(&mut world, TeamId(1)), 10);
    assert_eq!(ready_blocked_jobs(&world, [first, second]), 1);
}
```

`setup_two_ready_barracks_at_pop_9_of_10` and `ready_blocked_jobs` are private helpers defined in the same test module.

- [ ] **Step 4: Implement immediate spawn clearance and rally**

Choose an unblocked/unoccupied immediate perimeter slot around the producer. If none exists, leave the job at 100% with `NoSpawnSpace`. Allocate runtime UnitId only on actual spawn.

`SetRally` stores a target. After spawning, assign a normal Move; if rally routing fails, the unit remains spawned and idle.

- [ ] **Step 5: Implement Age 2 completion and gather-rate improvement**

Age job costs 300 Food + 200 Gold, takes 45 s, shares Town Center FIFO with Villager, and completes once. Completion sets Age 2. `gather_rate_for_age()` returns 2.2/s beginning on the next economy step.

- [ ] **Step 6: Lock canonical order in a Rust integration test**

Add a focused integration test that calls one logical tick in this order:

```rust
apply queued commands;
step_movement(...);
step_economy(...);
step_construction(...);
step_production(...);
```

Assert a movement arrival is consumable by economy/construction in that tick and Age 2 completion does not retroactively alter the already-run economy step.

The Godot runtime still uses the old HPA-470 movement-only fixture until Task 6.

- [ ] **Step 7: Verify and commit**

Run the Task 1 gate.

Commit:

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
- Produces `attach_missing_gameplay_views`, initially exercised by Unit entities on the existing 200-unit Startup fixture.
- Preserves HPA-470 unit presentation contracts exactly.
- Does **not** switch normal startup/reset yet.

- [ ] **Step 1: Move Unit GodotScene attachment out of fixture seeding**

Add an integration-only marker such as `GameplayViewRequested`. In `Update`, query `Unit` entities without the marker and insert:

```rust
(
    Transform::from_xyz(position.current.x, 0.0, position.current.y),
    TransformSyncMetadata::default(),
    Node3DMarker,
    GodotScene::from_path("res://scenes/unit_view.tscn"),
    GameplayViewRequested,
)
```

Remove direct `GodotScene` insertion from the 200-unit fixture seed. Metadata initialization still waits for `GodotNodeHandle`.

- [ ] **Step 2: Pin the unit-view compatibility contract**

Do not rename/remove:

```text
unit_views group
SelectionRing child
set_selected(bool)
unit_id meta
team_id meta
```

Add `unit_kind` metadata without changing old names.

- [ ] **Step 3: Run every HPA-470 integration gate before continuing**

```bash
cargo build -p grus-godot
mkdir -p godot/bin
cp target/debug/libgrus_godot.so godot/bin/libgrus_godot.so
godot --headless --path godot --editor --quit-after 120
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
```

CI must also keep the existing benchmark green. If the 200-unit smoke breaks here, fix the presentation seam before any economy cutover.

- [ ] **Step 4: Commit the isolated presentation seam**

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
- Normal Startup/reset now seeds the complete pure skirmish state.
- Benchmark reset seeds only `units_200()` on a fresh map.
- Full fixed chain is wired once.
- Runtime views cover Unit/Building/standalone ResourceSource; Farm gets one Building view.
- Bridge exposes final command write methods, snapshots, reject codes, and `set_sim_speed`.

- [ ] **Step 1: Wire normal and benchmark seed modes through one clear/reset path**

Clear unique gameplay entities with a `HashSet<Entity>` over `Unit`, `Building`, or `ResourceSource`, then reinitialize indexes/economy/allocator/pending feedback and replace GridMap with `MapFixture::battlefield().map`.

Expose:

```text
reset_fixture() -> normal economy skirmish
reset_benchmark_fixture() -> only 200 speed-12 units
```

Normal Startup calls the skirmish seed.

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

All systems operate on the same authoritative World/GridMap; no second update path.

- [ ] **Step 3: Extend runtime view attachment to Buildings and resources**

Attachment precedence:

```text
Unit                            -> unit_view.tscn
Building                        -> building_view.tscn
ResourceSource without Building -> resource_view.tscn
```

Metadata:

```text
unit: unit_id unit_kind team_id
building: building_id building_kind team_id
standalone resource: resource_id resource_kind
Farm building: building_* + team_id + resource_id + resource_kind
```

Add primitive kind/team/progress styling only. Remove static battlefield base markers because authoritative Town Center views replace them.

- [ ] **Step 4: Expose bridge commands and ECS-derived snapshots**

Write methods queue final `PlayerCommand` variants for Gather/Place/Resume/Enqueue Unit/Enqueue Age/Set Rally.

Read snapshots:

```text
economy: food wood gold age population_used population_cap idle_workers idle_worker_ids last_reject_code
building: id kind complete construction_progress queue_label queue_progress blocked_reason rally_x rally_y
placement: valid anchor_x anchor_y width height reject_code
```

Human-readable feedback remains available, but automated assertions use codes/state.

- [ ] **Step 5: Add Bevy virtual-time test hook without overriding max_delta**

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

Do not call `set_max_delta`; Bevy 0.18.1 clamps raw real delta before applying relative speed.

- [ ] **Step 6: Adapt all existing Godot gates in this same cutover commit**

`smoke_test.gd`:

- wait for 8 normal Unit views, not 200;
- use Team 1 UnitIds 1..4 and Team 2 starting at 5;
- preserve click/shift/box/control-group/move/stop/HUD/camera/unreachable coverage;
- update speed-6 one-second distance band to `4.0..8.0`;
- require max interpolated visual step `< 0.25`.

`reset_test.gd`:

- call normal reset;
- assert 8 unique Unit views, 2 Town Center Building views, 18 standalone authored Resource views, deterministic IDs, starting `200/300/100`, population `4/10` for Team 1.

`benchmark_200.gd`:

- call `reset_benchmark_fixture()` before waiting for 200 units;
- assert no Building/Resource views remain;
- continue commanding IDs 1..200 and preserve the existing active-movement benchmark contract.

- [ ] **Step 7: Run the complete pre-existing integration gate**

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

CI must run the benchmark on this exact cutover. Do not start Task 7 with any of these gates red.

- [ ] **Step 8: Commit the atomic cutover**

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
- Existing controller remains interaction owner; no UI framework.
- All costs/unlocks/progress/reject state comes from bridge snapshots.
- Idle navigation is minimal click-to-cycle over stable IDs.

- [ ] **Step 1: Expand HUD with required HPA-471 state**

Add labels/actions for:

```text
Food Wood Gold
population used/cap
Age
idle-worker count/navigation
selected entity
construction/queue progress
command feedback
House Storehouse Farm Barracks Archery Range Stable
Villager Spearman Archer Cavalry Advance Age
```

Keep controls under the existing HUD input shield.

- [ ] **Step 2: Extend selection while preserving unit box selection**

Keep `selected_ids` for units and add one `selected_building_id`. Box selection stays units-only. Friendly building click clears unit selection; unit click clears building selection.

- [ ] **Step 3: Resolve contextual right-click in one order**

```text
selected villager(s) + standalone resource -> Gather(resource_id)
selected villager(s) + completed Farm building -> Gather(resource_id)
selected villager + incomplete owned building -> ResumeConstruction
selected production building + ground -> SetRally
selected units + ground -> Move
```

Military units never gather/build.

- [ ] **Step 4: Implement one placement-preview mode**

Build button stores only `BuildingKind` code. Mouse motion queries authoritative placement preview; one translucent box mirrors returned footprint/validity. Valid left click sends Place using the lowest selected Villager ID. Escape/right-click exits without command/cost.

- [ ] **Step 5: Wire production, Age 2, rally display, and idle navigation**

Production buttons enqueue against selected building. Town Center shows Advance Age with cost/lock state. Building snapshot displays queue progress/blocked reason/rally target.

Idle-worker control fetches `idle_worker_ids`, sorts by stable ID in Rust, and cycles selection one ID per click. Godot stores only the current cycle cursor, not the authoritative list.

- [ ] **Step 6: Keep HPA-470 smoke/reset green and manually verify economy UI**

Run smoke/reset plus `godot --path godot`. Verify one Town Center/four Villagers per team, resource HUD `200/300/100`, population `4/10`, building selection/actions, and no UI click leaking into world movement.

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
- Smoke uses actual selection/contextual/HUD paths and `set_sim_speed(20.0)` only.
- No debug grants/state mutation.
- CI starts conservative and is tightened from CI evidence, not local workstation timing.

- [ ] **Step 1: Build a solvent UI-driven economy smoke**

At startup:

```gdscript
if not GrusBridge.set_sim_speed(20.0):
    _fail("failed to accelerate Bevy virtual time")
    return
```

Drive this sequence through the same controller/UI paths as a player:

1. assign villagers to berries, trees, and gold; prove stockpiles change only on deposit;
2. gather enough total Wood for House + Storehouse + Farm + Barracks + Archery Range + Archer Wood + Stable (615 Wood; starting 300 is insufficient);
3. gather enough Food/Gold for Spearman + Archer + Age 2 + Cavalry (480 Food, 260 Gold; starts are insufficient);
4. build House and assert cap `10 → 20`;
5. build Storehouse and verify a later worker deposit uses a reachable Storehouse slot;
6. build Farm, assign one worker, and assert a second worker returns `RejectReason::FarmOccupied` via `last_reject_code`;
7. build Barracks, train Spearman, assert new runtime Unit view;
8. build Archery Range, train Archer, assert view;
9. enqueue Age 2, assert Age becomes 2 and gathering-rate snapshot/catalogue shows 2.2/s;
10. verify Stable unlocks, build it, train Cavalry, assert view;
11. set a rally point on one producer and assert the next trained unit receives movement toward it;
12. click idle-worker navigation and assert it selects a bridge-reported idle Villager;
13. verify construction/queue progress and blocked/reject codes through snapshots.

Restore speed before success:

```gdscript
if not GrusBridge.set_sim_speed(1.0):
    _fail("failed to restore Bevy virtual time")
    return
```

- [ ] **Step 2: Add CI with an intentionally conservative first timeout**

Add after reset smoke and before export/benchmark:

```yaml
- name: Run HPA-471 economy integration smoke
  run: timeout 300s godot --headless --path godot res://scenes/economy_smoke_test.tscn
```

Make the smoke print wall-clock elapsed milliseconds in its success marker.

- [ ] **Step 3: Use the first successful CI run to tighten timeout in this same PR**

Read the CI log success marker. Choose a timeout with comfortable CI-only margin (at least 2× observed CI elapsed + 10 seconds, rounded up to a 15-second bucket). Update `.github/workflows/ci.yml` in a follow-up commit on this same PR.

Do not derive this from a local Mac/workstation measurement.

- [ ] **Step 4: Update README**

Document current controls, gather/build/train/advance loop, rally and idle navigation, Bevy/Godot ownership, reset modes, build/import sequence, benchmark-only 200-unit fixture, and historical HPA-470 performance baseline.

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

Expected: all commands exit 0. CI additionally exports the desktop build and runs the retained 200-unit benchmark after explicit benchmark reset.

- [ ] **Step 6: Commit final verification/docs**

```bash
git add .github README.md godot/scenes/economy_smoke_test.tscn godot/scripts/economy_smoke_test.gd
git commit -m "test: verify complete HPA-471 economy loop"
```

---

## Plan self-review

- Every HPA-471 acceptance item remains in scope, including rally points, Age 2 gather improvement, and idle-worker navigation.
- Buildings precede gathering, so Town Centers and Storehouses own one real `Footprint`; `Dropoff` is a marker rather than copied geometry.
- `RejectReason` / `CommandResult` are defined before feature-specific commands and bridge smokes assert codes/state.
- Worker approach tasks carry explicit adjacent slots; missing `MoveOrder` is never treated as proof of arrival.
- `Carry` cannot represent `None + positive amount`, and retasking never destroys carried resources.
- Most tests live in module-local `#[cfg(test)]` blocks with explicit private helpers. Only cross-module flow/system-order uses integration tests.
- Old `starting_resources` / `expansion_resources` are removed when typed resource descriptors land.
- The runtime stays on the known-green 200-unit fixture through Tasks 1–5. Task 5 validates the new attachment seam there; Task 6 performs the normal-start/reset/benchmark cutover atomically.
- Bevy `Time<Virtual>::max_delta` is not overridden; Bevy 0.18.1 clamps raw real delta before relative-speed multiplication.
- New economy smoke starts with a conservative CI timeout and tightens from CI evidence.
- No image generation or custom asset pipeline is required.
- Combat, fog, AI, destruction, persistence, generic frameworks, and final balance remain outside this PR.