#!/usr/bin/env bash
# Stage 20: Linux counterpart of capture-native.ps1. Renders the NATIVE app in an isolated profile
# against a synthetic backup fixture and saves the window as a PNG via the in-process
# STUDY_NATIVE_SNAPSHOT hook (femtovg pixels, no screen grabbing).
#
#   capture-native-linux.sh <exe> <fixture.json> <out.png> [KEY=VALUE ...]
#
# Runs on an X display of its own (default :99, e.g. a rootful `Xwayland :99 -geometry 1700x1200`)
# so the window gets exactly the requested logical size at scale 1 - the pairing for headless
# Chrome at DPR 1. Extra KEY=VALUE pairs are passed as environment (STUDY_NATIVE_VIEW=break,
# STUDY_NATIVE_STYLE=wabi-sabi, STUDY_NATIVE_THEME=light, ...).
set -euo pipefail
exe=$1; fixture=$2; out=$3; shift 3
data=$(mktemp -d /tmp/st-native-XXXXXX)
trap 'rm -rf "$data"' EXIT
env -u WAYLAND_DISPLAY DISPLAY=${CAPTURE_DISPLAY:-:99} WINIT_UNIX_BACKEND=x11 SLINT_SCALE_FACTOR=1 \
  STUDY_NATIVE_DATA_DIR="$data" STUDY_NATIVE_IMPORT_BACKUP="$fixture" \
  STUDY_NATIVE_NOW=${STUDY_NATIVE_NOW:-2026-09-30T12:00:00+02:00} TZ=${TZ:-Europe/Zurich} \
  STUDY_NATIVE_SIZE=${SIZE:-1520x980} STUDY_NATIVE_BREAK_PICK=${PICK:-0.25} \
  STUDY_NATIVE_SNAPSHOT="$out" "$@" timeout 60 "$exe" | grep -E '^SNAPSHOT' || { echo "no snapshot"; exit 1; }
