#!/usr/bin/env bash
# Stage 22a: one native Social / Daily Skribbl capture against the local mock Worker.
#
#   social-capture-native.sh <exe> <fixture.json> <out.png> <seed> [KEY=VALUE ...]
#
# Inside a loopback-only network namespace (`unshare -rn`): the mock (synthetic seed, frozen
# 2026-10-04 12:00 Zurich) on 127.0.0.1:47811 writes the synthetic local-test credentials into a
# throwaway profile (seed `none` writes no credentials: the NoIdentity state), and the native app
# runs with STUDY_NATIVE_SOCIAL_ENDPOINT pointing at it. X reaches :99 through its filesystem socket.
# Extra KEY=VALUE pairs go to the app (STUDY_NATIVE_VIEW=social, STUDY_NATIVE_SOCIAL_SUBTAB=friends,
# STUDY_NATIVE_STYLE=wabi-sabi, STUDY_NATIVE_OPEN_GAME=4, ...).
set -euo pipefail
exe=$1; fixture=$2; out=$3; seed=$4; shift 4
HERE="$(cd "$(dirname "$0")" && pwd)"; ROOT="$(cd "$HERE/../.." && pwd)"
data=$(mktemp -d /tmp/st-native-XXXXXX)
trap 'rm -rf "$data"' EXIT
creds=(--write-credentials "$data"); mseed=$seed
[ "$seed" = none ] && { creds=(); mseed=demo; }
export ST_ARGS="$*"
unshare -rn bash -c "ip link set lo up
  '$ROOT/target/debug/social-mock' --port 47811 --seed '$mseed' --now 2026-10-04T12:00:00+02:00 ${creds[*]:-} ${MOCK_ARGS:-} >'$data/mock.out' 2>&1 &
  for i in \$(seq 100); do grep -q SOCIAL_MOCK '$data/mock.out' 2>/dev/null && break; sleep 0.05; done
  env -u WAYLAND_DISPLAY DISPLAY=\${CAPTURE_DISPLAY:-:99} WINIT_UNIX_BACKEND=x11 SLINT_SCALE_FACTOR=1 \
    STUDY_NATIVE_DATA_DIR='$data' STUDY_NATIVE_IMPORT_BACKUP='$fixture' \
    STUDY_NATIVE_NOW=2026-10-04T12:00:00+02:00 TZ=Europe/Zurich STUDY_NATIVE_SIZE=\${SIZE:-1520x980} \
    STUDY_NATIVE_BREAK_PICK=0.25 STUDY_NATIVE_SOCIAL_ENDPOINT=http://127.0.0.1:47811 \
    STUDY_NATIVE_SNAPSHOT='$out' STUDY_NATIVE_SNAPSHOT_DELAY_MS=\${DELAY:-2500} \$ST_ARGS timeout 60 '$exe' | grep -E '^(SNAPSHOT|NET)' || echo 'no snapshot'
  kill %1"
