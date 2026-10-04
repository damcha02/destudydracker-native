# Stage 22 — Social / Network + Daily Skribbl

## 1. Verdict

**STOPPED BEFORE IMPLEMENTATION. No native code was changed.** Decisions D1–D7 are now recorded in §13; the D5 backend-reference sync is complete. Stage 22a has not started.

The production network inventory (sections 4–10 below) turned up several of the brief's §82 stop
conditions. The main one is that production identity *is* a client-held bearer credential. The two
ways to reach parity are migrating that credential (§35 says to stop first) or minting a new
production account on first launch (§5/§36 forbid this). Production also sends state-creating
requests on its own at startup and from the Timer. Section 12 lists the decisions needed before
implementation can start safely.

This document is the Stage 22 inventory and contract. It is meant to be extended into the full
§80 report once the decisions are made.

## 2. Starting checkpoint

- Branch `main`; `HEAD` == `origin/main` == **`e717cd9ad9272ad21ffc82e179c8577090be7f0e`**
  ("native: verify stage 21 on windows and fix travle parity"). This is the Stage 21 Windows
  verification commit.
- Working tree clean, `git diff --check` clean, `git diff -- desktop` empty. Stage 23 not started.

## 3. Production freshness

- Upstream `damcha02/destudydracker` `main` is **`853cdb0`** ("stuff", 2026-10-04):
  `git describe` = `v0.1.67-3-g853cdb0`, and `package.json` is still `0.1.67`. Inspected through a
  read-only clone in a scratch directory. No remote was added to this repository.
- `desktop/` here still equals `fe2f7a6:desktop`. There are two commits since then:
  - `e5c5a94` (already classified in Stage 21): exam `note`/`releaseDate`, planner/Wabi calendar
    markers, scrollable Wabi nav.
  - `853cdb0` (new; `App.tsx` only, +51/−4): the Wabi "one thing" picker gains a **Missed** group
    (unticked rows from the past 14 days), and timeline row titles use their position in the
    occurrence series ("Lecture 3").
  - Neither touches Social, networking, auth, Skribbl, Break Room, persistence, API URLs, protocol
    payloads, achievements or themes. **Classified as unrelated.** They belong to a later
    planner/Wabi sync, and `desktop/` was not modified.
- **Material finding: this repository's `cloudflare/` is stale.** It comes from the original
  "copied all files" commit (`f725b33`), not from a production sync. Upstream `cloudflare/` differs
  in these ways:
  - adds `migrations/0021_skribbl.sql` and `0022_fix_competitive_user_totals_fanout.sql`;
  - adds `test/skribbl.test.ts` and `test/verified-session-grace.test.ts`;
  - changes `src/index.ts` (3271 → 3656 lines): **all six `/skribbl/*` routes**, the verified-session
    2-hour normal-credit grace window, and the leaderboard join fixes.

  The local copy has **no Daily Skribbl backend at all**. All protocol analysis below uses upstream
  `853cdb0:cloudflare/` (read-only) as the backend authority.
- **Resolved by D5 (prerequisite commit "sync production cloudflare reference for stage 22").**
  The tracked `cloudflare/` was replaced with `git checkout <853cdb0> -- cloudflare`, fetched from a
  read-only scratch clone. No remote was added, and nothing was deployed, migrated or contacted.
  - Change set: 4 files added, 3 modified, 0 deleted. Added: migrations `0021_skribbl.sql` and
    `0022_fix_competitive_user_totals_fanout.sql`, tests `skribbl.test.ts` and
    `verified-session-grace.test.ts`. Modified: `src/index.ts`, `test/reconcile-offline.test.ts`,
    `test/verified-sessions.test.ts`.
  - Byte-equivalence: the git tree hash of `cloudflare/` is now
    `02c10052d1303ea0fcc9878686ae8f3a1afe0a97`, identical to `853cdb0:cloudflare`.
  - `desktop/` was deliberately **not** synced and still mirrors `fe2f7a6`, so the unrelated Wabi
    planner commits `e5c5a94` and `853cdb0` are not included.

## 4. Production network inventory (traced)

Every production network path, traced from imports:

| Source | What |
|---|---|
| `desktop/src/lib/social.ts` (709 lines) | All Social HTTP calls (36 functions) over `fetch` to `SOCIAL_API_URL` |
| `desktop/src/lib/skribbl.ts` (112 lines) | Five Daily Skribbl calls over `fetch` to the same `SOCIAL_API_URL` |
| `desktop/src/components/SkribblRoom.tsx` (548 lines) | Drawing canvas, WebP export, gallery and vote UI. Drawing images are loaded as `<img src=imageUrl>` from server-provided URLs |
| `desktop/src/features/social/SocialScreen.tsx` (1115 lines) + `App.tsx` glue | Social tab with five subtabs: Feed, Leaderboard, Friends, Squad, Profile. Wabi "Circle" variant. Avatar crop editor, feed image upload, owner-only admin/R2 panels |
| `desktop/src-tauri/src/lib.rs` | `native_social_sync` (reqwest POST relay, used only as a fallback when the WebView `fetch` of `/sync/v2` throws). `get_device_identity` (FNV-1a-64 hash of os/arch/machine-id plus a label). Updater `reqwest` calls are a separate concern (§62) |
| `App.tsx` effects | Scheduling: see §8 |
| Upstream `cloudflare/src/index.ts` | Worker with D1, R2 (`FEED_IMAGES`), KV (`RATE_LIMIT_KV`) and a daily cron at `0 3 * * *` |

**Transport:** HTTPS request/response only. There is **no WebSocket, EventSource, Durable Object
or long poll**. "Presence" means `POST /presence` plus server-side `last_seen_at`. Every
"real-time" surface is client polling.

**Base URL:** `VITE_SOCIAL_API_URL`, or the compiled-in default
`https://study-tracker-social.<account>.workers.dev`. This is public, non-secret configuration
(class A/B) and is defined twice, once in `social.ts` and once in `skribbl.ts`.

**Headers:** `content-type: application/json` on JSON calls, and multipart `FormData` on uploads.
There are **no auth headers, cookies or bearer tokens**. CORS on the Worker is `*`.

