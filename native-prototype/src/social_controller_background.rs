//! The Social controller's background work (Stage 22b): verified study sessions (D4), opt-in
//! telemetry (D3), app announcements and the owner-only usage / update-notice actions.
//!
//! Gating (decisions D2/D3, documented in the Stage 22 doc):
//! - verified sessions and announcements need an established identity: with `NoIdentity`
//!   nothing here ever builds a request;
//! - telemetry follows production exactly: it does not need an account, it is **off by
//!   default**, and only the user turning it on makes it send (`{ installId, app }`, never a
//!   credential);
//! - the owner actions are shown only for production's owner tag and the Worker refuses them for
//!   anyone else (403, shown as a message).

use sha2_ring::sha256_hex;
use study_tracker_core::social::announcement::{is_valid_app_version, Announcement, Dismissed};
use study_tracker_core::social::telemetry::InstallId;
use study_tracker_core::social::verified::{
    canonical_intervals, capped_message, offline_intervals, Failure, HistorySession, Intent,
    VerifiedMachine,
};
use study_tracker_core::social::{SocialTimestamp, VerifiedSessionId};
use study_tracker_core::timer::WallTimestamp;

use super::{Pending, SocialController, SyncContext};
use crate::net::http::{HttpResponse, NetError};
use crate::net::social_ext::{self, AdminUsage};
use crate::net_jobs::Outgoing;
use crate::persistence::social_port::SocialPrefs;

/// SHA-256 through the `ring` already linked by rustls (no new crate).
mod sha2_ring {
    pub fn sha256_hex(data: &[u8]) -> String {
        ring::digest::digest(&ring::digest::SHA256, data)
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackgroundPending {
    VerifiedStart { generation: u64 },
    VerifiedHeartbeat { session: VerifiedSessionId },
    VerifiedFinish,
    VerifiedReconcile,
    Announcement,
    UpdateNotice,
    Telemetry,
    AdminUsage,
}

#[derive(Debug)]
pub struct BackgroundState {
    pub verified: VerifiedMachine,
    /// The single heartbeat schedule the app glue keeps: on/off and a generation that changes
    /// whenever the machine asks for a fresh 15-minute interval.
    pub heartbeats_on: bool,
    pub heartbeat_generation: u64,
    pub telemetry_enabled: bool,
    install_id: Option<InstallId>,
    pub announcement: Option<Announcement>,
    dismissed: Dismissed,
    pub admin_usage: Option<AdminUsage>,
    pub admin_open: bool,
    pub admin_loading: bool,
    pub notice_sending: bool,
    /// Diagnostics.
    pub telemetry_sent: u64,
    pub announcements_polled: u64,
}

impl BackgroundState {
    pub fn new(prefs: SocialPrefs) -> Self {
        Self {
            verified: VerifiedMachine::default(),
            heartbeats_on: false,
            heartbeat_generation: 0,
            telemetry_enabled: prefs.telemetry_enabled,
            install_id: prefs.install_id.as_deref().and_then(InstallId::parse),
            announcement: None,
            dismissed: Dismissed::from_ids(prefs.dismissed_announcements),
            admin_usage: None,
            admin_open: false,
            admin_loading: false,
            notice_sending: false,
            telemetry_sent: 0,
            announcements_polled: 0,
        }
    }

    fn prefs(&self) -> SocialPrefs {
        SocialPrefs {
            telemetry_enabled: self.telemetry_enabled,
            install_id: self.install_id.as_ref().map(|i| i.as_str().to_string()),
            dismissed_announcements: self.dismissed.ids().to_vec(),
        }
    }
}

/// What the Timer looks like to the verified-session adapter (the Timer domain is not touched:
/// the application reads these values after each command and tick).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerView {
    pub phase: study_tracker_core::timer::TimerPhase,
    pub running: bool,
}

impl SocialController {
    fn send_bg(&mut self, p: BackgroundPending, request: crate::net::http::ApiRequest) -> Outgoing {
        self.send(Pending::Background(p), request)
    }

    fn save_prefs(&mut self) {
        let prefs = self.bg.prefs();
        self.port.save_prefs(&prefs);
    }

    // ------------------------------------------------------------------- verified sessions

