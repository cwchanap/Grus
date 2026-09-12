# HPA-471 Worker Economy, Construction, and Army Production Design

## Status

Planning baseline for HPA-471. This document and its implementation plan live on the same branch and draft PR that will carry the implementation; there is no follow-up implementation PR for this ticket.

This revision incorporates the pre-implementation review of the merged HPA-470 seams. In particular it locks worker command cancellation, fractional gather accounting, runtime presentation attachment, simulation-time acceleration for smoke tests, reset semantics, fixed-tick order, shared near-target slot reservation, and exact skirmish coordinates before implementation starts.

## Outcome

Turn the retained HPA-470 battlefield into a playable grow-and-spend RTS loop. A normal match starts with one Town Center and four villagers per side. The player can gather Food/Wood/Gold, shorten return routes with a Storehouse, construct the six buildable structures, train villagers/spearmen/archers, advance once, then build a Stable and train cavalry.

Combat, fog, AI, destruction, save/load, extra content, and balance polish remain later-ticket work.

## Baseline we keep

HPA-470 already established the seams HPA-471 needs:

- Bevy ECS owns authoritative gameplay state; Godot owns rendering, input, HUD, and audio/visual feedback.
- The simulation runs at 20 Hz and Godot interpolates presentation.
- `GridMap::set_blocked` is the only walkability mutation seam. Its revision drives movement replanning.
- Stable `UnitId` values cross the Godot bridge.
- Selection, move/stop commands, control groups, camera pan/zoom, headless Godot smoke coverage, export smoke, and the 200-unit benchmark already exist.

HPA-471 extends those seams rather than replacing them.

## Design choice

Use small feature modules inside `grus-sim`, a static typed balance catalogue, and a thin Godot bridge. Do not introduce a second mutable `GameState`, a generic service/repository layer, a data-driven content editor, or a generic behavior-tree/production framework.

### Alternatives rejected

1. **One monolithic `GameState` resource outside ECS.** This would duplicate entity truth already represented by Bevy components and make later combat/AI reconcile two state models.
2. **A generic data-driven RTS engine.** Loading unit/building definitions from JSON/RON and making every action generic is unnecessary for a fixed four-unit/seven-building MVP and slows iteration.
3. **Godot-owned economy state.** Mirroring stockpiles, queues, construction, or worker jobs in GDScript would violate the existing one-way presentation contract and make headless Rust tests less useful.
4. **A second navigation path for gathering/building.** HPA-470 already has destination-slot reservation and A*. Gathering and construction reuse that machinery instead of introducing a one-entity approach helper that would allow workers to stack.

## Simulation structure

Add four focused modules to `grus-sim`:

- `catalog.rs`: fixed enums/specs/costs and initial tuning values.
- `economy.rs`: team stockpiles, finite resource sources, generic drop-offs, worker carry/task state, fractional gather accumulation, deposit loop, idle-worker queries.
- `buildings.rs`: building identity/state, placement validation, footprint reservation, construction progress, completed-building effects, Farm creation.
- `production.rs`: production queues, population, rally/spawn clearance, age advancement, unit spawning.

Keep movement/pathfinding in the existing `movement.rs`/`commands.rs`. Generalize the current destination-slot/reservation code just enough for a blocked target center and reuse the same reservation loop for Move, Gather, and construction approach. Do not add a navigation service abstraction.

## Stable identity

Extend `ids.rs` with:

- `BuildingId(u32)`
- `ResourceId(u32)`

Add one monotonic `IdAllocator` resource with `next_unit`, `next_building`, and `next_resource`. Authored starting entities keep deterministic IDs; allocator counters start above the authored maxima so runtime-trained units, placed buildings, and completed Farms cannot collide with fixture IDs.

Do not invent a generic entity registry. `UnitIndex`, `BuildingIndex`, and `ResourceIndex` are small typed ID → `Entity` maps following the existing `UnitIndex` pattern.

## Typed catalogue

The catalogue is compile-time Rust data returned by `match` functions. It is the only source of gameplay costs, times, footprints, unlocks, population values, movement speeds, and gathering constants. Godot reads display values through bridge snapshots instead of duplicating costs in GDScript.

