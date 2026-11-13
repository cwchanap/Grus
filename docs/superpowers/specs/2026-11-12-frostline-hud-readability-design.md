# Frostline HUD readability and visual polish

## Intent and scope

Address the remaining differences identified in the latest 1280 × 720 mockup review, with underground readability first. This is presentation-only work on the existing draft PR #24; preserve the existing game, world models, Rust authority, commands and snapshot-derived data.

## Approaches

- **Full HUD polish (chosen):** prioritize underground readability, then make focused Start, Result and queue improvements. This addresses the review as a whole while retaining the existing Frostline assets and controls.
- **Underground only:** a smaller pass on the largest readability issue, but leaves the other reviewed differences in place.

## Design

- **Underground:** make room names, costs, lock requirements and benefits easier to read by reducing visual competition from ornate slot frames, increasing type size where space permits and strengthening contrast. Emphasize excavation progress without hiding its snapshot-derived values. Preserve distinct room states and understandable requirements.
- **Start:** improve the legibility and visual hierarchy of the existing text controls strip. Keep all documented controls and the existing Start action; separate key and mouse glyphs remain an accepted mockup difference.
- **Result:** improve the prominence of the banner, frost treatment and replay controls, and review the backdrop against the mockup. Preserve the real battlefield and snapshot-derived information; do not manufacture a victory.
- **Queue:** make the active item, timer and progress easier to distinguish from the queued and empty sockets. Keep all five sockets, overflow and existing queue behavior.

Reuse existing artwork, fonts, theme and Godot controls; do not add artwork or fabricate gameplay information. The exact spacing and styling are implementation choices, subject to the constraints below and comparison with rendered frames.

## Constraints and verification

At 1280 × 720 keep at least three complete underground rows visible, with scrolling, clipping, accessible labels, focus and usable command controls. Preserve Start/Pause/Result transitions, selection, input and queue functionality. Do not change gameplay rules or in-world models.

Add focused layout and real-input regression coverage where behavior or layout is at risk. Run applicable non-Linux checks and capture fresh final-head 1280 × 720 frames for direct comparison with the approved mockups. Label presentation-only Victory and synthetic deeper-level probes accurately. Keep the existing PR draft and skip the previously waived Linux checks. Report remaining visual gaps and any failed or retried checks honestly.