    /// The Timer's `(phase, running)` changed, or the identity did. With no identity nothing is
    /// observed (and nothing sent).
    pub fn observe_timer(&mut self, timer: TimerView, ctx: &SyncContext) -> Vec<Outgoing> {
        if !self.active() {
            return Vec::new();
        }
        let eligible = study_tracker_core::social::verified::eligible(timer.phase, timer.running);
        let intents = self.bg.verified.observe(eligible);
        self.run_intents(intents, None, ctx)
    }

    /// The 15-minute heartbeat schedule fired.
    pub fn verified_tick(&mut self, ctx: &SyncContext) -> Vec<Outgoing> {
        if !self.active() {
            return Vec::new();
        }
        let intents = self.bg.verified.heartbeat_due();
        self.run_intents(intents, None, ctx)
    }

    pub(super) fn verified_connectivity_restored(&mut self) -> Vec<Outgoing> {
        if !self.active() {
            return Vec::new();
        }
        let intents = self.bg.verified.connectivity_restored();
        // no reconcile can come out of these intents, so no context is needed
        self.run_intents_simple(intents)
    }

    fn run_intents_simple(&mut self, intents: Vec<Intent>) -> Vec<Outgoing> {
        let mut out = Vec::new();
        for i in intents {
            out.extend(self.run_intent(i, None, None));
        }
        out
    }

    fn run_intents(
        &mut self,
        intents: Vec<Intent>,
        now: Option<WallTimestamp>,
        ctx: &SyncContext,
    ) -> Vec<Outgoing> {
        let mut out = Vec::new();
        for i in intents {
            out.extend(self.run_intent(i, now, Some(ctx)));
        }
        out
    }

    fn run_intent(
        &mut self,
        intent: Intent,
        now: Option<WallTimestamp>,
        ctx: Option<&SyncContext>,
    ) -> Vec<Outgoing> {
        let Some(id) = self.identity().cloned() else {
            return Vec::new();
        };
        match intent {
            Intent::Start { generation } => {
                let req = social_ext::verified_start(&id);
                vec![self.send_bg(BackgroundPending::VerifiedStart { generation }, req)]
            }
            Intent::Heartbeat { session_id } => {
                let req = social_ext::verified_heartbeat(&id, &session_id);
                vec![self.send_bg(
                    BackgroundPending::VerifiedHeartbeat {
                        session: session_id,
                    },
                    req,
                )]
            }
            Intent::Finish { session_id } => {
                let req = social_ext::verified_finish(&id, &session_id);
                vec![self.send_bg(BackgroundPending::VerifiedFinish, req)]
            }
            Intent::ScheduleHeartbeats => {
                self.bg.heartbeats_on = true;
                self.bg.heartbeat_generation += 1;
                Vec::new()
            }
            Intent::StopHeartbeats => {
                self.bg.heartbeats_on = false;
                Vec::new()
            }
            Intent::SaveAnchor(anchor) => {
                if let Some(r) = self.record.as_mut() {
                    r.verified_anchor = Some(anchor);
                }
                self.persist();
                Vec::new()
            }
            Intent::Reconcile {
                anchor_session_id,
                since,
                until,
            } => {
                let Some(ctx) = ctx else {
                    self.bg.verified.reconcile_finished();
                    return Vec::new();
                };
                let _ = now;
                let history: Vec<HistorySession> = ctx
                    .academic
                    .sessions
                    .iter()
                    .map(|s| HistorySession {
                        study_or_exam: matches!(
                            s.kind,
                            study_tracker_core::academic::SessionKind::Study
                                | study_tracker_core::academic::SessionKind::Exam
                        ),
                        started_at: s.started_at,
                        ended_at: s.ended_at,
                    })
                    .collect();
                let intervals =
                    offline_intervals(&history, ctx.timer_phase, ctx.timer_segments, since, until);
                if intervals.is_empty() {
                    // `if (!intervals.length) return;` (before the flag is set in production)
                    self.bg.verified.reconcile_finished();
                    return Vec::new();
                }
                let hash = sha256_hex(canonical_intervals(&intervals).as_bytes());
                let req =
                    social_ext::verified_reconcile(&id, &anchor_session_id, &intervals, &hash);
                vec![self.send_bg(BackgroundPending::VerifiedReconcile, req)]
            }
        }
    }

