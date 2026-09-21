#!/usr/bin/env bash
# Prints one memory + sampled-CPU line for a running process (Stage 10 benchmark helper).
# usage: sample-linux.sh <pid> [cpu-sample-seconds=10] [label]
set -euo pipefail

PID="${1:?usage: $0 <pid> [seconds] [label]}"
SECONDS_TO_SAMPLE="${2:-10}"
LABEL="${3:-sample}"
[[ -d "/proc/${PID}" ]] || { echo "pid ${PID} not running" >&2; exit 1; }

ticks() { awk '{ print $14 + $15 }' "/proc/${PID}/stat"; }
BEFORE=$(ticks)
sleep "${SECONDS_TO_SAMPLE}"
AFTER=$(ticks)
HZ=$(getconf CLK_TCK)
CPU=$(awk -v d=$((AFTER - BEFORE)) -v hz="${HZ}" -v s="${SECONDS_TO_SAMPLE}" 'BEGIN { printf "%.2f", d / hz / s * 100 }')

mem() { awk -v k="$1" '$1 == k":" { printf "%.1f", $2 / 1024 }' "/proc/${PID}/smaps_rollup"; }
printf '%-22s rss=%sMB pss=%sMB priv_clean=%sMB priv_dirty=%sMB shared_clean=%sMB shared_dirty=%sMB cpu=%s%% (%ss window) threads=%s\n' \
  "${LABEL}" "$(mem Rss)" "$(mem Pss)" "$(mem Private_Clean)" "$(mem Private_Dirty)" \
  "$(mem Shared_Clean)" "$(mem Shared_Dirty)" "${CPU}" "${SECONDS_TO_SAMPLE}" \
  "$(awk '/^Threads:/ { print $2 }' "/proc/${PID}/status")"
