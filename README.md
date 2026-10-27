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

The old central lane barriers are gone. The only static blockers are sparse authored **ruins**: one `RUINS: &[(GridPos, GridPos)]` inclusive-rectangle table in `fixture.rs`, applied through `GridMap::set_blocked_rect` and exposed read-only to Godot as `GrusBridge.obstacle_rects()`. `godot/scripts/ruins.gd` stamps one primitive `BoxMesh` per rectangle from that payload — ruin coordinates exist in exactly one place. The ruins stay clear of the human base, every authored source and worker cell, the central construction/gather ring, the N/E/S/W access lanes, and benchmark staging regions; fixture tests prove each exclusion.

The active catalogue is the HPA-448 winter roster: **Worker / Guard / Rifleman** units and **Research Center / Bunkhouse / Depot / Barracks** buildings. The Barracks is the single military producer (Guard and Rifleman); the Research Center is seeded-only and trains Workers. The former Age/Age-up, Farm, Cavalry, Archery Range, and Stable systems were removed in HPA-448 checkpoint 1.

## HPA-449 alien survival

After Start, the match has a 300-second preparation countdown. The wave director announces fixture-authored entry zones during the final 30 seconds, launches the first wave at 300 simulated seconds, and continues every 100 seconds. Aliens use the existing spawn, AttackMove, movement and combat paths, with a 150-live-unit cap. Killing an alien with Team 1 credits its catalogue Crystal reward. Destroying the sole Research Center by aliens causes Defeat; no research Victory path exists in this checkpoint.

The wave HUD shows preparation/current wave, next arrival, incoming directions and live alien count. Brief minimap rings use the announced fixture anchors; individual alien markers and picking still require vision. Swarmer, Spitter, Brute and Behemoth use distinct primitive scales and role markers. See the [wave director contract](docs/superpowers/specs/2026-10-24-hpa-449-wave-director-design.md) and [verification evidence](docs/hpa-449-wave-director-evidence.md).

### Economy architecture

Bevy remains fully authoritative for gameplay. A typed catalogue (`crates/grus-sim/src/catalog.rs`) is the single source of costs, build/train seconds, footprints, unlocks, population values, speeds, and gather rates. Gameplay lives in four modules:

- **commands** — player command application, ownership validation, `RejectReason` codes, unit spawning
- **economy** — worker tasks, gather/carry/deposit, resource sources, drop-off routing
- **buildings** — placement validation, construction, completion grants (Dropoff, production queues)
- **production** — FIFO queues, population caps, spawn clearance, rally points

Systems tick in a fixed order (commands → waves → combat → movement → economy → construction → production → visibility → route feedback) so completions land deterministically. Godot drives gameplay only through `GrusBridge` command calls (move/stop/gather/place/resume/enqueue/rally) and reads state through typed snapshots: `economy_snapshot` (stockpiles, population, idle workers, last reject code), `building_snapshot` (completion, construction/queue progress, blocked reason, rally), and the read-only `placement_preview`. Numeric reject/blocked codes follow `RejectReason` declaration order; presentation flows one way from Bevy into Godot views.

## HPA-472 combat and match lifecycle

Combat runs in `grus-sim` on the same fixed tick: melee/ranged range and cooldowns, attack-move acquisition with pursuit, and building attacks that path to the nearest walkable perimeter and range-check against `Footprint::closest_point`. Godot stays presentational: contextual right-click attacks on enemy units/buildings, `A`-armed attack-move, health bars and role markers on unit/building views, and transient tracer/hit/death effects with a generated audio blip — all driven by Playing-gated `CombatEvents` drained from the bridge.

The match lifecycle is simulated-side: the skirmish boots into **Start** (gameplay orders reject with `SessionLocked`), the Start button moves to **Playing**, and `Esc` or the Pause button toggles **Paused** (sim and cosmetics freeze; interpolation collapses `previous = current` so nothing oscillates). In HPA-449, aliens destroying the sole Research Center settles **Result/Defeat**; Team 2 has no Research Center and there is no player Victory path in this checkpoint. Later gameplay mutation is rejected. Restart/Quit belong to Result: Restart reseeds one fresh match — no duplicate nodes, stale ids, leftover selection/control groups, or lingering effects; Quit exits the app. Workers, buildings, and construction sites die with full economy/occupancy/queue cleanup.

