# HPA-450 — Walls, gates, and Gun Turret: design contract

**Linear:** https://linear.app/cwchanap/issue/HPA-450/grus-survival-48-walls-gates-and-towers  
**Parent:** [Winter Survival design](2026-10-21-winter-survival-design.md)  
**Dependency:** HPA-449 / GitHub PR #10 merged into `main` (GitHub verified 2026-10-27 Pacific; Linear status may lag).  
**Status:** Proposed design for review. This draft PR is the **only PR** for HPA-450; implementation follows on the same branch/PR.

## User outcome

A player selects a Worker, lays out a run of inexpensive 1×1 Walls and Gates without rearming each placement, and builds a 2×2 Gun Turret. Walls shape alien routes; a Gate lets humans pass but not aliens. A turret automatically shoots currently visible aliens. If a defensive ring blocks the route to the Research Center, aliens attack reachable walls/gates/structures rather than sitting idle. The current one-base winter survival, waves, Crystals, fog, and Start/Paused/Result/Restart flow stay intact.

## Existing owners and decisions

| Responsibility | Reuse / minimal extension | Explicitly not building |
| --- | --- | --- |
| Balance | `crates/grus-sim/src/catalog.rs`: `BuildingKind`, `BuildingSpec`, optional `CombatSpec` | Second defence catalogue |
| Occupancy and pathfinding | `map.rs::GridMap`, its revision, existing A*, `MoveOrder` / `movement.rs` | Second alien-only map, flow fields |
| Placement and construction | `buildings.rs::{validate_placement,apply_place_building,apply_resume_construction,step_construction}` | Construction queue or build-order scheduler |
| Unit combat | `combat.rs::step_combat`, `CombatTarget`, `AcquisitionSnapshot`, `strike`, `CombatEvents` | Tower-specific combat loop/damage pipeline |
| Alien intent | `waves.rs` retry and ordinary attack/attack-move orders | New alien AI framework |
| Godot/UI | `GrusBridge` snapshots and event drain; `battlefield_controller.gd`, `combat_fx.gd`, existing building views | Godot-authoritative combat or new art/SFX |

Maintain the one authoritative Bevy World, one GridMap, one fixed 20 Hz simulation chain, and deterministic stable-ID tie breaks. Presentation only consumes bridge output; input only sends existing validated commands.

## 1. Typed structures and first-pass combat

Add `Wall`, `Gate`, `GunTurret` to `BuildingKind::ALL`. All three use ordinary building placement, construction, damage, destruction, selection and `BuildingIndex`. Walls and Gates are 1×1 (10 Scrap / 400 HP and 30 Scrap / 600 HP), never produce units, and have zero heat demand. The Gun Turret is 2×2, costs 100 Scrap + 20 Coal, fires at range 8, and has a Spitter counter bonus. The parent contract already records the Gun Turret's future heat demand of 2; **the heat simulation is HPA-181**, not this PR.

Extend `BuildingSpec` with `combat: Option<CombatSpec>`; non-towers use `None`. Reuse the existing single-counter `CombatSpec` instead of adding a damage table. **Proposed, tunable defaults only** for unspecified values: Wall 5s build, Gate 8s build, Gun Turret 25s build / 350 HP / 10 damage every 1.0s / +5 against Spitter. Put those numbers in the typed catalogue; finalize through review and defer broader balancing to HPA-289.

On **construction completion**, grant the Gun Turret `AttackCooldown` alongside existing completion-time grants (`Dropoff` / `ProductionQueue`); an incomplete site has no attacking component, even though its health is full at placement. A completed turret automatically selects a currently visible opposing alien inside its actual attack range. Reuse the existing per-tick acquisition snapshot and **extend `acquire_target` with an attack-radius parameter** (unit callers keep `ATTACK_MOVE_RADIUS`; the turret passes its catalogue range). Measure range from the **closest point on the turret's 2×2 footprint to the target position**, not footprint center; check range before the visibility lookup. Choose nearest eligible target, then lowest stable UnitId on ties. No tower `CombatOrder` or movement. Towers use the same damage, cooldown, hit event and target-destruction path as units; a tower kill awards Crystals through the existing authoritative kill seam, exactly once.

