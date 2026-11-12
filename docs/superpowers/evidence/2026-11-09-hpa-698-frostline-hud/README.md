# HPA-698 Frostline HUD — final-head evidence

Real 1280 × 720 frames captured from the Frostline mockup-alignment head on
the HPA-698 branch, with `godot --path godot …`, a real Metal renderer, saved
through `RenderingServer.frame_post_draw`. Six frames were re-captured at the
`8a702de` head; the warm-match and cold-warning frames are retained from the
`6523529` capture because their visible HUD content was unchanged by the
later Start/Result, queue and drawer reflows. Every frame comes from an existing
real-input smoke journey unless noted below; the throwaway capture helper was
deleted before commit, no gameplay state was manufactured and no approved
mockup is committed here. The final fix wave after `8a702de` changed Result
drawer gating, armed build-tile rendering, a hover tooltip and this note; none
of those states is captured in these frames, so no frame was re-captured.

| Frame | Journey |
| --- | --- |
| `hpa-698-start-1280x720.png` | `smoke_test` Start screen before the Start command: enlarged emblem/GRUS lockup and objectives |
| `hpa-698-warm-match-1280x720.png` | Retained from `6523529`: `smoke_test` Start → Playing, warm 15/10 surface |
| `hpa-698-cold-incoming-warning-1280x720.png` | Retained from `6523529`: Temporary real-input driver (deleted before commit): real Start click, worker gather and Barracks/Bunkhouse/GunTurret build-tile placement, then the authored 30 s North wave warning on the 15/16 (94%) cold surface |
| `hpa-698-barracks-five-sockets-1280x720.png` | `economy_smoke_test` real Barracks Guard + Rifleman FIFO with the enlarged head socket |
| `hpa-698-fifo-overflow-chip-1280x720.png` | `economy_smoke_test` six real Worker clicks: five enlarged sockets + `+1` overflow |
| `hpa-698-underground-l3-lab-dig-1280x720.png` | `underground_smoke_test` paid L3 with completed Lab and Tap, Excavate L4; the 152 px clipped scroll viewport shows three complete level rows at once and the command card below stays fully visible and clickable |
| `hpa-698-result-defeat-1280x720.png` | `combat_lifecycle_smoke_test` real alien-caused Defeat with the bottom Result bar, Restart/Quit and snapshot-derived Wave/Research/Pop/Crystals |
| `hpa-698-result-victory-presentation-probe-1280x720.png` | **Presentation only.** The existing `combat_lifecycle_smoke_test` both-winner probe calls `HUD.refresh_session("Result", 1)` with the controller's per-frame session render suspended for the capture; it is not a gameplay victory. A genuine gameplay Victory was not verified during QA; this frame verifies the Victory title/art/Result-bar presentation only. |
