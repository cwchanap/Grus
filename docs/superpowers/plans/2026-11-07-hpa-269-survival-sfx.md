# HPA-269 — Winter Survival SFX implementation plan

**Linear:** [HPA-269](https://linear.app/cwchanap/issue/HPA-269/grus-survival-sfx-combat-wave-and-ambience-sound-effects)
**Spec:** [HPA-269 SFX design](../specs/2026-11-07-hpa-269-survival-sfx-design.md)
**Branch:** `hpa-269-survival-sfx`
**PR policy:** This draft planning PR **is the only implementation PR for HPA-269**. Commit generated/edited sounds, integration, test changes, README updates, and review fixes to this branch. Do not open a second PR, or create a separate art ticket for these already-scoped audio assets.
**Current state:** Design/plan only. No new SFX, runtime wiring, audible QA, or test runs are claimed.

## Updated review decisions (GitHub design review)

- **Sourcing locked:** `scripts/build_sfx_assets.py` generates the original sound designs offline with deterministic Python waveform/noise/envelope synthesis (no paid API); a documented **one-pass ffmpeg** trim/edge-cleanup/loudness-normalization/OGG export creates final assets. The generator is authoring-only, not a runtime or CI pipeline. Capture seed, command and processing options in `godot/assets/audio/README.md`.
- **Audio format:** 18 final OGG Vorbis files only, exact filenames from the [design inventory](../specs/2026-11-07-hpa-269-survival-sfx-design.md#asset-inventory-18-original-audio-files). No WAV branch or alternative paths.
- **Mixer:** one `const CUES` table with `preload()`, `volume_db` and `max_polyphony` for each cue; construct one `AudioStreamPlayer` per cue at startup. Use Godot's built-in polyphony (combat 2 initially, alerts/wind/outcomes 1) and a 150 ms **per-cue** combat throttle. No recycled-voice pool, per-frame quota, bus/ducking or priority engine.
- **Identity:** scan view groups once per rendered frame into current ID→kind dictionaries, use previous-frame kinds only when the target has been despawned, then replace old dictionaries. Keep the existing live-view lookup for tracer placement. No growing cache, pruning or extra per-event audio scans. Hidden attacker IDs scrubbed to 0 stay silent.
- **Integrity:** final `const CUES` uses **18 static `preload()` values**, so missing files are load-time errors. A known supported attack kind with no mapping does `push_error()`; a deliberately unknown/scrubbed identity remains silent. Verify 18 non-null streams and all expected kind names in **one existing smoke**.
- **Transition state:** horns re-arm whenever `incoming_zones` is empty; cold state updates on every frame regardless of phase but only sounds on Playing warm→cold; wind derives from current phase; Result uses existing `result_frame`. These heal after `GrusBridge.reset_fixture()` even when the controller restart helper was skipped.
- **Listening gates:** two explicit **human approvals**, not assumptions made by a headless runner. When the three-clip pilot is ready, stop for human feedback before producing the remaining 15. After full implementation, stop for a real listening verdict before marking PR ready.
- **Verification:** CI is the canonical smoke runner; it checks **both expected marker and exit 0 under timeouts**. No standalone `godot ... || exit 1` loop, and no unnecessary local Rust Clippy/coverage reruns for a Godot-only PR.

## Checkpoint 1 — Generate and wire the three-sound prototype

**Files:** `scripts/build_sfx_assets.py`, `godot/assets/audio/{combat,ambience}/`, `godot/assets/audio/README.md`, `godot/scripts/{combat_fx,battlefield_controller}.gd`, relevant existing smoke.

- [ ] Author a small deterministic **offline Python source script** using standard-library synthesis (seeded noise, filtered layers, pitch sweeps, envelopes, simple resonances). Start with Gun Turret fire, Swarmer death and loopable winter wind. Prefer clear readable game sounds over complicated synthesis models; use the exact final `.ogg` filenames.
- [ ] Add a **single documented ffmpeg normalization/cleanup/OGG encoding pass** for generated temporary PCM files, including trim/edge smoothing, target loudness and peak ceiling. Wind needs a seam-matched/crossfaded loop without a level dip. Record exact invocation and settings in audio README. Keep runtime/CI independent of ffmpeg and the Python script.
- [ ] Implement the tiny `CombatFx` cue-table/player approach **with these three pilot entries**, including `volume_db`, `max_polyphony`, per-cue throttle and a cue-count inspectable test hook. Route Gun Turret fire, known Swarmer death and session-dependent wind through **real** `battlefield_controller.gd` event/phase paths. Keep current hit/death meshes. During the intentionally partial pilot, other sounds can temporarily retain the old generic blip; remove that fallback when the full cue table arrives. Never silently ship a missing supported kind in the final implementation.
- [ ] Extend the existing appropriate smoke(s) to require all three pilot streams, their real event-path routing and core gameplay outcomes. **Run the existing CI smoke gates green** at this checkpoint; do not add a temporary harness or new smoke scene.
- [ ] Do not mark full 18-stream completeness at this stage. The three-clip smoke assertions become 18-stream assertions in Checkpoint 3.

## Checkpoint 2 — Mandatory human listening gate #1

- [ ] Hand the **three actual in-game prototype sounds** to the user for listening in a normal-speed Godot session. Record the build/environment and feedback on gun-shot impact, recognizable Swarmer death, wind loop seam and relative mix.
- [ ] **Stop here until the user explicitly approves the sound direction.** A code agent, CI, or the Godot Dummy audio driver cannot substitute for listening; do not mark approved based on import success. If rejected, revise the script/envelopes or the per-cue `volume_db` and repeat this same gate, not a new issue or PR.

## Checkpoint 3 — Finish all 18 sounds and strict cue integrity

**Files:** `scripts/build_sfx_assets.py`, `godot/assets/audio/{combat,alerts,ambience}/*.ogg`, audio README, `godot/scripts/combat_fx.gd`, one existing smoke.

- [ ] Generate remaining **15** original clips: Flame Tower, Crystal Lance, Guard, Rifleman, four alien attack sounds, three other alien death sounds, wave horn, freeze warning, Victory and Defeat. Verify they share the approved prototype's sound direction, have distinct roles and preserve any chosen human adjustments.
- [ ] Convert every asset through the recorded ffmpeg pass to OGG; commit the **18 OGG files** plus deterministic source script and its README. Avoid paid-credit dependencies and undisclosed stock licenses; generated content is original to this project.
- [ ] Replace the pilot's partial table with exactly **18 `preload()` entries**. Remove `_make_blip()` and any temporary generic fallback. Construct one non-spatial `AudioStreamPlayer` per cue via the table, with `volume_db` and native `max_polyphony`; no custom voice pool.
- [ ] Route all 3 towers, 2 human combat roles, 4 alien attacks and **4 alien deaths** via view-kind metadata, with a once-per-frame current/previous ID→kind lookup. Unknown/scrubbed attackers are silent; a known supported kind with no table entry uses `push_error()`. Preserve all visual tracer/hit/death effects.
- [ ] Add **one** focused existing-smoke integrity assertion: exactly 18 imported non-null streams and all supported attacker/death keys (incl. `ALIEN_KINDS`). Ensure parse/load errors cannot masquerade as passing smoke success markers.

## Checkpoint 4 — Wave, cold, lifecycle edges and existing journey tests

**Files:** `godot/scripts/{combat_fx,battlefield_controller}.gd`, existing `wave_smoke_test.gd`, `economy_smoke_test.gd`, `combat_lifecycle_smoke_test.gd`, `defenses_smoke_test.gd`.

- [ ] From the **existing** `wave_snapshot()` in `_refresh_hud()`, clear `_horn_wave` on every empty `incoming_zones` snapshot (including Start and Paused); play once on the first nonempty warning for a different `next_wave` while Playing. Neither repeated frames, minimap ping expiry nor multiple announced zones trigger extras.
- [ ] From the **same** `economy_snapshot().surface_efficiency` used for the HUD cold icon, compute `cold = surface < 1.0` every frame in every phase; play once only on a Playing transition from warm to cold. Update `_was_cold` unconditionally, so a reset to warm (including direct `GrusBridge.reset_fixture()`) and full recovery re-arm it.
- [ ] Drive the quiet wind player's play/pause/stop directly from `session_snapshot().phase` on every frame. Use the already-computed `result_frame` to play one Victory or Defeat stinger **after** the settle-tick combat-event drain; don't stop/duck the killing-blow sound.
- [ ] `clear_all()` retains visual cleanup and stops live sound players; do not depend on it for logical warning/Result flags. Rebuild current unit/building kind dictionaries on every frame even when not Playing, so direct fixture reseeds converge naturally.
- [ ] Reuse `wave_smoke_test.gd`'s authored incoming warning to check one horn count. Reuse `economy_smoke_test.gd`'s existing heat deficit/recovery **across its direct fixture reset** to check one cold cue and re-arm. Use `combat_lifecycle_smoke_test.gd` for paused silence, death kind routing, first-Result stinger and Restart; `defenses_smoke_test.gd` for Gun Turret sound/tracer. Preserve all current journey checks and markers.
- [ ] Assertions inspect the `cue_counts` hook and loaded cue table, not headset-dependent `is_playing()` (headless Godot's Dummy driver cannot prove audible playback).

## Checkpoint 5 — CI-grade verification and mandatory human listening gate #2

- [ ] After staging the current native GDExtension, fresh Godot import must pass. Rely on **`.github/workflows/ci.yml`** for the seven existing smoke scenes; each CI gate uses a deadline, the scene's **specific success marker**, and exit code 0. The shared `smoke_test` additionally requires `SMOKE_STRUCTURE_AUDIT source_bytes=editor`. Do not replace this contract with an exit-only local loop. For manual local spot checks, mirror **both** the timeout and marker from CI.
- [ ] Review CI's Rust/coverage, Linux export/E2E and benchmark results for regressions; do not duplicate `cargo llvm-cov` or Rust Clippy on the local Godot-only plan. Do not claim any CI results before they are actually reported.
- [ ] **Human listening gate #2:** hand off a playable build for normal-speed listening with wind, representative simultaneous tower/alien combat (including Behemoth), incoming horn, warm→cold/recovery, Pause/Resume, Restart and both Victory/Defeat stingers. Confirm cues are audible, non-clipping, comprehensible and do not reveal hidden enemies; confirm wind is seamless and doesn't mask warnings.
- [ ] **Stop and request explicit user sign-off** before marking PR ready. If the human rejects any sound, edit the offline source or the specific cue's `volume_db`, retest on this PR, and ask for listening again. Do not invent listening evidence or treat a headless test as a substitute.

## Completion gate — one PR only

- [ ] 18 importable OGG clips, deterministic offline source script, reproducible ffmpeg command and README documentation.
- [ ] Native Godot per-cue polyphony/throttle, strict known-kind mapping, fog-safe silence, distinct alien deaths and intact visual effects.
- [ ] Self-healing wave/cold/wind/Result transitions even across direct bridge fixture resets; no new sim or bridge state.
- [ ] All seven **CI-equivalent** smoke gates plus relevant full workflow jobs pass, with no `crates/` changes.
- [ ] Both human listening approvals recorded: prototype before batch, full mix before Ready.
- [ ] All changes remain in **this same draft PR**, which becomes the HPA-269 implementation PR; no second PR.

**Current state remains planning only:** code, audio assets, smoke runs and human approval are not yet completed.
