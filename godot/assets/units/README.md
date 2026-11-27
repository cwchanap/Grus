# Painted unit sprites — HPA-1198

Every unit renders as a painted `Sprite3D` (`godot/scripts/unit_view.gd`), matching the painted buildings. Each of the seven kinds has two poses in `painted/`: `<kind>.png` (base) and `<kind>_action.png` (attack, or a gather/build swing for Workers), where kind is `worker`, `guard`, `rifleman`, `swarmer`, `spitter`, `brute` or `behemoth`.

Art contract (HPA-1210, regenerated and verified by `art-src/2026-11-26/process.py`): 512×512 RGBA, figures face right, both poses of a kind share one canvas scale, and the feet plus a baked soft contact shadow sit at the bottom center, 24 px above the bottom edge. Replacing art means overwriting these files under the same contract; no code changes.

Import settings match the painted buildings: lossless, mipmaps on (512 px figures are drawn at roughly 25–170 px tall and shimmer without them), no alpha-border fix so the base mip stays byte-identical to the PNG.

Presentation rules:

- The sprite pitches to face the fixed camera, keeps real depth testing, is unshaded and casts no 3D shadow (the baked shadow is the contact shadow).
- Each kind is sized by its painted figure, not its canvas: `unit_view.gd`'s `SIZES` table sets the figure height in world units on the camera-facing plane, plus the selection ring and health bar scale. One world unit is 20 px at 1920×1080 default zoom.
- Motion is code only: a left/right flip by heading, a small walk bob and waddle, the action pose on each attack event and each Worker swing, a short red flash when hurt, and a topple-and-fade corpse spawned by `combat_fx.gd` on death.
- Picking tests the sprite's opaque texels through the shared alpha cache in `building_view.gd`, including flipped sprites.