**Reuse `CombatTarget::{Unit(UnitId), Building(BuildingId)}` as the attacker identity too**; its two variants already represent the exact stable identities needed, so no duplicate `CombatAttacker` enum is warranted. Change `CombatEvent.attacker` from `UnitId` to `CombatTarget`. Let `strike` accept an attacker entity, team and combat spec; avoid duplicating damage, kill, health and cooldown handling. Bridge serialization includes both `attacker_kind` and attacker ID, matching the existing target-kind shape. Continue to scrub unseen/dead enemy attackers to a uniform absent identity (e.g. `CombatTarget::Unit(UnitId(0))`, with no original-kind leak), applying visibility to a unit's cell or a building's footprint; the controller resolves a unit or building view to spawn the existing tracer. No projectile simulation or dedicated sound assets.

## 2. One GridMap with team-aware Gate traversal

Preserve `GridMap::is_walkable(cell)` as **free/unoccupied ground** for placement previews, spawn slots, resource/approach destinations and other non-mover checks. It must be false for Walls *and* Gates so buildings and spawns never overlap them.

Keep Gate cells **inside `GridMap.blocked`** and mark them in one additional `gates: HashSet<GridPos>` that is always a subset of `blocked`; this is **one grid, not a second navigational map**. Existing `is_walkable` stays unchanged for all non-mover checks. Clone the gate flags with the map; `set_blocked(cell, false)` also clears the gate flag, preserving existing destruction/release code. A Gate-site placement sets `blocked` **and** the gate flag atomically for one cell/revision increment (do not bump twice); a Gate is **human-passable from placement, even while under construction**, just as other construction sites claim occupancy immediately. No completion-time grid flip is needed. Add one mover-aware predicate:
- Team 1 may traverse a Gate site or completed Gate, but no team may build/spawn on it.
- Team 2 may not traverse a Gate cell at any construction state.
- Both teams are blocked by Walls, terrain, all other building footprints and out-of-bounds cells.

In other words, `is_traversable_for(team, cell)` returns `is_walkable(cell) || (team == TeamId(1) && gates.contains(&cell))`. Only a successful placement may set a gate flag on an otherwise free cell; on removal the flag and blocking clear together.

Pass `TeamId` only along the existing **mover-facing paths**: A* route construction (`find_path` and its callers), route-replan/recovery, and per-tick movement candidate plus separation checks. Audit Move, AttackMove, pursue, worker gather/dropoff/construction, wave spawn route assignment and route recovery. Keep placement, resource positions, spawn-slot generation and non-movement checks on the existing team-agnostic free-cell predicate. No compatibility overloads: update signatures directly and compile-error-audit callers.

**A* start/goal boundary:** do not use `is_walkable(start)` as the initial guard for every mover: a Team-1 unit **already standing on a Gate** must be allowed to start/replan a route from that cell. A Team-2 alien starting on a Gate is invalid and returns no path. **Goals must be free ground for both teams** (a Gate is never a valid command destination or reserved goal), even though a Team-1 route may cross Gate cells as *intermediate* waypoints. Apply team-aware traversal to start and neighbor checks and the free-cell rule to goal checks. Test the three cases explicitly: Team-1 gate start succeeds, Team-2 gate start fails, and gate goal fails for both teams.

Wall placement/destruction uses existing blocking. Gate placement/destruction uses the same blocking transition plus its gate flag, with **one existing map-revision bump per changed cell**, including when a Gate site is first placed; no second update at completion. Existing `MoveOrder.map_revision` triggers route refresh: humans can traverse the Gate site immediately, while aliens must replan or attack it. Construction preview uses a cloned *post-placement* grid for builder reachability; when previewing a Gate, its gate flag must be present in the clone before calculating the builder route. A Gate is never a valid building-placement or destination slot.

