//! The update manifest (`latest.json`), in the exact shape Tauri's `tauri-action` publishes:
//!
//! ```json
//! { "version": "0.1.68", "notes": "...", "pub_date": "2026-10-01T10:00:00Z",
//!   "platforms": { "windows-x86_64": { "signature": "<base64 minisign .sig>", "url": "https://..." } } }
//! ```
//!
//! Parsing is strict about what matters for safety (a valid semantic version, `https` URLs without
//! credentials, a non-empty signature for the selected platform) and lenient about what does not
//! (unknown extra fields are ignored, `notes`/`pub_date` are optional).

use std::collections::BTreeMap;

use super::version::Version;
use super::UpdateError;

/// A manifest is a few hundred bytes; anything near this is not one.
pub const MAX_MANIFEST_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformEntry {
    pub url: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub version: Version,
    pub notes: String,
    pub pub_date: Option<String>,
    platforms: BTreeMap<String, PlatformEntry>,
}

pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest, UpdateError> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(UpdateError::TooLarge {
            limit: MAX_MANIFEST_BYTES,
        });
    }
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| UpdateError::MalformedManifest(format!("not valid JSON ({e})")))?;
    let object = value
        .as_object()
        .ok_or_else(|| UpdateError::MalformedManifest("the top level is not an object".into()))?;
    let version_text = object
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| UpdateError::MalformedManifest("missing \"version\"".into()))?;
    let version = Version::parse(version_text)
        .ok_or_else(|| UpdateError::BadVersion(version_text.to_string()))?;
    let platforms_value = object
        .get("platforms")
        .and_then(|v| v.as_object())
        .ok_or_else(|| UpdateError::MalformedManifest("missing \"platforms\"".into()))?;
    let mut platforms = BTreeMap::new();
    for (key, entry) in platforms_value {
        // A malformed *other* platform must not break this one: skip entries that are not objects.
        let Some(entry) = entry.as_object() else {
            continue;
        };
        let url = entry
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let signature = entry
            .get("signature")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        platforms.insert(key.clone(), PlatformEntry { url, signature });
    }
    Ok(Manifest {
        version,
        notes: object
            .get("notes")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        pub_date: object
            .get("pub_date")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        platforms,
    })
}

impl Manifest {
    /// Picks the entry for this platform: the first of `keys` (most specific first, e.g.
    /// `windows-x86_64-nsis`, then `windows-x86_64`) that exists. Validates it: a signature must be
    /// present and the URL must be acceptable (see [`check_url`]).
    pub fn entry_for(
        &self,
        keys: &[&str],
        allow_insecure_test_urls: bool,
    ) -> Result<&PlatformEntry, UpdateError> {
        let entry = keys
            .iter()
            .find_map(|key| self.platforms.get(*key))
            .ok_or(UpdateError::NoPlatformEntry)?;
        if entry.signature.trim().is_empty() {
            return Err(UpdateError::MissingSignature);
        }
        check_url(&entry.url, allow_insecure_test_urls)?;
        Ok(entry)
    }
}

/// Update URLs must be `https://host/...` with no embedded credentials. (`allow_insecure` exists
/// only so tests can use a loopback server; release code always passes `false`.)
pub fn check_url(url: &str, allow_insecure: bool) -> Result<(), UpdateError> {
    let rest = if let Some(rest) = url.strip_prefix("https://") {
        rest
    } else if allow_insecure && url.starts_with("http://127.0.0.1") {
        &url["http://".len()..]
    } else {
        return Err(UpdateError::InsecureUrl(url.to_string()));
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') || url.bytes().any(|b| b < 0x21 || b == 0x7f)
    {
        return Err(UpdateError::InsecureUrl(url.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> String {
        r#"{"version":"0.2.0","notes":"n","pub_date":"2026-10-01T10:00:00Z","extra":1,
            "platforms":{"windows-x86_64":{"signature":"c2ln","url":"https://example.test/a.exe"},
                         "darwin-aarch64":{"signature":"","url":"https://example.test/a.tar.gz"},
                         "linux-x86_64":"garbage"}}"#
            .to_string()
    }

    #[test]
    fn parses_a_tauri_action_manifest_and_ignores_unknown_fields_and_broken_other_platforms() {
        let m = parse_manifest(good().as_bytes()).unwrap();
        assert_eq!(m.version.to_string(), "0.2.0");
        assert_eq!(m.notes, "n");
        let e = m
            .entry_for(&["windows-x86_64-nsis", "windows-x86_64"], false)
            .unwrap();
        assert_eq!(e.url, "https://example.test/a.exe");
    }

    #[test]
    fn the_more_specific_platform_key_wins() {
        let m = parse_manifest(
            br#"{"version":"1.0.0","platforms":{
            "windows-x86_64":{"signature":"A","url":"https://h/generic.exe"},
            "windows-x86_64-nsis":{"signature":"B","url":"https://h/nsis.exe"}}}"#,
        )
        .unwrap();
        assert_eq!(
            m.entry_for(&["windows-x86_64-nsis", "windows-x86_64"], false)
                .unwrap()
                .url,
            "https://h/nsis.exe"
        );
    }

    #[test]
    fn malformed_manifests_are_rejected_not_panicked_on() {
        for (body, what) in [
            ("", "empty"),
            ("not json", "not json"),
            ("[]", "array"),
            ("{}", "no version"),
            (r#"{"version":"1.0.0"}"#, "no platforms"),
            (r#"{"version":7,"platforms":{}}"#, "numeric version"),
            (r#"{"version":"1.0","platforms":{}}"#, "short version"),
            (
                r#"{"version":"../../x","platforms":{}}"#,
                "path-like version",
            ),
        ] {
            assert!(
                parse_manifest(body.as_bytes()).is_err(),
                "{what} must be rejected"
            );
        }
        let huge = vec![b' '; MAX_MANIFEST_BYTES + 1];
        assert_eq!(
            parse_manifest(&huge),
            Err(UpdateError::TooLarge {
                limit: MAX_MANIFEST_BYTES
            })
        );
    }

    #[test]
    fn wrong_platform_and_missing_signature_are_distinct_errors() {
        let m = parse_manifest(good().as_bytes()).unwrap();
        assert_eq!(
            m.entry_for(&["windows-aarch64"], false),
            Err(UpdateError::NoPlatformEntry)
        );
        assert_eq!(
            m.entry_for(&["darwin-aarch64"], false),
            Err(UpdateError::MissingSignature)
        );
        assert_eq!(
            m.entry_for(&["linux-x86_64"], false),
            Err(UpdateError::NoPlatformEntry),
            "a non-object entry is ignored, as if absent"
        );
    }

    #[test]
    fn only_plain_https_urls_without_credentials_are_acceptable() {
        assert!(check_url("https://github.com/o/r/releases/download/v1/x.exe", false).is_ok());
        for bad in [
            "http://github.com/x.exe",
            "ftp://h/x",
            "file:///c:/x.exe",
            "https://user:pw@h/x",
            "https:///x",
            "https://h/x y",
            "//h/x",
            "",
            "javascript:alert(1)",
            "https://h/\u{0}",
        ] {
            assert!(check_url(bad, false).is_err(), "{bad:?}");
        }
        assert!(
            check_url("http://127.0.0.1:8080/x", false).is_err(),
            "release builds never allow http"
        );
        assert!(
            check_url("http://127.0.0.1:8080/x", true).is_ok(),
            "tests may use loopback"
        );
        assert!(
            check_url("http://evil.test/x", true).is_err(),
            "even the test allowance is loopback-only"
        );
    }
}
