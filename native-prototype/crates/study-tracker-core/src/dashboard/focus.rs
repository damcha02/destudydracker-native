//! The "Weekly focus" / focus-history timeline (production's `focusTimeline` memo in `App.tsx`,
//! rendered by `renderWeeklyChart`), as pure data: the days of the selected range with their
//! per-course layers, the milestone ("fossil") hits, and the summary stats. Chart *geometry*
//! (pixel heights, per-layer random radii) is the application layer's job; everything a chart
//! has to agree with production on - which days, which totals, which layers, in which order - is
//! decided here.

use std::collections::HashMap;

use super::civil::CivilDate;
use super::metrics::SessionDays;
use crate::academic::Course;

/// `focusMilestones` (hours, label, description).
pub const FOCUS_MILESTONES: [(u32, &str, &str); 7] = [
    (10, "Seed Fossil", "10 hours of study unearthed"),
    (25, "Shell Fragment", "25 hours - patterns forming"),
    (50, "Ammonite", "50 hours - taking shape"),
    (100, "Crystal Cluster", "100 hours crystallized"),
    (250, "Complete Specimen", "250 hours - a rare find"),
    (500, "Ancient Artifact", "500 hours - legendary"),
    (1000, "Golden Record", "1000 hours - transcendent"),
];

/// `FocusRange = "week" | 7 | 14 | 30 | 60 | 365`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusRange {
    /// Monday-start calendar week containing today (always 7 columns, future days empty).
    Week,
    Days(u16),
}

impl FocusRange {
    pub const ALL: [FocusRange; 6] = [
        FocusRange::Week,
        FocusRange::Days(7),
        FocusRange::Days(14),
        FocusRange::Days(30),
        FocusRange::Days(60),
        FocusRange::Days(365),
    ];

    pub fn day_count(self) -> usize {
        match self {
            FocusRange::Week => 7,
            FocusRange::Days(n) => n as usize,
        }
    }

    /// The toggle button text: "This week", "7d", ... "1y".
    pub fn button_label(self) -> String {
        match self {
            FocusRange::Week => "This week".into(),
            FocusRange::Days(365) => "1y".into(),
            FocusRange::Days(n) => format!("{n}d"),
        }
    }

