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

- Coal Furnace: **2×2**, **80 Scrap**. The normal Worker placement/construction process applies. Its build duration and HP are new provisional catalogue values to settle in review; suggested **20 s / 300 HP**, not separate balancing knobs outside the catalogue. Placeable only by Team 1, as the one-player winter fixture requires.
- Each completed Furnace consumes **0.5 Coal per simulated second** while fueled. Coal is taken from the **existing Team-1 integer stockpile**, not a furnace inventory. One pooled fractional burn remainder in the heat state accounts for partial Coal; no per-furnace entity component, distribution rules or fuel hauling. Count completed furnaces in stable world iteration; only the count is needed, not their order.
- At the start of a Playing tick, a furnace is active if Team 1 has Coal; all completed furnaces can contribute on that tick. Deduct whole Coal due from the accumulated rate, capped by stockpile, then clear the fractional remainder if the stockpile becomes empty. On subsequent ticks with zero Coal, furnaces contribute and burn **zero**; refueling from gathering enables them on the next heat tick. A final consumption on a tick can supply that tick, but never subsequent coal-empty ticks. Include an epsilon for floating-point whole-unit thresholds.
- Compute **supply** and **demand** from completed Team-1 buildings only. The Research Center is the permanent +15 source while alive. Never charge a construction site, damaged-but-incomplete site, Wall, Gate, or Furnace itself. No time-of-day, ambient temperature, spatial radius or per-building heat propagation.
- `efficiency = if demand == 0 { 1.0 } else { (supply / demand).clamp(0.25, 1.0) }`. Heat is a live flow; **do not** add a heat stockpile, resource enum value, or new currency. Values in the snapshot are current supply, current demand and the exact fractional efficiency; percentages are only formatting.
- HPA-188 adds Flame Tower demand 3 and Crystal Lance demand 4, and scales surface research by this same value. HPA-288 later adds Geothermal Tap supply from underground. No placeholder gameplay implementations of either ticket now.

## Authoritative data flow

Retain the one Rust/Bevy `World`, single fixed-step chain, one typed catalogue and Godot-as-presentation split.

1. In `crates/grus-sim/src/catalog.rs`, extend `BuildingKind::ALL` with `CoalFurnace` and `BuildingSpec` with `heat_supply: u32` and `heat_demand: u32`. Supply/demand and furnace footprint/cost/timing are from `building_spec`. Add named catalogue constants for burn rate (`0.5`) and efficiency floor (`0.25`). No duplicate values in Godot.
2. Add `crates/grus-sim/src/heat.rs` with one `HeatState` resource (supply, demand, efficiency, fractional Coal burn) and `step_heat(&mut World, seconds)`. The step checks `gameplay_active`, counts completed player structures, debits the Team-1 stockpile, and writes the snapshot. Initialize the resource on the existing fixture/benchmark seed path; compute a zero-elapsed initial snapshot so Start already reports 15/10/100% without consuming Coal. Restart reseeds the fractional burn state.
3. Insert `advance_heat` in the current bridge `FixedUpdate` sequence **after `advance_waves` and before `advance_combat`**. Production runs later in the same tick and consumes the already calculated efficiency. Since construction completes after combat, its heat change takes effect at the next fixed tick; this is intentional at 20 Hz. Update `crates/grus-sim/tests/system_order.rs` to lock the dependency.
4. Production's existing FIFO queue accumulates `seconds * efficiency` only for completed Research Center and Barracks producers. Keep its blocked-at-100%, spawn clearance, population cap and rally behavior unchanged. Do not scale `step_construction` or `step_economy`. **Read-side fallback:** if `HeatState` is absent (for example, direct pre-heat simulation test worlds), both `step_production` and `step_combat` must read efficiency as `world.get_resource::<HeatState>().map_or(1.0, |h| h.efficiency)`; do not assume the resource exists or panic.
5. For **completed surface building attackers only**, decrement their existing `AttackCooldown` by `seconds * efficiency`. Normal unit attackers decrement by `seconds`. Keep nominal catalogue cooldown and the existing acquisition/strike/damage pipeline unchanged; this scales rate dynamically without multiplying damage or imposing a persistent rescaled cooldown when heating recovers. Future towers automatically reuse this path.

## Bridge and presentation

- Add read-only `GrusBridge.heat_snapshot()` returning `{ supply, demand, efficiency }` from `HeatState`. It must not change the burn remainder, Coal or simulation; do not independently recalculate gameplay heat in GDScript.
- Add a compact HUD label, for example **Heat 15 / 20 · surface 75%** (use whole-number percent, avoid showing rounded 100% when efficiency is actually lower). Expand/reposition the existing economy panel and command buttons as necessary for the seventh build button; keep the UI usable at 720p and the current panel layout.
- Extend the existing building view's team-color styling with a **visibly colder desaturated, whitened-icy** tint when the shared efficiency is below 1.0. Team 1's existing normal color is already blue (`Color(0.2, 0.55, 0.95)`), so do **not** use another similar blue; make the cold state clearly distinguishable by its lighter value and lower saturation. Pass the bridge's same value through `battlefield_controller.gd` to existing building views; apply to completed friendly surface buildings, including any new view created during a deficit. Restore its normal team color when heating recovers. Preserve shared Gate-part material tinting and existing health bars. Do not generate artwork or SFX.
- Add a Coal Furnace button to the existing BuildGrid (cost comes from the catalogue snapshot); use a simple recognizable primitive body via `building_view.gd`. No new building menu, bespoke fuel UI or custom effect.

## Boundaries and edge decisions

- Coal expenditure occurs in the heat step; a furnace construction finishing later in that fixed step is active next tick. Paused, Start and Result steps do **not** burn Coal or progress production/attacks. `reset_fixture` and `restart_match` return to initial 15/10 full efficiency and clean burn accumulator; benchmark uses the same seed.
- If the stockpile empties after a burn, furnace output stops on the **next** tick. Do not store negative fuel or resume with stale fractional debt. If zero demand, efficiency is 1.0 even when supply is zero.
- Demand counts completed buildings even when damaged, until they are destroyed; destruction is visible on the next heat tick. There is no heating reachability or geographic coupling.
- Draw the heat snapshot from sim state **once per frame** for the HUD and views; keep all heat math in Rust. Tests should use exact fixed-step deltas and tolerate only float precision, not wall-clock time.
- Build the complete feature and its README/CI evidence in **this PR**. No new app framework, global settings manager, temperature map, save migration, weather/blizzards, new tower/research features, underground rooms, generated models or audio. HPA-289 owns final balance.

## Acceptance

1. Warm Research Center alone: 15 supply / 10 demand / 100% efficiency. Completed structures add exact demand; sites do not; demand 0 yields 100%; severe deficit floors at 25%.
2. Zero Coal disables furnace output and burn; fractional 0.5 Coal/s is charged correctly with one and multiple furnaces; zero-fuel and refuel transitions do not leave phantom heat, negative Coal or carry-over debt.
3. At controlled equal simulated time, a cold Research Center/Barracks trains proportionally slower, a cold Gun Turret fires proportionally less often, and normal units/gathering/construction remain unchanged. Recovery to 100% returns normal cadence.
4. Pausing and restart preserve the expected fixed-state semantics; bridge snapshot and HUD match sim; cold tint toggles on existing and newly constructed views.
5. Rust formatting, Clippy, workspace tests, ≥90% sim line coverage, Godot heat smoke plus existing smokes, export E2E and current benchmark run green or report explicit environment limitations.
