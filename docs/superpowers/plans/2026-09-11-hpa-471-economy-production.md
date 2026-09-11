# HPA-471 Worker Economy, Construction, and Army Production Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the HPA-470 battlefield into the complete HPA-471 gather/build/train/advance loop without creating a second gameplay state model.

**Architecture:** Bevy ECS remains authoritative. Four focused simulation modules (`catalog`, `economy`, `buildings`, `production`) extend the existing movement/command core; Godot stays a thin input/HUD/presentation layer that reads bridge snapshots and sends commands. The normal scene uses a small economic skirmish start while the existing 200-unit fixture becomes benchmark-only.

**Tech Stack:** Rust 1.89, Bevy 0.18 ECS, godot-bevy 0.11.x, godot-rust 0.4, Godot 4.6.2, GDScript, existing GitHub Actions CI.

**Spec:** `docs/superpowers/specs/2026-09-11-hpa-471-economy-production-design.md`

## Global Constraints

- One Linear ticket and one GitHub PR for all HPA-471 work; this planning draft becomes the implementation PR.
- Bevy owns every mutable gameplay rule and state; Godot may cache only presentation/selection state.
- Keep the current 20 Hz fixed simulation and interpolated Bevy → Godot transforms.
- All walkability mutations continue through `GridMap::set_blocked` / `set_blocked_rect`.
- Fixed content ceiling: Food/Wood/Gold, four unit kinds, seven building kinds, two ages, 100 population.
- No combat, fog, AI, destruction/refunds, save/load, content editor, hot reload, generic research tree, generic service/repository layer, or new rendering framework.
- Normal start: 4 villagers/team, 200 Food, 300 Wood, 100 Gold, one Town Center/team.
- Carry 10; gather 2.0/s in Age 1 and 2.2/s in Age 2.
- The 200-unit speed-12 fixture remains available only for benchmark/reset coverage.

---

### Task 1: Add typed content, stable IDs, and the economic starting fixture

**Files:**
- Create: `crates/grus-sim/src/catalog.rs`
- Modify: `crates/grus-sim/src/ids.rs`
- Modify: `crates/grus-sim/src/movement.rs`
- Modify: `crates/grus-sim/src/fixture.rs`
- Modify: `crates/grus-sim/src/lib.rs`

**Interfaces:**
- Produces `ResourceKind`, `UnitKind`, `BuildingKind`, `Age`, `Cost`, `UnitSpec`, `BuildingSpec`, `unit_spec()`, `building_spec()`.
- Produces `BuildingId`, `ResourceId`, and a monotonic `IdAllocator` resource.
- Extends `Unit` with `kind: UnitKind`.
- Produces deterministic skirmish starting descriptors while retaining `MapFixture::units_200()`.

- [ ] **Step 1: Add failing catalogue tests**

Add unit tests in `catalog.rs` that pin the MVP ceiling and initial balance:

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

Run:

```bash
cargo test -p grus-sim catalog
```

Expected: compile/test failure because catalogue types do not exist.

- [ ] **Step 2: Implement the static catalogue**

Define exact enums and structs:

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

Use `match`-based `unit_spec()` and `building_spec()` with the exact values from the spec. Add `AGE_TWO_COST`, `AGE_TWO_SECONDS`, `CARRY_LIMIT`, `BASE_GATHER_RATE`, `AGE_TWO_GATHER_RATE`, and `MAX_POPULATION` constants.

- [ ] **Step 3: Extend IDs and units**

Add:

```rust
pub struct BuildingId(pub u32);
pub struct ResourceId(pub u32);

#[derive(Debug, Resource)]
pub struct IdAllocator {
    pub next_unit: u32,
    pub next_building: u32,
}
```

Add allocation methods that increment monotonically and never emit zero. Extend `Unit` with `kind: UnitKind`, then update every existing `spawn_unit` caller so benchmark units use `UnitKind::Spearman` and preserve speed 12.0.

Run:

```bash
cargo test -p grus-sim
```

