# HPA-473 Scouting-Aware Economic AI Skirmish Design

## Status

Planning baseline for HPA-473. This design and its implementation plan live on the same branch and draft PR that will carry the implementation; there is no follow-up implementation PR for this ticket.

This extends the merged HPA-470 through HPA-472 seams. It deliberately avoids a second gameplay world, generic AI/behavior-tree framework, fog shader subsystem, minimap framework, duplicate command path, or extra content. HPA-474 remains the place for final tuning and measured performance work.

Two pre-implementation reviews are incorporated here. The architecture stays at two focused modules; the revisions lock the visibility geometry and predicates, runtime insertion seam, placement/attack anti-oracle behavior, AI determinism/idempotence, feedback ownership, no-leak presentation path, authored-coordinate ownership, smoke/test split and green-commit ordering before implementation starts.

## Outcome

Turn the current complete-but-full-information combat sandbox into the first real human-versus-economic-AI skirmish. Scouting must matter: unexplored terrain hides content, explored terrain remains known, enemy units/buildings require current vision, direct attacks cannot target hidden enemies, attack-move may continue through fog and reacquire visible targets, and the AI must obey the same information and economy rules as the human.

Runtime remains one human (Team 1) versus one scripted AI (Team 2). Rust tests parameterize the AI by TeamId and exercise both authored starting sides; no side-selection mode is added.

## Existing seams we keep

- Bevy ECS remains the only mutable gameplay model. Godot owns input, rendering, HUD and effects.
- Simulation remains fixed-step at 20 Hz.
- `PlayerCommand` / `CommandResult` / `RejectReason` remain the only gameplay-order contract.
- `target_eligible()` remains the one direct-attack / attack-move eligibility seam; HPA-473 adds current visibility there.
- `UnitIndex`, `BuildingIndex`, `ResourceIndex`, stable IDs and `MapFixture` remain identity/map seams.
- `validate_placement()` remains the placement authority.
- `TeamEconomy`, `WorkerTask`, `ProductionQueue`, `population_used()` and `population_cap()` remain the economy/production read surface.
- The existing clear + reseed path remains the restart seam.
- Existing CI keeps the >90% `grus-sim` production-line coverage gate, Godot smokes, export validation, Bevy E2E boot and 200-unit regression benchmark.

HPA-473 adds only two focused simulation modules: `visibility.rs` and `ai.rs`.

## Authoritative visibility

Use one small simulation-owned per-team visibility resource:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellVisibility {
    Unexplored = 0,
    Explored = 1,
    Visible = 2,
}

#[derive(Debug, Default, Resource)]
pub struct VisibilityMap {
    teams: HashMap<TeamId, TeamVision>,
    revision: u64,
}

