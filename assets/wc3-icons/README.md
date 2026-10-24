# WC3 Icons (colour icon font for the zellij claude-tabs plugin)

`wc3-icons.ttf` is a tiny CBDT colour bitmap font (family `WC3 Icons`, 99 glyphs: 7 race icons +
92 character icons) that the `claude-tabs` zellij plugin uses when `race_font` is on (`auto` = on when the
hook finds the font installed with `fc-list`, Linux only; with `race_font false` or without the font the bar
shows two-letter initials badges). Install it with `fontconfig/install.sh` (see Install); every terminal
then needs one config line (kitty `symbol_map U+1FA70-U+1FAD6 WC3 Icons`, see the README Quick start table).

`wc3-icons-wide.ttf` (family `WC3 Icons Wide`) has the same glyphs with a wider, shorter bitmap (120 x 116)
for cells about half as wide as tall (ghostty, foot, alacritty at 100 % line height, see Terminals below).

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
0.2.2 (checked all 92 + 7), so the plugin pads nothing. Side effect: wherever the font is picked up for these
codepoints, the emoji at the character codepoints U+1FA70..U+1FAD6 show as WC3 icons. The generated fontconfig
rules (`fontconfig/`) cover only those; the 7 race codepoints U+1F7E0..U+1F7E6 (the coloured-circle emoji)
stay with the system emoji font, and a character without an icon glyph then shows that circle emoji.
(An explicit kitty `symbol_map` for U+1F7E0..U+1F7E6 would override the circles in kitty again.)

## Install

- Linux: `fontconfig/install.sh` (or `setup.sh --font`) copies `wc3-icons.ttf` and `wc3-icons-wide.ttf` to
  `~/.local/share/fonts/`, writes `~/.config/fontconfig/conf.d/60-wc3-icons.conf` (prefer `WC3 Icons` for
  the character codepoints only; tag the `WC3 Icons*` families as emoji fonts, lang `und-zsye`, which foot
  needs) and runs `fc-cache -f` on the font dir. Then restart the terminal. Check: `fontconfig/install.sh --check`.
  `--uninstall` removes the files (and a `WC3 Icons Custom` build saved as `wc3-icons-custom.ttf`).
- macOS: `fontconfig/install.sh` (or `setup.sh --font`) hands over to `fontconfig/install-mac.sh`, which copies
  `wc3-icons-mac.ttf` (family `WC3 Icons Mac`, sbix, for kitty) and `wc3-icons-mac-ghostty.ttf` (family `WC3 Icons Wide`, the
  same name as the Linux wide font, for ghostty) to `~/Library/Fonts/` (a foreign file of that name is never
  overwritten; `--uninstall`, `--check`) and prints the terminal line (kitty `symbol_map`, ghostty
  `font-codepoint-map`); restart the terminal completely. The plugin's `race_font auto` is on when that file
  exists in `~/Library/Fonts` or `/Library/Fonts`.

## Rebuild

`build.py` reads `characters.tsv` and builds both sets. Needs python3 with `fonttools` and `Pillow` (Arch: `python-fonttools python-pillow`).

```sh
cd assets/wc3-icons
python3 build.py        # = FAMILY="WC3 Icons" PPEM=96 BY=116 ASPECT=0.7778 python3 build.py wc3-icons.ttf 1212 4 144
FAMILY="WC3 Icons Wide" ASPECT=1.0345 BY=92 python3 build.py wc3-icons-wide.ttf 1212 4 116
sh ../../fontconfig/install.sh   # installs both, then restart the terminal
```

The font is about 2.9 MB (each glyph is a ~30 KB PNG strike). Arguments: advance width in font units (1212 = 2 cells), crop in px
(removes the 4 px button frame, applied per icon only when `has_frame()` finds the grey frame; all 99 shipped icons
have it), bitmap height in px (144). Env: `ASPECT` = bitmap width/height (0.7778, bitmap 112 x 144), `CROP_KEEP` = fraction of the width kept
by the centre crop (default = `ASPECT`, so the icon is cropped to the bitmap aspect and NEVER squashed; a smaller
value stretches, a larger one squashes; above 1 the top and bottom are cropped instead, for wide bitmaps), `PPEM` = strike size, `BY` = bearing Y, `BX` = bearing X.

