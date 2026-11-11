# HPA-698 Frostline HUD — final-head evidence

Real 1280 × 720 frames captured from the HPA-698 branch (product files
unchanged since `d61224b`; only later test/evidence files differ) with
`godot --path godot …`, a real renderer, saved through
`RenderingServer.frame_post_draw`. Every frame comes from an existing
real-input smoke journey unless noted below; no gameplay state was
manufactured and no approved mockup is committed here.

| Frame | Journey |
| --- | --- |
| `hpa-698-start-1280x720.png` | `smoke_test` Start screen before the real Start command |
| `hpa-698-warm-match-1280x720.png` | `smoke_test` Start → Playing, warm 15/10 surface |
| `hpa-698-cold-incoming-warning-1280x720.png` | Temporary real-input driver (deleted before commit): real Start click, worker click and Barracks/Depot/GunTurret build-tile placement, then the authored 30 s North wave warning on the 15/16 (94%) cold surface |
| `hpa-698-barracks-five-sockets-1280x720.png` | `economy_smoke_test` real Barracks Guard + Rifleman FIFO |
| `hpa-698-fifo-overflow-chip-1280x720.png` | `economy_smoke_test` six real Worker clicks: five sockets + `+1` overflow |
| `hpa-698-underground-l3-lab-dig-1280x720.png` | `underground_smoke_test` paid L3, completed Lab, scrolled levels, Excavate L4 |
| `hpa-698-result-defeat-1280x720.png` | `combat_lifecycle_smoke_test` real alien-caused Defeat |
| `hpa-698-result-victory-presentation-probe-1280x720.png` | **Presentation only.** The existing `combat_lifecycle_smoke_test` both-winner probe calls `HUD.refresh_session("Result", 1)` with the controller paused for the capture; it is not a gameplay victory. A genuine gameplay victory is unavailable: the one-base fixture (300 Scrap, four workers) cannot fund the Lab economy needed to reach 9,000 research before the unbounded wave director, so the real journey above ends in Defeat. |
