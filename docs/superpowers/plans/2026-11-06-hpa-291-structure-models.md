# HPA-291 — Structure art implementation plan

**Status:** Implemented on this branch — post-review hardening verified locally; required CI evidence pending. Checkboxes remain unchecked: they record the planned checkpoints, not a per-item audit.
**Ticket:** https://linear.app/cwchanap/issue/HPA-291/grus-survival-art-structure-models
**Design:** [HPA-291 scoped design](../specs/2026-11-06-hpa-291-structure-models-design.md)
**Branch / PR:** `hpa-291-structure-models`; **one PR for HPA-291**. Add artwork, implementation, verification, and review fixes to this same draft PR.

## Audited existing seams and scope

- `crates/grus-sim/src/catalog.rs::building_spec` owns all ten building kinds/footprint sizes. `crates/grus-godot/src/lib.rs::building_view_transform` already centers and scales each view root X/Z to the footprint while Y stays 1.
- `crates/grus-godot/src/lib.rs::update_building_view_states` gathers completion and the authoritative cold flag per view, dispatching Godot `set_state(complete, cold)` on first sight or a transition without touching simulation or tick order. **No per-building `building_snapshot` polling.**
- `godot/scripts/resource_view.gd` already demonstrates kind-based model attachment, a visible fallback, and stopping `_process` when metadata has arrived. Reuse the pattern in `building_view.gd`; the building `HealthBar` and `set_health_ratio` stay in place.
- `scripts/build_winter_assets.py` is the mandatory authored source for this art family: extend its existing generator, export assertions, colors and single-command regeneration. Reuse the winter `.glb.import` recipe and existing post-import script at `godot/assets/winter/winter_glb_import.gd`; avoid a copied importer or forked generator.
- `economy_smoke_test.gd::_overlay_is` checks the **active model** meshes' `material_overlay` while retaining the heat/warm/cold/recovery checks.
- `battlefield_controller.gd` already has `_projected_art_rect` / `_view_art_bounds` for resources. Reuse them to pick large visible building silhouettes while excluding the `HealthBar` mesh.
- **Allowed change:** Godot scenes, scripts, GLBs, textures, import settings, Blender source, smoke tests, docs, and the tiny **`crates/grus-godot` view projection**. No changes under `crates/grus-sim`, catalogue, gameplay balance, movement, buildings state, or tick sequencing. Unit art and audio stay on their separate tickets.

## Checkpoint 1 — Freeze contracts and protect the pilot without breaking CI

**Files:** design/plan, existing `godot/scripts/{smoke_test,economy_smoke_test,defenses_smoke_test}.gd` when needed.

- [ ] Identify the exact `Model` child / metadata / health bar / root transform seam and update any test helpers using primitive `Body`.
- [ ] Keep the existing seven smoke flows **green at each checkpoint**. Do **not** assert all 11 missing GLBs in the first commit: `smoke_test.gd` quits on its first `_fail`, which would mask all later selection, fog, input and research regressions during the pilot.
- [ ] Stage only the targeted two-asset pilot checks alongside Checkpoint 2; defer the catalogue-wide import loop until all models exist in Checkpoint 3.
- [ ] Use a constant `MODELS` preload map keyed by catalogue kind; asset stems follow the catalogue names in snake case.

## Checkpoint 2 — Bunkhouse + construction-site pilot, including state/overlay/picking

**Files:** `scripts/build_winter_assets.py`, `godot/assets/structures/`, `godot/scenes/building_view.tscn`, `godot/scripts/building_view.gd`, `crates/grus-godot/src/lib.rs`, existing smoke scripts and a shared ice-overlay material if needed.

- [ ] Extend existing Blender `export()` with optional `out_dir`, `footprint`, `max_height` parameters while preserving the six existing winter-prop outputs. Author Bunkhouse at **true 2×2 world size**, then divide its Blender horizontal X/Y coordinates by 2 on export, leaving Blender Z/Godot Y untouched. Assert normalized ±0.5 horizontal bounds, grounded base, allowed height, <4k triangles. Use `--python-exit-code 1`. Produce `bunkhouse.glb`, `construction_site.glb` and the required texture PNG via this same source.
- [ ] Import pilot GLBs using the existing `.glb.import` recipe with `import_script/path="res://assets/winter/winter_glb_import.gd"` (the source is under `godot/assets/winter/`, **not** `godot/scripts/`). Verify texture references, real camera silhouette, clean imports and no accidentally generated physics/collision/navigation/occluder nodes.
- [ ] Change the bridge view projection to `set_view_state(handle, complete, cold)`, calling Godot `set_state(complete: bool, cold: bool)`; **do not add a sim state field or new bridge snapshot method**. Value-gate repeated pushes in Rust, and cache early pushes in the view until `building_kind` metadata arrives. Stop processing after a successful model attachment and retry failed attachments.
- [ ] Follow the `resource_view.gd` `Model` holder/fallback pattern. While `complete=false`, display the generic normalized site; on `complete=true`, swap **once** to the kind's final GLB. Preserve the root/id/footprint and health bar. Do not poll `GrusBridge` in `building_view.gd`; the selected-building HUD remains the consumer of `building_snapshot` progress.
- [ ] Remove obsolete solid Team-1/Team-2 building tinting. Reuse one pale translucent `StandardMaterial3D` overlay, assigned to active `MeshInstance3D.material_overlay` on cold and `null` when warm, excluding the `HealthBar`. Never duplicate/mutate the imported mesh or its materials. Position the health bar from the active model's AABB.
- [ ] Use the projected-art rectangle as a broad phase, then pick the depth-nearest ray hit on **the active `Model`**, excluding the `HealthBar`; preserve team/fog filters, center-radius fallback and resource/unit picks.
- [ ] Add just Bunkhouse/construction-site import, construction-completion, cold/warm/recovery, selectable-roof and geometry tests to **existing** smoke scenes. Preserve their remaining legs. Capture an actual 1280×720 pilot screenshot for review before batching.