## 5. Network contract

**Auth for every authenticated call:** `{userId, deviceSecret}` is sent in the JSON body. The
exceptions are `/skribbl/theme` and `/skribbl/leaderboard`, which send it in the **GET query
string**, and the multipart forms, which send it as form fields. The server runs `verifyUser`,
which checks an HMAC hash of the secret (`DEVICE_SECRET_HASH_SECRET`, server-only). Unknown users
get `404 "User has not synced a profile yet."`.

**Client behaviour shared by all calls:**
- There is no client timeout. Browser `fetch` has none, and the Tauri reqwest relay uses
  `Client::new()`, which has no total timeout.
- There is no retry or backoff. The exceptions are periodic re-polling and the verified-session
  "not found → restart" rule.
- Errors: a non-2xx response throws `Error(responseText || "… HTTP <status>")`. The server's
  plain-text message is therefore shown to the user verbatim.
- Unauthenticated endpoints are marked "none" in the table.

| # | Feature | Method / path | Request (besides auth) | Response | Auth | Trigger | Class |
|---|---|---|---|---|---|---|---|
| 1 | Sync | POST `/sync/v2` | `user{userId,deviceSecret,friendCode,displayName,avatar,isPrivate,showHoursToFriends,lifetimeStudyMinutes,lifetimeStudySessions,device{fingerprintHash,label},app{version,platform,runtimeChannel}}`, `stats[≤370]{date,minutes,sessions}`, `feedPosts[≤25]{id,type,subject,detail,note,icon,minutes,presetLabel,createdAt,poll}`. Body ≤256 KiB | `{social:{friends,…,squadMessages}, syncedAt}` | body (creates the account if unknown) | startup, hourly, 2 s after sessions change, opening Leaderboard, manual | **state-creating** |
| 2 | Sync (legacy) | POST `/sync` | as above | as above | body | not called by the current client | — |
| 3 | Presence | POST `/presence` | `app` | `{ok}` | body | startup, opening a Social subtab | mutating (`last_seen_at`) |
| 4 | Friends status | POST `/friends/status/v2` | — | `{social:{friends,requests,squad,squadMessages}}` | body | Social tab open: immediately, then every 2 min | read (bumps last_seen) |
| 5 | Friend request | POST `/friends/request` | `friendCode` | sync-shaped | body | user | mutating |
| 6 | Friend respond | POST `/friends/respond` | `requestId, response∈{accepted,declined}` | sync-shaped | body | user | mutating |
| 7 | Leaderboard | POST `/leaderboard` | `scope∈{global,friends,squad}, period∈{daily,weekly,overall}` | `{entries[]}` | body | subtab/scope/period change | read |
| 8 | Player stats | POST `/player-stats` | `targetUserId` | `PlayerStatsResponse` | body | open a profile | read |
| 9 | Feed | POST `/feed` | `scope∈{global,friends}` | `{feed[], r2Usage?}` | body | Feed open, every 2 min while open | read |
| 10 | React | POST `/feed/react` | `postId, emoji` | `{ok}` | body | user | mutating |
| 11 | Poll vote | POST `/feed/poll/vote` | `postId, optionId` | `{ok, poll}` | body | user | mutating |
| 12 | Comment | POST `/feed/comment` | `postId, body≤220` | `{ok, comment}` | body | user | mutating |
| 13 | Edit post | POST `/feed/update` | `postId, note` | `{ok}` | body | user | mutating |
| 14 | Delete post | POST `/feed/delete` | `postId` | `{ok}` | body | user | destructive |
| 15 | Post image | POST `/feed/image` (multipart) | `postId, image(webp)≤5 MiB` | `{ok,imageUrl,imageMimeType,imageExpiresAt,…}` | form | user | mutating (R2) |
| 16 | Delete image | POST `/feed/image/delete` | `postId` | `{ok}` | body | user | destructive |
| 17 | Feed image | GET `/feed/image/<key>` | — | image bytes | none | `<img>` render | read (R2 class B budget) |
| 18 | Avatar upload | POST `/profile/avatar` (multipart) | `name, image≤256 KiB` | `{avatar}` | form | user | mutating |
| 19 | Avatar | GET `/profile/avatar/<key>` | — | image bytes | none | `<img>` render | read |
| 20 | Squad create | POST `/squads/create` | `name≤48, isPrivate` | sync-shaped | body | user | state-creating |
| 21 | Squad search | POST `/squads/search` | `query` | `{squads[]}` | body | user / suggestions | read |
| 22 | Squad details | POST `/squads/details` | `squadId` | `{squad}` | body | user | read |
| 23 | Squad join | POST `/squads/join` | `squadId` | sync-shaped | body | user | mutating |
| 24 | Squad respond | POST `/squads/respond` | `requestId, response` | sync-shaped | body | user | mutating |
| 25 | Squad leave | POST `/squads/leave` | — | sync-shaped | body | user | mutating |
| 26 | Squad chat | POST `/squads/chat` | `body≤500` | sync-shaped | body | user | mutating |
| 27 | Squad chat delete | POST `/squads/chat/delete` | `messageId` | sync-shaped | body | user | destructive |
| 28 | Squad role | POST `/squads/promote` | `targetUserId, role` | sync-shaped | body | user | mutating |
| 29 | Squad kick | POST `/squads/kick` | `targetUserId` | sync-shaped | body | user | destructive |
| 30 | Squad settings | POST `/squads/settings` | `name, isPrivate` | sync-shaped | body | user | mutating |
| 31 | Squad scoreboard | POST `/squads/scoreboard` | `period∈{daily,season,overall}` | `{entries[]}` | body | subtab change; 60 s client cache | read |
| 32 | Verified start | POST `/verified-session/start` | — | `{sessionId,startedAt,resumed}` | body | **Timer** starts a study/exam phase | state-creating |
| 33 | Verified heartbeat | POST `/verified-session/heartbeat` | `sessionId` | `{ok}` | body | **every 15 min while the Timer runs** | mutating |
| 34 | Verified finish | POST `/verified-session/finish` | `sessionId` | `{ok,creditedMinutes,finishedAt}` | body | Timer stops / pauses / enters a break | mutating |
| 35 | Offline reconcile | POST `/verified-session/reconcile-offline` | `anchorSessionId, intervals[≤500]{startedAt,endedAt}, chainTipHash(SHA-256 of canonical JSON)` | `{ok,creditedMinutes,cappedFromClaimedMinutes,flagged}` | body | anchor gap > 2 h closes | mutating |
| 36 | Announcement | GET `/announcements/current?appVersion=` | — | `{announcement\|null}` | none | startup + poll | **anonymous read** |
| 37 | Update notice | POST `/announcements/update-notice` | `targetVersion` | `{ok,id,targetVersion}` | body (owner) | owner only | mutating |
| 38 | Telemetry | POST `/telemetry/heartbeat` | `installId`, `app` | `{ok}` | none | **opt-in setting**: at startup, then hourly | state-creating |
| 39 | Admin usage | POST `/admin/usage` | — | `AdminUsageResponse` | body (owner) | owner only | read (PII of other users) |
| 40 | Skribbl theme | GET `/skribbl/theme?userId&deviceSecret` | — | `{date,theme,submitted,drawingId,imageUrl}` | **query string** | open Skribbl | read (bumps last_seen; may pick the day's theme) |
| 41 | Skribbl gallery | POST `/skribbl/gallery` | `date, offset, limit(clamped 12–20; client 16)` | `{date,drawings[],total,nextOffset,hasMore}` | body | after submit, "load more" | read |
| 42 | Skribbl submit | POST `/skribbl/submit` (multipart) | `date, image(webp\|png)≤1.5 MB` | `{ok,drawingId,date,imageUrl}`; 409 if already submitted, 400 if wrong date | form | user | **state-creating, once per day** |
| 43 | Skribbl vote | POST `/skribbl/vote` | `drawingId, vote∈{-1,0,1}` | `{ok,score}`; 403 for own drawing, 400 if voting closed | body | user | mutating (idempotent upsert) |
| 44 | Skribbl leaderboard | GET `/skribbl/leaderboard?userId&deviceSecret` | — | `{date(yesterday),winner\|null}` | **query string** | open Skribbl | read |
| 45 | Skribbl drawing | GET `/skribbl/drawing/<key>` | — | image bytes (webp/png), `cache-control: max-age=1800` | none | `<img>` render | read |

Also present: `GET /health`, the owner-only admin migrations, and honeypot paths (`/admin`,
`/.env`, …). A client must never call any of these.

## 6. Secrets classification (no values)

| Item | Where | Class |
|---|---|---|
| `SOCIAL_API_URL` default / `VITE_SOCIAL_API_URL` | client | A — public config |
| Worker route paths, limits | client + server | B |
| `userId` (UUIDv4) | client `localStorage` / backup | C — per-user identifier |
| **`deviceSecret` (UUIDv4)** | client `localStorage` / backup; sent in bodies **and in GET query strings** | **C — per-user bearer credential** |
| `friendCode` | client; shared with friends | C (semi-public) |
| `verifiedAnchor{sessionId,confirmedAt}` | client | C — session state |
| telemetry `installId` | client `localStorage` | C (pseudonymous) |
| device `fingerprintHash` (FNV-1a of machine-id) | computed per launch | C (pseudonymous device ID) |
| `IP_HASH_SECRET`, `DEVICE_SECRET_HASH_SECRET` | Worker secrets | D — server-only. **Never present in the client.** ✔ |
| Owner friend code constant | server source | B/D-adjacent. It gates owner views. It is an identifier, not a secret, but it is not reproduced here |
| D1/KV/R2 binding IDs in `wrangler.toml` | server config | B |

**No server-only secret is exposed to the client.** The admin and owner gates are server-side
checks against the owner's authenticated identity.

Note: the `deviceSecret` travels in a URL query string on two Skribbl GETs. Production does this
by design; it is a weakness (URLs end up in logs), not a leak of a server secret. A native port
must reproduce the query-string form to stay protocol-compatible, and must make sure its own logs
never record the URL.

## 7. Identity and auth (exact mechanism)

- `makeDefaultSocialState()` creates the identity on first load:
  - `userId = crypto.randomUUID()`
  - `deviceSecret = crypto.randomUUID()`
  - `friendCode` = two blocks of four characters from a 32-character alphabet
  - `displayName = "Student " + last 4 characters of the friend code`
- There is **no login, password, OAuth or server-issued token**. The server account is created
  lazily by the first `/sync/v2` that carries an unknown `userId`. That call records an IP HMAC,
  country and ASN, and is rate-limited to 5 new accounts per IP per hour, with flags for
  `bulk_backfill`, an atypical friend code and implausible growth.
- After that, the server-side hash check makes the `deviceSecret` the only credential.
  Whoever holds `{userId, deviceSecret}` *is* the account. Production's `restoreBackup`
  deliberately carries both values, and the backup comment says that this is what "makes the
  restored device the same account".
- Stage 15 classified `social` as **Withheld**. Native has no identity today.

## 8. When production talks to the network (unprompted)

| Moment | Calls |
|---|---|
| App launch (Tauri only) | `/presence`; `/sync/v2` if `nextAutoSyncAt` is due (always true on a fresh profile, so **account creation**); `/announcements/current`; `/telemetry/heartbeat` if the user opted in |
| Every hour | auto-sync check; telemetry if opted in |
| Announcement poll | `/announcements/current` |
| 2 s after the session list changes | `/sync/v2` |
| Social tab visible | `/friends/status/v2` every 2 min; `/feed` every 2 min on the Feed subtab; presence and sync when a subtab is opened |
| **Timer running (study/exam phase)** | `/verified-session/start`, `heartbeat` every 15 min, `finish` on stop. Offline reconcile after a gap of more than 2 h. The tray close semantics already model "verified session active" |
| Opening Daily Skribbl | `/skribbl/theme`, `/skribbl/leaderboard`, then `/skribbl/gallery` if already submitted, plus image GETs |

## 9. Social feature inventory (what actually exists)

- **Profile:** display name, avatar (letter + style, icon, or a cropped photo uploaded to R2),
  private toggle, "show hours to friends", "auto-post sessions", friend code and invite link,
  badges.
- **Friends:** add by code, incoming and outgoing requests, accept or decline, live and recent
  status (`lastSeenAt` within 45 min counts as "live"), player-stats profile.
- **Leaderboard:** global / friends / squad × daily / weekly / overall. Local self-entry is merged
  in, with ranks and an "arena" presentation.
- **Feed:** global / friends scope; session posts (manual or auto), notes, a 5-day image TTL,
  polls (up to 12 options, single or multiple choice), three reaction emoji, comments of up to 220
  characters, edit and delete. Images and polls each have a visibility setting.
- **Squads:** up to 4 members, public or private, search, suggestions, join or request, roles
  (promote/demote), kick, settings, leave, chat (up to 500 characters, delete own message), and
  squad scoreboards for daily / season / overall, with hard-coded season dates.
- **Verified sessions:** Timer-driven leaderboard credit, plus offline reconcile.
- **App announcements** (anonymous), **opt-in telemetry**, and the **owner-only admin** usage, R2
  usage and update-notice panels.
- **Wabi:** the same tab re-skinned as "Circle", with an attendance list (48 h) and a
  competitive/quiet toggle.

There are **no** DMs, WebSockets or real-time rooms. "Presence" is `last_seen_at` only.

## 10. Daily Skribbl inventory (exact)

**What it is:** a single-player drawing prompt with an asynchronous community gallery. It is
**not** skribbl.io: there are no rooms, turns, guesses, word guessing or live players.

**Daily identity:**
- The **server** chooses the day's theme. It picks randomly from the seeded pool, avoids
  yesterday's theme, and persists the choice in `skribbl_daily_themes`.
- The day is the server date in **`Europe/Zurich`** (`SERVER_TIME_ZONE`). It is not the local date
  and not UTC, so it differs from Wordle/Travle (local date).

**Flow:**
1. Open the modal. `GET /skribbl/theme` returns `{theme, date, submitted, imageUrl}`.
2. Intro screen, then a 3-minute drawing phase (`SKRIBBL_DRAW_SECONDS = 180`).
3. Canvas: 900×600, a 21-colour palette, brush sizes {3, 6, 10, 16, 26}, brush and flood-fill
   tools (tolerance 40), undo stack.
4. Export with `canvas.toBlob("image/webp", 0.85)`, falling back to PNG.
5. Multipart `POST /skribbl/submit`, once per user per server day (409 on a second attempt).
6. Once submitted, the gallery unlocks: 16 drawings per page, paginated, with up/down votes
   (`-1/0/1`, no voting on your own drawing, today only). Drawings can be expanded; Escape closes.
7. Yesterday's winner comes from `GET /skribbl/leaderboard`.

**Server rules:** the cron job purges drawings, votes and themes for past days and snapshots the
winner. Gallery images are fetched from **server-provided `imageUrl`s**.

**Local persistence:** **none** beyond the Stage 20 break-room play log (`logPlayedBreak`) and the
token unlock. Production stores no Skribbl state locally. The server is the only record of
"submitted today".

**Failure handling:**
- A failed theme fetch shows an error with a retry button. A "Not found" response maps to "isn't
  live on the server yet".
- Gallery failures are ignored silently.
- Without an identity (`!userId || !deviceSecret`), the modal shows the "connect Social" intro.
  Native already reproduces this intro as its Stage 20 shell.

**Achievements:** none specific to Skribbl. It counts only through the Stage 20 unlock and play
machinery.

## 11. Why implementation cannot proceed safely as specified

1. **Identity = credential (§35/§36/§82).** Native parity needs an `{userId, deviceSecret}`. There
   are only two ways to get one:
   - **(a) Import it from the production backup.** That migrates a bearer credential, which §35
     says to stop before implementing.
   - **(b) Generate a new one.** Under production semantics, the first native launch would then
     silently create a **second, separate production account** for the same person: a new friend
     code, no friends, no squad, and a duplicate leaderboard presence. §36 forbids "silently
     generate a new incompatible identity". It also means that *any* run of a parity-faithful
     native build, including my own smoke, static-frame and Windows-VM verification runs, would
     create real production accounts (§5).
2. **Production mutates production without user action.** The startup sync and presence, the
   Timer-driven verified-session start/heartbeat/finish, and the post-session sync are all
   automatic. Exact parity therefore makes ordinary local use (and the Stage 14/19/21 regression
   runs, which run the Timer) state-mutating against the live Worker, unless the native default
   endpoint is disabled or gated. That would be a semantic difference from production and needs
   the user's approval.
3. **Telemetry.** The frozen architecture says "no telemetry added". Production has an *opt-in*
   telemetry heartbeat (`installId` + app metadata, hourly), and `/sync` and `/presence` also send
   app version, platform and a machine-ID-derived device fingerprint. It is unclear whether porting
   these counts as "adding telemetry" or as "preserving production". This needs a decision.
4. **The Timer domain is entangled.** Verified sessions hook into the Stage 14 Timer (start, 15-min
   heartbeat, finish, offline reconcile with a SHA-256 chain hash). That reaches into a frozen,
   Windows-verified domain, and it needs explicit scoping.
5. **Backend reference is stale in-repo.** The local `cloudflare/` lacks the Skribbl backend
   entirely (§3). Implementation would be built against upstream `853cdb0`, which is not the
   tracked reference here. This needs either a sanctioned `cloudflare/` sync (a chore commit like
   `e23871f`) or explicit approval to treat the scratch upstream as the authority.
6. **Remote image loading / SSRF (§61).** Feed images, avatars and Skribbl drawings are rendered
   from server-supplied absolute URLs. The native port needs a rule. The proposal is to accept only
   URLs whose origin equals the configured API origin and whose path is under
   `/feed/image/`, `/profile/avatar/` or `/skribbl/drawing/`, and to drop everything else. Display
   also needs a WebP **decoder**: `image-webp 0.2` is already linked through Slint, so this adds no
   crate. For uploads, the server accepts PNG for Skribbl (`png|webp`) and for feed images and
   avatars (`png|jpeg|webp|gif`). Native can therefore upload PNG everywhere with the
   already-linked `png` crate and needs no WebP encoder. This is a byte-format difference (PNG
   instead of WebP 0.85), and the size caps still apply: 1.5 MB for Skribbl, 256 KiB for avatars.

No mock or synthetic test is blocked. All of these protocols can be served by a local fake. The
blockers concern what the *shipped* native client does against the real Worker, and whose identity
it uses.

## 12. Decisions requested

| ID | Decision | Recommendation |
|---|---|---|
| D1 | Identity source | **Explicit, user-initiated "Link existing account" import** of `userId`/`deviceSecret` from a production backup the user chooses, stored only in the native store's `social` section (never logged; redacted in diagnostics). **No automatic generation.** "Create new account" is a separate explicit action with a warning. Until then, Social shows the production "not connected" state |
| D2 | Automatic network at startup / Timer | Preserve production's schedule **only after** an identity exists. With no identity, make zero connections, which matches production's own `socialConfigured`/identity gating for Skribbl. Tests and dev runs use an explicit local endpoint, and release builds refuse non-HTTPS non-loopback overrides |
| D3 | Telemetry heartbeat | **Do not port** in Stage 22: it conflicts with "no telemetry added". Keep the setting visible but inert, and document the difference. Device fingerprint and app metadata in `/sync`: port, since the server stores them for the account's device list |
| D4 | Verified sessions | Port in Stage 22 behind the Timer controller as an effect port (core emits `VerifiedSessionIntent`; the adapter performs the calls). Or defer to a Stage 22b to keep the Timer untouched. Recommendation: **22b** |
| D5 | `cloudflare/` reference | Separate chore commit syncing `cloudflare/` to upstream `853cdb0`, mirroring the `desktop/` sync procedure, **before** Stage 22 code |
| D6 | Scope split | 22a: transport, identity link, Daily Skribbl (complete), Friends/Leaderboard/Profile (read plus friend requests). 22b: Feed (posts, polls, comments, images), Squads (chat, roles), avatar photo crop/upload, verified sessions, owner panels. 22a alone is still a full stage |
| D7 | HTTP stack | `ureq 3` + `rustls` (ring provider) + `webpki-roots`, blocking calls on one dedicated network worker thread with a bounded queue and 10 s connect / 20 s total timeouts. No async runtime, no WebSocket. Measure binary growth before accepting (estimated +1.5–2.5 MB) |

## 13. Decision record (D1–D7, decided by the project owner after the stop)

| ID | Decision | Recorded outcome |
|---|---|---|
| D1 | Identity | **Accepted with modification.** Explicit identity states are `NoIdentity`, `ExistingIdentity` and `NewIdentity`. An existing identity is the production-compatible `{userId, deviceSecret}`. Launching native must never create a second identity automatically. Creating a new identity is always an explicit user action. `deviceSecret` is a credential: it is never logged, never displayed unnecessarily, and never put in diagnostics, screenshots or debug exports. Stage 22 uses **synthetic identities only**: the owner's real identity and `deviceSecret` are never imported or used, and tests never contact production. Stage 22 may build the secure storage/import boundary and exercise it with synthetic fixtures only. **The final automatic Tauri → native credential migration belongs to Stage 24.** |
| D2 | Background network | **Accepted.** Production schedules apply only while a valid identity exists. In `NoIdentity` there are zero Social, presence, verified-session and telemetry connections, and the app stays fully usable locally. Dev/test configuration points explicitly at localhost/mock infrastructure. Automated tests are structurally prevented from reaching production |
| D3 | Telemetry | **Modified.** Port production's opt-in telemetry faithfully (opt-in only, same default, same data semantics, same hourly cadence). It is migration of an existing feature, not new telemetry. Telemetry never carries `deviceSecret` or other credentials. Tests use mock transport, and dev/test mode never contacts production telemetry. **Stage 22b.** |
| D4 | Verified sessions | **Accepted: deferred to Stage 22b.** The pure Timer domain stays frozen. Integration goes Timer/domain event → application integration → verified-session network service |
| D5 | Backend reference | **Accepted, done.** Separate prerequisite commit syncing `cloudflare/` to upstream `853cdb0` (see §3) |
| D6 | Stage split | **Accepted.** **22a:** transport foundation, endpoint/config safety, identity architecture, synthetic credential support, local mock server and fixtures, Daily Skribbl complete, Friends, Leaderboards, Profile, offline/error/retry, the relevant Field Notebook/Wabi surfaces, security/validation, persistence boundaries. **22b:** Feed (polls, comments, reactions, images), Squads (chat, roles, kick, scoreboards), avatar/image upload not needed by Profile, verified sessions, opt-in telemetry, owner/admin panels, remaining Social parity. Stage 23 untouched |
| D7 | HTTP stack | **Provisionally accepted:** `ureq` + `rustls`. Before freezing, measure the dependency tree, release binary growth, startup, idle memory and network-thread behaviour, and report before switching stacks if the cost is unexpectedly large. Requirements: one controlled network worker, finite timeouts, rustls certificate validation (never disabled), no async runtime just for networking, no thread per request, bounded queues, clean shutdown, no network I/O on the Slint UI thread |
| — | Image URL policy | Server-supplied image URLs are never arbitrary network targets. The allow-list is derived from the backend contract: HTTPS, the configured API origin, and the path prefixes `/feed/image/`, `/profile/avatar/` and `/skribbl/drawing/`. Responses must be bounded in size and have an expected image content type. Credentials are never forwarded to an image origin. PNG upload is sufficient (production accepts PNG for Skribbl, feed images and avatars) |

## 14. Production integrity / data status

- `desktop/` unchanged. `cloudflare/` changed only by the D5 reference sync (source only: nothing deployed, migrated or contacted).
- No production endpoint was contacted. The only network access was a `git fetch` of the public
  upstream source repository into a scratch directory.
- No real user data or credentials were read or used.
- No native files changed except this document.

---

# STAGE 22a IMPLEMENTATION

Network foundation, Daily Skribbl, Friends, Leaderboards and Profile on Linux. Uncommitted;
Windows verification pending. Sections 1–14 above are the Stage 22 inventory, stop report and
decision record and are unchanged.

**Verdict: PASS WITH CONCERNS — PENDING WINDOWS VERIFICATION** (concerns in §22a.12).

## 22a.1 Scope delivered

| Area | Status |
|---|---|
| Transport (D7: `ureq` 3.4 + `rustls`, ring, webpki-roots) | Done. One lazily created worker thread, bounded queues, finite timeouts, no redirects, TLS validation never disabled, no async runtime |
| Endpoint safety | Done. `SocialEndpoint::{Production, Test(loopback only)}`; override `STUDY_NATIVE_SOCIAL_ENDPOINT` accepts only `http://<loopback>:<port>`; credentials bound to an endpoint class; test builds refuse non-loopback connections at the transport |
| Identity (D1) | Done. `NoIdentity` / `NewIdentity` / `ExistingIdentity`; a new identity only after an explicit confirmation; committed only after a successful bootstrap (rolled back otherwise) |
| Credential boundary | Done with synthetic credentials only. `social-credentials.json` (0600, atomic), `DeviceSecret` with redacted `Debug` and zeroing `Drop`. The production backup import still *withholds* the `social` section. Real migration: Stage 24 |
| Mock server + goldens | Done. `crates/social-mock` reproduces the 22a routes; `tests/fixtures/social/worker-goldens.jsonl` (73 responses) was recorded from the real Worker running offline; a conformance test replays them against the mock |
| Daily Skribbl | Done. Europe/Zurich day, 900×600 canvas, brush (5 sizes) / fill / undo (14) / clear, 21-colour palette + hex field, deadline-based 3-minute countdown with one auto-submit at expiry, PNG export, one submission per day, gallery (16 per page, load more), voting, lightbox, yesterday's winner, nothing persisted locally |
| Friends | Done. Send by tag (input upper-cased, validated), accept/decline, pending, friends with "seen", invite link copy |
| Leaderboards | Done. Friends / World arenas × Daily / Weekly / Overall; private-profile notice; Squad arena shows a 22b notice |
| Profile | Done. Name edit and default-name prompt, tag/invite copy, the six mini stats, sync console, the three toggles, player dialog for others |
| FN UI | Done. "Study Circle" page with Feed/Squad 22b notices, tab unread dot, dialogs, banner |
| Wabi UI | Done. Sidebar Circle item + submenu (Feed / Standings when competitive / Friends / Squad / Profile + competitive link, persisted as `preferences.wabiCircleCompetitive`), attendance chips, Friends, Standings podium, Profile |
| Offline / errors / retry | Done (§22a.7) |
| Deferred to 22b, shown honestly | Feed posts/polls/comments, Squads, Squad Arena, badges, avatar editor, verified sessions (D4), telemetry (D3) |

## 22a.2 Architecture

```
Slint UI ──callbacks──▶ app_social / app_skribbl (UI-thread glue, timers, view building)
                          │            ▲ replies via slint::invoke_from_event_loop
                          ▼            │
                 SocialController / SkribblController   (decisions; no I/O; headless-tested)
                          │ Outgoing{token, request, cancel, post}
                          ▼
                 app_net ──▶ net::worker (1 thread "social-net", API queue 32, image queue 48)
                                  └─▶ net::transport (ureq + rustls) ──▶ endpoint origin only
core: study-tracker-core::social (ids, avatar, identity, time, profile, friends, leaderboard,
      stats, limits) and break_room::skribbl (session state machine, Zurich day, flood fill)
```

- The worker is created at the first request. With no account it never exists.
- Image bytes are decoded on the worker. The decoder checks the format signature, declared type and pixel/byte limits.
- Image URLs pass `net::images::allow` (§13 policy). Credentials are never sent to images.
- Replies are matched by token. Stale or cancelled replies change nothing.
- Read-only refreshes (presence, status, the same leaderboard) are coalesced while an identical one is in flight.

## 22a.3 Files

- New core:
  - `crates/study-tracker-core/src/social/*` (19 tests).
  - `crates/study-tracker-core/src/break_room/skribbl/*` (17 tests).
- New app modules:
  - `src/net/{endpoint,http,transport,worker,device,multipart,images,social_api}.rs`, plus `social_api_tests.rs` and `mock_conformance_tests.rs`.
  - `src/{app_net,app_social,app_skribbl,net_jobs,image_cache,skribbl_canvas,skribbl_controller,social_controller}.rs`, plus `*_tests.rs`.
  - `src/persistence/{social_credentials,social_port}.rs`.
- New UI:
  - `ui/social/{types,fn-social,wabi-circle,dialogs}.slint`.
  - `ui/break/skribbl.slint`.
- New mock crate: `crates/social-mock`.
- Fixtures and scripts:
  - `tests/fixtures/social/`.
  - `scripts/stage22-worker-goldens/`.
  - `scripts/visual-parity/{social-capture-prod.sh,social-capture-native.sh,probe-box.js,open-skribbl.js}`.
- Modified:
  - `Cargo.toml`/`Cargo.lock` (ureq, getrandom, image, tiny-skia; dev: social-mock).
  - `src/main.rs`: install/shutdown, STATS fields, `STUDY_NATIVE_NET_AUDIT`, and the input script's `drag:` step.
  - `src/app_appearance.rs`, `src/app_break_games.rs`, `src/game_tokens.rs`, `src/persistence/mod.rs`.
  - `ui/main.slint`, `ui/fn/{shell,page}.slint`, `ui/break/{fn-break,games,types}.slint`, `ui/wabi/{page,sidebar}.slint`.
- Untouched:
  - the Timer domain;
  - `desktop/`;
  - `cloudflare/` (still equal to faa48d7).

## 22a.4 Production behaviour ported

- Startup: presence, then the auto-sync if it is due. The hourly due check follows.
- A sync runs 2 s after the session count changes, including at start when sessions exist.
- A friend-status poll runs every 2 minutes, only while the Social tab is visible.
- Subtab effects:
  - Leaderboard: presence + sync.
  - Feed, Squad and Profile: presence + status.
- Default-name prompt (`/^Student [A-Z0-9]{0,4}$/`) when the tab opens.
- Unread dot: set by every successful sync, cleared when the tab is opened. Production's quirk is kept: it also appears while Social is open.
- Message banner lasts 3.2 s. In FN its text has production's low contrast (`var(--bg)` on surface-2).
- Leaderboard limit 50; server date zone Europe/Zurich.

## 22a.5 Deviations (deliberate, documented)

| Production | Native 22a | Why |
|---|---|---|
| Interval-based Skribbl countdown | Deadline-based; one single-shot timer per displayed second, only while drawing and visible | Accurate after stalls; no idle wakeups |
| Busy auto-submit retry loop | One auto-submit at expiry; a failed submit keeps the drawing and offers retry | No loop against a failing server |
| WebP upload | PNG upload (RGB, max compression, ≤ 1.5 MiB checked locally) | Decision record: PNG accepted by the Worker |
| Native colour picker | `#rrggbb` field | Slint has no colour picker |
| Vote replies applied in arrival order | Matched by sequence; older replies ignored | Race safety |
| Gallery card overflow from lazy images | Not replicated (cards keep their size) | Production layout bug |
| Lightbox backdrop blur | Dim only | No backdrop blur in Slint/femtovg |
| Account created by the Worker bootstrap on load | Created only on explicit confirm; local record only after bootstrap succeeds | D1 |
| Name prompt only on tab open | Also right after creating an account while the tab is open | Same rule, applied at the moment the default name appears |
| Duplicate refreshes sent on fast tab switching | Identical in-flight refreshes coalesced | Bounded queue under a slow server (§22a.9) |
| Photo avatar URL normaliser (HTTPS only) | Also accepts `http://` *loopback* URLs | Lets the local mock serve photos; the image policy still binds every URL to the configured origin, and production's origin is HTTPS, so nothing changes against production |
| Profile restore for a credential without a local record | Fetched via `/player-stats` self; on failure an error + "Try again" (and a retry when the tab is reopened) | Only reachable with dev/test credentials until Stage 24 |

**Leaderboard semantics:**
- Since Worker migration 0019, leaderboards count only verified minutes.
- 22a syncs stats but does not send verified sessions (D4, 22b).
- So native study time does not raise a user's rank until 22b. The UI is correct; the numbers are the server's.

## 22a.6 Security review

- **deviceSecret:**
  - Exposed only to build request bodies, the two GET query strings production requires (`/skribbl/theme`, `/skribbl/leaderboard`), the multipart upload, and the credential file.
  - Logs carry method + path (query stripped) + status + size + time only.
  - `Debug` is redacted, the value is zeroed on drop, and it never enters a Slint model.
  - A grep of all run logs, captures and STATS output found no synthetic secret. The only occurrences are the synthetic import fixtures, whose `social` section the importer withholds.
- **Credential file:** 0600, written atomically, verified after a UI-created account.
- **TLS:** rustls with webpki roots; no verifier overrides. Redirects off. Proxies from the environment only for non-loopback.
- **Image URLs:**
  - HTTPS on the configured origin under `/feed/image/`, `/profile/avatar/` or `/skribbl/drawing/`.
  - Size caps: avatar 512 KiB, drawing 2 MiB.
  - Signature + declared type checked; 4096 px / 64 MB decode limits.
  - Rejected cases are covered by tests: userinfo, other hosts, look-alike hosts, other ports, other paths, `file:`, plain HTTP for production.
- **Server data:**
  - Bounded parsers: response cap 2 MiB, row caps, string caps.
  - Display text strips control and bidi-override characters.
  - The Worker stores any bytes as an image, so the client validates every image it shows.
- **Isolation:**
  - Every manual and capture run was inside `unshare -rn` (loopback only).
  - Chromium ran with a resolver mapping every host except 127.0.0.1 to NOTFOUND.
  - Tests use the in-process mock.
  - No production endpoint was contacted.

## 22a.7 Offline, errors, retry (verified in the real UI against the mock)

| Situation | Result |
|---|---|
| No account, production endpoint, Social tab open | 0 requests, no worker thread, no origins (`net_worker=none net_origins=none`) |
| Account, server refusing connections | App starts normally. Profile restore shows "Your profile could not be loaded … Everything else works as usual." + Try again. Stage 19–21 surfaces unaffected |
| Sync failure | "Sync Issue" pill + server/offline message, local data kept |
| Failed bootstrap | Credential and record rolled back to NoIdentity (test) |
| Failed Skribbl submit | Drawing kept, retry offered (test) |
| Closing Skribbl mid-request | All requests cancelled; late replies ignored (test) |

## 22a.8 Visual parity (Linux, 1520×980, scale 1)

Production capture: scratch build of `desktop/` with `VITE_SOCIAL_API_URL` set to the mock, headless Chromium, frozen 2026-10-04 12:00 Zurich. Native capture: `social-capture-native.sh`, same mock seed. Mean absolute difference per channel:

| Screen | Diff | Screen | Diff |
|---|---|---|---|
| FN Friends | 1.97 | Wabi light Feed (attendance) | 0.93 |
| FN Leaderboard | 2.62 | Wabi light Friends | 1.73 |
| FN Profile | 0.98 | Wabi light Profile | 0.74 |
| FN Feed (22b notice) | 4.69 | Wabi light Standings | 0.74 |
| FN Squad (22b notice) | 7.43 | Wabi dark Feed / Friends / Profile / Standings | 1.10 / 1.91 / 0.84 / 0.82 |
| Skribbl FN dark intro / drawing | 2.42 / 1.51 | Skribbl Wabi light intro / drawing | 2.44 / 1.53 |
| Skribbl FN dark / light gallery | 5.78 / 4.20 | Skribbl Wabi light / dark gallery | 4.18 / 5.31 |

**What remains in those diffs:**
- Feed/Squad are intentionally different (22b notices).
- The gallery diffs are mostly production's card-overflow bug.
- Remaining: font metrics, and the FN score badge's rotated text, which looks the same as Stage 21 (not 22a).
- Wabi sidebar items are ~1 px/item tighter than Linux Chromium. This predates 22a: Stage 19–21 were verified on Windows.

**Also verified visually:** new-account confirmation, name prompt (FN + Wabi), banner, offline restore notice, NoIdentity in both styles, drawing with strokes/colour/size.

## 22a.9 Performance (release, Linux, Xwayland at scale 1, loopback netns)

| Metric | Stage 21 | Stage 22a |
|---|---|---|
| Stripped binary | 38,107,240 B | 46,635,240 B (+8.53 MB, +22.4%) |
| First frame, median of 7 (no account) | 99.4 ms | 99.7 ms |
| Idle RssAnon, no account (3 samples) | 25.06–25.10 MiB | 25.42–25.47 MiB (+0.35 MiB) |
| Threads idle, no account | 8 | 8 |
| Threads with an account | — | 9 (the one network worker) |
| Idle CPU (21 s window) | 0 ticks | 0 ticks (no account), 0–2 ticks (account, Social open) |
| Rendered frames on a static screen, 10 s windows | 0 | 0 (dashboard, FN Friends, Wabi Standings, NoIdentity) |

**Binary growth by symbol attribution (unstripped Stage 21 vs 22a):**
- Generated Slint code for the new screens: +2.36 MB, plus related generic code in `i_slint_core`/`core`/`alloc` (~+2.0 MB).
- Unattributed: +1.8 MB.
- Network/TLS: ~+1.2 MB (rustls 373 K, ring 114 K, ureq 112 K, http 57 K, ureq_proto 46 K, webpki 45 K).
- Image decoders: ~+0.36 MB (image_webp 291 K, zune_jpeg 41 K, image 28 K).
- App logic: ~+0.3 MB.

The D7 transport-only measurement before the UI work was +2.42 MB.

**Memory fix found by this measurement:**
- The Skribbl canvas (900×600 RGBA, 2.06 MiB) used to be allocated at startup.
- It is now created when drawing starts and dropped, with the window's copy, when the modal closes.
- That cut the no-account idle cost from +2.5 MiB to +0.35 MiB.

**Stress (release, mock latency 300 ms / 150 ms):**
- **Social:**
  - 320 rapid clicks across subtabs, scopes and periods.
  - 122 requests sent (plus 1 avatar image), 318 duplicates coalesced; queue depth max 8 of 32; 0 failed; 0 refused; everything drained to 0 pending.
  - Anon memory steady at ~28 MiB.
  - Before coalescing, the same run filled the queue and ~293 refreshes were refused.
- **Skribbl:**
  - 300 strokes × 40 moves, undo every 25, clear every 100, then submit.
  - Submit accepted, gallery loaded 7 thumbnails, 0 failed.
  - Peak anon 42.7 MiB while drawing (canvas + 14 undo snapshots + window copy + thumbnails).
- **Harness note:** an exhausted `STUDY_NATIVE_INPUT` script keeps requesting redraws (~17 fps). Stage 21's binary does the same. This is a diagnostics artifact, never active without that variable.

**Bug found by stress:**
- The Skribbl modal's Flickable became drag-interactive once the drawing view was taller than the modal.
- It then swallowed every mouse drag, so the canvas received no pointer events.
- It is now wheel-only (as in the browser) and drawing works.

## 22a.10 Regressions (Stage 19–21)

**Pixel-identical screens (Stage 21 vs 22a, mean diff 0.000):**
- FN dashboard: dark, light, Sakura (petals frozen).
- Wabi dashboard: light, dark.
- FN Timer.
- FN Break Room.
- Wabi Rest.
- Wordle, Travle, Durak (FN).
- Geodle (Wabi).

**Tests:**

| Suite | Result |
|---|---|
| `study-tracker-core` | 210 passed, 1 ignored |
| app | 324 passed, 1 ignored |

The app suite includes 15 Social and 11 Skribbl end-to-end tests against the mock, golden parsing, mock conformance, endpoint/credential/image-policy tests, and all Stage 15–21 tests.

## 22a.11 How to run against the mock (never production)

```
cargo build -p social-mock
unshare -rn bash -c 'ip link set lo up
  target/debug/social-mock --port 47811 --seed demo --write-credentials /tmp/st-dev &
  STUDY_NATIVE_DATA_DIR=/tmp/st-dev STUDY_NATIVE_SOCIAL_ENDPOINT=http://127.0.0.1:47811 \
    target/debug/study-tracker-native-prototype'
```

- Seeds: `demo`, `demo-submitted`, `self-only`, `empty`.
- Omit `--write-credentials` to start without an account.
- Diagnostics: `STUDY_NATIVE_FRAME_STATS=1` (net/social/skribbl fields), `STUDY_NATIVE_NET_AUDIT=1` (one `NET` line with contacted origins at exit), `STUDY_NATIVE_SOCIAL_SUBTAB`, `STUDY_NATIVE_SOCIAL_COMPETITIVE=1`.

## 22a.12 Concerns and open items

1. **Windows not verified:**
   - Windows has not run the 22a code at all.
   - Still to check there: rustls/ring on MSVC, `social-credentials.json` permissions (0600 is a Unix mode; Windows relies on the per-user profile ACL), fonts/metrics of the new screens, clipboard through the hidden TextInput, and visual parity against WebView2.
2. **Binary size +22%:**
   - Mostly generated UI code, not the network stack.
   - Acceptable for now, but worth watching as 22b adds Feed and Squads.
3. **Native study time does not move leaderboard ranks until 22b** (verified sessions); see §22a.5.
4. **Profile restore in 22a only learns name, avatar and tag:**
   - The server does not return the privacy toggles, so production defaults apply until changed.
   - This path is only reachable with dev/test credentials before Stage 24.
5. **Social pages use the existing pages' drag-scroll Flickables:**
   - This matches Stage 19–21, but differs from browser behaviour (wheel only).
   - Text selection by dragging inside those pages' inputs is therefore limited.
6. **22b and Stage 24 boundaries:**
   - 22b: Feed, Squads, badges, avatar editor, verified sessions, telemetry.
   - Stage 24: credential migration from the Tauri app.
