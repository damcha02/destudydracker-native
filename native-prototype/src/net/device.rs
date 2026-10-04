//! Device and app metadata sent with `/sync/v2` and `/presence` (Stage 22a): exact ports of
//! production's `get_device_identity` (`desktop/src-tauri/src/lib.rs`: FNV-1a 64 over
//! `study-tracker-social-device-v1\nos=..\narch=..\nmachine=..`, label `"{os} {arch} {channel}"`)
//! and `getAppMetadata()` (`version`, `navigator.platform`, `runtimeChannel`).
//!
//! The machine id is read once per sync and only its hash leaves the process, exactly as in
//! production. Nothing here is logged.

/// `fnv1a64(value)` as a 16-digit lower-case hex string.
pub fn fnv1a64(value: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// `stable_machine_id()` per platform, as production reads it.
pub fn stable_machine_id() -> String {
    #[cfg(target_os = "linux")]
    {
        for path in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
            if let Ok(value) = std::fs::read_to_string(path) {
                let value = value.trim();
                if !value.is_empty() {
                    return value.into();
                }
            }
        }
        std::env::var("HOSTNAME").unwrap_or_default()
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var("COMPUTERNAME").unwrap_or_default()
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|stdout| {
                stdout.lines().find_map(|line| {
                    let value = line
                        .split("IOPlatformUUID")
                        .nth(1)?
                        .split('=')
                        .nth(1)?
                        .trim();
                    Some(value.trim_matches('"').to_string())
                })
            })
            .unwrap_or_default()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        String::new()
    }
}

/// `runtime_channel()`: "development" (debug), "official-release" (the release flag), otherwise
/// "source-build".
pub fn runtime_channel() -> &'static str {
    if cfg!(debug_assertions) {
        "development"
    } else if option_env!("STUDY_TRACKER_OFFICIAL_RELEASE") == Some("1") {
        "official-release"
    } else {
        "source-build"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceIdentity {
    pub fingerprint_hash: String,
    pub label: String,
}

/// `get_device_identity()` for a given machine id.
pub fn device_identity_for(machine_id: &str) -> DeviceIdentity {
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    DeviceIdentity {
        fingerprint_hash: fnv1a64(&format!(
            "study-tracker-social-device-v1\nos={os}\narch={arch}\nmachine={machine_id}"
        )),
        label: format!("{os} {arch} {}", runtime_channel()),
    }
}

pub fn device_identity() -> DeviceIdentity {
    device_identity_for(&stable_machine_id())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppMetadata {
    pub version: String,
    pub platform: String,
    pub runtime_channel: String,
}

/// `navigator.platform` as the production WebView reports it.
pub fn navigator_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "Win32"
    } else if cfg!(target_os = "macos") {
        "MacIntel"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "Linux x86_64"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "Linux aarch64"
    } else {
        "unknown"
    }
}

/// `getAppMetadata()`. The version is the native build's own (`identity::version()`), which is
/// deliberately not production's version track.
pub fn app_metadata() -> AppMetadata {
    AppMetadata {
        version: crate::platform::identity::version().to_string(),
        platform: navigator_platform().to_string(),
        runtime_channel: runtime_channel().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a64_matches_the_reference_vectors() {
        // FNV-1a 64 test vectors (Fowler/Noll/Vo): "" and "a"
        assert_eq!(fnv1a64(""), "cbf29ce484222325");
        assert_eq!(fnv1a64("a"), "af63dc4c8601ec8c");
        assert_eq!(fnv1a64("foobar"), "85944171f73967e8");
    }

    #[test]
    fn device_identity_uses_productions_exact_source_string() {
        let id = device_identity_for("synthetic-machine");
        let want = fnv1a64(&format!(
            "study-tracker-social-device-v1\nos={}\narch={}\nmachine=synthetic-machine",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
        assert_eq!(id.fingerprint_hash, want);
        assert_eq!(id.fingerprint_hash.len(), 16);
        assert!(
            id.label.ends_with("development"),
            "test builds are debug builds"
        );
        assert_ne!(
            device_identity_for("other").fingerprint_hash,
            id.fingerprint_hash
        );
    }
}
