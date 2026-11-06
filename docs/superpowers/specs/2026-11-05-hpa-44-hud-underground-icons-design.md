# HPA-44 — Winter Survival HUD and underground icon design

**Status:** Draft for review; no artwork or UI implementation in this commit.
**Linear:** [HPA-44 — HUD and underground room icons](https://linear.app/cwchanap/issue/HPA-44/grus-survival-art-hud-and-underground-room-icons)
**Parent:** [Winter Survival design contract](2026-10-21-winter-survival-design.md)
**Execution:** This planning draft becomes the **single asset-and-presentation implementation PR for HPA-44**. Do not open a second implementation PR.

## Goal and boundaries

Make the winter-survival HUD, underground room selection and existing build/train controls readable at small sizes with one consistent icon family. HPA-44 is the **separate art ticket**; it owns generating original icon assets and wiring them to already-shipped presentation. Do not bundle new 3D models (HPA-82/HPA-291), sound effects (HPA-269), gameplay features, or a new icon-generation framework.

The Linear issue explicitly requires PNG/SVG assets **committed under `godot/` and wired into HUD and underground panel**, while allowing changes to Godot scenes/GDScript only. No `crates/grus-sim`, bridge, catalogue, Rust, balance or simulation contract changes.

## Existing implementation seams (main at planning time)

- `godot/scenes/main.tscn` contains `HUD/EconomyPanel/EconomyLabel` (268 × 122 panel), `HUD/WaveLabel`, `HUD/ResearchPanel`, a 3-column `BuildGrid` of **nine** buildable structure Buttons, a 3-column `TrainGrid` of **three** human-unit Buttons, and `HUD/UndergroundPanel` (420 × 286).
- `godot/scripts/battlefield_controller.gd` caches the static catalogue in `_ready()`, updates resource/wave/research text in `_refresh_hud()` from existing read-only snapshots, and creates **six** underground room-kind Buttons in `catalogue_snapshot().underground.room_kinds` order. The Excavate Button is scene-authored. Preserve these UI entry points and kind metadata.
- The Economy HUD currently renders a **three-line `Label.text`**; `economy_smoke_test.gd` and `reset_test.gd` assert its node path, `Label` type, line grouping and readable numbers. `wave_smoke_test.gd` asserts `WaveLabel.text`, countdown and incoming-zone wording. `underground_smoke_test.gd` addresses room kinds by index and tests pressed/disabled state, Tab/Esc and live slot Buttons. **Those are existing test contracts that must be deliberately adapted for any `RichTextLabel` presentation change, while preserving live numeric meaning, layout, button order and behavior.**
- Godot project defaults to 1280 × 720, Compatibility renderer; `godot/assets/winter/` contains the existing cold-palette assets. Preserve research HUD, selection, input shielding and panel bounds; existing smokes already probe them.


## Asset inventory and names

Deliver **original illustrated raster PNG artwork**, not flat glyphs or SVG/vector assets. Ship transparent RGBA `.png` files under `godot/assets/ui/icons/`, grouped by category, with snake_case names. Runtime images are committed to Git; editable working files are optional. If images are AI-generated, retain a concise description of the prompts, cleanup/cropping, ownership and provenance in the icon README.

| Group | PNG names (without extension) | Count |
| --- | --- | ---: |
| `resources/` | `food`, `scrap`, `coal`, `crystals`, `heat` | 5 |
| `status/` | `wave_countdown`, `incoming_zone`, `research_progress`, `surface_cold` | 4 |
| `underground/` | `shelter`, `greenhouse`, `coal_vein`, `geothermal_tap`, `lab`, `deep_mine`, `excavate` | 7 |
| `buildings/` | `research_center`, `bunkhouse`, `depot`, `barracks`, `wall`, `gate`, `gun_turret`, `coal_furnace`, `flame_tower`, `crystal_lance` | 10 |
| `units/` | `worker`, `guard`, `rifleman`, `swarmer`, `spitter`, `brute`, `behemoth` | 7 |
| **Total** | | **33** |

Retain the previously agreed 33-artwork inventory. Only the currently actionable **nine build** and **three train** Buttons get these icons; do **not** invent Research Center construction or alien recruitment actions. The five catalogue-only icons (Research Center and four alien kinds) are art deliverables, not new UI features; inspect their scaled preview sheet and PNG import, not a nonexistent in-game Button. If scope is later reduced to 28, update the Linear issue and this inventory explicitly rather than silently dropping files.


## Visual contract — illustrated PNGs

- **Art direction:** cohesive small painted/illustrated game icons with recognizable objects and materials, subtle dimensional shading, lighting and texture (e.g. ration crate, rusted salvage, coal chunks, glowing crystals, furnace flame). Follow the winter-survival palette: cold blue-grey steel/ice, warm orange coal/furnace accents and restrained crystal cyan. **Not** flat monochrome symbols, UI emoji, stock clip-art or a vector-look SVG conversion.
- **Export contract:** make square **128 × 128 px RGBA PNGs with a transparent background**, consistent object framing, padding and edge treatment. Keep enough high-resolution detail for the art direction but remove thin details that disappear when scaled. Avoid baked lettering, numerals, UI frames and background squares. The committed PNGs are the Godot runtime textures; no SVG intermediate or runtime generator is required. Godot `.png.import` settings should be committed when needed for deterministic import.
- **Displayed size:** target **18–20 px** for economy/status HUD and **20–24 px** for action/room buttons. Constrain Godot `Button.icon` and inline HUD images to explicit display dimensions; a 128 px texture must **not** expand a 32–48 px Button or a ~65 px room-kind control. Check actual filtering/downscaling rather than assuming authored resolution guarantees legibility.
- Maintain distinct silhouettes at their **real displayed size**, not just at 128 px: avoid color-only distinctions, especially among six underground rooms. Check transparency/edge contrast on both light and dark backgrounds, including disabled and selected states.
- Numeric counts, ETA, prices, unlock requirements, progress and room effects stay as live text derived from existing snapshots and catalogue. Preserve readable names or tooltips where buttons allow them. Test at the project default **1280 × 720**; take a 1920 × 1080 check only if visual layout changes make it useful.
- **Prototype first:** before mass-producing the set, create *one resource*, *one building* and *one underground room* PNG, import them, wire those three samples in Godot and inspect a rendered 1280 × 720 frame. Settle the small-size art treatment and cramped room/toolbar spacing; only then author the remaining 30 assets. This avoids redoing a full painted icon set.


## Presentation integration

**Static Buttons:** Derive a PNG path from the existing wire kind using snake_case (e.g. `GunTurret` → `buildings/gun_turret.png`); use one tiny cached texture helper, not a manually duplicated kind-to-file table. Assign `Button.icon` to the nine `BuildGrid` and three `TrainGrid` Buttons and cap its displayed size. Preserve existing names, metadata, costs/tooltips, research locks, pressed callbacks, placement modes and training semantics. Existing button text remains.

**Underground:** Assign six room-kind icons in catalogue order when constructing Buttons in `_ready()`; assign `underground/excavate.png` to the existing Excavate Button. The 404 px-wide palette has ~65 px per kind: prefer **icon above the short label**, with a small bounded increase in row height if inspection proves necessary. Keep catalogue-derived tooltips, exclusive armed/disabled state, Tab/Esc and in-place row reuse. Also show the matching cached icon on **occupied/building room slot Buttons** in `_refresh_underground_slots`; clear the icon on empty/locked slots. No per-frame loading or slot reconstruction.

**HUD inline art:** Fixed-offset `TextureRect`s cannot stay adjacent to values that change width (e.g. `Food 9` → `Food 1200`); nor can one reliably precede the variable-length `Incoming:` suffix. Use Godot `RichTextLabel` with **inline scaled PNG images** for `HUD/EconomyPanel/EconomyLabel` and `HUD/WaveLabel`, preserving their node paths and original *semantic* snapshot-derived strings. Fit the existing economy panel with **icon + numeric value** for Food/Scrap/Coal/Crystals/Heat, retaining an accessible on-panel legend/tooltip for the icon order, and keep three logical economy lines where practical. Reuse the existing exact snapshot values, not a separate model. Update old smoke-test `Label` casts to `RichTextLabel` and compare parsed/plain text and value semantics, preserving viewport/bounds checks. Explicitly check width at 1280 × 720 and adjust only local panel cosmetics where needed.

For wave text, keep the same human-readable countdown, wave number, alien count and incoming zone names; render the incoming-zone PNG inline **only inside the existing nonempty incoming branch**, with no separate visibility flag. Research progress receives a small static PNG beside the existing `ResearchPanel` or its text, without relocating the ProgressBar/Analyze controls. `surface_cold.png` appears **only when surface efficiency is below 100%**, and the efficiency number remains readable; warm mode has no false cold warning.

**Ownership:** Load cached PNG textures once from `res://assets/ui/icons/` and reuse them when populating buttons/slots or assigning inline art. No added autoload, service, custom icon renderer, new snapshot fields, extra Rust bridge calls or gameplay calculations. Missing assets must not block gameplay input, but targeted existing smoke assertions should fail if required icons are absent.


## Acceptance and regression gates

1. All **33** original illustrated PNGs exist, are Godot-importable, have transparent backgrounds and form one coherent art family. Verify their silhouette/readability on a contact sheet at **18–24 px**, including the five not currently displayed in gameplay.
2. Actual in-game economy shows the five resource/heat icons and live values; status shows wave countdown, incoming-zone (only during a warning), research and surface-cold (only during a deficit). Layout stays within the 1280 × 720 viewport with no overlap/clipping and no misleading state icon. Keep the original data-derived text semantics.
3. Every existing build/train Button, room-kind/Excavate Button and occupied room slot shows its correct imported PNG. Empty room slots show none; all buttons keep real-click behavior, catalogue costs, research unlocks, armed/disabled states and tooltips. No new alien train or Research Center build actions.
4. Extend **existing** `smoke_test.gd` / `economy_smoke_test.gd`, `reset_test.gd`, `wave_smoke_test.gd` and `underground_smoke_test.gd` with icon/state assertions. Adjust intentional `RichTextLabel` type/parsed-text expectations without weakening numeric, line grouping, bounds, focus, input and room-order checks. No eighth smoke runner.
5. Show a **three-icon rendered prototype** before batch art production. Fresh Godot editor import and all seven headless gameplay smokes must pass; existing CI covers Rust gates, export/e2e and benchmark. Inspect real rendered frames (not just headless output), and record actual provenance/evidence rather than assuming it passed.
6. Changed runtime scope remains `godot/assets/ui/icons/`, Godot scenes/GDScript and existing presentation smoke tests/docs; **no `crates/`, Rust simulation, bridge or balance changes**.

## Deliberate exclusions

No 3D units/structures, SFX, animated icon state machine, responsive HUD rewrite, new build/train options, texture-atlas system, design-system plugin, major HUD layout relocation, localization, or gameplay tuning. The existing placeholders outside HPA-44 remain for their own art/audio tickets.
