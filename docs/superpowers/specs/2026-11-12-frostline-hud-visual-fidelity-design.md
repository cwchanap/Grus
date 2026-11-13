# Frostline HUD visual fidelity

## Intent and scope

Bring the HUD closer to the approved 1280 × 720 Frostline mockups after the first alignment pass. This is HUD presentation only: the in-world models and gameplay remain unchanged.

## Approaches considered

- **Closer mockup match (chosen):** reuse the existing painted Frostline assets and Godot controls to improve the Start, Result, queue and underground presentations while retaining their behavior.
- **Targeted polish:** fix only the Result title and backdrop and adjust spacing; smaller, but leaves the other visible differences.

## Design

- **Result:** center the Victory/Defeat titles within their respective visible banner cloth. Match the clear background beyond the map edge to the unexplored fog color to address the boundary visible through the existing full-screen dim; this also changes the background beyond the map during gameplay. Verify the actual rendered result rather than assuming matching color values remove the boundary. Keep the real world and snapshot-derived figures; do not fabricate statistics or a gameplay Victory.
- **Start:** use the existing painted status/action icons and plate styling for icon-led objectives and a horizontal controls strip; give the shared Start/Restart action style a brighter, beveled Frostline treatment. Preserve the documented controls, the actual Start button, and the SessionLabel/ControlsLabel contracts. Derive research target and preparation duration from the catalogue and wave snapshot; do not invent a regular wave interval not exposed at Start.
- **Queue:** use the theme's existing square Tile normal and pressed styles for queued and head sockets; retain the recessed empty style. Keep five reusable sockets, head timer and progress, and overflow.
- **Underground:** reuse Tile for kind buttons and use explicit styles for occupied/building, level-locked, armed eligible and unarmed empty slots so disabled completed rooms do not appear locked. Preserve a readable and accessible level requirement. Show excavation progress with the existing themed ProgressBar and snapshot values, outside the row viewport; do not invent hazard-stripe art. Keep at least three complete visible rows, clipping, scrolling, Analyze visibility and clickable command controls with populated rows.

## Constraints and verification

Reuse the existing art, fonts, theme and snapshots; do not introduce new artwork, change world models, game rules, command authority or available statistics. The approved clear-color change affects only background presentation, not world objects or gameplay. Preserve Start/Pause/Result transitions, named controls, input, focus, accessibility, queue state and drawer behavior. Add focused real-input and layout regressions, run the applicable non-Linux checks, and capture fresh final-head 1280 × 720 images for direct mockup comparison with a reusable opt-in smoke capture helper. Label presentation-only Victory and any synthetic deeper-level probe accurately. Continue on PR #24; skip the previously waived Linux checks.
