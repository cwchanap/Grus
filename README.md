# Grus

2.5D real-time strategy game built with Godot and Bevy ECS via godot-bevy.

## HPA-470 battlefield baseline

The first retained MVP slice runs a fixed-angle 3D Godot battlefield over a flat Bevy ECS simulation plane. Bevy owns stable unit identity, commands, occupancy/pathing, and authoritative movement. Godot owns presentation and input; gameplay transforms flow one way from Bevy into Godot.

The simulation runs on an explicitly configured **20 Hz Bevy fixed clock**. Godot presentation interpolates between the previous and current authoritative positions every visual update; the runtime smoke measured a maximum visual step of **0.095 world units** for a speed-12 unit, versus a 0.6-unit fixed-tick step without interpolation.

Occupancy changes use one shared `GridMap::set_blocked` seam. The map revision changes only when walkability actually changes, and active move orders replan once when they observe a newer revision instead of pathfinding every tick. This is the construction-facing occupancy contract consumed by later MVP work.

The first recorded desktop target is **Linux x86_64** with **Godot 4.7.1** and the repository-pinned **Rust 1.95.0** toolchain.

## HPA-471 economy skirmish

The runtime fixture has one human start: four Workers around one Research Center, 18 finite resource sources (ration caches/salvage ruins/coal seams, plus northwest/southeast expansion nodes), and starting stockpiles of 200 Food / 300 Scrap / 150 Coal / 0 Crystals. Aliens enter from the four fixture-authored map edges during the survival match; the eastern deposits are human expansion opportunities. Crystals are a stockpile/cost field earned by killing aliens — there are no crystal map nodes or gathering routes.

## HPA-448 central-city relayout (historical layout before HPA-449)

HPA-448 moved Team 1's 4×4 Research Center to the map midpoint (footprint anchor (62,46), center (64,48)) and authored Barracks/Depot anchors around the safe ring. Its two-base fixture and passive Team 2 start were removed by HPA-449. Resource ids 13–18 remain the authored northwest/southeast expansion nodes.

The old central lane barriers are gone. The only static blockers are sparse authored **ruins**: one `RUINS: &[(GridPos, GridPos)]` inclusive-rectangle table in `fixture.rs`, applied through `GridMap::set_blocked_rect` and exposed read-only to Godot as `GrusBridge.obstacle_rects()`. `godot/scripts/ruins.gd` fits one imported ruin prop to each rectangle from that payload — ruin coordinates exist in exactly one place. The ruins stay clear of the human base, every authored source and worker cell, the central construction/gather ring, the N/E/S/W access lanes, and benchmark staging regions; fixture tests prove each exclusion.

The active catalogue builds on the HPA-448 winter roster: **Worker / Guard / Rifleman** units and **Research Center / Bunkhouse / Depot / Barracks / Wall / Gate / Gun Turret / Coal Furnace / Flame Tower / Crystal Lance** buildings. The Barracks is the single military producer (Guard and Rifleman); the Research Center is seeded-only and trains Workers. The Flame Tower and Crystal Lance are the HPA-188 research-gated advanced towers (25% and 50% unlocks — see below). The former Age/Age-up, Farm, Cavalry, Archery Range, and Stable systems were removed in HPA-448 checkpoint 1.

## HPA-290 winter terrain and ruined-city art

The 128×96 ground uses a textured snow material. Static blockers cycle between snow-covered collapsed buildings, wrecked transports and frozen debris; Food, Scrap and Coal sources use distinct ration-cache, salvage-frame and coal-seam models. The [winter art pack](godot/assets/winter/README.md) includes all six GLBs, the snow texture, import settings, provenance and Blender regeneration source. The smoke checks actual imported geometry against bridge footprints and resource cells, checks the full visible resource silhouette against gathering picks at maximum zoom, and rejects physics bodies in static art. Simulation, occupancy, resource identities, fog and picking retain their existing contracts.

## HPA-449 alien survival

After Start, the match has a 300-second preparation countdown. The wave director announces fixture-authored entry zones during the final 30 seconds, launches the first wave at 300 simulated seconds, and continues every 100 seconds. Aliens use the existing spawn, AttackMove, movement and combat paths, with a 150-live-unit cap. Killing an alien with Team 1 credits its catalogue Crystal reward. Destroying the sole Research Center by aliens causes Defeat; HPA-188 (below) added the research Victory path on top of this director.

The wave HUD shows preparation/current wave, next arrival, incoming directions and live alien count. Brief minimap rings use the announced fixture anchors; individual alien markers and picking still require vision. Swarmer, Spitter, Brute and Behemoth use distinct primitive scales and role markers. See the [wave director contract](docs/superpowers/specs/2026-10-24-hpa-449-wave-director-design.md) and [verification evidence](docs/hpa-449-wave-director-evidence.md).

## HPA-450 defensive structures

The build panel offers Walls, Gates, and Gun Turrets (HPA-188 later added the research-gated Flame Tower and Crystal Lance to the same grid). Walls block both teams; a Gate is passable to human units from placement and blocks alien routes until it is destroyed. Wall and Gate placement stays armed for consecutive clicks. When barriers seal the Research Center, aliens use AttackMove to reach and attack a reachable structure under normal visibility rules. Completed Gun Turrets automatically fire at visible aliens in range and award Crystals through the shared combat path.

### Economy architecture

Bevy remains fully authoritative for gameplay. A typed catalogue (`crates/grus-sim/src/catalog.rs`) is the single source of costs, build/train seconds, footprints, unlocks, population values, speeds, and gather rates. The core economy modules are:

