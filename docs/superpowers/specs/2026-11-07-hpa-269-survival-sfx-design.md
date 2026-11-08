# HPA-269 — Winter Survival combat, wave, and ambience SFX design

**Status:** Draft for review — this commit adds design documents only, no audio assets or runtime changes.
**Linear:** [HPA-269 — Combat, wave, and ambience sound effects](https://linear.app/cwchanap/issue/HPA-269/grus-survival-sfx-combat-wave-and-ambience-sound-effects)
**Parent:** [Winter Survival design](2026-10-21-winter-survival-design.md)
**Execution:** The draft PR for these documents is the **one implementation PR for HPA-269**. Complete SFX generation, Godot integration, tests, and review changes on this branch; do not open a separate coding PR.


## Goal, source and boundaries

Replace the procedural 8-bit blip in `godot/scripts/combat_fx.gd` with a cohesive winter-survival sound set and add wave, cold, wind and outcome cues. **HPA-269 is the dedicated SFX/art ticket**; audio generation and its presentation hookup belong to this one PR, separate from the gameplay implementation tickets.

**Committed source of the sounds:** a small deterministic offline synthesis script, `scripts/build_sfx_assets.py`, using Python standard-library waveform/noise/envelope synthesis and fixed random seeds. This follows the repo's `scripts/build_unit_assets.py` / `scripts/build_winter_assets.py` convention and costs no API credits or licensing fees. It generates temporary PCM samples, then a **single documented ffmpeg cleanup/export pass** creates the committed OGG Vorbis files. The script is authoring-only, not part of game startup, CI, or a new asset pipeline. Capture the seed, settings, command and any manual editing in `godot/assets/audio/README.md`. Use distinctly layered noise/filter/pitch/envelope shapes for weapons, alien cries, stingers and a seamless wind loop; reject artificial-sounding results at the human listening gates rather than silently substituting unrelated stock audio.

**Cleanup versus mixing:** apply the same reproducible ffmpeg steps to each source (trim, short edge fade where appropriate, target loudness and peak ceiling, OGG encode); for looping wind, avoid fade-out/fade-in dips and construct/crossfade a seamless boundary before export. Document exact filter arguments and any per-sound exception. In-game balance is then a **per-cue `volume_db` field** in the one cue table, not re-encoding all 18 files each time the mix changes.

**Human quality gates:** after three prototype sounds are routed through the real Godot scene, **stop implementation and ask the user to listen and approve them**; repeat with all 18 sounds and actual gameplay before marking the PR ready. A coding agent or headless runner must not check these gates off without genuine human listening approval. The first gate controls whether to make the other 15 sounds; the second controls release readiness.

**Runtime scope:** `godot/assets/audio/`, `godot/scripts/combat_fx.gd`, `godot/scripts/battlefield_controller.gd`, existing Godot smoke scripts and minimal scene wiring if needed. The offline source script is the sole `scripts/` addition.

**Explicit exclusions:** `crates/`, bridge/event schema changes, gameplay/balance, new units or buildings, runtime synthesis, networking, third-party middleware, custom audio buses, spatial per-unit emitters, dynamic music, dialogue/voice-over, or changes to other art tickets. No second implementation PR and no separate SFX ticket for this existing task.

## Existing integration contracts checked on `main`

- `CombatFx` in `godot/scenes/main.tscn` currently owns one `AudioStreamPlayer`, a 60 ms procedurally generated blip, transient meshes, and `clear_all()` for effect hygiene. Keep its combat visuals.
- `battlefield_controller.gd::_refresh_session()` drains combat events in Playing and on the **first Result frame**, calls `_dispatch_combat_event()` and renders the result overlay. This path already guarantees the killing blow's visuals reach the Result frame.
- Combat dictionaries carry `attacker_kind` (identity category `unit` or `building`), `attacker` ID, `target_kind`, `target_id`, `ranged`, `killed`, and hit position, **not concrete kind**. Godot unit/building views already expose `unit_kind` / `building_kind` metadata. The bridge scrubs hidden enemy attackers to ID 0. Never infer a hidden attacker's type or add a Rust bridge field.
- A killed target's view can disappear before event drain, so maintain **current** and **previous-frame** ID→kind dictionaries for units and buildings. In `_process()`, scan the two view groups **unconditionally before** `_refresh_session()` (which returns early in Start/Paused), resolve living attackers from the current maps and killed alien targets from current or previous maps, then replace the previous maps with current **after** `_refresh_session()` on **every frame**, regardless of phase. No per-event additional group scans for *audio*, no cache growth/pruning, and no logical reset hook. Keep the existing live-view lookup for **tracer placement** unchanged. Entities born and killed entirely between frames may have no resolvable kind: keep the visual impact but do not invent audio.
- `_refresh_hud()` already reads `wave_snapshot()` and `economy_snapshot()` each frame; `session_snapshot()` supplies Start/Playing/Paused/Result and winner. Reuse those same reads to drive presentation edges; do not make new bridge queries.
- `smoke_test.gd` and `economy_smoke_test.gd` call `GrusBridge.reset_fixture()` directly, bypassing `_restart_match()`. Sound dedup must heal from snapshots across these reset paths, not require a centralized reset caller.
- Existing `combat_lifecycle_smoke_test.gd`, `wave_smoke_test.gd`, `economy_smoke_test.gd` and `defenses_smoke_test.gd` cover the appropriate real journeys. Extend them, not the number of shipped smoke runners.

## Asset inventory: 18 original audio files

Use concise, deterministic snake_case paths in `res://assets/audio/`. **All 18 final assets are OGG Vorbis** (no WAV exceptions); the table is the exact path contract for source generation, Godot preloads and smoke checks.

| Event | File under `godot/assets/audio/` | Sound direction |
| --- | --- | --- |
| Gun Turret fire | `combat/gun_turret_fire.ogg` | Short mechanical shot; dry, sharp |
| Flame Tower fire | `combat/flame_tower_fire.ogg` | Brief ignition/whoosh; not a long wall of noise |
| Crystal Lance fire | `combat/crystal_lance_fire.ogg` | Distinct crystalline, charged strike |
| Guard attack | `combat/guard_attack.ogg` | Heavy close-combat swing/hit |
| Rifleman attack | `combat/rifleman_shot.ogg` | Tight rifle crack |
| Swarmer attack | `combat/swarmer_attack.ogg` | Fast, small chitter/scrape |
| Spitter attack | `combat/spitter_attack.ogg` | Wet, acidic spit |
| Brute attack | `combat/brute_attack.ogg` | Deep crushing impact |
| Behemoth attack | `combat/behemoth_attack.ogg` | Massive, low-frequency strike |
| Swarmer death | `combat/swarmer_death.ogg` | Small, quick screech/collapse |
| Spitter death | `combat/spitter_death.ogg` | Short, wet dissolution |
| Brute death | `combat/brute_death.ogg` | Heavy collapse, recognizably larger |
| Behemoth death | `combat/behemoth_death.ogg` | Longer boss-scale collapse |
| Incoming wave | `alerts/wave_horn.ogg` | Recognizable warning horn, nonverbal |
| Surface freeze | `alerts/freeze_warning.ogg` | Short icy alarm when efficiency first drops |
| Victory | `alerts/victory.ogg` | Positive, concise completion stinger |
| Defeat | `alerts/defeat.ogg` | Dark, concise failure stinger |
| Wind loop | `ambience/winter_wind_loop.ogg` | Quiet seamless cold wind; no hard loop seam |

**18 files total:** 13 combat, 4 alerts/outcome, 1 ambience. Keep attacks separate from deaths. Target brief combat effects, concise 1–2-second warnings, short stingers and roughly 15–30 seconds of seamless ambient wind; adjust from listening. The production script/ffmpeg command and source/rights notes are recorded in `godot/assets/audio/README.md`. Commit source script and generated OGG files; **do not** run generation during application startup or CI.

**Three-clip pilot:** synthesize Gun Turret fire, Swarmer death and winter wind, import them, wire them into their actual in-game event paths, and run a focused smoke assertion before the first **human listening approval**. Only then author the remaining 15 sounds. This avoids a throwaway audio test harness.


## Presentation integration

### One cue table and native Godot polyphony

Replace `_make_blip()` with **one `const CUES` dictionary** in `combat_fx.gd`. Each row declares a cue ID, `preload("res://assets/audio/…ogg")`, starting `volume_db`, `max_polyphony` and a `loop` flag (true only for `winter_wind`). Construct one non-spatial `AudioStreamPlayer` per cue in a loop at `_ready()`. For the wind entry, explicitly set **`(stream as AudioStreamOggVorbis).loop = true`** on the preloaded resource before assigning it to its player: imported OGG streams are not guaranteed to loop by default, and a controller retry after natural playback exhaustion is not a seamless loop. Confirm the source seam and real looping behavior at both human listening gates. Use **Godot's built-in `AudioStreamPlayer.max_polyphony`** (2 for combat cues, 1 for warnings, wind and stingers initially), rather than a hand-managed pool. Keep the existing visual effects unchanged.

Provide one `play_cue(cue_id)` function with a short **per-cue 150 ms wall-clock throttle for combat**, not another per-frame quota. Polyphony naturally caps simultaneous voices of a cue; different categories do not cut off one another. `CUES` also contains adjustable `volume_db` for mixing without touching source audio. Add a tiny read-only `cue_counts` test hook incremented only when playback is actually requested after throttling, not for dropped calls. `CombatFx.clear_all()` stops all in-flight players, clears `cue_counts`, and resets per-cue throttle timestamps; it does **not** reset wave/cold dedup or view-kind maps, which are snapshot-derived. Direct `GrusBridge.reset_fixture()` calls bypass this hook, so tests around those resets compare **before/after count deltas**, not absolute lifetime totals. At 20× speed combat cue counts are nondeterministic: assert a positive increase for combat events, but an **exact +1 transition delta** for a single warning or Result entry. Use an independent player for wind and each Result cue, but they come from the same table. Preserve the killing-blow sound *alongside* the Result stinger; do not duck or stop combat one-shots before the stinger.

**Errors are not all silence:** (a) scrubbed/unknown attacker identity stays silent, with existing visual impact; (b) a **known supported attacking unit/building kind** missing its **attack** mapping calls `push_error()`; (c) a known **alien** killed target missing its **death** mapping calls `push_error()`; (d) a missing/broken asset fails at script load via `preload()`, not `ResourceLoader.exists()` fallback. **No friendly-unit or building death sound is required:** destroyed Worker/Guard/Rifleman units or ResearchCenter/Wall/Gate/other buildings retain the visual death effect but play no death clip and raise **no missing-cue error**. Only the four `ALIEN_KINDS` (`Swarmer`, `Spitter`, `Brute`, `Behemoth`) have death cues. A single existing smoke checks the **final 18 non-null preloads** and attack mappings for three towers, Guard, Rifleman and all four alien kinds, plus exactly four alien-death mappings. The pilot may assert its three provisional entries until the other files land. No missing-sound error for noncombat Worker attacks or scrubbed ID 0.

### Combat identity and routing

Rebuild the **current** unit/building view-kind dictionaries **once at the start of `_process()` even in Start/Paused/Result**, before `_refresh_session()` can return early. Use the current map for alive attackers, current then previous map for a killed alien target, and **update previous maps after `_refresh_session()` regardless of phase**. Keep tracer mesh placement using existing live view resolution. A known Guard/Rifleman or tower gets its own attack clip; each of the four alien kinds has a separate attack and killed-target death clip. Only attempt a death lookup for `killed=true` **and** `target_kind == "unit"` **and** the resolved kind is in `ALIEN_KINDS`; killed human units or buildings produce no death audio and no error. Unknown identity also stays silent. No per-hit node or asset loading.

The bridge already hides attackers not presentable under fog by substituting `attacker=0`; **do not** synthesize any specific sound for those events, look up their IDs elsewhere, or use target positions to leak the enemy's identity.

### Self-healing wave, cold, session and ambience edges

These rules use the **same already-fetched snapshots** as the HUD:

- **Wave horn:** on each frame, if `incoming_zones.is_empty()`, set `_horn_wave = 0`. Otherwise, only while Playing and `next_wave != _horn_wave`, play the horn and set `_horn_wave = next_wave`. The warning window is continuous and separated from the next by a quiet interval, so resetting the fixture to a no-warning Start snapshot re-arms it automatically. Several zones for one wave still yield one horn. Update the empty-state reset in every phase.
- **Freeze warning:** compute `cold = surface_efficiency < 1.0` (exactly the HUD predicate) on **every frame in every phase**. Play the warning only if `Playing && cold && !_was_cold`, then set `_was_cold = cold` unconditionally. Full recovery and a fixture reset to warm automatically re-arm it; a paused deficit does not cause a delayed alarm on Resume.
- **Wind:** the preloaded `AudioStreamOggVorbis` must have `loop = true` set at initialization (from its cue-table loop flag); verify it actually loops without a seam. Derive playback directly from phase each frame: Playing → start if stopped or unpause; Paused → set `stream_paused = true`; Start/Result → stop. Do not emulate looping by restarting an exhausted player. No separate restart timer; wind stays intentionally quiet.
- **Victory/Defeat:** use the existing `result_frame` (first transition into Result) to play exactly one corresponding stinger from `winner_team`. Drain/render the settle-tick killing blow first, then start the stinger; repeat `_process()` calls do not replay.
- **Cleanup:** `CombatFx.clear_all()` frees transient visuals, stops sound streams, **clears test-only `cue_counts`** and resets per-cue wall-clock throttle timestamps. Do not rely on it to reset warning IDs, cold flags or kind maps. Those values recover from live phase/snapshots/views even if `GrusBridge.reset_fixture()` is called without the controller's `_restart_match()`. Smoke tests covering direct fixture resets use count baselines/deltas instead of assuming `clear_all()` ran.

No duplicate heat state, audio lifecycle manager, new events, audio buses or autoplay background music. HUD text/icons remain authoritative for accessibility; audio supplements them.


## Verification and acceptance

1. **Source and format:** one deterministic offline Python sound-authoring script, a documented uniform ffmpeg cleanup/encode command, and exactly **18** committed, importable OGG Vorbis cues under `godot/assets/audio/`; reproducible seed/provenance in audio README. No paid service or runtime generator.
2. **Human gate #1:** the first **3** sounds are actually triggered through real gameplay paths and heard/approved by the user before producing the next 15. Stop execution for feedback; do not count a headless driver or programmatic stream load as listening.
3. **Cue integrity:** one `const CUES` preload table with 18 non-null streams, per-cue `volume_db` and native `max_polyphony`, and an explicit wind `loop=true` setting; known missing **attack** kinds or **alien death** mappings are errors, friendly/building deaths have no death sound or error, scrubbed attackers are silent and all 4 alien deaths are distinct.
4. **State correctness:** one horn per announced wave; one freeze warning per warm→cold edge, including when fixture reset bypasses controller; one stinger on Result; no pause/resume duplicate, no stale playback after restart, no hidden attacker identification. Wind follows session phase.
5. **Headless coverage:** extend existing combat, wave, economy and defenses smokes with a single table-integrity assertion and per-cue counts. Clear counters/throttle timestamps on `clear_all()`; for direct bridge fixture resets compare **relative deltas**. Under 20× acceleration, combat counts must increase but need not equal individual strikes; one-shot warning and Result edges should increase by exactly one per observed transition. Rebuild view-kind maps unconditionally in `_process()` and verify human/building deaths do not log missing-cue errors. No headset-dependent `is_playing()` check—headless Godot may use its Dummy driver—and **no eighth smoke scene**.
6. **CI-grade verification:** after Godot import, each of the **seven existing smoke scenes** must pass its actual success marker, timeout and exit-zero gate as defined in `.github/workflows/ci.yml`. CI also owns Rust/coverage/export/E2E gates; do not duplicate unrelated commands in this Godot-only change.
7. **Human gate #2:** listen to all cues during a real normal-speed playable wave, cold/recovery, pause/resume, restart and both outcomes. Confirm wind seam, balance, dynamic range, lack of clipping and recognizable event roles. Mark ready only after user audible sign-off; do not claim it from smoke output.

Planning-only docs never count as completed assets, runtime tests or human approvals.

## Deliberate tradeoffs

One dedicated audio/art task and one PR. An offline source script plus ffmpeg makes the set cheap and reproducible; human listening is the quality gate. Native Godot polyphony and snapshot-derived edges replace custom audio pools and restart bookkeeping. No middleware, service dependencies, separate runtime mixer, new gameplay events, backward-compatibility layer or second PR.