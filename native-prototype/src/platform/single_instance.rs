//! Single-instance enforcement (Stage 18).
//!
//! **Mechanism.** A *named kernel mutex* decides who is the primary instance; activation of the
//! primary is a single *posted window message* to its hidden message-only window. No pipe, socket,
//! lock file, polling loop or server is involved, and no data (arguments, paths) ever crosses from
//! the secondary to the primary - the message carries nothing, so there is no untrusted input to
//! parse.
//!
//! ```text
//! launch -> CreateMutexW(Local\<app-id>.<profile-hash>.single-instance)
//!   created (no ERROR_ALREADY_EXISTS) -> Primary: keep the guard for the process lifetime
//!   already exists -> Secondary: find the primary's message window (retrying for a bounded time:
//!       the primary may still be starting), allow it to take the foreground, post
//!       WM_ACTIVATE_REQUEST, exit 0
//! ```
//!
//! *Identity.* The name is derived from `identity::APP_ID` (`com.damcha.studytracker.native-shell`,
//! deliberately different from production's `com.damcha.studytracker`, so the installed Tauri app
//! and the native prototype never block each other) **and a hash of the profile's data
//! directory**: the invariant being protected is "one writer per store", so two launches against
//! *different* data directories (isolated test/benchmark profiles via `STUDY_NATIVE_DATA_DIR`) are
//! independent, while two launches against the same directory - the only case that could corrupt a
//! store - are not. `Local\` scopes the object to the logon session.
//!
//! *Lifecycle / stale instances.* Kernel mutexes are released by the OS when the owning process
//! dies, however it dies, so there is no stale state to clean up. *Failure behaviour.* If the
//! mutex cannot be created for any reason other than "already exists", the app **starts anyway**
//! (a broken lock must not make the app unlaunchable) and logs a warning; if a secondary cannot
//! find a primary window within the timeout (primary hung, or still starting), it exits without
//! activating rather than starting a second writer.

use super::win_util::{fnv1a64, wide};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND, LPARAM, WPARAM,
};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, FindWindowExW, PostMessageW, ASFW_ANY, HWND_MESSAGE, WM_APP,
};

/// Posted by a secondary instance; meaning "show yourself".
pub const WM_ACTIVATE_REQUEST: u32 = WM_APP + 1;

/// The names derived for one profile (pure; unit-tested).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceNames {
    pub mutex: String,
    pub window_class: String,
}

pub fn names_for(app_id: &str, data_dir: &std::path::Path) -> InstanceNames {
    // Case-insensitive, separator-insensitive: the same directory spelled two ways is one profile.
    let canonical = data_dir
        .to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase();
    let hash = fnv1a64(canonical.as_bytes());
    InstanceNames {
        mutex: format!("Local\\{app_id}.{hash:016x}.single-instance"),
        window_class: format!("{app_id}.{hash:016x}.platform-window"),
    }
}

pub enum Acquisition {
    /// This process is the primary instance; hold the guard until exit.
    Primary(InstanceGuard),
    /// Another instance owns this profile; activation was requested (or timed out).
    Secondary { activated: bool },
    /// The mutex could not be created; proceed unguarded (logged).
    Unguarded,
}

