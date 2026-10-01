#!/usr/bin/env bash
# Stage 20 memory stability (Linux baseline): runs the app with a stress hook and samples VmRSS /
# RssAnon every `every` seconds until the hook prints "done", then 10 s more.
#   stage20-memory-linux.sh <exe> <fixture.json> <every_s> KEY=VALUE...
set -euo pipefail
exe=$1; fixture=$2; every=$3; shift 3
data=$(mktemp -d /tmp/st-mem-XXXXXX); log=$(mktemp /tmp/st-mem-log-XXXXXX)
trap 'rm -rf "$data" "$log"' EXIT
env -u WAYLAND_DISPLAY DISPLAY=${DISPLAY_OVERRIDE:-:99} SLINT_SCALE_FACTOR=1 WINIT_UNIX_BACKEND=x11 \
  STUDY_NATIVE_DATA_DIR="$data" STUDY_NATIVE_IMPORT_BACKUP="$fixture" \
  STUDY_NATIVE_NOW=2026-09-30T12:00:00+02:00 TZ=Europe/Zurich STUDY_NATIVE_SIZE=1520x980 STUDY_NATIVE_BREAK_PICK=0.25 \
  STUDY_NATIVE_FRAME_STATS=1 "$@" "$exe" >"$log" 2>&1 &
pid=$!
t=0; done_at=""
while kill -0 $pid 2>/dev/null; do
  sleep "$every"; t=$((t + every))
  printf "t=%ss rss=%sMiB anon=%sMiB threads=%s\n" $t "$(awk '/VmRSS/{printf "%.1f",$2/1024}' /proc/$pid/status)" "$(awk '/RssAnon/{printf "%.1f",$2/1024}' /proc/$pid/status)" "$(awk '/Threads/{print $2}' /proc/$pid/status)"
  if [ -z "$done_at" ] && grep -q 'done' "$log"; then done_at=$t; grep 'done' "$log"; fi
  if [ -n "$done_at" ] && [ $t -ge $((done_at + 10)) ]; then break; fi
done
grep '^STATS' "$log" | tail -1 | sed 's/.*sakura_ticks=[0-9]* //' | cut -c1-200
kill $pid 2>/dev/null || true; wait $pid 2>/dev/null || true
