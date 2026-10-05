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

**Windows VM verification (2026-10-05, "STAGE 22a WINDOWS VERIFICATION" below): PASS WITH CONCERNS — WINDOWS FUNCTIONALLY VERIFIED.** The portable pass is `4b3a62b`; four Windows-found fixes (W22a-1…4) are left uncommitted on top for review. The Linux results below are unchanged.

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

---

# STAGE 22a WINDOWS VERIFICATION

Run 2026-10-04/05 on the Stage 20/21 VM. Labels as in Stage 20 §43 / Stage 21 §53: **A** = functional Windows result (closes the gate in a VM), **B** = VM diagnostic (software GL; regressions/leaks only), **C** = physical Windows required. The Linux sections above are unchanged.

**Verdict: PASS WITH CONCERNS — WINDOWS FUNCTIONALLY VERIFIED.**

## W.1 Checkpoint

Preflight before any edit: branch `main`; `HEAD` = `origin/main` = **`4b3a62bd3ee7376684d3b8facb37c30d51f1b86b`** ("native: implement stage 22a social network foundation and daily skribbl"); working tree clean; `git diff --check` clean; `git diff -- desktop` empty; `git diff faa48d7 -- cloudflare` empty. `4b3a62b` was not amended; nothing was committed or pushed.

## W.2 Environment

