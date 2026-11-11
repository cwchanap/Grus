# Frostline mockup alignment

## Intent

Bring the Start screen, production queue, Result screen, and underground drawer closer to the approved HPA-698 mockups at 1280 × 720 without changing gameplay or adding art.

## Approach

Reuse the existing Godot controls, snapshots, fonts, and painted assets. Enlarge the Start title and emblem; make the five queue sockets and icons more prominent; present existing Restart and Quit actions in a bottom Result bar, showing only statistics already available from snapshots; and increase the number of underground level rows visible while keeping command controls unobstructed and usable.

An exact mockup recreation that requires new art or gameplay statistics is out of scope. Leaving the current layout unchanged does not address the reported visual differences.

## Constraints and verification

Preserve Rust gameplay authority, button identities and real-input behavior, queue FIFO and overflow, drawer scrolling and focus, and Start/Pause/Result transitions. Do not fabricate victory or statistics. Update the existing smoke checks and compare fresh rendered frames with the approved mockups. Continue on the existing PR branch; do not rerun the user-waived Linux checks.
