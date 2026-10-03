#!/usr/bin/env bash
# Stage 20 Linux baselines (the Windows acceptance numbers are taken by stage19-perf.ps1-style
# scripts on Windows; these are for catching implementation problems and for comparison).
#
#   stage20-perf-linux.sh <exe> <fixture.json> <name> [settle_s] [sample_s] [KEY=VALUE ...]
#
# Runs the app on its own X display (DISPLAY, default :99), isolated profile, frozen date, with
# STUDY_NATIVE_FRAME_STATS; after `settle` seconds samples `sample` seconds of:
#   cpu%  (utime+stime ticks over the window, % of one core)
#   rss / anon (VmRSS / RssAnon, MiB, at the end)  threads (at the end)
#   frames (rendered frames reported by the app's STATS lines inside the window)
# and prints one tab-separated row. Extra KEY=VALUE pairs are passed as environment.
set -euo pipefail
exe=$1; fixture=$2; name=$3; settle=${4:-12}; sample=${5:-20}; shift 5 || shift $#
data=$(mktemp -d /tmp/st-perf-XXXXXX); log=$(mktemp /tmp/st-perf-log-XXXXXX)
trap 'rm -rf "$data" "$log"' EXIT
env -u WAYLAND_DISPLAY DISPLAY=${DISPLAY_OVERRIDE:-:99} SLINT_SCALE_FACTOR=1 WINIT_UNIX_BACKEND=x11 \
  STUDY_NATIVE_DATA_DIR="$data" STUDY_NATIVE_IMPORT_BACKUP="$fixture" \
  STUDY_NATIVE_NOW=${STUDY_NATIVE_NOW:-2026-09-30T12:00:00+02:00} TZ=Europe/Zurich \
  STUDY_NATIVE_SIZE=${SIZE:-1520x980} STUDY_NATIVE_BREAK_PICK=0.25 STUDY_NATIVE_FRAME_STATS=1 \
  "$@" "$exe" >"$log" 2>&1 &
pid=$!
sleep "$settle"
ticks() { awk '{print $14+$15}' /proc/$pid/stat; }
t0=$(ticks); s0=$(date +%s.%N)
lines0=$(grep -c '^STATS' "$log" || true)
sleep "$sample"
t1=$(ticks); s1=$(date +%s.%N)
hz=$(getconf CLK_TCK)
cpu=$(awk -v a=$t0 -v b=$t1 -v s=$s0 -v e=$s1 -v hz=$hz 'BEGIN{printf "%.2f", (b-a)/hz/(e-s)*100}')
rss=$(awk '/VmRSS/{printf "%.1f", $2/1024}' /proc/$pid/status)
anon=$(awk '/RssAnon/{printf "%.1f", $2/1024}' /proc/$pid/status)
threads=$(awk '/Threads/{print $2}' /proc/$pid/status)
# (Stage 21: summed with awk; the earlier `paste | bc` printed 0 whenever bc was not installed)
frames=$(tail -n +$((lines0 + 1)) <(grep '^STATS' "$log") | sed -n 's/.* frames=\([0-9]*\).*/\1/p' | awk '{s+=$1} END{print s+0}')
extra=$(grep -E 'done|SNAPSHOT' "$log" | tr '\n' ' ' || true)
kill $pid 2>/dev/null || true; wait $pid 2>/dev/null || true
printf "%s\tcpu=%s%%\trss=%sMiB\tanon=%sMiB\tthreads=%s\tframes=%s\t%s\n" "$name" "$cpu" "$rss" "$anon" "$threads" "${frames:-0}" "$extra"