Design: the bitmap is 112 x 144 strike px (aspect 0.7778, centre-cropped from the icon, never squashed) at PPEM 96.
kitty at font_size 18 (24 px em) scales a strike by 24/96 = 0.25 -> exactly 28 x 36 px = the 2-cell box of the
reference cells of 14x36 px (kitty font_size 18, `modify_font cell_height 110%`).

### The thin line above the icon (fixed 2026-10-20)
Root cause (measured in an isolated kitty with the reference settings above, solid-colour test bitmaps, pixel
analysis of a screenshot crop of that window only): with the old 145 px / PPEM 97 bitmap the scaled height is
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

### Terminals (measured 2026-10-21, Linux (Wayland), kitty 0.49.2, ghostty 1.3.1, alacritty 0.17.0, foot 1.28.0)

Method: one short-lived test window per terminal with an isolated config (private FONTCONFIG_FILE, test fonts
under other family names), a test pattern (cell ruler, red rows above/below two stacked 2-cell icon slots on a
blue background, a tab-bar mimic), a screenshot of that window only, then pixel analysis: a solid-green test
build of the same geometry gives the exact ink box; the real build in the same settings is compared with the
font's own PNG for that codepoint (mean abs error 3-10 of 255, vs 96-118 against Noto Color Emoji), so a
fallback emoji or tofu would fail the test. Text font Paper Mono (cell 0.606 x 1.2 em). "box" = the 2-cell slot.

How each terminal sizes a colour bitmap (verified with variants of PPEM, BY, ascent/descent and aspect):
- kitty: scale font_px/PPEM, shrunk by whole px to fit the box, bottom/left aligned. Pixel exact only when
  S*font_px/PPEM = cell height and W*font_px/PPEM <= 2*cell width.
- ghostty: ignores the font's size/PPEM/BY/ascent completely: fits the bitmap's aspect into
  2 cells x the font's NATURAL line height (adjust-cell-height is not counted), centred. So only ASPECT matters.
- foot (fcft) and alacritty: scale font_px/PPEM, NOT fitted; the top sits BY*scale above the baseline; foot
  clips at the cell, alacritty overflows into the line above. foot uses a fallback font for these
  emoji-presentation codepoints only if fontconfig gives it lang `und-zsye` (install.sh adds that tag).

| Terminal / settings | cells | WC3 Icons (112x144, BY 116) | WC3 Icons Wide (120x116, BY 92) |
|---------------------|-------|-----------------------------|---------------------------------|
| kitty 18, cell_height 110 % | 14x36 | 28x36, exact | (not for kitty) |
| kitty 18, 100 % | 14x33 | 25x32, 1 px top, 3 px right | |
| kitty 16, 110 % | 13x33 | 24x31, 2 px left/bottom | |
| kitty 14, 110 % | 11x29 | 21x26 | |
| kitty 12, 100 % | 10x22 | 16x20 | |
| ghostty 18, adjust-cell-height 0 | 15x29 | 22x29, 4 px each side | 30x29, exact |
| ghostty 18, adjust-cell-height 3 (or 10 %) | 15x32 | 22x29, 4 px sides, 2 px top, 1 px bottom | 30x29, 2 px top, 1 px bottom |
| ghostty 16, adjust-cell-height 3 | 13x28 | 20x25 | |
| ghostty 14 / 12, 0 | 11x23 / 10x19 | 18x23 / 14x19 (side gaps) | |
| foot monospace 8 (defaults, Noto Sans Mono) | 6x16 | 12x16, exact | 13x12, 1 px over the right edge |
| foot Paper Mono 18 | 15x29 | 28x29 visible (clipped) | 30x29, exact |
| foot Paper Mono 12 | 10x20 | | 20x19, 1 px top |
| alacritty 18, offset.y 0 | 15x29 | 28x35, 6 px over the line above | 30x29, exact |
| alacritty 18, offset.y -2 | 15x27 | 28x35, 8 px over | 30x29, 2 px over |
| alacritty 18, offset.y 6 | 15x35 | 28x35, 2 px right gap | |
| alacritty 16 / 14 / 12, 0 | 13x27 / 11x23 / 10x20 | 24x31 / 21x27 / 18x23, 4/4/3 px over | 16: 26x25, 1 px top/bottom |

