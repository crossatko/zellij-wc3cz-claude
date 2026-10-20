# WC3 Icons (colour icon font for the zellij claude-tabs plugin)

`wc3-icons.ttf` is a tiny CBDT colour bitmap font (family `WC3 Icons`, 99 glyphs: 7 race icons +
92 character icons) that the `claude-tabs` zellij plugin uses when `race_font` is on (default on Linux).
kitty picks it up through
`symbol_map U+1F7E0-U+1F7E6,U+1FA70-U+1FAFF WC3 Icons` in `kitty.conf` (race icons + character icons).

Race icons (`icons/`):

| Codepoint | Race / folder | Source icon (`icons/`) |
|-----------|---------------|------------------------|
| U+1F7E0   | human         | `BTNPeasant.webp`      |
| U+1F7E1   | orc           | `BTNPeon.webp`         |
| U+1F7E2   | undead        | `BTNAcolyte.webp`      |
| U+1F7E3   | nightelf      | `BTNWisp.webp`         |
| U+1F7E4   | naga          | `BTNNagaMyrmidon.webp` |
| U+1F7E5   | demon         | `BTNPitLord.webp`      |
| U+1F7E6   | neutral       | `BTNHeroTinker.webp`   |

Character icons: one per Czech voice-line character, defined by `characters.tsv` (columns `race character
codepoint clips icon_file`; codepoints U+1FA70..U+1FAD6 (the first 92 emoji-presentation wide codepoints of U+1FA70..), sorted by race then
character; `icon_file` is a 64x64 `icons/chars/BTN*.webp`, empty = no glyph, the plugin falls back to the
race icon). The plugin includes the TSV at compile time. Do not renumber codepoints without rebuilding
both the font and the plugin.

Why emoji codepoints: kitty renders a `symbol_map` font's glyphs for private-use codepoints (tried U+F0100..,
and U+E0100, CJK ext. U+20000/U+2A700/U+30000, unassigned U+1F7EC/U+1FA00/U+1F2F0) as monochrome alpha masks
in the foreground colour (grey blobs, 1 cell wide, or 2 for CJK), only emoji-presentation codepoints (here
U+1F7E0.. and U+1FA70..) get the colour bitmap, and they are 2 cells wide for kitty and for `unicode-width`
0.2.2 (checked all 92 + 7), so the plugin pads nothing. Side effect: in kitty the coloured-circle emoji
U+1F7E0..U+1F7E6 and U+1FA70..U+1FAFF (newer emoji) show as WC3 icons everywhere.

## Install

- Linux: the package `setup.sh` copies `wc3-icons.ttf` to
  `~/.local/share/fonts/wc3-icons.ttf` and run `fc-cache -f ~/.local/share/fonts`. Then reload kitty
  (ctrl+shift+f5) or restart it. Check: `fc-list | grep 'WC3 Icons'`.
- macOS: NOT installed. kitty on macOS renders through CoreText, which has no CBDT support, so the
  plugin uses plain unicode symbols there (`race_font "auto"`). The `symbol_map` line is harmless
  when the font is missing.

## Rebuild

`build.py` reads `characters.tsv` and builds both sets. Needs python3 with `fonttools` and `Pillow` (Arch: `python-fonttools python-pillow`).

```sh
cd assets/wc3-icons
python3 build.py        # = FAMILY="WC3 Icons" PPEM=96 BY=116 ASPECT=0.7778 python3 build.py wc3-icons.ttf 1212 4 144
cp wc3-icons.ttf ~/.local/share/fonts/ && fc-cache -f ~/.local/share/fonts   # then restart kitty
```

The font is about 2.9 MB (each glyph is a ~30 KB PNG strike). Arguments: advance width in font units (1212 = 2 cells), crop in px
(removes the 4 px button frame, applied per icon only when `has_frame()` finds the grey frame; all 99 shipped icons
have it), bitmap height in px (144). Env: `ASPECT` = bitmap width/height (0.7778, bitmap 112 x 144), `CROP_KEEP` = fraction of the width kept
by the centre crop (default = `ASPECT`, so the icon is cropped to the bitmap aspect and NEVER squashed; a smaller
value stretches, a larger one squashes), `PPEM` = strike size, `BY` = bearing Y, `BX` = bearing X.

