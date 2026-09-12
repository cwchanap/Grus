# HPA-471 Worker Economy, Construction, and Army Production Design

## Status

Planning baseline for HPA-471. This document and its implementation plan live on the same branch and draft PR that will carry the implementation; there is no follow-up implementation PR for this ticket.

This revision incorporates two pre-implementation reviews of the merged HPA-470 seams. It keeps the existing Bevy/Godot ownership model and locks the contracts that would otherwise cause rewrites: command results, worker retasking, adjacent approach slots, shared footprint geometry, runtime presentation, reset/cutover ordering, fixed-tick order, and test-time simulation speed.

## Outcome

Turn the retained HPA-470 battlefield into a playable grow-and-spend RTS loop. A normal match starts with one Town Center and four villagers per side. The player can gather Food/Wood/Gold, shorten return routes with a Storehouse, construct the six buildable structures, train villagers/spearmen/archers, advance once, then build a Stable and train cavalry.

Combat, fog, AI, destruction, save/load, extra content, and final balance polish remain later-ticket work.

## HPA-470 seams we keep

- Bevy ECS owns authoritative gameplay state; Godot owns rendering, input, HUD, and feedback.
- The simulation runs at 20 Hz and Godot interpolates presentation.
- `GridMap::set_blocked` is the walkability mutation seam; its revision drives movement replanning.
- Stable `UnitId` values cross the Godot bridge.
- HPA-470 already has reference-counted current/goal destination reservations, A*, selection, move/stop, control groups, camera pan/zoom, reset/smoke coverage, export smoke, and a 200-unit benchmark.

HPA-471 extends these seams. It does not add a second `GameState`, a persistent reservation table, a navigation service, JSON/RON content data, or a generic RTS framework.

## Simulation modules

Keep four focused simulation modules:

- `catalog.rs`: fixed enums/specs/costs and initial tuning values.
- `economy.rs`: team stockpiles, worker state, resource sources, drop-offs, gather/deposit loop, idle-worker queries.
- `buildings.rs`: building identity/state, placement, footprint reservation, construction, Storehouse/Farm completion effects.
- `production.rs`: FIFO queues, population, rally/spawn clearance, age advancement, unit spawning.

Movement/pathfinding stays in `movement.rs` / `commands.rs`. Common command/result and approach-slot contracts are introduced before feature-specific behavior so later modules do not invent parallel versions.

## Typed catalogue

The catalogue is compile-time Rust data returned by `match` functions. It is the only source of gameplay costs, times, footprints, unlocks, population values, movement speeds, and gathering rates. Godot reads display values through bridge snapshots instead of duplicating them in GDScript.

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
- Unit speed: villager/spearman/archer 6.0, cavalry 8.0.
- Benchmark fixture speed remains 12.0.
- Resource node amounts: berries 600 Food, tree 400 Wood, gold deposit 600 Gold.

The 2.2/s Age 2 rate, rally points, and idle-worker navigation remain in scope because HPA-471 explicitly requires them.

## Stable identity and authored fixture

Extend `ids.rs` with `BuildingId(u32)` and `ResourceId(u32)`. One monotonic `IdAllocator` owns `next_unit`, `next_building`, and `next_resource`. Authored entities use deterministic IDs; allocator counters start above authored maxima.

`MapFixture::battlefield()` keeps the current 128×96 map geometry and `left_spawn` / `right_spawn` because the benchmark uses them. Replace the old untyped `starting_resources` and `expansion_resources` arrays with typed `ResourceSpawn` descriptors; there must be only one resource-position table.

Town Center anchors:

- Team 1: `(12,46)`, footprint `x=12..15`, `y=46..49`.
- Team 2: `(112,46)`, footprint `x=112..115`, `y=46..49`.

Villager cells:

- Team 1: `(11,45)`, `(11,50)`, `(16,45)`, `(16,50)`.
- Team 2: `(116,45)`, `(116,50)`, `(111,45)`, `(111,50)`.

Safe resources:

- Team 1 berries `(22,42)`, `(22,54)`; trees `(20,45)`, `(20,48)`, `(20,51)`; gold `(25,48)`.
- Team 2 berries `(105,42)`, `(105,54)`; trees `(107,45)`, `(107,48)`, `(107,51)`; gold `(102,48)`.

Expansion resources:

- southwest: gold `(45,16)`, trees `(43,18)`, `(47,18)`;
- northeast: gold `(82,79)`, trees `(84,77)`, `(80,77)`.

