# HPA-291 — Structure art implementation plan

**Status:** Proposed; **documentation-only planning draft**. Checkboxes below are NOT claims that artwork, tests, or verification exist.
**Ticket:** https://linear.app/cwchanap/issue/HPA-291/grus-survival-art-structure-models
**Design:** [HPA-291 scoped design](../specs/2026-11-06-hpa-291-structure-models-design.md)
**Branch / PR:** `hpa-291-structure-models`; **one PR per ticket**. Continue implementation, evidence, and all review fixes on this same draft PR.

## Scope / audit summary

- The existing Rust bridge already applies the catalogue-derived 4×4 / 3×3 / 2×2 / 1×1 footprint and calls `set_cold` and `set_health_ratio`. Keep `crates/grus-sim` and `crates/grus-godot` untouched.
- `building_view.tscn` and `building_view.gd` own all existing surface-building visuals, and `GrusBridge.building_snapshot(id).complete` is enough to show a construction site without a bridge change.
- Current `economy_smoke_test.gd::_tint_is` is tied to a solid primitive `Body` material; its warm/cold proof must be moved to the actual imported mesh material and remain equally strong.
- Existing `scripts/build_winter_assets.py`, `godot/assets/winter/winter_glb_import.gd`, and `.glb.import` files are the known-good Blender/GLB import pattern.

## Checkpoint 1 — Freeze the asset/view contract, then add failing import assertions

**Files:** `godot/scripts/smoke_test.gd`, `godot/scripts/economy_smoke_test.gd`, `godot/scripts/defenses_smoke_test.gd`; maybe `godot/assets/structures/README.md`.

- [ ] Add a strict list of the **ten** expected `BuildingKind`/GLB path pairs (plus the visual-only `construction_site.glb`) to art documentation. Pull the actual dimensions from `GrusBridge.catalogue_snapshot().buildings` where available; do not create a second hand-maintained size catalogue.
- [ ] In the existing smoke, load/instantiate all eleven GLBs with a short bounded helper; assert nonempty mesh bounds, local X/Z within ±0.5, Y grounded, imported `scene_file_path` and absence of physics/navigation/occluder nodes. Fail visibly while they are missing.
- [ ] Add live Research Center / construction / finished-building checks: model path, world-space bounds matching the authoritative scale, intact health/selection identity. Add a Wall–Gate–Wall row/column visual contract to the existing defenses journey where viable, rather than spawning a new test framework.
- [ ] Preserve the real temperature/cold test semantics (`15/16` efficiency deficit and furnace recovery). Replace old `Body`-only tint assertions **only alongside the new model material integration**.

## Checkpoint 2 — Author one pilot model and a construction-site proof

**Files:** `godot/assets/structures/`; optional `scripts/build_structure_assets.py`; `godot/scenes/building_view.tscn` and `godot/scripts/building_view.gd`.

- [ ] Make one **2×2 Bunkhouse pilot** with original textured/glTF art and a reusable normalized `construction_site.glb`. Validate local origin, exact footprint, shape at default camera zoom, and clean Godot 4.7.1 import. Reuse existing import post-processor/settings, not a new tooling architecture.
- [ ] Wire `BuildingView` to load pilot art by metadata after the bridge stamps `building_kind`; leave `BuildingView` root scale and `HealthBar` untouched. While `building_snapshot(id).complete` is false, show generic construction art; switch to Bunkhouse GLB at completion. Poll the snapshot **only until finished**. Use the real gameplay construction state, not an invented timer.
- [ ] Implement view-local duplicated materials and the existing `set_cold(bool)` API's ice tint + warm restoration, retaining vertex colors/albedo texture. Confirm two instances never share tint state.
- [ ] Inspect a real 1280×720 Bunkhouse under construction, complete, cold, and reheated; correct silhouette/texture scale/health-bar offset before batch authoring. Do not batch ten models before the pilot is validated.

## Checkpoint 3 — Complete the ten-building art family, no new gameplay rules

**Files:** `godot/assets/structures/{research_center,bunkhouse,depot,barracks,wall,gate,gun_turret,coal_furnace,flame_tower,crystal_lance,construction_site}.glb`, referenced texture PNG(s), their import settings, optional Blender source, `godot/assets/structures/README.md`.