Fog of war is owned by HPA-473 (next section). HPA-445 removed the economic AI and HPA-449 removed the passive Team-2 base; the wave director now spawns actual alien attackers. Fog still controls which live aliens and markers are visible.

## HPA-473 fog of war and minimap

### Fog of war

Visibility is simulation-owned: `VisibilityMap` in `grus-sim` tracks three states per cell — **Unexplored**, **Explored** (remembered terrain, no live units), and **Visible**. Every unit projects a circular vision radius of **10 cells (Euclidean)** each tick, and completed buildings reveal from the boundary cells of their footprint (identical union to every cell — an interior cell's circle is always subsumed by the edge cells'). Standalone resource sources stay visible on the map once explored; enemy units and buildings require **current** vision to be shown, targeted, placed against, or gathered near. The boundary is enforced in the sim (combat acquisition, building placement, gather routing reject hidden targets — and unknown ids answer like missing ids so reject codes cannot probe fog) and again in the views: rendering, click picking, building inspection, combat tracers, and the minimap all hide what Team 1 cannot currently see — no hidden-enemy leak through any presentation path.

### Minimap

The minimap is a **128x96 runtime `ImageTexture`** (1 px per cell) redrawn from the Team-1 fog states: friendly markers always, enemy markers only while currently visible, resource markers once explored, plus the camera viewport rectangle. Left-clicking the minimap **recenters the camera only** — it never issues gameplay commands and never reveals anything.

### Runtime setup

The runtime match is a **human Team 1 survival battle against alien waves**. It starts with one Research Center, four Workers, and the authored resource stockpile; aliens attack after the 300-second preparation period. Fog of war, the minimap, combat, Defeat, restart and the session-free benchmark remain operational. Restart returns the match to a fresh **Start** state: initial fog (explored cells back to Unexplored, aliens hidden again).

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
```

The main Godot smoke exercises real input-derived click/box/additive selection, control groups, move, stop, HUD input shielding, zoom, pan, the 20 Hz cadence and interpolated presentation. It pins battlefield contracts: the boot camera frames the live Research Center, rendered `Ruins` match `GrusBridge.obstacle_rects()`, blocked-ground clicks receive `unreachable` feedback, and a human scout reveals an expansion resource before leaving it explored under fog. Every pre-wave Playing smoke checks the shared wave snapshot before exit or reset. The reset smoke requires exactly 4 unit views / 1 building view / 18 resource views, unique stable ids and the starting 200/300/150/0 stockpile. The economy smoke retains its full real-UI gather/build/train/rally/reject loop at 20×, split into two guarded preparation legs. The combat lifecycle smoke uses live controller attack-move and visible-alien right-click input, checks fog-gated markers and a Crystal kill reward, freezes combat while Paused, resumes into an alien-caused Defeat, and checks Result input lock and restart freshness. The dedicated wave smoke adds Start/Pause countdown freeze, HUD warning text, authoritative minimap-anchor pings and their expiry, plus labeled white-box checks of all four role silhouettes.

### Controls

The Start overlay shows a compact first-run controls label until the match starts:

```text
Select: left click / drag / Shift
Command: right click
Attack-move: A then right click
Stop: S
Pause/resume: Esc
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
- `Esc`: pause/resume the match (cancels an armed placement first)
- `Ctrl+1` through `Ctrl+9`: assign a control group
- `1` through `9`: recall a control group
- Mouse wheel: zoom
- Middle drag: pan the camera
- Minimap (bottom right): left-click recenters the camera on that map cell (view only — no commands, no reveal)
- Build buttons (bottom panel): arm placement for Bunkhouse/Depot/Barracks; the ground preview renders green when valid and red when blocked; left-click places, right-click or `Esc` cancels
- Train buttons: queue Worker on a selected Research Center, or Guard/Rifleman on a selected Barracks
- Idle button (top right): selects and navigates to the next idle worker

HUD controls consume their mouse events so UI interaction does not leak into world commands.

### Reset modes

