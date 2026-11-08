# HPA-269 — Winter Survival SFX implementation plan

**Linear:** [HPA-269](https://linear.app/cwchanap/issue/HPA-269/grus-survival-sfx-combat-wave-and-ambience-sound-effects)
**Spec:** [HPA-269 SFX design](../specs/2026-11-07-hpa-269-survival-sfx-design.md)
**Branch:** `hpa-269-survival-sfx`
**PR policy:** This draft planning PR **is the only implementation PR for HPA-269**. Commit generated/edited sounds, integration, test changes, README updates, and review fixes to this branch. Do not open a second PR, or create a separate art ticket for these already-scoped audio assets.
**Current state:** Design/plan only. No new SFX, runtime wiring, audible QA, or test runs are claimed.

## Goal and verified integration surfaces

Replace `combat_fx.gd`'s generated sawtooth blip with a small original winter-survival sound pack and integrate combat, incoming-wave warnings, surface-freeze warnings, looping wind, and outcome stingers into existing Godot presentation. All 18 assets and their use belong to this task.

The codebase already provides the necessary read-only signals:

- `godot/scripts/combat_fx.gd`: `CombatFx` owns one generated `AudioStreamPlayer`, transient effects, and `clear_all()`. Keep effect meshes and add only a small sound router/pool here.
- `godot/scripts/battlefield_controller.gd`: `_refresh_session()` drives phase/Result and drains combat event dictionaries, `_dispatch_combat_event()` drives hit/death visuals, `_refresh_hud()` reads wave and heat snapshots, and `_restart_match()` already clears `CombatFx`.
- `crates/grus-godot/src/lib.rs` documents the current bridge payload: combat dictionaries carry stable attacker/target identities, `ranged`, `killed`, and hit coordinates, **not concrete kind strings**. The bridge hides a fogged enemy attacker's identity as ID 0. Use `unit_kind`/`building_kind` metadata from existing Godot views; cache previous-frame kinds to survive death-view deletion; do not expand the bridge.
- `wave_snapshot().next_wave` + `incoming_zones` identifies warning transitions. `economy_snapshot().surface_efficiency` identifies heat deficit. `session_snapshot().phase` and `winner_team` identify Result. No secondary gameplay state model.

## Checkpoint 1 — Lock assets, audio quality and focused assertions

**Files:** `godot/assets/audio/README.md`; existing smoke scripts; optionally a temporary local playback harness that is **not** a new shipped smoke runner.

- [ ] Use the spec's exact **18** names: 3 tower fire, 2 human attacks, 4 alien attacks, 4 alien deaths, wave horn, freeze warning, wind loop, Victory and Defeat. Prefer `res://assets/audio/combat/*.ogg`, `alerts/*.ogg`, `ambience/*.ogg`; a justifiable short WAV substitution is fine, but keep code paths synchronized.
- [ ] Record the asset inventory, audio-generation/source attribution and editing process in an audio README; use assets with clear usage rights. Avoid a source-asset framework or remote runtime fetch.
- [ ] Add small focused expectations to existing Godot smoke scripts for resolvable asset paths and conditional cue logic. Their current real combat/wave/cold/restart journeys must remain intact; do not add an eighth Godot runner or depend on the host having a physical sound device.
- [ ] Plan for the **bridge payload limitation** explicitly: any target kind cache is presentational, bounded and cleared at restart; unknown targets/hidden attackers use generic impact or silence, not fabricated unit types.

## Checkpoint 2 — Author three sample sounds and validate the mix before batch production

**Files:** `godot/assets/audio/combat/gun_turret_fire.ogg`, `combat/swarmer_death.ogg`, `ambience/winter_wind_loop.ogg`; audio README.

- [ ] Produce **three representative original sounds**, import with the pinned Godot 4.7.1 editor, and listen in a real running scene: sharp turret shot, tiny alien death, and low-volume looped winter wind. Check overlap and contrast at the normal 1280×720 RTS view.
- [ ] Verify clean starts/ends, no clipping, no crackle at the loop boundary, quiet wind underneath important cues, and acceptable file size. Adjust normalization/processing before scaling out the whole inventory.
- [ ] After the prototype is satisfactory, create the **remaining 15**: Flame Tower, Crystal Lance, Guard, Rifleman, four alien attacks, three other alien deaths, wave horn, freeze warning, Victory, Defeat. Confirm the final count is 18 and no two creature roles are indistinguishable.
- [ ] Commit actual runtime audio files and ordinary Godot import metadata as necessary; do not commit a runtime generator, binary authoring toolchain or placeholder tones for final deliverables.

## Checkpoint 3 — Replace the one-blip path with a bounded combat cue router

**Files:** `godot/scripts/combat_fx.gd`, `godot/scripts/battlefield_controller.gd`; existing combat/defense smokes.

- [ ] Preload/cache a **small fixed dictionary** of asset streams (attack and death by existing kind string) in `CombatFx`. Remove `_make_blip()` only after streams are real and import cleanly. Keep `spawn_tracer()`, `spawn_hit()`, `spawn_death()`, and transient visual cleanup as-is.
- [ ] Use up to **8 recycled non-spatial one-shot players** to permit useful overlap while bounding simultaneous voices. Cap sounds per rendered frame or common-category rate so 150 active aliens do not create a deafening wall; reserve a distinct player for higher-priority alerts/outcomes. Keep the logic compact, with no AudioServer middleware or per-entity sound components.
- [ ] At each render-frame drain, classify the attacker's live view metadata or previous-frame kind cache. Building attackers map to GunTurret/FlameTower/CrystalLance. Unit attackers map to Guard/Rifleman and four alien kinds. Skip specific alien cue selection for `attacker=0` (fog-scrubbed) or unresolved/hidden attackers.
- [ ] On a `killed` event for a known alien unit target, play the appropriate **separate death** clip; do not play death on normal hits or identify unknown targets by guessing. Refresh/prune the transient stable-ID cache **after** event dispatch; clear it on Restart. Use the presentational cache for audio only, never for orders/visibility/simulation truth.
- [ ] Check audible cadence at normal and 20× test speeds; preserve every visual hit/death event even if audio overload protection drops a cue.

## Checkpoint 4 — Hook warning and session sounds without duplicate state

**Files:** `godot/scripts/battlefield_controller.gd`, `godot/scripts/combat_fx.gd`; existing wave/economy/combat smokes.

- [ ] Feed the **existing** per-frame wave and economy snapshots to `CombatFx` from `_refresh_hud()`; do not call duplicate bridge queries or recompute wave/heat gameplay rules in GDScript.
- [ ] On first announced nonempty `incoming_zones` for each `next_wave`, play wave horn **once**, including the first prep→wave warning; subsequent frames and ping expiry do not re-trigger. Multiple zones in the same wave still make one alert.
- [ ] When the Playing snapshot crosses from `surface_efficiency == 1.0` to below 1.0, play freeze warning once; stay silent until efficiency fully recovers and falls again. No alert polling while paused or in Result; retain existing HUD cold icon and percent.
- [ ] Maintain one soft `AudioStreamPlayer` for looping wind. Start on Playing, pause/resume coherently, stop on Result or Restart. Do not run ambient audio over a paused game or over a settled ending.
- [ ] On `_session_phase` transition into Result, play Victory if `winner_team == 1`, otherwise Defeat. Deduplicate on phase transition, stop/duck lower-priority sounds, and **still drain/render the killing blow once** on the initial Result frame as the current controller does.
- [ ] Extend `_fx.clear_all()` and existing `_restart_match()` to reset audio players, any dedup keys/cooldowns, recent kind cache, and win/lose state. No lingering horn, death sound or wind from a previous match.

## Checkpoint 5 — Exercise existing smokes and do real audible QA

**Files:** edit the three/four relevant existing smoke scripts; no simulation files.

- [ ] **Combat lifecycle smoke:** use its real combat and alien Defeat journey to verify named sound routes, a killed alien's separate death sound when known, paused silence, one Result stinger and Restart cleanup. If multiple kinds cannot be reached deterministically in the short journey, use a tiny presentation-only routing assertion for the remaining kinds, not a new gameplay shortcut.
- [ ] **Wave smoke:** reuse the real incoming warning window; verify horn exactly once for that `next_wave`, not on repeated frames, not on minimap ping expiry, and no replay after resume. Keep the existing source-of-truth wave HUD and fog marker checks.
- [ ] **Economy smoke:** reuse the actual Depot/tower heat deficit → Bunkhouse → fueled Coal Furnace recovery path; assert one cold warning on entry, no repeat throughout cold, and eligibility after full recovery. Preserve existing live heat, resource, placement and overlay assertions.
- [ ] **Defenses smoke:** ensure the Gun Turret's real shot still has a tracer and a selected gun-turret sound; do not weaken turret kill/Crystal reward checks.
- [ ] Assert audio via loaded streams, cue selection or a minimal inspectable last-cue counter/path, not headset-dependent `is_playing()` assertions in headless mode. Verify import failures fail the smoke. Avoid altering the Rust tests for presentation-only sound behavior.
- [ ] **Audible QA on desktop:** manually play a normal-speed game with wind and active fights, waves including one Behemoth, a cold deficit/recovery, Pause/Resume, Restart and both Result stingers; listen for clipping, loop seams, missed alerts, excessive stacking and hidden-attacker leakage. A headless pass is not listening evidence.
- [ ] Retain the current seven Godot headless smoke scenes: `smoke_test`, `reset_test`, `economy_smoke_test`, `combat_lifecycle_smoke_test`, `wave_smoke_test`, `defenses_smoke_test`, `underground_smoke_test`. On a clean checkout, first build/stage the current GDExtension and run editor import.

## Verification commands (run later on this same PR)

```bash
cargo build -p grus-godot --locked
# Copy target/debug/libgrus_godot.{dylib,so} into godot/bin/ for the host.
# On macOS, re-sign the staged dylib if needed (see CLAUDE.md).
godot --headless --path godot --editor --quit-after 120
for scene in smoke_test reset_test economy_smoke_test combat_lifecycle_smoke_test wave_smoke_test defenses_smoke_test underground_smoke_test; do
  godot --headless --path godot "res://scenes/${scene}.tscn" || exit 1
done
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo llvm-cov -p grus-sim --all-targets --fail-under-lines 90 --locked --ignore-filename-regex 'tests\.rs$'
```

Also review the existing Linux Godot export/E2E, benchmark and coverage CI outputs for regressions. Record **observed** pass/fail results and audible-QA environment in the PR, rather than pre-checking boxes in a planning-only draft.

## Completion gate for this one PR

- [ ] Exactly **18** importable, rights-cleared audio clips + provenance README.
- [ ] All current combat varieties and four alien deaths routed by existing view kinds; hidden/unknown identity never leaks.
- [ ] Wave, cold, ambient, victory/defeat cues have correct phase transitions and one-shot deduplication.
- [ ] Audio remains intelligible under real wave load; Pause/Result/Restart sound cleanup is verified.
- [ ] Existing seven Godot smokes, Rust/CI gates, and actual desktop listening evidence recorded and passing.
- [ ] No `crates/` changes, no new gameplay state/systems, **no second implementation PR**.

**Review-ready point:** only after the above work is committed and verified on `hpa-269-survival-sfx`; leave the initial design/plan commit Draft.