Expected: existing movement/command tests pass after caller updates.

- [ ] **Step 4: Replace marker-only fixture data with authored start descriptors**

In `fixture.rs`, introduce small descriptor structs such as:

```rust
pub struct ResourceSpawn {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub position: Vec2,
    pub amount: u32,
}

pub struct TeamStart {
    pub team: TeamId,
    pub town_center: Vec2,
    pub villagers: [Vec2; 4],
}
```

Keep the current map geometry and `units_200()`. Use mirrored safe resources around the current base areas and preserve the exposed expansion anchors. Add fixture tests proving both teams have identical counts of each starting resource kind and four villagers.

- [ ] **Step 5: Verify and commit**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p grus-sim
```

Commit on the existing HPA-471 branch:

```bash
git add crates/grus-sim
git commit -m "feat: define HPA-471 economy catalogue and start state"
```

---

### Task 2: Implement team stockpiles and the gather/carry/deposit loop

**Files:**
- Create: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/movement.rs`
- Modify: `crates/grus-sim/src/lib.rs`
- Create: `crates/grus-sim/tests/economy_flow.rs`

**Interfaces:**
- Produces `TeamEconomy`, `ResourceStockpile`, `ResourceSource`, `Carry`, `WorkerTask`, `ResourceIndex`.
- Produces `assign_gather_order()` and `step_economy()`.
- Produces one reusable movement helper that routes an entity to a reachable walkable slot adjacent to a blocked target cell.

- [ ] **Step 1: Write failing accounting/depletion tests**

Add integration tests covering no income before deposit and final delivery after depletion:

```rust
#[test]
fn gathered_resources_are_not_income_until_deposit() {
    let mut game = test_economy_world();
    assign_gather_order(&mut game.world, &game.map, TeamId(1), vec![UnitId(1)], ResourceId(1)).unwrap();

    advance_until_worker_is_carrying(&mut game, UnitId(1));
    assert_eq!(game.stockpile(TeamId(1)).food, 0);

    advance_until_worker_deposits(&mut game, UnitId(1));
    assert!(game.stockpile(TeamId(1)).food > 0);
}

#[test]
fn depleted_source_delivers_final_load_then_worker_becomes_idle() {
    let mut game = test_economy_world_with_source_amount(5);
    run_until_settled(&mut game);
    assert_eq!(game.stockpile(TeamId(1)).food, 5);
    assert!(game.worker_is_idle(UnitId(1)));
}
```

Run:

```bash
cargo test -p grus-sim --test economy_flow
```

Expected: failure because economy types/functions do not exist.

- [ ] **Step 2: Extract near-target routing without changing move semantics**

Keep user Move commands requiring a walkable target. Add a helper in `movement.rs` that accepts a blocked goal cell and returns the first reachable perimeter/ring slot, using the current A* map and current unit position. Reuse it only for resource/building approach behavior.

Add a regression test proving a worker can route beside a blocked source while a normal Move command to that blocked source remains `Unreachable`.

- [ ] **Step 3: Add team/resource/worker state**

Implement:

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResourceStockpile { pub food: u32, pub wood: u32, pub gold: u32 }

#[derive(Debug, Resource)]
pub struct TeamEconomy(pub HashMap<TeamId, TeamState>);

#[derive(Component, Debug)]
pub struct ResourceSource {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub remaining: Option<u32>,
    pub cell: GridPos,
}

#[derive(Component, Debug, Default)]
pub struct Carry { pub kind: Option<ResourceKind>, pub amount: u32 }

