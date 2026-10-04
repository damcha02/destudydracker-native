//! The Social credential boundary (Stage 22a, decision D1, brief §10/§35/§36).
//!
//! The `{userId, deviceSecret}` pair is a bearer credential, so it is **not** part of the
//! ordinary store (`store.json`, which also holds the non-secret Social profile and caches):
//!
//! - its own file, `social-credentials.json`, next to the store in the per-user data directory;
//!   written atomically (temp file + rename), owner-only permissions on Unix (`0600`); on Windows
//!   the per-user `%LOCALAPPDATA%` ACL applies;
//! - bound to an [`EndpointClass`]: a credential made against the local mock Worker is never
//!   presented to production, and a production credential is never sent to a test server;
//! - read and written only through [`CredentialStore`]. Stage 24 can replace the file with an OS
//!   credential store (Windows Credential Manager / DPAPI, macOS Keychain, Secret Service) behind
//!   the same trait without touching any caller;
//! - never logged, never formatted (`DeviceSecret` has a redacted `Debug` and no `Display`), and
//!   never part of the diagnostics or debug exports.
//!
//! Production import ([`identity_from_production_backup`]) is the Stage 24 migration's boundary:
//! it reads a production backup's `social.userId`/`social.deviceSecret` and returns them as a
//! typed identity. In Stage 22a it is exercised **only with synthetic fixtures**; no runtime path
//! calls it, so no real credential is migrated.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use study_tracker_core::social::{DeviceSecret, SocialIdentity, UserId};

use crate::net::endpoint::EndpointClass;

pub const FILE_NAME: &str = "social-credentials.json";
const VERSION: u32 = 1;

/// A stored identity and the endpoint family it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCredential {
    pub identity: SocialIdentity,
    pub endpoint: EndpointClass,
}

#[derive(Debug)]
pub enum CredentialError {
    Io(io::ErrorKind),
    /// The file exists but is not a credential this build understands. It is left in place
    /// (never overwritten automatically) and Social starts without an identity.
    Malformed,
}

impl std::fmt::Display for CredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // never the file contents
        match self {
            Self::Io(kind) => write!(f, "credential file I/O error ({kind:?})"),
            Self::Malformed => f.write_str("credential file is not readable"),
        }
    }
}

pub trait CredentialStore {
    /// `Ok(None)`: no credential (a fresh profile, or cleared).
    fn load(&self) -> Result<Option<StoredCredential>, CredentialError>;
    fn save(&mut self, credential: &StoredCredential) -> Result<(), CredentialError>;
    #[cfg_attr(not(test), allow(dead_code))]
    fn clear(&mut self) -> Result<(), CredentialError>;
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileFormat {
    version: u32,
    endpoint: String,
    user_id: String,
    device_secret: String,
}

pub struct FileCredentialStore {
    path: PathBuf,
}

impl FileCredentialStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join(FILE_NAME),
        }
    }

    #[cfg(test)]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn parse(text: &str) -> Result<StoredCredential, CredentialError> {
    let f: FileFormat = serde_json::from_str(text).map_err(|_| CredentialError::Malformed)?;
    if f.version != VERSION {
        return Err(CredentialError::Malformed);
    }
    Ok(StoredCredential {
        endpoint: EndpointClass::parse(&f.endpoint).ok_or(CredentialError::Malformed)?,
        identity: SocialIdentity {
            user_id: UserId::parse(&f.user_id).ok_or(CredentialError::Malformed)?,
            device_secret: DeviceSecret::parse(&f.device_secret)
                .ok_or(CredentialError::Malformed)?,
        },
    })
}

fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    {
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp)?;
        io::Write::write_all(&mut file, bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}

