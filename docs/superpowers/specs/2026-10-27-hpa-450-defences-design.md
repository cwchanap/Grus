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

Add `Wall`, `Gate`, `GunTurret` to `BuildingKind::ALL`. All three use ordinary building placement, construction, damage, destruction, selection and `BuildingIndex`. Walls and Gates are 1×1 (10 Scrap / 400 HP and 30 Scrap / 600 HP), never produce units, and have zero heat demand. The Gun Turret is 2×2, costs 100 Scrap + 20 Coal, fires at range 8, and has a Spitter counter bonus. Record the future heat demand of 2 in the parent contract; the **heat simulation is HPA-181**, not this PR.

Extend `BuildingSpec` with `combat: Option<CombatSpec>`; non-towers use `None`. Reuse the existing single-counter `CombatSpec` instead of adding a damage table. **Proposed, tunable defaults only** for unspecified values: Wall 5s build, Gate 8s build, Gun Turret 25s build / 350 HP / 10 damage every 1.0s / +5 against Spitter. Put those numbers in the typed catalogue; finalize through review and defer broader balancing to HPA-289.

A *completed* Gun Turret has `AttackCooldown` and automatically selects a currently visible opposing alien inside actual attack range. Use the existing shared per-step acquisition snapshot and predicate (range checked before fog lookup); choose the nearest eligible target, then lowest stable unit ID for ties. No tower `CombatOrder` or movement. Incomplete sites cannot fire, including zero-progress sites spawned at full health. Towers use the exact same damage, cooldown, hit event and target-destruction path as units; a tower kill awards Crystals through the existing authoritative kill seam and can never create duplicate awards.

Generalize the event **attacker identity** to a narrow enum, e.g. `CombatAttacker::{Unit(UnitId), Building(BuildingId)}`, distinct from the existing target enum. Let `strike` accept an attacker entity, team and combat spec; avoid duplicating damage, kill, health and cooldown handling. Bridge serialization includes attacker *kind* and stable ID (or scrubs identity for an unseen/dead enemy), preserving the current friendly/visible-only presentation rule; the controller resolves a unit or building view to spawn an existing ranged tracer. No projectile simulation or dedicated sound assets.

## 2. One GridMap with team-aware Gate traversal

Preserve `GridMap::is_walkable(cell)` as **free/unoccupied ground** for placement previews, spawn slots, resource/approach destinations and other non-mover checks. It must be false for Walls *and* Gates so buildings and spawns never overlap them.

Inside that same `GridMap`, represent free / ordinary blocked / gate cells (e.g. a small `gates` set alongside the existing blocked set; never a second navigational grid). Add one explicit mover-aware predicate:
- Team 1 may traverse a Gate cell, but no team may build/spawn on it.
- Team 2 may not traverse a Gate cell.
- Both teams are blocked by Walls, terrain, all other building footprints and out-of-bounds cells.

Pass `TeamId` only along the existing **mover-facing paths**: A* route construction (`find_path` and its callers), route-replan/recovery, and per-tick movement candidate plus separation checks. Audit Move, AttackMove, pursue, worker gather/dropoff/construction, wave spawn route assignment and route recovery. Keep placement, resource positions, spawn-slot generation and non-movement checks on the existing team-agnostic free-cell predicate. No compatibility overloads: update signatures directly and compile-error-audit callers.

Wall placement/destruction uses existing blocking; Gate placement/destruction sets/removes a gate cell and increments **the existing map revision** on every actual occupancy change. Avoid double revision increments for one cell update. Existing `MoveOrder.map_revision` triggers route refresh; a human can enter/exit a gate, while a moving alien must replan or attack it. Construction preview uses a cloned, *post-placement* grid for builder reachability, as today. The Gate is traversable by the human builder when built, but not a valid building-placement or destination slot.

## 3. Alien fallback for sealed routes

Do not change HPA-449's deterministic spawning, timing or budget. Only when an alien's Research Center route cannot be completed (including an invalidated/preserved route after wall/gate revision), pick the **nearest reachable Team-1 structure**, including Wall, Gate, unfinished construction sites and other buildings. Deterministic ranking: geometric distance to the target footprint, then stable BuildingId; verify reachability to an outside immediate-perimeter approach slot via existing `approach_slots` and team-aware A*. Prefer an actually reachable alternative over an unrouteable, geometrically closer one.

