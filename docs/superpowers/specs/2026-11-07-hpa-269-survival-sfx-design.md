# HPA-269 — Winter Survival combat, wave, and ambience SFX design

**Status:** Draft for review — this commit adds design documents only, no audio assets or runtime changes.
**Linear:** [HPA-269 — Combat, wave, and ambience sound effects](https://linear.app/cwchanap/issue/HPA-269/grus-survival-sfx-combat-wave-and-ambience-sound-effects)
**Parent:** [Winter Survival design](2026-10-21-winter-survival-design.md)
**Execution:** The draft PR for these documents is the **one implementation PR for HPA-269**. Complete SFX generation, Godot integration, tests, and review changes on this branch; do not open a separate coding PR.

## Goal and scope

Replace the procedural 8-bit blip in \`godot/scripts/combat_fx.gd\` with a coherent winter-survival sound set and add actionable wave, cold, win/lose, and winter-wind cues. All gameplay decisions remain in Rust/Bevy. HPA-269 is the **dedicated audio-art ticket**: author original (or appropriately licensed) sound files here rather than folding asset generation into another feature's code task.

**In scope:** files in \`godot/assets/audio/\`; the existing \`godot/scenes/main.tscn\`, \`godot/scripts/combat_fx.gd\`, \`godot/scripts/battlefield_controller.gd\`; focused edits to current Godot smoke scripts; documentation and audible QA.

**Out of scope:** \`crates/\`, combat events or bridge schemas, wave/heat/result rules, balance, new unit/building features, new persistent game state, a sound middleware layer, dialogue/music/voice-over, dynamic music, a procedural SFX generator, and changes to HPA-44/HPA-82/HPA-291 visual assets. No need for a second audio ticket.

## Existing code contracts checked on \`main\`

- \`CombatFx\` is a single \`Node3D\` in \`godot/scenes/main.tscn\` using \`combat_fx.gd\`. It currently owns one \`AudioStreamPlayer\` and generates a 60 ms sawtooth blip via \`_make_blip()\`. It also owns tracer/hit/death meshes and \`clear_all()\` for restart.
- \`battlefield_controller.gd::_refresh_session()\` reads \`session_snapshot()\` once per frame, drains \`drain_combat_events()\` while Playing and on the **first Result frame**, and calls \`_dispatch_combat_event()\`. This is the integration point, not a new Godot event bus.
- Each combat dictionary has \`attacker_kind\` (\`unit\`, \`building\` or scrubbed), \`attacker\` (stable ID), \`target_kind\`, \`target_id\`, hit \`x/y\`, \`ranged\`, and \`killed\`. **These are identity categories, not specific unit/building kinds.** The current controller resolves \`_unit_view(id)\` and \`_building_view(id)\`; view metadata contains \`unit_kind\`, \`building_kind\`, and \`team_id\`. The bridge scrubs hidden enemy attacker identity to unit ID 0 so effects cannot reveal unseen positions.
- The target of a killing blow may already have lost its Godot view when GDScript drains the event. Resolve victim/attacker kind from a live view **or a bounded previous-frame stable-ID-to-kind cache** collected from view metadata. Refresh/prune the cache *after* processing each frame's events. A unit spawned and killed entirely between rendered frames may be unknown: use generic impact/silence, never guess a specific alien kind or change Rust just for cosmetics.
- \`_refresh_hud()\` already reads \`wave_snapshot()\` (with \`next_wave\`, \`incoming_zones\` and \`seconds_remaining\`) and \`economy_snapshot()\` (with \`surface_efficiency\`). \`_refresh_session_overlay()\` knows Start, Playing, Paused, Result and \`winner_team\`. \`_restart_match()\` already calls \`_fx.clear_all()\`. Reuse those existing reads and transitions.
- \`godot/scripts/{combat_lifecycle_smoke_test,wave_smoke_test,economy_smoke_test}.gd\` cover real combat/result/restart, public warning windows and a real heat deficit respectively; extend them instead of creating an eighth headless smoke runner.

## Asset inventory: 18 original audio files

Use concise, deterministic, snake_case paths in \`res://assets/audio/\`. Prefer compressed OGG Vorbis for all shipped files; WAV is acceptable for a short effect when quality or import behavior justifies its size. The table names use \`.ogg\` as the default actual deliverable, not an arbitrary naming generator.

| Event | File under \`godot/assets/audio/\` | Sound direction |
| --- | --- | --- |
| Gun Turret fire | \`combat/gun_turret_fire.ogg\` | Short mechanical shot; dry, sharp |
| Flame Tower fire | \`combat/flame_tower_fire.ogg\` | Brief ignition/whoosh; not a long wall of noise |
| Crystal Lance fire | \`combat/crystal_lance_fire.ogg\` | Distinct crystalline, charged strike |
| Guard attack | \`combat/guard_attack.ogg\` | Heavy close-combat swing/hit |
| Rifleman attack | \`combat/rifleman_shot.ogg\` | Tight rifle crack |
| Swarmer attack | \`combat/swarmer_attack.ogg\` | Fast, small chitter/scrape |
| Spitter attack | \`combat/spitter_attack.ogg\` | Wet, acidic spit |
| Brute attack | \`combat/brute_attack.ogg\` | Deep crushing impact |
| Behemoth attack | \`combat/behemoth_attack.ogg\` | Massive, low-frequency strike |
| Swarmer death | \`combat/swarmer_death.ogg\` | Small, quick screech/collapse |
| Spitter death | \`combat/spitter_death.ogg\` | Short, wet dissolution |
| Brute death | \`combat/brute_death.ogg\` | Heavy collapse, recognizably larger |
| Behemoth death | \`combat/behemoth_death.ogg\` | Longer boss-scale collapse |
| Incoming wave | \`alerts/wave_horn.ogg\` | Recognizable warning horn, nonverbal |
| Surface freeze | \`alerts/freeze_warning.ogg\` | Short icy alarm when efficiency first drops |
| Victory | \`alerts/victory.ogg\` | Positive, concise completion stinger |
| Defeat | \`alerts/defeat.ogg\` | Dark, concise failure stinger |
| Wind loop | \`ambience/winter_wind_loop.ogg\` | Quiet seamless cold wind; no hard loop seam |

**18 files total:** 13 combat, 4 alerts/outcome, 1 ambience. Deliver every listed cue, rather than conflating alien attacks with deaths. Aim for very short combat samples, alerts around 1–2 seconds, result stingers a few seconds, and a roughly 15–30-second seamless ambient bed; adjust by listening, not by artificially enforcing durations. Remove DC offset, clipping, excess silence, abrupt sample endpoints and loudness mismatches. Keep prompts/source records, license/rights where applicable, attribution, and any editing steps in \`godot/assets/audio/README.md\`. Runtime needs only the committed audio files and Godot's normal import settings—no generated-on-launch or network-loaded sound.

**Art prototype:** create and import *three* representative sounds first (Gun Turret shot, Swarmer death, wind loop). Play them together in a real 1280×720 Godot session, settle loudness/contrast and wind loop quality, then produce the other 15 sounds. One cohesive sound family, not a collection of unrelated loud stock clips.

## Presentation integration

### Combat: small cue router in existing \`CombatFx\`

Replace \`_make_blip()\` with static paths or a small \`Dictionary\` mapping existing gameplay kind names to audio streams; load/cached streams once on \`_ready()\`, without per-hit disk reads or a new autoload. Keep the current tracer/hit/death visual methods unchanged. \`_dispatch_combat_event()\` selects one attack sound by attacker kind (building: GunTurret/FlameTower/CrystalLance; unit: Guard/Rifleman/four alien kinds) and an additional death sound when \`killed\` identifies an alien victim. Ordinary impact visuals still play for all hits. Do not emit a death cue for a surviving unit. If a lookup is unavailable, keep impact feedback but omit the misidentified specific sound.

**Visibility:** do not infer hidden alien attack types from a scrubbed \`attacker=0\`; no audio based on hidden enemy positions, and no new reveal logic. A sound for a visible alien's action is fine; unseen attackers remain generic/silent. Stable ID cache is purely presentational and cleared on restart; never used to issue commands or remember gameplay state.

**Mixing:** use a small bounded pool (suggest up to 8 reusable non-spatial \`AudioStreamPlayer\` one-shot voices) for overlapping combat hits; the existing one-player model would cut off each preceding hit. Bound the number started per rendered frame and rate-limit common categories to avoid a 150-alien wave becoming constant noise, especially with 20× sim speed. Prefer representative audio over dropping visual impacts; alerts/outcomes must not compete for a combat voice. This is an RTS wide-view mix, so non-spatial players are sufficient for MVP; no per-unit 3D emitters, bus tree or spatialization framework.

### Wave, heat, ambient and result

- **Wave horn:** the first transition to nonempty \`incoming_zones\` for a new \`next_wave\` plays once. Keep the announced wave number as the dedup key; do not re-trigger each frame, on minimap ping expiry, or when several zones are listed. Use *only* the existing authoritative wave snapshot.
- **Freeze warning:** once on the Playing-state warm-to-cold transition when \`surface_efficiency < 1.0\`. Re-arm only after the base reaches full efficiency again. No repeated siren while the value remains below 100%, and no warnings while Start/Paused/Result. This is not a separate heat model.
- **Wind:** one quiet looping \`AudioStreamPlayer\` under \`CombatFx\`, begun in Playing, paused/resumed with the session, and stopped on Result or Restart. Listen for gapless loop boundaries and for alerts to remain clearly audible.
- **Victory / Defeat:** one stinger on *entry* into Result, chosen from \`session_snapshot().winner_team == 1\`. Stop or duck wind and stop combat one-shots before the stinger; the settle tick's existing visual hit/death effect must still render once. Re-entering \`_process()\` must not replay the stinger.
- **Session transitions:** Start is silent; Pause freezes audio without queuing/replaying old warning/attack effects; Resume continues the wind without replaying the last horn/cold warning; Restart clears every playing one-shot, ambience, warning dedup key, last-known identities and result flag. Maintain existing \`_fx.clear_all()\` responsibilities, extended for audio.
- **UI accessibility:** leave text and HUD signals intact. Alerts reinforce, never replace, existing wave and surface-efficiency indicators. Do not auto-unpause or change any gameplay rule to play a sound.

## Verification and acceptance

1. All **18** named OGG/WAV assets are committed and importable using Godot 4.7.1; wind loops seamlessly; provenance/rights and basic source/processing notes are in the audio README.
2. Specific tower/human/alien attack clips and four separate alien death clips are routed by *actual* presentation kind, with a safe fallback for a missing or fog-scrubbed ID; no change to \`CombatEvent\` or Rust/bridge schema.
3. One incoming-wave horn per announced wave, one cold warning per new deficit, one Victory/Defeat stinger per Result; no repeated cues on frame refresh, paused gameplay, or after restart. Existing text/icons and gameplay remain unchanged.
4. Bursty combat has bounded overlapping audio with clearly audible priority alerts and a quiet ambient bed. Test in a real audible Godot session (wind, a populated wave, heat loss/recovery, win/lose, restart); do **not** treat a headless audio smoke as proof of sound quality.
5. Extend existing Godot smoke assertions for asset paths/loading, correct kind routing, warning/recovery dedup, Result-entry/restart hygiene and paused sound behavior; retain their real gameplay legs, viewport checks, success markers and assertions. Use an inspectable last-cue/active-stream field only if needed, not a generic audio test framework.
6. Run fresh Godot import plus all **seven** existing headless smokes, and existing Rust/CI gates as regression evidence. Never claim tests passed from merely adding these documents. No \`crates/\` edits; this planning draft alone does not satisfy the runtime acceptance.

## Deliberate tradeoffs

One dedicated audio ticket and one PR, a finite 18-file set, existing events/snapshots, ordinary Godot players and a small cache are enough. No real-time mixing engine, server, codecs/transcoding pipeline, new gameplay events, backward-compatibility adapters, or separate audio implementation project.