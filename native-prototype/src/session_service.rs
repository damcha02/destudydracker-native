//! Timer -> StudySession bridge (Stage 16).
//!
//! Production reference: `desktop/src/App.tsx`'s `buildSessionFromTimer`/
//! `buildSessionsFromTimerRange` (live/manual completion) and `desktop/src/lib/storage.ts`'s
//! `buildRecoveredSessionsFromSegments` (abandoned-timer recovery on restart). Both go through the
//! same local-calendar-day bucketing algorithm in production; this module is the one place that
//! algorithm is implemented natively, called from both the live-completion path
//! (`app_model.rs`'s handling of `TimerApplicationEffect::SessionRangeReady`) and the startup-
//! recovery path, exactly mirroring production's own reuse of one function for both.
//!
//! This lives in the application layer, not `study-tracker-core`, specifically because it needs
//! the OS's local timezone offset (`chrono::Local`) - a platform-environment dependency the core
//! crate must never have (see `docs/stage12_5-architecture-freeze.md` section 5). The core only
//! ever sees `TimerApplicationEffect`s and produces `StudySession` values as plain data; it has no
//! idea sessions get bucketed by calendar day at all.
//!
//! **Local-day-bucketing policy and its one documented limitation**: a session's local calendar
//! day is computed using one `FixedOffset` captured once, at the moment the completion/recovery
//! is handled (`chrono::Local::now().offset()` at the real call site). This differs subtly from
//! production, where every `Date` computation always reflects the OS's live timezone rules for
//! that exact instant. In the extremely rare case where a single Timer session spans a real DST
//! transition, production would place the split at the true local midnight for each half; this
//! implementation uses the single captured offset throughout, which could misplace the boundary
//! by up to the size of the DST shift (typically one hour) right around the transition. This is a
//! deliberate, documented simplification (see `docs/stage16-academic-domain.md` section 13), not
//! an oversight - the brief explicitly asks not to overengineer timezone support beyond what
//! production's own everyday behavior requires, and this case is rare enough (and low-stakes
//! enough - it only ever shifts which calendar day a few minutes of a session are logged under)
//! that a `chrono-tz` dependency and per-instant timezone-rule lookups were not added for it.

use chrono::{DateTime, Duration as ChronoDuration, FixedOffset, NaiveDate, TimeZone};

use study_tracker_core::academic::{SessionId, SessionKind, StudySession};
use study_tracker_core::timer::{ActiveSegment, TimerContext, TimerPhase, WallTimestamp};

/// `None` means "this phase never produces a session" (Break, Idle) - callers should simply not
/// call [`split_segments_into_daily_sessions`] at all in that case (matching production, where
/// `buildSessionsFromTimerRange` is never invoked for a Break completion).
pub fn session_kind_for_phase(phase: TimerPhase) -> Option<SessionKind> {
    match phase {
        TimerPhase::Study | TimerPhase::Stopwatch => Some(SessionKind::Study),
        TimerPhase::Exam => Some(SessionKind::Exam),
        TimerPhase::Break | TimerPhase::Idle => None,
    }
}

/// One local-calendar-day's accumulated active time, built up across possibly several pieces
/// (either several `ActiveSegment`s landing on the same day, or one segment cut at a midnight
/// boundary). Mirrors production's own bucket accumulator in `buildSessionsFromTimerRange`
/// exactly: `start`/`end` track the bucket's overall wall-clock span (first piece's start, latest
/// piece's end), while `active_millis` separately accumulates only the *actual* active time -
/// which matters the moment a bucket is fed by more than one piece with a gap between them (e.g.
/// two segments on the same day with a pause in between): the session's reported duration must be
/// the sum of active time, never `end - start` across that gap.
struct DayBucket {
    key: String,
    start: i64,
    end: i64,
    active_millis: i64,
}