    // ------------------------------------------------------------------------- telemetry

    /// Start-up (`useEffect([telemetryEnabled])` on mount): one heartbeat if it is on.
    pub fn telemetry_startup(&mut self, random: [u8; 16]) -> Vec<Outgoing> {
        if self.bg.telemetry_enabled {
            self.telemetry_send(random)
        } else {
            Vec::new()
        }
    }

    /// The hourly schedule fired.
    pub fn telemetry_tick(&mut self, random: [u8; 16]) -> Vec<Outgoing> {
        self.telemetry_startup(random)
    }

    /// `toggleTelemetry`: saved, and turning it on sends at once (the effect re-runs).
    pub fn set_telemetry(&mut self, on: bool, random: [u8; 16]) -> Vec<Outgoing> {
        if self.bg.telemetry_enabled == on {
            return Vec::new();
        }
        self.bg.telemetry_enabled = on;
        self.save_prefs();
        if on {
            self.telemetry_send(random)
        } else {
            Vec::new()
        }
    }

    fn telemetry_send(&mut self, random: [u8; 16]) -> Vec<Outgoing> {
        if !self.configured() {
            return Vec::new();
        }
        if self.bg.install_id.is_none() {
            // `getTelemetryInstallId`: made once, kept
            self.bg.install_id = Some(InstallId::from_random(random));
            self.save_prefs();
        }
        let Some(install) = self.bg.install_id.clone() else {
            return Vec::new();
        };
        let app = crate::net::device::app_metadata();
        self.bg.telemetry_sent += 1;
        let req = social_ext::telemetry_heartbeat(&install, &app);
        vec![self.send_bg(BackgroundPending::Telemetry, req)]
    }

    // --------------------------------------------------------------------- announcements

    /// Start-up and every 2 minutes (`refreshAppAnnouncement`), only with an account (D2).
    pub fn poll_announcement(&mut self) -> Vec<Outgoing> {
        if self.identity().is_none() {
            return Vec::new();
        }
        let in_flight = self
            .pending
            .values()
            .any(|(p, _)| *p == Pending::Background(BackgroundPending::Announcement));
        if in_flight {
            self.coalesced += 1;
            return Vec::new();
        }
        self.bg.announcements_polled += 1;
        let req = social_ext::announcement_current(&crate::net::device::app_metadata());
        vec![self.send_bg(BackgroundPending::Announcement, req)]
    }

    /// `dismissAppAnnouncement`.
    pub fn dismiss_announcement(&mut self) {
        if let Some(a) = self.bg.announcement.take() {
            self.bg.dismissed.add(&a.id);
            self.save_prefs();
        }
    }

    // ------------------------------------------------------------------------- owner only

    /// `loadAdminUsage` (Settings > "Load usage data").
    pub fn load_admin_usage(&mut self) -> Vec<Outgoing> {
        if self.bg.admin_loading || !self.is_owner() {
            return Vec::new();
        }
        let Some(id) = self.identity().cloned() else {
            return Vec::new();
        };
        self.bg.admin_loading = true;
        let req = social_ext::admin_usage(&id);
        vec![self.send_bg(BackgroundPending::AdminUsage, req)]
    }

    pub fn close_admin_usage(&mut self) {
        self.bg.admin_open = false;
    }

    /// `sendUpdateNoticeToUsers` (asked first).
    pub fn send_update_notice(&mut self, version: &str, confirmed: bool) -> Vec<Outgoing> {
        if !self.is_owner() || self.bg.notice_sending {
            return Vec::new();
        }
        let version = version.trim().to_string();
        if !is_valid_app_version(&version) {
            self.say("Wait until the current app version is loaded before notifying users.");
            return Vec::new();
        }
        if !confirmed {
            self.squad.confirm = Some(super::squad::Confirm::UpdateNotice { version });
            return Vec::new();
        }
        let Some(id) = self.identity().cloned() else {
            return Vec::new();
        };
        self.bg.notice_sending = true;
        let req = social_ext::update_notice(&id, &version);
        vec![self.send_bg(BackgroundPending::UpdateNotice, req)]
    }

