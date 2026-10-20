#!/bin/sh
# Sound helper for the claude-tabs plugin. POSIX sh; works on Linux and macOS (BSD userland, bash 3.2).
#   play-phrase.sh SOUNDS_DIR CHARACTER [PLAYER]  play a random clip of that character: files named
#                                             "<Character> - <line>.<ext>" in any SOUNDS_DIR/<race>/ (never
#                                             the one played last for that character); a character without
#                                             clips plays any clip; exit 3 when there is nothing to play
#                                             (caller falls back)
#   play-phrase.sh --list SOUNDS_DIR [PLAYER] print "race<TAB>character" for every character with clips
#   play-phrase.sh --ding [PLAYER]            play sounds/ding.wav next to this script
# Playback is serialised across all callers (state root): one voice line at a time, see "Overlap policy" below.
# Env (set by the plugin, all optional): CLAUDE_TABS_DUP_MS (2000), CLAUDE_TABS_LEAD_IN_MS (0),
# CLAUDE_TABS_QUEUE_MAX_S (15), CLAUDE_TABS_TRIGGER / CLAUDE_TABS_WHO (only for ding.log).
# Exit: 0 played or dropped as an overlap, 3 nothing to play / no player (caller falls back), else the player's status.
# SOUNDS_DIR empty = the packaged library: sounds/wc3 next to this script.
# Folders whose name starts with `_` or `.` (e.g. `_excluded/`) are never races; only files directly inside
# SOUNDS_DIR/RACE/ count (no recursion).
# PLAYER empty = autodetect (afplay on macOS, pw-play / paplay / ffplay / aplay on Linux); the file types
# considered depend on the player (afplay: no Ogg; pw-play/paplay: no m4a; aplay: wav only).
# this script's own directory, symlinks resolved (siblings and sounds/ live there)
self=$0
case $self in */*) ;; *) if [ -f "$self" ]; then self=./$self; else self=$(command -v "$self") || self=./$0; fi ;; esac
while [ -h "$self" ]; do
  _l=$(readlink "$self") || break
  case $_l in /*) self=$_l ;; *) self=${self%/*}/$_l ;; esac
done
here=$(cd -P "${self%/*}/" 2>/dev/null && pwd -P) || here=$(dirname "$0")
. "$here/claude-lib.sh"

# expands a leading ~ / $HOME (no eval: the value is data and may contain quotes, $ or backticks)
expand() {
  case $1 in
    '~') printf '%s' "$HOME" ;;
    '~/'*) printf '%s' "$HOME/${1#'~/'}" ;;
    '$HOME') printf '%s' "$HOME" ;;
    '$HOME/'*) printf '%s' "$HOME/${1#'$HOME/'}" ;;
    '${HOME}') printf '%s' "$HOME" ;;
    '${HOME}/'*) printf '%s' "$HOME/${1#'${HOME}/'}" ;;
    *) printf '%s' "$1" ;;
  esac
}

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


# every playable file of every race folder (one path per line)
all_files() {   # $1 dir, $2 extensions
  for _x in "$1"/*/; do
    _x=${_x%/}
    is_race_dir "$_x" || continue
    audio_files "$_x" "$2"
  done
}