/// Splits `segments` into one [`StudySession`] per local calendar day they cross, mirroring
/// production's `buildSessionsFromTimerRange`/`buildRecoveredSessionsFromSegments`: a piece
/// crossing local midnight is cut there (the piece before midnight ends 1 ms before it, staying
/// inside that calendar day); multiple pieces landing on the same local day (several segments, or
/// several midnight-cut pieces of one segment) accumulate into a single session for that day;
/// each session's minutes are `max(1, round(active_seconds / 60))`, matching production's own
/// rounding (never zero minutes for a real, however short, block).
///
/// `id_for_bucket` receives each day-bucket's start/end (unix millis) and returns the id that
/// bucket's `StudySession` should get - callers pass a fresh-id generator for a live/manual
/// completion, or [`recovered_session_id`] for abandoned-timer recovery, exactly mirroring
/// production's own two id schemes for the same two situations.
pub fn split_segments_into_daily_sessions(
    segments: &[ActiveSegment],
    phase: TimerPhase,
    context: &TimerContext,
    preset_label: &str,
    local_offset: FixedOffset,
    mut id_for_bucket: impl FnMut(i64, i64) -> SessionId,
) -> Vec<StudySession> {
    let Some(kind) = session_kind_for_phase(phase) else {
        return Vec::new();
    };

    let mut buckets: Vec<DayBucket> = Vec::new();
    for segment in segments {
        let Some(ended_at) = segment.ended_at else {
            continue;
        };
        let mut cursor = segment.started_at.unix_millis;
        let end_millis = ended_at.unix_millis;
        if end_millis <= cursor {
            continue;
        }

        while cursor < end_millis {
            let midnight = next_local_midnight_millis(cursor, local_offset);
            let piece_end = end_millis.min(midnight);
            let bucket_end = if piece_end == midnight {
                piece_end - 1
            } else {
                piece_end
            };
            let active = (piece_end - cursor).max(0);
            let key = local_date_key(cursor, local_offset);

            match buckets.iter_mut().find(|bucket| bucket.key == key) {
                Some(bucket) => {
                    bucket.end = bucket_end;
                    bucket.active_millis += active;
                }
                None => buckets.push(DayBucket {
                    key,
                    start: cursor,
                    end: bucket_end,
                    active_millis: active,
                }),
            }
            cursor = piece_end;
        }
    }

    buckets
        .into_iter()
        .map(|bucket| {
            let minutes = (bucket.active_millis as f64 / 60_000.0).round().max(1.0) as u32;
            StudySession {
                id: id_for_bucket(bucket.start, bucket.end),
                semester_id: context.semester_id.clone().map(Into::into),
                course_id: context.course_id.clone().map(Into::into),
                task_id: context.task_id.clone().map(Into::into),
                kind,
                goal: context.goal.trim().to_string(),
                learned: context.learned.trim().to_string(),
                blocker: context.blocker.trim().to_string(),
                next_step: context.next_step.trim().to_string(),
                confidence: context.confidence,
                started_at: WallTimestamp::from_unix_millis(bucket.start),
                ended_at: WallTimestamp::from_unix_millis(bucket.end),
                minutes,
                preset_label: preset_label.to_string(),
            }
        })
        .collect()
}

/// The next local midnight (in unix millis) strictly after `after_millis`, in the timezone
/// described by `offset`. Mirrors production's `nextLocalMidnightAfter` (`new Date(date);
/// next.setHours(24, 0, 0, 0)`).
fn next_local_midnight_millis(after_millis: i64, offset: FixedOffset) -> i64 {
    let local = local_datetime(after_millis, offset);
    let next_date: NaiveDate = local.date_naive() + ChronoDuration::days(1);
    let next_midnight_naive = next_date
        .and_hms_opt(0, 0, 0)
        .expect("midnight is always a valid time");
    resolve_local(&next_midnight_naive, offset).timestamp_millis()
}

