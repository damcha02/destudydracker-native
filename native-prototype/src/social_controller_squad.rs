//! The Squad part of the Social controller (Stage 22b): production's `submitSquadCreate`,
//! `submitSquadSearch`, `loadSquadSuggestions`, `joinOrRequestSquad`, `answerSquadRequest`,
//! `leaveCurrentSquad`, `submitSquadChat`, `deleteOwnSquadMessage`, `changeSquadMemberRole`,
//! `kickFromSquad`, `submitSquadSettings`, `openSquadDetails`, `joinOrRequestViewedSquad` and
//! `refreshSquadScoreboard`.
//!
//! Every membership mutation answers with the full social snapshot, which replaces the local
//! lists (`applySocialSnapshot`). The role checks here only choose the controls; the Worker
//! decides, and its refusal ("You cannot kick that member.") is shown like any error - a stale
//! local role can never do more than ask.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use study_tracker_core::social::leaderboard::LeaderboardScope;
use study_tracker_core::social::squad::{
    clean_message, clean_squad_name, pick_suggestions, SquadAction, SquadDetails, SquadMessage,
    SquadRole, SquadScoreEntry, SquadScorePeriod, SquadSearchResult, SCOREBOARD_CACHE_MS,
};
use study_tracker_core::social::time::{next_auto_sync_at, SocialTimestamp};
use study_tracker_core::social::{MessageId, RequestId, SquadId, UserId};
use study_tracker_core::timer::WallTimestamp;

use super::{Pending, SocialController, Subtab, SyncContext};
use crate::net::http::{HttpResponse, NetError};
use crate::net::social_api;
use crate::net::social_ext;
use crate::net_jobs::Outgoing;

/// A `window.confirm` production asks before a destructive step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confirm {
    /// "You are the last member. Leaving will delete this squad. Continue?"
    LeaveLast,
    /// "Kick <name> from the squad?"
    Kick { user: UserId, name: String },
    /// "Delete this message?"
    DeleteMessage { id: MessageId },
    /// "Notify users below <version> about the update?" (owner)
    UpdateNotice { version: String },
}

impl Confirm {
    pub fn text(&self) -> String {
        match self {
            Self::LeaveLast => {
                "You are the last member. Leaving will delete this squad. Continue?".into()
            }
            Self::Kick { name, .. } => format!("Kick {name} from the squad?"),
            Self::DeleteMessage { .. } => "Delete this message?".into(),
            Self::UpdateNotice { version } => {
                format!("Notify users below version {version} about the update?")
            }
        }
    }
}

/// The squad details dialog (`viewingSquadEntry` / `viewingSquadDetails`).
#[derive(Debug, Clone, PartialEq)]
pub struct SquadViewing {
    pub entry: SquadScoreEntry,
    pub details: Option<SquadDetails>,
    pub loading: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SquadMutation {
    Create,
    Join { squad: SquadId, private: bool },
    JoinViewed { squad: SquadId, private: bool },
    Respond { accept: bool },
    Leave { last: bool },
    Chat { body: String },
    DeleteMessage,
    Role,
    Kick,
    Settings,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SquadPending {
    Mutation(SquadMutation),
    Search {
        suggestions: bool,
    },
    Details {
        squad: SquadId,
        after_join: Option<bool>,
    },
    Scoreboard {
        period: SquadScorePeriod,
    },
}

#[derive(Debug)]
pub struct SquadState {
    /// `squadMessages` (newest 60, oldest first). Memory only.
    pub messages: Vec<SquadMessage>,
    /// `cachedSquadScoreLeaderboards`. Memory only.
    pub scores: HashMap<SquadScorePeriod, Vec<SquadScoreEntry>>,
    fetched_at: HashMap<SquadScorePeriod, Instant>,
    pub score_period: SquadScorePeriod,
    pub name_draft: String,
    pub private_draft: bool,
    pub search_draft: String,
    pub results: Vec<SquadSearchResult>,
    pub searching: bool,
    pub suggestions: Vec<SquadSearchResult>,
    pool: Vec<SquadSearchResult>,
    pub suggestions_loading: bool,
    /// One initial suggestion load per Squad-tab visit (see `maybe_load_suggestions`).
    suggestions_tried: bool,
    pub settings_editing: bool,
    pub settings_name: String,
    pub settings_private: bool,
    pub chat_draft: String,
    pub expanded_member: Option<UserId>,
    pub viewing: Option<SquadViewing>,
    pub confirm: Option<Confirm>,
    rng: u64,
    /// A pinned `Math.random()` (parity captures): every pick is `floor(r * (i + 1))`.
    pinned: Option<f64>,
}

impl Default for SquadState {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            scores: HashMap::new(),
            fetched_at: HashMap::new(),
            score_period: SquadScorePeriod::default(),
            name_draft: String::new(),
            private_draft: false,
            search_draft: String::new(),
            results: Vec::new(),
            searching: false,
            suggestions: Vec::new(),
            pool: Vec::new(),
            suggestions_loading: false,
            suggestions_tried: false,
            settings_editing: false,
            settings_name: String::new(),
            settings_private: false,
            chat_draft: String::new(),
            expanded_member: None,
            viewing: None,
            confirm: None,
            rng: 0x9e37_79b9_7f4a_7c15,
            pinned: None,
        }
    }
}

