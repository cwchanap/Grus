# HPA-44 icon inventory

This checkpoint records the runtime PNG paths expected by the HUD and action controls. These runtime PNGs illustrate the existing HUD and action controls; the Research Center and four alien images are catalogue-only artwork.

All 33 files are original 128 × 128 transparent RGBA PNGs under `godot/assets/ui/icons/`, grouped by purpose. The Godot resource paths are:

## Resources and heat (5)

- `res://assets/ui/icons/resources/food.png`
- `res://assets/ui/icons/resources/scrap.png`
- `res://assets/ui/icons/resources/coal.png`
- `res://assets/ui/icons/resources/crystals.png`
- `res://assets/ui/icons/resources/heat.png`

## HUD status (4)

- `res://assets/ui/icons/status/wave_countdown.png`
- `res://assets/ui/icons/status/incoming_zone.png`
- `res://assets/ui/icons/status/research_progress.png`
- `res://assets/ui/icons/status/surface_cold.png`

## Underground (7)

- `res://assets/ui/icons/underground/shelter.png`
- `res://assets/ui/icons/underground/greenhouse.png`
- `res://assets/ui/icons/underground/coal_vein.png`
- `res://assets/ui/icons/underground/geothermal_tap.png`
- `res://assets/ui/icons/underground/lab.png`
- `res://assets/ui/icons/underground/deep_mine.png`
- `res://assets/ui/icons/underground/excavate.png`

## Buildings (10)

- `res://assets/ui/icons/buildings/research_center.png`
- `res://assets/ui/icons/buildings/bunkhouse.png`
- `res://assets/ui/icons/buildings/depot.png`
- `res://assets/ui/icons/buildings/barracks.png`
- `res://assets/ui/icons/buildings/wall.png`
- `res://assets/ui/icons/buildings/gate.png`
- `res://assets/ui/icons/buildings/gun_turret.png`
- `res://assets/ui/icons/buildings/coal_furnace.png`
- `res://assets/ui/icons/buildings/flame_tower.png`
- `res://assets/ui/icons/buildings/crystal_lance.png`

## Units (7)

- `res://assets/ui/icons/units/worker.png`
- `res://assets/ui/icons/units/guard.png`
- `res://assets/ui/icons/units/rifleman.png`
- `res://assets/ui/icons/units/swarmer.png`
- `res://assets/ui/icons/units/spitter.png`
- `res://assets/ui/icons/units/brute.png`
- `res://assets/ui/icons/units/behemoth.png`

The four alien unit and Research Center icons remain catalogue-only art; this inventory does not add gameplay controls. The contact sheet was inspected at 20 px on light and dark backgrounds; game-state visual verification is recorded separately.

Provenance: generated with OpenAI built-in image generation from individual subject prompts in a cohesive painted winter-survival style (cold steel and snow, orange warmth, cyan crystal light), with the food pilot supplied as a style reference for the remaining subjects. Outputs were cropped to their alpha bounds and downsampled to 128 × 128 with transparent padding. The committed PNGs are the runtime originals; `.png.import` files record Godot import settings.
