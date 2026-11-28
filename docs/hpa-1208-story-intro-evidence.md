# HPA-1208 — story intro before Start: capture evidence

**Date:** 2026-11-27, background motion added 2026-11-28 · **Renderer:** pinned Godot 4.7.1, `--rendering-method gl_compatibility` under Xvfb (software GL), real game scene with the full HUD, 1280×720.

Every image is a real frame of the launched game. Since the intro and Start now move, the frames come from Godot Movie Maker recordings (`--write-movie`, `--fixed-fps 30`, 1280×720) of `main.tscn` against a fresh Godot user data folder: the first launch plays the intro by itself (27 s recorded), and a second launch against the same folder goes straight to Start (13 s recorded). Panel frames are taken 2 s into the panel, once the crossfade and caption have finished. Images are JPEG copies of the PNG frames.

## Against the mockup

Left: the approved mockup boards. Right: the game. The mockup's panel art was a stand-in crop of the Start key art; the game shows the painted HPA-1210 panels that have since landed.

![](superpowers/evidence/hpa-1208/compare-mockup.jpg)

## Frames

| What | Frame |
| --- | --- |
| Mid panel (2 of 5): caption, progress diamonds (first done, second lit), SKIP with the Esc key chip, "continue" hint. | ![](superpowers/evidence/hpa-1208/intro-panel-2.jpg) |
| Last panel (5 of 5): the long caption wraps to two lines, every diamond before it is filled, and the hint says "begin". | ![](superpowers/evidence/hpa-1208/intro-panel-5.jpg) |
| Start after the intro, with the new STORY button under START. | ![](superpowers/evidence/hpa-1208/start-story.jpg) |
| First launch (fresh user folder), 12 s after boot: the intro played by itself and has moved on to panel 3 on its own. `intro_seen` now exists. | ![](superpowers/evidence/hpa-1208/first-launch.jpg) |

A second launch against the same user folder went straight to Start (the Start frame above comes from that launch). A launch with `BEVY_E2E=1` and a fresh user folder showed Start and wrote no `intro_seen`. The exported debug build (the one the bevy-e2e test runs) also played the intro on its first launch.

## Motion

Nine frames from the same recordings, read left to right. Row 1: panel 1 fading in from black at 0.5 s, the same panel at 4.6 s after its push in, and the 1-second crossfade into panel 4 (panel 3's city still showing through). Row 2: panel 3 at 9 s, panel 4 at 19 s (the key art's own dark left side, now pushed in), and the crossfade into panel 5. Row 3: Start at 1 s, 6 s and 12 s; the key art pushes in slowly over 20 s and glides back, under the same snow and fog. Each panel drifts from 1.04× to 1.16× zoom with a 3.5% pan that alternates left and right; Start drifts from 1.02× to 1.10× with a 2.5% pan. Snow is two seeded layers (110 small, slow, faint far flakes and 36 large, fast, bright near flakes that drift further sideways); the fog is a low band of scrolling noise in a shader. All of it runs on unscaled process time, so the smokes' raised `Engine.time_scale` does not speed it up.

![](superpowers/evidence/hpa-1208/motion-frames.jpg)

## What the smokes assert

`smoke_test` drives the intro on the real Start screen before the match starts: the scene does not auto-play it (the smoke instances `main.tscn` under its own root); STORY sits under START, on screen and clear of the controls strip; STORY opens panel 1 and starts the winter wind loop; Tab does not reach the Underground drawer; click, Space, click, Space walk panels 2 to 5 with the right caption and art (panel 4 is the Start key art) and the "begin" hint only on the last; the last click lands on Start and stops the wind; STORY replays from panel 1 and Esc lands on Start; STORY again and SKIP lands on Start; `Space` on Start then does not replay the intro, because STORY holds no keyboard focus; the Start key art is drifting (zoomed past 1× and changing over half a second) under a visible snow and fog layer that ignores the mouse; and no intro click changed the world selection. The other six smokes run unchanged.