- **commands** — player command application, ownership validation, `RejectReason` codes, unit spawning
- **economy** — worker tasks, gather/carry/deposit, resource sources, drop-off routing
- **buildings** — placement validation, construction, completion grants (Dropoff, production queues, turret combat)
- **production** — FIFO queues, combined surface/Shelter population caps, spawn clearance, rally points
- **underground** — one paid dig, four room slots per completed level, independent construction timers and fractional room income
- **heat / research** — live surface efficiency plus geothermal supply, and heat-scaled surface research plus cold-independent Labs

Systems tick in a fixed order (waves → underground → heat → combat → movement → economy → construction → production → research → visibility) so completions land deterministically. Godot drives gameplay only through `GrusBridge` command calls (move/stop/gather/place/resume/enqueue/rally/excavate/build_room) and reads state through typed snapshots: `economy_snapshot` (stockpiles, population, idle workers, heat supply/demand/surface efficiency, last reject code), `building_snapshot` (completion, construction/queue progress, blocked reason, rally), the read-only `placement_preview`, static `catalogue_snapshot().underground` (room costs/effects/minimum levels), and dynamic `underground_snapshot` (depth, Rust-priced next dig, progress and indexed slots). Numeric reject/blocked codes follow `RejectReason` declaration order; presentation flows one way from Bevy into Godot views.

## HPA-472 combat and match lifecycle

Combat runs in `grus-sim` on the same fixed tick: melee/ranged range and cooldowns, attack-move acquisition with pursuit, and building attacks that path to the nearest walkable perimeter and range-check against `Footprint::closest_point`. Godot stays presentational: contextual right-click attacks on enemy units/buildings, `A`-armed attack-move, health bars and role markers on unit/building views, and transient tracer/hit/death effects with a generated audio blip — all driven by Playing-gated `CombatEvents` drained from the bridge.

The match lifecycle is simulated-side: the skirmish boots into **Start** (gameplay orders reject with `SessionLocked`), the Start button moves to **Playing**, and `Esc` or the Pause button toggles **Paused** (sim and cosmetics freeze; interpolation collapses `previous = current` so nothing oscillates). In HPA-449, aliens destroying the sole Research Center settles **Result/Defeat**; since HPA-188, reaching the 9,000-point research target settles **Result/Victory** for Team 1 through the same session path. Later gameplay mutation is rejected. Restart/Quit belong to Result: Restart reseeds one fresh match — no duplicate nodes, stale ids, leftover selection/control groups, or lingering effects; Quit exits the app. Workers, buildings, and construction sites die with full economy/occupancy/queue cleanup.

Fog of war is owned by HPA-473 (next section). HPA-445 removed the economic AI and HPA-449 removed the passive Team-2 base; the wave director now spawns actual alien attackers. Fog still controls which live aliens and markers are visible.

## HPA-473 fog of war and minimap

### Fog of war

Visibility is simulation-owned: `VisibilityMap` in `grus-sim` tracks three states per cell — **Unexplored**, **Explored** (remembered terrain, no live units), and **Visible**. Every unit projects a circular vision radius of **10 cells (Euclidean)** each tick, and completed buildings reveal from the boundary cells of their footprint (identical union to every cell — an interior cell's circle is always subsumed by the edge cells'). Standalone resource sources stay visible on the map once explored; enemy units and buildings require **current** vision to be shown, targeted, placed against, or gathered near. The boundary is enforced in the sim (combat acquisition, building placement, gather routing reject hidden targets — and unknown ids answer like missing ids so reject codes cannot probe fog) and again in the views: rendering, click picking, building inspection, combat tracers, and the minimap all hide what Team 1 cannot currently see — no hidden-enemy leak through any presentation path.

### Minimap

The minimap is a **128x96 runtime `ImageTexture`** (1 px per cell) redrawn from the Team-1 fog states: friendly markers always, enemy markers only while currently visible, resource markers once explored, plus the camera viewport rectangle. Left-clicking the minimap **recenters the camera only** — it never issues gameplay commands and never reveals anything.

### Runtime setup

The runtime match is a **human Team 1 survival battle against alien waves**. It starts with one Research Center, four Workers, and the authored resource stockpile; aliens attack after the 300-second preparation period. Fog of war, the minimap, combat, Defeat, restart and the session-free benchmark remain operational. Restart returns the match to a fresh **Start** state: initial fog (explored cells back to Unexplored, aliens hidden again).

## HPA-181 heat and cold: surface efficiency

Completed Team-1 buildings demand heat; the Research Center supplies **+15** while alive and completed Coal Furnaces add **+12 each** while their fuel lasts. The economy HUD's third line reports the live flow as `Heat <supply> / <demand> · surface <p>%`, straight from `economy_snapshot`'s `heat_supply`/`heat_demand`/`surface_efficiency` — a real deficit is never rounded up to 100%. Friendly completed heat-demanding views show a desaturated icy tint while efficiency is below 100% and recover the normal team blue when warmed; Walls, Gates and Furnaces demand no heat and never tint.

| Completed structure | Heat supply | Heat demand |
| --- | ---: | ---: |
| Research Center | +15 | 10 |
| Bunkhouse | 0 | 1 |
| Depot | 0 | 1 |
| Barracks | 0 | 3 |
| Gun Turret | 0 | 2 |
| Flame Tower | 0 | 3 |
| Crystal Lance | 0 | 4 |
| Wall / Gate | 0 | 0 |
| Coal Furnace | +12 when fueled | 0 |
| Any construction site | 0 | 0 |