Fixture tests prove Town Center/resource non-overlap, authored cells in bounds, mirrored safe-source counts, and all villager starts walkable after Town Center footprints are blocked.

`units_200()` stays benchmark-only.

## One footprint contract

Use one spatial rectangle type everywhere:

```rust
#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub struct Footprint {
    pub anchor: GridPos,
    pub width: u8,
    pub height: u8,
}
```

`Footprint` provides `cells()` and immediate `perimeter_cells()` helpers. Every Building has one. Standalone finite resource entities use a 1×1 footprint. A Farm is one entity with both `Building` and `ResourceSource`, so its 2×2 Building footprint is also its gather footprint.

Do not copy anchor/width/height into `Dropoff` or another routing component.

## Command result contract

Replace feature-specific reject enums and string-parsed test assertions with one typed command contract:

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
```

Batch Move/Gather uses per-unit results. Single building/production commands use `reject`. `apply_player_command(&mut World, &mut GridMap, PlayerCommand) -> CommandResult` is the authoritative dispatcher; Godot only queues commands and formats presentation text.

The bridge stores both readable feedback text and `last_reject_code`. Godot smokes assert typed codes/state rather than English prose. Human-readable wording may change without breaking integration tests.

`PlayerCommand` grows only when its owning behavior lands; no intermediate placeholder variants are allowed.

## Shared approach-slot routing

HPA-470's reservation invariant stays: live current cells and existing `MoveOrder.goal` cells are reference-counted during assignment. Do not add a persistent reservation resource.

Keep normal Move destination generation unchanged: Move targets a walkable center and may fan out in rings.

Gather/build/drop-off actions use a separate **candidate generator** but the same reservation and path-selection loop:

```rust
approach_slots(map, footprint, used, count) -> Vec<GridPos>
```

`approach_slots` returns only walkable cells on the immediate perimeter of the target footprint. It never includes blocked footprint cells and never searches radius 2+. If there are not enough reachable immediate-adjacent slots, the remaining units reject as `Crowded`/`Unreachable` rather than working from a distance.

Worker tasks store the assigned slot explicitly:

```rust
Idle
ToSource { source: ResourceId, slot: GridPos }
Gathering { source: ResourceId }
ToDropoff { source: ResourceId, dropoff: BuildingId, slot: GridPos }
ToConstruction { building: BuildingId, slot: GridPos }
Constructing { building: BuildingId }
```

Arrival is positive: a worker transitions only when its current cell equals the stored slot. Absence of `MoveOrder` alone is not proof of adjacency.

Required regression: normal Move onto a blocked source remains `Unreachable`, while four villagers gathering one blocked source receive four unique immediate-perimeter goals.

## Team economy and worker state

`TeamEconomy`, keyed by `TeamId`, owns:

- `ResourceStockpile { food, wood, gold }`;
- `Age1` / `Age2`;
- age-up queued/completed state.

Population remains derived from live units plus completed Town Center/House capacity.

Use an invariant-preserving Carry type:

```rust
pub enum Carry {
    Empty,
    Holding { kind: ResourceKind, amount: NonZeroU32 },
}
```

A worker never loses carried resources because of retasking:

- Move, Stop, Place, and Resume preserve `Carry`.
- An accepted Gather while carrying any load first routes to a reachable Dropoff, deposits that load, then continues to the requested source.
- Carry never mixes resource kinds.

`GatherProgress(f32)` remains a per-worker fractional accumulator. At 20 Hz, gathering adds `rate * 0.05` and transfers whole units when progress crosses 1.0. Reset progress when leaving/canceling the active gathering phase. Tests exercise `step_economy`; they do not reimplement the accumulator algorithm in the test body.

## Buildings before gathering

The construction model lands before gather behavior so starting Town Centers are real Buildings from birth and drop-offs do not duplicate geometry.

`Building` owns stable ID, team, kind, construction state, and shares its entity with `Footprint`. `BuildingIndex` follows the existing `UnitIndex` pattern.

The pure simulation skirmish seed creates:

- `TeamEconomy` for both teams;
- complete starting Town Center Buildings with 4×4 Footprints;
- four Villagers/team with worker components;
- map occupancy for Town Center footprints;
- deterministic indexes and allocator state.

This seed is testable in Rust before the Godot main scene switches away from the 200-unit fixture.

### Placement and construction

Placement validates, in order:

1. owned villager;
2. kind unlocked/buildable;
3. footprint in bounds;
4. footprint cells walkable;
5. affordability;
6. immediate-perimeter builder slot reachable.

Only after validation succeeds does it cancel the accepted builder's previous worker activity, deduct cost once, allocate ID, block the footprint, spawn the Building, and route the builder to the stored approach slot.

One builder advances construction. Accepted replacement commands or Stop pause the building; rejected replacements preserve current work. Resume validates before replacing worker activity. No multi-builder acceleration, demolition, placement cancellation, or refund system is introduced.

`Dropoff` is a marker carrying only team ownership:

```rust
pub struct Dropoff { pub team: TeamId }
```

Starting Town Centers have it from birth. Completed Storehouses gain it once. Routing reads `Dropoff + Building + Footprint`; it does not duplicate geometry or branch on building names.

## Resource sources and gathering

Tasking a villager with Gather targets `ResourceId`, not a point.

Standalone berries/trees/gold are `ResourceSource + Footprint(1×1)`. Their source cells are blocked. A completed Farm adds `ResourceSource { remaining: None, assigned_worker: None }` to the existing Farm Building entity and receives a new runtime `ResourceId`; it keeps exactly one Godot building view.

Gather flow:

1. validate source and owned villagers;
2. reserve distinct immediate-perimeter slots;
3. validate/reserve Farm's one-worker slot when applicable;
4. if Carry is non-empty, route to a Dropoff first while remembering the requested source;
5. otherwise route to the source slot;
6. gather whole resources into Carry, never stockpile directly;
7. when full or depleted, route to nearest reachable same-team Dropoff immediate perimeter;
8. deposit atomically, then return to source if it still exists;
9. after finite depletion, deliver the final load then become Idle;
10. if a required route becomes impossible, become Idle with typed feedback rather than retrying A* every tick.

Finite depletion removes the source from `ResourceIndex`, despawns the entity, and unblocks its 1×1 footprint through `GridMap::set_blocked`.

## Worker cancellation

One `cancel_worker_activity()` helper owns cleanup. Replacement semantics are always:

**validate new command → cancel old activity for accepted worker → apply replacement.**

A rejected Move/Gather/Place/Resume does not destroy the previous task/order.

Cancellation:

- releases Farm assignment if held;
- clears an active construction builder if held;
- resets `GatherProgress`;
- sets `WorkerTask::Idle`;
- removes/replaces old `MoveOrder` as appropriate;
- never discards `Carry`.

Stop is explicit unconditional activity cancellation for owned units, but still preserves Carry.

## Production, rally, population, and Age 2

Completed producers own FIFO `ProductionQueue`:

- Town Center: Villager and Age 2.
- Barracks: Spearman.
- Archery Range: Archer.
- Stable: Cavalry.

Enqueue validates ownership, completion, producer compatibility, unlock, and affordability. Charge once on acceptance. Front job advances only at the queue head.

Population used is live units. Population cap is completed Town Center/House capacity clamped to 100. A ready unit job stays at 100% while cap or spawn clearance blocks it.

Process ready producers in ascending `BuildingId`; recompute population after each successful spawn so two same-tick completions cannot both consume the final slot.

Spawn uses immediate free perimeter cells. Runtime units receive allocator IDs. Rally points remain in scope: `SetRally` stores a target; after spawn, assign a normal Move. A failed rally route leaves the trained unit spawned and idle.

Age advancement is a Town Center FIFO job, can be queued/completed once, costs 300 Food + 200 Gold, and takes 45 seconds. Completion unlocks Stable/Cavalry and changes gathering from 2.0/s to 2.2/s beginning with the next economy tick.

## Canonical fixed-step order

When the Godot runtime switches to the economy skirmish, its one fixed chain is:

1. `apply_pending_commands`;
2. `step_movement`;
3. `step_economy`;
4. `step_construction`;
5. `step_production`.

Rust integration coverage locks the order before the Godot cutover. Movement arrival is visible to economy/construction in the same fixed tick; production runs last, so an Age 2 completion affects the next economy tick.

## Godot integration and green-commit rule

Every implementation commit must leave the currently enabled CI gates green. In particular, do not switch `setup_fixture` to the economy start while HPA-470 `smoke_test.gd`, `reset_test.gd`, and `benchmark_200.gd` still assume 200 startup units.

Use two integration stages:

1. **Presentation seam on the existing 200-unit baseline.** Replace fixture-only unit `GodotScene` insertion with one runtime attachment system while keeping the old startup fixture. This isolates view-attachment regressions against known-green HPA-470 smokes.
2. **Atomic skirmish cutover.** Switch normal startup/reset to the economy seed in the same commit that adapts `smoke_test.gd`, `reset_test.gd`, and `benchmark_200.gd`, adds building/resource runtime views, wires the full fixed chain, and adds benchmark reset.

`benchmark_200.gd` must call `reset_benchmark_fixture()` before waiting for 200 units. Normal `reset_fixture()` restores the economy start on a fresh `GridMap`.

The unit presentation seam must preserve these HPA-470 contracts through the refactor:

- `unit_views` group;
- `SelectionRing` child;
- `set_selected(bool)` method;
- `unit_id` and `team_id` metadata (plus new `unit_kind`).

### Runtime view attachment

One `grus-godot` system attaches views for ECS entities missing a view marker:

- `Unit` → `unit_view.tscn`;
- `Building` → `building_view.tscn`;
- `ResourceSource` without `Building` → `resource_view.tscn`.

Farm is both Building and ResourceSource, so it receives one building view with `resource_id`/`resource_kind` metadata. `grus-sim` never imports Godot types.

### Bridge snapshots and feedback

Bridge reads are snapshots computed from ECS:

- economy: Food/Wood/Gold, age, population used/cap, idle-worker IDs/count, latest reject code;
- building: kind, construction progress, queue/progress/blocked reason, rally point;
- catalogue action data: cost/unlock;
- placement preview: snapped anchor/footprint/valid/reject code.

GDScript does not mirror gameplay state.

## Integration-test simulation speed

`godot-bevy 0.11.0` installs Bevy `TimePlugin` and drives `app.update()` from Godot `_process`, so `Engine.time_scale` is not the economy-smoke acceleration contract.

Expose a test-only bridge method that validates a positive finite value and calls:

```rust
Time<Virtual>::set_relative_speed(relative_speed)
```

Do **not** modify `Time<Virtual>::max_delta`. In Bevy 0.18.1, `max_delta` clamps the raw real-time delta before relative-speed multiplication, so a 20× speed is not capped to ~15× at ordinary frame times.

The economy smoke sets 20× and restores 1× before exit. CI initially gives this new smoke a conservative 300-second timeout; after the first successful CI run, record the CI elapsed time and tighten the timeout within the same PR. Do not derive the CI timeout from a local workstation measurement.

## Godot interaction

Keep `battlefield_controller.gd` as interaction owner; no new UI framework.

- Box selection remains units only.
- Left click selects friendly units or one owned building.
- Right click villagers + resource/Farm → Gather.
- Right click villager + incomplete owned building → Resume.
- Right click selected production building + ground → Set Rally.
- Otherwise selected units + ground → Move.
- Build buttons enter one authoritative placement-preview mode.
- Production/age buttons read cost/unlock/progress from Rust snapshots.
- HUD always shows resources, population, age, idle-worker count, and feedback.
- Idle-worker navigation stays minimal: clicking the idle-worker control selects/cycles through bridge-provided stable idle IDs. Godot does not maintain an authoritative idle list.

## Verification strategy

Follow the repo's existing test style: focused `#[cfg(test)] mod tests` beside implementation, plus only a small number of cross-module integration tests. Do not invent unnamed `game.*` test harness APIs in the plan.

