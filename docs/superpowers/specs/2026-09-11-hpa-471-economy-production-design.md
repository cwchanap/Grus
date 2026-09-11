# HPA-471 Worker Economy, Construction, and Army Production Design

## Status

Planning baseline for HPA-471. This document and its implementation plan live on the same branch and draft PR that will carry the implementation; there is no follow-up implementation PR for this ticket.

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

## Simulation structure

Add four focused modules to `grus-sim`:

- `catalog.rs`: fixed enums/specs/costs and initial tuning values.
- `economy.rs`: team stockpiles, finite resource sources, worker carry/task state, gather/deposit loop, idle-worker queries.
- `buildings.rs`: building identity/state, placement validation, footprint reservation, construction progress, drop-off behavior.
- `production.rs`: production queues, population, rally/spawn clearance, age advancement, unit spawning.

Keep movement/pathfinding in the existing `movement.rs`/`commands.rs`. Extract only the minimum reusable near-target routing helper needed by gathering, construction, and spawn/rally behavior; do not add a navigation service abstraction.

### Stable identity

Extend `ids.rs` with:

- `BuildingId(u32)`
- `ResourceId(u32)`

Add a small monotonic allocator resource for runtime-trained units and newly placed buildings. Authored starting entities keep deterministic IDs so Godot tests can target them reliably.

### Typed catalogue

The catalogue is compile-time Rust data returned by `match` functions. It is the only source of gameplay costs, times, footprints, unlocks, population values, and gathering constants. Godot reads display values through bridge snapshots instead of duplicating costs in GDScript.

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
- Carry limit: 10.
- Base gathering rate: 2.0 resources/second.
- Age 2 gathering rate: 2.2 resources/second.
- Population ceiling: 100 per side.
- Base Town Center capacity: 10; each completed House adds 10.
- Unit movement speed: villager/spearman/archer 6.0, cavalry 8.0. The benchmark fixture retains speed 12.0.
- Resource node amounts: berries 600 Food, tree 400 Wood, gold deposit 600 Gold.

The numbers are deliberately easy to tune later in HPA-474 without changing code structure.

## Scenario and fixture split

`MapFixture::battlefield()` remains the authored map geometry. Replace the current normal-start assumption of 200 units with an explicit skirmish start:

- Team 1 Town Center near the current left base marker; Team 2 mirrors it on the right.
- Four villagers per side around their Town Center.
- Safe starting resources on both sides: two berry sources, three trees, one gold deposit.
- Existing exposed expansion anchors remain and gain gold/tree resources.

Keep `units_200()` as a benchmark-only fixture. The normal main scene boots the economy start. `benchmark_200.gd` explicitly requests the 200-unit reset before measuring, so HPA-470 performance coverage is preserved without making the normal game start population-capped.

## Domain model

### Team economy

A `TeamEconomy` resource keyed by `TeamId` owns only team-wide mutable values:

- `ResourceStockpile { food, wood, gold }`
- current age (`Age1` or `Age2`)
- whether age advancement is already queued/completed

Population is derived from live units plus completed capacity-granting buildings instead of storing a second mutable count.

### Units and workers

Extend `Unit` with `UnitKind`. Only villagers receive worker components:

- `Carry { kind: Option<ResourceKind>, amount }`
- `WorkerTask`

`WorkerTask` is a small explicit state machine:

- `Idle`
- `ToSource(ResourceId)`
- `Gathering(ResourceId)`
- `ToDropoff { source, building }`
- `ToConstruction(BuildingId)`
- `Constructing(BuildingId)`

Movement remains represented by the existing `MoveOrder`. The worker system advances task phases after movement reaches a valid nearby slot.

### Resource sources

`ResourceSource` stores stable ID, resource kind, remaining amount, and authored cell. Source cells are blocked in `GridMap`, so workers route to nearby walkable slots. When a finite source reaches zero, it is removed from presentation and its cell is unblocked through `GridMap::set_blocked`, incrementing the map revision normally.

A completed Farm is a renewable Food source but is still a building. Exactly one villager may hold the farm assignment at a time. A second gather command to the same occupied farm is rejected with a visible reason.

### Buildings

`Building` stores stable ID, team, kind, footprint anchor, and construction state. A placed building reserves its full footprint immediately. This makes placement authoritative and automatically triggers HPA-470 route replanning through the existing map revision.

Placement uses the lower-left grid cell as the canonical anchor. Validation checks, in order:

1. issuer owns the selected villager;
2. building kind is unlocked/buildable;
3. footprint is fully in bounds;
4. every footprint cell is currently walkable;
5. team can afford the cost;
6. at least one perimeter slot is reachable by the builder.

Only after all checks pass does the simulation deduct resources, reserve cells, create the under-construction building, and assign the builder. Preview cancellation therefore costs nothing. There is no demolition/refund path.

One active builder advances a site's catalogue build time. If that villager receives another command, construction pauses. Right-clicking the incomplete owned building with a villager selected reassigns that villager and resumes construction. No multi-builder speed stacking is implemented.

