#!/usr/bin/env bash
# Stage 17: render the production Dashboard and the native Dashboard for one scenario/layout/size
# and write <out>/prod-*.png and <out>/nat-*.png (plus a side-by-side). Usage:
#   pair.sh <prod-dist> <scenario> <layout> <width> <height> <outdir> [scale] [theme]
set -euo pipefail
DIST=$1; SC=$2; LAYOUT=$3; W=$4; H=$5; OUT=$6; SCALE=${7:-1}; THEME=${8:-dark}
HERE="$(cd "$(dirname "$0")" && pwd)"; ROOT="$HERE/../.."
FX="$ROOT/tests/fixtures/dashboard/$SC.json"
TAG="$SC-$LAYOUT-${W}x${H}"; [ "$SCALE" != "1" ] && TAG="$TAG@$SCALE"; [ "$THEME" != "dark" ] && TAG="$TAG-$THEME"
mkdir -p "$OUT"
node "$HERE/capture-prod.mjs" --dist "$DIST" --fixture "$FX" --out "$OUT/prod-$TAG.png" --w "$W" --h "$H" --layout "$LAYOUT" --theme "$THEME" --dpr "$SCALE" --now 2026-09-30T12:00:00+02:00 --tz Europe/Zurich >/dev/null
powershell -NoProfile -ExecutionPolicy Bypass -File "$HERE/capture-native.ps1" -Exe "$ROOT/target/${NATIVE_PROFILE:-debug}/study-tracker-native-prototype.exe" -Fixture "$FX" -Out "$OUT/nat-$TAG.png" -Layout "$LAYOUT" -Width "$W" -Height "$H" -Scale "$SCALE" -Theme "$THEME" >/dev/null
node "$HERE/imgtool.mjs" diff "$OUT/prod-$TAG.png" "$OUT/nat-$TAG.png"
echo "$TAG"
