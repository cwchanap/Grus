# Static unit models — HPA-82

Seven original low-poly glTF 2.0 models, authored for Grus in Blender 5.2.1 LTS. Humans wear blue cold-weather equipment; aliens use warm chitin, legs, and spikes. The worker carries a pack and pick, the guard a shield and spear, and the rifleman a long rifle. Alien scale increases from Swarmer through Spitter and Brute to Behemoth.

Each GLB is a single mesh and surface with an embedded palette texture (no external downloads). Import settings disable name-based physics and generated LODs; the post-import script keeps the palette nearest-filtered without mipmaps at RTS zoom. Origins are grounded and each asset has fewer than 2,000 triangles. `unit_view.gd` keeps the selection and health presentation on the existing unit root.

Regenerate from the repository root:

```sh
blender --background --factory-startup --python-exit-code 1 --python scripts/build_unit_assets.py
```

The script asserts grounding, height and triangle budgets. Models are static; movement and combat effects remain owned by the existing presentation.
