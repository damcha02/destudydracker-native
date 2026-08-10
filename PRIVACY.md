# Privacy — social sync & abuse prevention

This document covers the optional social/sync features of Study Tracker (friends, leaderboards, the
feed, squads) — the parts of the app that talk to a server. If you never enable social sync, none of
this applies: your study data stays entirely on your device.

The server side of these features is a Cloudflare Worker. Everything below describes what that
Worker itself records in its own database — it is not a statement about what Cloudflare's platform
sees or logs at the network edge, which is outside this document's scope.

## Signup telemetry — recorded once, on account creation

When you create a new synced profile, the Worker records:

- **Country and network provider (ASN)** — coarse location/network information, similar to what most
  websites can see from any visit.
- **A one-way, non-reversible fingerprint of your IP address** — not the IP address itself. It's
  produced with a keyed hash (HMAC) using a secret only the server holds, so it cannot be reversed
  back into your IP, but two signups from the same network will produce the same fingerprint. This
  lets the server notice "many accounts created from one place in a short time" without storing a
  raw IP address.

**This Worker does not persist raw IP addresses in its D1 database.**

This telemetry is recorded only at account creation, not on every sync afterward.

## Online leaderboard sessions

When the desktop timer runs while online, the Worker records server timestamps for the session start,
periodic timer check-ins, and finish. Those timestamps calculate leaderboard credit; locally saved or
offline timer sessions remain personal-only. The timer check-ins are automatic and occur about every
15 minutes while a verified session is active.

## Abuse-event telemetry — recorded only when a signal fires

Separately, if a request trips an automated abuse signal — for example, hitting a decoy endpoint no
real client ever calls, filling a hidden trap field, or a suspiciously large first-time data backfill
— that specific event is logged with: which rule was triggered, the request path, the user-agent
string, a short detail note, country, network provider (ASN/AS organization), and — when a usable IP
is available for that request — the same non-reversible IP fingerprint described above. This does not
happen on ordinary, non-triggering requests.

Flagged accounts are not blocked automatically; they're queued for manual review by the app's
operator.

## What this is used for

Solely to detect and slow down automated abuse of the free-to-create social accounts (fake accounts,
scripted account floods, leaderboard manipulation via fabricated study data). It is never used for
advertising, profiling, or sold or shared with anyone.

## Retention

Network telemetry — the country/ASN/IP-fingerprint fields on both accounts and abuse-event log
entries — is automatically deleted (or nulled out, for still-active accounts) **90 days** after it
was recorded, regardless of an account's flagged status. Nothing in this category is kept
indefinitely.

The *reason* an account was flagged (e.g. "hit a decoy endpoint," short text labels only, no network
data) is kept separately for as long as the account remains flagged, since that history is what the
operator reviews. Once an account is unflagged or removed, its stale reason history is cleaned up on
the same schedule.

## What's visible to the app's operator

The operator has an admin view of synced accounts for running the service, which can show country and
network provider for flagged accounts (useful for telling apart, say, a residential visitor from an
automated hosting provider). **The IP fingerprint itself is never shown there or returned by any API
response** — it exists only inside the server's own database, for the automated rate-limiting and
correlation logic to use internally.

## Account credentials

Each device generates its own random sync credential locally; there is no email, password, or login.
New accounts, and accounts that have made another authenticated request since this protection was
added, now also have a hashed copy of that credential, and verification checks the hash rather than
the plaintext value going forward. This migration is phased: the original plaintext credential
remains present in the database alongside the hash for now, and is only removed in a later cleanup
step once every account has been migrated. Until that cleanup happens, this document will not claim
plaintext storage has ended — it is in the process of being phased out, not finished.

## Questions

This is a personal / hobby project, not a company. If you have questions about this policy, open an
issue on the project's repository.
