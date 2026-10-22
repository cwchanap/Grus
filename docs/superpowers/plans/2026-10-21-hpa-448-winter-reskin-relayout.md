# HPA-448 — Winter re-skin and re-layout: implementation plan

**Issue:** https://linear.app/cwchanap/issue/HPA-448/grus-survival-28-winter-re-skin-and-city-re-layout
**Scoped spec:** ../specs/2026-10-21-hpa-448-winter-reskin-relayout-design.md
**Project-wide spec:** ../specs/2026-10-21-winter-survival-design.md
**Verified base:** main `5531919024bb5822dfa991d29f32c0daa3a4819b` (merged HPA-445). No Grus PR was open when this branch was created.
**Status:** planning document. **No implementation or test run is claimed.**
**PR rule:** Exactly **one PR for HPA-448**; the draft containing this plan becomes the implementation PR. Do not create a second implementation PR.

## Why this is the next task

HPA-445 has landed and Linear marks it Done. HPA-448 was Todo and blocked only by HPA-445, so it is the first unlocked coding issue in the eight-task Winter Survival chain; HPA-449 is blocked by HPA-448. Separate asset tasks should not be mixed into this code PR.

## Audited current-code facts and risks

1. `crates/grus-sim/src/catalog.rs` has exactly Food/Wood/Gold, Villager/Spearman/Archer/Cavalry, seven buildings, `Age`, `required_age`, and Age-2 constants. `CombatSpec` has an obligatory `counter_target` and nonzero PvP bonuses. `UnitKind::ALL` and `BuildingKind::ALL` drive bridge catalogue construction.
2. `crates/grus-sim/src/economy.rs` contains `ResourceStockpile` (3 fields), `TeamState.age`/`age_up_started`, `gather_rate_for_age`, Farm ownership/reservations, `FarmOccupied` rejection, worker dispatch and carries, deposit matching on `ResourceKind`. Every cashflow path must switch together.
3. `crates/grus-sim/src/production.rs` has `ProductionKind::Age2`, `apply_enqueue_age_up`, age-locked producer validation and per-producer match arms; the current Barracks makes only Spearmen and Archer training uses Archery Range. `buildings.rs` validates villager-only builders, age locks, and gives completed Farm a renewable resource. `commands.rs` exposes `EnqueueAgeUp`.
4. `crates/grus-sim/src/fixture.rs` owns `team_starts()`, `starting_resources()`, `expansion_resources()`, blocked PvP rectangles, two Barracks/Depot anchor helpers, and the original 8/2/18 seeded census. The exact center blockers are x=54–61/y=20–75, x=66–73/y=20–75 and x=62–65/y=38–57; their 3 visible Godot barrier meshes are in `godot/scenes/battlefield.tscn`.
5. Benchmark `BENCHMARK_ROSTER` is 16 Villagers + 30 Spearmen + 30 Archers + 20 Cavalry (96 extras **per team**); `safe_source_ids` uses `x < 64`; Team-2 staging is `point_reflect` of Team-1 cells, which fails after asymmetric central-city re-layout. `assert_non_mirrored_seed` verifies authored base anchors; retain a renamed authored-start assertion.
6. `crates/grus-godot/src/lib.rs` owns bridge parsing, economy/building/catalogue snapshots, numeric reject feedback, metadata and fixed-step ordering; it still exports `enqueue_age_up`, `age_number` and `age_up` keys. Preserve bridge ownership, typed truth, interpolation, visibility and command→combat→movement→economy→construction→production→visibility→route-feedback order.
7. `godot/scenes/main.tscn` has Age/Farm/ArcheryRange/Stable/Cavalry buttons, old metadata; `battlefield_controller.gd` has age state, `_selected_villager_ids`, and Farm-as-resource picking; `unit_view.gd`/`resource_view.gd` style old string names.
8. The Godot economy smoke currently asserts Age 2, FarmOccupied, cavalry, 3-resource solvency and old static positions. The combat smoke asserts four role kinds, Spearman training and old enemy coordinates. Reset smoke asserts 200/300/100; benchmark smoke asserts exactly 200 units/2 buildings. Rust system-order's Age-2 tick-arithmetic test is obsolete, **but its separate Storehouse-destruction/economy-route and visibility/order coverage is not**. `fixture/tests.rs`, `catalog/tests.rs` and module tests encode old roster, old resource/age fields, and old placement anchors.
9. `AGENTS.md` is a symlink to `CLAUDE.md`; only edit the target. Current README documents the old economy and commands. CI runs fmt/Clippy, >=90% grus-sim coverage, Rust tests, four Godot smoke scenes, exported boot, and the session-free 200-unit benchmark.
10. Runtime does **not** need a new asset loader or generated art. Placeholder view meshes are enough; real winter terrain/model/icons/audio are separate tickets.

