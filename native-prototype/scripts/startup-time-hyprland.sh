#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  printf 'usage: %s <window-title-substring> <command> [args...]\n' "$0" >&2
  exit 2
fi

TITLE_SUBSTRING="$1"
shift

if ! command -v hyprctl >/dev/null 2>&1 || ! command -v jq >/dev/null 2>&1; then
  printf 'error: hyprctl and jq are required for Hyprland startup timing\n' >&2
  exit 1
fi

START_NS="$(date +%s%N)"
"$@" >/tmp/study-tracker-startup-time.log 2>&1 &
PID="$!"

for _ in $(seq 1 500); do
  if hyprctl clients -j | jq -e --arg title "${TITLE_SUBSTRING}" --argjson pid "${PID}" \
    '.[] | select(.pid == $pid and (.title | contains($title)) and .mapped == true)' >/dev/null; then
    END_NS="$(date +%s%N)"
    awk -v start="${START_NS}" -v end="${END_NS}" -v pid="${PID}" 'BEGIN { printf "PID %s window visible in %.3f s\n", pid, (end - start) / 1000000000 }'
    kill -TERM "${PID}" >/dev/null 2>&1 || true
    exit 0
  fi

  if ! kill -0 "${PID}" >/dev/null 2>&1; then
    printf 'error: process %s exited before matching window appeared\n' "${PID}" >&2
    exit 1
  fi

  sleep 0.01
done

kill -TERM "${PID}" >/dev/null 2>&1 || true
printf 'error: timed out waiting for window title containing %s from PID %s\n' "${TITLE_SUBSTRING}" "${PID}" >&2
exit 1