| | |
|---|---|
| Machine | QEMU Q35 VM, 8 vCPU, 8 GB (Stage 20/21 VM) |
| Windows | 11 Pro 25H2, build 26200.8037, timezone Pacific |
| Display | RDP session, Microsoft Remote Display Adapter 2512×1500 physical at **175 % / 168 DPI**. Live real-input runs at the real 175 %; parity captures pin `SLINT_SCALE_FACTOR=1` (or 1.25) and render in-process |
| GL | Mesa llvmpipe test shim (`opengl32.dll` + `libgallium_wgl.dll` beside the exe, `GALLIUM_DRIVER=llvmpipe`), untracked (`git ls-files "*.dll"` empty). Without `GALLIUM_DRIVER` both Stage 21 (`e717cd9`) and 22a exit silently right after the platform host starts: VM infrastructure, not a 22a regression. Every CPU/fps/startup/memory number is **B** |
| Toolchain | rustc/cargo 1.99.0 (x86_64-pc-windows-msvc), VS 2022 Build Tools, Node 24.19 |
| Production reference | scratch `vite build` of a copy of `desktop/` + `design/` + `PRIVACY.md` with `VITE_SOCIAL_API_URL=http://127.0.0.1:47811`, headless Edge 154 (WebView2 154's Chromium) |

## W.3 Build / tests before any change (A)

`cargo fmt --check` clean. `cargo check --workspace`: **1 warning** (`SkribblCanvas::reset` unused outside tests; Stage 21 Windows had 0). `cargo test --workspace`: **core 210 passed / 1 ignored, app 332 passed / 1 ignored, 0 failed**. App count = Linux 325 − 1 Unix-only (`the_file_is_owner_only`) + 9 Windows-only (3 `single_instance`, 4 `updater::http`, 2 `win_host`). The test build has 8 warnings: Stage 21's `region` plus seven 22a test-only items (unused `port`/`origin`/`theme`/`label`/`available`/`class`, two unused `Result`s in `net/worker.rs` tests), cross-platform and unchanged by this pass. `cargo build --release`: 1 warning (the same `reset`), **45,322,752 bytes**.

## W.4 TLS / dependencies (A)

- `cargo tree`: one crypto provider, `ring` 0.17.14 under `rustls` 0.23.45 / `rustls-webpki`; `webpki-roots`; **no** `openssl`, `native-tls`, `schannel` or `aws-lc`.
- `dumpbin /dependents`: the only imports added since Stage 21 are **`bcrypt.dll`** (ring/getrandom RNG) and **`ws2_32.dll`** (sockets), both Windows system DLLs. No new runtime DLL; VCRUNTIME140 as before.
- Release exe starts normally (with the VM GL shim); the transport worked against the loopback mock throughout. A TLS handshake against production was **not** made (by design); rustls/ring initialisation on MSVC is exercised by the `UreqTransport` construction in every Social run.

## W.5 Production-network isolation (A)

- Windows Firewall **outbound block** rules on the release/debug app exe, both mock exes, the Stage 21 exe and (during captures) `msedge.exe`; loopback is exempt from WFP filtering, so the mock still worked.
- WFP connection-failure auditing (event 5157) and the DNS-Client operational log enabled. A positive control (`curl.exe` against TEST-NET `192.0.2.1` and `st22a-control.invalid`) produced one 5157 and 14 DNS events, so the monitors work.
- Whole session: **0 blocked connection attempts from the app or the mock; 0 DNS lookups naming the production host by any process.** The 2,453 blocked attempts were all headless Edge's own background services (Microsoft/Akamai endpoints, mDNS, gateway), none to the Worker's Cloudflare addresses.
- Every Social run used `STUDY_NATIVE_SOCIAL_ENDPOINT=http://127.0.0.1:<port>`; `net_origins` only ever listed `127.0.0.1:<port>`.
- Disclosure: before the monitors were enabled, I ran one `Resolve-DnsName` of the production host from PowerShell (DNS only, no connection, not the app).

## W.6 NoIdentity hard gate (A) — PASS

Production configuration (no endpoint override), fresh profile, no credential file. Visited with real clicks: Dashboard, Timer, Social, Break Room, Daily Skribbl (locked card on an empty profile, then the open modal with an unlocked fixture: "Connect the Social tab once to enable Daily Skribbl…"). Result: `net_worker=none net_origins=none`, `social_requests=0`, no TCP endpoint owned by the process, 0 WFP-blocked attempts, 0 DNS events, no `social-credentials.json`, the `NET` line at exit `net_worker=none net_origins=none`. Thread count equal to Stage 21 at idle (25). (Rust thread names are not exposed through `GetThreadDescription` with this toolchain, so the worker was counted with the app's `net_threads` counter plus OS thread deltas.)

## W.7 Mock (A)

Built and run on Windows. For manual fault scenarios the mock CLI gained `--fault <path>=<500|malformed|drop|delay:ms>[*n]` and prints its request log (`REQ <method> <path>`, never queries or bodies). This is test infrastructure in `crates/social-mock/src/main.rs` and does not touch the app. Exercised: normal, latency (300/500 ms), offline (mock stopped), malformed (gallery), server error (bootstrap, submit). Loopback-only validation was not relaxed.

## W.8 Account creation through the real UI (A) — PASS

From NoIdentity against the mock: "Create a new Social account" opens a confirmation dialog; nothing is sent and no file exists before **Create account**; **Cancel** sends nothing. First attempt with `--fault /sync/v2=500`: one `/sync/v2`, no credential file, no `social` key in `store.json`, back to NoIdentity. Second attempt: credential written after the server accepted, the default-name prompt appears; a blank name is refused (0 requests); a typed name with Enter saves (`Zoë Ålvarez`, real keyboard). Restart restores the identity.

## W.9 Credential file / Windows ACL (A)

- Location: `%LOCALAPPDATA%\com.damcha.studytracker.native-shell\social-credentials.json` by default, or the test `STUDY_NATIVE_DATA_DIR`. Contents never printed.
- DACL (`icacls`), inherited from the per-user profile tree: **NT AUTHORITY\SYSTEM (F), BUILTIN\Administrators (F), the current user (F)**. No Users / Everyone / Authenticated Users. The owner is `BUILTIN\Administrators` only because the test shell was elevated (an unelevated launch makes the user the owner). The default location was checked by an equivalent new directory under `%LOCALAPPDATA%`.
- **Classification: PASS** (current user plus expected privileged principals). No ACL code was added.
- Caveat: the protection is purely inherited. A file in a shared directory (probe under `C:\`) inherits `BUILTIN\Users (RX)` and `Authenticated Users (M)`. Only the dev/test `STUDY_NATIVE_DATA_DIR` override can put it there. Stage 24 should still move the secret to DPAPI/Credential Manager or set an explicit protected DACL.
- Atomic write (temp + rename, no `.tmp` left); malformed files are reported and left alone; clear → NoIdentity; replace works (unit tests on Windows, plus the live runs).

## W.10 Secret redaction (A) — PASS

Searched 185 files (app logs, stdout/stderr, mock request logs, stores, probes, STATS) for the synthetic secret and for the 36-character secret of the account created through the UI: **0 hits**; no credential-bearing query strings (logs carry `GET /skribbl/theme -> 200 …`, image paths as `/skribbl/drawing/…`). The clipboard never received a secret. Only the synthetic fixture's `imported-backups` copies contain the synthetic secret (Stage 15 behaviour: the importer keeps a copy of the input backup; the `social` section is withheld from the store).

## W.11 Friends, clipboard, polling (A)

- **Real input:** typing `dann-2345` shows `DANN-2345` (per-keystroke upper-casing), Enter sends one `/friends/request`, "Friend request sent." banner, Pending row. Accept → `/friends/respond`, Incoming empties, Kenji joins Your Friends. Decline (fresh mock) → `/friends/respond`, gone. Friend cards show seen times and avatars (photo for Amélie, emoji for Bob). The player dialog opens with stats, "Seen …", FRIEND badge. It closes with × (production has no Escape handler here either).
- **Clipboard:** Copy invite link, the tag chip and the invite chip put exactly `https://damcha02.github.io/destudydracker/?invite=SYNT-2345` / `SYNT-2345` on the Windows clipboard (production's format); no secret.
- **Polling:** one `/friends/status/v2` every 120 s while Social is visible (≈125/245/365/485 s), no duplicate after 30 rapid Social↔Dashboard switches (exactly 1 poll in the next 130 s), **0 polls in 150 s after leaving**, 0 persistence writes, 0 frames between polls. Each Social open does production's one status refresh.
- **Concern:** an unchanged poll reply still renders 2 frames. `app_social::push` builds new `ModelRc`s each time, so Slint sees changed properties. Platform-independent, ≈2 frames per 2-minute poll while visible; recorded, not changed.

## W.12 Leaderboards / Profile (A)

Friends / World / Squad arenas × Daily / Weekly / Overall all fetch with real clicks. Ranking, ties (two 1h 35m entries in server order), current-user styling (blue rank, tag chip, "(You)" in Wabi). The private-profile notice appears in World after turning on Private profile; the Squad arena shows the 22b notice. Wabi competitive mode adds Standings and persists `preferences.wabiCircleCompetitive`. Profile: name edit and validation, default-name prompt, tag/invite copy, six stats, sync console ("Sync Arena", offline message), three toggles (each a `/sync/v2`), avatar letter. Avatar editor remains 22b.

## W.13 Image policy (A)

The Windows suite covers: allowed Worker/loopback URLs; rejected other hosts, look-alikes, userinfo, other ports, wrong prefixes, traversal (including `\..\`), query/fragment, `file:`, `javascript:`, `data:`, plain HTTP in production mode. Also: requests without credentials or query, 512 KiB avatar / 2 MiB drawing caps, type and signature checks, decompression-bomb limits, golden drawings. Live runs fetched only the mock's own `/skribbl/drawing/` and `/profile/avatar/` paths.

## W.14 Daily Skribbl with real Windows input (A)

- Break Room → Play → intro → Start Drawing → **real mouse drags** (the Linux drag-capture fix holds) → colour change → brush size → enclosed rectangle → **flood fill stays inside** → Undo (fill removed, pixel back to white) → Clear → more strokes → countdown → exactly **one** automatic `/skribbl/submit` at the deadline → gallery → votes → lightbox → yesterday's winner → close/reopen (server-authoritative "You already submitted…", gallery re-fetched).
- **Pointer geometry:** at 175 % the drawn rectangle lands exactly where the cursor went (screen 1100,600–1500,900 → client 1088,548–1488,848). The canvas keeps 900×600 at every size tested.
- **Harness note:** drags whose moves were made with `SetCursorPos` lost their last 1–2 steps (Windows synthesises those moves lazily, after the button-up), leaving corner gaps through which a fill leaks. With real input-queue moves (`SendInput`, as a physical mouse produces) the corners close. Not an app defect.
- **Timer:** deadline-based. Minimize 20 s → restore shows 2:40 (correct); tray-hide via `WM_CLOSE` → restore via the tray icon after 70 s shows 1:50 (correct). Tray-hidden for the whole 3 minutes: exactly one submit at the deadline, 0 frames while hidden. Restoring a tray-hidden window with a raw `ShowWindow` (bypassing the app's tray path) leaves the display stale: a harness path, not a user path.
- **Submission:** fast real double-click on Submit with `--fault /skribbl/submit=500` → **one** request, "The Social server had a problem. Try again later.", drawing kept. Retry → accepted, gallery loads. A malformed gallery reply is ignored silently (production does the same); Refresh recovers. 409, timeout, offline and cancel-on-close are covered by the Windows end-to-end tests.
- **Voting:** up → +1, change to down → −1, down again → neutral; a fast double-click sends two toggles and ends consistent (production: one optimistic toggle per click, no in-flight guard either).
- **No local persistence:** `store.json` before vs after a full Skribbl run differs only in Stage 20's `playedBreaks` log (one entry per Play click). No drawing, theme, countdown, gallery or vote state. The data dir holds nothing else.

## W.15 Offline restore (A) — PASS

With a local profile record: the app starts responsive (window in 773 ms) and shows the cached profile and the offline sync message. Credential without a local record and the mock down: "Your profile could not be loaded / You're offline or the Social server can't be reached. Try again when you're connected. Everything else in Study Tracker works as usual." + **Try again**, 0 frames static. With the mock back, Try again restores the profile (`/player-stats`, presence, sync, status).

## W.16 Navigation, worker lifecycle, single instance, tray (A)

- **Navigation stress** (500 ms server, subtabs/Break Room/Skribbl, 384 steps): 162 sent, 253 coalesced, max queue **7/32**, 0 failed, 0 refused, all drained; no crash.
- **Worker:** created lazily (none without an account), `net_threads=1` in every run, `NET … net_worker=none` at exit (stopped).
- **Single instance** (Social open, account, mock): 8-way race → **1 survivor**. The mock saw exactly one instance's startup (1 presence, 2 sync, 1 player-stats, 2 status), so there is no duplicate sync, poller or worker. 20 second launches → 0 failures, 1 process, median 1,016 ms. Store hash and credential hash unchanged (no second writer). Tray Quit exits, removes icon and message window, "event loop exited normally", 0 panics.
- **Tray/minimize:** Social and an active drawing, each with 100 minimize/restore + 100 tray hide/restore cycles: 1 process, threads back to baseline, memory flat (drawing: private WS 156.6–157.6 MB). Minimized and tray-hidden: **0 frames**. Network replies while hidden render nothing.

## W.17 Static rendering and Sakura (A for frames)

0 frames over 20–30 s windows (complete STATS intervals): NoIdentity (production config), FN Friends / Leaderboard / Profile, Wabi Standings, Skribbl intro, submitted gallery, offline profile error. An active drawing renders only the countdown (≈2 frames/s visible, 0 hidden). Sakura with Social or the Skribbl gallery: **22 petals, one clock** (starts/stops 1/0 visible, 1/1 after minimize), 0 frames and 0 ticks minimized; Social traffic adds no clocks. Without `STUDY_NATIVE_INPUT` throughout (the exhausted-script ~17 fps artifact appears only in the scripted stress runs).

## W.18 Memory / stress (B)

Private WS / Private Bytes (MB), llvmpipe holds render buffers in process memory:

| Point | Value |
|---|---|
| Idle no account, Stage 21 vs 22a (same session, 3 runs, scale 1) | 69.2–71.7 / 96.1–97.0 vs 68.4–70.9 / 93.9–95.9 (equal) |
| NoIdentity Social / FN Friends / Leaderboard / Profile (175 %) | 92.6 / 102.8 / 99.3 / 102.0 |
| Skribbl intro / submitted gallery / active canvas | 110.4 / 140.6 / 153–157 |
| 500 Social open/close (300 ms server) | 75–86 / 101–116, flat; 255 sent, 251 coalesced, max queue 2; writes 4 → 4 |
| 500 Skribbl open/close (submitted day) | private bytes plateau 143–147, no trend; 1,514 requests, 7 images fetched once (cache), max queue 7 |
| 320 strokes × 30 moves + fills/undos/clears | 111–120 / 142–151, flat |

No crash, panic or `[ERROR]`; no worker leak; queues drained; image cache bounded (7 entries); canvas freed on close; no persistence churn beyond the Stage 20 Play log. 100 offline/reconnect cycles are covered deterministically by the controller tests (`offline_timeout_and_server_errors_fail_gracefully_without_retry_loops`, `many_open_close_cycles_keep_everything_bounded`, `a_failed_profile_restore_says_so_and_try_again_recovers`) plus the live offline runs above.

## W.19 Startup (B)

`STUDY_NATIVE_STARTUP_REPORT` (`FIRST_FRAME`), 12 launches per cell, Stage 21 (`e717cd9`, built on this VM) and 22a interleaved in alternating order:

| Scenario | Stage 21 median (min–max) | Stage 22a median (min–max) |
|---|---|---|
| fresh profile, no account (production config) | 580 (565–595) | 584 (573–598) |
| stored profile, no account | 650 (636–658) | 645 (630–657) |
| stored profile + synthetic account (mock) | — | 644 (632–663) |

No regression; NoIdentity and an account both start without waiting on the network.

## W.20 Binary size (A for size, attribution indicative)

Stage 21 `e717cd9` on this VM: 34,832,896 B. Stage 22a `4b3a62b`: **45,322,752 B (+10,489,856, +30.1 %)**. With the W22a fixes: **45,388,288 B**. Sections: `.text` 18.33 → 25.52 MB, `.rdata` 13.68 → 16.04 MB, `.pdata` 1.09 → 1.57 MB. Linker-map attribution (`.text`+`.rdata`, by symbol crate, +10.0 MB attributed):

- the app crate including generated Slint code: +4.70 MB, plus generic instantiations attributed to `core`/`i_slint_core`/`alloc`/`std`: +2.61 MB, so **≈7.3 MB UI-driven**;
- network/TLS: rustls +0.54, ureq +0.16, ring +0.12, http +0.08, ureq_proto +0.06, webpki +0.05, so **≈1.0 MB**, plus part of the +0.89 MB non-Rust bucket (ring assembly/tables);
- image decoders: image_webp +0.30, image +0.05, zune_jpeg +0.04;
- serde_json +0.23 MB.

Same shape as Linux; no duplicated crate or accidental dependency. **Stage 24 optimization candidate**, not changed here.

## W.21 Windows visual parity (production WebView2/Edge vs native)

`social-pair.ps1` (scratch; Windows twin of `social-capture-{prod,native}.sh`): one fresh mock per side (frozen 2026-10-04 12:00 Zurich), production in headless Edge with `--tz America/Los_Angeles` (the VM's zone, so local times agree), native in-process snapshot, 1520×980 unless noted. Mean absolute difference per channel /255, **after W22a-2/3/4**:

| Screen | Windows | Linux | Class |
|---|---|---|---|
| FN dark Friends (before: 3.13) | 2.89 | 1.97 | CLOSE |
| FN light Friends | 3.22 | — | CLOSE |
| FN dark Leaderboard | 4.26 | 2.62 | CLOSE (residual: head +2 px glyph placement; CJK/emoji row 44 vs Chromium's 48 px) |
| FN dark Profile (before: 3.53) | 2.91 | 0.98 | CLOSE |
| FN dark Skribbl intro (before: 3.18) | 3.04 | 2.42 | CLOSE |
| FN dark Skribbl drawing | 2.37 | 1.51 | CLOSE |
| FN dark Skribbl gallery (before W22a-2: **10.35**) | 6.02 | 5.78 | CLOSE (production card-overflow bug) |
| FN light Skribbl gallery | 4.19 | 4.20 | CLOSE |
| Wabi light Friends / dark Friends | 1.78 / 2.09 | 1.73 / 1.91 | MATCH / CLOSE |
| Wabi light Profile / dark Profile | 0.61 / 0.75 | 0.74 / 0.84 | MATCH |
| Wabi light Standings | 0.69 | 0.74 | MATCH |
| Wabi light Skribbl intro / drawing | 2.87 / 1.73 | 2.44 / 1.53 | CLOSE / MATCH |
| Wabi dark Skribbl gallery | 5.56 | 5.31 | CLOSE (production overflow bug) |
| FN dark Profile / Leaderboard / gallery at **1.25 (emulated)** | 3.74 / 4.95 / 7.32 | — | CLOSE, no clipping |
| FN dark Friends / gallery at **1100×760** | 3.82 / 7.79 | — | CLOSE, no clipping |
| FN NoIdentity | 7.55 | — | DEFERRED (deliberate D1: production auto-creates an account) |
| FN Friends, `empty` seed | 8.79 | — | not comparable (pre-seeded native credential without a server profile shows the 22a restore error) |
| Feed / Squad | — | — | DEFERRED (22b notices) |

**Typography and glyphs:**
- On Windows, Arial at weight ≥ 800 resolves to Arial Black in both engines (W21-1). That made the 22a heavy-label line boxes short; fixed in W22a-3.
- Arial, Consolas and Georgia lack `⧉ ↯ ◆ ★ ↻`; Chromium draws them from Segoe UI Symbol, Slint's fallback did not (tofu or a different face); fixed in W22a-3.
- Simplified Chinese stays tofu (concern 1). Japanese kanji, Hebrew, accented Latin and emoji render.

**Deliberate differences re-checked** (still acceptable, none hides a larger defect): the deadline timer (correct after hide/restore), PNG upload (normal drawings ≈5–20 KB), the hex colour field, the lightbox without blur, no gallery overflow.

## W.22 DPI / resize (A)

Real 175 % for all live input (dialogs, modal, canvas, gallery, chips: no clipping, hit boxes on the drawn controls, canvas mapping exact). Emulated 1.25 and the compact 1100×760 viewport: no clipping or overflow, modal scrolls as in production. **Real 125 % → Stage 24 (C).**

## W.23 Accessibility (A for the tree)

UIA tree inspected:
- Social subtabs are `TabItem`s (the Friends tab reads "Friends, 1 incoming friend requests").
- Accept/Decline/Send/Copy invite link, "Edit player name", the tag/invite chips, the three toggles and "Sync Arena" are named buttons.
- Leaderboard arena/period chips are tabs.
- Skribbl: dialog group "Daily Skribbl"; Brush/Fill, "Brush size N", Undo, Clear, "Custom color", "Close Daily Skribbl", Submit drawing.
- The canvas is **one** `Image` node, "Drawing canvas for today's theme: … 2:58 left." (no per-stroke nodes). Gallery cards are groups "Drawing by X, score N" with Upvote/Downvote.

Difference: palette swatches read "Color 0…20" where production says "Color #e53935" (platform-independent wording, noted). **TREE INSPECTED — NARRATOR SPEECH NOT VERIFIED (Stage 24).**

## W.24 Unicode (A)

- Accented Latin (Amélie, Zoë, typed `Zoë Ålvarez`), Japanese kanji (健二), Hebrew (שלום עולם) and emoji (🦊) render in Friends, Profile, Leaderboard, Standings and the gallery.
- The long seed name ("A very long display name that goes on and on and") fits its leaderboard column with no overlap.
- The Linux RTL fix holds: no spill outside the gallery card. Native's name area is slightly narrower, so the last Hebrew letter is clipped where production just fits (CLOSE).
- **Simplified Chinese (张伟) renders as tofu** (concern 1).

## W.25 Regressions (A)

- **Stage 21:** all Travle tests pass. Travle parity FN dark mid **1.784** (Stage 21 after W21-1: 1.79), Wabi dark mid **1.377** (1.38), so W21-1 is intact. Real-input smoke: typed `afgh`, dropdown pick, STEP ("4/7 countries guessed"), zoom in/out, close, guess persisted. Travle static 0 frames, no writes.
- **Stage 20:** Break Room, Durak, Wordle, Flaggle, Geodle, Travle, Wabi Rest games/album/meditation: **0 frames, no store write, 22 petals, no network worker**. Daily Skribbl is now the real implementation. All Stage 20 tests pass.
- **Stage 19:** FN dashboard/timer 0 frames; Wabi; Sakura 22 petals, one clock, stops when minimized (W.17).
- **Stage 18:** `git diff e717cd9 -- native-prototype/src/platform` empty. Updater, single-instance and notification code untouched; their tests pass; tray Quit verified (W.16). No updater/installer work.

## W.26 Bugs found and fixed (uncommitted)

| Id | Class | Symptom → root cause | Fix | Verification |
|---|---|---|---|---|
| **W22a-1** | build hygiene (portable) | `cargo check` on Windows: 1 warning where Stage 21 had 0. `SkribblCanvas::reset` is only used by tests | `#[cfg_attr(not(test), allow(dead_code))]`, as on its neighbour `undo_bytes` | `cargo check --workspace` and `cargo build --release`: 0 warnings |
| **W22a-2** | rendering (found on Windows) | Skribbl gallery thumbnails blank: 0–3 of the 6 friends' drawings visible at random (FN dark gallery diff 10.35); the lightbox and the header showed the same drawings. Instrumented: rows had `state=1` and correct opaque 480×320 pixels. Reproduced under Mesa llvmpipe **and** softpipe, independent of `image-fit` and `clip`. After close/reopen (cards created with their pictures) every thumbnail showed. **Cause:** a card whose picture arrived after its first paint was updated in place (`set_row_data`), and femtovg kept painting nothing for that `Image` item | `app_skribbl::sync_gallery`: a row whose picture changed (state or image) is removed and re-inserted, so its card is created with the picture; other changes (votes, scores) still update in place | 4/4 cold snapshots show all 7 thumbnails (before: 0/2 runs); real flow after submit shows all; FN gallery 10.35 → 6.02 (Linux 5.78). No GL harness exists to unit-test the renderer, so the captures are the regression check (as W20-2/W21-1) |
| **W22a-3** | Windows metrics / glyph fallback (portable formulas) | (a) tofu `⧉` copy chips, tofu `↯`, other faces for `◆ ★`, a box for `↻`: Arial/Consolas lack them and Slint's fallback does not pick Segoe UI Symbol as Chromium does. (b) Heavy-weight line boxes resolve to Arial Black on Windows (as W21-1): Profile mini-stat tiles 48.4 px in WebView2 vs native's fixed 46 (−4.8 px per two rows); Skribbl head 60 vs 56 px; leaderboard chips 39 vs 35 px. Georgia has no line gap: leaderboard head 78 vs 79 px and rows 44 vs 45 px. A CJK name grew `PersonRow` (preferred height of the fallback run) where Chromium's card stays at 59 px | (a) `Emoji.symbol` (Segoe UI Symbol on Windows, empty on Linux) for the copy chip, the coloured stat icons and the gallery refresh glyph. (b) Heights from the resolved faces through Stage 21's `CssLine.normal`: Profile `stat-h` (with a 46 px floor), Skribbl head (kicker + 4 + heading), `Chip` (20 + label), leaderboard head (52 + title) and subtitle, rows (27 + name), `PersonRow` (21 + max(38, name + 3 + detail)). With Liberation faces the formulas give the old constants (46, 56, 35, 79, 45), so Linux layout is unchanged by construction (`PersonRow`: 59 instead of 59.7) | Glyphs match production at 1.0/1.25; Profile 3.53 → 2.91, Skribbl intro 3.18 → 3.04, Friends 3.13 → 2.89; probed boxes (tiles 48.4, chips 39, head 60, rows 44) now equal production |
| **W22a-4** | parity (portable, found on Windows) | Friends: "Pending" and "Your Friends" sections 4 px too close to the card above. Production gives every `.arena-social-section h4` `margin: 4px 0 0`; native applied it only to Incoming | `padding-top: 4px` on the Pending and Your Friends sections | Friends shift profile: section titles back on production's rows |

Considered and **not** changed: simplified-Chinese fallback (concern 1), unchanged-reply redraw (concern 2), credential ACL hardening (W.9), swatch labels (W.23), 1-glyph RTL clipping in gallery cards (W.24).

## W.27 Security review

No production contact; loopback-only endpoint validation unchanged; credentials bound to the endpoint class (unit tests on Windows); test builds refuse non-loopback (transport guard). The secret appears in no log/output/clipboard. The credential DACL is current user + SYSTEM + Administrators by inheritance (W.9 caveat). The image policy is unchanged and verified. TLS has no verifier override and no OpenSSL/native runtime. Fixes W22a-1…4 touch only rendering, layout and a dead-code attribute; the mock change is test infrastructure.

## W.28 Concerns

1. ~~Simplified Chinese renders as tofu~~ → **fixed in W22a-5** (W.31).
2. ~~An unchanged Social reply repaints~~ → **fixed in W22a-6** (W.31).
3. **Credential file relies on the inherited profile ACL**; the dev override into a shared folder would expose it. Stage 24: DPAPI/Credential Manager or explicit DACL.
4. **Binary +30.1 % on Windows** (+22.4 % Linux), UI-dominated (W.20). Stage 24 candidate.
5. Residual parity: CJK/emoji leaderboard rows not growing like Chromium's; RTL last glyph clipped in gallery cards (Slint text-engine limitation, W.31). Leaderboard head (W22a-8) and swatch labels (W22a-7) are fixed.
6. The Mesa test shim needs `GALLIUM_DRIVER=llvmpipe` on this VM (Stage 21 identical).
7. `imported-backups/` keeps a byte-for-byte copy of an imported production backup including its `social.deviceSecret` (unchanged Stage 15 behaviour, per-user ACL; audit in W.31, Stage 24 item).

## W.29 Stage 24 physical gates (C)

Physical GPU (FemtoVG/NVIDIA) CPU and FPS, high refresh, authoritative startup and memory, real 125 % and multi-monitor DPI, Narrator speech, Japanese/Chinese IME and CJK rendering on localized Windows, sleep/resume with an active drawing, TLS handshake against the real Worker (with real credentials only in the migration stage), credential store migration, installer/updater.

## W.30 Final checks (Windows, after W22a-1…4)

`cargo fmt --check` clean; `cargo check --workspace` **0 warnings**; `cargo test --workspace` **core 210 passed / 1 ignored, app 332 passed / 1 ignored, 0 failed** (test build: the same 8 warnings as the checkpoint); `cargo test -p study-tracker-core` 210 / 1 ignored; `cargo build --release` 0 warnings, **45,388,288 bytes**; `git diff --check` clean; `git diff -- desktop` empty; `git diff faa48d7 -- cloudflare` empty; `git ls-files "*.dll"` empty.

Files changed (uncommitted): `src/app_skribbl.rs` (W22a-2), `src/skribbl_canvas.rs` (W22a-1), `ui/break/skribbl.slint` (W22a-3), `ui/social/fn-social.slint` (W22a-3/4), `crates/social-mock/src/main.rs` (mock `--fault` and request log), and this document. Verification scripts (pair capture, lifecycle, stress, startup, static matrix, map attribution) stayed in a scratch directory.

No real user data or credentials: synthetic fixtures and identities, throw-away profiles, no installed Study Tracker profile on the VM. No Cloudflare deploy, no migration.

## W.31 Follow-up: Windows parity / render-discipline cleanup (2026-10-05)

Requested after the first review; same VM, same isolation (firewall blocks on app/mock/Edge, WFP audit, DNS log), synthetic data only.

### Fixed

| Id | Finding | Root cause | Fix | Verification |
|---|---|---|---|---|
| **W22a-5** | Simplified-Chinese names drew as empty boxes (English Windows 11) | Production (Edge 154, probed with `CSS.getPlatformFontsForNode` on cloned production text nodes) resolves Han → **Microsoft YaHei** (Simplified, Traditional, and the kanji of Japanese names), kana → **Yu Gothic**, Hangul → **Malgun Gothic**, emoji → Segoe UI Emoji, Latin → Arial/Georgia. Native fontique asked DirectWrite for one family per script without a locale and got a Japanese face for Han (it has 健二 but not 张伟) | `src/font_fallback.rs`: right after the window is created (before the first layout), the per-script fallbacks of Slint's process-wide collection (`slint::fontique_010::shared_collection()`, Slint's documented hook; feature `unstable-fontique-010` enabled for Windows targets only) are set to installed system families in Chromium's order: Hani YaHei → JhengHei → SimSun → Yu Gothic → Malgun Gothic; Hira/Kana Yu Gothic → MS Gothic → YaHei; Hang Malgun Gothic; Bopo JhengHei → YaHei. Missing families are skipped (Meiryo, Gulim are not installed here). Nothing bundled or registered from disk; named families (Arial, Georgia, Segoe UI Symbol/Emoji) untouched; Linux compiles a no-op and does not enable the feature | A typed name `张伟 張偉 한국 ひらカ 健二 Zoë 🦊` renders every script in the sans field; 张伟 renders in Friends, the serif Leaderboard and the gallery ("张伟 Z…" elides normally). Latin screens' parity numbers unchanged; Segoe UI Symbol glyphs unchanged. **Cost:** binary +4,096 B; FIRST_FRAME fresh 592 → 588 ms, stored 656 → 655 ms (10 interleaved launches each); idle private bytes 94.2–96.0 → 93.9–97.0 MB, with CJK on screen 102.7–105.8 → 103.1–104.4 MB (no change beyond noise); `Cargo.lock` unchanged |
| **W22a-6** | An unchanged 2-minute friend-status poll rendered 2 frames | Traced HTTP reply → controller → `app_social::push` → `set_social`. Two causes: (1) `build_view` makes new `ModelRc`s, compared by identity; (2) Slint's `Image` equality is **false for two empty images** (`ImageInner` falls through to `_ => false`), so any view holding an avatar without a photo (rows, the profile avatar, the dialog avatar, Wabi attendance rows built with `Default::default()`) never compared equal. Found with a temporary debug log of the old/new view: writes with no printed difference still compared unequal | Avatars without a photo use one shared 1×1 transparent placeholder (`no_photo()`, never drawn: `has-photo` is false). `keep_unchanged_models` puts the window's current model back into every list whose rows are equal, then `push` skips `set_social` when the whole view is equal | Mock run: unchanged poll **0 frames** (twice; before: 2); swapping the mock to a different world makes the next poll repaint (2 frames) and the friend list update; persistence writes only on the change. Tests: `empty_images_never_compare_equal_so_avatars_use_one_placeholder`, `an_unchanged_reply_keeps_the_models_and_compares_equal`, `a_changed_reply_still_updates` |
| **W22a-7** | Swatches read "Color 3" | — | Production's `aria-label={`Color ${paletteColor}`}` ported: a parallel list of production's `PALETTE` strings in `SkribblPalette.names`; `accessible-checked` already mirrors `aria-pressed`. No layout change | UIA: 21 swatches `Color #000000` … `Color #e53935` … `Color #ffffff` |
| **W22a-8** | Leaderboard head ≈2 px low | Production's FN `article.arena-leaderboard` computes `border-width: 0 1px 0 0` (right side only) with padding 28/34/40; native drew a 1 px border on all four sides and inset the content 35/29 px, so content sat 1 px right and down and an extra top/left edge was drawn | Right-edge line only; content at (34, 28), width −69 px, height 28 + content + 40. Platform-independent CSS, so Linux benefits too | FN Leaderboard parity 4.26 → **3.48**; head content within ≤ 1 px (the remaining 1 px is the hero rule above, production's at a fractional 213.6 px: rounding) |

### Investigated, not changed

- **Hebrew in gallery cards (CLOSE, text-engine limitation):** the native name box is the right size (the card's width minus the 105.8 px vote group, the same space production has before its card-overflow bug pushes the votes out). LTR names elide correctly in it ("张伟 Z…", "Kenji …"). Slint 1.17's `sharedparley` elision works in visual x from the left edge; an RTL line that overflows extends past the *left* edge instead, so no elision fires and the box clip cuts the last letter. A generic fix needs RTL-aware elision in Slint; widening the box or special-casing scripts would only move the problem. Latin, CJK and long names were rechecked; no Arabic fixture exists in the seeds (same RTL path). Stage 24 / toolkit item.
- **Imported-backup secret (audit, no redesign):**
  1. Production's `buildBackup(state)` exports the whole `state`, including `social` with `deviceSecret`, so a user-saved production backup contains it.
  2. It is the original production backup the user selects (`STUDY_NATIVE_IMPORT_BACKUP`, an explicit diagnostic/user action).
  3. Native does **not** use the Social credential from it: the importer classifies `social` as *Withheld*; `identity_from_production_backup` is called only by tests.
  4. Stage 15's `discover_and_read` copies it with `read_to_string` → `fs::write`, byte-for-byte for any valid UTF-8 backup (synthetic check: SHA-256 equal to the source fixture).
  5. So the copy keeps `deviceSecret` (field present in the synthetic copy).
  6. Location: `%LOCALAPPDATA%\com.damcha.studytracker.native-shell\imported-backups\<stem>-<unix ms>.json`.
  7. DACL inherited: SYSTEM (F), Administrators (F), current user (F).
  8. No Users / Everyone / Authenticated Users; this VM has no other enabled local user, and the DACL grants none.
  9. Unchanged Stage 15 behaviour (`62ff25f`); 22a changed neither `migration.rs` nor the import path in `main.rs`.

  **Stage 24 migration/security item:** strip or encrypt the `social` section in the kept copy, or protect it like the credential store.

### Regression after the follow-up

`cargo fmt --check` clean; `cargo check --workspace` 0 warnings; `cargo test --workspace` **core 210 / 1 ignored, app 335 / 1 ignored, 0 failed** (+3 tests; test build: the same 8 warnings); `cargo test -p study-tracker-core` 210 / 1 ignored; `cargo build --release` 0 warnings, **45,470,208 B**; `git diff --check` clean; `desktop/` and `cloudflare/` (vs `faa48d7`) unchanged; `Cargo.lock` unchanged.

Parity after the follow-up: FN Friends 2.82, Leaderboard 3.48, Profile 2.91, Skribbl intro 3.04, gallery 6.03 / 4.19 (all thumbnails), Wabi Friends 1.77 / 2.08, Profile 0.61 / 0.75, Skribbl 2.87 / 5.56; Travle FN mid 1.784. Static matrix again 0 frames everywhere static; Sakura 22 petals, one clock, 0 when minimized. No production contact.

Files changed by the follow-up: `src/font_fallback.rs` (new), `src/main.rs`, `Cargo.toml` (Windows-target feature), `src/app_social.rs`, `ui/break/skribbl.slint`, `ui/social/fn-social.slint`, this document.

**NOT COMMITTED. NOT PUSHED. Stage 22b NOT STARTED.**

---

# STAGE 22b IMPLEMENTATION

Linux implementation pass, 2026-10-05. Uncommitted. Starting checkpoint `fd48867` (Windows-verified
Stage 22a, = `origin/main` at the start). Verdict for Linux: **PASS WITH CONCERNS — PENDING WINDOWS
VERIFICATION** (§22b.21).

## 22b.1 Freshness

- Upstream advanced to `00fdcd0` (v0.1.68) during the stage: planner/exam CSS, `ManageSemestersModal`,
  release notes and a planner `useEffect`. None of it touches Social, the Worker or `cloudflare/`
  (unchanged). Classified unrelated; no Social behaviour had to be re-read.
- Production references read: `desktop/src/App.tsx`, `desktop/src/features/social/SocialScreen.tsx`,
  `desktop/src/App.css`, `cloudflare/src/index.ts` (read only).

## 22b.2 Complete Stage 22 protocol

Every Worker route is accounted for. Routes the production client never calls are not implemented.

| Route | Client | Stage |
|---|---|---|
| `/sync/v2` (+ `feedPosts`), `/presence`, `/friends/status/v2`, `/friends/request`, `/friends/respond`, `/leaderboard`, `/player-stats`, `/profile/avatar/<key>` (GET) | yes | 22a (sync body extended in 22b) |
| `/skribbl/theme`, `/gallery`, `/submit`, `/vote`, `/leaderboard`, `/skribbl/drawing/<key>` (GET) | yes | 22a |
| `/feed`, `/feed/react`, `/feed/poll/vote`, `/feed/comment`, `/feed/update`, `/feed/delete`, `/feed/image` (upload), `/feed/image/delete`, `/feed/image/<key>` (GET) | yes | 22b |
| `/profile/avatar` (upload) | yes | 22b |
| `/squads/create`, `/search`, `/details`, `/join`, `/respond`, `/leave`, `/chat`, `/chat/delete`, `/promote`, `/kick`, `/settings`, `/scoreboard` | yes | 22b |
| `/verified-session/start`, `/heartbeat`, `/finish`, `/reconcile-offline` | yes | 22b |
| `/announcements/current`, `/announcements/update-notice` (owner) | yes | 22b |
| `/telemetry/heartbeat` | yes | 22b |
| `/admin/usage` (owner) | yes | 22b |
| `/sync`, `/friends/status` (v1), `/squads/status`, `/squads/demote`, `/health`, `/admin/migrate-device-secrets`, `/admin/migrate-profile-avatars` | **no** (0 references in `desktop/src`) | not implemented |

All 22b requests carry `{ userId, deviceSecret }` in the JSON body exactly as production does
(telemetry and `GET /announcements/current` carry no identity). Post creation goes through
`/sync/v2` `feedPosts` (outbox ≤ 25); queued deletions are sent after a successful sync.

## 22b.3 Architecture

Unchanged from 22a: **core → controller → application integration → one network worker**.

- `study-tracker-core/src/social/`: `feed.rs`, `squad.rs`, `verified.rs`, `telemetry.rs`,
  `announcement.rs` (pure; no I/O, no clock reads), `ids.rs` (new ids, UTF-16 truncation,
  `display_paragraph`: controls and bidi overrides stripped), `avatar.rs` (`crop` geometry).
- `src/net/social_ext.rs`: every 22b request builder and bounded parser (`Lenient`, `Rows`,
  `BoundedMap`); `images.rs` gains `ImageKind::FeedImage` (6 MB cap, GIF sniffing).
- Controller split: `social_controller_{feed,squad,avatar,background}.rs`.
- Application: `app_social.rs` (+ `app_social_view.rs`), `image_prep.rs` (decode/resize/encode off
  the UI thread through `app_net::submit_local`, i.e. on the same single worker), `file_picker.rs`
  (XDG portal over the already-linked zbus, zenity/kdialog fallback; Windows `IFileOpenDialog`),
  `backdrop.rs` (blurred backdrops).
- UI: `ui/social/{fn-parts,fn-feed,fn-squad,overlays,wabi-feed,wabi-squad,wabi-avatar}.slint`,
  `ui/backdrop.slint`; `SocialActions`/`SocialData` globals.
- Still one network worker ("social-net", bounded queues API 32 / images 48); still one Slint
  timer per schedule (§22b.12).

## 22b.4 Feed, posts, polls, comments, reactions

- Feed scopes Friends/Global (`/feed`, limit 40), refreshed on tab open, on scope change, after a
  sync and every 2 minutes while the Feed subtab is visible. Rows live in one persistent
  `VecModel`: unchanged replies write nothing; a row whose picture changes is re-inserted.
- Posting the latest session (`latestFeedSession` = first study/exam session, newest first), with an
  optional note (≤ 220 UTF-16), poll (question ≤ 180, 2–12 distinct options ≤ 100) and image. Without
  an image the post waits in the outbox for the next sync ("Post queued. Sync to publish it to the
  feed."); with an image a sync runs, then the upload. Already-posted sessions are refused.
  Auto-post queues completed sessions only when enabled. The fallback note is production's
  `pickFeedFallbackNote` (code-unit sum mod 28).
- Polls: optimistic vote, per-post sequence numbers so a stale reply never overwrites a newer vote,
  rollback on failure, never retried.
- Comments: trimmed, ≤ 220 units; a new comment by someone else on an own post raises one toast
  ("New comment on your post.", excerpt 72 chars, View / X); seen ids persisted (≤ 1000).
- Reactions: Field Notebook shows fire/brain/clap plus used keys and the 36-emoji picker; Wabi shows
  only "Nod N" (fire). Tooltip = up to three names "+N more". Production quirk kept server-side in the
  mock: the Worker maps `brain` (5 code points) to `fire`.
- Edit (note, replace/remove image) and delete for own posts; the server enforces ownership (tested
  with a forged local state).

## 22b.5 Feed images and image cache

- Upload: picked file ≤ 6 MB, decoded and resized on the worker; opaque images become JPEG q82, images
  with alpha PNG, GIF uploaded as is (production encodes WebP: parity debt). Uploads pause when the
  owner's R2 status says so.
- Download: `/feed/image/<key>` only, through the 22a image policy (same origin as the endpoint,
  bounded size, sniffed format), decoded to fit 1024 px, LRU of 16 pictures. Failure → "Image could
  not load"; expiry → "Image expired". The lightbox shows the picture at its own size (≤ 94% × 90%).

## 22b.6 Squads, permissions, chat, Squad Arena

- No squad: create (name ≤ 48, public/private), search, four suggested squads (production's shuffle;
  pinned by `STUDY_NATIVE_BREAK_PICK` in captures), join/request, pending note.
- Production bug not reproduced: suggestions refetch in a loop when there are none or the request
  fails; native tries once per visit ("Reload" reshuffles the pool it has).
- In a squad: head with Edit (leader) / Leave, totals, roster (expandable member cards with
  production's `getAssignableSquadRoles`/`canKickSquadMember` rules), internal leaderboard, chat
  (≤ 500 units, own messages deletable), join requests for elders and up. Leaving as the last member
  asks first (`window.confirm` → native confirmation dialog). The Worker decides every permission;
  stale local permissions are refused server-side (tested).
- Squad Arena (Leaderboard → Squad Arena): Daily / Seasonal Points / Overall Points, season notice,
  rows open the squad details dialog (join/request from there). Scoreboard cached 60 s per period.

## 22b.7 Avatar editor, badges, remaining profile state

- Avatar editor: Letter (6 styles, letter picker), Icon, Photo. A photo opens the crop editor (300 px
  stage, drag, wheel, slider 1–4×); the result is a 160 px PNG (JPEG ladder down to ≤ 96 KB), uploaded
  to `/profile/avatar`, then synced.
- Badges: the Profile "Badges" plate opens the collection from the Break Room's evaluated
  achievements (one source of truth): Break Room + Pet Rock subgroup, Focus Fossil, Garden; `×N`
  counts for daily badges; hover/focus tooltip.

## 22b.8 Verified sessions

- Through an application adapter only: `SocialController::observe_timer` reads a read-only
  `AppModel::timer_state()` snapshot after timer activity; the Timer domain is not modified.
- Eligible = running and phase not idle/break. Start → heartbeat every 15 minutes → finish; "Verified
  session not found" restarts. Exactly one heartbeat schedule (production leaks an interval on
  restarts: not reproduced).
- Offline: gaps longer than the 2-hour grace window are reconciled once with
  `/verified-session/reconcile-offline` carrying the offline intervals (≤ 500) and `chainTipHash`
  (SHA-256 of canonical JSON). The anchor is persisted.

## 22b.9 Telemetry (production semantics)

- Setting "Anonymous usage telemetry", **default off**, opt-in.
- Payload `{ installId, app: { version, platform, runtimeChannel } }` to `/telemetry/heartbeat`:
  no user id, no deviceSecret (tested), no new fields or events.
- Sent at start-up when enabled, on enabling, and hourly; no identity needed, as in production. In
  test/dev builds it can only reach the loopback mock (verified: 1 request, origin `127.0.0.1:47811`,
  with and without an identity; 0 when disabled).
- Native exposes the setting in a Settings sheet (menu → Settings) holding only the Stage 22b items;
  the rest of production's Settings panel (backup, privacy, updates) is not part of the native app yet.

## 22b.10 Owner / admin

Only what the production client exposes, for the owner tag `ZRWL-WKNF`: the R2 usage banner on the
Feed, "Notify users below <version>" (`/announcements/update-notice`) and the usage report
(`/admin/usage`: summary tiles, synced users, flagged accounts, opt-in installs, abuse events, with
production's columns and "Never"/"unknown"/"—" fallbacks). The Worker enforces ownership (tested).
Announcements (`/announcements/current`, polled every 2 minutes with an identity) show as a toast;
dismissals are stored (newest 100).

## 22b.11 Persistence

- `SocialRecord` gains `pending_posts`, `pending_post_deletions`, `verified_anchor`, `squad`,
  `squad_incoming`, `squad_outgoing`, `own_post_ids` (≤ 100), `seen_comment_ids` (≤ 1000).
- `SocialPrefs` (`telemetry_enabled`, `install_id`, `dismissed_announcements`) in the preferences section.
- Not persisted: feed rows, chat, scores, images.
- Stage 16 import fix: the academic import reversed production's newest-first `sessions` array. The
  native Timer list then showed the oldest session first, and the Feed composer picked the wrong
  "latest session". Now kept in order: the Timer list matches production's capture (40 m Oct 4 first,
  then 52 m Sep 20; HEAD had them inverted). Only affects new imports; no real users before Stage 24.

## 22b.12 Background scheduler inventory

| Scheduler | Cadence | Active only when |
|---|---|---|
| Status poll (22a) | 2 min | identity and Social tab visible |
| Feed poll | 2 min | identity, Social tab visible, Feed subtab |
| Hourly sync (22a) | 60 min | identity |
| Verified heartbeat | 15 min | identity and an eligible running Timer session (one schedule, restarted by generation) |
| Telemetry | 60 min | user opted in and Social configured (no identity needed) |
| Announcements | 2 min | identity |
| Session debounce / startup / banner (22a) | single-shot | as 22a |

No per-component timers; nothing animates. Measured: `social_timers` 3–4 with an identity, **0** with
no identity; 0 ticks in every 30 s window between polls.

## 22b.13 Mock

`crates/social-mock/src/world_22b.rs` follows the Worker for every 22b route (validation, limits,
ownership, roles, the `brain`→`fire` quirk). `seed::add_22b` adds synthetic posts, a poll, an image,
comments, reactions, squads, chat, scores and the user Mia. CLI: `--squad none`, `--owner`,
`--announcement`, `--latency-ms`. Synthetic secret `TEST_SECRET_MUST_NOT_APPEAR`.

## 22b.14 Security

- Endpoint isolation unchanged: test/dev builds accept only loopback endpoints; production
  configuration with NoIdentity sends nothing.
- `scrub()` redacts the secret from controller messages and stored errors (a leak into
  `last_sync_error` was found and fixed). 0 hits of the synthetic secret in app stdout/stderr and mock
  logs across every run in this section.
- Server text is bounded and cleaned (`display_paragraph`: controls and bidi overrides stripped);
  maps/rows bounded (`BoundedMap`, `Rows`); images through the 22a image policy.
- Mutations (votes, reactions, comments, chat, role changes, uploads) are never retried
  automatically; votes use sequence numbers.
- The picked file path is never logged.

## 22b.15 Visual parity (Linux, 1520×980, scale 1, mean absolute difference /255)

Production built from `desktop/` with `VITE_SOCIAL_API_URL=http://127.0.0.1:47811`, captured in
headless Chromium in a loopback-only netns; native on a private Xwayland; same synthetic fixture and mock.

| Field Notebook (dark) | Diff | | Wabi (light) | Diff |
|---|---|---|---|---|
| Feed (with comment toast) | 1.55 | | Circle feed | 1.97 |
| Squad (in squad) | 1.41 | | Squad (in squad) | 1.88 |
| Squad (no squad) | 2.80 | | Squad (no squad) | 2.40 |
| Leaderboard → Squad Arena | 1.93 | | Avatar editor, badges | not compared (see debt) |
| Squad details dialog | 0.72 | | | |
| Badges | 2.35 | | | |
| Avatar editor | 2.34 | | | |
| Feed image lightbox | 0.17 | | | |

Production findings that the native port reproduces:

- Field Notebook leaves `--accent` undefined, so every `color-mix(... var(--accent) ...)` drops out.
  Borders fall back to `currentColor` and backgrounds to none. This explains the self/hover arena
  rows, the transparent badges and avatar-editor cards, and the bare `×N` badge counts.
- The Wabi attendance chips have no border, because `var(--wabi-rule-soft)` substitutes a whole
  shorthand. This was a Stage 22a parity bug, fixed here.

## 22b.16 Stage 22 parity-debt register

| # | Production | Native | Why / status |
|---|---|---|---|
| 1 | `backdrop-filter: blur(10px)` (lightboxes, dialogs) | **Restored**: one snapshot per opening, CPU box blur ×3 at ¼ size, shown under the dim | Slint has no backdrop filter; measured 0.17/255 on the lightbox |
| 2 | `<input type="color">` (Skribbl) | **Restored**: HSV square + hue strip + preview + `#rrggbb` field | Slint has no colour dialog |
| 3 | WebP uploads (feed, avatar, Skribbl) | JPEG q82 / PNG (alpha) / GIF as is; avatar PNG with JPEG ladder | Worker accepts these; no WebP encoder in the binary |
| 4 | HEIC accepted via the browser | PNG, JPEG, WebP, GIF only | No HEIC decoder |
| 5 | Dashed borders (`.arena-empty`, upload box) | Solid | Slint has no dashed borders |
| 6 | Locked badges `filter: grayscale(1)` | Opacity only | Slint cannot desaturate an emoji glyph |
| 7 | Animated live dot / toast slide-in | Static | Static-rendering rule (0 frames) |
| 8 | Verified heartbeat interval leaks on "not found" restarts | One schedule | Production bug |
| 9 | Squad suggestions refetch loop on empty/failed | Once per visit | Production bug |
| 10 | `window` `online` event triggers verified reconnect | Connectivity inferred from the next successful request | No such event natively |
| 11 | Worker maps reaction `brain` → `fire` | Reproduced (server side, mock) | Production quirk |
| 12 | Full Settings panel | Only the 22b items (telemetry, owner cards) | Backup/privacy/updates belong to a later stage |
| 13 | Wabi dialogs (avatar editor, badges, squad details) | Wabi colours on the shared dialog geometry; not compared to production captures | Remaining Wabi capture work |
| 14 | `.arena-mini-stat__icon` colour `#081018` (nearly invisible) | Muted ink (as 22a) | Kept 22a decision |
| 15 | CJK/emoji fallback line boxes in Wabi (taller rows) | Native fonts' own line height | Font metrics (same class as W22a-3) |
| 16 | 22a items (§22a.5): deadline countdown, single auto-submit, gallery overflow, account on confirm | unchanged | see §22a.5 |

## 22b.17 Static rendering and polls

`STUDY_NATIVE_FRAME_STATS=1`, release, loopback netns, 10 s windows after load: **0 frames, 0
ticks** on FN Feed, Squad, Squad Arena, Wabi Feed, Wabi Squad, with the badges dialog open (blurred)
and with the squad details open; NoIdentity (production configuration): 0 frames, 0 requests, 0
Social timers, no origins.

Unchanged polls:

- Squad tab, 2-minute status poll: **0 frames**. Before the fix in §22b.20 it was 4.
- Feed tab, 2-minute feed poll: **2 frames**. That is production's own "· Refreshing" label
  switching on and off (`setFeedLoading` around every poll). Unchanged data writes nothing else.

## 22b.18 Performance, size, stress (release, Linux, Xwayland scale 1)

| Metric | Stage 22a (HEAD) | Stage 22b |
|---|---|---|
| Stripped binary | 46,635,240 B (22a doc) | 57,885,288 B (+11.25 MB, +24%) |
| First frame, median of 7, no account | 95.2 ms | 94.6 ms |
| Idle RssAnon, no account | 25,136 kB | 25,592 kB (+0.45 MiB) |
| Threads idle / with account | 8 / 9 | 8 / 9 |
| RssAnon with account, FN Friends | 26,968 kB | 27,456 kB |
| RssAnon with account, FN Feed | 26,192 kB (placeholder) | 30,516 kB (posts, decoded image, avatars) |

New dependencies: `ring` (already in the tree through rustls; now used directly for SHA-256),
`image` `gif` feature, `zbus` blocking API (already linked through accesskit) and Windows shell
features of the existing `windows` crate. Most of the growth is generated Slint code for the new
screens, as in 22a.

Stress (mock latency 300 ms): 262 scripted clicks on reactions, votes, comments, scope and subtab
switches: 242 requests + 2 images, 58 coalesced, queue depth max 4/32, 0 failed, 0 refused, drained to
0 pending, 0 secret hits. (Synthetic clicks pace the `STUDY_NATIVE_INPUT` script to ~1 step/s; the
Stage 22a binary does the same.)

## 22b.19 Connection audit

| Scenario | Origins contacted |
|---|---|
| NoIdentity, production configuration (Social, Timer running) | none (0 Social requests) |
| Existing synthetic identity (all surfaces, stress) | `127.0.0.1:47811` |
| Verified session (Timer start) | `127.0.0.1:47811` (start = 1) |
| Telemetry enabled, with / without identity | `127.0.0.1:47811` (1 request each); disabled: 0 |
| Feed images / avatars | `127.0.0.1:47811` |
| Production Worker | never |

## 22b.20 Bugs found and fixed in this pass

- The secret could reach a stored `last_sync_error`; `scrub()` now redacts it.
- Profile restore did not run the Feed effects (no posts until a tab change).
- Stage 16 import reversed session order (Timer list and latest session wrong).
- Slint compares two empty `Image`s as unequal, so the Feed and dialog views were rewritten on every
  push, and unchanged polls redrew (4 frames). Unset pictures now share one placeholder.
- A zero-height overlay container was never repainted (the toast stack); it now spans the window.
- The Settings blur snapshot captured the closing menu; the snapshot now follows one presented frame
  with the overlays hidden.
- Wabi attendance chips drew a border production does not have (22a).

## 22b.21 Regressions

- Pixel-identical against the HEAD binary (mean 0.000): FN dashboard dark and light, Wabi dashboard,
  FN Break Room, Skribbl intro, Wordle, Wabi Rest, FN Leaderboard (Friends), Wabi Friends.
- Intended differences: the Timer session list (§22b.11, now matching production), FN Friends icon
  avatars (scale with the box, as production), and the Profile "Badges" plate (now enabled).
- Tests: `study-tracker-core` 226 passed (1 ignored); app 365 passed (1 ignored). These include 22
  Stage 22b end-to-end controller tests against the mock, 8 DTO tests, 16 core tests, 3 image
  preparation, 2 backdrop-blur and 1 custom-colour tests, the Stage 22a
  Social/Skribbl tests and all Stage 15–21 tests.
- `cargo fmt --check` clean. `cargo clippy --workspace --all-targets` is equal to the HEAD baseline in
  every crate (bin 70 / bin test 32 / core 11 / core test 13 / mock 5 / mock test 5); no new warning
  kinds.

## 22b.22 How to run against the mock (never production)

As §22a.11. Additional switches: mock `--squad none|--owner|--announcement|--latency-ms N`; app
`STUDY_NATIVE_SOCIAL_SCOPE=friends|squad|world`, `STUDY_NATIVE_SOCIAL_DIALOG=details|badges|avatar|lightbox`
(captures), `STUDY_NATIVE_PICK_FILE=<path>` (file dialog answered without UI),
`STUDY_NATIVE_BREAK_PICK=<0..1>` (also pins the squad suggestion shuffle).

## 22b.23 Concerns

1. Windows has not run any 22b code. In particular `file_picker.rs` (Windows `IFileOpenDialog`) has
   never been compiled, and `take_snapshot` read-back on the Windows renderer is unverified (it falls
   back to dim-only).
2. Binary +24% (generated UI code).
3. Wabi dialogs are not parity-captured against production (debt 13).
4. The native Settings sheet carries only Stage 22b items.

## 22b.24 Windows verification plan

1. MSVC build, `cargo test --workspace`, fmt/clippy baseline; `file_picker.rs` compiles and links.
2. rustls/ring regression against the loopback mock; NoIdentity zero-network gate (production config).
3. Feed with real input: scopes, posting (with/without image, poll), votes, comments, reactions,
   edit/delete, lightbox; async feed-image row rendering; comment toast.
4. Image loading/upload through `IFileOpenDialog` (PNG, JPEG, WebP, GIF; > 6 MB refused).
5. Squads: create/search/suggest/join/request, roles, kick, leave (confirm), settings, chat; Squad Arena
   and details.
6. Avatar editor: letter/icon/photo, crop drag/wheel/slider, upload, sync.
7. Verified session with a real Timer run against the mock; heartbeat (shortened mock clock), offline
   gap > 2 h and reconcile; "not found" restart.
8. Telemetry disabled (0) / enabled with and without identity (loopback only).
9. Secret scan of logs, crash dumps, captures; credential-file ACL regression.
10. CJK/RTL/emoji names and messages; Windows CJK fallback (W22a-5) in feed and squad.
11. 0-frame checks on every Social surface and across unchanged polls (status and feed).
12. Backdrop blur on the Windows renderer (snapshot read-back) for lightboxes and dialogs.
13. Tray/minimize, single instance, shutdown with requests pending.
14. Memory/stress (the 262-click script), startup, binary size.
15. WebView2 parity captures of the §22b.15 surfaces (FN and Wabi, including Wabi dialogs).
16. DPI 125%/150% and resize; UI Automation tree of the new controls (buttons, tabs, sliders, switch).
17. Regression of Stages 18–22a.

Stage 24 physical gates remain separate (credential migration from the Tauri app, real accounts).