## Implementation strategy — one coordinated cut

The steps below are review/ownership boundaries, **not independently runnable commits**. Changing enum variants will intentionally break Rust/bridge/smokes until all dependents are migrated. Land the coordinated conversion in this **same PR**, with focused commits if convenient; do not weaken CI or leave dead API shims to obtain intermediate green status.

### A. Replace the catalogue/stockpile as one contract

**Files:** `catalog.rs`, `catalog/tests.rs`, `economy.rs`, `production.rs`, `commands.rs`, `buildings.rs`, `lib.rs` and affected tests.

- Change `ResourceKind` to the three harvestable Food/Scrap/Coal kinds; define explicit `Cost {food,scrap,coal,crystals}` and `ResourceStockpile` same fields. Crystals are currently a zero-start, non-harvestable currency; no Crystal `ResourceSpawn` and no Crystal gather job.
- Replace `UnitKind` with Worker/Guard/Rifleman; replace `BuildingKind` with ResearchCenter/Bunkhouse/Depot/Barracks. No Cavalry/Farm/Stable/ArcheryRange variants, no old-name aliases. Update `ALL`, `unit_spec`/`building_spec`, resource amounts, health/population/producer metadata per spec. Barracks produces **both** Guard and Rifleman.
- Remove `Age`, `required_age`, `AGE_TWO_*`, `ProductionKind::Age2`, `apply_enqueue_age_up`, `PlayerCommand::EnqueueAgeUp`, the TeamState age/age-up flag, and the bridge age control. Use `BASE_GATHER_RATE` independent of a team age. Keep working FIFO queues, pop limits, spawn clearance and rally points.
- Refactor all cost affordability/deduction sites (building placement and training queues) to check **every** Food/Scrap/Coal/Crystals field atomically; no partial charges on rejection. Update carry/deposit type match arms for three gathered kinds; preserve worker route/replan and same-team dropoff mechanics.
- Delete only Farm-specific workflows (renewable on-completion source, one-worker assignment/reservation, stale Farm resource mapping, FarmOccupied reject and tests). Preserve generic finite-source depletion, cancellation, dropoff relocation, fog-hidden enemy validation and construction. Rename NotVillager→NotWorker if appropriate, and update all numeric wire assertions.
- Guard and Rifleman keep ordinary base combat and attack-move. Set both counter bonuses to **0** while alien kinds do not exist, keeping `CombatSpec` schema minimal; remove old counter-triangle tests and replace with the new zero-bonus assertion. Do **not** implement Swarmer/Spitter here.
- Preserve `MatchSession` and passive Team-2 destroy-center Victory, despite the new ResearchCenter name.

### B. Author a central map that is usable, not just picturesque

**Files:** `fixture.rs`, `fixture/tests.rs`, `map/tests.rs` or a focused new fixture validation test, `godot/scenes/battlefield.tscn`.

