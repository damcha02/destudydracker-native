#!/usr/bin/env bash
# Stage 22a: one production Social / Daily Skribbl capture against the local mock Worker.
#
#   social-capture-prod.sh <prod-dist> <fixture.json> <out.png> <style> <theme> <seed> <js-steps> [probe.js probe.json] [WxH]
#
# Runs entirely inside a loopback-only network namespace (`unshare -rn`): the mock
# (`target/debug/social-mock`, synthetic seed, frozen 2026-10-04 12:00 Zurich) on 127.0.0.1:47811,
# the production scratch build (built with VITE_SOCIAL_API_URL=http://127.0.0.1:47811) in headless
# Chromium whose resolver maps every host except 127.0.0.1 to NOTFOUND. Production cannot be reached.
set -euo pipefail
DIST=$1; FX=$2; OUT=$3; STYLE=$4; THEME=$5; SEED=$6; STEPS=$7; PROBE=${8:-}; PROBE_OUT=${9:-}; SIZE=${10:-1520x980}
W=${SIZE%x*}; H=${SIZE#*x}
PAL=(); [ -n "${PALETTE:-}" ] && PAL=(--palette "$PALETTE" --anim-time 4000)
TAB=${TAB:-$( [ "$STYLE" = wabi-sabi ] && echo friends || echo social )}
HERE="$(cd "$(dirname "$0")" && pwd)"; ROOT="$(cd "$HERE/../.." && pwd)"
BROWSER=$(mktemp /tmp/st-chromium-XXXXXX.sh)
cat > "$BROWSER" <<'B'
#!/bin/sh
exec /usr/bin/chromium --no-sandbox --host-resolver-rules="MAP * ~NOTFOUND, EXCLUDE 127.0.0.1" "$@"
B
chmod +x "$BROWSER"
EVAL=(); [ -n "$PROBE" ] && EVAL=(--eval "$PROBE" --eval-out "$PROBE_OUT")
export STEPS
unshare -rn bash -c "ip link set lo up
  '$ROOT/target/debug/social-mock' --port 47811 --seed '$SEED' --now 2026-10-04T12:00:00+02:00 >/dev/null 2>&1 &
  sleep 0.5
  node '$HERE/capture-prod.mjs' --browser '$BROWSER' --dist '$DIST' --fixture '$FX' --out '$OUT' --w $W --h $H \
    --tab $TAB --style '$STYLE' --theme '$THEME' --now 2026-10-04T12:00:00+02:00 --tz Europe/Zurich \
    --math-random 0.25 --js \"\$STEPS\" --settle 1200 ${PAL[*]:-} ${EVAL[*]:-}
  kill %1"
rm -f "$BROWSER"
