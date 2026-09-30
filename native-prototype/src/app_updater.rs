//! Application wiring for the updater (Stage 18): *when* to check, *where* the feed is, and how
//! results reach the UI. All decisions about versions, signatures and staging live in
//! `platform::updater` (pure, tested); this file only schedules and displays.
//!
//! Policy (mirrors production's `runtime_channel()`): updates are active only in an **official
//! release build** (`STUDY_TRACKER_OFFICIAL_RELEASE=1` at compile time) that also has a feed URL
//! compiled in (`STUDY_TRACKER_UPDATE_FEED_URL`). The native prototype has neither - there is no
//! native release feed yet - so by default the updater reports itself disabled and does nothing,
//! in particular it never contacts production's live feed (which would offer Tauri installers to a
//! native binary).
//!
//! Cadence (production: startup + every 24 h): one check shortly after launch, then one per day,
//! from `slint::Timer`s that sleep between firings - no polling. The network work runs on a
//! short-lived worker thread; results come back through `invoke_from_event_loop`.
//!
//! Diagnostic hooks (off by default): `STUDY_NATIVE_UPDATE_FEED_FILE=<path to a latest.json>` reads
//! the feed and artifacts from local files instead of the network (the trust root is still the
//! compiled-in key, so this cannot install anything unsigned); `STUDY_NATIVE_UPDATE_CHECK_NOW=1`
//! checks ~1 s after launch; `STUDY_NATIVE_UPDATE_AUTO_DOWNLOAD=1` stages an available update
//! without a click; in **debug builds only**, `STUDY_NATIVE_UPDATE_TEST_PUBKEY=<base64 .pub>`
//! replaces the trust root so throw-away test keys can be used.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, Timer, TimerMode};

use crate::platform::updater::http::{FileFetcher, WinHttpFetcher};
use crate::platform::updater::service::{
    check, default_platform_keys, download, install_staged, AvailableUpdate, CheckOutcome, Fetcher,
    StagedUpdate, UpdaterConfig,
};
use crate::platform::updater::verify::TrustRoot;
use crate::platform::updater::version::Version;
use crate::platform::updater::UpdateError;
use crate::MainWindow;

const FIRST_CHECK_DELAY: Duration = Duration::from_secs(15);
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// What the updater is doing, for the UI and for logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateUiState {
    Disabled(String),
    Idle,
    UpToDate,
    Available(AvailableUpdate),
    Downloading(Version),
    Staged(StagedUpdate),
    Failed(String),
}

enum Source {
    Network,
    Files(PathBuf),
}

struct Policy {
    feed_url: String,
    source: Source,
    trust_key_b64: String,
}

pub struct UpdateController {
    state: UpdateUiState,
    policy: Option<Policy>,
    data_dir: PathBuf,
    window: slint::Weak<MainWindow>,
    _timers: Vec<Timer>,
}

thread_local! {
    static CONTROLLER: RefCell<Option<Rc<RefCell<UpdateController>>>> = const { RefCell::new(None) };
}

fn decide_policy() -> Result<Policy, String> {
    if let Some(path) = std::env::var_os("STUDY_NATIVE_UPDATE_FEED_FILE") {
        let path = PathBuf::from(path);
        let base = path.parent().map(PathBuf::from).unwrap_or_default();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Ok(Policy {
            feed_url: format!("https://local-test-feed.invalid/{name}"),
            source: Source::Files(base),
            trust_key_b64: trust_key(),
        });
    }
    if cfg!(debug_assertions) {
        return Err("updates are disabled in development builds".into());
    }
    if option_env!("STUDY_TRACKER_OFFICIAL_RELEASE") != Some("1") {
        return Err("updates are disabled for source builds".into());
    }
    match option_env!("STUDY_TRACKER_UPDATE_FEED_URL") {
        Some(url) if !url.is_empty() => Ok(Policy {
            feed_url: url.to_string(),
            source: Source::Network,
            trust_key_b64: trust_key(),
        }),
        _ => Err("no native update feed is configured yet".into()),
    }
}

fn trust_key() -> String {
    #[cfg(debug_assertions)]
    if let Ok(test_key) = std::env::var("STUDY_NATIVE_UPDATE_TEST_PUBKEY") {
        return test_key;
    }
    crate::platform::updater::verify::PRODUCTION_PUBLIC_KEY.to_string()
}

fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("Cargo package version is valid semver")
}

fn config_for(policy: &Policy, data_dir: &std::path::Path) -> Result<UpdaterConfig, UpdateError> {
    Ok(UpdaterConfig {
        feed_url: policy.feed_url.clone(),
        trust: TrustRoot::from_config_public_key(&policy.trust_key_b64)?,
        current: current_version(),
        platform_keys: default_platform_keys(),
        data_dir: data_dir.to_path_buf(),
        allow_insecure_test_urls: false,
    })
}

fn fetcher_for(policy: &Policy) -> Box<dyn Fetcher + Send> {
    match &policy.source {
        Source::Network => Box::new(WinHttpFetcher::new()),
        Source::Files(dir) => Box::new(FileFetcher {
            base_dir: dir.clone(),
        }),
    }
}

