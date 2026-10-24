#!/bin/sh
# macOS counterpart of install.sh (which hands over to this script on Darwin): installs the WC3 icon font as an
# Apple sbix colour bitmap font ("WC3 Icons Mac"; CoreText cannot draw the CBDT build used on Linux).
#   fontconfig/install-mac.sh               install / update (idempotent)
#   fontconfig/install-mac.sh --uninstall   remove it again
#   fontconfig/install-mac.sh --check       show whether the font is installed and registered, change nothing
# Writes exactly one file, owned by this package: ~/Library/Fonts/wc3-icons-mac.ttf (copy of
# assets/wc3-icons/wc3-icons-mac.ttf; a foreign file of that name is never overwritten), and removes the plugin's
# cached font flag (<state root>/font) so the bar re-checks at the next hook event. It edits no terminal config:
# the terminal needs one line (printed below), then restart it completely.
# Effect on other apps: none unless they ask for this family; the font only has glyphs for U+1FA70..U+1FAD6 and
# U+1F7E0..U+1F7E6, so macOS may also pick it for those emoji codepoints in apps that fall back by coverage.
set -u
here=$(cd "$(dirname "$0")" && pwd -P)
src="$here/../assets/wc3-icons/wc3-icons-mac.ttf"
family="WC3 Icons Mac"
font_dir="$HOME/Library/Fonts"
dst="$font_dir/wc3-icons-mac.ttf"
say() { printf '  wc3-icons: %s\n' "$*"; }
mode=install
case "${1:-}" in
  "") ;;
  --uninstall) mode=uninstall ;;
  --check) mode=check ;;
  --print-conf) say "no fontconfig rules on macOS; the terminal line is printed by the install and --check"; exit 0 ;;
  -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
  *) echo "unknown option $1 (see --help)" >&2; exit 2 ;;
esac
[ "$(uname -s)" = Darwin ] || { say "install-mac.sh is for macOS (use install.sh)"; exit 1; }
ours() { [ -f "$1" ] && grep -aqF "$family" "$1"; }   # the file carries our family name
. "$here/../claude-lib.sh" 2>/dev/null
drop_flag() { rm -f "$(claude_state_root)/font" 2>/dev/null; }

terminal_notes() {
  cat <<EOF2
  Terminals (nothing below is changed by this script; add the line yourself, then quit and reopen the terminal):
  - kitty (~/.config/kitty/kitty.conf):
      symbol_map U+1FA70-U+1FAD6,U+1F7E0-U+1F7E6 $family
  - ghostty (config):
      font-codepoint-map = U+1FA70-U+1FAD6=$family
      font-codepoint-map = U+1F7E0-U+1F7E6=$family
  The bitmap is 2 cells wide by 1 line high for kitty with Paper Mono 18 + cell_height 110% on a Retina display
  (cells 22x47 px); other fonts/sizes scale it, undistorted, to fit the width (assets/wc3-icons/README.md, macOS).
  Without the line the terminal draws other emoji for these codepoints. A terminal that cannot draw it: set
  race_font "false" in the plugin options (initials badges) or run --uninstall.
EOF2
}

check() {
  if [ -f "$dst" ]; then
    if ours "$dst"; then say "installed: $dst"; else say "$dst exists but is not the $family font"; fi
  else say "not installed ($dst missing)"; fi
  for d in "$font_dir" /Library/Fonts; do
    [ -f "$d/wc3-icons-mac.ttf" ] && ours "$d/wc3-icons-mac.ttf" && say "flag=1 source: $d/wc3-icons-mac.ttf"
  done
}

case "$mode" in
  check) check; exit 0 ;;
  uninstall)
    if [ -f "$dst" ]; then
      if ours "$dst"; then rm -f "$dst" && say "removed $dst" && drop_flag; say "done; restart terminals to drop the icons"
      else say "$dst is not the $family font, left alone"; fi
    else say "nothing to remove"; fi
    exit 0 ;;
esac

[ -f "$src" ] || { say "$src not found"; exit 1; }
mkdir -p "$font_dir" || { say "could not create $font_dir"; exit 1; }
if cmp -s "$src" "$dst"; then say "already installed, nothing changed"
else
  if [ -e "$dst" ] && ! ours "$dst"; then say "$dst exists and is not the $family font; not overwriting it. Stopped."; exit 1; fi
  tmp=$(mktemp "$font_dir/.wc3-icons-mac.XXXXXX") || { say "could not create a temp file in $font_dir"; exit 1; }
  trap 'rm -f "$tmp"' EXIT
  cp "$src" "$tmp" && chmod 644 "$tmp" && mv -f "$tmp" "$dst" || { say "could not write $dst"; exit 1; }
  say "installed $dst"
  drop_flag
fi
check
terminal_notes