- **Fixture reset** (`reset_fixture`, used by `reset_test.tscn`): despawns gameplay and reseeds the 4-worker / 1-Research-Center / 18-resource survival fixture with starting stockpiles and a fresh wave clock.
- **Benchmark reset** (`reset_benchmark_fixture`, used by `benchmark_200.tscn`): reseeds a **session-free** 200-unit survival workload with 50 human defenders and 150 actual Team-2 aliens, created through the shared wave spawn and AttackMove paths. It keeps one Research Center, the authored resource set and fog of war. A missing `MatchSession` reads as Playing, so the benchmark remains active without a lifecycle overlay. It is never used by the runtime match or other smokes.

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

CI runs three jobs. `rust-build-lint` gates formatting, Clippy, and workspace compilation. `unit-tests` enforces the 90% `grus-sim` line-coverage gate and runs the `grus-godot` library unit tests. `e2e` runs the Godot import, the bridge/reset/economy/combat-lifecycle/wave smokes, the Linux export validation, the bevy-e2e boot test against the exported build, and the 200-unit benchmark.

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

The benchmark fixture (50 human defenders and 150 aliens, see **Reset modes** above) exists only for `benchmark_200.tscn` after an explicit benchmark reset; the runtime match and other smokes run the one-base fixture. This workload replaces the previous mixed 100-vs-100 scenario, so historical benchmark measurements below are not comparable. The scene samples for a real-time duration set by `GRUS_BENCHMARK_SECONDS` (default 10, must be positive and finite): it waits for 200 live unit views with the exact team census and for the fog revision to advance, then records frame times plus live-unit and visible-in-tree unit counts (the visibility-flag render-load proxy), failing a sample that shows neither movement nor combat while the battle is still contested.

During measurement the runner issues no scripted orders: the wave director itself retargets aliens — Research Center first, then the lowest-id team-1 building, then the lowest-id live team-1 unit — so the session-free workload keeps alien intent through the ordinary command paths after the core falls. Workers retain their gather tasks; units are never respawned or healed, and attack cooldowns are unchanged. The liveness gate requires movement or combat in each window only while the battle is contested (both teams live); after one side is extinct, quiet windows are the real end state and pass. The earlier scripted `replay_benchmark_orders` seam was removed — its per-5-second route-planning bursts had been distorting the measured max/p99 frame times.

- **Current HPA-449, local headless 10 seconds (replay-free):** Godot 4.7.1, macOS 26.6.2 arm64, debug non-e2e GDExtension, `gl_compatibility`: 10.01 s / 771 frames, mean 12.980 ms (p50 6.888, p95 38.281, p99 48.150, max 84.717; 77.0 fps). Live units start 200 / min 163 / mean 184.1 / end 163; visible-in-tree min 72 / mean 96.3 / end 162; visibility revision 240. Exit 0 with the complete benchmark marker. With no scripted replay bursts, max frame time dropped from the replay-era 253–289 ms to 84.7 ms.

The two runs below were recorded **with the scripted order replay** (per-5-second route-planning bursts inside the sampled frames) — their max/p99 largely measured the ~196-route bursts, so they are historical, not current baselines.