`efficiency = demand == 0 ? 1.0 : clamp(supply / demand, 0.25, 1.0)` — a deficit floors at **25%**. Cold slows only the surface work completed buildings perform: Research Center/Barracks training, completed combat-tower firing (Gun Turret, Flame Tower, Crystal Lance) and the surface portion of passive research all scale with efficiency; underground Labs remain additive and cold-independent. **Cold does not slow** worker gathering/carrying, construction speed, unit movement, or unit combat — units have no surface efficiency. A completed Coal Furnace (2×2, 80 Scrap, 20 s, Team 1 only) burns **0.5 Coal per simulated second** from the team stockpile while Playing; when Coal runs out its output and burn stop on the next tick until gathering refuels. Heat is a live flow, not a stockpile — there is no heat currency or storage.

## HPA-188 research, advanced towers and Victory

The survival match is winnable: while **Playing**, Team 1 banks monotonic **research points** toward a **9,000-point** target. Passive research accrues at **2 points per simulated second × surface efficiency + 1 point/s per completed Lab**, so a cold base slows only the surface term (like training and turret cooldowns), never the underground Lab contribution. The research panel under the wave HUD shows points, floor-percent and a live **ETA = remaining ÷ live rate** — an estimate that follows the heat-scaled rate, never a fixed countdown. Research freezes in Start/Paused/Result and resets to zero on restart.

Selecting the owned, completed Research Center enables the **Analyze** button: each press queues an immediate conversion of **10 Crystals into +90 points** through the ordinary command/feedback path — never a production queue. Affordability is not pre-checked in the UI, so an unaffordable press surfaces `Command rejected (InsufficientResources)` (reject code 8) with the stockpile and points untouched.

Milestones derive from the monotonic total — no stored unlock flags:

| Research | Effect |
| --- | --- |
| 25% (2,250) | **Flame Tower** buildable (2×2, 120 Scrap + 60 Coal, 25 s, range 3, +10 vs Swarmer, heat demand 3) |
| 50% (4,500) | **Crystal Lance** buildable (2×2, 150 Scrap + 40 Crystals, 30 s, range 10, +15 vs Brute, heat demand 4) |
| 75% (6,750) | Every *subsequently launched* wave gets a 1.5× threat budget; already-committed rosters are unchanged |
| 100% (9,000) | Team-1 **Victory** through the existing MatchSession Result overlay |

Locked build buttons render their catalogue-derived `Unlocks at N research` threshold and disable while the live total is below it; sim placement validation stays authoritative either way. Victory and the alien-destruction Defeat share one rule: the first settled Result wins — a lethal hit on the Research Center in the same tick as the target's reach still defeats.

The Godot boundary is covered inside the existing smokes: the bridge smoke pins the research HUD geometry, the exact `research_snapshot` surface (numeric finite `points`/`rate_per_second`/`eta_seconds`), catalogue-bound tower locks/tooltips and a real-input Analyze leg; the combat lifecycle smoke exercises the fresh **zero-Crystal** Analyze press through real viewport input (typed reject 8, no spend, no +90 jump), live bar/ETA progress at 20×, and the real `_restart_match` path returning research to zero. Full 9,000-point Victory, exact milestone thresholds and the 75% wave escalation are pure-Rust `grus-sim` contracts — a natural run is ~4,500 simulated seconds, far beyond a 45–90 s CI smoke.

## HPA-288 underground levels and rooms

**Tab** toggles the scrollable underground panel, including after a button has keyboard focus. **Esc closes an open panel without pausing**; with it closed, Esc retains placement-cancel/pause/resume behavior. The Research Center need not be selected. Click **Excavate** to pay once up front; only one dig runs at a time. Each completed level provides four slots. Arm a room kind, then click an eligible empty slot to pay for automatic construction. Without an armed kind, empty slots are disabled; arming Lab disables L1/L2 targets with `Requires L3`, while switching to Shelter makes L1 eligible again. Occupied/building slots cannot be clicked again. Kind tooltips show costs, time and effects from the Rust catalogue. Prices below are provisional; HPA-289 owns final tuning.

| Room | Minimum level | Scrap | Build time | Completed effect |
| --- | ---: | ---: | ---: | --- |
| Shelter | 1 | 50 | 15 s | +10 shared population capacity (combined cap 100) |
| Greenhouse | 1 | 75 | 20 s | +0.5 Food/s |
| Coal Vein | 2 | 90 | 25 s | +0.4 Coal/s |
| Geothermal Tap | 3 | 120 | 30 s | +15 heat at L3, +5 for each deeper level; no Coal burn |
| Lab | 3 | 150 | 30 s | +1 research point/s, outside the surface cold multiplier |
| Deep Mine | 4 | 150 | 35 s | +0.4 Scrap/s |

Minimum levels are not exclusive: any unlocked kind can be repeated in separate slots on that level or deeper, including L5+. There is no arbitrary depth cap. To excavate level `n >= 1`: Scrap is `round-half-up(100 × 1.6^(n−1))`, Coal is half the **rounded Scrap**, rounded up, and duration is `30 + 15 × (n−1)` simulated seconds. At very deep levels, Scrap and duration saturate at `u32::MAX` rather than wrapping; Coal is ceiling-half of the saturated Scrap. Godot displays Rust's computed values.

| Target level | Scrap | Coal | Dig time |
| --- | ---: | ---: | ---: |
| L1 | 100 | 50 | 30 s |
| L2 | 160 | 80 | 45 s |
| L3 | 256 | 128 | 60 s |
| L4 | 410 | 205 | 75 s |
| L5 | 655 | 328 | 90 s |

