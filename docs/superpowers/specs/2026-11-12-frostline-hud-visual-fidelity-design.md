# Frostline HUD visual fidelity

## Intent and scope

Bring the HUD closer to the approved 1280 × 720 Frostline mockups after the first alignment pass. This is HUD presentation only: the in-world models and gameplay remain unchanged.

## Approaches considered

- **Closer mockup match (chosen):** reuse the existing painted Frostline assets and Godot controls to improve the Start, Result, queue and underground presentations while retaining their behavior.
- **Targeted polish:** fix only the Result title and backdrop and adjust spacing; smaller, but leaves the other visible differences.

## Design

- **Result:** center the Victory/Defeat title within the visible banner artwork, and replace the hard horizontal backdrop boundary with a continuous treatment. Keep the real world and snapshot-derived figures; do not fabricate statistics or a gameplay Victory.
- **Start:** use the existing painted status/action icons and plate styling for icon-led objectives and a horizontal controls strip; give the Start action a brighter, beveled Frostline treatment. Preserve all currently documented controls and the actual Start button.
- **Queue:** use existing painted socket/plate assets to make FIFO sockets visually closer to the mockup. Keep five reusable sockets, head timer and progress, and overflow.
- **Underground:** use the existing painted tile/slot treatments and improve room-icon and cost readability. Show excavation progress more clearly using existing snapshots. Keep at least three complete visible rows, clipping, scrolling, Analyze visibility and clickable command controls with populated rows.

## Constraints and verification

Reuse the existing art, fonts, theme and snapshots; do not introduce new artwork, change world models, game rules, command authority or available statistics. Preserve Start/Pause/Result transitions, named controls, input, focus, accessibility, queue state and drawer behavior. Add focused real-input and layout regressions, run the applicable non-Linux checks, and capture fresh final-head 1280 × 720 images for direct mockup comparison. Label presentation-only Victory and any synthetic deeper-level probe accurately. Continue on PR #24; skip the previously waived Linux checks.