    /// The right-aligned caption of the stats row denominator ("week", "1y", or the day count).
    pub fn stats_denominator(self) -> String {
        match self {
            FocusRange::Week => "week".into(),
            FocusRange::Days(365) => "1y".into(),
            FocusRange::Days(n) => n.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FossilLayer {
    /// The course id, or `"general"` for sessions with no (or a since-deleted) course.
    pub id: String,
    pub name: String,
    /// `None` = production's muted `--ink-4` (the "General" layer).
    pub color: Option<String>,
    pub minutes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FossilDay {
    pub date: CivilDate,
    pub is_today: bool,
    pub total_minutes: u64,
    /// Largest first; ties keep first-seen order (stable sort, like production).
    pub layers: Vec<FossilLayer>,
    pub cumulative: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MilestoneHit {
    /// Index into [`FOCUS_MILESTONES`].
    pub milestone: usize,
    pub day_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FocusTimeline {
    pub range: FocusRange,
    pub days: Vec<FossilDay>,
    pub milestone_hits: Vec<MilestoneHit>,
    pub active_days: usize,
    /// Index of the first day holding the maximum total (production's `reduce` with `>`).
    pub biggest_day: usize,
    pub max_day_minutes: u64,
    /// Legend: every course (or "General") with focus in the range, most minutes first.
    pub active_courses: Vec<FossilLayer>,
    /// Indexes into [`FOCUS_MILESTONES`] reached by the *lifetime* total.
    pub achieved_milestones: Vec<usize>,
    pub visible_minutes: u64,
}

/// `focusTimeline`. `today` is the local date right now; `lifetime_minutes` is
/// `AppState.lifetimeStudyMinutes` (which survives history pruning), used only for the
/// "Discovered"/"Latest find" milestones.
pub fn focus_timeline(
    range: FocusRange,
    today: CivilDate,
    sessions: &SessionDays<'_>,
    courses: &[Course],
    lifetime_minutes: u64,
) -> FocusTimeline {
    let course_lookup: HashMap<&str, &Course> =
        courses.iter().map(|c| (c.id.as_str(), c)).collect();

    // Bucket sessions by the local day they ended on, preserving `endedAt` order (production
    // sorts by the ISO `endedAt` string first; layer tie order depends on it).
    let mut order: Vec<usize> = (0..sessions.sessions.len()).collect();
    order.sort_by_key(|&i| sessions.sessions[i].ended_at);
    let mut by_day: HashMap<CivilDate, Vec<usize>> = HashMap::new();
    for i in order {
        by_day.entry(sessions.ended_day[i]).or_default().push(i);
    }

    let range_start = match range {
        FocusRange::Week => {
            let weekday = today.weekday() as i64;
            let monday_offset = if weekday == 0 { -6 } else { 1 - weekday };
            today.add_days(monday_offset)
        }
        FocusRange::Days(n) => today.add_days(-(i64::from(n)) + 1),
    };
    let mut cumulative: u64 = sessions
        .sessions
        .iter()
        .zip(&sessions.ended_day)
        .filter(|(_, day)| **day < range_start)
        .map(|(s, _)| u64::from(s.minutes))
        .sum();

    let mut days: Vec<FossilDay> = Vec::with_capacity(range.day_count());
    let mut milestone_hits = Vec::new();
    let mut max_day_minutes = 30u64;

    for i in 0..range.day_count() {
        let date = range_start.add_days(i as i64);
        let day_sessions = by_day.get(&date).map(Vec::as_slice).unwrap_or(&[]);
        let total: u64 = day_sessions
            .iter()
            .map(|&i| u64::from(sessions.sessions[i].minutes))
            .sum();
        let previous = cumulative;
        cumulative += total;
        max_day_minutes = max_day_minutes.max(total);
        for (index, (hours, _, _)) in FOCUS_MILESTONES.iter().enumerate() {
            let threshold = u64::from(*hours) * 60;
            if previous < threshold && cumulative >= threshold {
                milestone_hits.push(MilestoneHit {
                    milestone: index,
                    day_index: days.len(),
                });
            }
        }

        let mut layers: Vec<FossilLayer> = Vec::new();
        for &session_index in day_sessions {
            let session = &sessions.sessions[session_index];
            let course = session
                .course_id
                .as_ref()
                .and_then(|id| course_lookup.get(id.as_str()));
            let id = course.map_or("general", |c| c.id.as_str());
            match layers.iter_mut().find(|l| l.id == id) {
                Some(layer) => layer.minutes += u64::from(session.minutes),
                None => layers.push(FossilLayer {
                    id: id.to_string(),
                    name: course.map_or_else(|| "General".to_string(), |c| c.name.clone()),
                    color: course.map(|c| c.color.clone()),
                    minutes: u64::from(session.minutes),
                }),
            }
        }
        layers.sort_by(|a, b| b.minutes.cmp(&a.minutes));
        days.push(FossilDay {
            date,
            is_today: date == today,
            total_minutes: total,
            layers,
            cumulative,
        });
    }

    let active_days = days.iter().filter(|d| d.total_minutes > 0).count();
    let mut biggest_day = 0;
    for (index, day) in days.iter().enumerate() {
        if day.total_minutes > days[biggest_day].total_minutes {
            biggest_day = index;
        }
    }
    let mut legend: Vec<FossilLayer> = Vec::new();
    for day in &days {
        for layer in &day.layers {
            match legend.iter_mut().find(|l| l.id == layer.id) {
                Some(existing) => existing.minutes += layer.minutes,
                None => legend.push(layer.clone()),
            }
        }
    }
    legend.sort_by(|a, b| b.minutes.cmp(&a.minutes));
    let achieved_milestones = FOCUS_MILESTONES
        .iter()
        .enumerate()
        .filter(|(_, (hours, _, _))| lifetime_minutes >= u64::from(*hours) * 60)
        .map(|(i, _)| i)
        .collect();
    let visible_minutes = days.iter().map(|d| d.total_minutes).sum();

    FocusTimeline {
        range,
        days,
        milestone_hits,
        active_days,
        biggest_day,
        max_day_minutes,
        active_courses: legend,
        achieved_milestones,
        visible_minutes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::academic::{CourseId, SemesterId, SessionId, SessionKind, StudySession};
    use crate::dashboard::civil::FixedOffsetClock;
    use crate::timer::WallTimestamp;

    fn d(s: &str) -> CivilDate {
        CivilDate::parse_iso(s).unwrap()
    }
    fn course(id: &str, name: &str, color: &str) -> Course {
        Course::new(
            CourseId::new(id),
            SemesterId::new("s"),
            name.into(),
            color.into(),
            WallTimestamp::from_unix_millis(0),
        )
    }
    fn session(day: &str, hour: u32, minutes: u32, course: Option<&str>) -> StudySession {
        let ended = WallTimestamp::from_unix_millis(
            d(day).days() * 86_400_000 + i64::from(hour) * 3_600_000,
        );
        StudySession {
            id: SessionId::new(format!("{day}-{hour}")),
            semester_id: None,
            course_id: course.map(CourseId::new),
            task_id: None,
            kind: SessionKind::Study,
            goal: String::new(),
            learned: String::new(),
            blocker: String::new(),
            next_step: String::new(),
            confidence: 3,
            started_at: WallTimestamp::from_unix_millis(
                ended.unix_millis - i64::from(minutes) * 60_000,
            ),
            ended_at: ended,
            minutes,
            preset_label: String::new(),
        }
    }

    #[test]
    fn week_range_is_monday_to_sunday_with_future_days_empty() {
        // 2026-09-30 is a Wednesday.
        let sessions = vec![
            session("2026-09-28", 10, 60, Some("a")),
            session("2026-09-30", 10, 30, Some("a")),
        ];
        let sd = SessionDays::new(&sessions, &FixedOffsetClock::UTC);
        let t = focus_timeline(
            FocusRange::Week,
            d("2026-09-30"),
            &sd,
            &[course("a", "Alpha", "#111")],
            90,
        );
        assert_eq!(t.days.len(), 7);
        assert_eq!(t.days[0].date.to_iso(), "2026-09-28");
        assert_eq!(t.days[6].date.to_iso(), "2026-10-04");
        assert!(t.days[2].is_today);
        assert_eq!(
            t.days.iter().map(|x| x.total_minutes).collect::<Vec<_>>(),
            [60, 0, 30, 0, 0, 0, 0]
        );
        assert_eq!(t.active_days, 2);
        assert_eq!(t.biggest_day, 0);
        assert_eq!(t.visible_minutes, 90);
        assert_eq!(t.max_day_minutes, 60);
    }

    #[test]
    fn sunday_belongs_to_the_week_that_started_the_previous_monday() {
        let sd = SessionDays::new(&[], &FixedOffsetClock::UTC);
        // 2026-10-04 is a Sunday.
        let t = focus_timeline(FocusRange::Week, d("2026-10-04"), &sd, &[], 0);
        assert_eq!(t.days[0].date.to_iso(), "2026-09-28");
        assert_eq!(t.days[6].date.to_iso(), "2026-10-04");
    }

    #[test]
    fn max_day_floor_is_thirty_minutes_and_layers_sort_largest_first() {
        let sessions = vec![
            session("2026-09-30", 8, 10, Some("b")),
            session("2026-09-30", 9, 20, Some("a")),
            session("2026-09-30", 10, 5, Some("b")),
            session("2026-09-30", 11, 4, None),
            session("2026-09-30", 12, 6, Some("deleted")),
        ];
        let sd = SessionDays::new(&sessions, &FixedOffsetClock::UTC);
        let courses = [course("a", "Alpha", "#a"), course("b", "Beta", "#b")];
        let t = focus_timeline(FocusRange::Days(7), d("2026-09-30"), &sd, &courses, 45);
        assert_eq!(
            t.max_day_minutes,
            45.max(30),
            "a 45-minute day exceeds the 30-minute floor"
        );
        let names: Vec<&str> = t.days[6].layers.iter().map(|l| l.name.as_str()).collect();
        // a=20, b=15, general (no course 4 + deleted course 6) = 10
        assert_eq!(names, ["Alpha", "Beta", "General"]);
        assert_eq!(t.days[6].layers[2].minutes, 10);
        assert_eq!(
            t.days[6].layers[2].color, None,
            "General has no course color"
        );
        assert_eq!(
            t.active_courses
                .iter()
                .map(|l| l.minutes)
                .collect::<Vec<_>>(),
            [20, 15, 10]
        );

        let quiet = focus_timeline(
            FocusRange::Days(7),
            d("2026-09-30"),
            &SessionDays::new(&[], &FixedOffsetClock::UTC),
            &[],
            0,
        );
        assert_eq!(quiet.max_day_minutes, 30);
    }

    #[test]
    fn milestones_fire_on_the_day_the_cumulative_total_crosses_them() {
        // 9h before the range, then a 90-minute day pushes cumulative over 10h.
        let sessions = vec![
            session("2026-09-01", 10, 540, Some("a")),
            session("2026-09-29", 10, 90, Some("a")),
        ];
        let sd = SessionDays::new(&sessions, &FixedOffsetClock::UTC);
        let t = focus_timeline(
            FocusRange::Days(7),
            d("2026-09-30"),
            &sd,
            &[course("a", "A", "#a")],
            630,
        );
        assert_eq!(
            t.milestone_hits,
            vec![MilestoneHit {
                milestone: 0,
                day_index: 5
            }]
        );
        assert_eq!(t.achieved_milestones, vec![0]);
        assert_eq!(t.days[5].cumulative, 630);
        assert_eq!(t.days[0].cumulative, 540);
    }

    #[test]
    fn lifetime_minutes_not_the_visible_history_decides_discovered_milestones() {
        let sd = SessionDays::new(&[], &FixedOffsetClock::UTC);
        let t = focus_timeline(FocusRange::Week, d("2026-09-30"), &sd, &[], 100 * 60);
        assert_eq!(t.achieved_milestones, vec![0, 1, 2, 3]);
        assert_eq!(t.visible_minutes, 0);
    }

    #[test]
    fn biggest_day_keeps_the_first_of_equal_maxima() {
        let sessions = vec![
            session("2026-09-28", 10, 50, None),
            session("2026-09-29", 10, 50, None),
        ];
        let sd = SessionDays::new(&sessions, &FixedOffsetClock::UTC);
        let t = focus_timeline(FocusRange::Week, d("2026-09-30"), &sd, &[], 100);
        assert_eq!(t.biggest_day, 0);
    }

    #[test]
    fn year_range_has_365_days_ending_today() {
        let sd = SessionDays::new(&[], &FixedOffsetClock::UTC);
        let t = focus_timeline(FocusRange::Days(365), d("2028-03-01"), &sd, &[], 0);
        assert_eq!(t.days.len(), 365);
        assert_eq!(t.days[364].date.to_iso(), "2028-03-01");
        assert_eq!(
            t.days[0].date.to_iso(),
            "2027-03-03",
            "365 days back across the 2028-02-29 leap day"
        );
    }
}