- **Historical HPA-449, local headless 10 seconds (scripted-replay era, superseded):** Godot 4.7.1, macOS 26.6.2 arm64, debug non-e2e GDExtension, `gl_compatibility`: 10.00 s / 742 frames, mean 13.483 ms (p95 38.531, p99 54.077, max 253.382; 74.2 fps). Live units start 200 / min 165 / mean 184.6 / end 165; visible-in-tree min 71 / mean 95.9 / end 164. Exit 0 with the complete benchmark marker.
- **Historical HPA-449, rendered 60 seconds on Apple M1 Pro (scripted-replay era, superseded):** same OS/engine/debug extension, project runner, 1920×1080, OpenGL 4.1 Metal compatibility: 60.01 s / 4407 frames, mean 13.616 ms (p95 34.051, p99 52.784, max 288.669; 73.4 fps). Live units start 200 / min/end 150 / mean 155.5; visible-in-tree min/end 0 / mean 20.7. All liveness windows passed and the process exited 0. Fog hides survivors after all defenders die, so this is not a sustained 200-visible-unit render load. It is not the reference Linux release-export hardware-acceptance result. Full build metadata, limitations and regression evidence are in [HPA-449 verification](docs/hpa-449-wave-director-evidence.md).
- **Historical HPA-449, hosted Linux software GL (scripted-replay era, superseded):** [CI run 40366902630](https://github.com/cwchanap/Grus/actions/runs/40366902630) passed all jobs, including actual Linux exports, packaged smoke and exported-game E2E. Its 1920×1080 llvmpipe benchmark measured 10.16 s / 22 frames, mean 461.838 ms (p95 527.300, p99/max 672.910; 2.2 fps), live units 200 → 168 (mean 170.7), visible-in-tree mean 74.6. This is software-renderer CI contract evidence, not reference-host hardware performance.

- **CI regression:** 10 seconds (`GRUS_BENCHMARK_SECONDS=10` in the e2e job under software GL). It guards per-frame cost only — CI enforces no FPS floor.
- **Hardware acceptance:** 60 seconds on the reference Linux x86_64 host, on the release export packaged with the benchmark as its main scene (not the editor/debug project runtime):

```bash
GRUS_MAIN_SCENE=res://scenes/benchmark_200.tscn ./scripts/package_linux_x86_64.sh
GRUS_BENCHMARK_SECONDS=60 dist/grus-linux-x86_64-benchmark_200/Grus.x86_64
```

- **Historical HPA-449 one-off-order local run (superseded):** on 2026-10-26, Godot 4.7.1, macOS 26.6.2 arm64, headless `gl_compatibility`, debug GDExtension, `GRUS_BENCHMARK_SECONDS=10`: duration 10.00 s / 788 frames, mean 12.693 ms (p50 6.788, p95 37.433, p99 43.329, max 53.920; mean 78.8 fps), visibility revision 236. The real 50-human / 150-alien workload ran with live units start 200 / min 167 / mean 184.8 / end 167 and visible-in-tree units min 73 / mean 95.7 / end 166. The verbose process exited 0 with the benchmark marker; captured stdout also reported two ObjectDB instances at shutdown (`AudioStreamWAV` and `AudioStreamPlaybackWAV`), despite the existing `combat_fx.gd` stop/stream-clear teardown. This run predates scripted replay and is not a current baseline or a Linux release hardware-acceptance result.
- **Winter-workload measurements after the combat fixes (HPA-448 runtime-caveats follow-up) — per-tick sim profile:** the profile behind the earlier ~1 fps, on the same fixture fixture (Apple M1 Pro, macOS, arm64, release and debug builds of `grus-sim`). A scratch test, not kept in the repo, built the fogged benchmark world (`seed_skirmish`, `extend_with_benchmark_workload`, then a `VisibilityMap` inserted and refreshed every tick; the bridge's `seed_match_world` inserts the map *before* the workload commands, an ordering this probe does not reproduce and that is not expected to change per-tick cost) and timed each sim step with `Instant` over 200 ticks. The pre-fix profile was taken in its own session at host load 7–10; the snapshot-only and radius-first variants were alternated over three rounds at load 13–35, repeats within 5%. Before the fix (`163bcc1`), `acquire_target` re-resolved ~202 candidates through per-candidate ECS lookups for each of ~160 attack-moving units every tick, so `step_combat` alone cost **91–92 ms per tick in debug (3.5 ms in release)** against the 50 ms tick budget (82 ms / 2.8 ms on a fog-free world) — consistent with starving the 20 Hz catch-up loop. A per-tick candidate snapshot cut that to **9.3–9.5 ms (0.63–0.66 ms)** fogged, still 3× (debug) to 4–5× (release) the fog-free 3.0 ms (0.15 ms), because every candidate paid a fog lookup; testing the attack-move radius before that lookup brought it to **2.8–3.0 ms (0.135–0.139 ms)**, level with fog-free. `refresh_visibility`, run once per tick, is now the largest sim cost at 22.2–22.4 ms in debug and 1.4 ms in release (timed directly) and is unchanged by this follow-up.
- **Winter-workload measurements — windowed runs, pre-fix vs snapshot-only tree:** same fixture, scene-pinned 1920×1080 window, `gl_compatibility`, `GRUS_BENCHMARK_SECONDS=10`, Apple M1 Pro (macOS, arm64, Godot 4.7.1), real fogged workload, measured in one session on a host shared with unrelated apps (system load average 7–56; no per-run load was logged), so the figures are ranges, not a baseline. Alternating runs of the same benchmark on the pre-fix commit `163bcc1` and the snapshot-only tree (before the radius-first reorder), **debug** GDExtension: pre-fix 10 rendered frames, mean 1032 / 1025 ms (1.0 fps), 200 units live, one fog `Physics interpolation` warning per run; fixed 162 / 236 / 480 frames, mean 62.0 / 42.4 / 20.9 ms (16–48 fps), no warning. **Release** GDExtension on the snapshot-only tree (three runs): 523–675 frames, mean 14.8–19.1 ms (52–67 fps), p95 26–34 ms, max 187–403 ms; no release run exists for the pre-fix tree. Every post-fix run exited 0 with real combat: live units 200 → 188 (mean ≈196–197), visible-in-tree min 88 / mean ≈124–132, moved-unit counts (the log's `stage=active frames=N`, not rendered frames) positive in every window (59–178 in the snapshot-only release run whose windows were inspected). The two pre-fix runs show none of that: 200 units live throughout and a single moved-unit window.
- **Winter-workload measurements — windowed runs, final tree (radius-first acquisition):** same fixture and window, measured afterwards at host load 12–16, three runs each: **debug** 274–285 frames, mean 35.1–36.5 ms (27–28.5 fps), p95 55.6–62.6 ms, max 205–207 ms; **release** 884–897 frames, mean 11.2–11.3 ms (88–90 fps), p95 16.4–16.6 ms, max 128–150 ms. No `Physics interpolation` warning; every run exited 0 with live units 200 → 188 (mean 196.2–197.3) and visible-in-tree min 88 / mean 126.7–129.6; moved-unit windows were 55–178 in the release runs and at least 73 in the debug runs. Host load differed from the session above, so the gain over the snapshot-only figures cannot be attributed to the reorder alone — the per-tick profile is the controlled comparison. Run-to-run spread is host noise and is not characterized further; Linux CI's software-GL benchmark is a separate measurement, and reference-host hardware acceptance remains unmeasured.
- **Historical — pre-fix winter-workload re-baseline (HPA-448, superseded by the bullet above):** measured once on this repo's task-3 tree, before the combat-snapshot fix, in a real macOS window (Apple M1 Pro, arm64, Godot 4.7.1, `gl_compatibility`, **debug** GDExtension build, `GRUS_BENCHMARK_SECONDS=10`, scene-pinned 1920×1080 viewport): `duration_s=10.15`, **10 rendered frames**, mean **1014.781 ms** per frame (p50 1015.560, p95 1112.581, p99 1112.581, max 1112.581, mean 1.0 fps), `visibility_revision=83`. Workload: the session-free mixed 100-vs-100 fixture — 20 Workers / 40 Guards / 40 Riflemen per team on top of the two authored bases, 12 workers per side gathering the authored safe sources two-per-node and every combatant attack-moving the authored midfield waypoint. All 200 units stayed live (min/mean/end 200); the visible-in-tree proxy held at 121 (min/mean/end). Liveness windows passed on unit movement — the log's `stage=active frames=N` field is the moved-unit count, not a rendered-frame count. The ~1 fps cause was later profiled and fixed (see above). The workload changes again in HPA-449; treat these numbers as temporary markers, not a baseline contract.
- **Historical — interim post-AI re-baseline (HPA-445, superseded):** with the economic AI removed, the 60-second benchmark was measured locally (Apple M1 Pro, macOS 26.6.2, arm64; Godot 4.6.2 mono; then-current debug GDExtension build, `gl_compatibility`, headless): 60.01 s / 2343 frames, mean **25.611 ms** per frame (p50 6.263 ms, mean 39.0 fps), live units start 200 / mean 134.2 / end 87, visible-in-tree mean 81.5 (min/end 20). This ran the pre-relayout workload and a different engine version — it is retained as historical evidence only, not a current number. Details in [`docs/hpa-474-mvp-evidence.md`](docs/hpa-474-mvp-evidence.md).

Historical benchmark measurements (the HPA-445 interim post-AI re-baseline and the HPA-474 record) live in [`docs/hpa-474-mvp-evidence.md`](docs/hpa-474-mvp-evidence.md); the HPA-470 numeric baseline table was removed from this file — PR #1 and git history remain its historical record.
