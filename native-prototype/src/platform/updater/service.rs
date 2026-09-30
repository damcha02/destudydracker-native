//! The updater's orchestration: check the feed, decide, download, verify, stage. Pure logic over a
//! [`Fetcher`] (real: WinHTTP; tests: fakes), so every branch is deterministic and testable
//! without a network.
//!
//! ```text
//! check():    feed --fetch--> parse manifest --> strictly newer than me? --no--> UpToDate (same/older: never offered)
//!                                                         |yes
//!                                   platform entry + signature present + https URL?  (else typed error)
//!                                                         v
//!                                                   Available { version, notes, url, signature }
//! download(): url --fetch (size-capped)--> verify signature against the embedded key --fail--> discard, BadSignature
//!                                                         |ok
//!                                                   stage atomically --> Staged { version, path }
//! install():  NOT IMPLEMENTED - see `install_staged`.
//! ```

use std::path::PathBuf;

use super::manifest::{check_url, parse_manifest, MAX_MANIFEST_BYTES};
use super::staging;
use super::verify::TrustRoot;
use super::version::Version;
use super::UpdateError;

/// Installers are tens of MB; this bounds memory and disk against a hostile/broken server.
pub const MAX_ARTIFACT_BYTES: usize = 512 * 1024 * 1024;