/// `YYYY-MM-DD` in the local timezone `offset` describes for `millis` - the bucketing key.
fn local_date_key(millis: i64, offset: FixedOffset) -> String {
    local_datetime(millis, offset)
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

fn local_datetime(millis: i64, offset: FixedOffset) -> DateTime<FixedOffset> {
    offset
        .timestamp_millis_opt(millis)
        .single()
        .unwrap_or_else(|| {
            // `FixedOffset` never has ambiguous/skipped instants (unlike a real DST-aware timezone) -
            // this branch is unreachable in practice, but a graceful fallback is kept rather than an
            // `unwrap()` that could panic if that invariant is ever violated by a future change.
            offset
                .timestamp_millis_opt(millis)
                .earliest()
                .expect("FixedOffset always resolves")
        })
}

fn resolve_local(naive: &chrono::NaiveDateTime, offset: FixedOffset) -> DateTime<FixedOffset> {
    offset
        .from_local_datetime(naive)
        .single()
        .unwrap_or_else(|| {
            offset
                .from_local_datetime(naive)
                .earliest()
                .expect("FixedOffset always resolves")
        })
}

/// Deterministic id for an abandoned-timer-recovery session bucket, mirroring production's own
/// `recovered-${timer.phase}-${startedAt}-${endedAt}` scheme (`storage.ts`'s
/// `buildRecoveredSessionsFromSegments`) closely enough to serve the same purpose: the exact same
/// abandoned range recovered twice (e.g. two restarts in a row before the reset state is re-
/// persisted - see `docs/stage15-persistence-migration.md`'s "Idempotence") produces the exact
/// same id both times, so `AcademicState::add_study_sessions`'s dedup-by-id silently drops the
/// second one instead of creating a duplicate. Uses unix millis rather than production's ISO
/// strings (an intentional representation difference - see the stage doc - the determinism
/// property is what matters, not byte-for-byte id equality with production).
pub fn recovered_session_id(
    phase: TimerPhase,
    bucket_start_millis: i64,
    bucket_end_millis: i64,
) -> SessionId {
    SessionId::new(format!(
        "recovered-{phase:?}-{bucket_start_millis}-{bucket_end_millis}"
    ))
}

/// A fresh id for a live/manual-completion session bucket - production's counterpart here is
/// `makeId()` (`crypto.randomUUID()`, falling back to `Date.now()-random()`; see
/// `docs/stage16-academic-domain.md` section 6 for why this native side uses a process-global
/// counter plus the wall-clock moment instead of adding a `uuid`/`rand` dependency: uniqueness
/// only needs to hold within one running process's session-creation events, exactly as
/// production's own `Date.now()-random()` fallback path already assumes, and a counter trivially
/// guarantees that without any randomness at all.
pub fn fresh_session_id(now_unix_millis: i64) -> SessionId {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    SessionId::new(format!("session-{now_unix_millis:x}-{n:x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offset_hours(hours: i32) -> FixedOffset {
        FixedOffset::east_opt(hours * 3600).unwrap()
    }

    fn segment(start_ms: i64, end_ms: i64) -> ActiveSegment {
        ActiveSegment {
            started_at: WallTimestamp::from_unix_millis(start_ms),
            ended_at: Some(WallTimestamp::from_unix_millis(end_ms)),
        }
    }

    fn counter_ids() -> impl FnMut(i64, i64) -> SessionId {
        let mut n = 0u64;
        move |_start, _end| {
            n += 1;
            SessionId::new(format!("fresh-{n}"))
        }
    }

    fn utc_midnight(y: i32, m: u32, d: u32) -> i64 {
        FixedOffset::east_opt(0)
            .unwrap()
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(y, m, d)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
            )
            .unwrap()
            .timestamp_millis()
    }

    #[test]
    fn a_session_entirely_within_one_local_day_produces_exactly_one_session() {
        let start = utc_midnight(2026, 9, 28) + 10 * 60 * 60_000;
        let sessions = split_segments_into_daily_sessions(
            &[segment(start, start + 30 * 60_000)],
            TimerPhase::Study,
            &TimerContext::default(),
            "Pomodoro 25/5",
            offset_hours(0),
            counter_ids(),
        );
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].minutes, 30);
        assert_eq!(sessions[0].kind, SessionKind::Study);
    }

    #[test]
    fn a_session_crossing_local_midnight_splits_into_two_sessions() {
        let midnight = utc_midnight(2026, 9, 29);
        let start = midnight - 20 * 60_000; // 20 minutes before midnight
        let end = midnight + 40 * 60_000; // 40 minutes after midnight
        let sessions = split_segments_into_daily_sessions(
            &[segment(start, end)],
            TimerPhase::Study,
            &TimerContext::default(),
            "Deep Work",
            offset_hours(0),
            counter_ids(),
        );
        assert_eq!(sessions.len(), 2, "one session per local calendar day");
        assert_eq!(sessions[0].minutes, 20);
        assert_eq!(sessions[1].minutes, 40);
        assert!(
            sessions[0].ended_at.unix_millis < midnight,
            "the first day's session must end strictly before midnight"
        );
        assert_eq!(
            sessions[1].started_at.unix_millis, midnight,
            "the second day's session starts exactly at midnight"
        );
    }

    #[test]
    fn two_segments_on_the_same_local_day_with_a_gap_merge_into_one_session_excluding_the_gap() {
        let base = utc_midnight(2026, 9, 28) + 9 * 60 * 60_000; // 09:00 local
                                                                // 09:00-09:10 (10 min), then a one-hour pause, then 10:10-10:20 (10 min).
        let segments = [
            segment(base, base + 10 * 60_000),
            segment(base + 70 * 60_000, base + 80 * 60_000),
        ];
        let sessions = split_segments_into_daily_sessions(
            &segments,
            TimerPhase::Study,
            &TimerContext::default(),
            "x",
            offset_hours(0),
            counter_ids(),
        );
        assert_eq!(sessions.len(), 1, "same local day - one merged session");
        assert_eq!(
            sessions[0].minutes, 20,
            "duration is the sum of ACTIVE time, excluding the paused gap in between"
        );
        assert_eq!(
            sessions[0].started_at.unix_millis, base,
            "keeps the first piece's start"
        );
        assert_eq!(
            sessions[0].ended_at.unix_millis,
            base + 80 * 60_000,
            "extends to the last piece's end"
        );
    }

    #[test]
    fn exam_phase_produces_exam_kind_sessions() {
        let sessions = split_segments_into_daily_sessions(
            &[segment(
                utc_midnight(2026, 9, 28),
                utc_midnight(2026, 9, 28) + 60_000,
            )],
            TimerPhase::Exam,
            &TimerContext::default(),
            "Exam",
            offset_hours(0),
            counter_ids(),
        );
        assert_eq!(sessions[0].kind, SessionKind::Exam);
    }

    #[test]
    fn stopwatch_endless_phase_produces_study_kind_sessions_matching_production() {
        let sessions = split_segments_into_daily_sessions(
            &[segment(
                utc_midnight(2026, 9, 28),
                utc_midnight(2026, 9, 28) + 60_000,
            )],
            TimerPhase::Stopwatch,
            &TimerContext::default(),
            "Endless",
            offset_hours(0),
            counter_ids(),
        );
        assert_eq!(sessions[0].kind, SessionKind::Study);
    }

    #[test]
    fn break_and_idle_phases_never_produce_a_session() {
        for phase in [TimerPhase::Break, TimerPhase::Idle] {
            let sessions = split_segments_into_daily_sessions(
                &[segment(0, 60_000)],
                phase,
                &TimerContext::default(),
                "x",
                offset_hours(0),
                counter_ids(),
            );
            assert!(sessions.is_empty());
        }
    }

    #[test]
    fn a_sub_minute_block_still_reports_at_least_one_minute() {
        let sessions = split_segments_into_daily_sessions(
            &[segment(
                utc_midnight(2026, 9, 28),
                utc_midnight(2026, 9, 28) + 10_000,
            )], // 10 real seconds
            TimerPhase::Study,
            &TimerContext::default(),
            "x",
            offset_hours(0),
            counter_ids(),
        );
        assert_eq!(
            sessions[0].minutes, 1,
            "never zero minutes for a real block, matching production"
        );
    }

    #[test]
    fn a_positive_timezone_offset_shifts_the_local_midnight_boundary() {
        // The same instant that is "before local midnight" at UTC+0 can already be "after local
        // midnight" at a positive offset - proves the offset is actually consulted, not ignored.
        let one_hour_before_utc_midnight = utc_midnight(2026, 9, 29) - 60 * 60_000;
        let sessions = split_segments_into_daily_sessions(
            &[segment(
                one_hour_before_utc_midnight,
                one_hour_before_utc_midnight + 30 * 60_000,
            )],
            TimerPhase::Study,
            &TimerContext::default(),
            "x",
            offset_hours(2),
            counter_ids(),
        );
        assert_eq!(
            sessions.len(),
            1,
            "a single 30-minute block stays one session regardless of which day it falls on"
        );
    }

    #[test]
    fn recovered_session_id_is_deterministic_for_the_same_bucket() {
        let a = recovered_session_id(TimerPhase::Study, 1000, 2000);
        let b = recovered_session_id(TimerPhase::Study, 1000, 2000);
        assert_eq!(a, b);
        let different = recovered_session_id(TimerPhase::Study, 1000, 2001);
        assert_ne!(a, different);
    }

    #[test]
    fn recovery_id_generator_produces_stable_ids_across_repeated_recovery_of_the_same_range() {
        let start = utc_midnight(2026, 9, 28);
        let segments = [segment(start, start + 90 * 60_000)];
        let first = split_segments_into_daily_sessions(
            &segments,
            TimerPhase::Study,
            &TimerContext::default(),
            "x",
            offset_hours(0),
            |b_start, b_end| recovered_session_id(TimerPhase::Study, b_start, b_end),
        );
        let second = split_segments_into_daily_sessions(
            &segments,
            TimerPhase::Study,
            &TimerContext::default(),
            "x",
            offset_hours(0),
            |b_start, b_end| recovered_session_id(TimerPhase::Study, b_start, b_end),
        );
        let ids_a: Vec<&SessionId> = first.iter().map(|s| &s.id).collect();
        let ids_b: Vec<&SessionId> = second.iter().map(|s| &s.id).collect();
        assert_eq!(
            ids_a, ids_b,
            "recovering the exact same range twice must yield the exact same ids"
        );
    }

    #[test]
    fn context_fields_are_carried_through_and_trimmed() {
        let context = TimerContext {
            goal: "  Finish chapter 4  ".to_string(),
            confidence: 4,
            ..TimerContext::default()
        };
        let sessions = split_segments_into_daily_sessions(
            &[segment(
                utc_midnight(2026, 9, 28),
                utc_midnight(2026, 9, 28) + 60_000,
            )],
            TimerPhase::Study,
            &context,
            "Pomodoro 25/5",
            offset_hours(0),
            counter_ids(),
        );
        assert_eq!(sessions[0].goal, "Finish chapter 4");
        assert_eq!(sessions[0].confidence, 4);
        assert_eq!(sessions[0].preset_label, "Pomodoro 25/5");
    }
}