Rooms require **no workers, pathing, fuel or underground heat demand**, and neither digging, construction nor room output is slowed by surface cold. Construction of independent rooms can overlap each other and a deeper dig. Resource rooms bank fractional income into the ordinary stockpile only after completion (not retroactively on their completion tick); completed Shelter/Tap/Lab benefits are available to later systems in that same tick. Research now accrues at `2 × surface_efficiency + completed Labs` per second and still uses the single existing ETA/Victory path. Start, Pause and Result freeze underground timers/output; the panel remains inspectable with purchases disabled. The living Research Center is required. Purchases are non-cancelable; Restart clears all depth, rooms, paid jobs and fractional remainders.

The ordinary four-worker Rust feasibility test gathers real resources for L1–L3 plus Tap+Lab (786 Scrap / 258 Coal total), completing at **179.65 simulated seconds**, **120.35 s before wave 1**. The standalone Godot smoke uses real selection/Gather and paid Buttons, then additionally funds two surface Barracks to prove Lab +1/s under actual cold (15/16 heat → 2.875 research/s) before a Tap supplies +15 heat (30/16 → 3 research/s). It also checks focused Tab/Esc, pause freeze, 1-based levels/0-based slots, disabled-click no-queue/no-spend and mouse shielding, HUD queue/rejection feedback, and the production controller Restart handler. No stockpile injection or timer skips are used. The measured local headless run (Godot 4.7.1, macOS arm64, debug non-e2e extension) took **14.59 s**, observed at least **740 Scrap / 110 Coal** in worker deposits, and finished with **35–40 simulated seconds** of preparation remaining across measured local runs. It retasks the Coal worker to Scrap once L3 Coal is funded. Linux CI gates both exit 0 and `GRUS_UNDERGROUND_SMOKE_OK` within 45 seconds; this local run is not hosted-CI or Linux-export evidence.

### Build and launch

Install Godot 4.7.1 with export templates, then from the repository root:

```bash
cargo build -p grus-godot
mkdir -p godot/bin
# Linux/CI loads the .so, macOS loads the dylib — copy the one for your platform:
cp target/debug/libgrus_godot.so godot/bin/libgrus_godot.so
cp target/debug/libgrus_godot.dylib godot/bin/libgrus_godot.dylib  # macOS
godot --path godot
```

On a clean clone the GDExtension is not registered until the editor has scanned the project once (`godot --headless --path godot --editor --quit-after 120`), which writes `godot/.godot/extension_list.cfg`. The library in `godot/bin/` must be re-copied after every rebuild — a stale binary fails at startup (for example a false duplicate-`UnitId` panic) rather than silently.

### Verification

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked

godot --headless --path godot --editor --quit-after 120
godot --headless --path godot res://scenes/smoke_test.tscn
godot --headless --path godot res://scenes/reset_test.tscn
godot --headless --path godot res://scenes/economy_smoke_test.tscn
godot --headless --path godot res://scenes/combat_lifecycle_smoke_test.tscn
godot --headless --path godot res://scenes/wave_smoke_test.tscn
godot --headless --path godot res://scenes/defenses_smoke_test.tscn
godot --headless --path godot res://scenes/underground_smoke_test.tscn
```

The main Godot smoke exercises real input-derived click/box/additive selection, control groups, move, stop, HUD input shielding, zoom, pan, the 20 Hz cadence and interpolated presentation. It pins battlefield contracts: the boot camera frames the live Research Center, rendered `Ruins` match `GrusBridge.obstacle_rects()`, blocked-ground clicks receive `unreachable` feedback, and a human scout reveals an expansion resource before leaving it explored under fog. Its research section pins the panel geometry, the exact numeric `research_snapshot` surface, the 3×3 build grid's catalogue-bound tower locks and the enabled/disabled Analyze dispatch through real input. Every pre-wave Playing smoke checks the shared wave snapshot before exit or reset. The reset smoke requires exactly 4 unit views / 1 building view / 18 resource views, unique stable ids and the starting 200/300/150/0 stockpile. The economy smoke retains its full real-UI gather/build/train/rally/reject loop at 20×, split into two guarded preparation legs, and its second leg proves the HPA-181 heat boundary with real input: warm 15/13 baseline after the Barracks, a Depot+Gun Turret deficit (15/16 → `surface 94%` and icy tints), a fueled Coal Furnace restoring 27/16 with tint recovery, visibly decreasing Coal, and funding through real gather cycles. The combat lifecycle smoke uses live controller attack-move and visible-alien right-click input, checks fog-gated markers and a Crystal kill reward, freezes combat while Paused, resumes into an alien-caused Defeat, and checks Result input lock and restart freshness; it also drives the HPA-188 research boundary in its fresh zero-Crystal window — the enabled Analyze button pressed through real input returns typed `InsufficientResources` (code 8) with nothing spent or banked — plus live bar/ETA progress at 20× and the restart path resetting research to zero. The dedicated wave smoke adds Start/Pause countdown freeze, HUD warning text, authoritative minimap-anchor pings and their expiry, plus labeled white-box checks of all four role silhouettes, and runs the same shared journey including the research leg. The HPA-450 defense smoke builds a wall ring from the live Research Center footprint through the UI, proves human Gate traversal, observes a fogged alien Gate breach and post-breach structure damage, and verifies a Gun Turret Crystal kill and building-origin tracer.

### Controls

The Start overlay shows a compact first-run controls label until the match starts:

```text
Select: left click / drag / Shift
Command: right click
Attack-move: A then right click
Stop: S
Pause: Esc · Underground: Tab
Camera: wheel / middle-drag
Minimap: click to recenter
```

The full reference:

- Left click: select a friendly unit or building (units take priority within the click radius)
- Shift + left click: add to selection
- Left drag: box select
- Right click: contextual — gather on a resource, resume construction on an incomplete friendly building, attack an enemy unit/building with the selected combatants, set a rally point when a producer is selected, otherwise move
- `A`: arm attack-move; the next right-click moves to the ground point and engages enemies acquired on the way
- `S`: stop selected units
- `Tab`: toggle the underground panel (also when a panel button holds focus)
- `Esc`: close an open underground panel without pausing; otherwise cancel armed placement first, or pause/resume
- `Ctrl+1` through `Ctrl+9`: assign a control group
- `1` through `9`: recall a control group
- Mouse wheel: zoom
- Middle drag: pan the camera
- Minimap (bottom right): left-click recenters the camera on that map cell (view only — no commands, no reveal)
- Build buttons (bottom panel): arm placement for Bunkhouse/Depot/Barracks/Gun Turret/Coal Furnace plus the research-gated Flame Tower (25%) and Crystal Lance (50%); Wall and Gate placement stays armed for consecutive left-clicks. Locked towers show their unlock threshold in the tooltip and enable when banked research reaches it. The ground preview renders green when valid and red when blocked; right-click or `Esc` cancels placement without issuing a world command.
- Train buttons: queue Worker on a selected Research Center, or Guard/Rifleman on a selected Barracks
- Analyze button (research panel, top left): enabled while Playing with the owned Research Center selected — converts 10 Crystals into +90 research points; the sim's typed reject shows in the status line when Crystals are short
- Idle button (top right): selects and navigates to the next idle worker

HUD controls consume their mouse events so UI interaction does not leak into world commands.

### Reset modes

- **Fixture reset** (`reset_fixture`, used by `reset_test.tscn` and the Result **Restart** button): despawns gameplay and reseeds the 4-worker / 1-Research-Center / 18-resource survival fixture with starting stockpiles, a fresh wave clock, zeroed research and empty underground depth/jobs/rooms.
- **Benchmark reset** (`reset_benchmark_fixture`, used by `benchmark_200.tscn`): reseeds a **session-free** 200-unit stress workload — the Research Center inside an authored fortress of **146 completed buildings** (92 Walls, 4 Gates, 24 Gun Turrets, 10 Flame Towers, 6 Crystal Lances and 9 Coal Furnaces, with a benchmark-only 400 Coal stockpile) defended by 50 humans: 4 Workers gathering the safe sources plus 23 Guards and 23 Riflemen holding the four gate lanes on accepted `AttackMove`s. The **150 actual Team-2 aliens** are spawned through the shared wave/`AttackMove` path at the synthetic `BENCHMARK_WAVE = 100` — a live-cap stress load only, whose overflow HP is not representative of normal wave-20-to-25 balance. It keeps the authored resource set and fog of war. A missing `MatchSession` reads as Playing, so the benchmark remains active without a lifecycle overlay. It is never used by the runtime match or other smokes.

### Linux x86_64 export

```bash
mkdir -p build
godot --headless --path godot --export-debug "Linux x86_64" ../build/Grus.x86_64
./build/Grus.x86_64
```

For a reproducible release package on a Linux x86_64 host (Godot and the Linux export templates already installed), run:

```bash
./scripts/package_linux_x86_64.sh
```

The script builds `grus-godot` in release mode, stages the extension into `godot/bin/`, exports the `Linux x86_64` preset into `dist/grus-linux-x86_64/`, and archives the full directory as `dist/grus-linux-x86_64.tar.gz`. The script installs nothing and downloads no toolchains.

`GRUS_MAIN_SCENE` (optional) temporarily swaps the packaged main scene and `godot/project.godot` is always restored afterwards, on success, error or interruption. It exists for the hardware-acceptance benchmark package only — a release package built with it would boot the benchmark instead of the game:

```bash
GRUS_MAIN_SCENE=res://scenes/benchmark_200.tscn ./scripts/package_linux_x86_64.sh
```

CI also boots the exported executable headlessly and verifies that the Rust GDExtension initializes successfully.

## Testing / CI

CI runs three jobs. `rust-build-lint` gates formatting, Clippy, and workspace compilation. `unit-tests` enforces the 90% `grus-sim` line-coverage gate and runs the `grus-godot` library unit tests. `e2e` runs the Godot import, the bridge/reset/economy/combat-lifecycle/wave/defense/underground smokes, the Linux export validation, the bevy-e2e boot test against the exported build, and the 200-unit benchmark.

Rust build/lint:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --locked
```

Unit coverage:

```bash
cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\.rs$'
cargo test -p grus-godot --lib --locked
cargo test -p grus-godot --lib --features e2e --locked -- --test-threads=1
```

E2E:

```bash
cargo build -p grus-godot --features e2e --locked
# stage extension and export Godot build as CI does
GRUS_E2E_BINARY=<path-to-exported-grus> \
  xvfb-run -a cargo test \
  -p grus-godot \
  --features e2e \
  --test e2e_boot \
  --locked \
  -- --test-threads=1
```

`GRUS_E2E_BINARY` must point at the exported Grus executable (stage the extension, then `godot --headless --path godot --export-debug "Linux x86_64"` as in the export section above); without it the e2e test skips loudly instead of running.