#[derive(Component, Debug)]
pub enum WorkerTask {
    Idle,
    ToSource(ResourceId),
    Gathering(ResourceId),
    ToDropoff { source: ResourceId, building: BuildingId },
    ToConstruction(BuildingId),
    Constructing(BuildingId),
}
```

`ResourceStockpile` gets `can_afford`, `deduct`, and `deposit` methods with tests.

- [ ] **Step 4: Implement fixed-step worker transitions**

`assign_gather_order()` validates ownership/villager/source and routes to a nearby source slot. `step_economy()` performs gather accumulation, carry cap handling, nearest reachable Town Center/Storehouse lookup, deposit, repeat, final deposit after source depletion, and idle settlement on unreachable routes.

When a finite source hits zero, remove it from `ResourceIndex`, unblock its source cell using `GridMap::set_blocked(cell, false)`, and mark the ECS entity for despawn/presentation removal.

- [ ] **Step 5: Add Farm single-worker semantics in the same module**

Represent a completed Farm as a renewable Food source linked to its `BuildingId`. Track at most one assigned `UnitId`; a second gather request returns `FarmOccupied`. Reassignment/Stop releases the farm slot.

Add tests for one-worker acceptance, second-worker rejection, and release after Stop.

- [ ] **Step 6: Verify and commit**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p grus-sim
```

Commit:

```bash
git add crates/grus-sim
git commit -m "feat: add worker gathering and deposit loop"
```

---

### Task 3: Implement authoritative placement and one-builder construction

**Files:**
- Create: `crates/grus-sim/src/buildings.rs`
- Modify: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/lib.rs`
- Create: `crates/grus-sim/tests/construction.rs`

**Interfaces:**
- Produces `Building`, `ConstructionState`, `BuildingIndex`.
- Produces `validate_placement()`, `place_building()`, `assign_builder()`, `step_construction()`.
- Completed Town Center/Storehouse entities are discoverable as drop-offs by `economy.rs`.

- [ ] **Step 1: Write failing placement/cost tests**

Cover every authoritative rejection before implementation:

```rust
#[test]
fn invalid_placement_never_charges_resources() {
    let mut game = construction_world();
    let before = game.stockpile(TeamId(1));
    let result = place_building(
        &mut game.world,
        &mut game.map,
        TeamId(1), UnitId(1), BuildingKind::House, GridPos::new(-1, 4),
    );
    assert_eq!(result, Err(BuildRejectReason::OutOfBounds));
    assert_eq!(game.stockpile(TeamId(1)), before);
}

#[test]
fn accepted_placement_charges_once_and_blocks_full_footprint() {
    // place House at a clear 2x2 anchor, assert -50 Wood exactly once and all four cells blocked
}
```

Also cover occupied footprint, insufficient resources, locked Stable, non-villager builder, and unreachable perimeter.

- [ ] **Step 2: Implement building identity and placement validation**

`Building` contains `id`, `team`, `kind`, `anchor`, and `ConstructionState`. Compute footprint cells from the catalogue only. Validation order must match the spec so failed actions have deterministic feedback.

On acceptance:

1. deduct the catalogue cost once;
2. allocate `BuildingId`;
3. immediately call `GridMap::set_blocked` for every footprint cell;
4. spawn the under-construction building;
5. assign the chosen villager to a reachable perimeter slot.

- [ ] **Step 3: Implement one active builder and resumable construction**

`step_construction()` advances only when the assigned villager is adjacent and still in `WorkerTask::Constructing(building_id)`. Reissuing Move/Gather/Stop changes the worker task and pauses progress. `assign_builder()` allows a villager to right-click an incomplete owned building and resume it; it replaces the previous builder rather than stacking rates.

Add tests that two builders do not double speed and that reassignment resumes the same remaining build time.

- [ ] **Step 4: Initialize starting Town Centers through the same building path**

Add a fixture bootstrap helper that creates complete starting Town Centers without charging cost but still uses the same `Building` component/index and footprint blocking. This is the only special case; later teams use identical economy/building rules.

- [ ] **Step 5: Verify and commit**

Run the full Rust gate and commit:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p grus-sim
git add crates/grus-sim
git commit -m "feat: add building placement and construction"
```

---

### Task 4: Add production queues, population, rally points, and Age 2

