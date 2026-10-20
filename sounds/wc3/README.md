# Warcraft III voice lines, Czech dub (claude-tabs notification sounds)

When a Claude Code agent in a zellij tab finishes, the `claude-tabs` plugin plays a random line of
that tab's character from this folder (`play-phrase.sh`, never the same clip twice in a row).

## Layout

```
sounds/wc3/<race>/<character> - <transcript>.flac            active clips (played)
sounds/wc3/_excluded/<race>/<character> - <transcript>.flac  excluded clips (never played)
sounds/wc3/manifest.tsv                                     one row per clip
```

- Races (folders; a character is the file name part before the first ` - `): `human`, `orc`, `undead`, `nightelf`, `naga`, `demon`, `neutral` (heroes are filed under their faction).
- Only files directly inside `<race>/` are played; folders starting with `_` or `.` are ignored, so
  `_excluded/` is never a race. A new folder with audio becomes a new race (generic icon).
- Format: FLAC level 8, mono, 48 kHz, 16 bit, lossless copies of the WAV masters (decoded PCM
  verified identical). Loudness about -20 LUFS, true peak <= -2 dBTP. Plays with `pw-play`/`paplay`
  (Linux, libsndfile) and `afplay` (macOS 10.13+).
- File names are the speaking unit (Czech) and a best-effort transcript, Unicode NFC.

## Pruning / restoring

Move a file between `<race>/` and `_excluded/<race>/` (the plugin rescans every 30 s, no restart):

```sh
cd sounds/wc3   # in the package checkout
git mv "human/Sedlák - Cože.flac" "_excluded/human/"        # stop playing it
git mv "_excluded/human/Kněz - Mohu nějak pomoci.flac" human/   # play it again
```

`manifest.tsv` records the packaging state (`status` active/excluded, `exclusion_reason`); it is
informational, the folders are what counts. Exclusion reasons came from an independent review:
`too long` (> 8 s), `too short` (< 0.7 s), `garbled/unintelligible-suspect` (transcript unreliable),
`uncertain-cut` (cut may clip a word), `loudness outlier` (> 2 LU off), `wrong-character-suspect`,
`leftover music/noise`.

## How the clips were made

Source: the YouTube playlist of the Czech Warcraft III unit responses,
https://www.youtube.com/playlist?list=PLQNF-ZjhDFeDZaqd5qCr6dE60e1XXgNnP (7 videos: humans, orcs,
undead, night elves, naga, heroes, neutral units). Pipeline (scripts and WAV masters are kept
outside this repo):

1. download with yt-dlp (WAV 44.1 kHz),
2. vocal separation with Demucs (`htdemucs`, two stems) to remove music and effects,
3. transcription with Whisper (whisper.cpp, large-v3 q5_0, word timestamps via DTW),
4. segmentation into single lines (transcript + audio pauses, reviewed per video),
5. polish: mono 48 kHz, 70 Hz high-pass, short fades, loudness normalisation to -20 LUFS with a
   true-peak limiter at -2 dBTP; every clip re-transcribed to check the cut,
6. independent validation pass (exclusion list above), then FLAC conversion.

## Notice

The audio is third-party game content: voice recordings from the Czech dub of Warcraft III,
(c) Blizzard Entertainment. It is included in this repository deliberately.
No rights to it are claimed; it is not covered by any licence of this repository. See `NOTICE`.
