# WC3 Icons (colour icon font for the zellij claude-tabs plugin)

`wc3-icons.ttf` is a tiny CBDT colour bitmap font (family `WC3 Icons`, 7 glyphs) that the
`claude-tabs` zellij plugin uses as race icons when `race_font` is on (default on Linux).
kitty picks it up through `symbol_map U+1F7E0-U+1F7E6 WC3 Icons` in `config/kitty/kitty.conf`.

| Codepoint | Race / folder | Source icon (`icons/`) |
|-----------|---------------|------------------------|
| U+1F7E0   | human         | `BTNPeasant.webp`      |
| U+1F7E1   | orc           | `BTNPeon.webp`         |
| U+1F7E2   | undead        | `BTNAcolyte.webp`      |
| U+1F7E3   | nightelf      | `BTNWisp.webp`         |
| U+1F7E4   | naga          | `BTNNagaMyrmidon.webp` |
| U+1F7E5   | demon         | `BTNPitLord.webp`      |
| U+1F7E6   | neutral       | `BTNHeroTinker.webp`   |

Side effect: these codepoints are the coloured-circle emoji, so in kitty they show as WC3 icons everywhere.

## Install

- Linux: the package `setup.sh` copies `wc3-icons.ttf` to
  `~/.local/share/fonts/wc3-icons.ttf` and run `fc-cache -f ~/.local/share/fonts`. Then reload kitty
  (ctrl+shift+f5) or restart it. Check: `fc-list | grep 'WC3 Icons'`.
- macOS: NOT installed. kitty on macOS renders through CoreText, which has no CBDT support, so the
  plugin uses plain unicode symbols there (`race_font "auto"`). The `symbol_map` line is harmless
  when the font is missing.

## Rebuild

Needs python3 with `fonttools` and `Pillow` (Arch: `python-fonttools python-pillow`).

```sh
cd assets/wc3-icons
FAMILY="WC3 Icons" PPEM=97 BY=116 ASPECT=0.7778 python3 build.py wc3-icons.ttf 1212 4 145
cp wc3-icons.ttf ~/.local/share/fonts/ && fc-cache -f ~/.local/share/fonts   # then restart kitty
```

Those are also the defaults (`python3 build.py` alone gives the same font; only the `head`
timestamps differ). Arguments: advance width in font units (1212 = 2 cells), crop in px (removes
the 4 px button frame), bitmap height in px. `ASPECT` = glyph width/height (sides are cropped, not
stretched), `PPEM` = strike size.

Tuning: tuned for kitty `font_size 18` + `modify_font cell_height 110%` (cells 14x36 px, main font
24 px). kitty scales colour bitmaps by main-font-px / PPEM and only shrinks in whole px when the
box is exceeded, so `bitmap_height * 24 / PPEM` (145*24/97 = 35.9) must be just below the cell
height, else a 1 px ribbon gap shows above/below. Other font sizes or cell heights need retuning
(query the cell size with `CSI 16 t`). When testing, give test builds a different `FAMILY`: an
installed font with the same family silently wins.

## Swap an icon

Put a 64x64 Warcraft III button icon (`BTN*.webp`, or any 64x64 image Pillow reads) into `icons/`,
replace its name in the `ICONS` list in `build.py` (order = codepoints above), rebuild, reinstall,
restart kitty.

## Notice

The icon artwork is from Warcraft III, (c) Blizzard Entertainment. The 64x64 button images were
downloaded from https://wc3icons.coffbox.win/ (`BTN*.webp`). They are third-party game content,
included in this repo at the repo owner's decision; no other rights are claimed. `build.py` is part
of this package.