## 3. Alien fallback for sealed routes

Do not change HPA-449's deterministic spawning, timing or budget. Only when an alien's Research Center route cannot be completed (including an invalidated/preserved route after wall/gate revision), pick the **nearest reachable Team-1 structure**, including Wall, Gate, unfinished construction sites and other buildings. Deterministic ranking: geometric distance to the target footprint, then stable BuildingId; verify reachability to an outside immediate-perimeter approach slot via existing `approach_slots` and team-aware A*. Prefer an actually reachable alternative over an unrouteable, geometrically closer one.

**Fallback must dispatch an ordinary `AttackMove`, never a direct `CombatOrder::Attack` to a distant wall.** Direct Attack is rejected by `apply_attack` when the structure is outside the alien's current fog, and manually inserted direct orders are dropped by `refresh_target` on the next tick. A wall 40+ cells from an alien at the map edge is beyond its 10-cell vision; a pure-sim test without `VisibilityMap` would conceal this bug. When the Research Center's approach route is rejected as `Unreachable` (or a live core route fails after a map revision), rank structures by geometric distance and stable ID, probe reachable outside approach slots, and use the **existing `waves.rs::dispatch_attack_move` / validated command path** for the chosen slot. It can report acceptance/rejection through the existing `CommandResult`; no direct order insertion or new AI state. As the alien approaches, normal `AttackMove` acquisition already finds buildings inside `ATTACK_MOVE_RADIUS`, with vision guaranteed by the catalogue's `VISION_RADIUS_CELLS >= ATTACK_MOVE_RADIUS` regression. When the structure dies, the surviving alien's normal destination leg and one-second stranded retry restore progress toward the Research Center; **do not store a second pending-core destination**. Exercise this in Rust with seeded/refreshed real `VisibilityMap` as well as Godot.

**Wall-chewing is intentional.** `AttackMove` attacks the nearest currently visible enemy (including Walls and Gates) within its acquisition radius, so after a breach aliens may attack *other* nearby defences before proceeding to the Research Center. Do not add an exclusion rule for walls. Acceptance requires continued meaningful attacks on the core **or another structure** after the breach, not an immediate straight run through the gap. The existing `waves.rs::retry_stranded_aliens` one-second cadence is an **upper bound**, not sufficient suppression by itself: an alien without an accepted order would otherwise scan all structures and retry A* every second.

When a fallback probe establishes that **no player structure has a reachable approach slot on the current static map**, remember that `GridMap::revision()` **on that alien** (a minimal per-alien failed-revision marker, following `MoveOrder.last_failed_replan`; no global scheduler or path cache). At the same unchanged revision, skip further expensive fallback/core reachability probes for that alien **even across subsequent retry seconds**. Invalidate the marker when the map revision changes; clear it on successful objective assignment or reset/despawn. Do **not** cache transient command rejections caused by unit reservations/crowding as structural failures: those can still retry on the existing cadence. Reattempt the Research Center after wall/gate destruction changes the map revision. **Never scan every structure and run A* on every tick or every retry second at an unchanged failed revision.** A focused 150-alien sealed-out regression must show that `GridMap::path_call_count()` stays flat over at least three retry seconds at fixed revision (after the initial failure scan), without unrelated wave launches. Respect fog and the single visibility predicate for combat/bridge presentation; do not reveal hidden entities to the player.

## 4. Fast placement and builder handoff

For Wall and Gate **only**, successful left-click placement stays armed. Ordinary building placement remains one-shot. Right-click while placing cancels placement **without issuing a world command**; Esc also cancels before the normal pause behavior. Failed preview or rejected placement does not spend resources and leaves the placement mode armed. User-facing feedback should distinguish queued/accepted from rejected commands; server-side validation remains authoritative.

