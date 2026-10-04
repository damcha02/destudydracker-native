//! Social identity (Stage 22a, decision D1).
//!
//! Production identity is a client-held pair `{userId, deviceSecret}`: both `crypto.randomUUID()`
//! strings minted by `makeDefaultSocialState()`, the account created lazily by the first
//! `/sync/v2`. Whoever holds the pair *is* the account, so the `deviceSecret` is a bearer
//! credential.
//!
//! Native makes the states explicit (production has only "always an identity"):
//!
//! ```text
//!   NoIdentity ──(user confirms "create")──► NewIdentity(candidate) ──(bootstrap ok)──► ExistingIdentity
//!        ▲                                          │
//!        └──────────────(bootstrap failed: rollback, nothing kept)┘
//!   NoIdentity ──(credential store holds a pair)──► ExistingIdentity
//! ```
//!
//! `NewIdentity` is only ever entered by an explicit user action; launching never mints one.

use std::fmt;

use super::ids::UserId;
use super::limits::MAX_SECRET_LEN;

/// The `deviceSecret` credential. Its only way out is [`DeviceSecret::expose`], used by the
/// request builders and the credential file; `Debug` is redacted and there is no `Display`, so a
/// stray `{:?}`/`format!` cannot leak it into a log, an error string or a panic message.
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceSecret(String);

impl DeviceSecret {
    /// Bounded like the Worker's `cleanDeviceSecret` (`MAX_SECRET_LENGTH`), non-empty after trim,
    /// no controls or whitespace.
    pub fn parse(raw: &str) -> Option<Self> {
        let t = raw.trim();
        (!t.is_empty()
            && t.len() <= MAX_SECRET_LEN
            && !t.chars().any(|c| c.is_control() || c.is_whitespace()))
        .then(|| Self(t.to_string()))
    }

    /// The raw value, for the wire and the credential file only.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for DeviceSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeviceSecret(<redacted>)")
    }
}

impl Drop for DeviceSecret {
    fn drop(&mut self) {
        // Best effort: overwrite the bytes before the allocation is released, so a freed buffer
        // does not keep the credential around (not a guarantee against copies made earlier).
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.iter_mut().for_each(|b| *b = 0);
        std::hint::black_box(&bytes);
    }
}

/// A production-compatible identity.
#[derive(Clone, PartialEq, Eq)]
pub struct SocialIdentity {
    pub user_id: UserId,
    pub device_secret: DeviceSecret,
}

impl fmt::Debug for SocialIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SocialIdentity")
            .field("user_id", &self.user_id)
            .field("device_secret", &self.device_secret)
            .finish()
    }
}

/// Formats 16 random bytes as an RFC 4122 version-4 UUID (`crypto.randomUUID()`'s shape).
pub fn uuid_v4(mut bytes: [u8; 16]) -> String {
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

impl SocialIdentity {
    /// A brand-new identity from caller-supplied randomness (`makeDefaultSocialState`: two
    /// `crypto.randomUUID()`s). The application passes OS CSPRNG bytes.
    pub fn mint(user_bytes: [u8; 16], secret_bytes: [u8; 16]) -> Self {
        Self {
            user_id: UserId::parse(&uuid_v4(user_bytes)).expect("a UUID is a valid id"),
            device_secret: DeviceSecret::parse(&uuid_v4(secret_bytes))
                .expect("a UUID is a valid secret"),
        }
    }
}

/// Where the Social identity stands. "No account" is a different thing from "network
/// unavailable": the latter is a property of requests, never of this state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityPhase {
    /// No Social account on this device: zero Social network activity.
    NoIdentity,
    /// The user explicitly asked for a new account; `candidate` is not persisted until the
    /// bootstrap sync succeeds.
    NewIdentity { candidate: SocialIdentity },
    /// A usable account (loaded from the credential store, or a confirmed new one).
    ExistingIdentity { identity: SocialIdentity },
}

impl IdentityPhase {
    /// The identity requests may be made with. `NewIdentity` returns its candidate (only the
    /// bootstrap sync uses it).
    pub fn identity(&self) -> Option<&SocialIdentity> {
        match self {
            Self::NoIdentity => None,
            Self::NewIdentity { candidate } => Some(candidate),
            Self::ExistingIdentity { identity } => Some(identity),
        }
    }

    /// Only an established identity may run production's background schedule (D2).
    pub fn is_established(&self) -> bool {
        matches!(self, Self::ExistingIdentity { .. })
    }
}