if [ "$1" = "--list" ]; then
  d=$(sounds_dir "$2") || exit 0
  player=$(claude_player "$3") || exit 0
  exts=$(claude_exts "$player")
  # one line per character: race<TAB>character, where the character is the file name part before the
  # first " - " (a few names contain " - " themselves; the plugin resolves those against characters.tsv)
  for x in "$d"/*/; do
    x=${x%/}
    is_race_dir "$x" || continue
    audio_files "$x" "$exts" | awk -F/ -v race="$(basename "$x")" '
      { f = $NF; sub(/\.[^.]*$/, "", f); i = index(f, " - "); if (i > 0) f = substr(f, 1, i - 1); print race "\t" f }' | sort -u
  done
  exit 0
fi

# ---- serialised playback ------------------------------------------------------------------------------
# Overlap policy (callers: finish, attention, claude-ctl, several panes/tabs; each starts its own script):
#  - a ding for the SAME character as the previous one that started less than CLAUDE_TABS_DUP_MS ago is dropped
#    (Stop + attention of one tab, two panes of one tab finishing together);
#  - a ding while a clip is still playing waits for it to end (queue of ONE, at most CLAUDE_TABS_QUEUE_MAX_S),
#    so each tab's finish is heard but never two clips at once; a further ding while one waits is dropped;
#  - nothing is ever killed; the "playing" record holds this script's PID, trusted only while that PID is
#    alive, has the recorded command name and the record is < 2 min old.
# Everything is under a mkdir mutex ($root/play.lock, stale after a dead owner or 5 s), which also makes the
# read-pick-write of last-<character> atomic.
root=$(claude_state_root)
mkdir -p "$root" 2>/dev/null
num() { case "$1" in ''|*[!0-9]*) printf '%s' "$2" ;; *) printf '%s' "$1" ;; esac; }
dup_ms=$(num "${CLAUDE_TABS_DUP_MS:-}" 2000)
lead_ms=$(num "${CLAUDE_TABS_LEAD_IN_MS:-}" 0)
qmax_s=$(num "${CLAUDE_TABS_QUEUE_MAX_S:-}" 15)
[ "$lead_ms" -le 3000 ] || lead_ms=3000
trigger=${CLAUDE_TABS_TRIGGER:-manual}
who=${CLAUDE_TABS_WHO:--}
lock_dir=$root/play.lock
held=0
tick=0.1; sleep 0.1 2>/dev/null || tick=1   # BSD/GNU sleep take fractions; fall back to whole seconds

ms_now() { echo $(( $(now_ns) / 1000000 )); }
lock_release() { if [ "$held" = 1 ]; then rm -rf "$lock_dir"; held=0; fi; }
trap 'lock_release' EXIT
trap 'exit 143' TERM INT HUP
# lock_acquire: 0 = held, 1 = impossible (root not writable) or not obtained within ~12 s. A dead owner is
# replaced at once; a lock held (or half-created, no pid yet) for 5 s is replaced as stale. Stealing renames
# the lock dir to a unique name first (rename is atomic: of several waiters only one wins), then deletes it.
lock_acquire() {
  _t0=$(ms_now)
  while ! mkdir "$lock_dir" 2>/dev/null; do
    [ -d "$root" ] && [ -w "$root" ] || return 1
    _o=$(cat "$lock_dir/pid" 2>/dev/null)
    _age=$(( $(ms_now) - _t0 ))
    [ "$_age" -le 12000 ] || return 1
    if { [ -n "$_o" ] && ! kill -0 "$_o" 2>/dev/null; } || [ "$_age" -gt 5000 ]; then
      _mv=$lock_dir.stale.$$
      if mv "$lock_dir" "$_mv" 2>/dev/null; then
        _n=$(cat "$_mv/pid" 2>/dev/null)
        if [ "$_n" != "$_o" ] && [ -n "$_n" ] && kill -0 "$_n" 2>/dev/null; then
          # lost a race: that was a fresh live lock, give it back if the name is still free
          [ -e "$lock_dir" ] || mv "$_mv" "$lock_dir" 2>/dev/null || :
        fi
        rm -rf "$_mv"
      fi
      continue
    fi
    sleep "$tick"
  done
  held=1
  printf '%s' "$$" >"$lock_dir/pid" 2>/dev/null
  return 0
}

# ding.log: one line per attempt, rotated at ~100 KB to ding.log.1
ding_log() {   # $1 outcome, $2 clip path (may be empty), $3 extra
  _f=$root/ding.log
  _sz=0; [ -f "$_f" ] && _sz=$(wc -c <"$_f" | tr -d ' ')
  if [ "${_sz:-0}" -gt 100000 ]; then mv -f "$_f" "$_f.1" 2>/dev/null; fi
  printf '%s trigger=%s who=%s char="%s" clip="%s" outcome=%s%s\n' "$(date '+%Y-%m-%d %H:%M:%S')" \
    "$trigger" "$who" "$dkey" "${2##*/}" "$1" "${3:+ $3}" >>"$_f" 2>/dev/null
}

# the "playing" record: "<script pid> <command name> <start ms> <character>"
read_rec() {
  p_pid=; p_comm=; p_ms=; p_char=
  [ -f "$root/playing" ] && read -r p_pid p_comm p_ms p_char <"$root/playing"
  return 0
}
pid_is() {   # $1 pid, $2 recorded command name: alive and not a recycled PID
  [ -n "$1" ] && kill -0 "$1" 2>/dev/null || return 1
  _c=$(ps -p "$1" -o comm= 2>/dev/null)
  [ -z "$_c" ] || [ -z "$2" ] || [ "$_c" = "$2" ]
}
rec_alive() {   # the recorded script is still running (and is not a recycled PID)
  [ "$p_pid" != "$$" ] && pid_is "$p_pid" "$p_comm" || return 1
  [ $(( $(ms_now) - ${p_ms:-0} )) -lt 120000 ]
}

# silent mono 48 kHz s16 WAV of $1 ms (printf headers, no dependencies)
le() {   # $1 value, $2 byte count -> little-endian bytes as printf escapes
  _v=$1; _n=$2; _o=
  while [ "$_n" -gt 0 ]; do _o="$_o$(printf '\\%03o' $(( _v % 256 )))"; _v=$(( _v / 256 )); _n=$(( _n - 1 )); done
  printf '%s' "$_o"
}
silence_wav() {
  _w=$root/silence-$1.wav
  if [ ! -s "$_w" ]; then
    _d=$(( $1 * 96 ))
    { printf "RIFF$(le $((36 + _d)) 4)WAVEfmt $(le 16 4)$(le 1 2)$(le 1 2)$(le 48000 4)$(le 96000 4)$(le 2 2)$(le 16 2)data$(le $_d 4)"
      head -c "$_d" /dev/zero; } >"$_w.$$" 2>/dev/null && mv -f "$_w.$$" "$_w" || { rm -f "$_w.$$"; return 1; }
  fi
  printf '%s' "$_w"
}

