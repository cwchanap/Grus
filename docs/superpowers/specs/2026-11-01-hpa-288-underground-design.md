# HPA-288 — Underground levels and rooms: design specification

Status: **Planning draft** · 2026-11-01  
Issue: [HPA-288](https://linear.app/cwchanap/issue/HPA-288/grus-survival-78-underground-levels-and-rooms)  
Parent contract: [Winter Survival design, §8](2026-10-21-winter-survival-design.md)  
Dependency: HPA-188 / merged [Grus PR #15](https://github.com/cwchanap/Grus/pull/15)

## Goal and player experience

The player can open an underground panel with **Tab**, excavate a new level beneath the central Research Center, and buy rooms in four slots per completed level. Room construction proceeds automatically (no workers or pathing), and completed rooms continuously contribute to the shared economy, research, population or heat. The player can keep excavating as long as resources permit. Underground production is **not** slowed by surface cold.

This ticket owns the whole playable vertical slice—Rust simulation, queued commands, bridge snapshots, Godot placeholder panel, tests and README—in **one implementation PR**. This initial planning draft is that same PR, not a separate planning-only PR.

## Existing seams verified on `main`

- `crates/grus-sim/src/tick.rs::step_tick` is the canonical 20 Hz order: waves → heat → combat → movement → economy → construction → production → research → visibility.
- `PlayerCommand` and `apply_player_command` already enforce the common Playing/session gate, return `CommandResult`, and drive the bridge's queued-command/feedback path.
- `catalog.rs` owns all gameplay tuning; `ResourceStockpile::try_spend(Cost)` is an atomic four-currency spend, and `TeamEconomy` owns the stockpile.
- `HeatState` derives completed-building supply/demand in `step_heat`, with `surface_efficiency(world)` as the common cold multiplier. `research_rate(world)` currently contains only `RESEARCH_BASE_PER_SECOND * surface_efficiency`; HPA-188 explicitly left an additive Lab seam.
- `population_cap(world, team)` currently sums completed Research Centers/Bunkhouses and clamps to `MAX_POPULATION=100`.
- `seed_skirmish` installs `WaveState`, `HeatState` and `ResearchState` into a fresh world. Bridge boot, restart and benchmark use that fixture path. `catalogue_snapshot` is static; `economy_snapshot` and `research_snapshot` are dynamic. `battlefield_controller.gd` owns input and existing HUD.
- The old Linear Grus project description describes the completed skirmish MVP. The newer checked-in **Winter Survival** spec and HPA-288 ticket are the feature contract here.

## 1. Minimal authoritative model

Add `crates/grus-sim/src/underground.rs`, with no ECS entities, second map, scheduler, or new general framework:

- `Underground: Resource` holds `levels: Vec<UndergroundLevel>` plus `excavation: Option<Excavation>`.
- Each completed `UndergroundLevel` has exactly `[Option<Room>; 4]` slots. A `Room` stores `RoomKind`, elapsed build time, and a fractional production remainder. Completion is derived from elapsed time versus the catalogue's build duration. No persistent room IDs or separate production queues.
- `Excavation` tracks elapsed time; its target is always `levels.len() + 1`. Only one excavation may be active; excavated levels remain permanently available. Do not store a redundant current-depth field.
- Add `RoomKind::ALL`, `RoomSpec`, `room_spec(kind)` and named excavation/slot constants in `catalog.rs`. Use a small typed spec with `min_level`, `cost`, `build_seconds`, `population_capacity`, optional `(ResourceKind, units_per_second)`, `research_per_second`, and geothermal heat base/depth bonus. The kind catalogue is the only room-rule/price table; Godot reads it rather than replicating it.

Room catalogue (Scrap cost/build time below are **provisional first-pass choices** where the issue specified only 50–150 Scrap; HPA-289 owns tuning):

| Kind | Earliest level | Cost | Build time | Completed benefit |
| --- | ---: | ---: | ---: | --- |
| Shelter | 1 | 50 Scrap | 15 s | +10 population capacity |
| Greenhouse | 1 | 75 Scrap | 20 s | +0.5 Food/s |
| Coal Vein | 2 | 90 Scrap | 25 s | +0.4 Coal/s |
| Geothermal Tap | 3 | 120 Scrap | 30 s | +15 Heat at L3, +5 per deeper level |
| Lab | 3 | 150 Scrap | 30 s | +1 research point/s, unaffected by cold |
| Deep Mine | 4 | 150 Scrap | 35 s | +0.4 Scrap/s |

Availability is **minimum level**, not an exact-level-only assignment. At L5+ no new kind unlocks; any previously unlocked room kind remains buildable. Repeated kinds are allowed in separate empty slots. All room prices are fixed by kind, not scaled by depth. No underground heat demand.

## 2. Excavate and BuildRoom commands

Extend `PlayerCommand` with:

- `Excavate { issuer: TeamId, building: BuildingId }` — the Research Center's action. The Tab panel passes the authoritative Team-1 core ID from the read-only snapshot; selecting it in the 3D world is not required.
- `BuildRoom { issuer: TeamId, level: u32, slot: u8, kind: RoomKind }` — global underground panel action.

Both reuse the existing `apply_player_command` session gate. Commands are allowed only for Team 1 with a **living, completed** Team-1 Research Center; `Excavate` additionally validates the named core with the already-used commandable-building path (missing/ownership/completion/kind). For `BuildRoom`, reuse a tiny existing-core lookup rather than require selecting a building or add a second owner/state layer. The Research Center's death still causes the current Defeat behavior.

Validate every prerequisite **before** calling `try_spend` or mutating any slot/level. Accepted commands charge **once, up front**; a rejected request never changes any currency, progress or room. Two same-tick commands cannot buy the same slot or start two digs. Excavation and construction of rooms in already dug levels may proceed concurrently. Once started, jobs are non-cancelable in this MVP.

Append underground-only reject variants after the existing `RejectReason::Unexplored` to preserve all current numeric codes: `ExcavationInProgress`, `UndergroundLevelMissing`, `RoomSlotOccupied`. Continue using `Locked` for unavailable kinds/Team 2, `OutOfBounds` for slot 4+, `InsufficientResources` for failed spending, and existing core/ownership reasons where applicable. Reuse the bridge's normal feedback text/revision—no separate underground error channel.

### Excavation prices and durations

For target level `n >= 1`, use named `catalog.rs` constants and one Rust helper:

- Scrap = **round-half-up(100 × 1.6^(n−1))**.
- Coal = **round-half-up(Scrap / 2)**, using the already-rounded integer Scrap.
- Dig duration = **30 + 15 × (n−1)** simulated seconds.
- Assert examples in tests: **L1 100/50/30 s**, **L2 160/80/45 s**, **L3 256/128/60 s**, **L4 410/205/75 s**, **L5 655/328/90 s**.

No arbitrary depth cap. Use checked/widened calculations so implausibly large levels become **unaffordable**, never wrapped into free/cheap digs; `u32` stockpiles already make those depths unreachable. The snapshot should present an unavailable cost rather than overflow. Do not add BigInt or a numeric-framework dependency.

## 3. One fixed-step progression

Add `step_underground(world, seconds)` and insert it **after waves but before heat** in the one canonical `step_tick`:

**waves → underground → heat → combat → movement → economy → construction → production → research → visibility**

Reason: a just-completed Geothermal Tap contributes to this tick's heat supply, so surface turret/production/research consumers see the current heat total; a just-completed Lab is available to `step_research` later in that same tick. Existing heat→combat→production→research ordering stays intact, and lethal core combat still settles Defeat before research could finish Victory.

The new step:

1. No-op when not Playing or `seconds` is nonfinite/nonpositive; Start/Paused/Result do not advance timers or resources (session-free benchmark keeps the existing active convention). Do not advance jobs after the Research Center has been lost in a session-free world.
2. Advance the single paid dig timer; on completion append exactly one fresh four-slot level and clear excavation.
3. Advance independently paid room build timers. An unfinished room gives **no** benefit. A room that becomes complete may grant its nonperiodic effects (population/heat/research) on that tick, but only earns **per-second stockpile output from the next tick**—never retroactive production for the time before completion.
4. Completed Greenhouses, Coal Veins and Deep Mines accumulate fractional `rate × seconds` per room (use a consistent precise accumulator) and credit only whole resource units to the shared Team-1 stockpile. No workers or deposits are involved. Use safe stockpile additions; no floating-point rounding losses or credits while frozen.

No game timer, additional update systems, room entities, underground terrain/pathfinding, cold modifier, or background income service.

## 4. Hook into the existing systems

- `production.rs::population_cap`: add `+10` for each completed Shelter for **Team 1**, then clamp the combined surface/underground capacity at `MAX_POPULATION`. Incomplete Shelters and other teams never count. No separate underground population budget.
- `heat.rs::step_heat`: include the sum of completed Geothermal Taps in Team-1 **supply**; at level `n >= 3`, a tap contributes `15 + 5 × (n−3)`. No coal burn or surface demand. Existing surface efficiency, furnace burning and cold consumers are unchanged.
- `research.rs::research_rate`: return `RESEARCH_BASE_PER_SECOND * surface_efficiency(world) + sum(completed Labs × 1.0)`; the Lab term is **outside** the cold multiplier. Reuse this one function for passive point accrual and HUD ETA. Never bank Lab research separately in `step_underground`.
- `fixture.rs::seed_skirmish`: install `Underground::default()`; restart, reset and benchmark fixture creation inherit an empty underground automatically. No saved-game migration.

## 5. Godot bridge and usable placeholder panel

Add two typed queued-command methods to `GrusBridgeNode`: `excavate(research_center_id)` and `build_room(level, slot, kind)`. Parse room names from `RoomKind::ALL` as existing building/unit parsers do. Negative/invalid bridge arguments are rejected before enqueue; authoritative validation still occurs in Rust.

Keep the existing **static** `catalogue_snapshot()` as the sole source of room costs, durations, minimum levels and effects (add an `underground` section). Add a **dynamic**, read-only `underground_snapshot()` providing: current completed depth; living completed core ID or -1; active excavation target/progress/remaining; the next excavation's cost/time as calculated in Rust; and each level's four slots (empty, constructing with completion fraction, or completed with kind). No mutable shadow model or dig-price math in GDScript. Read the dynamic snapshot only while the panel is visible.

In `godot/scenes/main.tscn` and `battlefield_controller.gd`, Tab toggles a simple scrollable Underground panel with an **Excavate** button and one row per completed level containing four slot buttons. Empty slot buttons show eligible catalogue room choices in a lightweight popup; occupied slots show their room and progress. Show next dig cost/time/progress, level number and concise benefits. Disable actions while not Playing, but allow inspection; server-side command rejections remain final. Prevent panel mouse clicks/keyboard focus from issuing underlying battlefield orders. Esc closes the panel before the existing pause action. Keep visible command-feedback/status text and use existing Godot primitives—no art asset work or second UI system.

The existing Research Center Analyze action, research bar, wave HUD and build grid remain intact.

## 6. Acceptance and exclusions

**Rust correctness:** price/round-half-up examples and very-high-level safety; single paid excavation; no double-spend; 4-slot bounds and occupancy; min-level unlocks including L5+ reuse; concurrent room builds; freeze/restart; completion boundaries; per-room fractional resource output under varying fixed tick sizes; population cap; depth-scaled geothermal heat; Lab additive rate unaffected by cold; exact same-tick heat/research ordering and core destruction vs Victory. Extend `tests/system_order.rs` to exercise the canonical tick rather than a copied schedule.

**Real Godot boundary:** Tab and panel input; queued commands and feedback; excavate to L3 with real stockpile-gathering commands; construct a Geothermal Tap and Lab; assert the actual `economy_snapshot` heat supply and `research_snapshot` rate rise; pause freezes the timer, Restart clears levels. Keep the smoke bounded with the existing 20× simulation-speed mechanism, not test-only free-resource injection.

Run existing Rust workspace tests, 90% `grus-sim` line coverage, Clippy/rustfmt, Godot smoke suite, Linux exported-game/e2e and the current benchmark without changing its workload (HPA-289 owns the replacement). Update README and `CLAUDE.md` for controls and tick order. Record actual CI evidence before checking boxes.

**Out of scope:** additional level types or biome rules, mining units/pathfinding, per-room energy or cold, save/load, cancellation/demolition, research tree, new 3D underground scene, new models/UI icons/SFX, and final balance/performance tuning. Existing separate tickets HPA-291 (structures), HPA-44 (HUD/room icons), HPA-82 (units) and HPA-269 (SFX) continue independently; HPA-289 owns the final balance benchmark.
