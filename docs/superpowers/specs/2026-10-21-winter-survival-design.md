# Grus — Winter Survival design contract

**Status:** Working repository specification for the Winter Survival pivot.
**Source:** Current Grus Linear issues HPA-445, HPA-448, HPA-449, HPA-450, HPA-181, HPA-188, HPA-288, and HPA-289, read 2026-10-20 (Pacific). The originally linked Claude artifact could not be fetched; this is a synthesis of the issue contracts, **not** a verbatim reproduction of that artifact.
**Execution:** One implementation PR per ticket. This document is introduced with HPA-445; later tickets update the relevant sections if their approved scope changes.

## 1. Player experience and constraints

A single-player, 2.5D winter-city survival RTS. A small human settlement protects its central Research Center on a frozen ruined-city battlefield. The player gathers Food, Scrap, and Coal, builds and heats defenses, fights escalating alien waves to earn Crystals, and completes research to win. Losing the Research Center ends the run. The long-term tuning target is a victory around minute 38–45 (roughly waves 20–25), **not** a hard game timer.

Keep the existing Godot 4 presentation / Rust Bevy ECS simulation split, fixed 20 Hz authoritative simulation, typed catalogue, command validation, shared occupancy map, and Start / Playing / Paused / Result lifecycle. Work with existing modules and plain simulation-step functions. No new state-management framework, multiplayer, procedural map, generic event bus, bespoke asset pipeline, or new renderer.

The old five-ticket human-versus-economic-AI MVP is complete; its Linear project description still describes the old game. For this pivot, the eight survival tickets and this design are the active scope. **No backwards compatibility is needed for renamed catalogue entries or removed commands.**

## 2. Delivery chain and deliberate cutover states

| Task | Responsibility | Deliberate intermediate state |
| --- | --- | --- |
| HPA-445 (1/8) | Remove Team-2 economic AI and selectable mirrored start; commit this contract | The authored Team-2 Town Center and villagers remain **passive** so existing combat/victory tests still work |
| HPA-448 (2/8) | Rename/trim the catalogue and centralize the city; rework obstacles/resources/HUD | Passive Team-2 base stays east until aliens replace it |
| HPA-449 (3/8) | Add deterministic wave director, aliens and Crystal rewards | Remove Team-2 base; win journey temporarily yields to an alien-defeat journey |
| HPA-450 (4/8) | Walls, gates, Gun Turret and walled-in targeting | Static defenses added; no heat penalty yet |
| HPA-181 (5/8) | Heat supply/demand and cold efficiency | Applies to surface production and tower fire; no blizzards |
| HPA-188 (6/8) | Research milestones, advanced towers and Victory | Brings back a full win path |
| HPA-288 (7/8) | Underground levels and room production | Supplies heat/research/resources without underground units or cold |
| HPA-289 (8/8) | Survival benchmark, deterministic balance guard and first tuning pass | Replaces the legacy 200-unit benchmark workload |

Each ticket owns its code, UI, tests and README edits in **one PR**. Art models/icons and combat/wave/ambience SFX are separate Linear art/audio tasks; code uses existing placeholders until assets are ready.

## 3. World, commands and session

- Keep the authored 128×96 flat simulation grid and a fixed-angle Godot 3D view. A single central human city is created in task 2; four accessible map edges become alien entry zones in task 3.
- Bevy ECS remains authoritative for stockpiles, entities, combat, visibility, occupancy, wave/research/heat/underground state and session results. Godot issues commands and renders typed snapshots only.
- Keep one fixed-step chain; new systems are added at their required points. Preserve the existing command → combat → movement → economy → construction → production → visibility → route feedback ordering wherever unaffected. Task 5 inserts heat ahead of combat and production; task 3 inserts waves in a deterministic position. System-order tests document the final ordering.
- Retain shared map occupancy / revision-driven navigation and existing fog-of-war semantics. Friendly vision reveals enemy entities; undiscovered/hidden enemies are not leaked via bridge, minimap or picking.
- Start freezes gameplay, Playing runs the simulation, Paused freezes gameplay and cosmetics, Result rejects mutations, and Restart returns to a fresh start. A session-free benchmark may continue to use the existing always-playing convention.
- Task 1 deliberately keeps both seeded teams but no AI decisions: Team 2 is a passive combat target. Task 3 removes the passive base once the wave system owns the alien side.

## 4. Economy and catalogue after task 2