**Files:**
- Create: `crates/grus-sim/src/production.rs`
- Modify: `crates/grus-sim/src/buildings.rs`
- Modify: `crates/grus-sim/src/economy.rs`
- Modify: `crates/grus-sim/src/commands.rs`
- Modify: `crates/grus-sim/src/lib.rs`
- Create: `crates/grus-sim/tests/production_queue.rs`

**Interfaces:**
- Produces `ProductionQueue`, `ProductionJob`, `RallyPoint`.
- Produces `population_used()`, `population_cap()`, `enqueue_unit()`, `enqueue_age_up()`, `set_rally_point()`, `step_production()`.
- New units use the existing movement/presentation components and catalogue speed.

- [ ] **Step 1: Write queue charging/lock tests**

Add tests for:

```rust
#[test]
fn enqueue_charges_exactly_once_even_while_job_waits_to_spawn() { /* ... */ }

#[test]
fn stable_and_cavalry_are_rejected_before_age_two() { /* ... */ }

#[test]
fn age_up_can_only_be_queued_or_completed_once() { /* ... */ }
```

Include insufficient-resource rejection and producer/unit compatibility.

- [ ] **Step 2: Implement derived population**

`population_used()` counts live units for the team. `population_cap()` starts at 10 from the completed starting Town Center, adds 10 per completed House, and clamps to 100. Do not store a mutable duplicate counter.

- [ ] **Step 3: Implement FIFO production and deterministic same-tick completion**

Store a queue on completed producer buildings. Front-job progress advances each fixed step. Charge once at enqueue. When a unit job reaches 100%, choose a clear walkable perimeter cell; if cap/spawn is blocked, keep the job at 100% with a blocked reason.

Process ready producers sorted by `BuildingId`. After every successful spawn, the next building sees the updated live-unit count. Add the exact regression:

```rust
#[test]
fn two_buildings_finishing_on_the_last_population_slot_spawn_only_one_unit() {
    // cap=10, used=9, two 100%-complete queues; assert used==10 and one queue remains blocked
}
```

- [ ] **Step 4: Add rally behavior**

`SetRally` stores a map target on the producer. A spawned unit receives a normal move order after creation. If routing fails, the unit remains spawned beside the building and queue completion still succeeds.

- [ ] **Step 5: Add Age 2 as a Town Center queue job**

Age-up uses the same FIFO queue, costs 300 Food + 200 Gold, takes 45 seconds, and blocks a second age-up enqueue as soon as the first is accepted. Completion changes the team's age. `economy.rs` reads age when selecting 2.0 vs 2.2 gather rate. Stable placement and cavalry enqueue then become valid.

