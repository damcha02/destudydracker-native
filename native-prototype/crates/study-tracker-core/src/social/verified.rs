//! Verified study sessions (Stage 22b, decision D4): production's `App.tsx` effect that keeps a
//! server-witnessed session open while the Timer runs a study/exam/endless phase, as a pure state
//! machine.
//!
//! The Timer domain knows nothing about this. The application observes the Timer's
//! `(phase, running)` after each command/tick, feeds [`VerifiedMachine::observe`], performs the
//! [`Intent`]s it returns through the Social network worker, and reports the replies back. With
//! no Social identity the application never calls it, so no request can exist.
//!
//! Production semantics ported (the effect depends on `[configured, deviceSecret, userId,
//! timer.phase, timer.running]`; each change is one *generation*):
//! - **eligible** = running and the phase is neither idle nor break;
//! - becoming ineligible: forget the session and `finish` it (fire and forget);
//! - eligible with a session: (re)start the 15-minute heartbeat schedule, no immediate beat;
//! - eligible without one: `start`; a start that resolves after its generation ended, or after
//!   the Timer stopped, is `finish`ed at once (the orphan rule);
//! - every successful start/heartbeat **confirms the anchor** `{sessionId, confirmedAt}`
//!   (persisted); if the previous anchor is more than the 2-hour normal-credit grace old, the
//!   offline intervals between them are sent to `/verified-session/reconcile-offline` (one at a
//!   time);
//! - a heartbeat answered "Verified session not found." forgets the session and starts again;
//! - every other failure is ignored (no retry, no backoff) - the next heartbeat tick is the
//!   next attempt.
//!
//! Deliberate differences (documented in the Stage 22 doc, parity-debt register):
//! - production leaks the old `setInterval` when "not found" restarts a session (each restart
//!   adds one more heartbeat timer); native keeps exactly one schedule;
//! - production retries a failed start only on the browser's `online` event; native has no such
//!   event, so the application calls [`VerifiedMachine::connectivity_restored`] when another
//!   Social request succeeds after a network failure, and the heartbeat cadence re-attempts a
//!   start that failed for a network reason (never more often than every 15 minutes).

use serde::{Deserialize, Serialize};

use super::ids::VerifiedSessionId;
use super::time::SocialTimestamp;
use crate::timer::{ActiveSegment, TimerPhase, WallTimestamp};

/// `VERIFIED_SESSION_HEARTBEAT_MS` (client and Worker).
pub const HEARTBEAT_MS: i64 = 15 * 60 * 1000;
/// `VERIFIED_SESSION_NORMAL_CREDIT_GRACE_MS` (client and Worker): a gap up to this long is
/// credited normally by the server; only longer gaps are reconciled as offline time.
pub const NORMAL_CREDIT_GRACE_MS: i64 = 2 * 60 * 60 * 1000;
/// `MAX_OFFLINE_INTERVALS` (Worker); the client sends what it has, the Worker keeps 500.
pub const MAX_OFFLINE_INTERVALS: usize = 500;

/// The persisted `verifiedAnchor`: the last moment the server acknowledged a call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedAnchor {
    pub session_id: VerifiedSessionId,
    pub confirmed_at: SocialTimestamp,
}

/// The eligibility rule (also the tray's "hide instead of quit" rule).
pub fn eligible(phase: TimerPhase, running: bool) -> bool {
    running && !matches!(phase, TimerPhase::Idle | TimerPhase::Break)
}

/// One study interval claimed as offline time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub started_at: SocialTimestamp,
    pub ended_at: SocialTimestamp,
}

/// A study/exam session from the history (`state.sessions`), as `computeOfflineIntervals` reads it.
pub struct HistorySession {
    pub study_or_exam: bool,
    pub started_at: WallTimestamp,
    pub ended_at: WallTimestamp,
}

/// `computeOfflineIntervals(state, sinceIso, nowIso)`: every study/exam session that ended after
/// `since`, in history order, then the running Timer's segments (study/exam phase only), each
/// closed at `now` (`closeTimerSegments`) and kept when it ends after `since`. Unclamped - the
/// server clamps to its own gap boundary.
pub fn offline_intervals(
    history: &[HistorySession],
    timer_phase: TimerPhase,
    segments: &[ActiveSegment],
    since: SocialTimestamp,
    now: SocialTimestamp,
) -> Vec<Interval> {
    let mut out: Vec<Interval> = history
        .iter()
        .filter(|s| s.study_or_exam && s.ended_at.unix_millis > since.0)
        .map(|s| Interval {
            started_at: SocialTimestamp::from_wall(s.started_at),
            ended_at: SocialTimestamp::from_wall(s.ended_at),
        })
        .collect();
    if matches!(timer_phase, TimerPhase::Study | TimerPhase::Exam) {
        out.extend(
            segments
                .iter()
                .map(|seg| Interval {
                    started_at: SocialTimestamp::from_wall(seg.started_at),
                    ended_at: seg.ended_at.map_or(now, SocialTimestamp::from_wall),
                })
                .filter(|i| i.ended_at.0 > since.0),
        );
    }
    out
}