Initial tuning is intentionally simple and is not final balance:

| Item | Cost | Time | Footprint / Pop | Unlock |
| --- | --- | ---: | --- | --- |
| Villager | 50 Food | 15 s | 1 pop | Start |
| Spearman | 60 Food | 20 s | 1 pop | Start |
| Archer | 40 Food + 40 Wood | 25 s | 1 pop | Start |
| Cavalry | 80 Food + 60 Gold | 30 s | 1 pop | Age 2 |
| Town Center | Starting only | — | 4×4, +10 cap | Start |
| House | 50 Wood | 15 s | 2×2, +10 cap | Start |
| Storehouse | 75 Wood | 20 s | 2×2 | Start |
| Farm | 60 Wood | 15 s | 2×2 | Start |
| Barracks | 120 Wood | 30 s | 3×3 | Start |
| Archery Range | 120 Wood | 30 s | 3×3 | Start |
| Stable | 150 Wood | 40 s | 3×3 | Age 2 |
| Age 2 | 300 Food + 200 Gold | 45 s | Town Center queue job | Once |

Global values:

- Starting stockpile per team: 200 Food, 300 Wood, 100 Gold.
- Starting villagers per team: 4.
- Carry limit: 10 whole resources.
- Base gathering rate: 2.0 resources/second.
- Age 2 gathering rate: 2.2 resources/second.
- Population ceiling: 100 per side.
- Base Town Center capacity: 10; each completed House adds 10.
- Unit movement speed: villager/spearman/archer 6.0, cavalry 8.0. The benchmark fixture retains speed 12.0.
- Resource node amounts: berries 600 Food, tree 400 Wood, gold deposit 600 Gold.

The numbers are deliberately easy to tune later in HPA-474 without changing code structure.

## Authored skirmish fixture

`MapFixture::battlefield()` remains the 128×96 authored map geometry. The normal start is explicit rather than derived from the old base markers.

### Town Centers and villagers

Town Center footprint anchors use canonical lower-left `GridPos` values:

- Team 1 Town Center: `(12, 46)`, footprint `x=12..15`, `y=46..49`.
- Team 2 Town Center: `(112, 46)`, footprint `x=112..115`, `y=46..49`.

Villager starting cells are outside those footprints:

- Team 1: `(11,45)`, `(11,50)`, `(16,45)`, `(16,50)`.
- Team 2: `(116,45)`, `(116,50)`, `(111,45)`, `(111,50)`.

After both Town Center footprints are blocked, every villager start cell must remain walkable.

### Starting resources

Use typed `ResourceSpawn` descriptors with deterministic `ResourceId`, `ResourceKind`, `GridPos`, and finite amount.

Team 1 safe resources:

- berries: `(22,42)`, `(22,54)`;
- trees: `(20,45)`, `(20,48)`, `(20,51)`;
- gold: `(25,48)`.

Team 2 mirrors them:

- berries: `(105,42)`, `(105,54)`;
- trees: `(107,45)`, `(107,48)`, `(107,51)`;
- gold: `(102,48)`.

Exposed expansion resources keep the existing anchors:

- southwest cluster: gold `(45,16)`, trees `(43,18)`, `(47,18)`;
- northeast cluster: gold `(82,79)`, trees `(84,77)`, `(80,77)`.

Fixture tests must prove no resource cell overlaps a Town Center footprint and all authored resource/villager cells are in bounds before resource cells themselves are blocked.

Keep `units_200()` as a benchmark-only fixture. The normal main scene boots the economy start. The benchmark scene explicitly requests the 200-unit fixture before measuring.

## Domain model

### Team economy

A `TeamEconomy` resource keyed by `TeamId` owns only team-wide mutable values:

- `ResourceStockpile { food, wood, gold }` using whole `u32` resources;
- current age (`Age1` or `Age2`);
- whether age advancement is already queued/completed.

Population is derived from live units plus completed capacity-granting buildings instead of storing a second mutable count.

### Units and workers