impl SquadState {
    /// Seeds the suggestion shuffle (`Math.random()`); tests keep the default seed.
    pub fn seed(&mut self, seed: u64) {
        self.rng = seed | 1;
    }

    /// Pins the shuffle like a capture's pinned `Math.random()` (`STUDY_NATIVE_BREAK_PICK`).
    pub fn pin(&mut self, r: f64) {
        self.pinned = Some(r.clamp(0.0, 0.999_999));
    }

    fn random(&mut self, upto: usize) -> usize {
        if let Some(r) = self.pinned {
            return (r * (upto as f64 + 1.0)).floor() as usize;
        }
        // xorshift64*
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        let x = self.rng.wrapping_mul(0x2545_f491_4f6c_dd1d);
        (x % (upto as u64 + 1)) as usize
    }

    fn reshuffle(&mut self) {
        let pool = self.pool.clone();
        self.suggestions = pick_suggestions(&pool, |i| self.random(i));
    }

    pub fn has_pool_to_reload(&self) -> bool {
        self.pool.len() > 4
    }

    fn mark_pending(&mut self, squad: &SquadId) {
        for list in [&mut self.suggestions, &mut self.pool, &mut self.results] {
            for s in list.iter_mut().filter(|s| &s.id == squad) {
                s.mark_pending();
            }
        }
    }
}

impl SocialController {
    pub fn current_squad(&self) -> Option<&study_tracker_core::social::squad::Squad> {
        self.record.as_ref().and_then(|r| r.squad.as_ref())
    }

    fn send_squad(&mut self, p: SquadPending, request: crate::net::http::ApiRequest) -> Outgoing {
        self.send(Pending::Squad(p), request)
    }

    fn mutate(
        &mut self,
        m: SquadMutation,
        busy: bool,
        build: impl FnOnce(&study_tracker_core::social::SocialIdentity) -> crate::net::http::ApiRequest,
    ) -> Vec<Outgoing> {
        let Some(id) = self.identity().cloned() else {
            return Vec::new();
        };
        if !self.active() {
            return Vec::new();
        }
        if busy {
            self.syncing = true;
        }
        let req = build(&id);
        vec![self.send_squad(SquadPending::Mutation(m), req)]
    }

    /// A membership action is already on the wire (production disables those buttons with
    /// `socialSyncing`); a second click is ignored rather than sent twice.
    fn busy(&self) -> bool {
        self.syncing
    }

    /// `submitSquadCreate`.
    pub fn create_squad(&mut self) -> Vec<Outgoing> {
        if self.busy() {
            return Vec::new();
        }
        let Some(name) = clean_squad_name(&self.squad.name_draft) else {
            self.say("Name your squad first.");
            return Vec::new();
        };
        let private = self.squad.private_draft;
        self.mutate(SquadMutation::Create, true, |id| {
            social_ext::squad_create(id, &name, private)
        })
    }

    /// `submitSquadSearch`.
    pub fn search_squads(&mut self) -> Vec<Outgoing> {
        if !self.active() || self.squad.searching {
            return Vec::new();
        }
        let Some(id) = self.identity().cloned() else {
            return Vec::new();
        };
        self.squad.searching = true;
        let query = self.squad.search_draft.trim().to_string();
        let req = social_ext::squad_search(&id, &query);
        vec![self.send_squad(SquadPending::Search { suggestions: false }, req)]
    }