/// The canonical form both sides hash (`JSON.stringify(intervals.map(i => ({ startedAt,
/// endedAt })))`): ISO 8601 strings, keys in that order, no whitespace. The ISO strings contain
/// only digits, `-`, `:`, `.`, `T` and `Z`, so no JSON escaping is ever needed.
pub fn canonical_intervals(intervals: &[Interval]) -> String {
    let mut out = String::from("[");
    for (i, iv) in intervals.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"startedAt\":\"");
        out.push_str(&iv.started_at.to_iso());
        out.push_str("\",\"endedAt\":\"");
        out.push_str(&iv.ended_at.to_iso());
        out.push_str("\"}");
    }
    out.push(']');
    out
}

/// What the application must do next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    /// `POST /verified-session/start`, tagged with the generation that asked.
    Start { generation: u64 },
    /// `POST /verified-session/heartbeat`.
    Heartbeat { session_id: VerifiedSessionId },
    /// `POST /verified-session/finish` (reply ignored).
    Finish { session_id: VerifiedSessionId },
    /// (Re)start the single 15-minute heartbeat schedule.
    ScheduleHeartbeats,
    /// Stop the heartbeat schedule.
    StopHeartbeats,
    /// Persist the new anchor (and send the reconcile below when present).
    SaveAnchor(VerifiedAnchor),
    /// `POST /verified-session/reconcile-offline` for the previous anchor's gap.
    Reconcile {
        anchor_session_id: VerifiedSessionId,
        since: SocialTimestamp,
        until: SocialTimestamp,
    },
}

/// Why a verified-session call failed, as far as the machine cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// "Verified session not found." (404 with that text).
    NotFound,
    /// Offline / timeout: the network, not the server, said no.
    Network,
    /// Anything else (4xx/5xx/malformed): ignored like production.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VerifiedMachine {
    eligible: bool,
    generation: u64,
    session: Option<VerifiedSessionId>,
    anchor: Option<VerifiedAnchor>,
    reconciling: bool,
    /// A start is on the wire (so a heartbeat tick never starts a second one).
    starting: bool,
    /// The last start failed for a network reason; the heartbeat tick may retry it.
    start_failed_offline: bool,
    /// Diagnostics.
    pub starts: u64,
    pub heartbeats: u64,
    pub finishes: u64,
    pub reconciles: u64,
}

impl VerifiedMachine {
    /// `anchor`: the persisted `verifiedAnchor` (it survives restarts; a session id does not).
    pub fn new(anchor: Option<VerifiedAnchor>) -> Self {
        Self {
            anchor,
            ..Self::default()
        }
    }

    pub fn session(&self) -> Option<&VerifiedSessionId> {
        self.session.as_ref()
    }

    pub fn anchor(&self) -> Option<&VerifiedAnchor> {
        self.anchor.as_ref()
    }

