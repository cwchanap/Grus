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

`catalog.rs` remains the source of balance data. Update `ResourceStockpile` and `Cost` to explicit `food/scrap/coal/crystals` fields; all affordability, spending, depositing, command snapshot, and tests must use these names consistently. Crystals are stockpiled only this ticket: keep three gatherable source kinds; **do not** fabricate a Crystal node or a special crystal gather route merely to model future drops. HPA-449 can credit stockpile directly on kills.

Delete age-only `TeamState`/`ProductionKind` properties and commands. The unit catalogue has exactly Worker/Guard/Rifleman; the building catalogue exactly ResearchCenter/Bunkhouse/Depot/Barracks. Keep combat health, speeds, attack range/cooldown and ordinary damage behavior unless required to remove old age/counter mechanics. Guard/Rifleman **counter bonuses are exactly zero** now; the alien-specific counters wait for HPA-449. Do not preserve the old Spearman→Cavalry→Archer triangle as a hidden mechanic.

Because `CombatSpec.counter_target` is a concrete `UnitKind` today, keeping it with `counter_bonus = 0` is acceptable for the one-ticket interim state; do not create alien placeholder types or a generic counter framework. HPA-449 can assign actual aliens once their unit kinds exist.

## World layout and passive opponent

- Preserve the **128×96** GridMap and fixed-angle 2.5D Godot camera. One authored Team-1 Research Center moves near the map center, with its four initial Workers adjacent and navigable. A candidate anchor is **(62,46)** for its 4×4 footprint; confirm this against all chosen resource/obstacle/worker footprints during implementation. Keep Team 2's **passive** starting Research Center, four Workers, and stockpile east (currently anchor (112,46)); no opponent AI.
- Remove every old center PvP lane blocker: grid rectangles (54..61,20..75), (66..73,20..75), (62..65,38..57); delete their **matching visible** West/East/CenterBarrier meshes. Replace with a *small explicitly authored* set of scattered ruined-blockage rectangles, mirrored by Godot placeholder obstacles from the **same coordinate plan**, leaving a clear center construction/gather ring and access from all four map edges. No random generation.
- Preserve **18 deterministic finite resource IDs**, grouped into authored Team-1 safe nodes (1–6), Team-2 east-side safe nodes (7–12), and expansion nodes (13–18); relabel/reposition as ration caches, scrap ruins, and coal seams with positive finite amounts. No overlaps with buildings, workers, obstacles, or one another; keep safe dropoff approaches walkable. Update `resource_amount` in the catalogue for each of the three gatherable types. Retain Godot resource-view IDs/visibility semantics; visuals may be simple recolored primitives for now.
- **Do not classify safe sources by the half-map test `x < 64` anymore.** Team 1 is central and its nearby sources may straddle the map center. Give `safe_source_ids(team)` explicit authored membership (or derive it from a single typed authored table), never from mirrored geography; Team-2 grouping also stays explicit.
- The existing fixture helpers `barracks_anchor(team)` and `safe_storehouse_anchor(team)` serve non-AI integration/session tests; retarget them to collision-free Guard/Barracks and Depot sites. Do not prune them as obsolete AI. `team_starts` remains one truth for core anchors. Tests should validate actual reachability/placement and clear N/E/S/W border-to-city paths (not merely in-bounds coordinates).
- The initial camera in `godot/scenes/main.tscn` must frame the central city **before input**. In the headless `smoke_test.gd`, after the Team-1 Research Center building view is initialized, project its live center through `Main/Camera3D` and require it in the **central half** of the viewport *before pan/zoom*. Keep a separate Rust fixture assertion that the Research Center footprint is near the map midpoint: the Godot check should use the actual view (not a copied `team_starts()` literal) to catch camera/layout drift. Fog/minimap must still hide the eastern passive opponent until scouted. Preserve unit/building/resource stable IDs and deterministic spawn insertion (8 units, 2 Research Centers, 18 resource nodes at fresh boot/reset).
- The passive enemy must remain **reachable by attack-move**, and its Research Center destruction must still yield Victory; this is an intentional temporary state until waves arrive in HPA-449.

## Current public bridge and Godot UI

- Public Rust bridge commands use the **new enum names** for `enqueue_unit` and `place_building`. Remove `enqueue_age_up` entirely, not as a no-op. `catalogue_snapshot` exposes typed costs for all **four** resources and producer names, with **no** `age` or `age_up` keys. `economy_snapshot` exposes four stocks, population, constant gather rate, idle Worker list, and reject feedback; no age.
- Preserve `building_snapshot` queues, research-free production progress, visibility privacy, and `RejectReason` numeric feedback. Removing the obsolete FarmOccupied reject (and optionally renaming NotVillager→NotWorker) changes enum integer discriminants: synchronize **all** GDScript smoke numeric constants with the final Rust enum, never assume the old codes. Unknown/removed bridge inputs reject cleanly.
- `main.tscn`: retire Age/Cavalry/Farm/Range/Stable buttons, keep build Bunkhouse/Depot/Barracks and train Worker/Guard/Rifleman; update metadata keys to match Rust parser. `battlefield_controller.gd`: delete age wiring, relabel four currencies/Worker affordances, maintain contextual gather/build/attack commands, selection, pause/restart, camera, input shielding, control groups, idle-worker navigation.
- `unit_view.gd` and `resource_view.gd` use placeholder role silhouettes and distinct colors/markers for the new kinds; do not create art assets. `building_view.gd`/`minimap.gd` continue using runtime metadata from the bridge rather than hardcoded faction-specific economics.
- The new resource and producer names must also flow into the bridge metadata, snapshots, queue labels, selection labels, help text, and smoke scripts. No deprecated display aliases or old resource keys remain on live API paths.

## Scripted benchmark through HPA-449

The existing session-free 200-unit benchmark **must run under the new catalogue and asymmetric map**. Retain 100 units per team: four authored Workers plus **96 extra per team**, with the roster **16 Workers + 40 Guards + 40 Riflemen** (100 total per side). This mechanically replaces Cavalry with retained military types and keeps aggregate load/metric names; staging cells and shared engagement target must be newly authored walkable positions, **not** point-reflected from a no-longer-symmetric map. Keep 12 worker gather orders per team to its authored safe source IDs, and 80 fighter attack-move orders per side to reachable contested terrain. Setup must reject neither order nor spawn; benchmark stays session-free to avoid result freeze.

`benchmark_200.gd` keeps the exact 200-unit census, two-building check, movement/combat liveness, and current visibility/frame measurements. 10-second CI and 60-second reference hardware runs are comparable **only within the same workload**; record the revised HPA-448 roster/layout beside any fresh numbers. HPA-449 explicitly replaces this temporary two-sided benchmark with scripted alien attacks—**do not retain a PvP-only fixture just to keep benchmark running**.

## Acceptance and non-goals

One Rust+Godot implementation cut for HPA-448: fmt, no-warning Clippy, workspace and bridge/e2e tests, 90% sim line coverage, all four remaining headless Godot smokes, exported-build boot and the updated scripted benchmark. Add targeted sim tests that exercise each gatherable resource, four-stock affordability and atomic deduction, both Barracks units, Bunkhouse population, Depot dropoff, zero counter bonus, occupied/route revisions, city/edge connectivity, Team-2 passive Victory and fresh restart. Preserve meaningful UI smoke proofs, rewriting old Farm/Age/Cavalry portions as winter economy/production tests rather than deleting the smoke.

Later tickets own alien waves, path-aware walls/gates/towers, heat, research, underground levels, tuning and final presentation assets. No new game mode, persistent saves, procedural map, migration/backwards compatibility, or premature future catalogue definitions.