# pick the clip (called under the mutex); sets $pick
pick_clip() {
  pick=
  if [ -z "$files" ]; then pick=$ding_wav; return; fi
  # file name = readable ASCII part + hex of the name's bytes (distinct names never share a file)
  _h=$(printf '%s' "${char:-any}" | head -c 100 | od -An -tx1 | tr -d ' \n')
  state="$root/last-$(printf '%s' "${char:-any}" | tr -c 'A-Za-z0-9' '_' | cut -c1-24)-$_h"
  _f=$files
  n=$(printf '%s\n' "$_f" | wc -l | tr -d ' ')
  last=$(cat "$state" 2>/dev/null)
  if [ "$n" -gt 1 ]; then
    _f=$(printf '%s\n' "$_f" | grep -vxF "$last")
    n=$(printf '%s\n' "$_f" | wc -l | tr -d ' ')
  fi
  pick=$(printf '%s\n' "$_f" | sed -n "$(( $(rand) % n + 1 ))p")
  [ -n "$pick" ] && printf '%s' "$pick" >"$state" 2>/dev/null
}

# play_serial: runs with $player, $files (empty = the packaged ding), $dkey (character for the dup check)
play_serial() {
  ding_wav="$here/sounds/ding.wav"
  queued=; waited=0
  lock_acquire || { ding_log failed "" "no-lock"; exit 3; }
  read_rec
  now=$(ms_now)
  if [ -n "$p_ms" ] && [ "$p_char" = "$dkey" ] && [ $(( now - p_ms )) -lt "$dup_ms" ]; then
    lock_release; ding_log dropped-duplicate "" "age_ms=$(( now - p_ms ))"; exit 0
  fi
  if [ -n "$p_ms" ] && rec_alive; then
    _q=; _qc=; { read -r _q _qc <"$root/queued"; } 2>/dev/null
    if pid_is "$_q" "$_qc"; then
      lock_release; ding_log dropped-queue-full "" ""; exit 0
    fi
    _comm=$(ps -p "$$" -o comm= 2>/dev/null)
    printf '%s %s\n' "$$" "${_comm:-sh}" >"$root/queued"
    lock_release
    t0=$now
    while rec_alive_now; do
      if [ $(( $(ms_now) - t0 )) -gt $(( qmax_s * 1000 )) ]; then
        rm -f "$root/queued"; ding_log dropped-timeout "" "waited_ms=$(( $(ms_now) - t0 ))"; exit 0
      fi
      sleep "$tick"
    done
    waited=$(( $(ms_now) - t0 ))
    lock_acquire || { rm -f "$root/queued"; ding_log failed "" "no-lock"; exit 3; }
    rm -f "$root/queued"
    read_rec
    if [ -n "$p_ms" ] && rec_alive; then
      lock_release; ding_log dropped-busy "" "waited_ms=$waited"; exit 0
    fi
    queued=" queued_ms=$waited"
  fi
  pick_clip
  if [ -z "$pick" ]; then lock_release; ding_log failed "" "nothing-to-play"; exit 3; fi
  _comm=$(ps -p "$$" -o comm= 2>/dev/null)
  printf '%s %s %s %s\n' "$$" "${_comm:-sh}" "$(ms_now)" "$dkey" >"$root/playing.$$" && mv -f "$root/playing.$$" "$root/playing"
  lock_release
  lead=
  if [ "$lead_ms" -gt 0 ]; then
    # a silent clip on the same player first: a sleeping / Bluetooth sink wakes on it instead of eating the speech
    _s=$(silence_wav "$lead_ms") && { $player "$_s" >/dev/null 2>&1; lead=" lead_in_ms=$lead_ms"; }
  fi
  $player "$pick"
  rc=$?
  _x="$queued$lead"; _x=${_x# }
  if [ "$rc" -eq 0 ]; then ding_log played "$pick" "$_x"; else ding_log failed "$pick" "rc=$rc${_x:+ $_x}"; fi
  [ "$rc" -ne 127 ] || rc=3   # player command not found: the caller falls back
  exit "$rc"
}
rec_alive_now() { read_rec; rec_alive; }

if [ "$1" = "--ding" ]; then
  player=$(claude_player "${2:-}") || { dkey=ding; ding_log failed "" "no-player"; exit 3; }
  files=; dkey=ding; char=
  play_serial
fi

dir=$(sounds_dir "$1") || exit 3
char=$2
player=$(claude_player "$3") || { dkey=$char; ding_log failed "" "no-player"; exit 3; }
exts=$(claude_exts "$player")
files=""
if [ -n "$char" ]; then
  # clips of this character only: "<Character> - <line>.<ext>" in any race folder
  files=$(all_files "$dir" "$exts" | PP_CHAR="$char" awk -F/ 'BEGIN { p = ENVIRON["PP_CHAR"] " - " } index($NF, p) == 1')
fi
if [ -z "$files" ]; then
  # this character has no clips (yet): any clip of the library
  files=$(all_files "$dir" "$exts")
  char=""
fi
dkey=${2:-any}
[ -n "$files" ] || { ding_log failed "" "empty-library"; exit 3; }
play_serial