    /// The suggestions effect: on the Squad subtab without a squad, load the pool once.
    /// (Production re-runs its effect whenever loading ends with no suggestions, which refetches
    /// without end when there are no squads or the server fails; native tries once per visit.)
    pub(super) fn maybe_load_suggestions(&mut self) -> Vec<Outgoing> {
        if !self.active() || !self.tab_visible || self.subtab != Subtab::Squad {
            self.squad.suggestions_tried = false;
            return Vec::new();
        }
        if self.current_squad().is_some()
            || !self.squad.suggestions.is_empty()
            || self.squad.suggestions_loading
            || self.squad.suggestions_tried
        {
            return Vec::new();
        }
        self.squad.suggestions_tried = true;
        self.load_suggestions(true)
    }

    /// `loadSquadSuggestions({ forceFetch })`: "Reload" reshuffles the pool it already has.
    pub fn load_suggestions(&mut self, force: bool) -> Vec<Outgoing> {
        if !self.active() || self.current_squad().is_some() {
            return Vec::new();
        }
        if !force && !self.squad.pool.is_empty() {
            self.squad.reshuffle();
            return Vec::new();
        }
        let Some(id) = self.identity().cloned() else {
            return Vec::new();
        };
        self.squad.suggestions_loading = true;
        let req = social_ext::squad_search(&id, "");
        vec![self.send_squad(SquadPending::Search { suggestions: true }, req)]
    }

    /// `joinOrRequestSquad` (a suggestion or search card).
    pub fn join_squad(&mut self, squad: &SquadId) -> Vec<Outgoing> {
        if self.busy() {
            return Vec::new();
        }
        let private = self
            .squad
            .suggestions
            .iter()
            .chain(self.squad.results.iter())
            .chain(self.squad.pool.iter())
            .find(|s| &s.id == squad)
            .map(|s| s.is_private);
        let Some(private) = private else {
            return Vec::new();
        };
        let target = squad.clone();
        self.mutate(
            SquadMutation::Join {
                squad: squad.clone(),
                private,
            },
            true,
            |id| social_ext::squad_join(id, &target),
        )
    }

    /// `answerSquadRequest`.
    pub fn answer_squad_request(&mut self, request: &RequestId, accept: bool) -> Vec<Outgoing> {
        if self.busy() {
            return Vec::new();
        }
        let request = request.clone();
        self.mutate(SquadMutation::Respond { accept }, true, |id| {
            social_ext::squad_respond(id, &request, accept)
        })
    }

    /// `leaveCurrentSquad` (the last member is asked first).
    pub fn leave_squad(&mut self, confirmed: bool) -> Vec<Outgoing> {
        let Some(sq) = self.current_squad() else {
            return Vec::new();
        };
        let last = sq.is_last_member();
        if last && !confirmed {
            self.squad.confirm = Some(Confirm::LeaveLast);
            return Vec::new();
        }
        if self.busy() {
            return Vec::new();
        }
        self.mutate(SquadMutation::Leave { last }, true, social_ext::squad_leave)
    }

    /// `submitSquadChat`: the draft is cleared at once (so a fast second Enter sends nothing)
    /// and restored if the server refused.
    pub fn send_chat(&mut self) -> Vec<Outgoing> {
        let Some(body) = clean_message(&self.squad.chat_draft) else {
            return Vec::new();
        };
        self.squad.chat_draft.clear();
        let text = body.clone();
        self.mutate(SquadMutation::Chat { body }, false, |id| {
            social_ext::squad_chat(id, &text)
        })
    }

    /// `deleteOwnSquadMessage` (asked first).
    pub fn delete_message(&mut self, message: &MessageId, confirmed: bool) -> Vec<Outgoing> {
        if !confirmed {
            self.squad.confirm = Some(Confirm::DeleteMessage {
                id: message.clone(),
            });
            return Vec::new();
        }
        let message = message.clone();
        self.mutate(SquadMutation::DeleteMessage, false, |id| {
            social_ext::squad_chat_delete(id, &message)
        })
    }

    /// `changeSquadMemberRole`.
    pub fn change_role(&mut self, target: &UserId, role: SquadRole) -> Vec<Outgoing> {
        if self.busy() || role == SquadRole::Leader {
            return Vec::new();
        }
        let target = target.clone();
        self.mutate(SquadMutation::Role, true, |id| {
            social_ext::squad_role(id, &target, role)
        })
    }