- Move Team 1's 4×4 Research Center to a checked center anchor (suggest (62,46) as a candidate), four Worker spawn cells beside it; keep the Team-2 passive Research Center, four Workers and east-side identity until HPA-449.
- Remove the three PvP blocker rectangles in the Rust map AND their three rendered barrier meshes. Add a handful of **authored** smaller ruin rectangles with matching simple Godot placeholder box footprints; do not duplicate procedural mapping and do not leave invisible colliders. Keep N/E/S/W edge corridors open for HPA-449 and city approach slots free.
- Relocate/retag 18 source IDs deterministically: Team-1 ration/Scrap/Coal supply near the central city (IDs 1–6), Team-2 east safe supply (7–12), midfield/outer caches (13–18). Source counts, IDs, finite amounts, in-bounds constraints and no overlap stay asserted.
- Replace side-half filtering in `safe_source_ids(team)` with **explicit authored source IDs/team**, as central Team-1 resources may cross x=64. Do not retain any point-reflection assumption for asymmetric terrain. Revise `MapFixture::barracks_anchor(team)`/`safe_storehouse_anchor(team)` for reachable new footprints; tests in `session/tests.rs` and `system_order.rs` consume these two helpers. Visibility tests use `team_starts()`.
- Add fixture tests: all initial Workers occupy free adjacent cells, resources and obstacles do not overlap center/east core, valid Bunkhouse/Depot/Barracks buildable sites exist with 360° approach lanes, resource paths and dropoff paths exist, four border entry zones can reach the city, passive east base reachable by Team 1, and fog at boot doesn't reveal Team 2. Assert the actual Team-1 Research Center footprint center lies within a small, documented tolerance of the map midpoint `(64,48)`; the separate headless smoke checks **camera projection of that live view** rather than hardcoding the same anchor in GDScript. Keep unit/building/source stable-ID and order assertions.

### C. Migrate bridge/UI strings, wire keys and presentation simultaneously

**Files:** `crates/grus-godot/src/lib.rs` (and bridge/e2e tests), `godot/scenes/main.tscn` (**initial `Camera3D` is here**), `godot/scripts/battlefield_controller.gd`, `godot/scripts/unit_view.gd`, `godot/scripts/resource_view.gd`; inspect `building_view.gd`, `minimap.gd`, `combat_fx.gd` for indirect metadata dependencies.

- Remove `GrusBridgeNode.enqueue_age_up` and Rust bridge references. `economy_snapshot`: `food/scrap/coal/crystals`, population, idle Worker IDs, constant gather rate, reject code; no `age` key. `catalogue_snapshot`: units and buildings list new kinds/producers and **four** cost fields; no required-age or `age_up` keys. Keep `building_snapshot` live queue state and fog-gating.
- Update parse-helpers, debug-kind strings, meta kind labels, old production queue labels, and test setup calls. Removed names should fail instead of silently mapping to legacy kinds; no old command alias.
- Replace the existing **six build buttons** (House, Storehouse, Farm, Barracks, ArcheryRange, Stable) with **three** (Bunkhouse, Depot, Barracks), and the existing **five TrainGrid controls** (four unit-train buttons plus Age-up) with **three unit-train buttons** (Worker, Guard, Rifleman; Barracks produces the latter two). Remove obsolete scene nodes, Age button and event handler. Adjust panel layout to fit the smaller button set.
- HUD shows four currencies and population with no age; preserve session panel, feedback, pause, selection, input shielding, contextual gathering, unit selection/commands, placement preview, producer routing, rally, idle Worker. Remove Farm-building-as-resource special picking if now dead. Change `_selected_villager_ids`-style naming/copy to Worker; keep camera/minimap recenter and fog hiding.
- Reuse primitive meshes and role/ore tint differences in `unit_view.gd` and `resource_view.gd`. Update session/control help text and title only as needed; no art/model dependencies.
- If `RejectReason::FarmOccupied` is removed, recompute GDScript numeric constants from the **final Rust discriminant order** and add assertions that known active reject reasons still map. This repo deliberately has no pre-1.0 backcompat obligation.

### D. Replace obsolete test journeys with equivalent *new* gameplay proof

**Files:** `crates/grus-sim/src/**/tests.rs`, `crates/grus-sim/tests/{economy_flow,system_order,occupancy_replan}.rs`, `crates/grus-godot/src/{lib,e2e}.rs`, `crates/grus-godot/tests/e2e_boot.rs`, the four existing Godot smoke scripts, and optionally narrowly targeted tests.