Resources become **Food, Scrap, Coal and Crystals**, starting at **200 / 300 / 150 / 0**. Crystals have no map source at the re-skin stage; alien kills award them in task 3. Heat is a live flow, **not** stockpiled.

| Human unit | Cost | Producer |
| --- | --- | --- |
| Worker (renamed Villager) | 50 Food | Research Center |
| Guard (renamed Spearman) | 60 Food | Barracks |
| Rifleman (renamed Archer) | 40 Food + 40 Scrap | Barracks |

Cavalry, farms, Archery Range, Stable, Age 2 and the Age-up command disappear in task 2. Guard bonus vs Swarmer and Rifleman bonus vs Spitter are applied once the aliens exist.

| Structure | Cost or footprint | Purpose |
| --- | --- | --- |
| Research Center (old Town Center) | Retains initial construction semantics | Core to defend, trains Workers, later conducts research |
| Bunkhouse | 50 Scrap | +10 population |
| Depot | 75 Scrap | Worker dropoff |
| Barracks | 120 Scrap | Trains Guard / Rifleman |
| Wall | 1×1, 10 Scrap, 400 HP | Blocks pathing, no heat demand |
| Gate | 1×1, 30 Scrap, 600 HP | Walkable to humans, extra obstacle to aliens |
| Gun Turret | 2×2, 100 Scrap + 20 Coal | Range 8; bonus vs Spitter |
| Coal Furnace | 2×2, 80 Scrap | Generates surface heat by burning Coal |
| Flame Tower | 2×2, 120 Scrap + 60 Coal | Unlocked at 25% research; range 3, high damage, bonus vs Swarmer |
| Crystal Lance | 2×2, 150 Scrap + 40 Crystals | Unlocked at 50% research; range 10, bonus vs Brute |

Detailed base HP, rates, queue times and unspecified values remain in the single typed catalogue. Do not invent duplicate balance constants in Godot.

## 5. Alien wave director (task 3)

The wave system is a plain simulation module operating on the ECS World and GridMap, without RNG or a generalized scheduler.

- **300 seconds** of preparation after entering Playing; then launch a wave every **100 simulated seconds**, regardless of remaining enemies.
- Four entry zones. Waves **1–9** choose one zone rotating North → East → South → West; **10–19** choose two zones; **20+** choose three. Announce affected zones **30 seconds** before arrival, with HUD and minimap feedback.
- Threat-point budget for wave number w: **8 + 4w + 0.4w²**. Spend on a fixed deterministic mix: roughly half Swarmers, with Spitters/Brutes available only after their unlock waves. A Behemoth joins every tenth wave.
- Cap simultaneous live aliens at **150**. Any unspent budget from hitting the cap increases that wave's alien HP instead of spawning more bodies. Alien spawns bypass human population and production queues.
- Aliens attack-move toward the Research Center, using existing acquisition; guard against the walled-in case in task 4 by targeting a reachable structure.
- Each alien kill credits Crystals directly to the player's shared stockpile.

| Alien | Threat points | HP | Speed | Attack | First wave | Crystal reward |
| --- | ---: | ---: | ---: | --- | --- | ---: |
| Swarmer | 1 | 40 | 8 | Melee 6 / 1.0 s | 1 | 1 |
| Spitter | 3 | 70 | 6 | Range 6, 8 / 1.5 s | 4 | 2 |
| Brute | 6 | 300 | 4 | Melee 30 / 2.0 s | 8 | 5 |
| Behemoth | 40 | 2,000 | 3 | Melee 80 / 2.0 s | Every 10th wave | 50 |

The first pass uses placeholder meshes and existing combat feedback. Dedicated art and SFX are separate tasks.

## 6. Defenses, heat and cold (tasks 4–5)

Buildings with combat stats auto-acquire visible aliens and reuse combat damage/cooldown/event presentation. Generalize the combat attacker identity only as far as necessary to support buildings; do not add a separate tower combat engine.