    /// `kickFromSquad` (asked first).
    pub fn kick(&mut self, target: &UserId, name: &str, confirmed: bool) -> Vec<Outgoing> {
        if !confirmed {
            self.squad.confirm = Some(Confirm::Kick {
                user: target.clone(),
                name: name.to_string(),
            });
            return Vec::new();
        }
        if self.busy() {
            return Vec::new();
        }
        let target = target.clone();
        self.mutate(SquadMutation::Kick, true, |id| {
            social_ext::squad_kick(id, &target)
        })
    }

    /// `startSquadSettingsEdit`.
    pub fn start_squad_settings(&mut self) {
        if let Some(sq) = self.current_squad().cloned() {
            self.squad.settings_name = sq.name;
            self.squad.settings_private = sq.is_private;
            self.squad.settings_editing = true;
        }
    }

    /// `submitSquadSettings`.
    pub fn save_squad_settings(&mut self) -> Vec<Outgoing> {
        if self.busy() {
            return Vec::new();
        }
        let Some(name) = clean_squad_name(&self.squad.settings_name) else {
            self.say("Name your squad first.");
            return Vec::new();
        };
        let private = self.squad.settings_private;
        self.mutate(SquadMutation::Settings, true, |id| {
            social_ext::squad_settings(id, &name, private)
        })
    }

    /// The confirmation dialog was answered.
    pub fn answer_confirm(&mut self, yes: bool) -> Vec<Outgoing> {
        let Some(c) = self.squad.confirm.take() else {
            return Vec::new();
        };
        if !yes {
            return Vec::new();
        }
        match c {
            Confirm::LeaveLast => self.leave_squad(true),
            Confirm::Kick { user, name } => self.kick(&user, &name, true),
            Confirm::DeleteMessage { id } => self.delete_message(&id, true),
            Confirm::UpdateNotice { version } => self.send_update_notice(&version, true),
        }
    }

    /// `openSquadDetails` (a Squad Arena row).
    pub fn open_squad_details(&mut self, squad: &SquadId) -> Vec<Outgoing> {
        let Some(entry) = self
            .squad
            .scores
            .get(&self.squad.score_period)
            .and_then(|rows| rows.iter().find(|e| &e.squad_id == squad))
            .cloned()
        else {
            return Vec::new();
        };
        let Some(id) = self.identity().cloned() else {
            return Vec::new();
        };
        self.squad.viewing = Some(SquadViewing {
            entry,
            details: None,
            loading: true,
        });
        let req = social_ext::squad_details(&id, squad);
        vec![self.send_squad(
            SquadPending::Details {
                squad: squad.clone(),
                after_join: None,
            },
            req,
        )]
    }

    pub fn close_squad_details(&mut self) {
        self.squad.viewing = None;
    }

    /// `joinOrRequestViewedSquad`.
    pub fn join_viewed_squad(&mut self) -> Vec<Outgoing> {
        let Some(d) = self.squad.viewing.as_ref().and_then(|v| v.details.clone()) else {
            return Vec::new();
        };
        if !d.action.can_join_or_request() || self.busy() {
            return Vec::new();
        }
        let target = d.id.clone();
        self.mutate(
            SquadMutation::JoinViewed {
                squad: d.id,
                private: d.is_private,
            },
            true,
            |id| social_ext::squad_join(id, &target),
        )
    }

    pub fn set_squad_score_period(&mut self, period: SquadScorePeriod) -> Vec<Outgoing> {
        if period == self.squad.score_period {
            return Vec::new();
        }
        self.squad.score_period = period;
        self.refresh_squad_scoreboard(false)
    }

