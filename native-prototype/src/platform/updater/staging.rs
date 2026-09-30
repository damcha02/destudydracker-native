//! Safe staging of a *verified* update artifact.
//!
//! Layout (under the app's own data directory, never a shared/temp location):
//!
//! ```text
//! <data_dir>/updates/<version>/Study-Tracker-<version>-windows-x86_64-setup.exe
//! ```
//!
//! * The file name is built from the **validated** [`Version`] only (its `Display` admits just
//!   `[0-9A-Za-z.-]`), never from the feed's URL or from any server-supplied text, so a hostile
//!   manifest cannot choose a path (no traversal, no device names, no separators).
//! * Bytes are written to `<name>.part` and atomically renamed, so a crash or power loss can never
//!   leave a half-written file under the final name; stale `.part` files are removed.
//! * Only the newest staged version is kept.
//! * Staging happens only for artifacts whose signature already verified in memory; this module
//!   never verifies, never executes, never changes permissions.

use std::fs;
use std::path::{Path, PathBuf};

use super::version::Version;
use super::UpdateError;

pub const ARTIFACT_KIND: &str = "windows-x86_64-setup.exe";

pub fn staged_dir(data_dir: &Path, version: &Version) -> PathBuf {
    data_dir.join("updates").join(version.to_string())
}

pub fn staged_path(data_dir: &Path, version: &Version) -> PathBuf {
    staged_dir(data_dir, version).join(format!("Study-Tracker-{version}-{ARTIFACT_KIND}"))
}

/// Writes `bytes` (already verified) to the staged location. Returns the final path.
pub fn stage(data_dir: &Path, version: &Version, bytes: &[u8]) -> Result<PathBuf, UpdateError> {
    let dir = staged_dir(data_dir, version);
    let final_path = staged_path(data_dir, version);
    let part_path = final_path.with_extension("exe.part");
    let io = |what: &str, e: std::io::Error| UpdateError::Staging(format!("{what}: {e}"));

    fs::create_dir_all(&dir).map_err(|e| io("create directory", e))?;
    // Defense in depth: whatever the path construction did, the result must still live inside
    // `<data_dir>/updates`.
    let updates_root = fs::canonicalize(data_dir.join("updates"))
        .map_err(|e| io("resolve updates directory", e))?;
    let resolved_dir = fs::canonicalize(&dir).map_err(|e| io("resolve staging directory", e))?;
    if !resolved_dir.starts_with(&updates_root) {
        return Err(UpdateError::Staging(
            "staging directory escaped the updates directory".into(),
        ));
    }
    // A stale partial or an already-staged identical version is simply replaced.
    let _ = fs::remove_file(&part_path);
    fs::write(&part_path, bytes).map_err(|e| io("write", e))?;
    if final_path.exists() {
        fs::remove_file(&final_path).map_err(|e| io("replace existing", e))?;
    }
    fs::rename(&part_path, &final_path).map_err(|e| io("finalize", e))?;
    cleanup_other_versions(data_dir, version);
    Ok(final_path)
}

/// Removes staged versions other than `keep` (best effort).
pub fn cleanup_other_versions(data_dir: &Path, keep: &Version) {
    let Ok(entries) = fs::read_dir(data_dir.join("updates")) else {
        return;
    };
    let keep_name = keep.to_string();
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy() != keep_name {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("st18-staging-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn the_staged_name_comes_only_from_the_version() {
        let dir = PathBuf::from("data");
        let path = staged_path(&dir, &v("0.2.0"));
        assert_eq!(
            path,
            PathBuf::from("data/updates/0.2.0/Study-Tracker-0.2.0-windows-x86_64-setup.exe")
        );
    }

    #[test]
    fn staging_is_atomic_and_keeps_only_the_newest_version() {
        let dir = temp_dir("atomic");
        let first = stage(&dir, &v("0.2.0"), b"first").unwrap();
        assert_eq!(fs::read(&first).unwrap(), b"first");
        assert!(
            !first.with_extension("exe.part").exists(),
            "no partial file is left behind"
        );

        let second = stage(&dir, &v("0.3.0"), b"second").unwrap();
        assert_eq!(fs::read(&second).unwrap(), b"second");
        assert!(!first.exists(), "the older staged version is removed");
        assert!(second.exists());

        // Re-staging the same version replaces it cleanly.
        let again = stage(&dir, &v("0.3.0"), b"third").unwrap();
        assert_eq!(fs::read(&again).unwrap(), b"third");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stale_partial_from_a_crash_is_overwritten() {
        let dir = temp_dir("stale");
        let final_path = staged_path(&dir, &v("1.0.0"));
        fs::create_dir_all(final_path.parent().unwrap()).unwrap();
        fs::write(
            final_path.with_extension("exe.part"),
            b"half-written garbage",
        )
        .unwrap();
        let staged = stage(&dir, &v("1.0.0"), b"complete").unwrap();
        assert_eq!(fs::read(staged).unwrap(), b"complete");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hostile_version_text_can_never_become_a_path() {
        // Version::parse is the only way to obtain a Version, and it refuses all of these.
        for hostile in [
            "..\\..\\windows\\system32\\evil",
            "../../x",
            "1.0.0/../../x",
            "1.0.0\\x",
            "C:\\x",
            "1.0.0:stream",
            "CON",
            "1.0.0\u{0}",
        ] {
            assert!(Version::parse(hostile).is_none(), "{hostile:?}");
        }
    }

    #[test]
    fn staging_fails_cleanly_when_the_data_directory_is_unusable() {
        let dir = temp_dir("blocked");
        // `updates` exists as a *file*: create_dir_all must fail and report, not panic.
        fs::write(dir.join("updates"), b"i am a file").unwrap();
        let result = stage(&dir, &v("0.2.0"), b"x");
        assert!(matches!(result, Err(UpdateError::Staging(_))), "{result:?}");
        let _ = fs::remove_dir_all(&dir);
    }
}
