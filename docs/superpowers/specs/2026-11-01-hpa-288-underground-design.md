# HPA-288 — Underground levels and rooms: design specification

Status: **Planning draft; revised after GitHub design review** · 2026-11-02  
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
- Each completed `UndergroundLevel` has exactly `[Option<Room>; 4]` slots. A `Room` stores `RoomKind`, `progress_seconds: f32`, and a `production_remainder: f64`. Like existing unit production, dig and room construction snap to the required time using `PROGRESS_EPSILON`, rather than relying on exact repeated `0.05_f32` additions. For each completed resource-producing room, accumulate `f64` output and extract whole units with a tiny `f64` epsilon following the existing furnace-burn convention. No persistent room IDs or separate production queues.
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

- `Excavate { issuer: TeamId }` — one global underground action, independent of selected buildings or any Research Center ID round-trip.
- `BuildRoom { issuer: TeamId, level: u32, slot: u8, kind: RoomKind }` — build inside a completed level.

Both reuse the existing `apply_player_command` session gate. Both require `issuer == TeamId(1)` and the same **living Research Center** fact checked by `research.rs::living_research_core(world)`. **Expose that existing helper as `pub(crate)` at the start of the first implementation checkpoint**, before compiling `step_underground`; both player commands reuse it in the next checkpoint. The core is seeded complete and cannot be placed; there is no value in separately validating a completed-core ID for this global panel. An in-session destroyed core already causes Defeat and therefore `SessionLocked`; a session-free test/benchmark must still refuse action when the core is gone.

**Wire indexing is fixed:** `level` is **1-based depth everywhere** (L1 is `1`; `0` rejects `OutOfBounds`), and `slot` is **0-based** (valid `0..=3`; `4+` rejects `OutOfBounds`). Snapshot level rows carry an explicit `level` number. Do not expose `levels` vector indices as the public level IDs.

Validate every prerequisite **before** calling `try_spend` or mutating any slot/level. Accepted commands charge **once, up front**; a rejection changes no currency, progress or room. Two same-tick commands cannot buy the same slot or start two digs because existing queued commands are applied sequentially. Excavation and construction of rooms in already completed levels may proceed concurrently. Once started, jobs are non-cancelable in this MVP.

Append **only** `ExcavationInProgress` after the existing `RejectReason::Unexplored` to preserve numeric reject codes. Reuse `Occupied` for a taken slot, `OutOfBounds` for a missing/zero level or invalid slot, `Locked` for unavailable kinds/Team 2 or missing core, and `InsufficientResources` for failed spending. Feedback already formats `RejectReason` generically; no new mapping or error channel.

### Excavation prices and durations

For target level `n >= 1`, use named `catalog.rs` constants and one Rust helper:

- Scrap = **round-half-up(100 × 1.6^(n−1))**.
- Coal = **round-half-up(Scrap / 2)**, using the already-rounded integer Scrap.
- Dig duration = **30 + 15 × (n−1)** simulated seconds.
- Assert examples in tests: **L1 100/50/30 s**, **L2 160/80/45 s**, **L3 256/128/60 s**, **L4 410/205/75 s**, **L5 655/328/90 s**.

No arbitrary depth cap. For positive values, `f64::round()` gives the required half-up rounding; `as u32` saturates instead of wrapping even when an exponential becomes infinity. For Coal use `scrap.div_ceil(2)`; for duration use `saturating_mul` and `saturating_add`. Use an exponent expression that handles the full `u32` depth range (such as `powf(f64::from(n - 1))` after checking `n >= 1`), not an unchecked `u32 -> i32` conversion. Return a plain `(Cost, u32)` and render the Rust-computed value in the snapshot; a very-deep level's saturated cost remains practically unaffordable in the normal economy. No special `Option`, overflow UI state or BigInt machinery.

## 3. One fixed-step progression

Add `step_underground(world, seconds)` and insert it **after waves but before heat** in the one canonical `step_tick`:

**waves → underground → heat → combat → movement → economy → construction → production → research → visibility**

Reason: a just-completed Geothermal Tap contributes to this tick's heat supply, so surface turret/production/research consumers see the current heat total; a just-completed Lab is available to `step_research` later in that same tick. Existing heat→combat→production→research ordering stays intact, and lethal core combat still settles Defeat before research could finish Victory.

