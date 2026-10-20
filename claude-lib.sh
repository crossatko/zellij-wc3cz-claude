# Shared helpers for the claude-tabs scripts. POSIX sh, works on Linux and macOS (BSD userland, bash 3.2).
# Source it: . "$(dirname "$0")/claude-lib.sh"

# Hooks started by a GUI app on macOS get a minimal PATH without Homebrew.
case "$(uname -s 2>/dev/null)" in
  Darwin) PATH="/opt/homebrew/bin:/usr/local/bin:$PATH" ;;
esac
export PATH

# Directory the plugin sees as /tmp: zellij maps its ZELLIJ_TMP_DIR = <std::env::temp_dir()>/zellij-<uid>
# (temp_dir() = $TMPDIR or /tmp; on macOS $TMPDIR is /var/folders/.../T/). The server creates
# `zellij-log` inside it, which is used to pick the right candidate when TMPDIR differs between the
# server and this process.
claude_state_root() {
  _uid=$(id -u)
  for _b in "${TMPDIR:-}" /tmp "$(getconf DARWIN_USER_TEMP_DIR 2>/dev/null)"; do
    [ -n "$_b" ] || continue
    _b=${_b%/}
    if [ -d "$_b/zellij-$_uid/zellij-log" ]; then
      printf '%s\n' "$_b/zellij-$_uid/zellij-claude"
      return 0
    fi
  done
  _b=${TMPDIR:-/tmp}
  printf '%s\n' "${_b%/}/zellij-$_uid/zellij-claude"
}

# run_timeout SECONDS COMMAND...  (macOS has no `timeout`)
run_timeout() {
  _t=$1; shift
  if command -v timeout >/dev/null 2>&1; then
    timeout "$_t" "$@"
  elif command -v gtimeout >/dev/null 2>&1; then
    gtimeout "$_t" "$@"
  elif command -v perl >/dev/null 2>&1; then
    perl -e 'alarm shift; exec @ARGV' "$_t" "$@"
  else
    "$@"
  fi
}

# nanoseconds since the epoch; BSD `date` has no %N (prints a literal N)
now_ns() {
  _n=$(date +%s%N 2>/dev/null)
  case "$_n" in
    ''|*[!0-9]*) ;;
    *) printf '%s\n' "$_n"; return 0 ;;
  esac
  if command -v perl >/dev/null 2>&1; then
    perl -MTime::HiRes=time -e 'printf "%d\n", time()*1e9' && return 0
  fi
  printf '%s%06d000\n' "$(date +%s)" "$(( $$ % 1000000 ))"
}

# Audio player: $1 = explicit choice (may be empty). Prints the command, or nothing.
claude_player() {
  if [ -n "$1" ]; then printf '%s\n' "$1"; return 0; fi
  for _p in afplay pw-play paplay ffplay aplay; do
    if command -v "$_p" >/dev/null 2>&1; then
      case "$_p" in
        ffplay) printf '%s\n' "ffplay -nodisp -autoexit -loglevel quiet" ;;
        *) printf '%s\n' "$_p" ;;
      esac
      return 0
    fi
  done
  return 1
}

# Audio extensions the player can decode: afplay (CoreAudio, FLAC since macOS 10.13) cannot play Ogg;
# pw-play/paplay decode through libsndfile (no AAC/m4a; mp3 needs libsndfile >= 1.1); aplay only WAV;
# ffplay and unknown players get everything.
claude_exts() {
  case "$1" in
    afplay*)          printf '%s\n' "flac wav mp3 m4a aif aiff" ;;
    pw-play*|paplay*) printf '%s\n' "flac wav ogg oga mp3 aif aiff" ;;
    aplay*)           printf '%s\n' "wav" ;;
    *)                printf '%s\n' "flac wav mp3 m4a ogg oga aif aiff" ;;
  esac
}
