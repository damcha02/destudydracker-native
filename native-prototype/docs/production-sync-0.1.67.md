# Production reference sync audit: v0.1.66 → v0.1.67

Audit performed **before** replacing `desktop/` (read-only upstream clone in a scratch directory; nothing in `desktop/` was edited, only replaced by the documented `git checkout prod-upstream/main -- desktop` procedure; no real user data was read).

## 1. Repository check

`desktop/` is a plain, git-native copy of the upstream `desktop/` subdirectory (`git ls-files -s desktop` shows ordinary blobs; there is no `.gitmodules`; there is no native code under it). The copied root-level `.github/workflows/*` are byte-identical to upstream's (see §6).

## 2. Upstream state

| | |
|---|---|
| Repository / branch | `https://github.com/damcha02/destudydracker.git`, `main` |
| Latest commit | `fe2f7a60704aa77b3d72a1c86912e2d2a60274b9` |
| Commit date | 2026-09-30T19:58:48+02:00 |
| Subject | "exam in wabi sabi" |
| Version | **0.1.67** (`package.json`, `tauri.conf.json`, `Cargo.toml`) |
| Previous reference | `b095706994b6caaf18432d0d41b4534f4b85be98` (v0.1.66) |
| Commits since | **4**: `6945ca1` feat(planner) "Pinwall of the past", `e04f4fc` merge, `101c2bd` Release v0.1.67, `fe2f7a6` exam in wabi sabi |
| Files changed | **17** (+2251/−59), **all under `desktop/`**; nothing outside it (no workflow, doc, cloudflare or root change) |

Changed files: `package.json`, `package-lock.json`, `src-tauri/{Cargo.toml,Cargo.lock,tauri.conf.json}` (version bump only), `src/App.css` (+1138, wabi/pinwall styling), `src/App.tsx` (+318), new `features/planner/PinwallModal.tsx`, `WabiManageSemestersModal.tsx`, new `lib/examPhase.ts`(+test), new `lib/pinwall.ts`(+test), `lib/plannerSchedule.ts`, `lib/releaseNotes.ts`, `lib/storage.ts` (+1), `types.ts` (+9).

## 3. Impact audit

| Area | Change | Classification |
|---|---|---|
| Timer behavior / persistence | none (`timer*.ts`, `useTimer*`, `src-tauri/src/lib.rs` untouched) | **NO IMPACT** |
| Sessions, metrics (`metrics.ts`), Dashboard render | none in the default Field Notebook Dashboard. The only Dashboard-adjacent edit is the exam-runway line, which prefixes the exam **kind** *only when `appStyle === "wabi-sabi"`* | **NO IMPACT** (default style); kind label = Stage 19 |
| Courses, semesters | none to the data model. Wabi "current semester" stays in focus through exam prep (wabi only) | **NO IMPACT** now; Planner/Wabi follow-up |
| **Exams** | new optional `Exam.kind` (`midterm|endterm|semester-end|project|session`; absent = `session`), persisted and normalized in `storage.ts` `normalizeExams` and therefore present in backups | **NATIVE FOLLOW-UP REQUIRED** (§4) |
| **Tasks** | new optional `Task.prep` / `Task.prepOf` (wabi exam-prep revision tasks; `totalUnits` hand-set). `storage.ts` spreads task objects, so both survive production load/backup. App.tsx recomputes a prep task's `completedUnits` from ticked occurrences instead of the schedule | **NATIVE FOLLOW-UP REQUIRED** (§4) |
| Planner / calendar | `expandTimetableEvents` gained an optional 6th `ExpandOptions {prepTaskIds, prepEndDate}` argument and split the old early returns per event; **with no options (every non-wabi style, i.e. the default) the behavior is identical** to v0.1.66 (archived → none; non-`semester` phase → none; range bounded by semester dates) | **DOCUMENTATION UPDATE** — Stage 17's `schedule.rs` port stays correct for the default style; prep options belong to the Planner/Wabi stages |
| Pinwall of the past (new feature) | Planner button + modal listing unchecked past items (`lib/pinwall.ts`); count badge `pinwallWeekCount` | **DOCUMENTATION UPDATE** (Planner, Stage 22) |
| Wabi-Sabi, visual styles | large additions (runway banner, exam strip, exam chip/kinds form, prep-stage course chips); `App.css` +1138 lines | **DOCUMENTATION UPDATE** (Stage 19 scope grows) |
| Knowledge Garden, Break Room, achievements, Travle/maps, Social | none | **NO IMPACT** |
| Settings / preferences, backup & restore | no new preference keys; backup shape unchanged (`backupVersion: 2`) except the optional fields above | covered by §4 |
| Release notes / what's-new | new `0.1.67` entry ("Nothing slips through") | **DOCUMENTATION UPDATE** |
| Tray, notifications, single-instance, updater, Tauri config, identity (`com.damcha.studytracker`), capabilities, Cargo dependencies | **none** (only the version string changed in `tauri.conf.json`/`Cargo.toml`) — the Stage 18 platform inventory below is unchanged from v0.1.66 | **NO IMPACT** |
| GitHub Actions / release workflow / packaging / signing / updater manifest generation | none (workflows unchanged; see §6) | **NO IMPACT** |