/// Owns the mutex handle. Dropping closes it (the OS would anyway at process exit).
pub struct InstanceGuard {
    handle: HANDLE,
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateMutexW and is closed exactly once.
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

/// Tries to become the primary instance for `names`.
pub fn acquire(names: &InstanceNames) -> Acquisition {
    let name = wide(&names.mutex);
    // SAFETY: `name` is a valid NUL-terminated UTF-16 string that outlives the call; a null
    // security-attributes pointer requests the default descriptor.
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if handle.is_null() {
        // SAFETY: GetLastError is thread-local.
        let error = unsafe { GetLastError() };
        log::warn!(
            "single-instance mutex could not be created (error {error}); continuing unguarded"
        );
        return Acquisition::Unguarded;
    }
    // SAFETY: GetLastError is thread-local and read immediately after CreateMutexW.
    let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if !already_exists {
        return Acquisition::Primary(InstanceGuard { handle });
    }
    // We hold a second handle to the primary's mutex; it is not ours to keep.
    // SAFETY: valid handle from CreateMutexW, closed once.
    unsafe {
        CloseHandle(handle);
    }
    Acquisition::Secondary {
        activated: request_activation(names),
    }
}

/// Finds the primary's message-only window and posts the activation message. Retries for a
/// bounded time because a primary that just won the mutex may not have created its window yet
/// (the concurrent-start case). This is the only place any waiting happens, and only in the
/// short-lived secondary process.
fn request_activation(names: &InstanceNames) -> bool {
    const ACTIVATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);
    let class = wide(&names.window_class);
    let started = std::time::Instant::now();
    loop {
        // SAFETY: class is NUL-terminated; HWND_MESSAGE restricts the search to message-only windows.
        let hwnd: HWND = unsafe {
            FindWindowExW(
                HWND_MESSAGE,
                std::ptr::null_mut(),
                class.as_ptr(),
                std::ptr::null(),
            )
        };
        if !hwnd.is_null() {
            // A secondary launched by the user may let the primary take the foreground.
            // SAFETY: plain Win32 calls with valid arguments.
            unsafe {
                AllowSetForegroundWindow(ASFW_ANY);
                return PostMessageW(hwnd, WM_ACTIVATE_REQUEST, 0 as WPARAM, 0 as LPARAM) != 0;
            }
        }
        if started.elapsed() >= ACTIVATION_TIMEOUT {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn names_are_stable_per_profile_and_distinct_from_production_and_other_profiles() {
        let id = "com.damcha.studytracker.native-shell";
        let a = names_for(
            id,
            Path::new(r"C:\Users\x\AppData\Local\com.damcha.studytracker.native-shell"),
        );
        let same = names_for(
            id,
            Path::new("c:/users/X/appdata/local/com.damcha.studytracker.native-shell/"),
        );
        let other = names_for(id, Path::new(r"C:\temp\profile-b"));
        assert_eq!(
            a, same,
            "case/separator/trailing-slash differences are one profile"
        );
        assert_ne!(
            a.mutex, other.mutex,
            "different data directories are different profiles"
        );
        assert!(a
            .mutex
            .starts_with("Local\\com.damcha.studytracker.native-shell."));
        assert!(
            !a.mutex.contains("-sic"),
            "never production's plugin mutex name"
        );
        assert_ne!(a.mutex, a.window_class);
    }

    #[test]
    fn exactly_one_of_many_concurrent_acquirers_becomes_primary() {
        let name = format!("Local\\st18-test-{}.race", std::process::id());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(16));
        let results: Vec<(usize, bool)> = (0..16)
            .map(|_| {
                let (b, n) = (barrier.clone(), name.clone());
                std::thread::spawn(move || {
                    b.wait();
                    let wide_name = wide(&n);
                    // SAFETY: valid NUL-terminated name; last-error read right after the call.
                    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide_name.as_ptr()) };
                    let already = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
                    (handle as usize, already)
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect();
        let primaries = results
            .iter()
            .filter(|(h, already)| *h != 0 && !*already)
            .count();
        assert_eq!(primaries, 1, "exactly one creator observes a fresh mutex");
        for (handle, _) in results {
            if handle != 0 {
                // SAFETY: handles created above, closed once.
                unsafe { CloseHandle(handle as HANDLE) };
            }
        }
    }

    #[test]
    fn the_name_is_taken_while_the_primary_lives_and_free_again_after_it_exits() {
        let names = InstanceNames {
            mutex: format!("Local\\st18-test-{}.reacquire", std::process::id()),
            window_class: format!("st18-test-class-{}", std::process::id()),
        };
        let guard = match acquire(&names) {
            Acquisition::Primary(g) => g,
            _ => panic!("the first acquirer must be primary"),
        };
        let wide_name = wide(&names.mutex);
        // SAFETY: valid name; last-error read right after.
        let h = unsafe { CreateMutexW(std::ptr::null(), 0, wide_name.as_ptr()) };
        assert!(
            unsafe { GetLastError() } == ERROR_ALREADY_EXISTS,
            "while the primary lives, the name is taken"
        );
        unsafe { CloseHandle(h) };
        drop(guard);
        assert!(
            matches!(acquire(&names), Acquisition::Primary(_)),
            "after the primary is gone the name is free again (no stale lock)"
        );
    }
}