- **Rust:** replace catalogue age-gate/counter-cycle assertions with 3-unit/4-building roster, zero bonuses, new prices and four-field stockpile; rewrite Farm/age-specific tests into finite-source gathering, Depot reroute/depletion, build/training/population and session paths. Replace `system_order.rs`'s Age-2 tick test with a same-tick order invariant for **construction/production/economy**; do not delete the unrelated dropoff-destroyed reroute and building-vision ordering tests. Retain combat, session, visibility and movement assertions with new anchors.
- **reset_test.gd:** keep exact **8 unique units / 2 buildings / 18 resources**, stable 1–8/1–2/1–18 IDs and fog reset; assert renamed Research Center views and stockpiles **200 Food / 300 Scrap / 150 Coal / 0 Crystals**, no age.
- **economy_smoke_test.gd:** retain real UI input-selection/preview/build/training/rally/idle-worker proof, including gathering all **three** harvestables, resource spend rejection, Bunkhouse 10→20 capacity, Depot dropoff delivery at a distinct site, Barracks producing **both** Guard/Rifleman, and Research Center producing Worker. Replace the Age2/FarmOccupied/Cavalry branches rather than removing the smoke; recompute solvency targets, resource IDs, building anchors and reject integers for new map.
- **combat_lifecycle_smoke_test.gd:** retain Start→Playing→Paused→Victory→Result→Restart, invalid-target rejections, damage/health-bar/tracer/death proof; switch Spearman→Guard, four roles→three roles, new Barracks anchor and revised camera/path timing for center-to-east attack. Keep Team-2 base until HPA-449.
- **smoke_test.gd:** adapt human-scout fog/minimap discovery legs from left start to central city and east opponent, including visible-then-hidden enemy after retreat and stale enemy selection reconciliation. **Before any pan, zoom or minimap click**, add one real headless initial-camera framing assertion: after the seeded **Team-1 Research Center building view (stable BuildingId 1)** is initialized, read `Main/Camera3D`, project the Research Center's actual view center with `camera.unproject_position(...)`, and assert the projected point lies within the **central half of the viewport**, not merely somewhere onscreen. This tests framing using live bridge-backed view metadata, while Rust fixture tests independently assert the authored Research Center sits near the 128×96 map midpoint. The camera node and initial transform are in `godot/scenes/main.tscn` (**not** `godot/scenes/battlefield.tscn`); the existing `(64,65,95)` view already looks near map center, so adjust it only if the verified new layout needs it. No shortcut to set fog explicitly; actual movement reveals it.
- **benchmark_200.gd** and e2e: preserve a 200-unit/2-building/session-free load, visible-in-tree fog stats, real movement/combat and finite frame-time assertions. Rewrite only staging/gather/roster assumptions; do not disable failing probes.

### E. Migrate the scripted benchmark with the catalogue (same PR)

**Files:** `fixture.rs` benchmark roster/`safe_source_ids`/staging/`assert_non_mirrored_seed`, `fixture/tests.rs`, `godot/scripts/benchmark_200.gd` and benchmark guidance.

- Use **four seeded Workers + 16 scripted Workers + 40 Guards + 40 Riflemen** per team (100/team, 200 total), with 12 workers per side gathering from that side's explicit safe-source list and military attack-moving toward an authored reachable skirmish target.
- Refactor staging to two independently authored deterministic walkable zones, not point reflection. Validate 96 unique cells per side and avoid overlap with new resource/obstacle/core footprints; fail loudly on rejected scripted commands. Preserve session-free bridge reset and movement/combat liveness.
- Run 10 s CI benchmark; for 60 s packaged hardware run, capture/rebaseline measurements when a reference Linux machine is available, labeling **HPA-448 temporary winter re-skin workload**. Do not copy HPA-445 FPS/visibility means to the changed scenario, and do not invent hardware numbers. HPA-449 will replace the benchmark with aliens-vs-defenders.

### F. Update active documentation only

- README: winter catalogue, stockpiles, centered-city layout and passive east fixture, controls, three-resource gathering + Crystal future income, smoke commands, benchmark roster and measurements with workload label. No claim of wave enemies or art asset upgrades.
- `CLAUDE.md` only (AGENTS symlink): updated module shape, typed keys, commands, IDs, no Age/Farm jobs, new smoke expectations, fixed order and benchmark seed; fix obsolete test-by-name examples.
- Keep old HPA-471/472/473 historical design/plan documents as project history, not compatibility obligations. The HPA-445 contract and project-wide spec remain the source of downstream boundaries.

## Exact regression/verification matrix