    /// `refreshSquadScoreboard`: the Squad Arena of the Leaderboard subtab, at most once a
    /// minute per period (`SQUAD_SCOREBOARD_CACHE_TTL_MS`); one request per period in flight.
    pub(super) fn refresh_squad_scoreboard(&mut self, force: bool) -> Vec<Outgoing> {
        if !self.active()
            || !self.tab_visible
            || self.subtab != Subtab::Leaderboard
            || self.scope != LeaderboardScope::Squad
        {
            return Vec::new();
        }
        let period = self.squad.score_period;
        let fresh = self
            .squad
            .fetched_at
            .get(&period)
            .is_some_and(|t| t.elapsed() < Duration::from_millis(SCOREBOARD_CACHE_MS as u64));
        let in_flight = self.pending.values().any(|(p, _)| {
            matches!(p, Pending::Squad(SquadPending::Scoreboard { period: q }) if *q == period)
        });
        if (!force && fresh) || in_flight {
            self.coalesced += 1;
            return Vec::new();
        }
        let Some(id) = self.identity().cloned() else {
            return Vec::new();
        };
        // production stamps the request time
        self.squad.fetched_at.insert(period, Instant::now());
        let req = social_ext::squad_scoreboard(&id, period);
        vec![self.send_squad(SquadPending::Scoreboard { period }, req)]
    }

    // ------------------------------------------------------------------------------ replies

    pub(super) fn squad_cancelled(&mut self, p: SquadPending) {
        match p {
            SquadPending::Mutation(m) => {
                self.syncing = self.sync_in_progress;
                if let SquadMutation::Chat { body } = m {
                    if self.squad.chat_draft.is_empty() {
                        self.squad.chat_draft = body;
                    }
                }
            }
            SquadPending::Search { suggestions } => {
                if suggestions {
                    self.squad.suggestions_loading = false;
                } else {
                    self.squad.searching = false;
                }
            }
            SquadPending::Details { squad, .. } => {
                if let Some(v) = self
                    .squad
                    .viewing
                    .as_mut()
                    .filter(|v| v.entry.squad_id == squad)
                {
                    v.loading = false;
                }
            }
            SquadPending::Scoreboard { period } => {
                self.squad.fetched_at.remove(&period);
            }
        }
    }

    /// `applySocialSnapshot`.
    fn apply_squad_snapshot(
        &mut self,
        resp: HttpResponse,
        now: WallTimestamp,
        ctx: &SyncContext,
    ) -> Result<(), NetError> {
        let snapshot = social_api::parse_snapshot(&resp)?;
        self.apply_snapshot(snapshot);
        if let Some(r) = self.record.as_mut() {
            r.sync.last_synced_at = Some(SocialTimestamp::from_wall(now));
            r.sync.last_sync_error = None;
            r.sync.next_auto_sync_at = Some(SocialTimestamp::from_wall(next_auto_sync_at(
                now, ctx.clock,
            )));
        }
        self.persist();
        Ok(())
    }

    pub(super) fn squad_reply(
        &mut self,
        p: SquadPending,
        result: Result<HttpResponse, NetError>,
        now: WallTimestamp,
        ctx: &SyncContext,
    ) -> Vec<Outgoing> {
        match p {
            SquadPending::Mutation(m) => {
                if !matches!(m, SquadMutation::Chat { .. } | SquadMutation::DeleteMessage) {
                    self.syncing = self.sync_in_progress;
                }
                let applied = result.and_then(|r| self.apply_squad_snapshot(r, now, ctx));
                self.squad_mutation_done(m, applied)
            }
            SquadPending::Search { suggestions } => {
                let parsed = result.and_then(|r| social_ext::parse_squad_search(&r));
                if suggestions {
                    self.squad.suggestions_loading = false;
                    match parsed {
                        Ok(rows) => {
                            self.squad.pool = rows;
                            self.squad.reshuffle();
                        }
                        Err(err) => self.say(err.user_message("Could not load squad suggestions.")),
                    }
                } else {
                    self.squad.searching = false;
                    match parsed {
                        Ok(rows) => self.squad.results = rows,
                        Err(err) => self.say(err.user_message("Could not search squads.")),
                    }
                }
                Vec::new()
            }
            SquadPending::Details { squad, after_join } => {
                let parsed = result.and_then(|r| social_ext::parse_squad_details(&r));
                let Some(v) = self
                    .squad
                    .viewing
                    .as_mut()
                    .filter(|v| v.entry.squad_id == squad)
                else {
                    // the dialog was closed or moved on: a late reply changes nothing
                    self.stale_replies += 1;
                    return Vec::new();
                };
                v.loading = false;
                match (parsed, after_join) {
                    (Ok(d), _) => v.details = Some(d),
                    (Err(_), Some(was_private)) => {
                        // the join worked, the refresh did not: production assumes the outcome
                        if let Some(d) = v.details.as_mut() {
                            d.action = if was_private {
                                SquadAction::Pending
                            } else {
                                SquadAction::Current
                            };
                        }
                    }
                    (Err(err), None) => self.say(err.user_message("Could not load squad.")),
                }
                Vec::new()
            }
            SquadPending::Scoreboard { period } => {
                match result.and_then(|r| social_ext::parse_squad_scoreboard(&r)) {
                    Ok(rows) => {
                        self.squad.scores.insert(period, rows);
                    }
                    Err(_) => {
                        // production logs and keeps what it had; it may try again at once
                        self.squad.fetched_at.remove(&period);
                    }
                }
                Vec::new()
            }
        }
    }