Custom builds for one exact cell (alacritty/foot: S = cell height * 96 / font_px, W = 2 * cell width * 96 /
font_px, BY = baseline-from-top * 96 / font_px; kitty: S/W as for the default; ghostty: only ASPECT = 2*cw/ch
of the natural cell): name them `WC3 Icons Custom`, save as `~/.local/share/fonts/wc3-icons-custom.ttf`
(install.sh tags and uninstalls that family/file), run `fc-cache -f ~/.local/share/fonts`. Verified example,
alacritty Paper Mono 18 with `font.offset.y = -2` (15x27 cells): `FAMILY="WC3 Icons Custom" ASPECT=1.1111
BY=84 python3 build.py wc3-icons-custom.ttf 1212 4 108` -> 30x27, exact (also stacked, no overlap).

Rule of thumb: kitty with a tall line height -> `WC3 Icons`; ghostty -> `WC3 Icons Wide` (any size);
foot/alacritty -> the variant whose height (1.5 x font px for `WC3 Icons`, 1.21 x font px for Wide) equals
the cell height, else a custom build. Untested: wezterm, konsole, gnome-terminal, xterm, st, rio, contour;
macOS terminals other than kitty (see "macOS font" below). A terminal that cannot draw CBDT shows another emoji or a box:
`race_font false` (initials badges).

Limits of the CBDT small metrics: height/width <= 255 and BY <= 127 as bytes. When testing, give test builds a
different `FAMILY` (an installed font with the same family silently wins; use a private FONTCONFIG_FILE and
XDG_DATA_HOME, or the real user fonts dir is scanned too).

## macOS font (sbix)

CoreText does NOT draw the CBDT font (tested with kitty 0.49.2 on macOS 26: boxes). macOS needs Apple's `sbix`
(PNG strikes), so `build.py` has a second output, selected with `SBIX=1`; same glyphs, codepoints and bitmaps,
but an `sbix` table instead of CBDT/CBLC (plus a full and PostScript name: without them `CTFontCreate...` and kitty
find the family only intermittently). `glyf` stays (empty outlines; a box outline per glyph made CoreText drop
the family, so do not add one). The default (CBDT) builds are unchanged: only the `head` timestamp differs on rebuild.

```sh
cd assets/wc3-icons
FAMILY="WC3 Icons Mac" SBIX=1 PPEM=72 BY=72 ASPECT=0.9362 python3 build.py wc3-icons-mac.ttf 1222 4 94
```

Geometry (kitty on macOS, Retina = 2 pixels per point, so font_px = 2 x font_size): the strike is scaled by
font_px/PPEM, the glyph origin is the pen position on the baseline, the bitmap bottom-left sits at
(BX, BY - S) strike pixels from it (`originOffsetY = BY - S`, S = bitmap height, the 4th argument). Wanted: a bitmap
of exactly 2 cells x 1 line. With Paper Mono 18 + `modify_font cell_height 110%` the cell is 22 x 47 px, baseline
36 px below the cell top (descent 11 px). So: S = 47 x PPEM/36 = 94, W = 2 x 22 x PPEM/36 = 88 (ASPECT = 88/94 = 0.9362),
advance = 2 x 22/36 x 1000 = 1222 units, BY = S - descent px x PPEM/36 = 94 - 22 = 72. kitty clips glyphs to the
cell: a BY too large clips the top of the icon, too small leaves a gap at the top (BY=188 at PPEM 144 clipped 11 px,
BY=144 was exact). Measured on the screenshot of an isolated kitty: icon 44 x 47 px per cell pair, stacked rows
seamless, whole faces, tinted backgrounds visible only left and right of the 2-cell icon. Other fonts/sizes:
measure the cell (`TIOCGWINSZ` pixel size / rows) and the baseline offset, re-derive with the formulas, pass the
numbers to `build.py` and use a new family name AND file name (CoreText caches by family; a changed font under an old
name showed the fallback "?" boxes until the name changed).

### ghostty on macOS

ghostty (1.3.1, read in `src/font/face/coretext.zig` and `SharedGrid.zig`) renders every colour glyph (any font with an
`sbix` table is "emoji") with the constraint size=cover, centred, 2.5 % side padding (undone by a GPOS shift, item 4): the glyph's bounding rect (from
the font OUTLINE, `CTFontGetBoundingRectsForGlyphs`; a glyph without outline has a 0-size rect and draws NOTHING) is
scaled uniformly into 2 cells x the primary font's natural line height (`face_height`, NOT the cell height:
`adjust-cell-height` only adds space around it), centred, then CoreText draws the bitmap into that canvas and
anything outside the canvas is cropped. So:

1. every glyph needs an outline = a box the size of the bitmap (`SBIX_BOX=1`; ONE glyph object per glyph name: a shared
   object is compiled once and the others come out empty). kitty shrinks icons that have the box, so the kitty font has
   none.
2. CoreText does NOT scale `originOffsetY` with the font size (measured with `CTFontDrawGlyphs` at 36 px, strike
   72: offset -22 strike px moved the bitmap 22 px, the outline rect only 11), so any offset puts bitmap and outline
   rect out of register and the canvas crops the bottom (first try: 32 of 43 px, face cut off at the mouth).
   Use offset 0: `BY=S`, the bitmap then sits on the baseline exactly where its outline is.
3. Bitmap natural size = W x font_px/PPEM, outline = same in units (`W*1000/PPEM`), so they agree at every size.
4. Emoji padding and the shift. ghostty constrains every colour glyph with size=cover, centred,
   `pad_left/right = 0.025` (`src/font/SharedGrid.zig` `renderGlyph`, 1.3.1), then rounds an sbix canvas to whole
   px (`src/font/face/coretext.zig` `renderGlyph`). The renderer adds the SHAPER's x offset after that
   (`src/renderer/generic.zig` `addGlyph`, `bearings = offset_x + x_offset`; `src/font/shaper/coretext.zig`
   `x_offset = round(position.x - cell_offset.x)`), so a GPOS `XPlacement` on every icon (`SBIX_DX`, feature
   `kern`, on by default) moves the finished canvas without clipping it. Glyphs are not clipped to their cell: a
   negative result draws over the previous cell. Left gap in px, with font px = pt x 2 on Retina:
   `left = round(x) + round(SBIX_DX x px / 1000)`, `x = max(0.025 fw, (fw + cw - icon_w)/2) + (cw - fw)/2`
   (only the fractional part when `cw < fw`; `fw` = primary advance in px, Paper Mono 0.606 em; `cw = round(fw)`).
   For Paper Mono `round(x)` is 0 or 1 depending on the size. Vertically the icon fills the font's natural line
   height (ASPECT 0.99 = 93 x 94 strike) and `adjust-cell-height = N` gives N/2 px above and below.
   A font cannot read `adjust-cell-height` or the size, so `SBIX_DX` is tuned per setting. Shipped: 48 = left gap
   2 px at font-size 21 + `adjust-cell-height = 4` (cells 25 x 54 px, icon 50 x 50, 2 px on every side; the right
   side overlaps the plugin's padding space by 2 px). Measured in an isolated ghostty 1.3.1, Paper Mono, Retina:

   | font-size / adjust-cell-height | SBIX_DX | left | top | bottom | icon |
   |---|---|---|---|---|---|
   | 21 / 4 | -22 | -1 | 2 | 2 | 50 x 50 |
   | 21 / 4 | 48 (shipped) | 2 | 2 | 2 | 50 x 50 |
   | 18 / 0 | -22 | 0 | 0 | 0 | 42 x 43 |
   | 18 / 0 | 0 (ASPECT 1.0) | 1 | ~0.5 | ~0.5 | 42 x 42 |

   For another setting pick `SBIX_DX` so `round(SBIX_DX x px/1000)` = wanted gap - `round(x)`, mid-interval
   (e.g. 2.0 px at 42 px = 48), and check with a screenshot.
5. The family is named exactly like the Linux font ("WC3 Icons Wide"), because `font-codepoint-map` with a family
   that does not exist is silently ignored (system emoji show) but the LAST matching map line wins even if its family
   is missing, so two lines (one per OS) cannot share a config; one name can.

```sh
FAMILY="WC3 Icons Wide" SBIX=1 SBIX_BOX=1 SBIX_DX=48 PPEM=72 BY=94 ASPECT=0.99 python3 build.py wc3-icons-mac-ghostty.ttf 1222 4 94
```

Verified by screenshot of an isolated ghostty (Paper Mono 18, `adjust-cell-height = 0`): no background row between
stacked icons, whole faces. Map the race range U+1F7E0-U+1F7E6 too, or the colour circle emoji show for characters
without an icon.

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
included in this repo deliberately; no other rights are claimed. `build.py` is part
of this package.