Rust tests cover:

- catalogue age gates;
- fixture/footprint invariants;
- shared immediate approach slots and reservation behavior;
- typed command results/rejects;
- Carry invariants and retasking without resource loss;
- actual `step_economy` gather/deposit/depletion behavior;
- Farm one-worker assignment;
- placement/cost/occupancy/construction;
- Storehouse drop-off behavior;
- queue charging, producer compatibility, age lock, rally/spawn clearance;
- same-tick population-cap completion;
- canonical system order;
- both teams seeded through the same simulation path.

Godot coverage:

- HPA-470 selection/move/stop/camera/HUD/interpolation remains green after each integration change;
- normal reset checks deterministic economy-start IDs/views/state;
- benchmark reset checks exactly 200 unique benchmark UnitIds;
- economy smoke drives a solvent real UI path through gather → Storehouse/Farm/buildings → train → Age 2 → Stable/Cavalry without debug grants.

The economy smoke gathers enough resources to pay the full route, verifies typed reject codes/state rather than English feedback strings, and proves runtime unit/building/Farm presentation.

## Single-PR boundary

This draft PR remains the implementation PR. Commit-level gates provide review boundaries, but HPA-471 is not split into catalogue/building/HUD/test tickets or PRs.

No combat, fog, AI, destruction, persistence, generic framework, image-generation pipeline, or final balance work is added.