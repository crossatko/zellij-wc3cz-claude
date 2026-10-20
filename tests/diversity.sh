#!/bin/sh
# Character-assignment diversity: runs the simulation tests (1000 simulated sessions, fixed seed) of the plugin.
# Usage: sh tests/diversity.sh      (exit 0 = all passed; needs cargo)
cd "$(dirname "$0")/.." || exit 1
cargo test diversity -- --nocapture 2>&1 | grep -E '^(diverse|random)|^test |test result'
exit "$(cargo test diversity >/dev/null 2>&1; echo $?)"