/// Fetches a URL's body, refusing anything larger than `max_bytes`.
pub trait Fetcher {
    fn get(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>, UpdateError>;
}

pub struct UpdaterConfig {
    pub feed_url: String,
    pub trust: TrustRoot,
    pub current: Version,
    /// Manifest platform keys to look for, most specific first.
    pub platform_keys: Vec<String>,
    pub data_dir: PathBuf,
    /// Tests only: permit `http://127.0.0.1`. Release code constructs configs with `false`.
    pub allow_insecure_test_urls: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableUpdate {
    pub version: Version,
    pub notes: String,
    pub url: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    /// The feed's version is the same as, or older than, the running one.
    UpToDate {
        feed_version: Version,
    },
    Available(AvailableUpdate),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedUpdate {
    pub version: Version,
    pub path: PathBuf,
}

/// The production platform keys for this build (Tauri: `windows-x86_64`, and the bundle-specific
/// variants newer `tauri-action` versions add).
pub fn default_platform_keys() -> Vec<String> {
    vec!["windows-x86_64-nsis".into(), "windows-x86_64".into()]
}

pub fn check(config: &UpdaterConfig, fetcher: &dyn Fetcher) -> Result<CheckOutcome, UpdateError> {
    check_url(&config.feed_url, config.allow_insecure_test_urls)?;
    let body = fetcher.get(&config.feed_url, MAX_MANIFEST_BYTES)?;
    let manifest = parse_manifest(&body)?;
    // Never offer a same-or-older version, whatever the feed claims (downgrade protection).
    if manifest.version <= config.current {
        return Ok(CheckOutcome::UpToDate {
            feed_version: manifest.version,
        });
    }
    let keys: Vec<&str> = config.platform_keys.iter().map(String::as_str).collect();
    let entry = manifest.entry_for(&keys, config.allow_insecure_test_urls)?;
    Ok(CheckOutcome::Available(AvailableUpdate {
        version: manifest.version.clone(),
        notes: manifest.notes.clone(),
        url: entry.url.clone(),
        signature: entry.signature.clone(),
    }))
}

/// Downloads the artifact, **verifies it against the trust root**, and only then stages it. A
/// failed download or verification stages nothing and leaves any previously staged update intact.
pub fn download(
    config: &UpdaterConfig,
    fetcher: &dyn Fetcher,
    update: &AvailableUpdate,
) -> Result<StagedUpdate, UpdateError> {
    // The URL came from the (untrusted) manifest; re-check it at the point of use.
    check_url(&update.url, config.allow_insecure_test_urls)?;
    // Defense in depth: never stage something that is not newer than the running build.
    if update.version <= config.current {
        return Err(UpdateError::MalformedManifest(
            "refusing to stage a version that is not newer than the running one".into(),
        ));
    }
    let bytes = fetcher.get(&update.url, MAX_ARTIFACT_BYTES)?;
    config.trust.verify(&bytes, &update.signature)?;
    let path = staging::stage(&config.data_dir, &update.version, &bytes)?;
    Ok(StagedUpdate {
        version: update.version.clone(),
        path,
    })
}

/// Final replace-and-relaunch. **Not implemented in Stage 18, on purpose.**
///
/// A running Windows executable cannot overwrite itself, and what replaces it is decided by the
/// installer/packaging work of Stage 23 (installer technology, per-user vs per-machine,
/// upgrade-in-place identity, who relaunches). Hacking around file locking here (rename-and-copy
/// tricks, self-deleting batch files) would bake in a packaging decision and weaken the security
/// story, so the honest state is: *discovery, version comparison, authenticity verification and safe
/// staging are complete and tested; installation returns [`UpdateError::InstallNotSupported`]*.
/// Stage 23 adds the installer launch (`/passive`-style, exactly like production's
/// `installMode: passive`) on top of the verified, staged file this module produces.
pub fn install_staged(_staged: &StagedUpdate) -> Result<(), UpdateError> {
    Err(UpdateError::InstallNotSupported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    const TEST_KEY: &str = include_str!("../../../tests/fixtures/updater/test-public-key.txt");
    const ARTIFACT: &[u8] = include_bytes!("../../../tests/fixtures/updater/artifact-v0.2.0.bin");
    const GOOD_SIG: &str = include_str!("../../../tests/fixtures/updater/artifact-v0.2.0.bin.sig");
    const WRONG_KEY_SIG: &str =
        include_str!("../../../tests/fixtures/updater/artifact-v0.2.0.wrong-key.sig");

    const FEED: &str = "https://updates.example.test/latest.json";
    const ARTIFACT_URL: &str = "https://updates.example.test/Study-Tracker_0.2.0_x64-setup.exe";

    #[derive(Default)]
    struct FakeFetcher {
        responses: HashMap<String, Result<Vec<u8>, UpdateError>>,
        requested: RefCell<Vec<(String, usize)>>,
    }
    impl FakeFetcher {
        fn with(mut self, url: &str, body: &[u8]) -> Self {
            self.responses.insert(url.to_string(), Ok(body.to_vec()));
            self
        }
        fn failing(mut self, url: &str, error: UpdateError) -> Self {
            self.responses.insert(url.to_string(), Err(error));
            self
        }
    }
    impl Fetcher for FakeFetcher {
        fn get(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>, UpdateError> {
            self.requested
                .borrow_mut()
                .push((url.to_string(), max_bytes));
            match self.responses.get(url) {
                Some(Ok(body)) if body.len() > max_bytes => {
                    Err(UpdateError::TooLarge { limit: max_bytes })
                }
                Some(result) => result.clone(),
                None => Err(UpdateError::Fetch("unreachable".into())),
            }
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("st18-updater-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn config(current: &str, dir: &PathBuf) -> UpdaterConfig {
        UpdaterConfig {
            feed_url: FEED.into(),
            trust: TrustRoot::from_config_public_key(TEST_KEY).unwrap(),
            current: Version::parse(current).unwrap(),
            platform_keys: default_platform_keys(),
            data_dir: dir.clone(),
            allow_insecure_test_urls: false,
        }
    }

    fn manifest(version: &str, platform: &str, signature: &str, url: &str) -> String {
        format!(
            r#"{{"version":"{version}","notes":"Faster tray","pub_date":"2026-10-01T10:00:00Z","platforms":{{"{platform}":{{"signature":"{sig}","url":"{url}"}}}}}}"#,
            sig = signature.trim()
        )
    }

    fn available(dir: &PathBuf) -> (UpdaterConfig, AvailableUpdate) {
        let cfg = config("0.1.67", dir);
        let fetcher = FakeFetcher::default().with(
            FEED,
            manifest("0.2.0", "windows-x86_64", GOOD_SIG, ARTIFACT_URL).as_bytes(),
        );
        match check(&cfg, &fetcher).unwrap() {
            CheckOutcome::Available(a) => (cfg, a),
            other => panic!("expected Available, got {other:?}"),
        }
    }

    #[test]
    fn a_valid_newer_update_is_discovered() {
        let dir = temp_dir("newer");
        let (_cfg, update) = available(&dir);
        assert_eq!(update.version.to_string(), "0.2.0");
        assert_eq!(update.notes, "Faster tray");
        assert_eq!(update.url, ARTIFACT_URL);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn same_version_and_older_versions_are_never_offered() {
        let dir = temp_dir("downgrade");
        for (current, feed_version) in [
            ("0.2.0", "0.2.0"),
            ("0.3.0", "0.2.0"),
            ("0.2.0", "0.1.99"),
            ("1.0.0", "0.9.9"),
        ] {
            let cfg = config(current, &dir);
            let fetcher = FakeFetcher::default().with(
                FEED,
                manifest(feed_version, "windows-x86_64", GOOD_SIG, ARTIFACT_URL).as_bytes(),
            );
            assert!(
                matches!(check(&cfg, &fetcher), Ok(CheckOutcome::UpToDate { .. })),
                "running {current}, feed {feed_version}: must not be offered"
            );
        }
        // A pre-release of the running version is older than the release.
        let cfg = config("1.0.0", &dir);
        let fetcher = FakeFetcher::default().with(
            FEED,
            manifest("1.0.0-rc.1", "windows-x86_64", GOOD_SIG, ARTIFACT_URL).as_bytes(),
        );
        assert!(matches!(
            check(&cfg, &fetcher),
            Ok(CheckOutcome::UpToDate { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_manifest_wrong_platform_missing_signature_and_insecure_urls_are_typed_errors() {
        let dir = temp_dir("errors");
        let cfg = config("0.1.67", &dir);
        let run = |body: &str| check(&cfg, &FakeFetcher::default().with(FEED, body.as_bytes()));
        assert!(matches!(
            run("garbage"),
            Err(UpdateError::MalformedManifest(_))
        ));
        assert!(matches!(
            run(r#"{"version":"x","platforms":{}}"#),
            Err(UpdateError::BadVersion(_))
        ));
        assert_eq!(
            run(&manifest("0.2.0", "darwin-aarch64", GOOD_SIG, ARTIFACT_URL)),
            Err(UpdateError::NoPlatformEntry)
        );
        assert_eq!(
            run(&manifest("0.2.0", "windows-x86_64", "", ARTIFACT_URL)),
            Err(UpdateError::MissingSignature)
        );
        assert!(matches!(
            run(&manifest(
                "0.2.0",
                "windows-x86_64",
                GOOD_SIG,
                "http://updates.example.test/x.exe"
            )),
            Err(UpdateError::InsecureUrl(_))
        ));
        assert!(matches!(
            run(&manifest(
                "0.2.0",
                "windows-x86_64",
                GOOD_SIG,
                "https://user:pw@h/x.exe"
            )),
            Err(UpdateError::InsecureUrl(_))
        ));
        // Even the feed URL itself must be https.
        let mut insecure = config("0.1.67", &dir);
        insecure.feed_url = "http://updates.example.test/latest.json".into();
        assert!(matches!(
            check(&insecure, &FakeFetcher::default()),
            Err(UpdateError::InsecureUrl(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unreachable_feed_is_a_fetch_error_not_a_panic() {
        let dir = temp_dir("unreachable");
        let cfg = config("0.1.67", &dir);
        assert!(matches!(
            check(&cfg, &FakeFetcher::default()),
            Err(UpdateError::Fetch(_))
        ));
        let failing = FakeFetcher::default().failing(FEED, UpdateError::Fetch("HTTP 503".into()));
        assert_eq!(
            check(&cfg, &failing),
            Err(UpdateError::Fetch("HTTP 503".into()))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_valid_update_downloads_verifies_and_stages() {
        let dir = temp_dir("stage");
        let (cfg, update) = available(&dir);
        let fetcher = FakeFetcher::default().with(ARTIFACT_URL, ARTIFACT);
        let staged = download(&cfg, &fetcher, &update).unwrap();
        assert_eq!(std::fs::read(&staged.path).unwrap(), ARTIFACT);
        assert!(staged.path.starts_with(dir.join("updates")));
        assert_eq!(
            fetcher.requested.borrow()[0].1,
            MAX_ARTIFACT_BYTES,
            "downloads are size-capped"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupted_artifact_stages_nothing_and_keeps_the_previous_staged_update() {
        let dir = temp_dir("corrupt");
        let (cfg, update) = available(&dir);
        // A previously verified staged update exists.
        let good = download(
            &cfg,
            &FakeFetcher::default().with(ARTIFACT_URL, ARTIFACT),
            &update,
        )
        .unwrap();

        let mut tampered = ARTIFACT.to_vec();
        tampered[100] ^= 0xff;
        let result = download(
            &cfg,
            &FakeFetcher::default().with(ARTIFACT_URL, &tampered),
            &update,
        );
        assert!(
            matches!(result, Err(UpdateError::BadSignature(_))),
            "{result:?}"
        );
        assert_eq!(
            std::fs::read(&good.path).unwrap(),
            ARTIFACT,
            "the verified staged file is untouched"
        );
        let leftovers: Vec<_> = std::fs::read_dir(good.path.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            leftovers.len(),
            1,
            "no .part or corrupted file was written: {leftovers:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bad_or_foreign_signature_is_rejected_and_nothing_is_staged() {
        let dir = temp_dir("badsig");
        let cfg = config("0.1.67", &dir);
        let fetcher = FakeFetcher::default().with(ARTIFACT_URL, ARTIFACT);
        let forged = AvailableUpdate {
            version: Version::parse("0.2.0").unwrap(),
            notes: String::new(),
            url: ARTIFACT_URL.into(),
            signature: WRONG_KEY_SIG.trim().into(),
        };
        assert!(matches!(
            download(&cfg, &fetcher, &forged),
            Err(UpdateError::BadSignature(_))
        ));
        let unsigned = AvailableUpdate {
            signature: String::new(),
            ..forged.clone()
        };
        assert_eq!(
            download(&cfg, &fetcher, &unsigned),
            Err(UpdateError::MissingSignature)
        );
        assert!(!dir.join("updates").exists(), "nothing was staged");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_download_and_an_oversized_response_are_errors() {
        let dir = temp_dir("dlfail");
        let (cfg, update) = available(&dir);
        let down = FakeFetcher::default()
            .failing(ARTIFACT_URL, UpdateError::Fetch("connection reset".into()));
        assert!(matches!(
            download(&cfg, &down, &update),
            Err(UpdateError::Fetch(_))
        ));
        struct Huge;
        impl Fetcher for Huge {
            fn get(&self, _: &str, max_bytes: usize) -> Result<Vec<u8>, UpdateError> {
                Err(UpdateError::TooLarge { limit: max_bytes })
            }
        }
        assert!(matches!(
            download(&cfg, &Huge, &update),
            Err(UpdateError::TooLarge { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hostile_artifact_url_is_rechecked_at_download_time_and_a_non_newer_update_is_never_staged()
    {
        let dir = temp_dir("hostile");
        let (cfg, mut update) = available(&dir);
        update.url = "http://evil.test/x.exe".into();
        assert!(matches!(
            download(&cfg, &FakeFetcher::default(), &update),
            Err(UpdateError::InsecureUrl(_))
        ));
        update.url = ARTIFACT_URL.into();
        update.version = Version::parse("0.1.67").unwrap();
        assert!(
            download(
                &cfg,
                &FakeFetcher::default().with(ARTIFACT_URL, ARTIFACT),
                &update
            )
            .is_err(),
            "same version as running"
        );
        update.version = Version::parse("0.1.0").unwrap();
        assert!(
            download(
                &cfg,
                &FakeFetcher::default().with(ARTIFACT_URL, ARTIFACT),
                &update
            )
            .is_err(),
            "older than running"
        );
        assert!(!dir.join("updates").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn installation_is_explicitly_not_supported_yet_and_never_executes_anything() {
        let staged = StagedUpdate {
            version: Version::parse("0.2.0").unwrap(),
            path: PathBuf::from("does-not-matter.exe"),
        };
        assert_eq!(
            install_staged(&staged),
            Err(UpdateError::InstallNotSupported)
        );
    }
}
