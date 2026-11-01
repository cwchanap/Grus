# HPA-188 — Research timer, milestones, advanced towers and Victory

**Issue:** https://linear.app/cwchanap/issue/HPA-188/grus-survival-68-research-timer-and-victory  
**Parent contract:** [Winter Survival](2026-10-21-winter-survival-design.md), section 7  
**Dependency:** HPA-181 / [PR #14](https://github.com/cwchanap/Grus/pull/14), merged into main on 2026-10-31  
**Status:** Implemented locally on this branch (all six checkpoints landed; see the implementation plan for macOS arm64 evidence); required Linux x86_64 CI still pending. This PR is the **only PR** for HPA-188.

## Goal and boundaries

Make the winter survival match winnable by completing research at the central Research Center. Research also unlocks two stronger defensive towers and escalates later alien waves. Keep Godot as a snapshot-driven renderer/input layer and Rust/Bevy ECS as the one authoritative gameplay source. Reuse the existing catalog, building-placement path, combat, wave director, command feedback, and MatchSession Result overlay.

No underground Labs/rooms (HPA-288), balance pass (HPA-289), new building models/icons (HPA-291/HPA-44), or additional sound effects (HPA-269). Use the existing primitive building view/combat effects. No generalized research tree, technology framework, event bus, second game state, or mid-match persistence.

## Current implementation seams (verified against main)

- crates/grus-sim/src/tick.rs has one canonical step_tick: waves → heat → combat → movement → economy → construction → production → visibility. crates/grus-godot/src/lib.rs calls it from advance_sim.
- HeatState/surface_efficiency already supplies the active 0.25–1.0 surface multiplier. Research Center is the sole seeded Team-1 core; seed_skirmish and the bridge reset paths recreate the world.
- buildings.rs::validate_placement rejects locked kinds before charging or allocating anything. A BuildingSpec with combat is handled by the existing turret combat path, but step_construction currently initializes AttackCooldown only for GunTurret.
- waves.rs computes a deterministic integer budget with wave_budget(wave), buys a roster with plan_roster(wave) and calls that from launch_wave. Reuse this algorithm; adjust only the budget provided to it.
- GrusBridge already exposes wave_snapshot, economy_snapshot, catalogue_snapshot, building_snapshot and queued PlayerCommand feedback. battlefield_controller.gd already owns the build grid, HUD, and the existing Victory/Defeat overlay. main.tscn has a 3-column BuildGrid with 7/9 slots occupied.

## 1. Authoritative research state and tick behavior

- Add a small `crates/grus-sim/src/research.rs` module with a **single** `ResearchState { points: f64 }` resource (initially zero, capped at 9,000), plus `research_rate(world)`, `building_unlocked(world, kind)`, `add_research(world, points)` and `step_research(world, seconds)`. No per-building research table or duplicate milestone flags. Research advances whenever `gameplay_active` is true: the only Research Center is seeded complete and unplaceable, and its destruction already settles `Result` during combat. Reuse that existing lifecycle gate rather than adding an unreachable living/completed-core check.
- Keep research tuning in `catalog.rs`: `RESEARCH_TARGET_POINTS = 9000`, `RESEARCH_BASE_PER_SECOND = 2`, `ANALYZE_COST: Cost` (10 Crystals, other fields zero), `ANALYZE_RESEARCH_GAIN = 90`, `RESEARCH_WAVE_BOOST_PERCENT = 75` and numerator/denominator 3/2. Add `BuildingSpec.research_unlock_percent: u32` with `0` for always-available buildings, `25` for Flame Tower and `50` for Crystal Lance. Derive an unlock's point threshold from target × percent / 100 in one Rust helper, not a separate table of point constants, bridge flags or GDScript matches. Define the const `Cost` with explicit currency fields (non-const `Default::default()` cannot be used in a const initializer).
- While Playing, advance research by simulated seconds × (surface source + underground source). The surface source is 2 × `surface_efficiency(world)` points/s; the underground contribution is **zero in this ticket** and can be added by HPA-288 as an additive term without changing the cold multiplier. Convert the existing `Time<Fixed>` delta to f64 for accumulation. The test/benchmark session-free convention still applies; Start/Paused/Result freeze progress. A missing `ResearchState` reads as zero in read-only helpers, following the existing neutral-default resource pattern.
- Add step_research to the canonical tick **after combat, movement, economy, construction and production; before visibility**: waves → heat → combat → movement → economy → construction → production → research → visibility. Combat resolves Research Center destruction first if it occurs in the same fixed tick as passive research would finish; defeat therefore wins that same-tick tie. A successful Analyze command can separately settle victory immediately in the existing pre-tick command phase. Subsequent simulation steps honor the Result gate.
- Milestones are **derived from monotonic points** (no repeated mutable unlock events to drift). Entering or staying above a threshold does not repeatedly grant anything. At the target, call the existing resolve_result(world, TeamId(1)); the existing alien destruction path remains defeat. Once settled, no later tick or command overwrites the winner.
- Read-only rate and ETA derive from current heat and the source sum. ETA = remaining points / rate when rate is positive, else unavailable; at target it is zero. This ETA is a live estimate, not a countdown deadline. Do not round a positive sub-100% progress up to 100%.

## 2. Analyze command

- Add PlayerCommand::Analyze { issuer, building } and route it through apply_player_command, preserving the existing single gameplay_active/session gate and CommandResult feedback.
- Validate the requested `BuildingId` through existing `commandable_building_entity`, mirroring the minimal `production.rs::validate_enqueue` order: exists/visible → owned → complete → right kind (`ResearchCenter`). Spend `ANALYZE_COST` through `TeamEconomy` / `ResourceStockpile::try_spend` (with `can_afford` only if needed for display); insufficient Crystals are rejected in Rust, not pre-checked in GDScript. Reuse `BuildingMissing` / `NotOwned` / `Locked` / `WrongProducer` / `InsufficientResources`; do not add reject variants or a separate generic validation framework. No dedicated test is needed for a hand-created incomplete Research Center, which cannot arise in normal gameplay.
- Accepted Analyze **atomically** spends `ANALYZE_COST` (10 Crystals) from the shared stockpile and calls the same `add_research` helper as passive generation with `ANALYZE_RESEARCH_GAIN` (90 points), clamped at 9,000. Rejections spend nothing. It is not a production queue or a timed job; multiple Analyze commands are processed in order, with subsequent commands rejected once the first has settled Victory. At 100%, resolve victory immediately through the existing session helper.

## 3. Unlocks and advanced towers

| Threshold | Unlock/effect | Authoritative enforcement |
| --- | --- | --- |
| 25% (2,250 points) | Flame Tower | Placement validation rejects Locked before threshold |
| 50% (4,500 points) | Crystal Lance | Same server-side/sim-side gate |
| 75% (6,750 points) | Every **subsequently launched** wave gets 1.5× budget | Wave launch reads ResearchState; no retroactive adjustment |
| 100% (9,000 points) | Team-1 Victory | Existing MatchSession::Result and overlay |

- Append FlameTower and CrystalLance to `BuildingKind::ALL`, with typed `BuildingSpec` entries including `research_unlock_percent`. Both are Team-1-only, 2×2, and use existing builder, paid construction, footprints, shared tower combat, cold cooldown scaling, snapshot metadata and placeholder views. `research.rs::building_unlocked(world, kind)` compares the live points to the catalogue's percentage threshold; call it from the existing `validate_placement` Locked gate so both `placement_preview` and actual placement share the same authoritative rule.
- **Locked costs/roles from Linear:** Flame Tower = 120 Scrap + 60 Coal, range 3, bonus vs Swarmer, heat demand 3. Crystal Lance = 150 Scrap + 40 Crystals, range 10, bonus vs Brute, heat demand 4. Both have heat supply 0.
- **Provisional first-pass catalogue values** (tune only when implementing, then defer detailed balancing to HPA-289): Flame Tower 350 HP, 25-second construction, 20 damage every 1.0 s, +10 vs Swarmer; Crystal Lance 350 HP, 30-second construction, 30 damage every 1.5 s, +15 vs Brute. Keep existing ranged tracer/hit cosmetics rather than adding bespoke weapons or art.
- Generalize the single GunTurret-only `AttackCooldown` initialization at construction completion to any completed `BuildingSpec` with combat. In `godot/scripts/building_view.gd::_style_kind`, add FlameTower and CrystalLance to the existing GunTurret primitive styling arm (distinct art is tracked separately). No duplicate tower targeting, cooldown, kill-credit or projectile logic.
- The UI can disable locked build buttons using catalogue-provided `unlock_points` and the dynamic research points, with a generic per-frame update so buttons unlock after the HUD initializes. The sim still validates both `placement_preview` and actual delayed `PlaceBuilding`; GDScript holds no hard-coded unlock percentages, per-kind conditions or separate mutable unlock flags.

## 4. Later-wave escalation

- The normal integer threat-point formula, wave timing, N/E/S/W entry zones, 150 live-alien cap and overflow-health behavior remain unchanged.
- At each `launch_wave`, inspect the current `ResearchState` against the catalogue's `RESEARCH_WAVE_BOOST_PERCENT` and pass either `wave_budget(wave)` or **floor(base_budget × 3 / 2)** (using widened/saturating arithmetic) to `plan_roster(wave, budget)`. Change the existing one-argument roster function and its callers/tests to accept the explicit budget; do not add a second roster builder or multiply counts/HP after purchase. The existing cap and overflow-HP behavior then consume the boosted roster unchanged.
- Do not stack the multiplier if research remains above 75%. Waves already launched retain their committed roster and any overflow-HP calculation. With waves earlier in step_tick than research, a threshold crossed by passive progress during a tick affects the **next** wave launch, not a wave launched earlier that tick.

## 5. Godot bridge and player UX

- Keep **static** research configuration in `GrusBridge.catalogue_snapshot()` (its `research` dictionary contains `target_points`, `analyze_cost` using the existing four-currency `cost_dict`, and `analyze_gain_points`; each building entry includes catalogue-derived `unlock_points`). Read that once at UI initialization. Add a small **dynamic** `GrusBridge.research_snapshot()` returning only `points`, `rate_per_second` and `eta_seconds`, fetched once per HUD frame. Derive percentage and generic button eligibility from those values; no duplicated milestone booleans, costs or mutable GDScript research state.
- Expose GrusBridge.analyze(building_id) as a queued PlayerCommand using the existing feedback revision and rejection text. Use a completed, selected Team-1 Research Center as the Analyze action context; do not give arbitrary buildings or units an Analyze command.
- Add a compact `ProgressBar` with points/percent and live ETA beneath the WaveLabel, plus an Analyze button with its **catalogue-derived** Crystal cost/point reward. Enable Analyze whenever the match is Playing and the selected building is the owned Research Center; **do not disable on Crystal affordability**. Rust reports `InsufficientResources` via the existing feedback path, which the player and smoke must be able to observe. Keep the existing EconomyPanel, WaveLabel, and pause/result overlays intact.
- Add FlameTower and CrystalLance buttons to the existing three-column BuildGrid, filling the **two remaining slots of the existing 3×3 layout**. The existing generic button loop binds catalogue-sourced costs, construction times and `unlock_points` (0 = always unlocked); refresh `button.disabled` on every HUD update from `research_snapshot.points`, not just in `_ready`. Rust placement validation remains authoritative. Use current building views/cold tint and tracer feedback. No new command panel, custom per-kind wiring or UI framework.
- Victory uses the already implemented session_snapshot winner_team=1 / SessionLabel “Victory!” path. Defeat on Research Center destruction is untouched.

## 6. Acceptance, testing and review boundaries

**Pure Rust tests** (fast deterministic simulated time, no sleeps): passive rate × heat efficiency; freeze at Start/Paused/Result; restart reset; 9,000-point cap and Victory; each catalogue-derived building unlock boundary crossed by passive ticks and Analyze; locked placement rejects before spend; accepted tower placement/construction/combat/cold cadence; Analyze ownership/wrong-kind/missing/insufficient-Crystals failures with unchanged stockpile; atomic spend/advance; 75% multiplier at launch only, explicit-budget roster equivalence before boost, exact integer rounding and 150-alien cap/overflow; 100% research vs lethal combat on the same tick. Do not create impossible absent/incomplete Research Center scenarios solely to test redundant gates.

**Godot boundary smoke**: real research HUD bar and ETA advance under 20× simulation time without reaching 9,000. In a fresh Playing match with zero Crystals, select the Research Center, press the **enabled** real Analyze button and assert feedback `InsufficientResources` (existing reject code 8), Crystals remain zero and research points do not jump by 90. Verify button/command feedback, catalogue-derived lock presentation and Restart resetting progress. The successful Analyze spend/credit path is covered by Rust tests, without an additional ten-kill combat journey or test-only resource injection. Full 9,000-point completion at 2 pts/s takes 4,500 simulated seconds (225 real seconds at 20×); full Victory and milestone escalation stay in Rust, not a 45–90 s Godot CI smoke.

Maintain the existing rustfmt, warnings-as-errors Clippy, workspace tests, >=90% grus-sim line coverage, Godot smoke suite, Linux export/e2e and benchmark CI gates. Any smoke changes must use real gameplay commands and headless exit-status + sentinel checks. Update README research/tower/victory instructions and the system-order docs/CLAUDE.md if the executed order changes.

## Deferred work

- HPA-288: underground excavation, Geothermal Taps and Lab +1 research/s independent of cold.
- HPA-289: comprehensive survival performance and balance tuning; provisional tower numbers are not a locked final balance.
- HPA-291, HPA-44 and HPA-269: dedicated structure/UI artwork and SFX production, without asset generation embedded in this coding ticket.