- [ ] Produce all ten building models with the specified silhouette distinctions, plus the construction-site asset. Keep the frozen/scrap/concrete palette consistent with existing winter terrain and HPA-44 PNG icons.
- [ ] Use 1×1 normalized X/Z bounds, Y-grounded origin, no overhang, no added collision/nav/occlusion, sensible triangle limits. Make Wall and Gate connect along both X and Z when placed tile-to-tile; avoid neighbor masks, rotations, special junction state and consolidated mesh placement.
- [ ] Commit Godot-importable GLBs, at least one shared diffuse texture PNG referenced by the models, required `.import` settings and short asset provenance/regeneration notes. Test import on a clean Godot cache; no uncommitted generated resource dependencies.
- [ ] Connect **all ten** `building_kind` strings to matching imported assets in the one `BuildingView` presentation path. Keep a visible fallback (with warning) for an unknown kind; the import smoke must fail if any expected catalogue kind is unmapped.

## Checkpoint 4 — State, selection, and view regressions

**Files:** `godot/scripts/building_view.gd`, existing `godot/scripts/{smoke_test,economy_smoke_test,defenses_smoke_test,reset_test}.gd`; only touch `battlefield_controller.gd` if actual model-picking proof fails.

- [ ] Check seeded Research Center, then a real Bunkhouse construction transition. Assert the site and final model share the same root/id/footprint; after restart, no stale scaffold/old material remains. Confirm cold-state calls arriving before art load are replayed on the loaded material.
- [ ] Keep cold/warm assertions at the **actual imported MeshInstance3D materials**, and verify a second same-kind building has an independent material. Cold is only applied to completed Team-1, heat-demanding structures; Wall, Gate and Furnace remain normal even in deficits.
- [ ] In the defenses smoke, verify imported Wall/Gate/GunTurret art during construction and after completion; preserve human path through Gate, alien obstruction, turret fire/tracer and damaged-health-bar checks. Add at most local presentation-level picking/height corrections where the real asset would otherwise be unclickable.
- [ ] Verify Research Center HUD/selection, Bunkhouse/Depot/Barracks build flow, research-gated asset paths (Flame Tower/Crystal Lance), fog, restart and benchmark fortress census. Do not require building a 50%-research tower in a short Godot smoke; test import for all kinds separately.

## Checkpoint 5 — Run the existing gates and finish this PR

- [ ] Fresh Godot import: `godot --headless --path godot --editor --quit-after 120`; check import warnings/errors for each GLB and texture.
- [ ] Stage the current GDExtension as documented in `CLAUDE.md`, then run all seven scenes: `smoke_test`, `reset_test`, `economy_smoke_test`, `combat_lifecycle_smoke_test`, `wave_smoke_test`, `defenses_smoke_test`, `underground_smoke_test`. Require their existing success markers **and process exit 0**.
- [ ] Use existing CI for `cargo fmt --all -- --check`, Clippy, workspace/Rust bridge tests, >90% sim coverage, Linux export/E2E, and survival wave benchmark; no new CI job or unnecessary Rust-only test target. Any failed gate is reported honestly and fixed on this PR.
- [ ] Capture and inspect actual rendered **1280×720** scenes: completed city, partially built structure, cold tint/recovery, and horizontal/vertical Wall–Gate–Wall. Include images/provenance/observed limits in `docs/hpa-291-structure-art-evidence.md`; never claim visual review from headless-only logs.
- [ ] Update `README.md` only for the new structures art and verification, check no diff under `crates/`, and link the completed implementation/evidence back to Linear HPA-291. Mark PR ready **only when assets, integration and proof are present**.

## Acceptance checklist

- [ ] Exactly ten distinct structure GLBs + one reusable construction-site appearance, Godot-importable with referenced texture assets committed under `godot/`.
- [ ] Visual footprints match the live catalogue, and Wall/Gate runs visibly tile along both axes.
- [ ] Cold and construction states are readable, reversible/authoritative, and do not mutate shared imported materials.
- [ ] Existing health bars, selecting, placing, constructing, combat, pause/restart, fog and seven smokes remain valid.
- [ ] Real gameplay screenshots + provenance and actual CI results documented; **no Rust, no unit art, no SFX, one PR for HPA-291**.
