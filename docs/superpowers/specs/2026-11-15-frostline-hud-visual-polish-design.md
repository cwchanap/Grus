# Frostline HUD visual polish

## Intent

Address the latest visual review on draft PR #24 without changing gameplay. Prefer quieter uses of the existing Frostline art and controls over new artwork. The underground drawer and Pause screen are the priorities; refine queue, Start and Result where existing assets can improve hierarchy.

## Design

- Underground: reduce competing frame detail and make locked, occupied and actionable states distinguishable without relying on color alone. Retain catalogue- and snapshot-derived text and values.
- Pause: replace the oversized empty panel with a compact, centered Paused/Resume composition; keep the existing Resume action, keyboard focus and Esc behavior.
- Queue: separate the active portrait from its frame and queued/empty sockets without changing FIFO, progress or overflow.
- Start: use existing control cues where they improve the text strip while preserving every instruction and the Start action.
- Result: improve composition using the real battlefield and existing presentation only; don't fabricate a city, statistics or a gameplay Victory.

## Constraints and verification

Reuse existing art, fonts, theme and Godot controls. Do not change Rust authority, commands, rules, world models or snapshots. At 1280 × 720 preserve three complete visible underground rows, scrolling, clipping, accessibility, focus and usable commands. Preserve Start/Pause/Result transitions, selection and real input.

Add focused regression checks and run relevant non-Linux tests. Capture fresh 1280 × 720 frames and compare with the approved mockups; label presentation-only Victory and synthetic deeper-level probes. Record failures, retries, limitations and remaining visual gaps. Keep PR #24 draft; Linux checks are waived.