When a Worker finishes a site, `step_construction` may reuse `apply_resume_construction` to move that same Worker to the nearest **unfinished, unassigned, friendly** site that has a reachable builder slot. Sort candidate sites by worker-to-footprint distance, then BuildingId, and try each at most once for that handoff. If no reachable site remains, Worker becomes Idle. A newly placed site still reassigns that Worker immediately as it does today; previous unfinished sites are picked up in follow-on handoffs. Make the minimal signature change needed to pass `GridMap` to the completion step. Never steal an actively owned builder, double construction rate or introduce a persisted queue.

## 5. Presentation and constraints

- Add Wall, Gate, and Gun Turret to the existing build-button/catalogue, placement preview and selection/health feedback, with simple visually distinguishable primitive meshes. Tower tracers originate at the turret view, with the existing hit/death FX.
- Leave original human and alien placeholder models, winter terrain art, icons and new SFX to existing separate HPA-291/HPA-290/HPA-82/HPA-44/HPA-269 tickets.
- Keep wave HUD, minimap warnings, Crystal economy, fog-gated picking, core Defeat, pause/restart and benchmark fixture working. New defences must not appear by default in the 200-unit benchmark unless explicitly authored as part of a workload change.
- No Flame Tower / Crystal Lance (HPA-188), Coal Furnace/efficiency (HPA-181), underground work (HPA-288), new map mode, replay/save system or backwards-compatibility layer.

## Acceptance scenarios

1. A Worker lays multiple Walls and Gates without rearming; right-click/Esc cancels **without issuing a world order or pausing**. A focused Godot smoke assertion covers this interaction as soon as the control is added (stage 5), in addition to the integrated defense smoke. Cost, footprint, blocked preview, health, construction completion, stop/resume and Restart behave like current buildings.
2. A friendly Worker receives a real route through a completed Gate and passes it, including replanning while already standing inside the Gate. An alien cannot traverse or start a route on that Gate, and neither team may use it as a path goal. Adding/removing the Gate changes revision and forces appropriate route replanning.
3. A completed Gun Turret engages a *visible* Swarmer/Spitter in range, uses cooldown and Spitter bonus, renders a tracer and awards kill Crystals. An unfinished/hidden/out-of-range turret target is never shot.
4. When a perimeter seals the center, aliens `AttackMove` to a reachable Wall/Gate approach slot and destroy defences under **real fog**; after a breach they continue attacking the Research Center **or another reachable/visible structure**, including when the barrier was placed mid-route. The smoke must not require a straight-line sprint to the core through a gap. If 150 aliens cannot reach **any** structure, the failed-revision guard prevents repeated A* across three or more retry seconds until the map changes.
5. A Worker finishing a newly placed wall automatically resumes the nearest reachable earlier unfinished site without taking an already assigned construction job. It idles when none remain.
6. Start/Pause/Result do not progress tower or construction behavior; Restart re-seeds the baseline one-core fixture. Rust tests, 90% simulation line coverage, existing Godot smokes and exported-game E2E remain green.

## Risk controls for implementation review

The biggest regression risk is accidentally using team-aware Gate traversal for placement/spawn checks or forgetting a movement/replanning caller. Audit all call sites and test both teams on the same cells. The second risk is a repeated A* scan with 150 aliens: keep the sealed-out fixed-revision path-call regression **and measure path calls per revision bump while a wall ring is attacked/destroyed**; record the observed delta without inventing a performance pass threshold or preemptively adding a flood-fill cache. The third is combat-event identity/visibility leakage: test friendly turret tracers, hidden enemy attacker scrubbing and existing unit events. Run fallback tests with a real refreshed visibility map, not only permissive fog-free sim fixtures.

All implementation, test and documentation commits stay on this draft PR until accepted; this document records proposed rules, not implemented behavior.