Extend `Unit` with `UnitKind`. Only villagers receive worker components:

- `Carry { kind: Option<ResourceKind>, amount: u32 }`;
- `GatherProgress(f32)`;
- `WorkerTask`.

`WorkerTask` is a small explicit state machine:

- `Idle`;
- `ToSource(ResourceId)`;
- `Gathering(ResourceId)`;
- `ToDropoff { source: ResourceId, dropoff: BuildingId }`;
- `ToConstruction(BuildingId)`;
- `Constructing(BuildingId)`.

Movement remains represented by the existing `MoveOrder`.

Gathering uses fractional progress because a 2.0/s rate at a 0.05 s fixed step is 0.1 resource/tick. Each gathering tick adds `rate * SIM_STEP_SECONDS` to `GatherProgress`; whenever it reaches at least 1.0, transfer whole units into `Carry` up to carry/source limits and subtract the transferred whole amount from progress. Stockpiles, source remaining amounts, and carried amounts remain integers. Canceling/replacing a gather job resets fractional progress so it cannot leak into a different source/job.

### Generic drop-offs

`Dropoff` is an economy component and carries the immutable routing geometry needed by the gather loop:

```rust
pub struct Dropoff {
    pub team: TeamId,
    pub building: BuildingId,
    pub anchor: GridPos,
    pub width: u8,
    pub height: u8,
}
```

Economy routing searches `Dropoff` components directly and generates reachable reserved slots around that footprint; it does not branch on `BuildingKind` names or require a Godot node.

- Starting Town Centers receive `Dropoff` during economy bootstrap using their authored 4×4 anchors.
- When Task 3 adds the full `Building` component, the same starting entities keep their `Dropoff`; they are not replaced by a second Town Center entity.
- Completed Storehouses receive `Dropoff` on construction completion.
- No other building is a drop-off in HPA-471.

This lets finite gathering be implemented/tested before the full construction subsystem without creating a throwaway building model.

### Resource sources

`ResourceSource` stores stable ID, resource kind, remaining amount (`Some(u32)` for finite nodes, `None` for renewable Farm), blocked cell/footprint reference, and optional Farm assignment.

Finite source cells are blocked in `GridMap`, so workers route to nearby walkable slots. When a finite source reaches zero, it is removed from `ResourceIndex`, despawned, and its source cell is unblocked through `GridMap::set_blocked`, incrementing the map revision normally.

A completed Farm remains a building and creates a renewable Food `ResourceSource` using `IdAllocator::next_resource`. Exactly one villager may hold the Farm assignment at a time. A second gather command to the same occupied Farm is rejected with a visible reason.

## Shared approach-slot routing

HPA-470 already has `destination_slots` plus reference-counted current/goal reservations for group Move. Generalize that code rather than adding a separate single-worker route helper.

The shared reservation routine accepts a target cell/footprint and whether the target itself may be blocked:

- normal Move requires the target cell to be walkable;
- Gather/build/drop-off approach may target blocked resource/building footprints and generates walkable perimeter/ring candidates;
- current cells and existing goals remain reference-counted reservations;
- each accepted worker receives a distinct reachable slot;
- rejected workers keep their existing reservation/order.

Required regression: a normal Move onto a blocked source remains `Unreachable`, while a Gather command for multiple villagers targeting that same source assigns distinct reachable perimeter slots.

## Player commands and worker cancellation

The final command surface is one top-level simulation enum:

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

The implementation may grow this enum as each owning task lands (`Units/Gather` first, construction variants next, production variants last) so every intermediate commit remains fully implemented and exhaustive; do not add placeholder command variants.

`apply_player_command()` lives in `grus-sim`; Godot only queues commands and formats returned feedback. Move/Stop remain `UnitCommand` variants inside `PlayerCommand::Units`.

Promote the existing unit ownership lookup to one reusable internal helper rather than duplicating ownership checks across economy/buildings/production.

A single `cancel_worker_activity()` helper owns worker-side cleanup. It starts with worker/movement cleanup in Task 2 and is extended in Task 3, when Farm/construction components exist, to also:

- release an assigned Farm slot if the current task owns one;
- clear the building's active builder if this worker was constructing.

In its final form it sets `WorkerTask::Idle`, resets `GatherProgress`, and removes/replaces the old `MoveOrder` as required.

Command semantics are **validate → cancel old accepted worker job → apply replacement**. A rejected Move/Gather/Place/Resume command does not destroy a valid existing worker job/order. `Stop` is the explicit unconditional cancellation command for owned units.

## Buildings and construction

`Building` stores stable ID, team, kind, footprint anchor, and construction state. A placed building reserves its full footprint immediately. This makes placement authoritative and automatically triggers HPA-470 route replanning through the existing map revision.

Placement validation checks, in order:

1. issuer owns the selected villager;
2. building kind is unlocked/buildable;
3. footprint is fully in bounds;
4. every footprint cell is currently walkable;
5. team can afford the cost;
6. at least one reserved approach slot is reachable by the builder.

Only after all checks pass does the simulation cancel the accepted builder's prior worker job, deduct resources, reserve cells, create the under-construction building, and assign the builder. Preview cancellation therefore costs nothing. There is no demolition/refund path.

One active builder advances a site's catalogue build time. If that villager receives an accepted replacement command or Stop, construction pauses. Right-clicking the incomplete owned building with a villager selected validates, cancels the villager's previous accepted job, assigns it, and resumes construction. No multi-builder speed stacking is implemented.

The two authored Town Center entities created by economy bootstrap receive the full completed `Building` component and `BuildingIndex` entry when the building subsystem lands; they keep the same `BuildingId`, footprint, and `Dropoff` rather than being respawned.

Completed Storehouses receive `Dropoff`. Completed Farms allocate a new `ResourceId` and create their renewable one-worker Food source. This happens only after the building subsystem exists; Farm is not modeled as a pre-building economy special case.

## Gathering loop

A contextual gather command targets a `ResourceId`, not a point.

1. Validate selected units are owned villagers and the source still exists.
2. Reserve distinct reachable approach slots for accepted workers using the shared reservation routine.
3. For Farm, validate/reserve its single worker assignment.
4. For each accepted worker only, cancel the previous worker activity and install the new Gather task/route.
5. At the source, accumulate fractional gather progress and transfer whole units into carry until full or the finite source depletes.
6. Route to the nearest reachable completed `Dropoff` for the same team.
7. Deposit the carried amount atomically into the team's stockpile.
8. If the source still exists, route back and repeat.
9. If the source depleted, deposit any carried amount and settle visibly to `Idle`.
10. If a required route later becomes unavailable, cancel the worker activity, release Farm assignment if applicable, and publish feedback instead of retrying A* every tick.

Income is never credited before deposit.

## Production, rally, population, and age-up

Completed production buildings own a simple FIFO `ProductionQueue`.

- Town Center: Villager and Age 2.
- Barracks: Spearman.
- Archery Range: Archer.
- Stable: Cavalry.

Enqueue validation checks ownership, building completion, producer/unit compatibility, unlock state, and affordability. Cost is deducted once on accepted enqueue. Queue progress advances only for the front job.

A finished unit job waits at 100% when population is full or no perimeter spawn cell is free. It resumes automatically when space exists. To make simultaneous completions deterministic, ready production buildings are processed in ascending `BuildingId`; population is recomputed after each successful spawn, so two same-tick completions cannot both consume the final slot.

Spawn clearance uses the same map occupancy rules and selects an unblocked perimeter cell not occupied/reserved by a live unit. Newly spawned units receive a stable runtime `UnitId`. If a rally point is set, the unit receives a normal move order after spawning; a failed rally route does not cancel the trained unit.

Age advancement is a Town Center queue job and competes with villager production. The team may have at most one age-up job queued or completed. Completion flips the team to Age 2, unlocks Stable/cavalry, and changes the gather rate to 2.2/s starting with the next economy step. No tech tree or additional upgrades are introduced.

## Fixed simulation order

The Godot integration owns one explicit fixed-step order and keeps it stable as systems are added:

1. `apply_pending_commands`;
2. `step_movement`;
3. `step_economy`;
4. `step_construction`;
5. `step_production`.

This order is part of the test contract. Movement resolves before worker/construction work for the tick; economy resolves before construction; production/age completion resolves last, so an Age 2 completion changes gather rate on the next tick rather than retroactively changing the current economy step.

Do not leave `advance_simulation()` as movement-only after adding the new systems.

## Godot bridge and runtime presentation

`PendingCommands` becomes `Vec<PlayerCommand>`. Each fixed tick drains the queue through `apply_player_command()` and updates the existing feedback revision/text resource.

Read-only bridge methods expose snapshots rather than mutable Godot state:

- player stockpile, age, population used/cap, idle-worker IDs/count;
- building state, queue item/progress/blocked reason;
- catalogue cost/footprint/unlock data for HUD labels;
- placement preview result and snapped anchor;
- resource/building stable metadata for contextual targeting.

Integer kind codes at the GDScript boundary are converted in one Rust mapping function. GDScript never owns gameplay enums/costs.

### One runtime view-attachment path

Fixture spawn and runtime spawn must use the same presentation path. Add one Godot-side Bevy system that finds gameplay ECS entities missing a requested view and attaches the appropriate `GodotScene` plus transform-sync metadata:

- `Unit` → `unit_view.tscn`;
- `Building` → `building_view.tscn`;
- standalone `ResourceSource` without `Building` → `resource_view.tscn`.

A completed Farm has both `Building` and `ResourceSource`, so it receives **only** `building_view.tscn`. Its building-view metadata also includes the Farm's `resource_id`, allowing contextual Gather targeting without a second overlapping resource node.

Use a small integration-only marker to prevent duplicate attachment. `grus-sim` never imports Godot types. Metadata initialization runs after `GodotNodeHandle` exists.

This guarantees trained units, newly placed buildings, completed Farms/resources, and authored fixture entities all become visible without special-case scene attachment in fixture seeding. Despawning an ECS resource/building/unit removes its Godot node through the existing godot-bevy scene ownership path.

### Simulation speed for integration smoke

The pinned `godot-bevy 0.11.0` drives Bevy's `app.update()` from Godot `_process` and installs Bevy `TimePlugin`; changing `Engine.time_scale` is therefore not the contract for accelerating Bevy `FixedUpdate`.

Expose a bridge-only test hook:

```rust
#[func]
fn set_sim_speed(&self, relative_speed: f64) -> bool
```

It validates a positive finite value and calls `Time<Virtual>::set_relative_speed(relative_speed as f32)` on the Bevy app. The playable scene never calls it. `economy_smoke_test.gd` sets 20× at startup and restores 1× before exit.

## Reset and seeding

Use one clear-and-seed path for both normal and benchmark resets.

Before reseeding, reset must:

- collect the unique gameplay entities carrying `Unit`, `Building`, or `ResourceSource` and despawn each entity once;
- remove/reinitialize `UnitIndex`, `BuildingIndex`, `ResourceIndex`, `TeamEconomy`, and `IdAllocator`;
- clear pending commands and reset feedback;
- replace the `GridMap` resource with a fresh `MapFixture::battlefield().map`, so previous construction/resource blocking cannot leak across resets.

Then seed exactly one mode:

- `reset_fixture()` → normal two-team economy start;
- `reset_benchmark_fixture()` → only the retained 200-unit speed-12 movement fixture.

The benchmark path does not create Town Centers/resources/economy entities. The normal reset test asserts deterministic unit/building/resource IDs and no duplicates; the benchmark reset asserts exactly 200 unique UnitIds.

## Godot presentation and interaction

Use primitive low-poly scenes; HPA-471 does not require generated art.

Add:

- `resource_view.tscn` + `resource_view.gd`: one simple mesh styled by resource kind and tagged with `resource_id`;
- `building_view.tscn` + `building_view.gd`: one simple mesh scaled/styled by building kind, team color, construction progress, selection state, and Farm `resource_id` metadata when present;
- extend `unit_view.gd` to style the four unit kinds while preserving team color and selection rings.