impl CredentialStore for FileCredentialStore {
    fn load(&self) -> Result<Option<StoredCredential>, CredentialError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => parse(&text).map(Some),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(CredentialError::Io(e.kind())),
        }
    }

    fn save(&mut self, credential: &StoredCredential) -> Result<(), CredentialError> {
        let file = FileFormat {
            version: VERSION,
            endpoint: credential.endpoint.id().to_string(),
            user_id: credential.identity.user_id.as_str().to_string(),
            device_secret: credential.identity.device_secret.expose().to_string(),
        };
        let mut bytes = serde_json::to_vec_pretty(&file).map_err(|_| CredentialError::Malformed)?;
        let result = write_private(&self.path, &bytes).map_err(|e| CredentialError::Io(e.kind()));
        bytes.iter_mut().for_each(|b| *b = 0);
        result
    }

    fn clear(&mut self) -> Result<(), CredentialError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CredentialError::Io(e.kind())),
        }
    }
}

/// In-memory store for tests and for runs that must never touch disk.
#[derive(Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct MemoryCredentialStore {
    pub stored: Option<StoredCredential>,
    pub fail_saves: bool,
}

impl CredentialStore for MemoryCredentialStore {
    fn load(&self) -> Result<Option<StoredCredential>, CredentialError> {
        Ok(self.stored.clone())
    }

    fn save(&mut self, credential: &StoredCredential) -> Result<(), CredentialError> {
        if self.fail_saves {
            return Err(CredentialError::Io(io::ErrorKind::PermissionDenied));
        }
        self.stored = Some(credential.clone());
        Ok(())
    }

    fn clear(&mut self) -> Result<(), CredentialError> {
        self.stored = None;
        Ok(())
    }
}

/// The identity a credential grants for an endpoint: only when the classes match.
pub fn usable_for(
    stored: Option<StoredCredential>,
    endpoint: EndpointClass,
) -> Option<SocialIdentity> {
    match stored {
        Some(c) if c.endpoint == endpoint => Some(c.identity),
        Some(c) => {
            log::warn!(
                "social: the stored credential belongs to the {} endpoint, not {}; Social starts without an account",
                c.endpoint.id(),
                endpoint.id()
            );
            None
        }
        None => None,
    }
}

/// Why a backup cannot provide an identity.
#[derive(Debug, PartialEq, Eq)]
pub enum BackupIdentityError {
    NotABackup,
    NoIdentity,
}

