#!/usr/bin/env bash
# Stage 22a: regenerate tests/fixtures/social/worker-goldens.jsonl from the PRODUCTION Worker code.
#
# Copies the tracked cloudflare/ reference into a scratch directory (cloudflare/ itself is never
# modified), installs its own dev dependencies there (npm registry only; --ignore-scripts because
# wrangler's optional `sharp` does not build on new Node versions and workerd needs no script),
# adds golden.test.ts and runs it with @cloudflare/vitest-pool-workers: the real Worker inside a
# local workerd with a local, in-memory D1/R2/KV. The test run happens in a network namespace with
# only loopback (`unshare -rn`), so it cannot reach Cloudflare or anything else.
#
#   scripts/stage22-worker-goldens/regenerate.sh [scratch-dir]
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
SCRATCH="${1:-$(mktemp -d /tmp/st-worker-golden-XXXXXX)}"
mkdir -p "$SCRATCH"
rm -rf "$SCRATCH/worker"
cp -r "$ROOT/cloudflare" "$SCRATCH/worker"
cp "$HERE/golden.test.ts" "$SCRATCH/worker/test/golden.test.ts"
cd "$SCRATCH/worker"
npm ci --ignore-scripts --no-audit --no-fund >/dev/null
unshare -rn sh -c 'ip link set lo up; npx vitest run test/golden.test.ts --reporter=verbose' > golden.log 2>&1
grep '^GOLDEN' golden.log | sed 's/^GOLDEN //' > "$ROOT/native-prototype/tests/fixtures/social/worker-goldens.jsonl"
echo "wrote $(wc -l < "$ROOT/native-prototype/tests/fixtures/social/worker-goldens.jsonl") golden responses (scratch: $SCRATCH)"