**No BLOCKING INCOMPATIBILITY.** Stages 14–17 (Timer semantics/compat spec, persistence, academic domain, Dashboard metrics) remain valid against v0.1.67 for the default style.

## 4. Native compatibility follow-up (Stage 18 preflight; *not* in the sync commit)

The sync commit deliberately changes no native code. The required compatibility work, to be done as a clearly labelled **Stage 18 preflight** with tests:

1. `study_tracker_core::academic`: add optional `Exam.kind` (enum `ExamKind`, absent ≡ `Session`) and `Task.prep: bool` / `Task.prep_of: Option<TaskId>`, serde-defaulted so Stage 15/16 stores keep loading (schema stays v1).
2. `persistence::migration_academic`: convert the new optional fields from a v0.1.67 backup (unknown/invalid `kind` → absent, like production's normalizer).
3. Do **not** change Dashboard behavior: default style is unaffected. (Whether prep tasks' `completedUnits` should follow production's new rule is Planner/Wabi work; the fields are preserved so no user data is lost on import.)
4. Tests: round-trip a backup containing kinds and prep tasks; old backups without them unchanged.

## 5. Why nothing else needs to move

The Stage 14 timer compatibility spec, Stage 15 envelope/migration classification, Stage 16 domain inventory and Stage 17 metric ports reference only files that did not change (`metrics.ts`, `scheduleWorkload.ts`, `scheduleHealth.ts`, `timer*.ts`, `App.tsx`'s Field Notebook dashboard render functions). The single shared function that did change, `expandTimetableEvents`, is option-gated as described in §3.

## 6. Production release pipeline, end to end (read from the repo, secrets not accessed)

```text
git tag vX.Y.Z (update-version.sh bumps package.json, lock, tauri.conf.json, Cargo.toml/lock; requires a releaseNotes.ts entry)
 └─ push tag "v*"  ──►  GitHub Actions `release.yml`
      1. job `secret-scan`: gitleaks (full history)
      2. job `release` (needs 1), matrix, max-parallel 1, fail-fast off:
           macos-latest  --target aarch64-apple-darwin  --bundles app,dmg
           macos-latest  --target x86_64-apple-darwin   --bundles app,dmg
           ubuntu-22.04  --bundles deb,rpm,appimage
           windows-latest --bundles nsis,msi
         each: checkout → Node 24 (npm ci in desktop/) → Rust stable → `tauri-apps/tauri-action@v0`
         env: VITE_SOCIAL_API_URL, STUDY_TRACKER_OFFICIAL_RELEASE=1 (compile-time flag that turns the updater on),
              TAURI_SIGNING_PRIVATE_KEY(+_PASSWORD) from repository secrets, GITHUB_TOKEN
      3. tauri-action: `tauri build` (Vite build → Rust build → bundle) with `bundle.createUpdaterArtifacts: true`
         → installers + updater signatures (`*.sig`, minisign-style, made with the secret key) 
         → uploads to a **draft** GitHub Release "Study Tracker vX.Y.Z" and generates/uploads `latest.json`
      4. a human publishes the draft (required: the updater URL below resolves only for the *published latest* release)
installed app (release builds only):
  startup and every 24 h (`AUTO_UPDATE_CHECK_INTERVAL_MS`) + manual "Check for updates" in Settings
  → tauri-plugin-updater `check()` GET https://github.com/damcha02/destudydracker/releases/latest/download/latest.json
  → compares versions, picks the `platforms[<target>]` entry (url + signature)
  → on user action: `downloadAndInstall(progress)`: download, **verify the signature against the public key embedded in tauri.conf.json**, run the installer (Windows `installMode: "passive"`: NSIS/MSI with a progress-only UI)
  → frontend calls `relaunch()` (tauri-plugin-process)
failures → message + link to the releases page; dev/source builds never update (`runtime_channel()`: debug → "development", no flag → "source-build").
```

Who supplies what:

| Piece | Supplied by |
|---|---|
| Workflow, matrix, `tauri.conf.json` (endpoint, **public** key, installMode), `update-version.sh`, release notes | this repository (identical copies exist in `destudydracker-native/.github`) |
| Runners (Windows/macOS/Linux), tag trigger, secrets store | GitHub Actions |
| Release hosting, draft→publish step, the stable `…/latest/download/latest.json` URL | GitHub Releases |
| Bundling (NSIS, WiX/MSI, `.app`/`.dmg`, deb/rpm/AppImage), updater signing, `latest.json` generation | Tauri CLI + `tauri-action` |
| Manifest fetch, version compare, signature verification, installer launch, relaunch | Tauri updater + process plugins (in the installed app) |
| Signing key pair | maintainer; the **private key and password live only in repository secrets** (never read here); the **public** key is committed in `tauri.conf.json` |
| Windows installer tooling | NSIS and WiX, preinstalled on `windows-latest`, driven by the Tauri bundler |
| macOS code signing / notarization | **not configured**: the workflow has no Apple certificate/notarization secrets, `tauri.conf.json` has no signing identity, and no Authenticode certificate exists for Windows either. The only signature is the updater (minisign) signature. |

## 7. What the native replacement must reuse, and what will be needed

Keep: the tag-triggered workflow shape, the gitleaks job, GitHub draft releases, the `latest.json` format and stable URL, the **existing minisign key pair and public key** (so already-installed Tauri builds can verify the first native update), `update-version.sh`, release notes, the 24 h check cadence and "Check for updates" UX.

Replace: `tauri build` / `tauri-action` with `cargo build --release` + an installer step that produces the same artifact names and `.sig` files, and a small script that writes `latest.json` (platform keys `windows-x86_64`, `darwin-aarch64`, `darwin-x86_64`; `version`, `notes`, `pub_date`, `platforms.*.{url,signature}`). Replace the in-app Tauri updater with the native updater implemented in Stage 18 (check → verify → stage) plus a Stage 23 install/relaunch step.

**Windows, eventual chain:** Rust binary → NSIS (or WiX) installer with the *same* product identity/upgrade code/AppUserModelID as v0.1.x so installing over the Tauri app is an in-place upgrade → installer signed with the updater key (`.sig`) → published draft release → installed native app downloads the installer, verifies the signature against the embedded public key, exits, runs the installer passively, installer relaunches the app. Open decisions for Stage 23: installer tool, per-user vs per-machine install (Windows cannot replace a running `.exe`, hence the installer-driven flow), relaunch responsibility, whether to add Authenticode signing (currently absent, so SmartScreen behavior is unchanged), data-directory continuity with `%LOCALAPPDATA%\com.damcha.studytracker` (the native prototype deliberately uses `…\com.damcha.studytracker.native-shell` until cutover).

**macOS, eventual chain (not built in Stage 18):** Rust binary → `.app` bundle (Info.plist, bundle id `com.damcha.studytracker`, icns, hardened runtime) → **Developer ID signing and notarization (new: none exists today)** → `.dmg` for first install and a signed `.app.tar.gz` (+ minisign `.sig`) for updates → in-app update downloads and verifies the archive, replaces the bundle (helper process/relaunch, Sparkle-like; the running bundle cannot overwrite itself), relaunches. Stage 23 must validate: signing identity/secrets in CI, notarization round-trip, Gatekeeper with quarantine on the downloaded update, both architectures (aarch64 + x86_64 or universal), and the `darwin-*` manifest keys.