#[derive(Debug, Default)]
struct TeamVision {
    explored: HashSet<GridPos>,
    visible: HashSet<GridPos>,
}
```

The battlefield is only 128x96. Two `HashSet<GridPos>` collections per team are simpler than a compressed bitset and are sufficient for the MVP. HPA-474 may optimize only if measurement justifies it.

Keep the one initial tuning constant beside `ATTACK_MOVE_RADIUS` in `catalog.rs`:

```rust
pub const VISION_RADIUS_CELLS: i32 = 10;
```

Radius 10 is intentional: with the authored starts, Team 1's gold at (25, 48) is 10 cells from the nearest Town Center footprint cell, and the mirrored Team 2 gold has the same geometry. The safe starting resource ring must be explored after initial reveal; expansion resources must remain unexplored. A fixture-level regression locks both facts so later tuning cannot silently make starting gold unknowable.

A catalogue regression also asserts `VISION_RADIUS_CELLS as f32 >= ATTACK_MOVE_RADIUS`. Attack-move must never have a wider autonomous acquisition radius than the owning team can actually see.

Visibility geometry is one contract for every consumer:

- unit reveal origin: the unit's current `GridMap::world_to_cell(SimPosition.current)`;
- completed-building reveal origins: **every** cell in its `Footprint::cells()`;
- reveal circle: Euclidean cell distance, `dx*dx + dy*dy <= radius*radius`;
- unit known/visible: its current cell is known/visible;
- building or standalone-resource known/visible: **any** footprint cell is known/visible.

Unfinished construction sites reveal nothing. There is no terrain occlusion, elevation, facing cone, Chebyshev shortcut or line-of-sight raycast. Consumers do not independently choose center-vs-edge semantics.

Exactly two public knowledge predicates own the optional-resource fallback:

```rust
visible_to(world, team, subject) -> bool
explored_by(world, team, subject) -> bool
```

`subject` resolves to the locked cell/footprint geometry above. Both predicates return `true` when `VisibilityMap` is absent. **No combat, placement, gather, rendering, minimap or AI consumer checks for `VisibilityMap` directly.** They call these predicates unconditionally. Internal helpers may resolve cells/footprints, but the missing-resource/full-information rule lives in one place only.

`refresh_visibility(world, map)` is the exclusive world-level visibility step. It recomputes the current-visible set from live ECS entities, unions it into explored state, and increments `revision` only when the resulting state changes.

For isolated pure-sim tests and the benchmark, a missing `VisibilityMap` means full information. **Do not insert visibility in `seed_skirmish()`.** That seed remains the shared full-information fixture used by existing simulation tests. Normal runtime inserts a fresh `VisibilityMap` only in `setup_fixture()` / `reset_fixture_world()`, beside `MatchSession`, then performs one initial refresh so the Start screen already has correct fog. Fog-aware Rust journeys opt in explicitly through their own test setup helper. The benchmark remains visibility-free.

## Fixed-step order

Extend the current canonical order to:

```text
human pending commands
-> combat
-> movement
-> economy
-> construction
-> production
-> visibility refresh
-> AI decision/application
-> route feedback
```

This gives visibility the post-movement/post-production world for the next decision. AI sees the same freshly computed visibility that presentation receives. AI commands are applied through `apply_player_command()` after the decision and take effect in movement/combat on the next fixed step.

A just-completed building grants vision in that tick. A newly placed incomplete site does not. Human commands at the start of a tick validate against the visibility produced at the end of the previous tick, which is the normal 20 Hz authoritative state.

## Visibility rules at gameplay boundaries

### Combat

`target_eligible(world, attacker_team, target)` keeps its current existence/health/enemy-team checks and calls `visible_to(...)` unconditionally.

This automatically gives the right behavior to both direct Attack and attack-move acquisition:

- a hidden enemy cannot receive a new direct attack;
- a direct-attack target that leaves vision is cleared and pursuit stops;
- an attack-move target that leaves vision is cleared, the unit resumes its ground destination and may later acquire another visible enemy;
- no last-known enemy unit/building position is read from live ECS combat queries.

Do not add a second combat visibility check.

Direct Attack also must not become an ID-enumeration oracle. With runtime visibility enabled, a missing target ID and a live-but-hidden target both reject as `InvalidTarget`; `TargetMissing` remains available only in the full-information/missing-visibility test seam. GDScript must not be able to distinguish “hidden live entity” from “nonexistent id.”

### Placement

Append one `RejectReason::Unexplored` variant. Placement validation order is a privacy contract:

1. owned villager;
2. kind unlocked/buildable;
3. footprint in bounds;
4. **every footprint cell explored via `explored_by(...)`;**
5. walkability/live-unit/current-goal occupancy;
6. affordability;
7. reachable perimeter slot.

The Unexplored check must happen immediately after bounds and **before the function builds the all-unit occupancy set**. Otherwise `placement_preview()` becomes a fog oracle where `Occupied` reveals a hidden building/resource/unit or hidden enemy move goal. A regression places an unexplored footprint over a hidden enemy unit and asserts the reject code is `Unexplored`, never `Occupied`.

The Godot placement preview already calls this authority for Team 1, so preview and final placement stay aligned.

Placing a site never changes visibility by itself. Because incomplete buildings grant no vision, placement cannot be used as a remote scout.

### Resource knowledge and gathering

Standalone finite resources are static map contents:

- hidden while their cell is Unexplored;
- remain known/rendered after first exploration even when no longer currently visible;
- AI may select them only after its team has explored their cell.

Own completed Farms are always valid gather knowledge. Enemy Farms are enemy buildings and therefore require current visibility; they do not become last-seen ghosts and are never admitted merely because they are present in `ResourceIndex`.

The gather command calls `explored_by(...)` unconditionally so guessed ResourceIds cannot bypass fog: explored standalone sources or own completed Farms are valid candidates; an enemy Farm is never gathered as an own economic source. The predicate's missing-resource fallback preserves existing full-information pure-sim behavior.

## Presentation: one sim truth, no hidden-node leak

Rust remains responsible for turning authoritative visibility into entity presentation state.

Gameplay view scene roots default to hidden. The first `initialize_view_metadata` pass stamps both metadata **and** the authoritative initial visibility before a new node can be picked or intentionally rendered; a later Update system beside metadata/health keeps `Node3D.visible` synchronized as visibility changes:

- friendly units/buildings: always visible;
- enemy units/buildings: visible only while currently visible to Team 1;
- standalone resource views: visible once explored;
- enemy Farm/resource view: follows enemy-building current visibility.

Godot picking helpers must explicitly skip nodes that are not visible in tree. A stale selected enemy building is cleared if it becomes hidden, and `building_snapshot()` returns an empty dictionary for a hidden enemy so the HUD cannot inspect health/queue/construction through fog.

Combat cosmetics must also obey visibility. `take_presentable_events()` derives local-player presentation permission before dictionaries reach GDScript: a hidden enemy attacker cannot expose its stable id/position for a tracer origin, while the same event may still carry hit/death position for a visible friendly target. GDScript never performs a second live hidden-attacker lookup.

No gameplay truth is duplicated into Godot; node visibility is only a presentation projection of `VisibilityMap`.

## Fog overlay

Expose two local-player bridge calls following the existing command-feedback revision pattern:

- `visibility_revision() -> int`: cheap per-frame poll;
- `visibility_snapshot()`: map width/height, packed cell states (0 Unexplored / 1 Explored / 2 Visible), **and the same revision**.

Fog/minimap poll only `visibility_revision()`; they fetch the 12,288-cell payload only when it changes. Keeping revision in the snapshot lets a consumer detect a revision change during the read. The packed conversion is isolated at this bridge boundary so the underlying HashSet representation remains replaceable if HPA-474 ever measures a need.

Add a small `fog_of_war.gd` presentation node. It rebuilds only when the sim revision changes and uses primitive horizontal quads above the terrain:

- Unexplored: opaque/dark
- Explored: translucent/dark
- Visible: no overlay instance

Use two `MultiMeshInstance3D` collections (Unexplored and Explored) with simple transparent StandardMaterial3D. No custom shader, image asset or fog framework is needed.

## Minimap

Add one `minimap.gd` Control inside the existing HUD.

The base texture is a 128x96 Image updated only when visibility revision changes. It reflects unexplored/explored/visible terrain states. Overlay drawing uses existing live scene views:

- friendly unit/building markers always;
- enemy markers only when their node is currently visible;
- explored standalone resource markers;
- current camera viewport rectangle.

Clicking the minimap converts the Control coordinate back into a map cell/world point and recenters the existing camera. It does not issue simulation commands and does not own gameplay state.

Do not create a minimap scene graph of one Control per unit/cell.

## Feature-local economic AI

Add one small runtime resource:

```rust
pub struct AiController {
    pub team: TeamId,
    decision_accumulator: f32,
    scout_route_index: usize,
    remembered_enemy_town_center: Option<GridPos>,
    next_army_kind: usize,
}
```

There is no behavior tree, utility scorer, planner graph, blackboard framework, cloned simulation world or persistent “squad entity”. Military groups are derived from live owned units at each decision.

`PlayerCommand`, `UnitCommand`, and `UnitCommandKind` derive `PartialEq` so fairness tests compare actual command lists. AI enumeration is deterministic: every unit/building/resource candidate list is sorted by stable ID before policy choice, matching the existing combat/idle-worker determinism.

The policy runs at a modest `AI_DECISION_SECONDS = 1.0` cadence through `step_ai(world, map, seconds)` and issues ordinary `PlayerCommand` values through `apply_player_command()`. `step_ai` returns immediately when `!gameplay_active(world)` **before advancing its accumulator**, so Start/Pause/Result do not create a catch-up decision on resume. AI command results are not written to the human command-feedback HUD.

The 1 Hz policy is explicitly idempotent. A decision may emit several commands, but it never reissues a command merely because a goal remains true:

- gather/build workers must be truly idle: `WorkerTask::Idle` and no `MoveOrder`; lift the existing bridge `idle_worker_ids` semantics into `grus-sim` and reuse it from both HUD and AI;
- production enqueues only when the relevant producer queue is empty (or below the one explicitly tested cap);
- building placement skips a kind/slot already satisfied by a completed or incomplete building;
- scout/attack decisions skip units already holding the intended `MoveOrder` / `CombatOrder`.

This prevents a 1 Hz Gather from repeatedly calling the existing cancellation path and resetting `GatherProgress`, prevents movement/combat thrash, and prevents queue flooding.

The controller may query all of its **own** live state. Enemy units/buildings must be filtered through current visibility before their position/health is read. Neutral standalone resources must be explored before the policy can choose them. The only retained enemy memory is the last cell of an enemy Town Center that was genuinely observed.

### Ordered policy

Keep the decision logic as explicit ordered **pure** functions, not a generic goal system. Each policy step reads `&World`, `&AiController`/decision state and `&AiMapPlan` and returns zero or one `PlayerCommand`; `decide_ai_commands()` composes them in the fixed order below into `Vec<PlayerCommand>`. `step_ai()` is only the Playing/cadence gate plus the apply loop. This keeps the 90% production-line coverage gate practical and makes the hidden-state invariance test compare the pure decision output directly.

1. **Defend visible threats.** If a currently visible enemy is near the AI Town Center/base area, direct available military units at the nearest visible threat.
2. **Avoid population stalls.** If free capacity is low and cap is below 100, place the next authored House using a real villager command.
3. **Replace workers.** Keep a small target worker count (initially 8). Queue Villagers at the Town Center through normal production.
4. **Scout early.** Before expansion-dependent growth or army training, advance along the authored route with the lowest stable-ID idle military unit when available; otherwise use the lowest stable-ID **surplus idle Villager** once the economy is above its minimum worker floor. Villagers use ordinary Move; combatants may use AttackMove. Never retask an active gatherer/build worker just to scout.
5. **Allocate idle workers.** Assign remaining idle villagers to known available sources toward a simple Food/Wood/Gold split. Existing WorkerTask/resource assignment state is the source of truth.
6. **Grow the base.** Build Barracks and Archery Range, then a Storehouse near a discovered expansion when useful; add Farms when known food supply is insufficient. Use authored slots and real placement validation.
7. **Advance.** Attempt Age 2 only after the worker target and Age-1 production core exist and the shared catalogue cost is affordable.
8. **Complete production.** Build Stable after Age 2.
9. **Train army.** Queue Spearman/Archer/Cavalry through existing producer queues using a small round-robin composition, respecting affordability and population.
10. **Attack.** Once at least six military units are available, issue one grouped AttackMove. Use the remembered enemy Town Center cell if it has actually been seen; otherwise advance along the authored route. Visible threats can be directly attacked. Losses simply drop the live army count below threshold, so normal production/regrouping rebuilds the force.

The exact worker target, surplus-worker scouting floor, six-unit attack threshold, route points and resource split are HPA-473 starting values; HPA-474 may tune them without changing architecture. Early scouting is structural, not a tuning knob: the AI must be able to discover expansion resources before its late-game gold demand depends on them.

## Authored one-map AI data

Map-specific authored data belongs beside `MapFixture`, not in a generic AI map service.

Add a compact `AiMapPlan` returned by team:

- mirrored House slots;
- Barracks / Archery Range / Stable slots;
- Farm slots;
- safe and expansion Storehouse slots;
- one mirrored scout/attack route.

The plan contains only static map coordinates. It does not contain live enemy/resource identities. Team 1 and Team 2 plans are mirror images so the same policy can be tested from both starts.

This becomes the **one authored Rust base-coordinate table** for simulation/runtime AI data. Existing duplicate Rust anchors in session/system-order tests migrate to `MapFixture::team_plan(team)`. GDScript smoke constants remain test choreography for the human UI and are **not** exposed through a new bridge snapshot; do not couple those scripts to AI tuning just to deduplicate six test constants.

The existing 18-source map is the planned content. Do not pre-emptively adjust `expansion_resources()` to compensate for policy ordering. The full-match proof must demonstrate that the AI actually reached/discovered the authored expansion. If it still stalls after doing so, revise this plan with that evidence before changing map content; do not add procedural resources or a second map.

## AI visibility and memory contract

The policy may read:

- own units/buildings/tasks/queues/economy;
- static GridMap bounds/blockers and its own `AiMapPlan`;
- standalone resource sources whose cells are explored;
- enemy entities currently visible to its team;
- `remembered_enemy_town_center`, written only while that Town Center is visible.

It may not read hidden enemy positions, health, queues, stockpiles or hidden resource positions.

A required regression seeds **two separate worlds** with identical AI own/observed state, then places hidden enemies at different unseen cells. The next command list must be identical; this repository does not need a `World` clone facility. Making a threat visible is then allowed to change the command list.

## Session and restart

AI decisions run only in Playing. Start, Paused and Result perform no AI command generation and no simulation mutation.

Normal setup/restart:

1. clears old gameplay entities/resources;
2. reseeds the same authored skirmish;
3. inserts fresh `VisibilityMap` and performs initial reveal;
4. inserts a fresh Team-2 `AiController`;
5. returns MatchSession to Start.

`clear_gameplay_world()` removes visibility/AI state and clears human pending commands. AI has no separate pending-command queue, so there is nothing else to drain.

Route-failure feedback remains one sim resource/channel but becomes team-aware (one latest reject per team). Worker route failure records the worker's TeamId; the bridge drains only Team 1 into human `CommandFeedback`. Team 2 failures therefore cannot surface as “Worker route impossible” or overwrite a same-tick Team 1 reject.

Restart therefore clears prior exploration, remembered Town Center location, scout route progress and any derived attack grouping automatically.

The benchmark reset remains visibility/AI-free so the existing HPA-470 movement baseline stays comparable; HPA-474 owns final full-game performance measurement.

## Verification strategy

### Rust

Focused unit/integration tests must cover:

- first reveal, retained exploration and loss of current vision;
- incomplete sites grant no vision; completion starts vision;
- hidden direct target rejection and direct-order cancellation when vision is lost;
- attack-move reacquisition only from visible enemies;
- explored-only placement and resource knowledge;
- AI decision invariance under different hidden enemy positions;
- visible threat changes AI defense decision;
- worker replacement, idle-worker allocation, population recovery;
- ordinary-command building/training/age-up with no grants;
- scouting updates remembered enemy Town Center only after observation;
- attack assembly, loss/regroup/rebuild;
- real worker raids reduce AI income/production;
- pause/result freeze and fresh restart state;
- the same AI controller works from both authored team starts.

### Godot/runtime

Add **one** HPA-473 integration smoke and grow that same scene/script incrementally across the PR. It runs with the real AI enabled and verifies:

- initial fog and hidden enemy views;
- reveal -> hide -> no stale selection/inspection;
- hidden enemies absent from contextual attack and minimap;
- explored standalone resources persist on map/minimap;
- minimap click recenters the existing camera;
- ordinary AI economy/build/train/scout/age progress;
- restart returns to fresh Start/fog/AI state.

The complete fogged match -> Result proof stays in bounded pure-Rust journeys, parameterized by TeamId and capped by state/tick budget rather than wall-clock-exact arithmetic. The Godot smoke does **not** duplicate an entire long match.

Existing HPA-470 command, HPA-471 economy and HPA-472 combat lifecycle smokes must be adapted under runtime fog in this same PR: they explicitly scout/march to reveal far placement/resource/combat targets, but they do not bypass visibility. Once AI lands, these **legacy scripted smokes disable only `AiController` while still in Start through one narrow test bridge helper** so their old deterministic choreography is not coupled to opponent timing; the HPA-473 smoke leaves AI enabled. Reset coverage still verifies a normal restart recreates fresh AI/visibility state.

## Risks and accepted limitations

- **Shared-pathing information leak (accepted):** `GridMap` remains global. An unexplored building/resource blocker can indirectly affect a route, revealing that some obstacle exists even though its identity is hidden. Eliminating this requires per-team knowledge-aware path maps or speculative routing, which is disproportionate to this MVP and explicitly out of scope. Fog prevents direct rendering/selection/targeting/preview oracles; path-shape inference is accepted.
- **Coverage pressure:** `ai.rs` is production code under the >90% line gate. Pure per-policy decision functions plus a thin `step_ai` keep branches independently testable instead of forcing every branch through a long match.
- **Runtime-fog migration:** turning visibility on breaks older full-information Godot choreography. Runtime insertion, presentation projection, and retargeting those smokes land together in one green commit.
- **Optional-resource fallback:** missing `VisibilityMap` intentionally means full information for focused sim tests/benchmark. Consumers cannot branch on resource presence; only `visible_to` / `explored_by` own that fallback, and runtime setup/reset tests assert the resource is actually present.

## No art/SFX task

This ticket needs no generated image art or new SFX. Fog uses primitive transparent quads; minimap uses runtime pixels/markers; existing low-poly unit/building presentation and combat cue remain sufficient. Therefore no separate asset-generation ticket is required.

## Out of scope

Extra difficulty modes, machine learning, behavior trees, utility-AI frameworks, influence maps, navmesh/flow-field work, last-seen enemy-building ghosts, terrain occlusion, elevation vision, stealth, radar, extra units/buildings/factions/maps, procedural map generation, replay/save/load, multiplayer, custom fog shader pipeline, minimap framework, image-generation pipeline and final balance/performance tuning.