Reuse existing `CombatOrder::Attack` / `CombatTarget::Building` and pursuit/strike/destruction rather than changing the wave director into a second AI. Existing local acquisition still picks up targets en route. Only run fallback upon actual route failure / relevant map revision or on the wave retry cadence; **never scan every structure and run A* every tick for each alien**. If no structure is reachable, retain retryable intent until the grid changes. After the gate/wall dies, reattempt the Research Center route so the wave continues advancing. Respect fog and the single visibility predicate for combat/bridge presentation; do not reveal hidden entities to the player.

## 4. Fast placement and builder handoff

For Wall and Gate **only**, successful left-click placement stays armed. Ordinary building placement remains one-shot. Right-click while placing cancels placement **without issuing a world command**; Esc also cancels before the normal pause behavior. Failed preview or rejected placement does not spend resources and leaves the placement mode armed. User-facing feedback should distinguish queued/accepted from rejected commands; server-side validation remains authoritative.

When a Worker finishes a site, `step_construction` may reuse `apply_resume_construction` to move that same Worker to the nearest **unfinished, unassigned, friendly** site that has a reachable builder slot. Sort candidate sites by worker-to-footprint distance, then BuildingId, and try each at most once for that handoff. If no reachable site remains, Worker becomes Idle. A newly placed site still reassigns that Worker immediately as it does today; previous unfinished sites are picked up in follow-on handoffs. Make the minimal signature change needed to pass `GridMap` to the completion step. Never steal an actively owned builder, double construction rate or introduce a persisted queue.

## 5. Presentation and constraints

- Add Wall, Gate, and Gun Turret to the existing build-button/catalogue, placement preview and selection/health feedback, with simple visually distinguishable primitive meshes. Tower tracers originate at the turret view, with the existing hit/death FX.
- Leave original human and alien placeholder models, winter terrain art, icons and new SFX to existing separate HPA-291/HPA-290/HPA-82/HPA-44/HPA-269 tickets.
- Keep wave HUD, minimap warnings, Crystal economy, fog-gated picking, core Defeat, pause/restart and benchmark fixture working. New defences must not appear by default in the 200-unit benchmark unless explicitly authored as part of a workload change.
- No Flame Tower / Crystal Lance (HPA-188), Coal Furnace/efficiency (HPA-181), underground work (HPA-288), new map mode, replay/save system or backwards-compatibility layer.

## Acceptance scenarios

1. A Worker lays multiple Walls and Gates without rearming; right-click/Esc cancels. Cost, footprint, blocked preview, health, construction completion, stop/resume and Restart behave like current buildings.
2. A friendly Worker receives a real route through a completed Gate and passes it. An alien given the same destination cannot pass through that Gate. Adding/removing the Gate changes revision and forces appropriate route replanning.
3. A completed Gun Turret engages a *visible* Swarmer/Spitter in range, uses cooldown and Spitter bonus, renders a tracer and awards kill Crystals. An unfinished/hidden/out-of-range turret target is never shot.
4. When a perimeter seals the center, aliens choose a reachable Wall/Gate and destroy it; once opened, surviving aliens advance toward the Research Center, including after a mid-route gate/wall placement.
5. A Worker finishing a newly placed wall automatically resumes the nearest reachable earlier unfinished site without taking an already assigned construction job. It idles when none remain.
6. Start/Pause/Result do not progress tower or construction behavior; Restart re-seeds the baseline one-core fixture. Rust tests, 90% simulation line coverage, existing Godot smokes and exported-game E2E remain green.

## Risk controls for implementation review

The biggest regression risk is accidentally using team-aware gate traversal for placement/spawn checks, or forgetting one movement/replanning caller. Audit all call sites and test both teams on the same cells. The second risk is a repeated A* scan under a 150-alien load: keep path-call-count regressions around fallback and benchmark liveness. The third is combat event identity or visibility leakage: test friendly tower tracers, hidden enemy scrubbing and existing unit events without changing their meaning.

All implementation, test and documentation commits stay on this draft PR until accepted; this document records proposed rules, not implemented behavior.
