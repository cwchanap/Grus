# Battlefield access and shutdown diagnostics

## Intent

Make opening-view world targets easier to reach without losing the closer Frostline framing, and investigate the reported Godot audio shutdown diagnostics.

## Approach

Adjust existing camera framing and/or HUD geometry only as needed to clear the obstructed opening targets, with usable pointer margin. Preserve visibility and selection of the actual starting core, workers and resources; do not move fixture content. Compare real rendered frames and exercise commands through input, not test-only camera adjustments. Retain zoom-out, panning, minimap and existing HUD functionality.

Verbose smokes identify outstanding Ogg playback references at shutdown. A standalone local probe reproduced a warning on immediate exit and did not on two delayed exits; this suggests a timing dependency but does not prove a reliable game fix. Establish the real shutdown path and a reproducible regression check before changing it. Preserve normal audio, pause, restart, result and quit behavior; do not hide engine diagnostics or assume a delay is a guarantee.

Use existing assets and Godot controls. Do not change simulation authority, commands, models, fixture, fog or rules. Retain the underground drawer's three complete visible rows and its real-input behavior at 1280 × 720. Document limitations, failures and retries accurately; Linux checks remain waived and PR #24 remains draft.
