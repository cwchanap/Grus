# HPA-44 — Illustrated PNG HUD and underground icons implementation plan

**Linear:** [HPA-44](https://linear.app/cwchanap/issue/HPA-44/grus-survival-art-hud-and-underground-room-icons)
**Spec:** [HPA-44 design](../specs/2026-11-05-hpa-44-hud-underground-icons-design.md)
**Branch / PR:** `hpa-44-survival-ui-icons`; this **one draft PR** becomes the eventual artwork/presentation implementation PR. Do not create a second coding or implementation PR.
**State:** Planning only. None of the image assets, UI checks or runtime tests are claimed completed yet.

## Verified codebase seams and latest design-review feedback

- `godot/scenes/main.tscn`: nine existing BuildGrid Buttons and three TrainGrid Buttons have `metadata/kind`; the UI has `EconomyPanel/EconomyLabel`, `WaveLabel`, `ResearchPanel`, `UndergroundPanel/ExcavateButton` and `KindsBox`.
- `godot/scripts/battlefield_controller.gd`: `_ready()` uses catalogue-driven Buttons; `_refresh_hud()` formats resource/wave/research values from snapshots; `_refresh_underground_panel()` creates/reuses ordered room-kind and slot Buttons. Existing kinds and gameplay semantics are authoritative — icons are decoration only.
- Current smoke tests cast `EconomyLabel` and `WaveLabel` as plain `Label`. Economy tests require three logical rows, correct numbers and panel bounds; wave tests check readable countdown/incoming text; underground tests depend on ordered kind Buttons, slot click handling, locked levels, Tab/Esc and focused control behavior. A `RichTextLabel` migration **must update the tests deliberately**, not bypass their behavioral assertions.
- Existing review on [PR #19](https://github.com/cwchanap/Grus/pull/19#issuecomment-6675603970) correctly identified that fixed-position `TextureRect`s cannot align beside shifting multi-digit values, and that all art should not be produced before a real small-size layout proof. Adopt its inline-image, prototype-first and occupied-slot suggestions, but follow the **owner's newer decision to use painted raster PNG icons instead of SVG**.
- Keep **33 catalogue-aligned PNG assets** as previously agreed, with only the existing nine build / three train actions wired. The five otherwise-unused art files are previewed as a small-size contact sheet, not falsely tested in nonexistent UI actions.

## Checkpoint 1 — Define contracts and add failing icon assertions

**Files:** existing `godot/scripts/{smoke_test,economy_smoke_test,wave_smoke_test,underground_smoke_test,reset_test}.gd`; icon README.

- [ ] Record 33 explicit expected PNG paths in the icon README (5 resource/heat + 4 status + 7 underground + 10 buildings + 7 units). No crate or catalogue changes.
- [ ] Add focused checks to **existing** smoke scenes: correct `Button.icon` for all nine build, three train, six room-kind and Excavate controls; occupied room slots have a texture while Empty/Requires-level slots do not; HUD includes required images, no incorrect incoming/cold warning and no clipping.
- [ ] For `RichTextLabel`, keep original value and layout checks by asserting parsed/visible text and snapshot values instead of raw BBCode or the old `Label` cast. Preserve real click, focus, input shielding, research unlock and slot order assertions. Demonstrate new icon checks fail on the placeholder UI first; do not add an eighth smoke runner.

## Checkpoint 2 — Create three real PNG samples, render, then complete the set

**Files:** `godot/assets/ui/icons/{resources,status,underground,buildings,units}/*.png`, `godot/assets/ui/icons/README.md` and needed `.png.import` settings.

- [ ] **2a — art pilot:** author **three original illustrated icons** — one resource, one building and one underground room. Produce **128 × 128 transparent RGBA PNG images** with cohesive painterly shading and the existing snow/ice/rust/coal/crystal palette. No SVG, rasterized vector glyphs, clip-art or baked labels. Clean up transparent edges and tiny artifacts.
- [ ] Import the pilot in Godot 4.7.1 and wire each sample to its real HUD/button surface. Constrain *display* to **18–20 px** for HUD and **20–24 px** for buttons; a 128 px image must not expand the button layout. Take an actual rendered **1280 × 720** screenshot, including four-digit economy values and the ~65 px underground kind control. Approve silhouette, filtering, contrast, spacing and icon-above-text layout before batch generation.
- [ ] **2b — remaining art:** author the other **30 PNGs** with matching framing, lighting, transparent padding and material treatment. Each needs a recognizable silhouette at its real displayed size; remove fine details that blur to mush when downscaled. Preview all 33 at 20 px on light and dark backgrounds, including the five catalogue-only icons.
- [ ] Commit runtime PNGs, any necessary Godot `.png.import` settings and a short provenance/style/inventory README. Editable authoring sources are optional; document image-generation prompts or other source provenance when applicable. No atlas, custom export pipeline, remote runtime asset loading or additional framework.

## Checkpoint 3 — Bind action and underground PNGs through existing kinds

**Files:** `godot/scripts/battlefield_controller.gd`, `godot/scenes/main.tscn`, existing underground smoke.

- [ ] Use one small cached icon lookup deriving file paths by group + `kind.to_snake_case()` (e.g. `GunTurret` → `buildings/gun_turret.png`). Reuse current `metadata/kind` and catalogue loops; do not duplicate the allowed-build/train list in UI code.
- [ ] Assign resized `Button.icon` textures to the **nine build** and **three human train** Buttons, preserving names, tooltips, metadata, locked states, button callbacks and queued commands.
- [ ] Assign icons to **six catalogue-ordered room-kind** Buttons and **Excavate**. Use icon-above-short-text for the cramped ~65 px kind Buttons, adjusting only necessary vertical spacing/bounds. Keep existing exclusive selection, min-depth locks, Tab/Esc, mouse input and index order.
- [ ] Reuse cached room textures for **occupied/building underground slot Buttons** during the existing in-place refresh; clear for Empty/Requires-level slots. Do not reload assets every simulation tick or recreate slot controls under pending clicks.
- [ ] Do not invent Research Center build or alien train controls just because the PNG inventory includes those kinds.

## Checkpoint 4 — Inline PNGs alongside live economy/wave values

**Files:** `godot/scenes/main.tscn`, `godot/scripts/battlefield_controller.gd`, existing smoke assertions.

- [ ] Convert `EconomyLabel` and `WaveLabel` to Godot `RichTextLabel` **at the same paths**, with explicitly sized inline image BBCode. Retain current snapshot data and human-readable values; compare `get_parsed_text()` / displayed semantics in the updated smoke tests. Use native Godot text layout rather than positioning `TextureRect`s at variable-width text offsets.
- [ ] For the cramped EconomyPanel, use **icon + number** for Food, Scrap, Coal, Crystals and Heat, with a readable legend/tooltip describing the resource order. Preserve heat supply/demand, population, efficiency and **three logical lines** if the prototype fits; make only a minimal local panel adjustment if it does not. Show `surface_cold.png` **only during a genuine efficiency deficit**, while leaving its numeric percent readable.
- [ ] Inline `wave_countdown.png` near the original countdown text, and append `incoming_zone.png` **inside** the existing nonempty incoming-zone text branch so no extra visibility state is needed. Preserve wave number, countdown, live alien count and zone names.
- [ ] Show `research_progress.png` beside the existing ResearchBar/ResearchLabel without moving Analyze or changing progression, ETA or unlock logic.
- [ ] Keep all icon loading cached/static and entirely in Godot; no new GDExtension methods, Rust changes or duplicate UI state model.

## Checkpoint 5 — Verify and finish on **this PR**

**Files:** existing smoke scripts, optional `README.md` UI notes, icon README and evidence; no simulation changes.

- [ ] Inspect real Playing, Paused, cold deficit, incoming warning and underground-open screenshots at **1280 × 720**. Test digit-width changes, 18–24 px legibility, transparent edge contrast and widget bounds. Inspect the five catalogue-only icons at target size on the icon preview sheet. Do not claim visual QA from headless-only tests.
- [ ] Run the **seven existing** headless Godot smoke scenes after staging the current GDExtension and fresh import: `smoke_test`, `reset_test`, `economy_smoke_test`, `combat_lifecycle_smoke_test`, `wave_smoke_test`, `defenses_smoke_test` and `underground_smoke_test`. Require exit 0 and success markers.
- [ ] Reuse existing CI for Rust format/Clippy/tests/90% sim coverage, Godot import, Linux export/e2e and benchmark. Do not add duplicate jobs or a new icon-only runner. Report actual CI results.
- [ ] Verify the diff changes no files under `crates/`; check original economy/wave value semantics, room ordering, tooltips, locks and real click paths. Count **33 importable PNG assets** and verify every currently displayed button/slot/icon binding.
- [ ] Commit artwork, Godot hookup, test adaptations, documentation and any review fixes **to this same draft PR**; mark ready only after working assets and evidence land.

## Final acceptance checklist

- [ ] 33 original illustrated **PNG** icons committed at `godot/assets/ui/icons/` with alpha, consistent art direction, provenance and working Godot import.
- [ ] 9 build, 3 train, 6 room-kind, Excavate and occupied room slots show appropriately scaled PNGs; inactive slots do not show a misleading icon.
- [ ] 5 economy/heat and 4 status icons work with live values, incoming/cold state and research; 1280 × 720 HUD remains usable.
- [ ] Original game behavior, relevant updated smoke assertions and existing CI remain green.
- [ ] Real screenshots/contact-sheet visual inspection documented; no Rust simulation/bridge changes; **one implementation PR only**.