    pub fn is_eligible(&self) -> bool {
        self.eligible
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    fn start(&mut self) -> Vec<Intent> {
        if self.starting {
            return Vec::new();
        }
        self.starting = true;
        self.start_failed_offline = false;
        self.starts += 1;
        vec![Intent::Start {
            generation: self.generation,
        }]
    }

    /// A dependency of production's effect changed (Timer phase/running, or the identity).
    /// Callers only call this on a real change; every call is a new generation.
    pub fn observe(&mut self, eligible: bool) -> Vec<Intent> {
        self.generation += 1;
        self.eligible = eligible;
        // the previous run's interval is always cleared by its cleanup
        let mut out = vec![Intent::StopHeartbeats];
        if !eligible {
            self.start_failed_offline = false;
            if let Some(id) = self.session.take() {
                self.finishes += 1;
                out.push(Intent::Finish { session_id: id });
            }
            return out;
        }
        if self.session.is_some() {
            out.push(Intent::ScheduleHeartbeats);
        } else {
            // a start from the previous generation may still be on the wire: production starts
            // again regardless (its `disposed` flag finishes the older one when it lands)
            self.starting = false;
            out.extend(self.start());
        }
        out
    }

    fn confirm(&mut self, session_id: VerifiedSessionId, now: SocialTimestamp) -> Vec<Intent> {
        let previous = self.anchor.take();
        let anchor = VerifiedAnchor {
            session_id,
            confirmed_at: now,
        };
        self.anchor = Some(anchor.clone());
        let mut out = vec![Intent::SaveAnchor(anchor)];
        if let Some(prev) = previous {
            if !self.reconciling && now.0 - prev.confirmed_at.0 > NORMAL_CREDIT_GRACE_MS {
                self.reconciling = true;
                self.reconciles += 1;
                out.push(Intent::Reconcile {
                    anchor_session_id: prev.session_id,
                    since: prev.confirmed_at,
                    until: now,
                });
            }
        }
        out
    }

    /// `/verified-session/start` answered `{ sessionId }`.
    pub fn start_succeeded(
        &mut self,
        generation: u64,
        session_id: VerifiedSessionId,
        now: SocialTimestamp,
    ) -> Vec<Intent> {
        if generation == self.generation {
            self.starting = false;
        }
        if generation != self.generation || !self.eligible {
            // the orphan rule: `disposed || !timerRunningRef.current`
            self.finishes += 1;
            return vec![Intent::Finish { session_id }];
        }
        self.session = Some(session_id.clone());
        let mut out = self.confirm(session_id, now);
        out.push(Intent::ScheduleHeartbeats);
        out
    }

    pub fn start_failed(&mut self, generation: u64, failure: Failure) {
        if generation == self.generation {
            self.starting = false;
            self.start_failed_offline = failure == Failure::Network && self.eligible;
        }
    }

    /// The 15-minute schedule fired. Production's interval only heartbeats; native also uses the
    /// tick to re-attempt a start that failed for a network reason (see the module docs).
    pub fn heartbeat_due(&mut self) -> Vec<Intent> {
        if !self.eligible {
            return Vec::new();
        }
        match &self.session {
            Some(id) => {
                self.heartbeats += 1;
                vec![Intent::Heartbeat {
                    session_id: id.clone(),
                }]
            }
            None if self.start_failed_offline => self.start(),
            None => Vec::new(),
        }
    }

    pub fn heartbeat_succeeded(
        &mut self,
        session_id: VerifiedSessionId,
        now: SocialTimestamp,
    ) -> Vec<Intent> {
        // production confirms with the session the heartbeat was sent for
        self.confirm(session_id, now)
    }

    pub fn heartbeat_failed(
        &mut self,
        session_id: &VerifiedSessionId,
        failure: Failure,
    ) -> Vec<Intent> {
        if failure != Failure::NotFound || self.session.as_ref() != Some(session_id) {
            return Vec::new();
        }
        self.session = None;
        if !self.eligible {
            return Vec::new();
        }
        let mut out = self.start();
        // the restarted session gets its own schedule once the start lands (one schedule only)
        if !out.is_empty() {
            out.insert(0, Intent::StopHeartbeats);
        }
        out
    }

    /// `attemptVerifiedSyncRef` (production: the `online` event): heartbeat now, or start.
    pub fn connectivity_restored(&mut self) -> Vec<Intent> {
        if !self.eligible {
            return Vec::new();
        }
        match &self.session {
            Some(id) => {
                self.heartbeats += 1;
                vec![Intent::Heartbeat {
                    session_id: id.clone(),
                }]
            }
            None if !self.starting => self.start(),
            None => Vec::new(),
        }
    }

    /// The reconcile call finished (success or failure: production clears the flag either way).
    pub fn reconcile_finished(&mut self) {
        self.reconciling = false;
    }

    /// Identity removed / shutdown: forget the in-memory session without any request (production
    /// sends nothing when the app exits; the Worker settles a silent session itself).
    pub fn reset(&mut self) {
        self.eligible = false;
        self.generation += 1;
        self.session = None;
        self.starting = false;
        self.start_failed_offline = false;
    }
}

/// The message after a reconcile capped the claim (`"N offline minute(s) could not be verified."`).
pub fn capped_message(capped_minutes: u64) -> Option<String> {
    (capped_minutes > 0).then(|| {
        format!(
            "{capped_minutes} offline minute{} could not be verified.",
            if capped_minutes == 1 { "" } else { "s" }
        )
    })
}