## Benchmark

The benchmark fixture (50 human defenders and 150 aliens, see **Reset modes** above) exists only for `benchmark_200.tscn` after an explicit benchmark reset; the runtime match and other smokes run the one-base fixture. Since HPA-289 the workload is the walled-city stress fixture described there — a 146-building fortress under a synthetic wave-100 horde — so every earlier measurement below (the pre-fortress 50/150 runs, the older mixed 100-vs-100 scenario and the historic one-core timings) is **not comparable**. The scene samples for a real-time duration set by `GRUS_BENCHMARK_SECONDS` (default 10, must be positive and finite): it waits for 200 live unit views with the exact team census plus all 146 building views with the sole Research Center identity, and for the fog revision to advance, then records frame times plus live-unit and visible-in-tree unit counts (the visibility-flag render-load proxy), failing a sample that shows neither movement nor combat while the battle is still contested. Note `GRUS_BENCHMARK_SECONDS` bounds **wall-clock** time, not simulated time — on a slow renderer the fixed 20 Hz ticks lose time and a single long frame overshoots the sample deadline, so wall-clock seconds do not translate one-to-one into simulated seconds.

During measurement the runner issues no scripted orders: the wave director itself retargets aliens — Research Center first, then the lowest-id team-1 building, then the lowest-id live team-1 unit — so the session-free workload keeps alien intent through the ordinary command paths after the core falls. Workers retain their gather tasks; units are never respawned or healed, and attack cooldowns are unchanged. The liveness gate requires movement or combat in each window only while the battle is contested (each side still has live units or live buildings); after one side is fully extinct, quiet windows are the real end state and pass. The earlier scripted `replay_benchmark_orders` seam was removed — its per-5-second route-planning bursts had been distorting the measured max/p99 frame times.

- **HPA-289 local headless 10 s requested (fortress tree, the content committed as `ee6a32a`):** Godot 4.7.1, macOS 25.6.0 arm64, debug non-e2e extension, `--headless` with `--rendering-method gl_compatibility` — a configured method only: the headless display driver uses the dummy rendering driver and the marker's `adapter=`/`vendor=` fields are empty, so **no GPU rendering occurred** and these are scene-contract runs, not rendered-workload evidence. `GRUS_BENCHMARK_SECONDS=10`, two exit-0 runs with the full `GRUS_BENCHMARK_200` sentinel. Requested 10 wall-clock s produced `duration_s` 19.74 / 19.75 s because one ~10.1 s frame overshot mid-sample (stall cause unverified; `visibility_revision=206` is a fog-change counter, not a clock — the sampled simulated duration is unmeasured): 489 / 485 frames, mean 40.372 / 40.724 ms (p50 7.724 / 7.671, p95 42.124 / 42.586, p99 83.189 / 83.789, max 10,144.203 / 10,179.708; 24.8 / 24.6 fps). Population: live units start 200 (50 human / 150 alien) / min 178 / mean 184.8 / end 178; visible-in-tree min 50 / mean 150.8 / end 177. All **146 buildings** stood at sample end (CoalFurnace:9, CrystalLance:6, FlameTower:10, Gate:4, GunTurret:24, ResearchCenter:1, Wall:92 — no breach claimed for this headless run; the Rust 60-*simulated*-second test owns breach evidence); heat 123 supply / 112 demand / 1.000 efficiency; coal 350 at end (benchmark-seeded 400). Local contract evidence only — not Linux software-GL CI and not the (still missing) Linux hardware reference host. Full evidence, including the fixed-defense balance guard and the catalogue tuning pass, lives in [docs/hpa-289-survival-balance-evidence.md](docs/hpa-289-survival-balance-evidence.md).
- **HPA-288 local headless 10 seconds (underground tree):** Godot 4.7.1, macOS arm64, debug non-e2e extension, `gl_compatibility`, unchanged `GRUS_BENCHMARK_SECONDS=10` workload: 10.01 s / 544 frames, mean 18.399 ms (p50 7.524, p95 45.887, p99 192.424, max 309.533; 54.4 fps). Live units start 200 / min 163 / mean 182.4 / end 163; visible-in-tree min 72 / mean 95.9 / end 151; visibility revision 240. Exit 0 and full benchmark sentinel, with the existing two-ObjectDB-instance shutdown warning. Slower than the historical HPA-290 sample below, but no same-session pre-feature control was measured, so the difference cannot be attributed to underground. This is not rendered reference-host/Linux-export hardware acceptance.
- **Historical HPA-290, local headless 10 seconds (winter-art tree):** Godot 4.7.1, macOS 26.6.2 arm64, debug non-e2e GDExtension, `gl_compatibility`: 10.01 s / 786 frames, mean 12.735 ms (p50 6.999, p95 37.705, p99 45.184, max 54.072; 78.5 fps). Live units start 200 / min 163 / mean 184.1 / end 163; visible-in-tree min 72 / mean 96.4 / end 162; visibility revision 240. Exit 0 with the complete benchmark marker. Re-measures the replay-free HPA-449 workload after the winter art landed — imported props carry one vertex-colored surface each instead of a material slot per color, the ground samples planar instead of triplanar, and the snow texture ships 1024² VRAM-compressed — and the mean, tail and workload trajectory are level with the superseded baseline. A same-session sibling run under unrelated host load read 20.683 ms mean / 183.913 ms max; that spread is host noise, not a regression signal.
- **Historical HPA-449, local headless 10 seconds (replay-free, superseded):** Godot 4.7.1, macOS 26.6.2 arm64, debug non-e2e GDExtension, `gl_compatibility`: 10.01 s / 771 frames, mean 12.980 ms (p50 6.888, p95 38.281, p99 48.150, max 84.717; 77.0 fps). Live units start 200 / min 163 / mean 184.1 / end 163; visible-in-tree min 72 / mean 96.3 / end 162; visibility revision 240. Exit 0 with the complete benchmark marker. With no scripted replay bursts, max frame time dropped from the replay-era 253–289 ms to 84.7 ms.

