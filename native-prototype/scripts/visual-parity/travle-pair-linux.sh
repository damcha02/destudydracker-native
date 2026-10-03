#!/usr/bin/env bash
# Stage 21: one deterministic Travle screenshot pair (production | native) and its mean abs
# difference. Both apps import the same synthetic backup ("full" Break Room, every game unlocked)
# whose travlePuzzle is today's production puzzle in a fixed state, frozen at 2026-09-30 12:00
# Europe/Zurich, 1520x980 at scale 1 (headless Chromium DPR 1 | native on a private Xwayland).
#
#   travle-pair-linux.sh <prod-dist> <native-exe> <outdir> <state> <style> <theme> [palette] [WxH]
#     state:   fresh | mid | won | lost      style: field-notebook | wabi-sabi
#     theme:   dark | light                  palette: default | sakura (petals frozen at 4000 ms)
#
# Production opens Travle by clicking its card's Play button (Field Notebook page, or the Wabi Rest
# room's games list); native opens it with STUDY_NATIVE_OPEN_GAME=2 (the same play path).
# Requires: node, chromium, an X server on $CAPTURE_DISPLAY (default :99, e.g. `Xwayland :99`).
set -euo pipefail
DIST=$1; EXE=$2; OUT=$3; STATE=$4; STYLE=$5; THEME=$6; PALETTE=${7:-default}; SIZE=${8:-1520x980}
W=${SIZE%x*}; H=${SIZE#*x}
HERE="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$OUT"
TAG="travle-$STATE-$STYLE-$THEME"; [ "$PALETTE" != default ] && TAG="$TAG-$PALETTE"; [ "$SIZE" != 1520x980 ] && TAG="$TAG-$SIZE"
FX="$OUT/fixture-$STATE.json"
if [ ! -f "$FX" ]; then
  node "$HERE/gen-fixture.mjs" realistic "$OUT/base.json" --today 2026-09-30 >/dev/null
  node --no-warnings "$HERE/gen-break-fixture.mjs" full "$OUT/base.json" "$FX" --today 2026-09-30 --travle "$STATE" >/dev/null
fi
OPEN="$(grep -v '^//' "$HERE/open-travle.js")"
SAKURA=(); NSAKURA=()
if [ "$PALETTE" = sakura ]; then SAKURA=(--palette sakura --anim-time 4000); NSAKURA=(STUDY_NATIVE_SAKURA_TIME=4000); fi
node "$HERE/capture-prod.mjs" --browser "${PROD_BROWSER:-/usr/bin/chromium}" --dist "$DIST" --fixture "$FX" --out "$OUT/prod-$TAG.png" \
  --w "$W" --h "$H" --tab break --style "$STYLE" --theme "$THEME" "${SAKURA[@]}" \
  --now 2026-09-30T12:00:00+02:00 --tz Europe/Zurich --math-random 0.25 --js "$OPEN" >/dev/null
SIZE=$SIZE bash "$HERE/capture-native-linux.sh" "$EXE" "$FX" "$OUT/nat-$TAG.png" \
  STUDY_NATIVE_VIEW=break STUDY_NATIVE_OPEN_GAME=2 STUDY_NATIVE_STYLE="$STYLE" STUDY_NATIVE_THEME="$THEME" \
  STUDY_NATIVE_PALETTE="$PALETTE" "${NSAKURA[@]}" >/dev/null
printf '%s\t' "$TAG"
node "$HERE/imgtool.mjs" diff "$OUT/prod-$TAG.png" "$OUT/nat-$TAG.png"
