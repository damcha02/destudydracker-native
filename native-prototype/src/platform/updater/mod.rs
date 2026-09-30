//! Native updater (Stage 18): discovery, version comparison, authenticity verification and safe
//! staging of updates. **Installation of the staged artifact is deliberately not implemented here**
//! (see [`service::install_staged`]) - it depends on packaging decisions reserved for Stage 23.
//!
//! # Production model being preserved (Tauri updater, `desktop/src-tauri/tauri.conf.json`)
//!
//! * **Feed**: one JSON manifest, `latest.json`, published as an asset of the GitHub Release
//!   (`…/releases/latest/download/latest.json`). `{ version, notes, pub_date,
//!   platforms: { "<os>-<arch>[-<bundle>]": { url, signature } } }`.
//! * **Trust root**: a *minisign* (Ed25519) public key embedded in the application
//!   (`plugins.updater.pubkey`, base64 of the `.pub` file). Releases are signed by CI with the
//!   matching private key (`TAURI_SIGNING_PRIVATE_KEY` secret, never in this repository).
//! * **What is signed**: the *artifact* (installer), not the manifest. `platforms.*.signature` is
//!   the base64 of the minisign `.sig` file. HTTPS protects transport and the manifest's
//!   `version`/`url`; authenticity of what will be executed rests on the signature alone.
//! * **Flow**: check on startup and every 24 h, compare versions, on user action download,
//!   **verify the signature against the embedded key**, run the installer (Windows:
//!   `installMode: passive`), relaunch.
//!
//! # What the native implementation does differently, and why
//!
//! * The HTTP client is the OS's **WinHTTP** (system certificate store, proxy settings, TLS
//!   stack): no TLS/HTTP crate is linked into the binary.
//! * **Downgrades are refused**: only a strictly newer semantic version is offered (Tauri offers
//!   any version `!=` current by default comparator being `>`; we additionally never stage a same
//!   or older version even if the feed says so).
//! * Staged file names are **derived from the validated version**, never from the feed's URL.
//! * The public key used in release builds is a compile-time constant; it cannot be overridden at
//!   runtime. (Debug builds accept a test key through `STUDY_NATIVE_UPDATE_TEST_PUBKEY` so the whole
//!   path can be exercised with throw-away keys.)

pub mod manifest;
pub mod service;
pub mod staging;
pub mod verify;
pub mod version;

#[cfg(windows)]
pub mod http;

use std::fmt;

/// Every way the updater can refuse or fail. None of them is ever fatal to the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateError {
    /// The feed/artifact could not be fetched (network, TLS, HTTP status, timeout).
    Fetch(String),
    /// A response exceeded the size limit.
    TooLarge { limit: usize },
    /// The manifest is not valid JSON or lacks required fields.
    MalformedManifest(String),
    /// The manifest (or its `version`) is not a valid semantic version.
    BadVersion(String),
    /// The manifest has no entry for this platform.
    NoPlatformEntry,
    /// The platform entry has no (or an empty) signature.
    MissingSignature,
    /// A URL is not `https` (or contains credentials).
    InsecureUrl(String),
    /// The signature is not well-formed, or does not verify against the trust root (wrong key,
    /// tampered/corrupted artifact, or tampered signature).
    BadSignature(String),
    /// The embedded public key could not be parsed (a build problem, not a network one).
    BadTrustRoot(String),
    /// Writing the staged file failed.
    Staging(String),
    /// Installing is not available yet (packaging decision reserved for Stage 23).
    InstallNotSupported,
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fetch(e) => write!(f, "could not fetch the update information: {e}"),
            Self::TooLarge { limit } => {
                write!(f, "the response is larger than the {limit}-byte limit")
            }
            Self::MalformedManifest(e) => write!(f, "the update manifest is malformed: {e}"),
            Self::BadVersion(v) => write!(f, "'{v}' is not a valid version"),
            Self::NoPlatformEntry => {
                write!(f, "the update manifest has no build for this platform")
            }
            Self::MissingSignature => write!(f, "the update has no signature and was rejected"),
            Self::InsecureUrl(u) => write!(f, "refusing a non-https update URL: {u}"),
            Self::BadSignature(e) => write!(
                f,
                "the update's signature is not valid and it was rejected: {e}"
            ),
            Self::BadTrustRoot(e) => write!(f, "the embedded update key is unusable: {e}"),
            Self::Staging(e) => write!(f, "could not store the downloaded update: {e}"),
            Self::InstallNotSupported => write!(
                f,
                "installing updates from inside the app is not available in this build yet"
            ),
        }
    }
}

impl std::error::Error for UpdateError {}