The two runs below were recorded **with the scripted order replay** (per-5-second route-planning bursts inside the sampled frames) — their max/p99 largely measured the ~196-route bursts, so they are historical, not current baselines.

- **Historical HPA-449, local headless 10 seconds (scripted-replay era, superseded):** Godot 4.7.1, macOS 26.6.2 arm64, debug non-e2e GDExtension, `gl_compatibility`: 10.00 s / 742 frames, mean 13.483 ms (p95 38.531, p99 54.077, max 253.382; 74.2 fps). Live units start 200 / min 165 / mean 184.6 / end 165; visible-in-tree min 71 / mean 95.9 / end 164. Exit 0 with the complete benchmark marker.
- **Historical HPA-449, rendered 60 seconds on Apple M1 Pro (scripted-replay era, superseded):** same OS/engine/debug extension, project runner, 1920×1080, OpenGL 4.1 Metal compatibility: 60.01 s / 4407 frames, mean 13.616 ms (p95 34.051, p99 52.784, max 288.669; 73.4 fps). Live units start 200 / min/end 150 / mean 155.5; visible-in-tree min/end 0 / mean 20.7. All liveness windows passed and the process exited 0. Fog hides survivors after all defenders die, so this is not a sustained 200-visible-unit render load. It is not the reference Linux release-export hardware-acceptance result. Full build metadata, limitations and regression evidence are in [HPA-449 verification](docs/hpa-449-wave-director-evidence.md).
- **Historical HPA-449, hosted Linux software GL (scripted-replay era, superseded):** [CI run 40366902630](https://github.com/cwchanap/Grus/actions/runs/40366902630) passed all jobs, including actual Linux exports, packaged smoke and exported-game E2E. Its 1920×1080 llvmpipe benchmark measured 10.16 s / 22 frames, mean 461.838 ms (p95 527.300, p99/max 672.910; 2.2 fps), live units 200 → 168 (mean 170.7), visible-in-tree mean 74.6. This is software-renderer CI contract evidence, not reference-host hardware performance.

- **CI regression:** 10 wall-clock seconds (`GRUS_BENCHMARK_SECONDS=10` in the e2e job under Linux xvfb + software GL/llvmpipe at 1920×1080, gated on exit 0 and the `GRUS_BENCHMARK_200 start_units=200` sentinel). It guards per-frame cost only — CI enforces no FPS floor — and llvmpipe's multi-hundred-ms frames mean the 10 wall-clock seconds cover far fewer than 10 simulated seconds. The first fortress-workload CI run has **not run yet** at the time of writing; results land on the next CI execution.
- **Hardware acceptance:** 60 wall-clock seconds on a real-GPU Linux x86_64 reference host, on the release export packaged with the benchmark as its main scene (not the editor/debug project runtime):

```bash
GRUS_MAIN_SCENE=res://scenes/benchmark_200.tscn ./scripts/package_linux_x86_64.sh
GRUS_BENCHMARK_SECONDS=60 dist/grus-linux-x86_64-benchmark_200/Grus.x86_64
```

  **Status: BLOCKED / not run — no suitable hardware host exists.** The macOS headless and software-GL numbers on this page are contract evidence, not substitutes, and verified engineering is not held indefinitely on this external dependency.

- **Historical HPA-449 one-off-order local run (superseded):** on 2026-10-26, Godot 4.7.1, macOS 26.6.2 arm64, headless `gl_compatibility`, debug GDExtension, `GRUS_BENCHMARK_SECONDS=10`: duration 10.00 s / 788 frames, mean 12.693 ms (p50 6.788, p95 37.433, p99 43.329, max 53.920; mean 78.8 fps), visibility revision 236. The real 50-human / 150-alien workload ran with live units start 200 / min 167 / mean 184.8 / end 167 and visible-in-tree units min 73 / mean 95.7 / end 166. The verbose process exited 0 with the benchmark marker; captured stdout also reported two ObjectDB instances at shutdown (`AudioStreamWAV` and `AudioStreamPlaybackWAV`), despite the existing `combat_fx.gd` stop/stream-clear teardown. This run predates scripted replay and is not a current baseline or a Linux release hardware-acceptance result.
- **Winter-workload measurements after the combat fixes (HPA-448 runtime-caveats follow-up) — per-tick sim profile:** the profile behind the earlier ~1 fps, on the same fixture fixture (Apple M1 Pro, macOS, arm64, release and debug builds of `grus-sim`). A scratch test, not kept in the repo, built the fogged benchmark world (`seed_skirmish`, `extend_with_benchmark_workload`, then a `VisibilityMap` inserted and refreshed every tick; the bridge's `seed_match_world` inserts the map *before* the workload commands, an ordering this probe does not reproduce and that is not expected to change per-tick cost) and timed each sim step with `Instant` over 200 ticks. The pre-fix profile was taken in its own session at host load 7–10; the snapshot-only and radius-first variants were alternated over three rounds at load 13–35, repeats within 5%. Before the fix (`163bcc1`), `acquire_target` re-resolved ~202 candidates through per-candidate ECS lookups for each of ~160 attack-moving units every tick, so `step_combat` alone cost **91–92 ms per tick in debug (3.5 ms in release)** against the 50 ms tick budget (82 ms / 2.8 ms on a fog-free world) — consistent with starving the 20 Hz catch-up loop. A per-tick candidate snapshot cut that to **9.3–9.5 ms (0.63–0.66 ms)** fogged, still 3× (debug) to 4–5× (release) the fog-free 3.0 ms (0.15 ms), because every candidate paid a fog lookup; testing the attack-move radius before that lookup brought it to **2.8–3.0 ms (0.135–0.139 ms)**, level with fog-free. `refresh_visibility`, run once per tick, is now the largest sim cost at 22.2–22.4 ms in debug and 1.4 ms in release (timed directly) and is unchanged by this follow-up.
- **Winter-workload measurements — windowed runs, pre-fix vs snapshot-only tree:** same fixture, scene-pinned 1920×1080 window, `gl_compatibility`, `GRUS_BENCHMARK_SECONDS=10`, Apple M1 Pro (macOS, arm64, Godot 4.7.1), real fogged workload, measured in one session on a host shared with unrelated apps (system load average 7–56; no per-run load was logged), so the figures are ranges, not a baseline. Alternating runs of the same benchmark on the pre-fix commit `163bcc1` and the snapshot-only tree (before the radius-first reorder), **debug** GDExtension: pre-fix 10 rendered frames, mean 1032 / 1025 ms (1.0 fps), 200 units live, one fog `Physics interpolation` warning per run; fixed 162 / 236 / 480 frames, mean 62.0 / 42.4 / 20.9 ms (16–48 fps), no warning. **Release** GDExtension on the snapshot-only tree (three runs): 523–675 frames, mean 14.8–19.1 ms (52–67 fps), p95 26–34 ms, max 187–403 ms; no release run exists for the pre-fix tree. Every post-fix run exited 0 with real combat: live units 200 → 188 (mean ≈196–197), visible-in-tree min 88 / mean ≈124–132, moved-unit counts (the log's `stage=active frames=N`, not rendered frames) positive in every window (59–178 in the snapshot-only release run whose windows were inspected). The two pre-fix runs show none of that: 200 units live throughout and a single moved-unit window.
- **Winter-workload measurements — windowed runs, final tree (radius-first acquisition):** same fixture and window, measured afterwards at host load 12–16, three runs each: **debug** 274–285 frames, mean 35.1–36.5 ms (27–28.5 fps), p95 55.6–62.6 ms, max 205–207 ms; **release** 884–897 frames, mean 11.2–11.3 ms (88–90 fps), p95 16.4–16.6 ms, max 128–150 ms. No `Physics interpolation` warning; every run exited 0 with live units 200 → 188 (mean 196.2–197.3) and visible-in-tree min 88 / mean 126.7–129.6; moved-unit windows were 55–178 in the release runs and at least 73 in the debug runs. Host load differed from the session above, so the gain over the snapshot-only figures cannot be attributed to the reorder alone — the per-tick profile is the controlled comparison. Run-to-run spread is host noise and is not characterized further; Linux CI's software-GL benchmark is a separate measurement, and reference-host hardware acceptance remains unmeasured.
- **Historical — pre-fix winter-workload re-baseline (HPA-448, superseded by the bullet above):** measured once on this repo's task-3 tree, before the combat-snapshot fix, in a real macOS window (Apple M1 Pro, arm64, Godot 4.7.1, `gl_compatibility`, **debug** GDExtension build, `GRUS_BENCHMARK_SECONDS=10`, scene-pinned 1920×1080 viewport): `duration_s=10.15`, **10 rendered frames**, mean **1014.781 ms** per frame (p50 1015.560, p95 1112.581, p99 1112.581, max 1112.581, mean 1.0 fps), `visibility_revision=83`. Workload: the session-free mixed 100-vs-100 fixture — 20 Workers / 40 Guards / 40 Riflemen per team on top of the two authored bases, 12 workers per side gathering the authored safe sources two-per-node and every combatant attack-moving the authored midfield waypoint. All 200 units stayed live (min/mean/end 200); the visible-in-tree proxy held at 121 (min/mean/end). Liveness windows passed on unit movement — the log's `stage=active frames=N` field is the moved-unit count, not a rendered-frame count. The ~1 fps cause was later profiled and fixed (see above). The workload changes again in HPA-449; treat these numbers as temporary markers, not a baseline contract.
- **Historical — interim post-AI re-baseline (HPA-445, superseded):** with the economic AI removed, the 60-second benchmark was measured locally (Apple M1 Pro, macOS 26.6.2, arm64; Godot 4.6.2 mono; then-current debug GDExtension build, `gl_compatibility`, headless): 60.01 s / 2343 frames, mean **25.611 ms** per frame (p50 6.263 ms, mean 39.0 fps), live units start 200 / mean 134.2 / end 87, visible-in-tree mean 81.5 (min/end 20). This ran the pre-relayout workload and a different engine version — it is retained as historical evidence only, not a current number. Details in [`docs/hpa-474-mvp-evidence.md`](docs/hpa-474-mvp-evidence.md).

Historical benchmark measurements (the HPA-445 interim post-AI re-baseline and the HPA-474 record) live in [`docs/hpa-474-mvp-evidence.md`](docs/hpa-474-mvp-evidence.md); the HPA-470 numeric baseline table was removed from this file — PR #1 and git history remain its historical record.