Design: the bitmap is 112 x 144 strike px (aspect 0.7778, centre-cropped from the icon, never squashed) at PPEM 96.
kitty at font_size 18 (24 px em) scales a strike by 24/96 = 0.25 -> exactly 28 x 36 px = the 2-cell box of the
owner's 14x36 cells (font_size 18, `modify_font cell_height 110%`).

### The thin line above the icon (fixed 2026-10-20)
Root cause (measured in an isolated kitty with the owner's real kitty.conf, solid-colour test bitmaps, pixel
analysis of a `grim` crop of that window only): with the old 145 px / PPEM 97 bitmap the scaled height is
145*24/97 = 35.88 px, not 36. kitty resampled it to ~35.4 px, so the top cell row was only ~41-50 % covered
(rows: 0.41, 1, 1, ..., 1; next line again 0.41), i.e. a half-background row above every icon and between two
icon lines. The bottom row was full. It is not a bearing error: BY only moves the icon in 0.25 px steps.
kitty also shrinks a colour bitmap by whole pixels as soon as it exceeds the box, so S = 146..150 at PPEM 97
(36.1..37.1 px, and W rounding to 114+ -> 28.2 px > 28) got WORSE (icon 35.0..35.6 px high, 26.5 px wide,
partial top and bottom rows). The fix is to hit the box exactly and not a hair over or under: PPEM 96,
S 144, W 112 (144*0.25 = 36.00, 112*0.25 = 28.00), BY 116 (BY 115 -> top row 47 % covered, BY 117 -> bottom row
47 % covered; each BY unit = 0.25 px). Result at 14x36: icon 28.0 x 36.0 px, gaps 0/0/0/0 (all 14 tested
icons, two stacked lines, no seam, no overlap, nothing drawn in the blank lines around).
Equivalent exact pairs for other box heights: `S/PPEM = ch/(font_px)`; for 14x36 also S 150 / PPEM 100 or
S 147 / PPEM 98 give 36.0 but widths 28.1/27.7 (slightly worse), so 144/96 is the default.

### Other cell sizes and terminals (measured 2026-10-20, Linux/Hyprland, kitty 0.49.2, alacritty 0.17.0,
ghostty 1.3.1; solid-colour test font, same bitmap geometry; "px" = measured ink box)

kitty (`symbol_map`; real kitty.conf, cell sizes via font_size + `modify_font`, same bitmap):

| cell WxH | font_size | icon w x h (px) | gap top/bottom | left/right | note |
|----------|-----------|-----------------|----------------|-----------|------|
| 14x36 (owner) | 18, cell_height 110% | 28.0 x 36.0 | 0 / 0 | 0 / 0 | exact fit, no line |
| 8x16  | 9  | 11.4 x 14.7 | 1.0 / 0.3 | 0 / 4.6 | undistorted, left aligned |
| 10x22 | 11 | 16.2 x 20.7 | 1.3 / 0 | 0 / 3.8 | undistorted |
| 18x40 | 24 | 30.2 x 36.0 | 4 / 0 | 0 / 5.8 | ~8 % wider than tall |
| 9x30  | 11 + cell_height | 17.3 x 22.2 | 3 / 4.8 | 0 / 0.7 | undistorted |
| 22x51 | 30 | not measured | | | |

kitty keeps the aspect (within ~8 %), never exceeds the cell, and only the 14x36 reference is pixel exact; at
other cell sizes the icon is smaller than the 2-cell box (left/bottom aligned) because the strike scales with
the FONT size, not the cell. Retune (S/PPEM so that S*font_px/PPEM = cell height) for another fixed setup.