/// **Stage 24 boundary - synthetic fixtures only in Stage 22a.** Reads the account identity from
/// a production backup the way production's own `restoreBackup` validates it (wrapped
/// `{backupVersion, state}` or bare state; `sessions`/`courses` arrays and a `social` object with
/// non-empty `userId`/`deviceSecret` strings).
#[cfg_attr(not(test), allow(dead_code))]
pub fn identity_from_production_backup(text: &str) -> Result<SocialIdentity, BackupIdentityError> {
    let root: serde_json::Value =
        serde_json::from_str(text).map_err(|_| BackupIdentityError::NotABackup)?;
    let state = match root.get("backupVersion") {
        Some(v) if v.is_number() && root.get("state").is_some_and(|s| s.is_object()) => {
            &root["state"]
        }
        _ => &root,
    };
    if !state["sessions"].is_array() || !state["courses"].is_array() || !state["social"].is_object()
    {
        return Err(BackupIdentityError::NotABackup);
    }
    let social = &state["social"];
    let (Some(user), Some(secret)) = (social["userId"].as_str(), social["deviceSecret"].as_str())
    else {
        return Err(BackupIdentityError::NoIdentity);
    };
    Ok(SocialIdentity {
        user_id: UserId::parse(user).ok_or(BackupIdentityError::NoIdentity)?,
        device_secret: DeviceSecret::parse(secret).ok_or(BackupIdentityError::NoIdentity)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "TEST_SECRET_MUST_NOT_APPEAR";

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("st22-cred-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn credential(endpoint: EndpointClass) -> StoredCredential {
        StoredCredential {
            identity: SocialIdentity {
                user_id: UserId::parse("synthetic-user-0001").unwrap(),
                device_secret: DeviceSecret::parse(SECRET).unwrap(),
            },
            endpoint,
        }
    }

    #[test]
    fn absent_save_load_replace_clear() {
        let dir = temp_dir("cycle");
        let mut store = FileCredentialStore::new(&dir);
        assert!(store.load().unwrap().is_none(), "absent");
        store.save(&credential(EndpointClass::LocalTest)).unwrap();
        assert_eq!(
            store.load().unwrap(),
            Some(credential(EndpointClass::LocalTest))
        );
        let mut other = credential(EndpointClass::LocalTest);
        other.identity.user_id = UserId::parse("synthetic-user-0002").unwrap();
        store.save(&other).unwrap();
        assert_eq!(store.load().unwrap(), Some(other), "replace");
        assert!(
            !dir.join("social-credentials.json.tmp").exists(),
            "no temp file left behind"
        );
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none(), "clear");
        store.clear().unwrap();
        fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("perm");
        let mut store = FileCredentialStore::new(&dir);
        store.save(&credential(EndpointClass::LocalTest)).unwrap();
        let mode = fs::metadata(store.path()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn malformed_files_are_reported_and_left_alone() {
        let dir = temp_dir("bad");
        let store = FileCredentialStore::new(&dir);
        for bad in [
            "",
            "{",
            "[]",
            "{\"version\":1}",
            "{\"version\":2,\"endpoint\":\"production\",\"userId\":\"u\",\"deviceSecret\":\"s\"}",
            "{\"version\":1,\"endpoint\":\"staging\",\"userId\":\"u\",\"deviceSecret\":\"s\"}",
            "{\"version\":1,\"endpoint\":\"production\",\"userId\":\"\",\"deviceSecret\":\"s\"}",
        ] {
            fs::write(store.path(), bad).unwrap();
            let err = store.load().unwrap_err();
            assert!(matches!(err, CredentialError::Malformed), "{bad}");
            assert_eq!(
                fs::read_to_string(store.path()).unwrap(),
                bad,
                "never rewritten"
            );
            assert!(!err.to_string().contains("deviceSecret"));
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn credentials_only_serve_their_own_endpoint_class() {
        assert!(usable_for(
            Some(credential(EndpointClass::LocalTest)),
            EndpointClass::LocalTest
        )
        .is_some());
        assert!(
            usable_for(
                Some(credential(EndpointClass::LocalTest)),
                EndpointClass::Production
            )
            .is_none(),
            "a synthetic test identity never reaches production"
        );
        assert!(usable_for(
            Some(credential(EndpointClass::Production)),
            EndpointClass::LocalTest
        )
        .is_none());
        assert!(usable_for(None, EndpointClass::Production).is_none());
    }

    #[test]
    fn errors_and_debug_output_never_contain_the_secret() {
        let c = credential(EndpointClass::Production);
        let shown = format!(
            "{c:?} {:?} {}",
            CredentialError::Malformed,
            CredentialError::Io(io::ErrorKind::Other)
        );
        assert!(!shown.contains(SECRET), "{shown}");
        let mut mem = MemoryCredentialStore {
            fail_saves: true,
            ..Default::default()
        };
        let err = mem.save(&c).unwrap_err();
        assert!(!err.to_string().contains(SECRET));
    }

    #[test]
    fn backup_import_boundary_reads_only_synthetic_fixtures() {
        let fixture = include_str!("../../tests/fixtures/sanitized-production-backup.json");
        let id = identity_from_production_backup(fixture).unwrap();
        assert_eq!(
            id.user_id.as_str(),
            "fixture-user-id-not-real",
            "the fixture's obviously fake id"
        );
        assert_eq!(
            identity_from_production_backup("not json"),
            Err(BackupIdentityError::NotABackup)
        );
        assert_eq!(
            identity_from_production_backup("{\"sessions\":[],\"courses\":[]}"),
            Err(BackupIdentityError::NotABackup)
        );
        assert_eq!(
            identity_from_production_backup(
                "{\"sessions\":[],\"courses\":[],\"social\":{\"userId\":\"u\"}}"
            ),
            Err(BackupIdentityError::NoIdentity)
        );
        let bare = format!("{{\"sessions\":[],\"courses\":[],\"social\":{{\"userId\":\"synthetic-user\",\"deviceSecret\":\"{SECRET}\"}}}}");
        assert_eq!(
            identity_from_production_backup(&bare)
                .unwrap()
                .device_secret
                .expose(),
            SECRET
        );
    }
}
