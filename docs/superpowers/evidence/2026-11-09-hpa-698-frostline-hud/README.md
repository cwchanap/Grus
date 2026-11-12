# HPA-698 Frostline HUD — final-head evidence

Real 1280 × 720 frames captured from the final Frostline visual-fidelity
head on branch `hpa-698-frostline-hud` (Task 3 code commit `f22d60e`).
Every frame comes from an existing real-input smoke journey unless the row
says otherwise. Captures are opt-in (`GRUS_CAPTURE_DIR`), use the reusable
`SmokeHelpers.capture_frame` helper, and wait for
`RenderingServer.frame_post_draw` on a real Metal renderer. The helper returns
before touching the renderer in headless mode, and no frame manufactures
gameplay state. No approved mockup is committed here.

| Frame | Journey / provenance | Captured | sha256 |
| --- | --- | --- | --- |
| `hpa-698-start-1280x720.png` | `smoke_test` real Start before the Start command: icon-led SURVIVE/PROTECT/RESEARCH objectives, horizontal controls strip and bright shared Start action. Pixel-identical to the Task 1 capture. | 2026-11-12 12:11:44 | `97989f426bc28700585251c5819e05eda8948a298637c5831beff75c7fa2180f` |
| `hpa-698-result-defeat-1280x720.png` | `combat_lifecycle_smoke_test` genuine alien-caused Defeat: Defeat title on the red cloth, bottom Result bar with snapshot-derived Wave/Research/Pop/Crystals, Restart/Quit. | 2026-11-12 12:12:27 | `9c873d19f48592c181ce81275d95cb36ef2c3fa3c4e36e1cb828ed655454c111` |
| `hpa-698-result-victory-presentation-probe-1280x720.png` | **Presentation only.** `combat_lifecycle_smoke_test` calls `HUD.refresh_session("Result", 1)` with the controller's per-frame session render suspended for the capture; the Victory title is centered on its own white cloth. This is not a gameplay Victory. | 2026-11-12 12:12:28 | `b64f869380d707efda2b884c0ed62f2d0425a85f3db370d7499932632978e130` |
| `hpa-698-barracks-five-sockets-1280x720.png` | `economy_smoke_test` real Barracks Guard→Rifleman FIFO: pressed head socket (portrait, `17s`, progress) and painted queued socket, recessed empties. Pixel-identical to the Task 2 capture. | 2026-11-12 12:12:55 | `31c91cf279944b29d7cefe894a8daf9f44e95e0ff1d5ad1af4962a4b398d982e` |
| `hpa-698-fifo-overflow-chip-1280x720.png` | `economy_smoke_test` Research Center after six real Worker clicks: five painted sockets plus the amber `+1` overflow chip. | 2026-11-12 12:12:44 | `0aeb4762cf44e2173bd3d8a096f62277419f6d6636a52dc576830e7d094969a9` |
| `hpa-698-underground-l3-lab-dig-1280x720.png` | `underground_smoke_test` paid L3 with completed Lab and Tap, Tap kind armed: locked L1/L2 slots (locked plate + padlock + `L3`), occupied L3 rooms (normal painted plate + room icon), and armed-eligible empty L3 slots (selected plate). Three complete rows stay inside the 152 px clipped viewport; Analyze and the command card stay clear. | 2026-11-12 12:23:46 | `bd4509eadd0dfb582e8e6970bf54d0c2f7931a990b16394111c3ceeea0993853` |
| `hpa-698-underground-dig-progress-1280x720.png` | `underground_smoke_test` real L1 dig in progress: themed ProgressBar fill from the snapshot `progress`, amber `27s left` readout from the snapshot `remaining`, outside the level viewport; the header repeats the same values. | 2026-11-12 12:23:46 | `f32bbe39e1d3d6e238c28ce3783096a1b6e03ae06e3f7566818b379ac699eab0` |
| `hpa-698-underground-unarmed-slots-1280x720.png` | `underground_smoke_test` after disarming the Tap: every empty slot is the flat recessed square; completed Lab/Tap keep their normal painted plates and room icons. This is the fourth slot appearance. | 2026-11-12 12:23:46 | `5dea375bd05a36f998792bc22d32fdca3c783197bfe1806c2b8e98d7fe0abe27` |
| `hpa-698-warm-match-1280x720.png` | **Retained from `6523529`**: warm in-match worker selection and build grid. Its visible HUD regions were not changed by the later Start/Result, queue or drawer passes; the drawer is closed and no production queue is shown. | retained | `3c690ceb7a6c83fde817c30027e463866d749a20273c2d3aec9074d91a009f16` |
| `hpa-698-cold-incoming-warning-1280x720.png` | **Retained from `6523529`**: temporary real-input driver (deleted before commit) produced the authored 30 s North warning on the 15/16 (94%) cold surface. Same retention caveat as the warm frame. | retained | `df60fbe399458c1af90d72e906cc192cdb11f337726be18db5b237eb6b51de3f` |