- Gate passability is **team-aware in the same GridMap**: at HPA-450, extend the current free/blocked occupancy representation with a minimal explicit Gate state (free / blocked / gate), and pass the moving team to walkability and A* checks. A player unit treats gates as walkable; an alien treats them as blocked. Today's global `is_walkable` boolean and unparameterized `find_path` cannot express this alone. Gate placement/destruction bumps the existing map revision so affected paths replan; **do not create a second occupancy map**.
- Wall/Gate placement stays armed for multiple clicks. A finished builder proceeds to the nearest unfinished friendly construction site.
- Surface heat supply: Research Center **15**; active Coal Furnace **+12** while spending **0.5 Coal/s** through a fractional accumulator; Geothermal Taps from task 7 also contribute.
- Heat demand sums completed above-ground structures: Research Center **10**, Bunkhouse **1**, Depot **1**, Barracks **3**, Gun Turret **2**, Flame Tower **3**, Crystal Lance **4**. Walls, gates, furnaces and construction sites consume **0**.
- Surface efficiency = **min(1, max(0.25, heat_supply / heat_demand))**, with no heat-damage mechanic.
- Efficiency scales surface producer speed, tower firing cadence and (once available) surface research rate. **It never scales unit speed, worker gathering or underground output.** HUD reports supply/demand and efficiency; buildings visually tint blue when inefficient.
- Blizzards are deferred.

## 7. Research, victory and escalation (task 6)

A Research Center holds a **9,000-point** research bar. Its base generation rate is **2 points/second**, multiplied by surface efficiency. Model research as an additive sum of sources so underground Labs can contribute separately without cold penalties.

An **Analyze** action converts **10 Crystals to 90 research points**. Research milestones:
- **25%:** unlock Flame Tower.
- **50%:** unlock Crystal Lance.
- **75%:** all later waves receive **×1.5** threat budget.
- **100%:** the player wins; resolve the existing session result, with Research Center destruction remaining defeat.

Show research progress and ETA based on the current rate. Preserve deterministic milestones across pause/restart. Test complete victory in Rust; the full-length run is intentionally too long for CI Godot smoke timeouts.

## 8. Underground extension (task 7)

The Research Center grants an **Excavate** command, one level at a time. Level n costs **100 × 1.6^(n−1) Scrap** and half as much Coal (the issue's rounded examples: L1 100/50; L2 160/80; L3 256/128; L4 410/205). Dig time starts at **30 s**, increasing **15 s per level**. There is no depth cap.

Each level contains **four room slots**. Rooms cost **50–150 Scrap** depending on catalogue kind; they are paid upfront, built on timers, and have fractional production accumulators. No underground units, navigation, waves, or cold penalty.

| Level | Available rooms |
| --- | --- |
| L1 | Shelter (+10 population), Greenhouse (+0.5 Food/s) |
| L2 | Coal Vein (+0.4 Coal/s) |
| L3 | Geothermal Tap (+15 Heat, +5 for each additional level below L3), Lab (+1 research/s, independent of cold) |
| L4 | Deep Mine (+0.4 Scrap/s) |
| L5+ | No new kinds; already unlocked kinds remain available according to the room catalogue |

Shelters respect the existing maximum population cap. Tab opens a simple levels-and-slots panel with Excavate and room actions; placeholder visuals suffice. Restart discards underground progress.

## 9. Performance, balance and acceptance

Task 1 does **not** replace the current 200-unit scripted benchmark; it removes AI decisions while preserving existing scripted gather/attack-move workload, metrics and CI checks. Because the original Team-2 AI reassigns workers, removing it can change later time-weighted load and visibility means: **re-run the 60-second hardware benchmark and update README/evidence with observed numbers**, not historical AI-run measurements. Task 8 replaces the scenario with **150 live aliens against about 40 towers and 50 friendly units**, including wall/gate replanning pressure.

Keep the existing benchmark measurement contract: **10 s in CI**, **60 s for hardware acceptance** (GRUS_BENCHMARK_SECONDS). In task 8, add a deterministic Rust journey: a fixed authored defense survives through wave 10 but falls before wave 40. Catalogue-only tuning targets the minute-38-to-45 win window; owner playtests evaluate feel.

Every coding ticket must pass formatting, Clippy with warnings denied, Rust workspace tests, the **90% grus-sim line coverage gate**, relevant real Godot smoke scenes, and the current benchmark. Do not substitute unit tests for essential Godot bridge or presentation smoke coverage.

## 10. Explicit exclusions and decisions pending owner review

No multiplayer, multiplayer-grade determinism framework, networking, campaign, saves/migration layer, complex AI, new map modes, art generation inside coding PRs, blizzard simulation, second navigation grid, or generic game/survival orchestration framework.

This document follows the current Linear issue contracts rather than guessing any missing content in the inaccessible external design artifact. Resolve any discrepancy with that artifact deliberately in a review, without silently expanding HPA-445.