    // ------------------------------------------------------------------------------ replies

    pub(super) fn background_cancelled(&mut self, p: BackgroundPending) {
        match p {
            BackgroundPending::VerifiedStart { generation } => {
                self.bg.verified.start_failed(generation, Failure::Other)
            }
            BackgroundPending::VerifiedReconcile => self.bg.verified.reconcile_finished(),
            BackgroundPending::AdminUsage => self.bg.admin_loading = false,
            BackgroundPending::UpdateNotice => self.bg.notice_sending = false,
            _ => {}
        }
    }

    fn failure(err: &NetError) -> Failure {
        match err {
            NetError::NotFound(Some(m)) if m.contains("Verified session not found") => {
                Failure::NotFound
            }
            NetError::Offline | NetError::Timeout => Failure::Network,
            _ => Failure::Other,
        }
    }

    pub(super) fn background_reply(
        &mut self,
        p: BackgroundPending,
        result: Result<HttpResponse, NetError>,
        now: WallTimestamp,
        ctx: &SyncContext,
    ) -> Vec<Outgoing> {
        let ts = SocialTimestamp::from_wall(now);
        match p {
            BackgroundPending::VerifiedStart { generation } => {
                match result.and_then(|r| social_ext::parse_verified_start(&r)) {
                    Ok(session) => {
                        let intents = self.bg.verified.start_succeeded(generation, session, ts);
                        self.run_intents(intents, Some(now), ctx)
                    }
                    Err(err) => {
                        self.bg
                            .verified
                            .start_failed(generation, Self::failure(&err));
                        Vec::new()
                    }
                }
            }
            BackgroundPending::VerifiedHeartbeat { session } => {
                match result.and_then(|r| crate::net::social_api::parse_ok(&r)) {
                    Ok(()) => {
                        let intents = self.bg.verified.heartbeat_succeeded(session, ts);
                        self.run_intents(intents, Some(now), ctx)
                    }
                    Err(err) => {
                        let intents = self
                            .bg
                            .verified
                            .heartbeat_failed(&session, Self::failure(&err));
                        self.run_intents(intents, Some(now), ctx)
                    }
                }
            }
            // `finishVerifiedSession(...).catch(() => undefined)`: the reply changes nothing
            BackgroundPending::VerifiedFinish => Vec::new(),
            BackgroundPending::VerifiedReconcile => {
                self.bg.verified.reconcile_finished();
                if let Ok(capped) = result.and_then(|r| social_ext::parse_reconcile(&r)) {
                    if let Some(m) = capped_message(capped) {
                        self.say(m);
                    }
                }
                Vec::new()
            }
            BackgroundPending::Announcement => {
                // failures are only logged (production `console.warn`)
                match result.and_then(|r| social_ext::parse_announcement(&r)) {
                    Ok(Some(a)) if !self.bg.dismissed.contains(&a.id) => {
                        self.bg.announcement = Some(a)
                    }
                    Ok(_) => self.bg.announcement = None,
                    Err(err) => log::info!("social: announcement refresh failed ({})", err.kind()),
                }
                Vec::new()
            }
            BackgroundPending::Telemetry => {
                if let Err(err) = result.and_then(|r| crate::net::social_api::parse_ok(&r)) {
                    log::info!("social: telemetry heartbeat failed ({})", err.kind());
                }
                Vec::new()
            }
            BackgroundPending::AdminUsage => {
                self.bg.admin_loading = false;
                match result.and_then(|r| social_ext::parse_admin_usage(&r)) {
                    Ok(usage) => {
                        self.bg.admin_usage = Some(usage);
                        self.bg.admin_open = true;
                    }
                    Err(err) => self.say(err.user_message("Could not load usage data.")),
                }
                Vec::new()
            }
            BackgroundPending::UpdateNotice => {
                self.bg.notice_sending = false;
                match result.and_then(|r| social_ext::parse_update_notice(&r)) {
                    Ok(version) => {
                        self.bg.announcement = None;
                        self.say(format!("Update notice sent to users below {version}."));
                    }
                    Err(err) => {
                        self.say(err.user_message("Could not notify users about the update."))
                    }
                }
                Vec::new()
            }
        }
    }
}
