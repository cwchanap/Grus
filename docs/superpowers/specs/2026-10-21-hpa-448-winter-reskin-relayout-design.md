# HPA-448 — Winter re-skin and central-city layout: design decisions

**Linear:** https://linear.app/cwchanap/issue/HPA-448/grus-survival-28-winter-re-skin-and-city-re-layout
**Parent contract:** ../specs/2026-10-21-winter-survival-design.md
**Audited main:** `5531919024bb5822dfa991d29f32c0daa3a4819b` (HPA-445 PR #8 merged).
**Status:** proposed design; documentation only. Implementation belongs in this same draft PR, not a follow-up PR.

## Product slice

Turn the existing passive-opponent skirmish into the _pre-wave_ winter survival baseline. This ticket renames/removes the old economy, recasts unit/building production, and recenters the player's base with four accessible entry edges for HPA-449. No alien behavior yet; until HPA-449, the authored Team-2 base must remain east and passive so attack/Victory/Restart coverage survives. Player can still gather, build, train, attack, use fog/minimap, and win by destroying that passive Research Center.

This is a **code/data/UI re-skin with existing primitive meshes**, not the winter art pack (HPA-290/291/82/44), tower/heat systems (HPA-450/181), research (HPA-188), or audio (HPA-269). Do not introduce a second faction, dynamic map generator, new UI/state framework, or compatibility aliases for removed names.

## Catalogue and authoritative state

| Old | HPA-448 name / fate | Contract |
| --- | --- | --- |
| Food | Food | Gatherable ration caches; 200 starting |
| Wood | Scrap | Gatherable salvage ruins; 300 starting |
| Gold | Coal | Gatherable coal seams; 150 starting |
| — | Crystals | Fourth **stockpile and cost field**; starts at 0, **no gatherable world source or income** until HPA-449 |
| Villager | Worker | 50 Food, noncombatant, Research Center trains |
| Spearman | Guard | 60 Food, combatant, Barracks trains |
| Archer | Rifleman | 40 Food + 40 Scrap, ranged combatant, Barracks trains |
| Cavalry | remove | No replacement cavalry producer/command |
| TownCenter | ResearchCenter | Seeded 4×4, dropoff, 10 population, trains Workers, 800 HP; cannot be player-placed |
| House | Bunkhouse | 50 Scrap, +10 population, current 2×2 and health/time unless changed by ticket |
| Storehouse | Depot | 75 Scrap, generic dropoff, current 2×2 and health/time |
| Barracks | Barracks | 120 Scrap, current 3×3 and health/time, produces Guard **and** Rifleman |
| Farm/ArcheryRange/Stable | remove | No renewable Farm source, secondary producer, or cavalry building |
| Age 1/2, Age-up | remove | No `Age`/`required_age` state or Age-2 queue command/HUD; one baseline `BASE_GATHER_RATE` |

`catalog.rs` remains the source of balance data. Update `ResourceStockpile` and `Cost` to explicit `food/scrap/coal/crystals` fields. Preserve them as distinct semantic types and centralize spending at `ResourceStockpile::can_afford(Cost)` (read-only placement validation) and `ResourceStockpile::try_spend(Cost) -> bool` (atomic all-or-nothing deduction). Both building placement and unit enqueue use the same helper; later Crystal Analyze, excavation, rooms and Crystal Lance costs can reuse it. The Godot bridge should reuse a small `cost_dict(Cost)` helper for unit/building cost snapshots instead of duplicating four field writes; economy stocks remain a separate snapshot. Crystals are stockpiled only this ticket: keep three gatherable source kinds; **do not** fabricate a Crystal node or a special crystal gather route merely to model future drops. HPA-449 can credit stockpile directly on kills.

Delete age-only `TeamState`/`ProductionKind` properties and commands. The unit catalogue has exactly Worker/Guard/Rifleman; the building catalogue exactly ResearchCenter/Bunkhouse/Depot/Barracks. Keep combat health, speeds, attack range/cooldown and ordinary damage behavior unless required to remove old age/counter mechanics. Guard/Rifleman **counter bonuses are exactly zero** now; the alien-specific counters wait for HPA-449. Do not preserve the old Spearman→Cavalry→Archer triangle as a hidden mechanic.

Because `CombatSpec.counter_target` is a concrete `UnitKind` today, keeping it with `counter_bonus = 0` is acceptable for the one-ticket interim state; do not create alien placeholder types or a generic counter framework. HPA-449 can assign actual aliens once their unit kinds exist.

## World layout and passive opponent

- Preserve the **128×96** GridMap and fixed-angle 2.5D Godot camera. One authored Team-1 Research Center moves near the map center, with its four initial Workers adjacent and navigable. A candidate anchor is **(62,46)** for its 4×4 footprint; confirm this against all chosen resource/obstacle/worker footprints during implementation. Keep Team 2's **passive** starting Research Center, four Workers, and stockpile east (currently anchor (112,46)); no opponent AI.
- Remove every old center PvP lane blocker: grid rectangles (54..61,20..75), (66..73,20..75), (62..65,38..57), and their **matching visible** West/East/CenterBarrier nodes and old BoxMesh subresources. A small single **`RUINS` authored rectangle table in `fixture.rs`** is the one source for `GridMap::set_blocked_rect` and the visible simple ruin meshes: expose the table through one narrow read-only bridge `obstacle_rects()` method and instantiate primitive `MeshInstance3D` rectangles in the Godot battlefield from that payload (inclusive grid bounds converted to scene center/size). No second handwritten coordinate list, new ECS gameplay entities, procgen, or collision representation. Smoke coverage verifies rendered rectangles match the bridge output and a blocked-ground click targets an actual ruin; leave a clear central construction/gather ring and access from all four map edges.
- Preserve **18 deterministic finite resource IDs**, grouped into authored Team-1 safe nodes (1–6), Team-2 east-side safe nodes (7–12), and expansion nodes (13–18); relabel/reposition as ration caches, scrap ruins, and coal seams with positive finite amounts. No overlaps with buildings, workers, obstacles, or one another; keep safe dropoff approaches walkable. Update `resource_amount` in the catalogue for each of the three gatherable types. Retain Godot resource-view IDs/visibility semantics; visuals may be simple recolored primitives for now.
- **Do not classify safe sources by the half-map test `x < 64` anymore.** Team 1 is central and its nearby sources may straddle the map center. Make `TeamStart` the one per-team authored table, carrying `barracks_anchor`, `depot_anchor` and `safe_sources: [ResourceId; 6]` alongside its existing Research Center/Worker start data. `safe_source_ids(team)` and placement-test helpers read this table; delete the old `mirror_anchor` function, whose horizontal-mirror logic would place Team-2's Barracks in the central city after relocation. Keep deterministic source IDs and validate membership against the 18 resource entries.
- Retain public `MapFixture::barracks_anchor(team)` and `MapFixture::safe_storehouse_anchor(team)` for non-AI integration/session tests, but implement each as a **lookup of the authored TeamStart fields** (the Storehouse-named helper can be renamed to `depot_anchor(team)` if all its consumers are migrated together). Delete `left_spawn`/`right_spawn` in `MapFixture`, which duplicate the old starting positions; derive any route endpoints from `team_starts()` or its Workers. Tests should validate actual reachability/placement and clear N/E/S/W border-to-city paths (not merely in-bounds coordinates).
- The initial camera in `godot/scenes/main.tscn` must frame the central city **before input**. In the headless `smoke_test.gd`, after the Team-1 Research Center building view is initialized, project its live center through `Main/Camera3D` and require it in the **central half** of the viewport *before pan/zoom*. Keep a separate Rust fixture assertion that the Research Center footprint is near the map midpoint: the Godot check should use the actual view (not a copied `team_starts()` literal) to catch camera/layout drift. Fog/minimap must still hide the eastern passive opponent until scouted. Preserve unit/building/resource stable IDs and deterministic spawn insertion (8 units, 2 Research Centers, 18 resource nodes at fresh boot/reset).
- The passive enemy must remain **reachable by attack-move**, and its Research Center destruction must still yield Victory; this is an intentional temporary state until waves arrive in HPA-449.

## Current public bridge and Godot UI

- Public Rust bridge commands use the **new enum names** for `enqueue_unit` and `place_building`. Remove `enqueue_age_up` entirely, not as a no-op. `catalogue_snapshot` exposes typed costs for all **four** resources and producer names, with **no** `age` or `age_up` keys. `economy_snapshot` exposes four stocks, population, constant gather rate, idle Worker list, and reject feedback; no age.
- Preserve `building_snapshot` queues, research-free production progress, visibility privacy, and `RejectReason` numeric feedback. Remove obsolete `FarmOccupied` and **rename `NotVillager` to `NotWorker`** (same variant position). Recalculate numeric GDScript reject codes and pin at least `Locked = 7` and `SessionLocked = 17` after deletion in one Rust test next to the current enum-order test. Unknown/removed bridge inputs reject cleanly. Parse unit/building names by matching `kind_name(kind)` over the existing `UnitKind::ALL` / `BuildingKind::ALL`, rather than a second handwritten enum-to-string mapping; keep seeded ResearchCenter's existing `Locked` placement rejection.
- `main.tscn`: retire Age/Cavalry/Farm/Range/Stable buttons, keep build Bunkhouse/Depot/Barracks and train Worker/Guard/Rifleman; update metadata keys to match Rust parser. `battlefield_controller.gd`: delete age wiring, relabel four currencies/Worker affordances, maintain contextual gather/build/attack commands, selection, pause/restart, camera, input shielding, control groups, idle-worker navigation.
- `unit_view.gd` and `resource_view.gd` use placeholder role silhouettes and distinct colors/markers for the new kinds; do not create art assets. `building_view.gd`/`minimap.gd` continue using runtime metadata from the bridge rather than hardcoded faction-specific economics.
- The new resource and producer names must also flow into the bridge metadata, snapshots, queue labels, selection labels, help text, and smoke scripts. No deprecated display aliases or old resource keys remain on live API paths.

## Scripted benchmark through HPA-449

The existing session-free 200-unit benchmark **must run under the new catalogue and asymmetric map**. Retain 100 units per team: four authored Workers plus **96 extra per team**, with the roster **16 Workers + 40 Guards + 40 Riflemen** (100 total per side). Reuse the current `benchmark_staging_cells` row-major west scan and point-reflected east candidate positions **only if** explicit checks under the **new** map prove all 96 cells per side are free/unique, do not overlap ruined blockers, and every side can issue accepted gather/attack-move orders. This is a *local staging layout invariant*, **not** a requirement that the whole city map be mirrored. Prefer retaining the existing `(64,12)` attack-move waypoint while it remains reachable and the actual 10-second benchmark demonstrates movement/combat liveness; revise only failing positions instead of rewriting throwaway benchmark choreography. Derive resource assignment from TeamStart's explicit six safe-source IDs, **never** `x < 64`; delete the now-redundant `assert_non_mirrored_seed` (the only seed path already uses `team_starts()`). Benchmark stays session-free to avoid result freeze. HPA-449 replaces this temporary workload with aliens-versus-defenders.

`benchmark_200.gd` keeps the exact 200-unit census, two-building check, movement/combat liveness, and current visibility/frame measurements. 10-second CI and 60-second reference hardware runs are comparable **only within the same workload**; record the revised HPA-448 roster/layout beside any fresh numbers. HPA-449 explicitly replaces this temporary two-sided benchmark with scripted alien attacks—**do not retain a PvP-only fixture just to keep benchmark running**.

## Acceptance and non-goals

One HPA-448 PR with **three independently green checkpoints** (delete obsolete content, rename catalogue/wire/UI, then re-layout geometry): fmt, no-warning Clippy, workspace and bridge/e2e tests, 90% sim line coverage, all four remaining headless Godot smokes, exported-build boot and the updated scripted benchmark at each checkpoint. Add targeted sim tests that exercise each gatherable resource, four-stock affordability and atomic deduction, both Barracks units, Bunkhouse population, Depot dropoff, zero counter bonus, occupied/route revisions, city/edge connectivity, Team-2 passive Victory and fresh restart. Preserve meaningful UI smoke proofs, rewriting old Farm/Age/Cavalry portions as winter economy/production tests rather than deleting the smoke.

Later tickets own alien waves, path-aware walls/gates/towers, heat, research, underground levels, tuning and final presentation assets. No new game mode, persistent saves, procedural map, migration/backwards compatibility, or premature future catalogue definitions.
