//! Opt-in usage telemetry (Stage 22b, decision D3): production's `sendOptInTelemetryHeartbeat`,
//! migrated - not new telemetry.
//!
//! Exactly production's semantics:
//! - the user-facing setting "Anonymous usage telemetry" (`settings.telemetryEnabled`) is **off
//!   by default**;
//! - while it is on: one `POST /telemetry/heartbeat` when it is turned on (or at start-up when it
//!   is already on), then one every hour; turning it off stops the schedule;
//! - the payload is `{ installId, app: { version, platform, runtimeChannel } }` and nothing else:
//!   no `userId`, no `deviceSecret`, no device fingerprint; it does not need a Social account;
//! - `installId` is a random UUID made once per installation (`crypto.randomUUID()`, kept in
//!   `localStorage`, not in the backup);
//! - a failed heartbeat is only logged (no retry).

use serde::{Deserialize, Serialize};

/// `TELEMETRY_HEARTBEAT_INTERVAL_MS`.
pub const HEARTBEAT_INTERVAL_MS: i64 = 60 * 60 * 1000;

/// The settings row's caption (`On`/`Off` + this).
pub const SETTING_TITLE: &str = "Anonymous usage telemetry";
pub const SETTING_DETAIL: &str =
    "shares only app version, platform, runtime channel, and last-opened time";

/// The per-install pseudonymous id (`study-tracker-telemetry-install-id`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstallId(String);

impl InstallId {
    /// A v4 UUID from 16 random bytes (the caller owns the RNG).
    pub fn from_random(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        Self(format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        ))
    }

    /// A stored id: the Worker accepts up to 80 characters of trimmed text.
    pub fn parse(raw: &str) -> Option<Self> {
        let t = raw.trim();
        (!t.is_empty()
            && t.len() <= super::limits::MAX_ID_LEN
            && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .then(|| Self(t.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
