#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  printf 'usage: %s <root-pid> [cpu-sample-seconds]\n' "$0" >&2
  exit 2
fi

ROOT_PID="$1"
CPU_SECONDS="${2:-10}"

if [[ ! "${ROOT_PID}" =~ ^[0-9]+$ ]] || [[ ! -d "/proc/${ROOT_PID}" ]]; then
  printf 'error: PID %s does not exist under /proc\n' "${ROOT_PID}" >&2
  exit 1
fi

if [[ ! "${CPU_SECONDS}" =~ ^[0-9]+$ ]] || [[ "${CPU_SECONDS}" -lt 1 ]]; then
  printf 'error: cpu sample seconds must be a positive integer\n' >&2
  exit 2
fi

collect_tree() {
  local pending=("$1")
  local seen=" "
  local pid child process_table

  process_table="$(ps -e -o pid= -o ppid=)"

  while [[ ${#pending[@]} -gt 0 ]]; do
    pid="${pending[0]}"
    pending=("${pending[@]:1}")
    [[ -d "/proc/${pid}" ]] || continue
    [[ "${seen}" == *" ${pid} "* ]] && continue
    seen+="${pid} "
    printf '%s\n' "${pid}"

    while read -r child child_ppid; do
      [[ "${child_ppid}" == "${pid}" ]] && pending+=("${child}")
    done <<< "${process_table}"
  done
}

mapfile -t PIDS < <(collect_tree "${ROOT_PID}")

printf 'Study Tracker Linux process-tree benchmark\n'
printf 'Root PID: %s\n' "${ROOT_PID}"
printf 'Included PIDs: %s\n\n' "${PIDS[*]}"

printf 'Memory from /proc/<pid>/smaps_rollup, summed across included PIDs\n'
declare -A MEMORY_TOTALS=([Rss]=0 [Pss]=0 [Private_Clean]=0 [Private_Dirty]=0 [Shared_Clean]=0 [Shared_Dirty]=0)
READABLE_ROLLUPS=0
for pid in "${PIDS[@]}"; do
  rollup="/proc/${pid}/smaps_rollup"
  if [[ -r "${rollup}" ]] && exec {rollup_fd}<"${rollup}" 2>/dev/null; then
    while read -r key value _unit; do
      key="${key%:}"
      case "${key}" in
        Rss|Pss|Private_Clean|Private_Dirty|Shared_Clean|Shared_Dirty)
          MEMORY_TOTALS["${key}"]=$((MEMORY_TOTALS[${key}] + value))
          ;;
      esac
    done <&${rollup_fd} || true
    exec {rollup_fd}<&-
    READABLE_ROLLUPS=$((READABLE_ROLLUPS + 1))
  fi
done

if [[ ${READABLE_ROLLUPS} -gt 0 ]]; then
  for key in Rss Pss Private_Clean Private_Dirty Shared_Clean Shared_Dirty; do
    printf '%-16s %12d kB\n' "${key}:" "${MEMORY_TOTALS[${key}]}"
  done
  printf '%-16s %12d kB\n' "Private_Total:" "$((MEMORY_TOTALS[Private_Clean] + MEMORY_TOTALS[Private_Dirty]))"
else
  printf 'smaps_rollup was not readable for included PIDs.\n'
fi

printf '\nPer-process memory\n'
for pid in "${PIDS[@]}"; do
  if [[ -r "/proc/${pid}/smaps_rollup" ]]; then
    comm="unknown"
    [[ -r "/proc/${pid}/comm" ]] && read -r comm < "/proc/${pid}/comm" || true
    awk -v pid="${pid}" -v comm="${comm}" '
      /^(Rss|Pss|Private_Clean|Private_Dirty|Shared_Clean|Shared_Dirty):/ { values[$1] = $2 }
      END { printf "%8s %-28s RSS=%9s kB PSS=%9s kB Private=%9d kB\n", pid, comm, values["Rss:"], values["Pss:"], values["Private_Clean:"] + values["Private_Dirty:"] }
    ' "/proc/${pid}/smaps_rollup" 2>/dev/null || true
  fi
done

printf '\nProcess tree\n'
if command -v pstree >/dev/null 2>&1; then
  pstree -aps "${ROOT_PID}"
else
  ps -o pid,ppid,pgid,sid,stat,comm -p "${PIDS[*]}"
  printf 'pstree not found.\n'
fi

printf '\nInstantaneous CPU information\n'
ps -o pid,ppid,stat,pcpu,pmem,etime,time,comm -p "$(IFS=,; printf '%s' "${PIDS[*]}")"

printf '\nSampled CPU average\n'
declare -A BEFORE
for pid in "${PIDS[@]}"; do
  [[ -r "/proc/${pid}/stat" ]] && BEFORE["${pid}"]="$(awk '{ print $14 + $15 }' "/proc/${pid}/stat")"
done
sleep "${CPU_SECONDS}"
CPU_DELTA=0
for pid in "${PIDS[@]}"; do
  if [[ -n "${BEFORE[${pid}]:-}" && -r "/proc/${pid}/stat" ]]; then
    after="$(awk '{ print $14 + $15 }' "/proc/${pid}/stat")"
    CPU_DELTA=$((CPU_DELTA + after - BEFORE[${pid}]))
  fi
done
HZ="$(getconf CLK_TCK 2>/dev/null || printf '100')"
awk -v ticks="${CPU_DELTA}" -v hz="${HZ}" -v seconds="${CPU_SECONDS}" 'BEGIN { printf "Approx average CPU over %ss: %.2f%% of one CPU\n", seconds, (ticks / hz / seconds) * 100 }'

printf '\nExecutable and bundle paths\n'
for pid in "${PIDS[@]}"; do
  if [[ -e "/proc/${pid}/exe" ]]; then
    path="$(readlink -f "/proc/${pid}/exe" 2>/dev/null || true)"
    if [[ -n "${path}" && -r "${path}" ]]; then
      stat -c "PID ${pid} executable: %n (%s bytes)" "${path}"
    fi
  fi
done
