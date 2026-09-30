# Production reference tracking

`desktop/` in this repository is a **plain, git-native copy** of the `desktop/` subdirectory from the actual production Study Tracker repository, not a submodule/subtree with automatic linkage. This file is the durable record of exactly which upstream commit it mirrors, since earlier stages copied it in without recording that (an oversight this file corrects going forward — update it every time `desktop/` is resynced).

**Upstream repository**: `https://github.com/damcha02/destudydracker.git` (branch `main`). This is a different repository from this one's own `origin` (`https://github.com/damcha02/destudydracker-native.git`) — there is no git-level relationship between them; syncing is a manual, deliberate content copy.

## History

| Date synced | Upstream commit | Upstream commit date | Upstream message | How determined |
| --- | --- | --- | --- | --- |
| (original copy, undated) | `3906b937f0e7219fbae6dcf8a0caba65eb7ff49e` | 2026-08-10T18:58:32+02:00 | "Merge pull request #6 from damcha02/performance-optimization" | Reconstructed retroactively (Stage 14 prep): the original copy's `package.json` read `"0.1.58"`, but that version string had not yet been bumped at the exact commit copied. Found by diffing the working `desktop/` tree against several nearby upstream commits (`git archive <sha> desktop \| diff -rq`) until an exact content match (zero differing files besides local build artifacts and one missing `.env.example`) was found at `3906b93`'s content. |
| 2026-09-28 | `b095706994b6caaf18432d0d41b4534f4b85be98` | 2026-09-27T13:07:10+02:00 | "stuff" | `git fetch` of upstream `main`, `git checkout <ref> -- desktop` (see Stage 14's production-sync report for the full method and the reasoning behind using git checkout rather than a raw file copy). |
| 2026-09-30 | `fe2f7a60704aa77b3d72a1c86912e2d2a60274b9` | 2026-09-30T19:58:48+02:00 | "exam in wabi sabi" (4 commits after v0.1.66's b095706: `6945ca1` Pinwall, merge `e04f4fc`, `101c2bd` Release v0.1.67, `fe2f7a6`) | `git remote add prod-upstream` / `git fetch` / `git rm -r desktop` / `git checkout prod-upstream/main -- desktop` / `git remote remove`, exactly as "How to resync" below. Audited before replacing - see `docs/production-sync-0.1.67.md`. 17 files changed, all under `desktop/`. |

**Current reference: `fe2f7a60704aa77b3d72a1c86912e2d2a60274b9` (upstream `main`, 2026-09-30), production version `0.1.67`.** Previous reference: `b095706994b6caaf18432d0d41b4534f4b85be98` (v0.1.66), which Stages 14-17 were built and verified against.

## How to resync

1. Add the upstream repo as a temporary local remote and fetch its default branch — do **not** merge, rebase, or otherwise touch this repository's own history:
   ```
   git remote add prod-upstream https://github.com/damcha02/destudydracker.git
   git fetch prod-upstream main
   ```
2. Replace the tracked content: `git rm -r desktop` then `git checkout prod-upstream/main -- desktop` (git-native, fully reversible up to that point via `git restore`/`git checkout HEAD -- desktop`; avoids a raw bulk file-write/extraction, which environment safety checks may otherwise refuse as an unreviewable destructive operation).
3. Remove the temporary remote: `git remote remove prod-upstream`.
4. Update the table above with the new commit/date/message and how it was determined.
5. Diff the previous and new reference for anything relevant to the domain currently being migrated before continuing feature work (see the per-stage compatibility specs, e.g. `timer-compatibility-spec.md`) — do not assume nothing changed.
6. `desktop/` is never committed as part of native feature-migration work; a resync is its own separate, clearly labeled change, left uncommitted until the user asks for a commit (matching this project's established practice for `native-prototype/`-only changes too).