| Contract | Validation |
| --- | --- |
| Rust style | `cargo fmt --all -- --check` |
| Clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo clippy -p grus-godot --all-targets --features e2e --locked -- -D warnings` |
| Rust unit/integration | `cargo test --workspace --locked` and `cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1` |
| Sim coverage | `cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\.rs$'` (the same exclusion as CI) |
| Fixture geometry | Stable 8/2/18, four-edge access, resource/obstacle non-overlap, center/east anchors, all resource/Depot routes reachable, basic build placements legal |
| Economy/catalogue | Four stockpile/cost fields, Crystals remain zero without kill source, all three finite resources gather, exactly 3 unit kinds/4 building kinds, correct two Barracks producers and non-age behavior |
| Godot input/fog/reset + **boot camera** | `godot --headless --path godot res://scenes/smoke_test.tscn` and `godot --headless --path godot res://scenes/reset_test.tscn`; assert fresh-boot Team-1 Research Center projects into the **central half of the viewport before pan/zoom**, preserving later minimap-recenter checks |
| Godot economy | `godot --headless --path godot res://scenes/economy_smoke_test.tscn` — build+gather+both military train types+idle+rally |
| Godot combat/lifecycle | `godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn` — Victory on passive east Research Center, restart freshness |
| Export and e2e | Existing CI packaged Linux export boot, plus `GRUS_E2E_BINARY`-driven bevy-e2e |
| Benchmark | `GRUS_BENCHMARK_SECONDS=10` on `benchmark_200.tscn` in CI; 60 s on reference package if available, record metrics as new workload |
| Dead **names, coordinates and comments** | Search active Rust/GDScript/scene/README/CLAUDE paths for `Age2`, `EnqueueAgeUp`, `Farm`, `Cavalry`, old resources/producer strings, old anchor/worker/staging coordinates (e.g. `12,46`, `11,45`, benchmark `(64,12)`), old PvP blocker rectangles, `WestBarrier`/`EastBarrier`/`CenterBarrier`, geographic `x < 64` assumptions, stale `Town Center`/`mirrored` comments and misleading population/source claims; inspect matches before removing rather than blanket rewriting historical docs. No compatibility shims |

For real Godot tests, build `grus-godot` first, copy the platform GDExtension into `godot/bin`, and scan the editor once to register classes before invoking smoke scenes. Confirm full CI on the pushed PR head; do not call documentation-only planning verification a gameplay test run.

## Risk-driven sequencing and review criteria

1. Audit all static symbol **and numeric-coordinate** consumers (sim, bridge, GDScript, test files, scenes, README/current guidance) **before** deleting variants; make a migration inventory. Inventory the old `(12,46)` Team-1 core/adjacent Worker cell literals, `(64,12)` benchmark midpoint, all former wall rectangle extents, scene obstacle-node names, and camera in `godot/scenes/main.tscn`. Run a targeted sweep such as `rg -n 'WestBarrier|EastBarrier|CenterBarrier|Town Center|mirrored|12, ?46|11, ?45|64, ?12|54, ?20|66, ?20|62, ?38' crates/ godot/ README.md CLAUDE.md`, and separately search old enum labels. Investigate and intentionally adapt live references rather than blindly deleting historical explanations. Keep a single source for costs and shape.
2. Coordinate the core catalogue, economy, production, command and fixture edits with bridge updates, then fix compiler errors. Do **not** preserve obsolete APIs solely to reduce the patch.
3. Convert scene/UI paths and all four smoke journeys together; preserve real input + camera/fog privacy checks. Numeric reject codes are a known cross-language hazard.
4. Verify geometry with explicit path tests **plus** a headless pre-input camera-framing assertion, not assumptions from a pretty map; ensure the original passive eastern opponent stays reachable and the 96-per-team benchmark staging does not use map symmetry.
5. Run the actual headless Godot smokes + benchmark + packaged boot. Check 90% coverage **after** deleting age/farm lines; add tests for surviving behavior instead of dropping a gate.
6. Final README/evidence includes actual tests run and labels HPA-448 interim benchmark; mark the draft PR ready only after implementation and verification. No extra PR for implementation.

## Scope exclusions

No alien spawn director, Crystal income, walls/gates/towers, heat production/efficiency, research progress, underground, new high-quality model/terrain art, new SFX, multiplayer, data migration, backcompat, generalized faction system, generic data-driven UI refactor, or procedural map authoring. Those have later/different tickets.

**Definition of done for the *planning* draft:** this spec + implementation plan are reviewed and linked to HPA-448; no implementation or CI pass is implied.