- [ ] **Step 6: Verify and commit**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p grus-sim
git add crates/grus-sim
git commit -m "feat: add production population and age advancement"
```

---

### Task 5: Expose HPA-471 commands/snapshots and render resources/buildings/unit kinds

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
- `PendingCommands` becomes `Vec<PlayerCommand>`.
- Bridge write methods: `gather`, `place_building`, `resume_construction`, `enqueue_unit`, `enqueue_age_up`, `set_rally_point`.
- Bridge read methods: economy snapshot, building snapshot, catalogue action info, placement preview, idle-worker IDs.

- [ ] **Step 1: Convert bridge command queue without regressing move/stop**

Wrap current `UnitCommand` inside `PlayerCommand::Units`. Keep `move_units()` and `stop_units()` public signatures unchanged so HPA-470 Godot controls do not need a parallel command path.

Add new bridge methods using integer enum codes mapped in one Rust function; reject unknown codes instead of letting GDScript construct gameplay enums.

- [ ] **Step 2: Add read-only Dictionary snapshots**

Return Godot `Dictionary` values with stable keys. The economy snapshot must contain:

```text
food, wood, gold, age, population_used, population_cap, idle_workers
```

The building snapshot must contain:

```text
id, kind, complete, construction_progress, queue_label, queue_progress, blocked_reason, rally_x, rally_y
```

Placement preview returns:

```text
valid, anchor_x, anchor_y, width, height, reason
```

These methods compute from ECS state each call; they do not create mutable mirror state.

- [ ] **Step 3: Spawn ECS-backed resource/building views**

Attach `GodotScene::from_path()` for resource/building entities and initialize metadata exactly once:

```text
resource_id, resource_kind
building_id, building_kind, team_id
unit_id, unit_kind, team_id
```

When a resource despawns on depletion, godot-bevy removes its node with the ECS entity.

- [ ] **Step 4: Add primitive presentation**

`resource_view.gd` adds nodes to `resource_views` and styles berries/tree/gold with clearly different primitive shape/scale/material.

`building_view.gd` adds nodes to `building_views`, applies team tint + per-kind scale/shape, exposes `set_selected(bool)`, and renders under-construction state by scaling height/progress rather than creating an animation framework.

`unit_view.gd` keeps team coloring and uses simple size/shape cues for Villager/Spearman/Archer/Cavalry.

Remove the old static base markers from `battlefield.tscn`; authoritative Town Center views replace them.

- [ ] **Step 5: Build/import smoke**

```bash
cargo build -p grus-godot
mkdir -p godot/bin
cp target/debug/libgrus_godot.so godot/bin/libgrus_godot.so
godot --headless --path godot --editor --quit-after 120
```

Expected: project imports and the GDExtension loads without placeholder classes.

- [ ] **Step 6: Commit**

```bash
git add crates/grus-godot godot
git commit -m "feat: bridge economy state into Godot presentation"
```

---

### Task 6: Add contextual gather/build/train UI and authoritative placement preview

**Files:**
- Modify: `godot/scenes/main.tscn`
- Modify: `godot/scripts/battlefield_controller.gd`

**Interfaces:**
- Keeps world interaction in the existing controller; no new UI framework.
- Uses bridge snapshots for every cost/unlock/progress/blocked display.

- [ ] **Step 1: Expand the HUD scene**

Add labels for Food/Wood/Gold, population, age, idle workers, selected entity, queue/progress, and command feedback. Add explicit buttons for:

```text
House, Storehouse, Farm, Barracks, Archery Range, Stable,
Villager, Spearman, Archer, Cavalry, Advance Age
```

Buttons default hidden/disabled until the current selection makes them relevant. Keep all HUD controls under the existing input-shielded `CanvasLayer`.

- [ ] **Step 2: Extend selection without breaking unit box selection**

Track `selected_ids` for units and one `selected_building_id`. Left click chooses the nearest friendly unit/building projected under the cursor; box selection still selects only friendly units. Selecting a building clears unit selection and vice versa.

- [ ] **Step 3: Implement contextual right-click**

Resolve screen-space resource/building hits before ground movement:

1. selected villagers + resource → `GrusBridge.gather(...)`;
2. selected villager + incomplete owned building → `resume_construction(...)`;
3. otherwise selected units + ground → existing Move command.

Military units never receive gather/build actions.

- [ ] **Step 4: Implement one placement mode**

Pressing a build button stores only the requested `BuildingKind` code. Mouse motion calls the bridge preview and updates one translucent `MeshInstance3D` box at the returned snapped anchor/footprint. Green/red visual state mirrors `valid`; the HUD shows `reason`.

Left click on a valid preview calls `place_building()` using the lowest selected villager ID as the single initial builder, then exits placement mode. Right click/Escape exits without sending a command or deducting cost.

- [ ] **Step 5: Wire production/age/rally and idle-worker navigation**

Production buttons send enqueue commands for the selected building. Advance Age appears only on the Town Center and displays lock/cost feedback from Rust. Right click ground while a production building is selected sets its rally point instead of issuing unit movement.

The idle-worker label/button retrieves stable IDs and cycles selection through them; no Godot-maintained idle list is authoritative.

- [ ] **Step 6: Manually verify the normal start**

Run:

```bash
godot --path godot
```

Verify Team 1 starts with one Town Center, four villagers, visible Food/Wood/Gold resources, `200/300/100` stockpile, population `4/10`, and no 200-unit fixture.

- [ ] **Step 7: Commit**

```bash
git add godot/scenes/main.tscn godot/scripts/battlefield_controller.gd
git commit -m "feat: add economy construction and production controls"
```

---

### Task 7: Preserve HPA-470 regression coverage and add end-to-end HPA-471 verification

**Files:**
- Modify: `godot/scripts/smoke_test.gd`
- Modify: `godot/scripts/reset_test.gd`
- Modify: `godot/scripts/benchmark_200.gd`
- Create: `godot/scripts/economy_smoke_test.gd`
- Create: `godot/scenes/economy_smoke_test.tscn`
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`

