# Command-hint contrast on the battlefield

Fix evidence for the final whole-branch review's P2 finding 1: the shared
`HUD/CommandStatus` label was 12 px light ink with no backing, and its armed
Bunkhouse hint lost the end of "right-click or Esc to cancel" where the string
crossed snow.

**Implementation content:** the source that became `99d251c`, on top of `ce51f3b`. The new raw capture was taken before that production commit was created.

## Frames

| Frame | Provenance | UTC | SHA-256 |
| --- | --- | --- | --- |
| `command-tiles-armed.png` | Real Bunkhouse arming through the build button; real pointer click, separate Wall focus ring set by the test; 3 px dark Label outline at this head. Full unmodified 1280 × 720 viewport | 20:17:59 | `5391cac953e006355684556020de4de3b68d62fb321b6045d5b48149ab5cb528` |
| `command-status-before-after-label-4x.png` | **Derived comparison, not a raw frame.** Top: `framing-2026-11-15/command-tiles-armed.png`, captured at implementation `7a66955` and published by `ce51f3b`. Bottom: the frame above. Both cropped to x = 456–600, y = 472–506 and enlarged 4× with nearest-neighbour; a magenta separator was inserted. No other pixel changes | 20:18:26 | `160abf3ef7591daa4729e08ff04cea01f9d44939522d096b346b69cef01e34e7` |

The before frame is unchanged and still the one described by the review
(`9d0bbb416f999e232659072141c524c508904b1b43905feecb1f8e6ad6969dc4`); it was
not overwritten. Both raw frames come from the same real-input main smoke
journey, in separate runs at the same camera size 54, not from a synthetic
probe. Live countdown, research and unit positions differ between the two runs,
so this is not a frozen-state comparison.

## Treatment

The existing label keeps its rectangle (216, 478 → 720, 502), 12 px Barlow and
the shared light Label ink. It gains only the engine-drawn Label outline as a
scoped scene override: `font_outline_color = Color(0.02, 0.03, 0.05, 0.95)` and
`outline_size = 3`, the same dark edge the underground room names already use.
No node, texture, font, control tree, mouse filter or command path changed.

## Measured comparison

Measured on the two raw frames above, in the label band (x = 180–760,
y = 470–499):

- **Before:** bare ink over the snow. Nominal ink `(224, 237, 245)` against the
  mean local ground under the selected pixels is approximately **1.18:1**;
  the review's own background samples in that suffix were 1.10–1.13:1.
- **After:** the same pixels now carry the outline. Their mean edge colour
  gives approximately **10.35:1** against the ink; the darkest edge pixels
  `(15, 17, 23)` give **15.82:1**. Because the edge is painted around every
  glyph, glyph contrast no longer depends on whether the ground is snow or fog.
- **Extent:** 86 pixels that were bright ground (channel sum > 550) are dark
  (sum < 200) in the new frame; 0 move the other way. They cover x = 473–557,
  y = 481–490 — the suffix that previously disappeared, inside the label rect
  and 20 px above the selection card at y = 510.
- **Current fit:** in the same comparison there are zero threshold-selected
  newly dark pixels below y = 500 across x = 180–760, left of x = 216 or right
  of x = 720. Those counts are not bounds for every glyph or outline pixel.
  The frame shows the current hint clear of adjacent controls. The implementer's
  calibration measured a shaped string of 345 × 15 px; its size plus 6 px edge
  fits the 504 × 24 label rectangle, but is not a glyph-bearing containment proof.

These are local pixel samples chosen at the demonstrated suffix, not exhaustive
per-glyph measurements, and they are not a formal accessibility certification.

## Verification

- Headless main smoke: the extended status check is RED against the pre-fix
  scene (`ce51f3b` plus only the test change; `CommandStatus dark edge is 0 px,
  not a thick outline`) and GREEN at `99d251c`. Negative controls:
  `outline_size = 2` fails as "2 px", and a light outline colour fails as "not
  a dark, opaque edge". The real Bunkhouse arming leg additionally asserts the
  controller's exact long hint before capturing.
- All seven headless smoke scenes exited 0 with their usual success markers on
  the source content that became `99d251c`.
- Both rendered main smoke journeys on that source content exited 0; the second, run
  after the negative-control edits were reverted, wrote the raw frame above.

## Limitations

- The label remains 12 px text; the outline is a contrast treatment, not a size
  change. It still sits over the battlefield and can still overlap world
  content, as it did before.
- The new check measures the theme values and the current strings' fit. Another
  locale or a much longer hint is not exercised; the fit guard covers only the
  texts that run in the smoke.
- No screen-reader, OS high-DPI scaling, colour-vision or full keyboard
  traversal check was run. Contrast is sampled from these frames, not measured
  over every background the label can cross.