/// Installs the updater: decides the policy, schedules the checks, binds the notice callbacks.
pub fn install(window: &MainWindow, data_dir: PathBuf) {
    let (policy, state) = match decide_policy() {
        Ok(policy) => (Some(policy), UpdateUiState::Idle),
        Err(reason) => {
            log::info!("updater: {reason}");
            (None, UpdateUiState::Disabled(reason))
        }
    };
    let controller = Rc::new(RefCell::new(UpdateController {
        state,
        policy,
        data_dir,
        window: window.as_weak(),
        _timers: Vec::new(),
    }));

    if controller.borrow().policy.is_some() {
        let first = Timer::default();
        let delay = if std::env::var_os("STUDY_NATIVE_UPDATE_CHECK_NOW").is_some() {
            Duration::from_secs(1)
        } else {
            FIRST_CHECK_DELAY
        };
        first.start(TimerMode::SingleShot, delay, || start_check());
        let daily = Timer::default();
        daily.start(TimerMode::Repeated, CHECK_INTERVAL, || start_check());
        controller.borrow_mut()._timers.extend([first, daily]);
    }

    window.on_update_notice_dismiss(|| {
        with_controller(|c| {
            if !matches!(c.state, UpdateUiState::Downloading(_)) {
                hide_notice(c);
            }
        });
    });
    window.on_update_notice_action(|| start_download());
    CONTROLLER.with(|slot| *slot.borrow_mut() = Some(controller));
}

fn with_controller<R>(f: impl FnOnce(&mut UpdateController) -> R) -> Option<R> {
    let rc = CONTROLLER.with(|slot| slot.borrow().clone())?;
    let mut guard = rc.borrow_mut();
    Some(f(&mut guard))
}

fn hide_notice(c: &UpdateController) {
    if let Some(window) = c.window.upgrade() {
        window.set_update_notice_visible(false);
    }
}

fn show_notice(c: &UpdateController, title: &str, body: &str, action: &str) {
    if let Some(window) = c.window.upgrade() {
        window.set_update_notice_title(title.into());
        window.set_update_notice_body(body.into());
        window.set_update_notice_action_label(action.into());
        window.set_update_notice_visible(true);
    }
}

fn set_state(c: &mut UpdateController, state: UpdateUiState) {
    // Never log the feed's signature/URL blobs; the state name and version are enough.
    match &state {
        UpdateUiState::Available(u) => log::info!("updater: available {}", u.version),
        UpdateUiState::Downloading(v) => log::info!("updater: downloading {v}"),
        UpdateUiState::Staged(s) => {
            log::info!("updater: staged {} (signature verified)", s.version)
        }
        other => log::info!("updater: {other:?}"),
    }
    c.state = state;
}

/// Runs a check on a worker thread.
fn start_check() {
    let Some((policy_config, fetcher)) = with_controller(|c| {
        let policy = c.policy.as_ref()?;
        match config_for(policy, &c.data_dir) {
            Ok(config) => Some((config, fetcher_for(policy))),
            Err(error) => {
                set_state(c, UpdateUiState::Failed(error.to_string()));
                None
            }
        }
    })
    .flatten() else {
        return;
    };
    std::thread::spawn(move || {
        let outcome = check(&policy_config, fetcher.as_ref());
        let _ = slint::invoke_from_event_loop(move || on_check_finished(outcome));
    });
}

fn on_check_finished(outcome: Result<CheckOutcome, UpdateError>) {
    let auto_download = std::env::var_os("STUDY_NATIVE_UPDATE_AUTO_DOWNLOAD").is_some();
    let mut download_now = false;
    with_controller(|c| match outcome {
        Ok(CheckOutcome::UpToDate { feed_version }) => {
            log::info!(
                "updater: up to date (feed offers {feed_version}, running {})",
                current_version()
            );
            set_state(c, UpdateUiState::UpToDate);
        }
        Ok(CheckOutcome::Available(update)) => {
            // Production: an *automatic* check that finds an update shows the "New update available"
            // notice; installing is a separate, user-initiated step.
            show_notice(
                c,
                "New update available.",
                &format!("Version {} is available.", update.version),
                "Download",
            );
            set_state(c, UpdateUiState::Available(update));
            download_now = auto_download;
        }
        Err(error) => {
            // Silent for the user (production logs automatic failures and shows nothing).
            log::warn!("updater: automatic check failed: {error}");
            set_state(c, UpdateUiState::Failed(error.to_string()));
        }
    });
    if download_now {
        start_download();
    }
}

/// The notice's "Download" button (or the auto-download hook): stage + verify on a worker thread.
fn start_download() {
    let Some((config, fetcher, update)) = with_controller(|c| {
        let UpdateUiState::Available(update) = c.state.clone() else {
            return None;
        };
        let policy = c.policy.as_ref()?;
        let config = config_for(policy, &c.data_dir).ok()?;
        let fetcher = fetcher_for(policy);
        show_notice(
            c,
            "Downloading update...",
            &format!("Version {}", update.version),
            "",
        );
        set_state(c, UpdateUiState::Downloading(update.version.clone()));
        Some((config, fetcher, update))
    })
    .flatten() else {
        return;
    };
    std::thread::spawn(move || {
        let result = download(&config, fetcher.as_ref(), &update);
        let _ = slint::invoke_from_event_loop(move || on_download_finished(result));
    });
}

fn on_download_finished(result: Result<StagedUpdate, UpdateError>) {
    with_controller(|c| match result {
        Ok(staged) => {
            // Installing is Stage 23 (packaging); say so plainly instead of pretending.
            let body = match install_staged(&staged) {
                Err(UpdateError::InstallNotSupported) => format!(
                    "Version {} was downloaded and its signature verified. Installing from inside the app arrives with the installer.",
                    staged.version
                ),
                _ => format!("Version {} is ready to install.", staged.version),
            };
            show_notice(c, "Update downloaded.", &body, "");
            set_state(c, UpdateUiState::Staged(staged));
        }
        Err(error) => {
            log::warn!("updater: download rejected: {error}");
            show_notice(c, "Update failed.", &error.to_string(), "");
            set_state(c, UpdateUiState::Failed(error.to_string()));
        }
    });
}