Godot never decides whether a resource is depleted, a building is complete, or a queue is blocked.

### Selection and contextual orders

Keep the current battlefield controller as the interaction owner to avoid adding a UI framework.

- Box selection remains units only.
- Left click can select friendly units or one owned building.
- Right click with villagers over a resource view or completed Farm building view issues Gather using `resource_id`.
- Right click with a villager over an incomplete owned building issues ResumeConstruction.
- Right click on normal ground keeps the existing Move behavior.
- Build buttons appear when a villager is selected and enter a single placement mode.
- Placement preview is a translucent box snapped to the authoritative anchor returned by the bridge; valid/invalid state and reason come from simulation validation.
- Owned production building selection shows train/age actions, queue progress, rally status, costs, unlock reasons, and blocked reasons.
- HUD always shows Food/Wood/Gold, used/cap population, age, idle-worker count, and command feedback.
- Clicking the idle-worker count cycles/selects idle villagers using bridge-provided stable IDs.

All HUD controls continue consuming mouse events so they do not leak into world commands.

## Verification strategy

### Rust tests

Add focused tests for:

- gather/carry/deposit accounting and no income before deposit;
- fractional 20 Hz gathering (`2.0/s × 0.05s`) transferring whole resources correctly;
- finite source depletion followed by final deposit and idle;
- accepted Move/Stop/Gather/Place/Resume worker-task cancellation and Farm assignment release;
- rejected replacement commands preserving the previous worker job;
- normal Move to a blocked source remaining unreachable while Gather assigns distinct perimeter slots to multiple workers;
- one-worker Farm assignment;
- unreachable gather/construction jobs becoming idle instead of replanning forever;
- placement bounds/occupancy/affordability/reachability and one-time cost deduction;
- exact fixture Town Center/villager/resource non-overlap and walkability;
- building completion, Storehouse drop-off insertion, Farm `ResourceId` allocation, and occupancy revision behavior;
- queue cost deduction exactly once and insufficient-resource rejection;
- Stable/cavalry age lock and one-time age advancement;
- rally/spawn clearance;
- simultaneous completions at the population cap;
- both teams initialized through the same economy/production code path.

### Godot integration

Retain HPA-470 movement/selection coverage, adapted to the smaller normal start and catalogue villager speed 6.0. The cadence/interpolation smoke must update its one-second distance expectation from the old speed-12 fixture to the speed-6 normal villager while preserving the 20 Hz/interpolation assertions.

Add one `economy_smoke_test.tscn` that uses real selection, contextual right-clicks, HUD buttons, and placement input. It calls `GrusBridge.set_sim_speed(20.0)` rather than `Engine.time_scale` and exercises:

`gather Food/Wood/Gold → deposit → build House/Storehouse/Farm/production buildings → train Spearman/Archer → age-up → build Stable → train Cavalry`

The smoke must gather enough Wood before Stable/Archer/building costs; it may not rely on the starting 300 Wood. It also gathers enough Food and Gold for unit/age costs. It must not grant resources, force completion, mutate age, or bypass command validation.

The smoke verifies runtime-spawned resource/building/unit views, Farm gather targeting through its building view, stockpile changes only after deposit, population-cap changes, construction/queue progress, unlock/blocked feedback, and age transition through the same bridge snapshots used by the playable HUD.

Measure the smoke's real wall-clock duration at 20× after implementation. The CI timeout is chosen from that measured run with startup margin rather than assuming 60 seconds in advance.

The benchmark scene explicitly reseeds the 200-unit movement fixture before measuring, preserving HPA-470's 1080p benchmark contract.

## Single-PR boundary

This branch/PR will contain, in order:

1. this design and implementation plan;
2. simulation catalogue, commands/routing, economy, building, and production implementation plus Rust tests;
3. Godot bridge, runtime view attachment, primitive views, HUD, contextual commands, and integration smoke;
4. reset/benchmark adaptations and README/CI updates.

Do not split HPA-471 into separate catalogue, building, HUD, test, or production PRs/tickets. HPA-472 starts only after this PR is merged and HPA-471 is Done.
