//! Typed identifiers and the display-text rule shared by every server string (Stage 22a).

use serde::{Deserialize, Serialize};

use super::limits::{MAX_FRIEND_CODE_LEN, MAX_ID_LEN};

/// Production's friend-code alphabet (`randomToken` in `storage.ts`, `FRIEND_CODE_ALPHABET` in
/// the Worker): no I, O, 0 or 1.
pub const FRIEND_CODE_ALPHABET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// True for characters that must never reach a label: C0/C1 controls (newlines included - every
/// Social string is single-line in production's layout) and the explicit bidi embedding,
/// override and isolate controls (U+202A..U+202E, U+2066..U+2069), which would reorder the text
/// *around* an untrusted name (" (You)", a score, a log line). Implicit marks (LRM/RLM/ALM) and
/// all ordinary RTL text are kept.
fn is_unsafe_display_char(c: char) -> bool {
    c.is_control() || matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// The display rule for untrusted single-line text: unsafe characters removed, at most
/// `max_chars` characters kept (a defensive bound; production limits are far below it).
pub fn display_text(raw: &str, max_chars: usize) -> String {
    raw.chars()
        .filter(|c| !is_unsafe_display_char(*c))
        .take(max_chars)
        .collect()
}

/// True when `raw` is a usable identifier: non-empty after trimming, bounded, no controls or
/// whitespace inside. (`requiredText` trims; ids are UUIDs or `crypto.randomUUID()` strings.)
fn valid_id(raw: &str, max_len: usize) -> bool {
    let t = raw.trim();
    !t.is_empty() && t.len() <= max_len && !t.chars().any(|c| c.is_control() || c.is_whitespace())
}

macro_rules! string_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Validates and trims; `None` for an empty, overlong or control-bearing value.
            pub fn parse(raw: &str) -> Option<Self> {
                valid_id(raw, MAX_ID_LEN).then(|| Self(raw.trim().to_string()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

string_id!(
    /// A Social account id (`crypto.randomUUID()` on the client that created it).
    UserId
);
string_id!(
    /// A friend-request id (Worker `crypto.randomUUID()`).
    RequestId
);
string_id!(
    /// A Daily Skribbl drawing id (Worker `crypto.randomUUID()`).
    DrawingId
);

/// A player tag. Production generates `XXXX-XXXX` from [`FRIEND_CODE_ALPHABET`]; codes received
/// from the server are accepted more loosely (legacy accounts exist), but always bounded and
/// printable.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FriendCode(String);

impl FriendCode {
    /// A code received from the server or the store.
    pub fn parse(raw: &str) -> Option<Self> {
        let t = raw.trim();
        (!t.is_empty()
            && t.len() <= MAX_FRIEND_CODE_LEN
            && !t.chars().any(|c| c.is_control() || c.is_whitespace()))
        .then(|| Self(t.to_string()))
    }

    /// What the user typed, as production normalizes it before sending (`trim().toUpperCase()`;
    /// the input itself upper-cases on every keystroke). `None` when empty.
    pub fn from_user_input(raw: &str) -> Option<Self> {
        Self::parse(&raw.trim().to_uppercase())
    }

    /// True for production's own generated shape (`^[ALPHABET]{4}-[ALPHABET]{4}$`).
    pub fn is_standard(&self) -> bool {
        let b = self.0.as_bytes();
        b.len() == 9
            && b[4] == b'-'
            && b.iter()
                .enumerate()
                .all(|(i, ch)| i == 4 || FRIEND_CODE_ALPHABET.as_bytes().contains(ch))
    }

    /// `makeFriendCode()`: `randomToken(4)-randomToken(4)`, each character
    /// `alphabet[byte % alphabet.length]` of a random byte. The caller supplies the 8 random
    /// bytes (the core never touches an RNG).
    pub fn generate(random: [u8; 8]) -> Self {
        let alphabet = FRIEND_CODE_ALPHABET.as_bytes();
        let pick = |b: u8| alphabet[usize::from(b) % alphabet.len()] as char;
        let mut code = String::with_capacity(9);
        for (i, b) in random.iter().enumerate() {
            if i == 4 {
                code.push('-');
            }
            code.push(pick(*b));
        }
        Self(code)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