## Mockup comparison

- **`mockup-1.png` (warm match):** top bar, wave medal, research tube, minimap,
  worker selection and painted build tiles match the mockup's composition.
  Remaining differences are the mockup's illustrative snow drift/lightning art
  and its larger portrait panel; neither exists in the shipped art set.
- **`mockup-2.png` (Barracks and queue):** the FIFO head carries the painted
  selected plate with portrait, timer and progress; queued jobs carry the
  painted normal plate; empties stay recessed; the `+1` overflow chip remains
  legible. The mockup's larger head tile and painted arrow are not reproduced
  because the game keeps five equal reusable sockets.
- **`mockup-3.png` (Underground):** kind tiles reuse the same normal/pressed/
  locked Tile states as the mockup; armed-eligible slots are cyan, locked
  slots carry the padlock and their required level, occupied rooms keep the
  normal painted plate and room icon, and unarmed empty slots are flat recesses.
  The three visible rows and the digging readout follow the mockup's structure.
  Remaining differences: the square Tile art is stretched into the game's wider
  84 × 48 slot plates, the armed empty slots say `Empty` instead of the mockup's
  `+` glyph, the game has no hazard-stripe digging row by design and uses the
  existing theme ProgressBar instead, and the mockup's bottom benefit chips are
  not implemented.
- **`mockup-4.png` (Start, Result and design tokens):** Start's three
  icon-led objectives, one controls strip and bright START match the approved
  treatment; Victory and Defeat titles sit on their own cloths and the Result
  bar is present. Remaining differences: title-case wording, separate key
  glyph cells replaced by one readable strip, and the frozen first-wave
  countdown shown instead of the mockup's illustrative regular interval.

## Limits

- Victory is a presentation-only `refresh_session` probe, not a gameplay win.
- The **only** synthetic presentation probe in this task is the deeper
  L4 four-row scroll check in `underground_smoke_test`; it is not captured in
  these frames. The real journey stops at L3 because the unchanged first-wave
  guard leaves no time to fund L4. The deeper-row frame/behaviour is therefore
  widget-only evidence from the smoke assertions.
- The map-edge clear-colour change from Task 1 is not re-litigated here: its
  before/after captures used different camera framing, so there is no
  controlled pixel comparison. The grey band is substantially improved but a
  faint boundary remains visible in the gameplay map-edge frame.
- Successful headless and real-render runs still log
  `WARNING: 4 ObjectDB instances were leaked at exit` and
  `ERROR: 2 resources still in use at exit`. Earlier QA recorded similar
  shutdown diagnostics, but this pass did not independently confirm an
  identical baseline; the cause is unconfirmed.
- An opt-in real-render `frame_post_draw` stall (observed in Task 2, ~88 s,
  window likely occluded) remains a capture-only risk. No stall occurred in
  this task's four real-render runs.
- Linux checks, Docker, cargo coverage and the e2e exported-build boot test
  were not part of this task; there are no Rust changes.