The new step:

1. No-op when not Playing or `seconds` is nonfinite/nonpositive; Start/Paused/Result do not advance timers or resources (session-free benchmark keeps the existing active convention). Reuse `living_research_core` so a session-free world with a destroyed core cannot continue underground work.
2. **First credit already-completed rooms**: for each resource-producing room completed at tick entry, add `rate × seconds` to its `f64` remainder, extract whole units with the furnace-style epsilon and credit the shared stockpile. This precedes timer advancement so the completion tick never awards retroactive income.
3. Advance the single paid dig timer with the existing production-style snap-to-required tolerance; once complete append exactly one fresh four-slot level and clear excavation.
4. Advance independently paid room timers with the same completion tolerance. A previously unfinished room contributes nothing that tick to periodic stockpile output. Rooms completing now may contribute their nonperiodic population/heat/research effects when those existing systems read the completed state later this tick.

Extract a small saturating `ResourceStockpile::add(ResourceKind, amount)` from `economy.rs::deposit_carry` and reuse it for both worker deposits and underground room credits. Do not duplicate the kind-to-currency match or use unchecked additions.

No game timer, additional update systems, room entities, underground terrain/pathfinding, cold modifier, or background income service.

## 4. Hook into the existing systems

- `production.rs::population_cap`: add `+10` for each completed Shelter for **Team 1**, then clamp the combined surface/underground capacity at `MAX_POPULATION`. Incomplete Shelters and other teams never count. No separate underground population budget.
- `heat.rs::step_heat`: include the sum of completed Geothermal Taps in Team-1 **supply**; at level `n >= 3`, a tap contributes `15 + 5 × (n−3)`. No coal burn or surface demand. Existing surface efficiency, furnace burning and cold consumers are unchanged.
- `research.rs::research_rate`: return `RESEARCH_BASE_PER_SECOND * surface_efficiency(world) + sum(completed Labs × 1.0)`; the Lab term is **outside** the cold multiplier. Reuse this one function for passive point accrual and HUD ETA. Never bank Lab research separately in `step_underground`.
- `fixture.rs::seed_skirmish`: install `Underground::default()`; also explicitly remove `Underground` inside `crates/grus-godot/src/lib.rs::clear_gameplay_world` beside `HeatState` and `ResearchState`, keeping restart/reset/benchmark resource teardown symmetric. Extend existing bridge restart/reset tests rather than inventing a second reset path. No saved-game migration.

## 5. Godot bridge and usable placeholder panel

Add two typed queued-command methods to `GrusBridgeNode`: parameterless `excavate()` and `build_room(level, slot, kind)`. Both supply `TeamId(1)` internally. Parse names from `RoomKind::ALL` via the existing `kind_name_str` pattern; reject negative/invalid bridge inputs before enqueue, and let Rust validate affordability/level/slot authoritatively.

Keep the existing **static** `catalogue_snapshot()` as the sole source of room costs, durations, minimum levels and effects (add an `underground` section). Add a **dynamic**, read-only `underground_snapshot()`: current completed depth; active excavation target/progress/remaining; next dig's Rust-computed cost/time; and each completed level row with an explicit **1-based `level`** and four **0-based slots** (empty, constructing with progress, or completed with kind). **No core-ID field, -1 sentinel or recalculation of dig cost in GDScript.** Fetch the dynamic snapshot only while the panel is visible.

In `godot/scenes/main.tscn` and `battlefield_controller.gd`, Tab toggles a scrollable Underground panel with an **Excavate** button, one row of six room-kind buttons, and one row per completed level containing four slot buttons. **Keep the global two-click flow without selected-level state.** A kind Button becomes selectable when completed depth reaches its catalogue `min_level`; clicking it arms that kind. While a kind is armed, enable **only empty slots on completed 1-based levels `level >= room_spec(armed_kind).min_level` and only while Playing**. Empty slots are disabled until a kind is armed; too-shallow slots stay disabled and visibly explain their required depth (for example, `Requires L3`). Occupied/constructing slots are never actionable. An eligible empty-slot click queues `build_room`; Rust separately rejects stale or otherwise invalid requests with `Locked`, and frontend eligibility never replaces command validation. When the armed kind changes, refresh existing slot eligibility from static catalogue data and dynamic level rows, with no second state owner or selected-level step. Kind tooltips show catalogue cost, build time and effect. This uses the **same ordinary Buttons** already exercised by the Godot smoke helpers, not a new popup/window primitive. Occupied slots show kind and build progress. Show next dig cost/time/progress and the current depth.

