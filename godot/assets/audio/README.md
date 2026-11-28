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

# HPA-1208 intro voice-over

The five story-intro lines in `voice/intro_ja/` were generated on 2026-11-28
with ElevenLabs text to speech, model `eleven_v4`, voice "ryo kazuma"
(`jyogAI7BN5DyoykAgSk6`), chosen by the owner from two takes per line. They are
Japanese while the on-screen captions stay English; localisation is a later task.

| Asset | Script | Duration |
| --- | --- | --- |
| `voice/intro_ja/line_1.ogg` | [dramatically] 空は……灰色に閉ざされ、[pause] 冬は、終わらなくなった。 | 5.4 s |
| `voice/intro_ja/line_2.ogg` | [ominous] そして……[pause] 奴らが、雪とともに降りてきた。 | 4.8 s |
| `voice/intro_ja/line_3.ogg` | [sorrowful] ひとつ……またひとつと。[pause] 都市は、沈黙していった。 | 6.2 s |
| `voice/intro_ja/line_4.ogg` | [dramatically] グルス。[pause] 今も火を灯し続ける……[resolute] 最後の都市。 | 5.2 s |
| `voice/intro_ja/line_5.ogg` | [grave] その研究センターでは……奴らを滅ぼす、唯一の兵器が造られている。[pause] [commanding] 城壁を守れ。[short pause] 火を絶やすな。[short pause] [intense] 時間を……稼げ。 | 11.3 s |

The takes are mono MP3s. Each was converted without trimming or fades, with a
single gain (+0.8, +1.8, +3.8, -1.4 and -3.0 dB) that brings every line to
-21 LUFS, and upmixed to stereo because the native Vorbis encoder needs two
channels:

```sh
ffmpeg -i INPUT.mp3 -af volume=GAINdB -ac 2 -c:a vorbis -strict experimental -q:a 4 OUTPUT.ogg
```

The same distribution caveat applies: check ElevenLabs' terms before the
voice files ship in a public build or repository.
