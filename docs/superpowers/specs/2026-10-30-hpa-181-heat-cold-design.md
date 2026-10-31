# HPA-181 — Heat and cold: surface efficiency design

**Issue:** https://linear.app/cwchanap/issue/HPA-181/grus-survival-58-heat-and-cold-surface-efficiency  
**Parent:** [Winter Survival contract](2026-10-21-winter-survival-design.md)  
**Prerequisite:** HPA-450 / [merged PR #11](https://github.com/cwchanap/Grus/pull/11)  
**Status:** Proposed for review. This draft PR is the **single implementation PR** for HPA-181; implement and test here, not in a follow-up PR.

## Player outcome

Keeping a winter city warm becomes an economy-versus-defence decision. Completed buildings demand heat, the Research Center provides baseline heat, and completed Coal Furnaces burn Coal to supply more. A deficit reduces **surface building efficiency** down to a 25% floor, slowing Research Center/Barracks training and completed turret firing. Units, gathering, construction speed, movement, and future underground rooms stay unaffected. No cold damage or blizzards.

## Locked balance and rules

| Structure | Completed heat supply | Completed heat demand |
| --- | ---: | ---: |
| Research Center | +15 (no fuel) | 10 |
| Bunkhouse | 0 | 1 |
| Depot | 0 | 1 |
| Barracks | 0 | 3 |
| Gun Turret | 0 | 2 |
| Wall / Gate | 0 | 0 |
| Coal Furnace | +12 **only when fueled** | 0 |
| Any construction site | 0 | 0 |

- Coal Furnace: **2×2**, **80 Scrap**. The normal Worker placement/construction process applies. Its build duration and HP are **20 s / 300 HP** (same as the 2×2 Depot), set directly in the catalogue. Rebalance belongs to HPA-289. Placeable only by Team 1, as the one-player winter fixture requires.
- Each completed Furnace consumes **0.5 Coal per simulated second** while fueled. Coal is taken from the **existing Team-1 integer stockpile**, not a furnace inventory. One pooled fractional burn remainder in the heat state accounts for partial Coal; no per-furnace entity component, distribution rules or fuel hauling. Count completed furnaces in stable world iteration; only the count is needed, not their order.
- At the start of a Playing tick, a furnace is active if Team 1 has Coal; all completed furnaces can contribute on that tick. Deduct whole Coal due from the accumulated rate, capped by stockpile, then clear the fractional remainder if the stockpile becomes empty. On subsequent ticks with zero Coal, furnaces contribute and burn **zero**; refueling from gathering enables them on the next heat tick. A final consumption on a tick can supply that tick, but never subsequent coal-empty ticks. Include an epsilon for floating-point whole-unit thresholds.
- Compute **supply** and **demand** from completed Team-1 buildings only. The Research Center is the permanent +15 source while alive. Never charge a construction site, damaged-but-incomplete site, Wall, Gate, or Furnace itself. No time-of-day, ambient temperature, spatial radius or per-building heat propagation.
- `efficiency = if demand == 0 { 1.0 } else { (supply / demand).clamp(0.25, 1.0) }`. Heat is a live flow; **do not** add a heat stockpile, resource enum value, or new currency. Values in the snapshot are current supply, current demand and the exact fractional efficiency; percentages are only formatting.
- HPA-188 adds Flame Tower demand 3 and Crystal Lance demand 4, and scales surface research by this same value. HPA-288 later adds Geothermal Tap supply from underground. No placeholder gameplay implementations of either ticket now.

## Authoritative data flow

Retain the one Rust/Bevy `World`, typed catalogue and Godot-as-presentation split. **One ordered simulation function must be the canonical tick sequence, shared by runtime and integration tests**; currently the bridge and tests type out different sequences.

1. In `crates/grus-sim/src/catalog.rs`, append `BuildingKind::CoalFurnace` and add `heat_supply: u32` and `heat_demand: u32` to `BuildingSpec`. Set the furnace footprint/cost/duration/HP to **2×2 / 80 Scrap / 20 s / 300 HP**; keep the burn rate (`0.5`) and efficiency floor (`0.25`) as named catalogue constants. Extend the existing `buildings.rs::validate_placement` `Locked` check so Team 1 alone can place Furnaces. No new construction command, currency or Godot balance literals.
2. Add `crates/grus-sim/src/heat.rs` with `HeatState { supply, demand, burn_remainder }` (`Default` yields **0/0**), `HeatState::efficiency()` and `surface_efficiency(world)`. Compute the formula from supply/demand **on read**, rather than store a separate efficiency field: an unstepped default state and a world without `HeatState` must both yield **1.0**, never 0.0. `surface_efficiency(world)` is the one neutral-default accessor consumed by production, turret cooldowns and future research; do not duplicate `map_or` expressions at every caller.
3. `step_heat(&mut World, seconds)` recomputes completed Team-1 building supply/demand **every tick in every session phase**. Only the fractional Coal burn and stockpile debit are gated by `gameplay_active`. Insert `HeatState::default()` into the existing fixture/benchmark seed next to `WaveState`; reset replaces it, clearing the burn remainder. The first scheduled tick even in Start recomputes the 15/10/100% snapshot without burning Coal. No bespoke zero-time seed refresh is necessary.
4. Add `pub fn step_tick(world: &mut World, map: &mut GridMap, seconds: f32)` to `grus-sim`. Its sole authoritative order is **waves → heat → combat → movement → economy → construction → production → visibility**. In the bridge, replace the redundant wrappers with `apply_pending_commands → advance_sim → drain_route_reject_feedback`, where `advance_sim` reads `Time<Fixed>`, scopes the map resource once, and calls `step_tick`. Have `system_order.rs` and ordinary full-tick test helpers call `step_tick`; targeted tests intentionally stopping mid-tick may still call individual steps. Update `CLAUDE.md`'s chain and order-test claims during implementation.
5. All existing `ProductionQueue` instances belong to completed Research Center/Barracks producers; therefore multiply the single FIFO head-progress accumulation by `seconds * surface_efficiency(world)` without adding a per-kind filter. Keep blocked/spawn/population/rally rules and worker gathering/construction unchanged. For **completed surface building attackers only**, decrement `AttackCooldown` by `seconds * surface_efficiency(world)`; units use unscaled `seconds`, and `strike` continues to install the nominal cooldown. Heat computed at the start of a tick affects combat/production immediately; constructions completed later enter the next tick's heat totals.

## Bridge and presentation

- Extend the existing read-only `GrusBridge.economy_snapshot()` dictionary with `heat_supply`, `heat_demand` and `surface_efficiency`. It already supplies the HUD's stockpiles, population, idle workers and gather rate; **do not add a second `heat_snapshot()` call or new heat label**. GDScript only displays these Rust-derived values.
- Show **Heat 15 / 16 · surface 94%** as the **third line of the existing `EconomyLabel`**, retaining the first two currency/population lines verbatim. Expand the EconomyPanel height and shift its idle-worker button/neighboring panel where needed so the label's rendered bounds remain inside the panel and 1280×720 viewport. Avoid rounding a real deficit up to 100%.
- The cold indicator belongs to the Rust-to-Godot per-view projection, **not to per-building GDScript snapshot polling**. Add an `Update` system near `update_health_bars` that visits `(&Building, &GodotNodeHandle)` and sends `set_cold(bool)` to the view. Its catalogue-derived condition is **Team 1 && completed && building_spec(kind).heat_demand > 0 && surface_efficiency(world) < 1.0**. Because Walls, Gates and Furnaces do not consume heat and cannot be slowed by cold, **never tint them**. Every-frame value-gated updates handle late-attached views and heating recovery without additional state machinery in the controller.
- In `building_view.gd`, keep a cached cold boolean and make `set_cold` return immediately when unchanged. Reapply the cached normal-vs-cold material after the existing team metadata styling so initialization order cannot reset it. Use a **visibly desaturated, whitened-icy** tint instead of Team 1's normal `Color(0.2, 0.55, 0.95)`; restore that normal blue when warm. The Gate-part special case is unnecessary because zero-demand Gates never tint. Preserve health bars and fog visibility; no artwork or SFX work.
- Append the Coal Furnace button to the existing 3-column BuildGrid; prepare its height/adjacent TrainGrid position for **three build rows** (seven buttons now, nine after HPA-188) so they cannot overlap. Read its cost/footprint from the catalogue snapshot. Use the existing placement/Worker construction pipeline and a simple primitive body.

## Boundaries and edge decisions

- Coal expenditure occurs in the heat step only while Playing; a furnace completed during construction joins supply next tick. Paused, Start and Result steps **still recompute supply/demand**, but do **not** burn Coal or progress production/attacks. `reset_fixture` and `restart_match` reseed the clean accumulator; the first fixed tick then reports the initial 15/10 full efficiency. The benchmark uses the same seed.
- If the stockpile empties after a burn, furnace output stops on the **next** tick. Do not store negative fuel or resume with stale fractional debt. If zero demand, efficiency is 1.0 even when supply is zero.
- Demand counts completed buildings even when damaged, until they are destroyed; destruction is visible on the next heat tick. There is no heating reachability or geographic coupling.
- Read heat HUD fields through the existing **one economy snapshot per frame**; push each completed building's cold state from a Rust `Update` system (skip redundant view writes). No GDScript gameplay math or N snapshot calls per building. Tests should use exact fixed-step deltas and tolerate only float precision, not wall-clock time.
- Build the complete feature and its README/CI evidence in **this PR**. No new app framework, global settings manager, temperature map, save migration, weather/blizzards, new tower/research features, underground rooms, generated models or audio. HPA-289 owns final balance.

## Acceptance

1. Warm Research Center alone: 15 supply / 10 demand / 100% efficiency. Completed structures add exact demand; sites do not; demand 0 yields 100%; severe deficit floors at 25%.
2. Zero Coal disables furnace output and burn; fractional 0.5 Coal/s is charged correctly with one and multiple furnaces; zero-fuel and refuel transitions do not leave phantom heat, negative Coal or carry-over debt.
3. At controlled equal simulated time, a cold Research Center/Barracks trains proportionally slower, a cold Gun Turret fires proportionally less often, and normal units/gathering/construction remain unchanged. Recovery to 100% returns normal cadence.
4. Pausing and restart preserve fixed-state semantics; `economy_snapshot` and its third HUD line match sim; cold tint applies only to completed friendly heat-demanding buildings, and toggles on both existing and newly attached views.
5. Rust formatting, Clippy, workspace tests, ≥90% sim line coverage, existing economy smoke's real-input heat leg plus reset smoke, export E2E and benchmark run green or report explicit environment limitations. Rust tests own rate/pause/depletion math; Godot smoke owns button/placement/HUD/tint/burn/restart boundary checks.