    fn squad_mutation_done(
        &mut self,
        m: SquadMutation,
        applied: Result<(), NetError>,
    ) -> Vec<Outgoing> {
        let failed = |c: &mut Self, err: NetError, fallback: &str| {
            c.say(err.user_message(fallback));
            Vec::new()
        };
        match (m, applied) {
            (SquadMutation::Create, Ok(())) => {
                self.squad.name_draft.clear();
                self.say("Squad created.");
                Vec::new()
            }
            (SquadMutation::Create, Err(e)) => failed(self, e, "Could not create squad."),
            (SquadMutation::Join { squad, private }, Ok(())) => {
                if private {
                    self.squad.mark_pending(&squad);
                }
                self.say(if private {
                    "Join request sent."
                } else {
                    "Joined squad."
                });
                // production searches again with whatever is in the search field
                self.squad.searching = false;
                self.search_squads()
            }
            (SquadMutation::Join { .. }, Err(e)) => failed(self, e, "Could not join squad."),
            (SquadMutation::JoinViewed { squad, private }, Ok(())) => {
                self.say(if private {
                    "Join request sent."
                } else {
                    "Joined squad."
                });
                let Some(id) = self.identity().cloned() else {
                    return Vec::new();
                };
                let req = social_ext::squad_details(&id, &squad);
                vec![self.send_squad(
                    SquadPending::Details {
                        squad,
                        after_join: Some(private),
                    },
                    req,
                )]
            }
            (SquadMutation::JoinViewed { .. }, Err(e)) => failed(self, e, "Could not join squad."),
            (SquadMutation::Respond { accept }, Ok(())) => {
                self.say(if accept {
                    "Squad request accepted."
                } else {
                    "Squad request declined."
                });
                Vec::new()
            }
            (SquadMutation::Respond { .. }, Err(e)) => {
                failed(self, e, "Could not update squad request.")
            }
            (SquadMutation::Leave { last }, Ok(())) => {
                self.squad.settings_editing = false;
                self.squad.expanded_member = None;
                self.squad.suggestions_tried = false;
                self.say(if last {
                    "Squad deleted."
                } else {
                    "You left the squad."
                });
                self.maybe_load_suggestions()
            }
            (SquadMutation::Leave { .. }, Err(e)) => failed(self, e, "Could not leave squad."),
            (SquadMutation::Chat { .. }, Ok(())) => Vec::new(),
            (SquadMutation::Chat { body }, Err(e)) => {
                if self.squad.chat_draft.is_empty() {
                    self.squad.chat_draft = body;
                }
                failed(self, e, "Could not send message.")
            }
            (SquadMutation::DeleteMessage, Ok(())) => {
                self.say("Message deleted.");
                Vec::new()
            }
            (SquadMutation::DeleteMessage, Err(e)) => failed(self, e, "Could not delete message."),
            (SquadMutation::Role, Ok(())) => {
                self.squad.expanded_member = None;
                self.say("Squad rank updated.");
                Vec::new()
            }
            (SquadMutation::Role, Err(e)) => failed(self, e, "Could not update squad rank."),
            (SquadMutation::Kick, Ok(())) => {
                self.squad.expanded_member = None;
                self.say("Member kicked.");
                Vec::new()
            }
            (SquadMutation::Kick, Err(e)) => failed(self, e, "Could not kick member."),
            (SquadMutation::Settings, Ok(())) => {
                self.squad.settings_editing = false;
                self.say("Squad settings saved.");
                Vec::new()
            }
            (SquadMutation::Settings, Err(e)) => {
                failed(self, e, "Could not update squad settings.")
            }
        }
    }
}
