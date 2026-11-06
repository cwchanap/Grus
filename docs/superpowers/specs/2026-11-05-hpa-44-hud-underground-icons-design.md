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
- The Economy HUD currently renders a **three-line `Label.text`**; `economy_smoke_test.gd` and `reset_test.gd` assert its node path, `Label` type, line grouping and readable numbers. `wave_smoke_test.gd` asserts `WaveLabel.text`, countdown and incoming-zone wording. `underground_smoke_test.gd` addresses room kinds by index and tests pressed/disabled state, Tab/Esc and live slot Buttons. **Icons must not replace or silently invalidate these contracts.**
- Godot project defaults to 1280 × 720, Compatibility renderer; `godot/assets/winter/` contains the existing cold-palette assets. Preserve research HUD, selection, input shielding and panel bounds; existing smokes already probe them.

## Asset inventory and names

Use authored, self-contained SVGs under `godot/assets/ui/icons/`, grouped by category, with predictable snake_case names. No external fonts, embedded bitmap images, scripts or remote resources in SVGs.

| Group | Assets | Count |
| --- | --- | ---: |
| `resources/` | `food`, `scrap`, `coal`, `crystals`, `heat` | 5 |
| `status/` | `wave_countdown`, `incoming_zone`, `research_progress`, `surface_cold` | 4 |
| `underground/` | `shelter`, `greenhouse`, `coal_vein`, `geothermal_tap`, `lab`, `deep_mine`, `excavate` | 7 |
| `buildings/` | `research_center`, `bunkhouse`, `depot`, `barracks`, `wall`, `gate`, `gun_turret`, `coal_furnace`, `flame_tower`, `crystal_lance` | 10 |
| `units/` | `worker`, `guard`, `rifleman`, `swarmer`, `spitter`, `brute`, `behemoth` | 7 |
| **Total** | | **33** |

The catalogue includes Research Center and four alien unit kinds, but the current UI intentionally has **no build/train buttons for them**. Commit their catalogue-covering icon assets without adding invalid actions or new gameplay controls. Wire all **nine actual build** and **three actual train** buttons.

## Visual contract

- A small, flat vector family: recognizable filled silhouette plus a simple distinguishing interior cut/mark, consistent stroke thickness and generous padding. Prefer the existing winter palette (charcoal/dark navy outlines, pale snow/ice highlights, restrained coal/rust/warm-orange and crystal-cyan accents), not fine-grained illustration.
- Design with a 24 × 24 viewBox, visible silhouette at **16–20 px** in the actual Godot HUD and on both light and dark UI surfaces. Use a dark edge plus light internal contrast rather than relying on theme tint. Never encode a distinction by hue alone; wave/incoming and each room need different shapes.
- No text or digits baked into the icons; numeric counts, ETA, costs, unlock requirements and slot effects remain live text. Use existing disabled/pressed styles, tooltips and human-readable labels for accessibility. Test at 100% display scale with 1280 × 720 and at the 1920 × 1080 benchmark resolution.
- Commit the SVG source as the runtime Godot asset; no separate export/atlas pipeline. Include `godot/assets/ui/icons/README.md` with the checklist, visual conventions and provenance. Godot import cache stays ignored.

## Presentation integration

**Static buttons:** Bind an icon to each current `BuildGrid` and `TrainGrid` Button from its existing `metadata/kind` or catalogue kind string (not a second button list). Preserve button names, costs/tooltips, research locks, pressed callbacks, placement modes and training semantics. Existing text labels remain.

**Underground:** Assign the six room-kind icons when constructing catalogue-ordered Buttons in `_ready()`; assign `excavate.svg` to the existing Excavate Button. Keep room kind labels/tooltips and exclusive pressed state, disabled locked levels, slot eligibility and in-place row refresh. Do not add dynamic per-frame icon loading or rebuild rows on every tick. The room palette is narrow: use a compact icon and existing short labels; adjust cosmetic width/spacing only after visual inspection.

**HUD:** Show all five resource/heat icons beside their live values within the existing economy surface, and surface-cold beside the existing efficiency text. **Keep `HUD/EconomyPanel/EconomyLabel` a `Label` with its current three-line content contract**; add adjacent cosmetic `TextureRect`s and only bounded positioning adjustments rather than replacing it with a new read-model or modifying the string formatting relied on by existing tests. Wave countdown and incoming-zone icons belong beside the existing `WaveLabel` (incoming visual visibility from the already-read wave snapshot). Show research-progress next to `ResearchPanel`; keep its `ProgressBar`, ETA, Analyze and control geometry intact. All icons are presentation only; never change countdown, heat, wave or research calculations.

**Ownership:** One small, explicit immutable mapping from current wire kind names to icon resource paths/textures in the existing controller or scene; initialize once. No new autoload, service, generalized UI library, new snapshot shape, or additional Rust bridge calls. Missing artwork must not make the input path reject commands; detect absent required icons in the icon smoke checks.

## Acceptance and regression gates

1. All **33** icons exist as valid Godot-importable assets, use the declared naming scheme, and are individually recognizable at 16–20 px against light/dark backgrounds.
2. Economy shows Food, Scrap, Coal, Crystals and Heat; HUD shows wave, incoming-zone, research and cold/surface icons with original live text, with no overlap or clipping at 1280 × 720. No misleading incoming warning when there are no incoming zones.
3. Every current build/train Button and underground kind/Excavate Button has its matching imported texture. Buttons still respond to real clicks and preserve catalogue-derived costs, research unlocks, armed/disabled states, and tooltips. No alien train or Research Center build button is invented.
4. Extend **existing** `smoke_test.gd` / `economy_smoke_test.gd`, `wave_smoke_test.gd`, and `underground_smoke_test.gd` with focused presence/bounds/state assertions. Avoid a duplicate icon smoke runner. Retain all pre-existing checks, including exact economy/wave strings and slot/button order.
5. Fresh Godot editor import succeeds; all seven headless gameplay smokes, native export/e2e and relevant repo CI pass. Inspect a real rendered frame for legibility; headless smoke passing alone does not prove visual quality. Record what was actually run.
6. Only `godot/assets/ui/icons/`, affected `godot/scenes/`, `godot/scripts/`, documentation and existing presentation smoke assertions change. No simulation/source catalogue edits.

## Deliberate exclusions

No 3D units/structures, SFX, animated icon state machine, responsive HUD rewrite, new build/train options, texture-atlas system, design-system plugin, major HUD layout relocation, localization, or gameplay tuning. The existing placeholders outside HPA-44 remain for their own art/audio tickets.
