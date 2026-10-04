//! The player's own study numbers for Social (Stage 22a): what production derives locally from
//! `state.sessions` (`getLocalSocialStats`, `getSyncSocialStats`, `getLocalLeaderboardEntry`,
//! `getLocalMonthlyStats`). These are the *only* numbers the client computes itself; every other
//! user's numbers come from the Worker.

use crate::academic::{AcademicState, SessionKind, StudySession};
use crate::dashboard::civil::{CivilDate, LocalClock};
use crate::timer::WallTimestamp;

use super::leaderboard::LeaderboardPeriod;
use super::limits::MAX_SYNC_STAT_ROWS;

/// One `{date, minutes, sessions}` row of the sync payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyStat {
    pub date: CivilDate,
    pub minutes: u64,
    pub sessions: u64,
}

/// Minutes and session count of one period, plus the last active date (Profile mini-stats and
/// the self-profile dialog).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PeriodStat {
    pub minutes: u64,
    pub sessions: u64,
    pub last_active: Option<CivilDate>,
}

fn counts(session: &StudySession) -> bool {
    matches!(session.kind, SessionKind::Study | SessionKind::Exam)
}

/// `sessionDateKey`: the local date the session *ended* on.
fn date_key(session: &StudySession, clock: &dyn LocalClock) -> CivilDate {
    clock.local_date(session.ended_at)
}

/// `getLocalSocialStats`: study/exam sessions grouped by local end date, ascending.
pub fn daily_stats(state: &AcademicState, clock: &dyn LocalClock) -> Vec<DailyStat> {
    let mut by_date: std::collections::BTreeMap<CivilDate, DailyStat> = Default::default();
    for s in state.sessions.iter().filter(|s| counts(s)) {
        let date = date_key(s, clock);
        let row = by_date.entry(date).or_insert(DailyStat {
            date,
            minutes: 0,
            sessions: 0,
        });
        row.minutes += u64::from(s.minutes);
        row.sessions += 1;
    }
    by_date.into_values().collect()
}

/// `getSyncSocialStats`: the last 370 daily rows.
pub fn sync_stats(state: &AcademicState, clock: &dyn LocalClock) -> Vec<DailyStat> {
    let all = daily_stats(state, clock);
    let skip = all.len().saturating_sub(MAX_SYNC_STAT_ROWS);
    all.into_iter().skip(skip).collect()
}

/// The local Monday 00:00 of the week containing `now` (`startOfWeek`).
fn week_start(now: WallTimestamp, clock: &dyn LocalClock) -> WallTimestamp {
    let today = clock.local_date(now);
    let monday_offset = (i64::from(today.weekday()) + 6) % 7;
    clock.local_midnight(today.add_days(-monday_offset))
}

/// `getLocalLeaderboardEntry`'s minutes/sessions/lastActiveDate for a period. `overall` uses the
/// lifetime running totals (they survive history pruning); `lastActiveDate` is the end date of
/// the most recent session of *any* kind, as production sorts `state.sessions` without a filter.
pub fn period_stat(
    state: &AcademicState,
    period: LeaderboardPeriod,
    now: WallTimestamp,
    clock: &dyn LocalClock,
) -> PeriodStat {
    let today = clock.local_date(now);
    let monday = week_start(now, clock);
    let matching = state
        .sessions
        .iter()
        .filter(|s| counts(s))
        .filter(|s| match period {
            LeaderboardPeriod::Daily => date_key(s, clock) == today,
            LeaderboardPeriod::Weekly => s.ended_at.unix_millis >= monday.unix_millis,
            LeaderboardPeriod::Overall => true,
        });
    let (minutes, sessions) =
        matching.fold((0u64, 0u64), |(m, n), s| (m + u64::from(s.minutes), n + 1));
    let last_active = state
        .sessions
        .iter()
        .max_by_key(|s| s.ended_at.unix_millis)
        .map(|s| date_key(s, clock));
    match period {
        LeaderboardPeriod::Overall => PeriodStat {
            minutes: state.lifetime_study_minutes,
            sessions: state.lifetime_study_sessions,
            last_active,
        },
        _ => PeriodStat {
            minutes,
            sessions,
            last_active,
        },
    }
}

/// `getLocalMonthlyStats`: study/exam sessions ending on or after local midnight of the 1st.
pub fn monthly_stat(
    state: &AcademicState,
    now: WallTimestamp,
    clock: &dyn LocalClock,
) -> PeriodStat {
    let today = clock.local_date(now);
    let first = today.add_days(-(i64::from(today.day()) - 1));
    let start = clock.local_midnight(first);
    let (minutes, sessions) = state
        .sessions
        .iter()
        .filter(|s| counts(s) && s.ended_at.unix_millis >= start.unix_millis)
        .fold((0u64, 0u64), |(m, n), s| (m + u64::from(s.minutes), n + 1));
    PeriodStat {
        minutes,
        sessions,
        last_active: None,
    }
}
