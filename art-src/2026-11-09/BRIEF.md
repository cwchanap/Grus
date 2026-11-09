# HUD redesign art brief (Codex batch, 2026-11-09)

Raw art for the "Frostline" HUD redesign and painted in-game buildings. Generate every entry below with Codex, run as:

```bash
codex --model gpt-6.1-terra -c model_reasoning_effort=xhigh
```

(`codex exec` with the same flags is fine for a non-interactive run.) Attach these existing icons as style references on every request, so the batch matches the shipped HPA-44 set:

- `godot/assets/ui/icons/resources/food.png`
- `godot/assets/ui/icons/buildings/gun_turret.png`
- `godot/assets/ui/icons/buildings/research_center.png`

## Output rules

- One image per entry, saved at the path given (relative to this folder).
- **Transparent background** (RGBA PNG). If transparency is impossible, use a flat solid `#FF00FF` background with nothing else on it, and say so in the report.
- No text, letters, numbers or logos anywhere in any image.
- After saving, downscale with `sips -Z 512 <file>` (banners: `sips -Z 768`) so the repo stays small.
- Icons and sprites are square (1024 × 1024 before downscale). Banners are wide (1536 × 1024).

## Shared style (prepend to every prompt)

> Painted game art in a cohesive winter-survival style: cold blue-grey steel and packed snow, warm orange lamp and fire glow as the accent colour, cyan crystal light for research and alien tech. Soft painterly shading, clean readable silhouette with a slight dark outline, lit from the upper left. Match the attached reference icons exactly in rendering style, palette and level of detail. No text, letters or numbers. Transparent background.

## HUD icons (15) → `icons/`

Single centred subject, must read clearly at 32 px.

| File | Subject |
| --- | --- |
| `icons/move.png` | A frosted steel ground waypoint disc with a bold cyan chevron arrow pointing forward. |
| `icons/stop.png` | A raised gloved hand in front of a frosted steel octagonal plate with a warm orange rim. |
| `icons/attack_move.png` | A rifle crossed with a combat knife over a forward chevron arrow, red-orange glow. |
| `icons/gather.png` | A pickaxe leaning on a small sack of scrap metal pieces, snow on the sack. |
| `icons/rally.png` | A tattered orange banner flag on a steel pole planted in a snow mound. |
| `icons/pause.png` | Two thick vertical frosted steel bars side by side (a pause symbol), warm orange rim light. |
| `icons/population.png` | Three huddled survivors in winter parkas with fur hoods, chest up, close together. |
| `icons/health.png` | A dented steel field medkit with a bright green cross, frost on the edges. |
| `icons/time.png` | A brass hourglass with frosted glass and glowing orange sand. |
| `icons/research_rate.png` | A cyan glowing crystal flask with a beam of light rising upward like an arrow. |
| `icons/idle_workers.png` | A worker's orange hard hat resting on a shovel stuck upright in snow. |
| `icons/locked.png` | A heavy iron padlock covered in frost and small icicles. |
| `icons/mouse_left.png` | A computer mouse seen from above, frosted steel-grey body, the LEFT button glowing cyan. |
| `icons/mouse_right.png` | The same mouse seen from above, the RIGHT button glowing cyan. |
| `icons/mouse_wheel.png` | The same mouse seen from above, the scroll wheel glowing cyan. |

## Screen art (3) → `screens/`

| File | Size | Subject |
| --- | --- | --- |
| `screens/emblem.png` | 1024 × 1024 | Game emblem: a round frosted steel badge showing a stylised crane (the bird) flying over a domed city, a cyan research beacon on the dome, warm orange glow under it. |
| `screens/victory_banner.png` | 1536 × 1024 | A wide horizontal cloth banner in cyan and white with steel end caps, warm sunrise light breaking through snow clouds behind it, icicles melting. Leave the centre of the banner plain for a title laid on top later. |
| `screens/defeat_banner.png` | 1536 × 1024 | A wide horizontal torn, scorched banner in dark red and charcoal, frost and alien claw slashes, wisps of smoke. Leave the centre plain for a title laid on top later. |

## Building sprites (9) → `sprites/`

These replace the low-poly building models in play. Add this to the shared style for every sprite:

> Single building sprite for a top-down real-time strategy game, drawn from a high three-quarter view: the camera faces straight north and is tilted 54 degrees down, orthographic. NOT an isometric 45-degree view: the front wall faces the viewer squarely and the square footprint appears as a wide rectangle at the bottom of the image. The base fills the full width of the frame with a thin margin. Snow on roofs and ledges, warm orange lit windows, cold steel and timber. Only a soft contact shadow directly under the base. No ground tile, no surrounding scene.

| File | Footprint (cells) | Subject |
| --- | --- | --- |
| `sprites/research_center.png` | 4 × 4 | The city's Research Center: a large fortified lab with a central cyan-glowing glass dome, antennas and a beacon, heated vents. The most important building, clearly the biggest. |
| `sprites/barracks.png` | 3 × 3 | Military barracks: a long timber-and-steel hall with a pitched snowy roof, front door with a lantern, weapon racks, a small orange banner. |
| `sprites/bunkhouse.png` | 2 × 2 | Small bunkhouse: a log-and-sheet-metal cabin, smoking chimney, warm windows. |
| `sprites/depot.png` | 2 × 2 | Resource depot: an open-front steel shed with crates, a scrap pile and coal sacks, a small crane arm. |
| `sprites/coal_furnace.png` | 2 × 2, tall | Coal furnace: a squat brick-and-iron furnace with a tall smokestack, glowing orange firebox door, coal heap beside it. |
| `sprites/gun_turret.png` | 2 × 2 | Gun turret: an armoured steel turret with twin barrels on a round sandbagged concrete base. |
| `sprites/flame_tower.png` | 2 × 2 | Flame tower: a squat armoured tower with a flamethrower nozzle and fuel tanks, small orange pilot flame. |
| `sprites/crystal_lance.png` | 2 × 2 | Crystal lance: a tall steel pylon with a cyan alien crystal held at the tip, energy coils, glowing. |
| `sprites/construction_site.png` | any | A generic construction site: square foundation with wooden scaffolding, a few steel beams, stacked crates and a tarp, light snow. No finished building. |

## Report back

List every file written, which ones needed the `#FF00FF` fallback, and any entry that failed.