Town Centers and Storehouses are all-resource drop-offs once complete. The starting Town Center begins complete and is not buildable.

## Gathering loop

A contextual gather command targets a `ResourceId` rather than a point.

1. Validate selected units are owned villagers and the source still exists.
2. For Farm, reserve its single worker slot.
3. Route each accepted worker to a nearby walkable source slot.
4. At the source, gather at the current age rate until carry is full or the finite source depletes.
5. Route to the nearest reachable completed Town Center/Storehouse.
6. Deposit the carried amount atomically into the team's stockpile.
7. If the source still exists, route back and repeat.
8. If the source depleted, deposit any carried amount and settle visibly to `Idle`.
9. If a required route cannot be found, clear the worker task, release a Farm assignment if applicable, and publish feedback instead of retrying every tick.

Income is never credited before deposit.

## Production, rally, population, and age-up

Completed production buildings own a simple FIFO `ProductionQueue`.

- Town Center: Villager and Age 2.
- Barracks: Spearman.
- Archery Range: Archer.
- Stable: Cavalry.

Enqueue validation checks ownership, building completion, producer/unit compatibility, unlock state, and affordability. Cost is deducted once on accepted enqueue. Queue progress advances only for the front job.

A finished unit job waits at 100% when population is full or no perimeter spawn cell is free. It resumes automatically when space exists. To make simultaneous completions deterministic, completed production buildings are processed in ascending `BuildingId`; population is recomputed/updated after each successful spawn, so two same-tick completions cannot both consume the final slot.

Spawn clearance selects an unblocked perimeter cell not occupied by a live unit. Newly spawned units receive a stable `UnitId`. If a rally point is set, the unit immediately receives a normal move order toward it; a failed rally route does not cancel the trained unit.

Age advancement is a Town Center queue job and competes with villager production. The team may have at most one age-up job queued or completed. Completion flips the team to Age 2, unlocks Stable/cavalry, and applies the 2.2/s gathering rate. No tech tree or additional upgrades are introduced.

## Commands and bridge

Keep the existing `UnitCommand` for move/stop. Add a compact top-level command enum for HPA-471 actions:

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

`PendingCommands` in `grus-godot` becomes `Vec<PlayerCommand>`. Each fixed tick drains the queue, validates against authoritative Bevy state, applies accepted actions, and updates the existing feedback revision/text resource.

Read-only bridge methods expose snapshots rather than mutable Godot state:

- player stockpile, age, population used/cap, idle-worker IDs/count;
- building state, queue item/progress/blocked reason;
- catalogue cost/footprint/unlock data for HUD labels;
- placement preview result and snapped anchor;
- resource/building stable metadata for contextual targeting.

## Godot presentation and interaction

Use primitive low-poly scenes; HPA-471 does not require generated art.

Add:

- `resource_view.tscn` + `resource_view.gd`: one simple mesh styled by resource kind and tagged with `resource_id`.
- `building_view.tscn` + `building_view.gd`: one simple mesh scaled/styled by building kind, team color, construction progress, and selection state.
- extend `unit_view.gd` to style the four unit kinds while preserving team color and selection rings.

The bridge attaches the appropriate `GodotScene` and stable metadata to ECS entities. Godot never decides whether a resource is depleted, a building is complete, or a queue is blocked.

### Selection and contextual orders

Keep the current battlefield controller as the interaction owner to avoid adding a UI framework.

- Box selection remains units only.
- Left click can select friendly units or one owned building.
- Right click with villagers over a resource view issues Gather.
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
- finite source depletion followed by final deposit and idle;
- one-worker Farm assignment;
- unreachable gather/construction jobs becoming idle instead of replanning forever;
- placement bounds/occupancy/affordability/reachability and one-time cost deduction;
- building completion and occupancy revision behavior;
- queue cost deduction exactly once and insufficient-resource rejection;
- Stable/cavalry age lock and one-time age advancement;
- rally/spawn clearance;
- simultaneous completions at the population cap;
- both teams initialized through the same economy/production code path.

### Godot integration

Retain HPA-470 movement/selection coverage, adapted to the smaller normal start. Add one `economy_smoke_test.tscn` that uses real selection, contextual right-clicks, HUD buttons, and placement input to exercise:

`gather → deposit → place/build → train → age-up`

The smoke may raise `Engine.time_scale` so production/build durations complete quickly, but it must not grant resources, force completion, or bypass command validation. It verifies stockpile/progress feedback through the same HUD snapshots used by the playable scene.

The benchmark scene explicitly reseeds the 200-unit movement fixture before measuring, preserving HPA-470's 1080p benchmark contract.

## Single-PR boundary

This branch/PR will contain, in order:

1. this design and implementation plan;
2. simulation catalogue/economy/building/production implementation and Rust tests;
3. Godot bridge, views, HUD, contextual commands, and integration smoke;
4. benchmark/reset adaptations and README/CI updates.

Do not split HPA-471 into separate catalogue, building, HUD, test, or production PRs/tickets. HPA-472 starts only after this PR is merged and HPA-471 is Done.