alacritty (0.17, no `symbol_map`; fonts come from fontconfig fallback):
- With a stock fontconfig it never uses WC3 Icons for U+1F7E0.. / U+1FA70..: it shows other fonts' glyphs
  (Noto Sans Symbols 2 / Font Awesome outlines, Noto/Apple Color Emoji when installed, tofu). With the emoji
  fonts hidden it uses ours only for some codepoints. Forcing it works with a fontconfig rule that appends the
  icon font to the terminal font's family list: `<match target="pattern"><test name="family"><string>Paper Mono
  </string></test><edit name="family" mode="append" binding="same"><string>WC3 Icons</string></edit></match>`
  (all 13 tested codepoints then used it).
- It draws the colour bitmap at fixed 24/96-ish scale (0.241): 27 x 35 px at font.size 18, NOT fitted to the
  cell; natural cell 15x29 at 100 % line height, so a 35 px icon overflows ~6 px upwards over the previous line
  (bottom aligned). `font.offset.y = 6` gives a 35 px line pitch and a seamless column of icons; icon width 27
  of a 30 px box (3 px right margin). Undistorted. CSI 16t is not answered (cell size from pixel pitch).

ghostty (1.3.1, GTK):
- Ignores the fontconfig fallback for emoji codepoints: always draws the system emoji font (here Noto/Apple
  Color Emoji), also with the rule above. It works with `font-codepoint-map = U+1F7E0-U+1F7E6=WC3 Icons` (and
  `U+1FA70-U+1FAD6=WC3 Icons`): colour bitmap drawn, undistorted, scaled by the font's NATURAL cell (not the
  adjusted one) and centred: 22 x 29 px at natural 15x29 (exact height fit, 4 px side gaps), 23 x 29 in
  14x36 cells (adjust-cell-height; 3-4 px gap top/bottom), 11 x 14 in 8x16, 15 x 18 in 10x22, 29 x 39 in
  18x40 (fills); in a narrow/tall 9x30 cell it overflows the 2-cell box horizontally (about 4 px each side; the
  vertical extent there was not measured cleanly). So in ghostty the icons are not full height unless the line height is left at 100 %.

Not installed here, so not tested and not claimed: wezterm, foot, konsole, gnome-terminal, xterm, st,
terminator, tilix, rio, contour, Terminal.app/iTerm2 (CoreText has no CBDT anyway, see Install). Mitigations
(not implemented): for alacritty ship the fontconfig snippet in `setup.sh`; for ghostty document the
`font-codepoint-map` lines; for terminals that cannot draw CBDT the plugin shows a tofu or another emoji font's
glyph instead of the icon, so `race_font off` (plain unicode symbols) is the fallback to document.

Limits of the CBDT small metrics: height/width <= 255 and BY <= 127 as bytes. When testing, give test builds a
different `FAMILY` (an installed font with the same family silently wins; use a private FONTCONFIG_FILE and
XDG_DATA_HOME, or the real user fonts dir is scanned too).

## Swap an icon

Race icon: put a 64x64 Warcraft III button icon (`BTN*.webp`, or any 64x64 image Pillow reads) into
`icons/`, replace its name in the `ICONS` list in `build.py` (order = race codepoints above).
Character icon: put the image into `icons/chars/` and change that row's `icon_file` in `characters.tsv`
(do not touch codepoints). Then rebuild, reinstall, restart kitty. Source of all icons: the site's
zip `warcraft3_icons.zip` (link on the site), PNGs converted to 64x64 webp (some are 256x256
Reforged-style art, downscaled).

## Notice

The icon artwork is from Warcraft III, (c) Blizzard Entertainment. The button images come from
https://wc3icons.coffbox.win/ (bulk zip: https://github.com/CoffMakesThings/wc3-icons/releases/download/1.0.0/warcraft3_icons.zip;
no licence statement found on the site). They are third-party game content,
included in this repo at the repo owner's decision; no other rights are claimed. `build.py` is part
of this package.