**Interfaces:**
- Normal reset restores the economic starting state.
- Benchmark reset explicitly restores the 200-unit fixture before timing.
- Economy smoke exercises actual UI/contextual command paths, not direct state mutation.

- [ ] **Step 1: Adapt the old smoke to the smaller normal start**

Keep HPA-470 coverage for click/additive/box selection, control groups, move, stop, HUD shielding, unreachable feedback, camera pan/zoom, 20 Hz cadence, and interpolated presentation. Update expected IDs/counts to the four Team 1 villagers and four mirrored Team 2 villagers.

- [ ] **Step 2: Split normal reset from benchmark reset**

`GrusBridge.reset_fixture()` now restores the normal skirmish state and `reset_test.gd` asserts deterministic starting unit/building/source IDs with no duplicates.

Add `GrusBridge.reset_benchmark_fixture()` and make `benchmark_200.gd` call it before waiting for 200 unit views. `benchmark_move_all()` remains benchmark-only and continues commanding IDs 1..200.

- [ ] **Step 3: Add the HPA-471 economy smoke**

Create a scene that instances `main.tscn`, sets `Engine.time_scale = 20.0`, and drives the same controller/UI paths used by a player:

1. select a villager;
2. right-click berries and wait for a deposit; assert Food HUD increases only after return;
3. press House build action, place it on a valid preview, and wait for completion; assert population cap becomes 20;
4. build Barracks, select it, enqueue Spearman, and assert a new spearman view appears;
5. build Archery Range, enqueue Archer, and assert it appears;
6. gather enough Food/Gold, select Town Center, enqueue Age 2, and assert Age HUD changes to 2;
7. verify Stable button unlocks, place/complete Stable, enqueue Cavalry, and assert it appears;
8. verify queue/progress/command feedback changed during the sequence.

The test may accelerate time only. It must not modify stockpiles, completion progress, age, or entity state directly.

- [ ] **Step 4: Add CI step**

After the existing bridge/reset smoke steps, add:

```yaml
- name: Run HPA-471 economy integration smoke
  run: timeout 60s godot --headless --path godot res://scenes/economy_smoke_test.tscn
```

Keep export and 200-unit benchmark steps unchanged except for the benchmark fixture reset performed inside the scene script.

- [ ] **Step 5: Update README**

Replace the HPA-470-only usage section with current controls and HPA-471 architecture notes while preserving the pinned toolchain, build/import sequence, export instructions, and recorded HPA-470 performance baseline. Document that the baseline benchmark uses a benchmark-only 200-unit fixture.

- [ ] **Step 6: Run the final local gate**

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

Expected: every command exits 0 and the two gameplay smokes print their success markers.

- [ ] **Step 7: Commit final verification changes**

```bash
git add .github README.md godot
git commit -m "test: verify complete HPA-471 economy loop"
```

---

## Plan self-review

- Every HPA-471 acceptance item maps to a task above.
- The design preserves one PR and does not create separate catalogue/UI/test tickets.
- Costs, durations, footprints, age locks, population, and gather rates have one Rust source of truth.
- Construction and resource depletion reuse the existing map occupancy revision seam.
- Normal gameplay no longer boots the 200-unit benchmark fixture; benchmark coverage remains explicit.
- Combat, fog, AI, destruction, persistence, generic frameworks, and final balance remain outside this PR.