**Key routing:** handle Tab in `_input`, not `_unhandled_input`, and mark it handled, because Godot's `ui_focus_next` can consume Tab when a button has keyboard focus. Also handle Esc in `_input` **only while the panel is open**, marking it handled and closing the panel ahead of the existing Esc-to-cancel/pause flow. Other key commands continue through the existing `_unhandled_input`. Set the panel's mouse filter to **STOP** so its GUI interactions cannot fall through to battlefield selection/placement; no parallel mouse-gating code. Disable mutating controls when not Playing but allow read-only inspection; Rust command rejections stay authoritative. The panel does not need the Research Center selected.

Keep visible command feedback in the existing HUD `CommandStatus`. The existing Analyze action, research bar, wave HUD and build grid remain intact. No art asset work or second UI system.

## 6. Acceptance and exclusions

**Rust correctness:** price/round-half-up examples and very-high-level saturation; single paid excavation; no double-spend; 1-based level and 0-based slot rejection; min-level unlocks including L5+ reuse; concurrent room builds; freeze/restart; snap-to-required completion boundaries; per-room `f64` fractional output under varying tick sizes; saturating shared stockpile credit; population cap; depth-scaled geothermal heat; Lab additive rate unaffected by cold; exact same-tick heat/research ordering and core destruction vs Victory. Extend `tests/system_order.rs` to exercise the canonical tick rather than a copied schedule. Add a **gather-funded L3-before-wave-1 feasibility test** using `seed_skirmish`, real `PlayerCommand::Gather`, and the canonical tick before designing the Godot smoke. L1–L3 excavation plus Tap+Lab costs **786 Scrap / 258 Coal**, against the fixture's **300 Scrap / 150 Coal**; gather at least **486 Scrap + 108 Coal**. Serial dig and room timer floor is **165 simulated seconds**; the first wave launches at **300 seconds**. Measure how much margin remains rather than assuming the full journey fits; use real additional Worker training if needed.

**Real Godot boundary:** Tab and panel input **including Tab after clicking a focused room/Excavate button and Esc while the panel is open**; queued commands and feedback; excavate to L3 with real stockpile-gathering commands after the Rust feasibility gate. With L1–L3 visible, **arm Lab and verify L1/L2 slots are disabled and cannot queue or spend, while an L3 empty slot can construct it; then arm Shelter and verify an L1 empty slot becomes eligible again**. Keep a direct Rust `BuildRoom` below-minimum-level `Locked` rejection test independently. Construct a Geothermal Tap and Lab; assert the actual `economy_snapshot` heat supply and `research_snapshot` rate rise; pause freezes the timer, Restart clears levels. Size the CI 45 s/90 s timeout from measured Rust feasibility and smoke elapsed time. Keep the smoke bounded with the existing 20× simulation-speed mechanism, not test-only free-resource injection.

Register the new standalone smoke as a **dedicated timed/sentinel-checked CI step** in `.github/workflows/ci.yml` **and** in its teardown-backtrace scene list. Run existing Rust workspace tests, 90% `grus-sim` line coverage, regular and `grus-godot --features e2e` Clippy/rustfmt, Godot smoke suite, Linux exported-game/e2e and the current benchmark without changing its workload (HPA-289 owns the replacement). Update README and `CLAUDE.md` for controls and tick order. Record actual CI evidence before checking boxes.

**Out of scope:** additional level types or biome rules, mining units/pathfinding, per-room energy or cold, save/load, cancellation/demolition, research tree, new 3D underground scene, new models/UI icons/SFX, and final balance/performance tuning. Existing separate tickets HPA-291 (structures), HPA-44 (HUD/room icons), HPA-82 (units) and HPA-269 (SFX) continue independently; HPA-289 owns the final balance benchmark.