## Checkpoint 3 — Complete models, then enable catalogue-wide import coverage

**Files:** `scripts/build_winter_assets.py`, `godot/assets/structures/README.md`, 11 GLBs, shared texture(s), Godot import settings, existing `smoke_test.gd`.

- [ ] Add nine remaining finished models to the same Blender source: Research Center, Depot, Barracks, Wall, Gate, Gun Turret, Coal Furnace, Flame Tower, Crystal Lance. Together with pilot Bunkhouse: **ten final building models plus one construction site**.
- [ ] Author **at each structure's actual world footprint** (4×4, 3×3, 2×2, 1×1), then normalize only horizontal dimensions before export. This preserves round barrel/chimney proportions when the existing bridge root scale is applied. Keep grounded, non-overhanging geometry; Wall and Gate meet neighboring edges in either axis without rotations or neighbor masks.
- [ ] Keep a single small committed diffuse texture referenced by the GLBs and vertex-color accents. Use the existing winter palette; avoid new normal/metallic atlases, shaders, LODs, animation or additional runtime systems. Commit glTF, textures, Godot import settings (each structures `.glb.import` references `res://assets/winter/winter_glb_import.gd`) and mandatory generation/provenance README.
- [ ] **Only after all eleven files are committed**, extend the existing `smoke_test.gd` to iterate `GrusBridge.catalogue_snapshot().buildings`: for each kind, derive `res://assets/structures/%s.glb` from `to_snake_case()`, import/instantiate, verify nonempty meshes, normalized X/Z bounds, grounded Y, absence of foreign gameplay nodes, and correct view hookup. Check `construction_site.glb` separately. No duplicate kind/size table.
- [ ] Run the **whole existing smoke suite** at this checkpoint before continuing. No eighth runner or separate implementation PR.

## Checkpoint 4 — Stress view transitions and existing gameplay journeys

**Files:** existing `godot/scripts/{smoke_test,economy_smoke_test,defenses_smoke_test,reset_test}.gd`, `godot/scripts/battlefield_controller.gd` only for agreed picking change; update README where appropriate.

- [ ] Assert seeded 4×4 Research Center and active Bunkhouse/Depot/Barracks/Wall/Gate/Turret views use catalogue-scaled, grounded assets; Wall–Gate–Wall reads as one line in both orientations.
- [ ] Validate push timing: state delivered before metadata, unfinished site → completed kind exactly once, later cold on/off and recovery, no stale model after restart. No per-frame snapshot reads; `_process` stops once metadata styling is done, while `set_state` continues reacting to authoritative pushes.
- [ ] Replace `_tint_is` old `Body.material_override.albedo_color` check with every active model mesh's shared `material_overlay == ICE` when cold, `null` when warm, including repeated same-kind buildings. Retain the existing 15/16 deficit, furnace restoration, and zero-demand (Wall/Gate/Furnace) guards. No mutable per-instance material duplication test is needed.
- [ ] Keep all established defenses smoke checks: Gate permits humans, walls obstruct aliens, turrets attack, health bars react, and selection/repair continues. Verify clicking the **Research Center roof** at maximum zoom and building silhouettes beyond the old 20px center, not merely the ground center. Exclude health-bar geometry from silhouette picking.
- [ ] Preserve resource-gather picking, unit picking, fog visibility, research UI, pause/restart, underground panel and benchmark fortress census. Research-gated Flame Tower/Crystal Lance require asset-import proof, **not** a fabricated mid-game research unlock for short smoke runs.

## Checkpoint 5 — Existing quality gates and evidence on this PR

- [ ] Fresh editor import: `godot --headless --path godot --editor --quit-after 120`; check glTF/texture import errors/warnings and confirm cached imports regenerate.
- [ ] Stage the current compiled GDExtension per `CLAUDE.md` (do not run smokes against a stale library). Execute **all seven** existing headless scenes: `smoke_test`, `reset_test`, `economy_smoke_test`, `combat_lifecycle_smoke_test`, `wave_smoke_test`, `defenses_smoke_test`, `underground_smoke_test`. Require success markers **and exit code 0**.
- [ ] Existing CI gates: Rust fmt/Clippy/workspace tests and bridge tests, >90% sim coverage, Linux export/packaged E2E, survival benchmark. If an artifact or gate fails, document actual evidence; no duplicate CI job.
- [ ] Inspect real **1280×720** frames of a completed city, construction in progress, cold deficit/recovery and horizontal/vertical Wall–Gate–Wall. Capture screenshots/provenance, observed visual issues and executed commands in `docs/hpa-291-structure-art-evidence.md`. Headless success alone is not visual proof.
- [ ] Update `README.md`, ensure only the narrowly permitted `crates/grus-godot` view-projection lines changed and **no `crates/grus-sim` changes**, link evidence to HPA-291 and complete this same PR. Mark ready only after assets and verification land.

## Acceptance checklist

- [ ] Ten distinct final structure GLBs + one construction-site GLB; Godot-importable with diffuse texture and required importer settings under `godot/`; **reproducible generation source in existing script**.
- [ ] Models authored at true world footprint, exported normalized, then correctly scaled by unchanged catalogue-driven runtime transform; walls and gates tile in both axes.
- [ ] Authoritative bridge-pushed completion/cold state; reversible shared ice overlay, no mutated imported materials and no GDScript snapshot polling.
- [ ] Building silhouette picking, health bars, selection, placement, construction, fog, restart and combat continue to work.
- [ ] Seven existing smokes and CI results reported, real screenshots inspected; **one PR, no gameplay/sim change, no unit art or SFX**.
