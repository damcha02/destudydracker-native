#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  printf 'usage: %s <pid>\n' "$0" >&2
  exit 2
fi

PID="$1"

if [[ ! "${PID}" =~ ^[0-9]+$ ]] || [[ ! -d "/proc/${PID}" ]]; then
  printf 'error: PID %s does not exist under /proc\n' "${PID}" >&2
  exit 1
fi

SMAPS="/proc/${PID}/smaps_rollup"
STATUS="/proc/${PID}/status"
EXE="/proc/${PID}/exe"

printf 'Study Tracker native prototype Linux benchmark\n'
printf 'PID: %s\n\n' "${PID}"

printf 'Memory from /proc/%s/smaps_rollup\n' "${PID}"
if [[ -r "${SMAPS}" ]]; then
  awk '/^(Rss|Pss|Private_Clean|Private_Dirty|Shared_Clean|Shared_Dirty):/ { printf "%-16s %12s %s\n", $1, $2, $3 }' "${SMAPS}"
else
  printf 'smaps_rollup is not readable for this process.\n'
fi

printf '\nMemory labels\n'
printf 'RSS includes resident private and shared pages.\n'
printf 'PSS divides shared pages across processes and is usually better for app comparisons.\n'
printf 'Private_* pages are attributed only to this process. Shared_* pages may also be used by other processes.\n'

printf '\nProcess tree\n'
if command -v pstree >/dev/null 2>&1; then
  pstree -aps "${PID}"
else
  ps -o pid,ppid,pgid,sid,stat,comm -p "${PID}"
  printf 'pstree not found; showing only the target process.\n'
fi

printf '\nInstantaneous CPU information\n'
ps -p "${PID}" -o pid,ppid,stat,pcpu,pmem,etime,time,comm
if [[ -r "${STATUS}" ]]; then
  awk '/^(voluntary_ctxt_switches|nonvoluntary_ctxt_switches|Threads):/ { print }' "${STATUS}"
fi

printf '\nShort sampled CPU average\n'
if command -v pidstat >/dev/null 2>&1; then
  pidstat -p "${PID}" 1 3
else
  BEFORE_CPU=$(awk '{ print $14 + $15 }' "/proc/${PID}/stat")
  BEFORE_TOTAL=$(awk '/^cpu / { total = 0; for (i = 2; i <= NF; i++) total += $i; print total; exit }' /proc/stat)
  sleep 3
  AFTER_CPU=$(awk '{ print $14 + $15 }' "/proc/${PID}/stat")
  AFTER_TOTAL=$(awk '/^cpu / { total = 0; for (i = 2; i <= NF; i++) total += $i; print total; exit }' /proc/stat)
  CPU_DELTA=$((AFTER_CPU - BEFORE_CPU))
  TOTAL_DELTA=$((AFTER_TOTAL - BEFORE_TOTAL))
  CPU_COUNT=$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '1')
  awk -v c="${CPU_DELTA}" -v t="${TOTAL_DELTA}" -v n="${CPU_COUNT}" 'BEGIN { if (t > 0) printf "Approx average CPU over 3s: %.2f%% of one CPU\n", (c / t) * n * 100; else print "Approx average CPU over 3s: unavailable" }'
  printf 'pidstat not found; sampled /proc CPU counters over 3 seconds.\n'
fi

printf '\nExecutable size\n'
if [[ -e "${EXE}" ]]; then
  EXE_PATH=$(readlink -f "${EXE}")
  if [[ -n "${EXE_PATH}" && -r "${EXE_PATH}" ]]; then
    stat -c 'Executable path: %n
Executable file size: %s bytes' "${EXE_PATH}"
    printf 'Executable file size is disk size, not runtime memory.\n'
  else
    printf 'Executable path is not readable.\n'
  fi
else
  printf 'Executable link is unavailable.\n'
fi
