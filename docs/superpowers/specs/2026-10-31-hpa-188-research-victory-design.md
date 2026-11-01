# HPA-188 — Research timer, milestones, advanced towers and Victory

**Issue:** https://linear.app/cwchanap/issue/HPA-188/grus-survival-68-research-timer-and-victory  
**Parent contract:** [Winter Survival](2026-10-21-winter-survival-design.md), section 7  
**Dependency:** HPA-181 / [PR #14](https://github.com/cwchanap/Grus/pull/14), merged into main on 2026-10-31  
**Status:** Design proposal only. This documentation draft PR is the **only PR** for HPA-188; implement and verify in the same branch before marking ready for review.

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

- Add a small grus-sim/src/research.rs module with a **single** ResearchState resource storing accumulated research points (f64; initialized to 0, capped at 9,000). One central Research Center means no per-building research ownership table. A missing/dead/incomplete Team-1 Research Center cannot generate research.
- Keep constants in catalog.rs: RESEARCH_TARGET_POINTS = 9000, RESEARCH_BASE_PER_SECOND = 2, ANALYZE_CRYSTAL_COST = 10, ANALYZE_RESEARCH_GAIN = 90, and milestone thresholds 25% / 50% / 75% (derived from the target or named integer point thresholds of 2,250 / 4,500 / 6,750). No magic amounts in Godot or waves.rs.
- While Playing, advance research by simulated seconds × (surface source + underground source). The surface source is 2 × surface_efficiency(world) points/s; the underground contribution is **zero in this ticket** and can be added by HPA-288 as an additive term without changing the cold multiplier. Use the actual Time<Fixed> delta passed to step_tick; the test/benchmark session-free convention still applies. Pause/Start/Result must freeze progress.
- Add step_research to the canonical tick **after combat, movement, economy, construction and production; before visibility**: waves → heat → combat → movement → economy → construction → production → research → visibility. Combat resolves Research Center destruction first if it occurs in the same fixed tick as passive research would finish; defeat therefore wins that same-tick tie. A successful Analyze command can separately settle victory immediately in the existing pre-tick command phase. Subsequent simulation steps honor the Result gate.
- Milestones are **derived from monotonic points** (no repeated mutable unlock events to drift). Entering or staying above a threshold does not repeatedly grant anything. At the target, call the existing resolve_result(world, TeamId(1)); the existing alien destruction path remains defeat. Once settled, no later tick or command overwrites the winner.
- Read-only rate and ETA derive from current heat and the source sum. ETA = remaining points / rate when rate is positive, else unavailable; at target it is zero. This ETA is a live estimate, not a countdown deadline. Do not round a positive sub-100% progress up to 100%.

## 2. Analyze command

- Add PlayerCommand::Analyze { issuer, building } and route it through apply_player_command, preserving the existing single gameplay_active/session gate and CommandResult feedback.
- Validate the requested BuildingId with the existing commandable_building_entity seam, verify Team-1 ownership, completed ResearchCenter kind, and 10 available Crystals in TeamEconomy. Reuse BuildingMissing / NotOwned / Locked / WrongProducer / InsufficientResources rather than introducing new reject codes.
- Accepted Analyze **atomically** spends 10 Crystals from the shared stockpile via ResourceStockpile::try_spend and grants 90 points immediately, clamped at 9,000. Rejections spend nothing. It is not a production queue or a timed job; multiple accepted Analyze commands are processed in order. At 100%, resolve victory immediately through the same helper used by passive generation.

## 3. Unlocks and advanced towers

| Threshold | Unlock/effect | Authoritative enforcement |
| --- | --- | --- |
| 25% (2,250 points) | Flame Tower | Placement validation rejects Locked before threshold |
| 50% (4,500 points) | Crystal Lance | Same server-side/sim-side gate |
| 75% (6,750 points) | Every **subsequently launched** wave gets 1.5× budget | Wave launch reads ResearchState; no retroactive adjustment |
| 100% (9,000 points) | Team-1 Victory | Existing MatchSession::Result and overlay |

- Append FlameTower and CrystalLance to BuildingKind::ALL, with typed BuildingSpec entries. Both are Team-1-only, 2×2, use the existing builder, paid construction, footprints, shared tower combat, cold cooldown scaling, snapshot metadata, and existing placeholder views.
- **Locked costs/roles from Linear:** Flame Tower = 120 Scrap + 60 Coal, range 3, bonus vs Swarmer, heat demand 3. Crystal Lance = 150 Scrap + 40 Crystals, range 10, bonus vs Brute, heat demand 4. Both have heat supply 0.
- **Provisional first-pass catalogue values** (tune only when implementing, then defer detailed balancing to HPA-289): Flame Tower 350 HP, 25-second construction, 20 damage every 1.0 s, +10 vs Swarmer; Crystal Lance 350 HP, 30-second construction, 30 damage every 1.5 s, +15 vs Brute. Keep existing ranged tracer/hit cosmetics rather than adding bespoke weapons or art.
- Generalize the single GunTurret-only AttackCooldown initialization at construction completion to any completed BuildingSpec with combat. No duplicate tower targeting, cooldown, kill-credit or projectile logic.
- The UI can show disabled/locked build buttons but the sim must validate both placement_preview and the actual delayed PlaceBuilding command. Existing research unlocks never need a separate unlock flag in the Godot scene.

## 4. Later-wave escalation

- The normal integer threat-point formula, wave timing, N/E/S/W entry zones, 150 live-alien cap and overflow-health behavior remain unchanged.
- At each launch_wave, inspect the current research fraction and use either the original wave_budget(wave) or an integer **floor(base_budget × 3 / 2)** budget for waves launched after the 75% milestone. Use a widened/saturating intermediate as wave_budget already does. Feed the effective budget into the existing roster-purchase logic rather than multiplying entity counts or enemy HP after the fact.
- Do not stack the multiplier if research remains above 75%. Waves already launched retain their committed roster and any overflow-HP calculation. With waves earlier in step_tick than research, a threshold crossed by passive progress during a tick affects the **next** wave launch, not a wave launched earlier that tick.

## 5. Godot bridge and player UX

- Add a read-only GrusBridge.research_snapshot() (points, target, fraction, effective points/s, estimated seconds remaining, milestone/unlock booleans, Analyze cost/gain). Avoid caching mutable gameplay state in GDScript; fetch once per HUD frame.
- Expose GrusBridge.analyze(building_id) as a queued PlayerCommand using the existing feedback revision and rejection text. Use a completed, selected Team-1 Research Center as the Analyze action context; do not give arbitrary buildings or units an Analyze command.
- Add a compact ProgressBar with points/percent and live ETA beneath the WaveLabel, plus an Analyze button with its catalogue-provided Crystal cost/point reward. Disable it outside Playing, when no completed Research Center is selected or when fewer than 10 Crystals are available. Keep the existing EconomyPanel, WaveLabel, and pause/result overlays intact.
- Add FlameTower and CrystalLance buttons to the existing three-column BuildGrid. This fills the **two remaining slots of its existing 3×3 layout**; no new command panel or UI framework is needed. Tooltips use catalogue costs and construction times. UI disables build buttons below their research thresholds, while Rust placement validation is the authority. Use the current building views/cold tint and tracer feedback.
- Victory uses the already implemented session_snapshot winner_team=1 / SessionLabel “Victory!” path. Defeat on Research Center destruction is untouched.

## 6. Acceptance, testing and review boundaries

**Pure Rust tests** (fast deterministic simulated time, no sleeps): passive rate × heat efficiency; RC missing/destroyed; freeze at Start/Paused/Result; restart reset; 9,000-point cap and Victory; first threshold crossed by passive tick and by Analyze; unlocks remain idempotent; locked placement rejects before spend; accepted unlocked tower placement/construction/combat/cold cadence; Analyze ownership/incomplete/insufficient-Crystals failures with unchanged stockpile; atomic spend/advance; 75% multiplier at launch only, exact integer-rounding and 150-alien cap/overflow behavior; 100% research vs lethal combat on the same tick.

**Godot boundary smoke**: real research HUD bar and ETA advance under 20× simulation time without reaching 9,000, the Analyze button on a selected Research Center spends Crystals earned from actual alien kills, command feedback and snapshot move together, unlocked/locked button state follows the Rust snapshot, and restart resets progress. Reuse existing smoke helpers/journeys without test-only resource injection. Full 9,000-point completion at 2 pts/s takes 4,500 simulated seconds (225 real seconds at 20×), so complete Victory and milestone escalation are Rust tests; do not put that duration into the 45–90 s Godot CI smoke.

Maintain the existing rustfmt, warnings-as-errors Clippy, workspace tests, >=90% grus-sim line coverage, Godot smoke suite, Linux export/e2e and benchmark CI gates. Any smoke changes must use real gameplay commands and headless exit-status + sentinel checks. Update README research/tower/victory instructions and the system-order docs/CLAUDE.md if the executed order changes.

## Deferred work

- HPA-288: underground excavation, Geothermal Taps and Lab +1 research/s independent of cold.
- HPA-289: comprehensive survival performance and balance tuning; provisional tower numbers are not a locked final balance.
- HPA-291, HPA-44 and HPA-269: dedicated structure/UI artwork and SFX production, without asset generation embedded in this coding ticket.
