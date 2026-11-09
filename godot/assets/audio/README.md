# HPA-269 pilot sound effects

These three local pilot takes were generated on 2026-11-08 with the ElevenLabs
Sound Effects API, model `eleven_text_to_sound_v3`, prompt influence 0.4, and
MP3 source output. Each was the first and only take. The approved OGG, when
selected after listening, is the asset of record; generation is not deterministic.

| Asset | Prompt | Duration | Loop | Cost |
| --- | --- | --- | --- | --- |
| `combat/gun_turret_fire.ogg` | Single automated gun turret firing one dry, sharp mechanical shot, metallic snap with a short low punch and quick decay, no speech or music. | 1 s | false | 10 credits |
| `combat/swarmer_death.ogg` | A small alien creature dying: a brief high-pitched chittering screech followed by a quick soft chitinous collapse, no speech or music. | 1 s | false | 10 credits |
| `ambience/winter_wind_loop.ogg` | Steady quiet winter wind across a frozen barren plain, soft airy gusts, distant cold atmosphere, no voices, animals, music, or abrupt events. | 20 s | true | 200 credits |

Each source was converted without trimming, fades, or loudness changes, using
FFmpeg's native Vorbis encoder (this installation does not include libvorbis):

```sh
ffmpeg -i INPUT.mp3 -c:a vorbis -strict experimental -q:a 4 OUTPUT.ogg
```

The wind was generated with loop enabled; its seam and the in-game mix still
require listening. Do not publish individual files until the public repository
distribution question is resolved: ElevenLabs' [Sound Effects terms](https://elevenlabs.io/creative-terms)
restrict making outputs available as isolated audio files. Account-plan rights,
any attribution requirements, and written permission have not been confirmed.
