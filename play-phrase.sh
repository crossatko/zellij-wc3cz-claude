#!/bin/sh
# Sound helper for the claude-tabs plugin. POSIX sh; works on Linux and macOS (BSD userland, bash 3.2).
#   play-phrase.sh SOUNDS_DIR RACE [PLAYER]   play a random clip from SOUNDS_DIR/RACE/ (never the one played
#                                             last for that race); a race without clips uses any race that has
#                                             some; exit 3 when there is nothing to play (caller falls back)
#   play-phrase.sh --list SOUNDS_DIR [PLAYER] print the race folders that hold playable audio
#   play-phrase.sh --ding [PLAYER]            play sounds/ding.wav next to this script
# SOUNDS_DIR empty = the packaged library: sounds/wc3 next to this script.
# Folders whose name starts with `_` or `.` (e.g. `_excluded/`) are never races; only files directly inside
# SOUNDS_DIR/RACE/ count (no recursion).
# PLAYER empty = autodetect (afplay on macOS, pw-play / paplay / ffplay / aplay on Linux); the file types
# considered depend on the player (afplay: no Ogg; pw-play/paplay: no m4a; aplay: wav only).
here=$(dirname "$0")
. "$here/claude-lib.sh"

expand() { eval "printf '%s' \"$1\"" 2>/dev/null; }   # expands a leading ~ / $HOME

default_dir() { printf '%s' "$here/sounds/wc3"; }
sounds_dir() {   # $1 = option value (may be empty)
  if [ -n "$1" ]; then expand "$1"; else default_dir; fi
}

# the audio files directly inside a dir for the given extension list (one path per line)
audio_files() {   # $1 dir, $2 extensions
  for _e in $2; do
    find "$1" -maxdepth 1 -type f -iname "*.$_e" 2>/dev/null
  done
}
has_audio() { [ -d "$1" ] && [ -n "$(audio_files "$1" "$2" | head -n 1)" ]; }
is_race_dir() {   # race folders: not `_*`, not hidden
  case "$(basename "$1")" in _*|.*) return 1 ;; esac
  [ -d "$1" ]
}
rand() { od -An -N2 -tu2 /dev/urandom | tr -d ' '; }

if [ "$1" = "--ding" ]; then
  player=$(claude_player "${2:-}") || exit 3
  exec $player "$here/sounds/ding.wav"
fi

if [ "$1" = "--list" ]; then
  d=$(sounds_dir "$2") || exit 0
  player=$(claude_player "$3") || exit 0
  exts=$(claude_exts "$player")
  for x in "$d"/*/; do
    x=${x%/}
    is_race_dir "$x" || continue
    has_audio "$x" "$exts" && basename "$x"
  done
  exit 0
fi

dir=$(sounds_dir "$1") || exit 3
race=$2
player=$(claude_player "$3") || exit 3
exts=$(claude_exts "$player")
d="$dir/$race"
case "$race" in ''|_*|.*|*/*) d="" ;; esac
if [ -z "$d" ] || ! has_audio "$d" "$exts"; then
  # this race has no clips (yet): use a random race that has
  cands=$(for x in "$dir"/*/; do x=${x%/}; is_race_dir "$x" && has_audio "$x" "$exts" && printf '%s\n' "$x"; done)
  [ -n "$cands" ] || exit 3
  n=$(printf '%s\n' "$cands" | wc -l | tr -d ' ')
  d=$(printf '%s\n' "$cands" | sed -n "$(( $(rand) % n + 1 ))p")
  race=$(basename "$d")
fi
state="$(claude_state_root)/last-$race"
files=$(audio_files "$d" "$exts")
[ -n "$files" ] || exit 3
n=$(printf '%s\n' "$files" | wc -l | tr -d ' ')
last=$(cat "$state" 2>/dev/null)
if [ "$n" -gt 1 ]; then
  files=$(printf '%s\n' "$files" | grep -vxF "$last")
  n=$(printf '%s\n' "$files" | wc -l | tr -d ' ')
fi
pick=$(printf '%s\n' "$files" | sed -n "$(( $(rand) % n + 1 ))p")
[ -n "$pick" ] || exit 3
mkdir -p "$(dirname "$state")" 2>/dev/null
printf '%s' "$pick" >"$state" 2>/dev/null
exec $player "$pick"
