# Frostline battlefield framing and command-panel clarity

## Intent

Address the latest mockup comparison on draft PR #24: the real base appears too small at the opening view, while the command panel's repeated painted frames compete with its action icons. Improve their visual balance without fabricating the illustrative city.

## Approach

Use a closer initial orthographic camera view and quieter treatments of the existing command-tile art. Keep the existing models, fixture, fog, artwork and player-controlled pan/zoom; a full visual-world redesign would exceed this presentation pass. The starting view trades some map coverage for larger units and buildings, so players must still be able to zoom out to the original view and beyond.

## Constraints

Do not change Rust authority, commands, rules, snapshots, world-model size or fixture content. Preserve pan, zoom limits, picking, minimap, placement, focus, accessibility, session transitions and every real command, including clickable unaffordable actions and disabled locks. At 1280 × 720 retain three complete visible underground rows, scrolling, clipping and usable commands. Do not add art or invent statistics, a city or a gameplay Victory.

Use existing Godot controls, fonts and assets. Validate with real-input smokes and final-head 1280 × 720 rendered comparisons. Accurately label synthetic probes and record all failures, retries and limits. Keep PR #24 draft; Linux checks are waived.
