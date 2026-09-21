//! Stage 10 dashboard presentation model.
//!
//! Everything here is plain Rust: deterministic mock data plus the *preparation* step that turns
//! raw study minutes into chart geometry (normalized 0..1 positions, path commands, nice axis
//! ticks, rounded percentages, selection lookups). The Slint layer only draws what is prepared
//! here. Nothing in this module knows about Slint, and none of it belongs in `study-tracker-core`:
//! chart geometry is a presentation concern, not a domain concept.
//!
//! Geometry convention: positions are fractions in `0.0..=1.0`. `x` runs left to right and `y`
//! runs top to bottom (screen order), so Slint can multiply by the plot size and never has to
//! flip or scale data. Slint's `Path` scales its viewbox *uniformly* (aspect-preserving), so a
//! fixed square viewbox cannot stretch across a wide plot. Path commands are therefore emitted in
//! the plot's real pixel size; Slint reports that size back on resize and Rust re-emits only the
//! two path strings (`HistoryChart::set_plot_size`), never the statistics.

/// Plot size assumed until Slint reports the real one.
const DEFAULT_PLOT_SIZE: (f32, f32) = (600.0, 300.0);

/// Deterministic "today" for mock data: Monday 2026-09-21, 16:00 local.
const TODAY_DAYS_SINCE_EPOCH: i64 = 20_717;
const NOW_MINUTE_OF_DAY: u32 = 16 * 60;
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Data shapes used to exercise edge cases at runtime and in tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardScenario {
    Typical,
    Empty,
    OnePoint,
    AllZero,
    Identical,
    Outlier,
}

impl DashboardScenario {
    pub const ALL: [DashboardScenario; 6] = [
        DashboardScenario::Typical,
        DashboardScenario::Empty,
        DashboardScenario::OnePoint,
        DashboardScenario::AllZero,
        DashboardScenario::Identical,
        DashboardScenario::Outlier,
    ];

    pub fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Self::Typical)
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DashboardSnapshot {
    pub scenario: DashboardScenario,
    pub summary_cards: Vec<SummaryCard>,
    pub weekly: WeeklyChart,
    pub history: HistoryChart,
    pub courses: CourseBreakdown,
    pub sessions: Vec<RecentSession>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryCard {
    pub label: String,
    pub value: String,
    pub detail: String,
    pub tone: i32,
}

/// A tick or label positioned along an axis (`position` is a 0..1 fraction from top or left).
#[derive(Debug, Clone, PartialEq)]
pub struct AxisMark {
    pub position: f32,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeeklyBar {
    pub day: String,
    pub minutes: u32,
    /// Bar height as a fraction of the (nice) axis maximum.
    pub fraction: f32,
    pub is_today: bool,
    /// Duration only ("2h 35m") for tooltips; `detail` is the full "Mon · 2h 35m" text.
    pub value: String,
    pub detail: String,
    pub tone: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeeklyChart {
    pub bars: Vec<WeeklyBar>,
    pub y_ticks: Vec<AxisMark>,
    pub selected: usize,
    pub total_minutes: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryPoint {
    pub label: String,
    pub minutes: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryChart {
    pub points: Vec<HistoryPoint>,
    /// Normalized positions, one per point. Same length as `points`.
    pub positions: Vec<(f32, f32)>,
    /// Path commands in plot pixels for `plot_size`.
    pub line_commands: String,
    pub area_commands: String,
    pub plot_size: (f32, f32),
    pub y_ticks: Vec<AxisMark>,
    pub x_marks: Vec<AxisMark>,
    pub selected: Option<usize>,
    pub total_minutes: u32,
    pub peak_minutes: u32,
    pub active_days: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CourseStatistic {
    pub name: String,
    pub detail: String,
    pub minutes: u32,
    /// Whole percent; the set of percentages always sums to exactly 100 (or all 0 when empty).
    pub percent: u8,
    pub fraction: f32,
    /// Cumulative fraction of all preceding courses (left edge in the stacked share bar).
    pub start: f32,
    pub health: u8,
    pub tone: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CourseBreakdown {
    pub courses: Vec<CourseStatistic>,
    pub total_minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentSession {
    pub course: String,
    pub kind: String,
    pub minutes: u32,
    pub when: String,
    pub tone: i32,
}

// ---------------------------------------------------------------------------------------------
// Snapshot construction (mock data)
// ---------------------------------------------------------------------------------------------

impl DashboardSnapshot {
    pub fn build(scenario: DashboardScenario, history_points: usize) -> Self {
        let weekly_minutes: [u32; 7] = match scenario {
            DashboardScenario::Empty => [0; 7],
            DashboardScenario::AllZero => [0; 7],
            DashboardScenario::Identical => [90; 7],
            DashboardScenario::Outlier => [30, 35, 28, 26, 32, 40, 480],
            _ => [82, 0, 126, 45, 94, 118, 155],
        };
        let history = match scenario {
            DashboardScenario::Empty => HistoryChart::prepare(Vec::new()),
            DashboardScenario::OnePoint => HistoryChart::prepare(labelled(&[75])),
            DashboardScenario::AllZero => HistoryChart::prepare(labelled(&vec![0; 30])),
            DashboardScenario::Identical => HistoryChart::prepare(labelled(&vec![90; 30])),
            DashboardScenario::Outlier => {
                let mut values = mock_history_values(30);
                values[17] = 960;
                HistoryChart::prepare(labelled(&values))
            }
            DashboardScenario::Typical => {
                let mut values = mock_history_values(history_points);
                // Keep the last seven days consistent with the weekly bar chart.
                let n = values.len();
                for (i, minutes) in weekly_minutes.iter().enumerate().take(n.min(7)) {
                    values[n - n.min(7) + i] = *minutes;
                }
                HistoryChart::prepare(labelled(&values))
            }
        };
        let courses = match scenario {
            DashboardScenario::Empty => CourseBreakdown::prepare(Vec::new()),
            _ => CourseBreakdown::prepare(mock_courses(scenario)),
        };
        let sessions = match scenario {
            DashboardScenario::Empty => Vec::new(),
            _ => mock_sessions(&courses),
        };
        let weekly = WeeklyChart::prepare(&weekly_minutes);
        let summary_cards = summary_cards(&weekly, &history, &sessions);
        Self {
            scenario,
            summary_cards,
            weekly,
            history,
            courses,
            sessions,
        }
    }
}

fn summary_cards(
    weekly: &WeeklyChart,
    history: &HistoryChart,
    sessions: &[RecentSession],
) -> Vec<SummaryCard> {
    const DAILY_GOAL: u32 = 120;
    let today = weekly.bars.last().map(|b| b.minutes).unwrap_or(0);
    let active_week = weekly.bars.iter().filter(|b| b.minutes > 0).count();
    let streak = current_streak(&history.points);
    let sessions_today = sessions
        .iter()
        .filter(|s| s.when.starts_with("Today"))
        .count();
    vec![
        SummaryCard {
            label: "TODAY".into(),
            value: format_minutes(today),
            detail: format!(
                "Goal {} · {}%",
                format_minutes(DAILY_GOAL),
                percent_of(today, DAILY_GOAL)
            ),
            tone: 0,
        },
        SummaryCard {
            label: "THIS WEEK".into(),
            value: format_minutes(weekly.total_minutes),
            detail: format!("{active_week} of {} days active", weekly.bars.len()),
            tone: 1,
        },
        SummaryCard {
            label: "STREAK".into(),
            value: if streak == 1 {
                "1 day".into()
            } else {
                format!("{streak} days")
            },
            detail: if streak == 0 {
                "Start a session to begin".into()
            } else {
                "Consecutive study days".into()
            },
            tone: 2,
        },
        SummaryCard {
            label: "SESSIONS".into(),
            value: sessions.len().to_string(),
            detail: format!("{sessions_today} completed today"),
            tone: 3,
        },
    ]
}

/// Consecutive non-zero days ending today; if today is empty, count from yesterday
/// (mirrors `getStreakDays` in the production `desktop/src/lib/metrics.ts`).
pub fn current_streak(points: &[HistoryPoint]) -> u32 {
    let mut end = points.len();
    if end > 0 && points[end - 1].minutes == 0 {
        end -= 1;
    }
    points[..end]
        .iter()
        .rev()
        .take_while(|p| p.minutes > 0)
        .count() as u32
}

fn labelled(values: &[u32]) -> Vec<HistoryPoint> {
    let n = values.len() as i64;
    values
        .iter()
        .enumerate()
        .map(|(i, minutes)| HistoryPoint {
            label: date_label(TODAY_DAYS_SINCE_EPOCH - (n - 1 - i as i64)),
            minutes: *minutes,
        })
        .collect()
}

/// Deterministic, plausible-looking study history: rest days, weekday variation, no RNG.
pub fn mock_history_values(count: usize) -> Vec<u32> {
    (0..count)
        .map(|i| {
            let rest = i % 9 == 3 || i % 17 == 0;
            if rest {
                0
            } else {
                20 + ((i * 37 + 19) % 140) as u32 + ((i * i + 11) % 40) as u32
            }
        })
        .collect()
}

fn mock_courses(scenario: DashboardScenario) -> Vec<(&'static str, &'static str, u32, u8, i32)> {
    if scenario == DashboardScenario::AllZero {
        return vec![
            ("Analysis II", "No sessions yet", 0, 0, 0),
            ("日本語", "No sessions yet", 0, 0, 1),
        ];
    }
    vec![
        ("Analysis II", "Problem sets · target 5.5", 510, 86, 0),
        ("日本語 (Japanese)", "Vocabulary review · N4", 280, 72, 1),
        (
            "Quantum Mechanics",
            "第4章 revision · 3 open tasks",
            365,
            64,
            2,
        ),
        (
            "Seminar: Computational Models of Cognition and Learning",
            "Reading queue · 3 papers",
            145,
            48,
            3,
        ),
    ]
}

fn mock_sessions(courses: &CourseBreakdown) -> Vec<RecentSession> {
    const KINDS: [&str; 4] = ["Pomodoro", "Deep Work", "Stopwatch", "Exam block"];
    if courses.courses.is_empty() {
        return Vec::new();
    }
    let mut minute_of_day = NOW_MINUTE_OF_DAY as i64 - 45;
    let mut day = TODAY_DAYS_SINCE_EPOCH;
    (0..48usize)
        .map(|i| {
            let course = &courses.courses[(i * 7 + i / 3) % courses.courses.len()];
            let minutes = [25u32, 52, 38, 90, 25, 45][(i * 5) % 6];
            let session = RecentSession {
                course: course.name.clone(),
                kind: KINDS[i % KINDS.len()].to_string(),
                minutes,
                when: relative_when(day, minute_of_day as u32),
                tone: course.tone,
            };
            minute_of_day -= 210 + (i as i64 % 3) * 40;
            while minute_of_day < 8 * 60 {
                minute_of_day += 12 * 60;
                day -= 1;
            }
            session
        })
        .collect()
}

fn relative_when(day: i64, minute_of_day: u32) -> String {
    let clock = format!("{:02}:{:02}", minute_of_day / 60, minute_of_day % 60);
    match TODAY_DAYS_SINCE_EPOCH - day {
        0 => format!("Today {clock}"),
        1 => format!("Yesterday {clock}"),
        _ => format!("{} {clock}", date_label(day)),
    }
}

// ---------------------------------------------------------------------------------------------
// Chart preparation
// ---------------------------------------------------------------------------------------------

/// Chooses a "nice" axis maximum and step for minute values. Always returns finite, positive
/// numbers with at most four intervals, even for empty or all-zero data.
pub fn nice_scale(max_value: u32) -> (u32, u32) {
    const STEPS: [u32; 15] = [
        5, 10, 15, 30, 60, 120, 180, 240, 360, 480, 600, 720, 960, 1200, 1440,
    ];
    // Empty data still gets a readable 0..1h axis; ~5% headroom keeps peaks off the top edge.
    let needed = max_value.max(60);
    let needed = needed + needed.div_ceil(20);
    for step in STEPS {
        if needed.div_ceil(step) <= 4 {
            return (needed.div_ceil(step) * step, step);
        }
    }
    let step = needed.div_ceil(4).next_multiple_of(60);
    (needed.div_ceil(step) * step, step)
}

fn y_ticks(top: u32, step: u32) -> Vec<AxisMark> {
    (0..=top / step)
        .map(|i| AxisMark {
            position: 1.0 - (i * step) as f32 / top as f32,
            label: axis_label(i * step),
        })
        .collect()
}

fn axis_label(minutes: u32) -> String {
    if minutes == 0 {
        "0".into()
    } else if minutes % 60 == 0 {
        format!("{}h", minutes / 60)
    } else if minutes > 60 {
        format!("{}h{:02}", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m")
    }
}

impl WeeklyChart {
    pub fn prepare(minutes: &[u32; 7]) -> Self {
        let max = minutes.iter().copied().max().unwrap_or(0);
        let (top, step) = nice_scale(max);
        let today = minutes.len().saturating_sub(1);
        let bars = minutes
            .iter()
            .enumerate()
            .map(|(i, m)| {
                // Day names count back from today (Monday in the mock clock).
                let weekday =
                    ((TODAY_DAYS_SINCE_EPOCH - (today - i) as i64 + 3).rem_euclid(7)) as usize;
                let day = WEEKDAYS[weekday].to_string();
                WeeklyBar {
                    value: format_minutes(*m),
                    detail: format!("{day} · {}", format_minutes(*m)),
                    day,
                    minutes: *m,
                    fraction: *m as f32 / top as f32,
                    is_today: i == today,
                    tone: 0,
                }
            })
            .collect();
        Self {
            bars,
            y_ticks: y_ticks(top, step),
            selected: today,
            total_minutes: minutes.iter().sum(),
        }
    }

    pub fn select(&mut self, index: usize) {
        if !self.bars.is_empty() {
            self.selected = index.min(self.bars.len() - 1);
        }
    }

    pub fn step(&mut self, delta: i32) {
        self.select(step_index(self.selected, delta, self.bars.len()));
    }
}

impl HistoryChart {
    pub fn prepare(points: Vec<HistoryPoint>) -> Self {
        let max = points.iter().map(|p| p.minutes).max().unwrap_or(0);
        let (top, step) = nice_scale(max);
        let n = points.len();
        let positions: Vec<(f32, f32)> = points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let x = if n <= 1 {
                    0.5
                } else {
                    i as f32 / (n - 1) as f32
                };
                (x, 1.0 - p.minutes as f32 / top as f32)
            })
            .collect();
        let (line_commands, area_commands) = path_commands(&positions, DEFAULT_PLOT_SIZE);
        let x_marks = x_marks(&points, &positions);
        Self {
            total_minutes: points.iter().map(|p| p.minutes).sum(),
            peak_minutes: max,
            active_days: points.iter().filter(|p| p.minutes > 0).count() as u32,
            selected: n.checked_sub(1),
            y_ticks: y_ticks(top, step),
            points,
            positions,
            line_commands,
            area_commands,
            plot_size: DEFAULT_PLOT_SIZE,
            x_marks,
        }
    }

    /// Re-emits path commands for a new plot size in pixels. Invalid sizes yield empty paths.
    pub fn set_plot_size(&mut self, width: f32, height: f32) {
        self.plot_size = (width, height);
        (self.line_commands, self.area_commands) = path_commands(&self.positions, self.plot_size);
    }

    /// Index of the point nearest to a 0..1 horizontal fraction. Points are evenly spaced, so
    /// this is O(1) regardless of dataset size.
    pub fn nearest_index(&self, fraction: f32) -> Option<usize> {
        match self.points.len() {
            0 => None,
            1 => Some(0),
            n => {
                let f = if fraction.is_finite() {
                    fraction.clamp(0.0, 1.0)
                } else {
                    0.0
                };
                Some((f * (n - 1) as f32).round() as usize)
            }
        }
    }

    pub fn select_fraction(&mut self, fraction: f32) {
        self.selected = self.nearest_index(fraction);
    }

    pub fn step(&mut self, delta: i32) {
        if let Some(current) = self.selected {
            self.selected = Some(step_index(current, delta, self.points.len()));
        }
    }
}

fn step_index(current: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (current as i64 + delta as i64).clamp(0, len as i64 - 1) as usize
}

fn path_commands(positions: &[(f32, f32)], (width, height): (f32, f32)) -> (String, String) {
    // A single point has no line or area; the selection marker still shows it.
    let valid = width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0;
    if positions.len() < 2 || !valid {
        return (String::new(), String::new());
    }
    let mut line = String::with_capacity(positions.len() * 16);
    for (i, (x, y)) in positions.iter().enumerate() {
        line.push_str(if i == 0 { "M " } else { " L " });
        line.push_str(&format!("{:.2} {:.2}", x * width, y * height));
    }
    let first_x = positions[0].0 * width;
    let last_x = positions[positions.len() - 1].0 * width;
    let area = format!("{line} L {last_x:.2} {height:.2} L {first_x:.2} {height:.2} Z");
    (line, area)
}

fn x_marks(points: &[HistoryPoint], positions: &[(f32, f32)]) -> Vec<AxisMark> {
    let n = points.len();
    if n == 0 {
        return Vec::new();
    }
    let wanted = n.min(5);
    if wanted == 1 {
        return vec![AxisMark {
            position: positions[0].0,
            label: short_label(&points[0].label),
        }];
    }
    (0..wanted)
        .map(|k| {
            let i = k * (n - 1) / (wanted - 1);
            AxisMark {
                position: positions[i].0,
                label: short_label(&points[i].label),
            }
        })
        .collect()
}

/// "Mon 21 Sep" -> "21 Sep".
fn short_label(label: &str) -> String {
    label
        .split_once(' ')
        .map(|(_, rest)| rest)
        .unwrap_or(label)
        .to_string()
}

impl CourseBreakdown {
    pub fn prepare(input: Vec<(&str, &str, u32, u8, i32)>) -> Self {
        let total: u32 = input.iter().map(|c| c.2).sum();
        let shares: Vec<u32> = input.iter().map(|c| c.2).collect();
        let percents = largest_remainder_percents(&shares);
        let mut start = 0.0;
        let courses = input
            .into_iter()
            .zip(percents)
            .map(|((name, detail, minutes, health, tone), percent)| {
                let fraction = if total == 0 {
                    0.0
                } else {
                    minutes as f32 / total as f32
                };
                let course = CourseStatistic {
                    name: name.to_string(),
                    detail: detail.to_string(),
                    minutes,
                    percent,
                    fraction,
                    start,
                    health,
                    tone,
                };
                start += fraction;
                course
            })
            .collect();
        Self {
            courses,
            total_minutes: total,
        }
    }
}

/// Whole percentages that sum to exactly 100 (Hamilton / largest-remainder method).
pub fn largest_remainder_percents(values: &[u32]) -> Vec<u8> {
    let total: u64 = values.iter().map(|v| *v as u64).sum();
    if total == 0 {
        return vec![0; values.len()];
    }
    let mut floors: Vec<u64> = values.iter().map(|v| *v as u64 * 100 / total).collect();
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by_key(|i| std::cmp::Reverse((values[*i] as u64 * 100) % total));
    let missing = 100 - floors.iter().sum::<u64>();
    for i in order.into_iter().take(missing as usize) {
        floors[i] += 1;
    }
    floors.into_iter().map(|p| p as u8).collect()
}

// ---------------------------------------------------------------------------------------------
// Formatting and calendar helpers
// ---------------------------------------------------------------------------------------------

pub fn format_minutes(minutes: u32) -> String {
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

fn percent_of(value: u32, total: u32) -> u32 {
    if total == 0 {
        0
    } else {
        (value as u64 * 100 / total as u64) as u32
    }
}

/// "Mon 21 Sep" for a day count since 1970-01-01.
pub fn date_label(days_since_epoch: i64) -> String {
    let (_, month, day) = civil_from_days(days_since_epoch);
    // 1970-01-01 was a Thursday (index 3 with Monday = 0).
    let weekday = (days_since_epoch + 3).rem_euclid(7) as usize;
    format!(
        "{} {} {}",
        WEEKDAYS[weekday],
        day,
        MONTHS[(month - 1) as usize]
    )
}

/// Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points(values: &[u32]) -> Vec<HistoryPoint> {
        labelled(values)
    }

    fn assert_finite_unit(positions: &[(f32, f32)]) {
        for (x, y) in positions {
            assert!(x.is_finite() && y.is_finite(), "non-finite {x} {y}");
            assert!(
                (0.0..=1.0).contains(x) && (0.0..=1.0).contains(y),
                "out of range {x} {y}"
            );
        }
    }

    fn assert_path_valid(commands: &str) {
        assert!(
            !commands.contains("NaN") && !commands.contains("inf"),
            "{commands}"
        );
    }

    #[test]
    fn mock_calendar_is_a_monday() {
        assert_eq!(date_label(TODAY_DAYS_SINCE_EPOCH), "Mon 21 Sep");
        assert_eq!(date_label(TODAY_DAYS_SINCE_EPOCH - 1), "Sun 20 Sep");
        assert_eq!(date_label(0), "Thu 1 Jan");
    }

    #[test]
    fn nice_scale_is_finite_and_covers_the_maximum() {
        for max in [
            0,
            1,
            7,
            59,
            60,
            61,
            155,
            240,
            241,
            480,
            960,
            5_000,
            100_000,
            u32::MAX / 2,
        ] {
            let (top, step) = nice_scale(max);
            assert!(step > 0 && top >= step, "max {max}");
            assert!(top >= max, "top {top} < max {max}");
            assert!(
                top / step <= 4 || max > 1440 * 4,
                "too many intervals for {max}"
            );
            assert_eq!(top % step, 0);
        }
        assert_eq!(nice_scale(0), (90, 30));
        assert_eq!(nice_scale(155), (180, 60));
    }

    #[test]
    fn y_ticks_span_zero_to_top() {
        let ticks = y_ticks(180, 60);
        assert_eq!(ticks.len(), 4);
        assert_eq!(ticks[0].position, 1.0);
        assert_eq!(ticks[0].label, "0");
        assert_eq!(ticks[3].position, 0.0);
        assert_eq!(ticks[3].label, "3h");
    }

    #[test]
    fn weekly_bars_scale_with_arbitrary_values() {
        let chart = WeeklyChart::prepare(&[10, 20, 30, 40, 50, 60, 240]);
        assert_eq!(chart.bars.len(), 7);
        assert_eq!(chart.total_minutes, 450);
        assert_eq!(chart.selected, 6);
        for bar in &chart.bars {
            assert!((0.0..=1.0).contains(&bar.fraction));
        }
        // Max 240 plus headroom rounds up to a 360 axis top.
        assert!((chart.bars[6].fraction - 240.0 / 360.0).abs() < 1e-6);
        assert!((chart.bars[1].fraction - 20.0 / 360.0).abs() < 1e-6);
        assert_eq!(chart.bars[6].day, "Mon");
        assert_eq!(chart.bars[0].day, "Tue");
        assert_eq!(chart.bars[6].detail, "Mon · 4h");
    }

    #[test]
    fn weekly_selection_clamps_and_steps() {
        let mut chart = WeeklyChart::prepare(&[1, 2, 3, 4, 5, 6, 7]);
        chart.select(99);
        assert_eq!(chart.selected, 6);
        chart.step(-100);
        assert_eq!(chart.selected, 0);
        chart.step(1);
        assert_eq!(chart.selected, 1);
    }

    #[test]
    fn weekly_all_zero_has_no_nan() {
        let chart = WeeklyChart::prepare(&[0; 7]);
        assert!(chart.bars.iter().all(|b| b.fraction == 0.0));
        assert!(chart.y_ticks.iter().all(|t| t.position.is_finite()));
    }

    #[test]
    fn history_empty() {
        let chart = HistoryChart::prepare(Vec::new());
        assert!(chart.line_commands.is_empty() && chart.area_commands.is_empty());
        assert!(chart.x_marks.is_empty());
        assert_eq!(chart.selected, None);
        assert_eq!(chart.nearest_index(0.5), None);
        assert!(chart.y_ticks.iter().all(|t| t.position.is_finite()));
    }

    #[test]
    fn history_one_point_is_centered_without_path() {
        let chart = HistoryChart::prepare(points(&[75]));
        assert_eq!(chart.positions, vec![(0.5, 1.0 - 75.0 / 90.0)]);
        assert!(chart.line_commands.is_empty());
        assert_eq!(chart.selected, Some(0));
        assert_eq!(chart.nearest_index(0.9), Some(0));
        assert_eq!(chart.x_marks.len(), 1);
    }

    #[test]
    fn history_all_zero_sits_on_the_baseline() {
        let chart = HistoryChart::prepare(points(&[0; 30]));
        assert_finite_unit(&chart.positions);
        assert!(chart.positions.iter().all(|(_, y)| *y == 1.0));
        assert_eq!(chart.active_days, 0);
        assert_path_valid(&chart.line_commands);
    }

    #[test]
    fn history_identical_values_are_flat_and_inside_the_plot() {
        let chart = HistoryChart::prepare(points(&[90; 30]));
        assert_finite_unit(&chart.positions);
        let y0 = chart.positions[0].1;
        assert!(chart.positions.iter().all(|(_, y)| *y == y0));
        assert!(y0 > 0.0 && y0 < 1.0);
    }

    #[test]
    fn history_outlier_does_not_flatten_or_escape() {
        let mut values = vec![30; 30];
        values[10] = 5_000;
        let chart = HistoryChart::prepare(points(&values));
        assert_finite_unit(&chart.positions);
        assert!(chart.positions[10].1 < 0.2);
        assert!(chart.positions[9].1 > 0.9);
        assert_eq!(chart.peak_minutes, 5_000);
    }

    #[test]
    fn history_paths_are_well_formed_for_all_sizes() {
        for n in [2usize, 30, 365, 1_000, 5_000] {
            let chart = HistoryChart::prepare(points(&mock_history_values(n)));
            assert_finite_unit(&chart.positions);
            assert_path_valid(&chart.line_commands);
            assert!(chart.line_commands.starts_with("M "));
            assert_eq!(chart.line_commands.matches('L').count(), n - 1);
            assert!(chart.area_commands.ends_with('Z'));
            assert_eq!(chart.positions[0].0, 0.0);
            assert_eq!(chart.positions[n - 1].0, 1.0);
        }
    }

    #[test]
    fn plot_size_scales_paths_to_pixels_and_rejects_invalid_sizes() {
        let mut chart = HistoryChart::prepare(points(&[0, 60, 0]));
        chart.set_plot_size(200.0, 100.0);
        assert!(chart.line_commands.starts_with("M 0.00 "));
        assert!(chart.line_commands.contains("L 200.00 "));
        assert!(chart.area_commands.contains("100.00"));
        for (w, h) in [
            (0.0, 10.0),
            (-5.0, 10.0),
            (f32::NAN, 10.0),
            (10.0, f32::INFINITY),
        ] {
            chart.set_plot_size(w, h);
            assert!(chart.line_commands.is_empty() && chart.area_commands.is_empty());
        }
        chart.set_plot_size(1_000.0, 1_000.0);
        assert!(!chart.line_commands.is_empty());
    }

    #[test]
    fn history_selection_maps_fractions_and_steps() {
        let mut chart = HistoryChart::prepare(points(&mock_history_values(11)));
        assert_eq!(chart.selected, Some(10));
        chart.select_fraction(0.0);
        assert_eq!(chart.selected, Some(0));
        chart.select_fraction(0.5);
        assert_eq!(chart.selected, Some(5));
        chart.select_fraction(0.54);
        assert_eq!(chart.selected, Some(5));
        chart.select_fraction(0.56);
        assert_eq!(chart.selected, Some(6));
        for bad in [f32::NAN, f32::INFINITY, -3.0, 9.0] {
            chart.select_fraction(bad);
            assert!(chart.selected.unwrap() <= 10);
        }
        chart.select_fraction(0.0);
        chart.step(-1);
        assert_eq!(chart.selected, Some(0));
        chart.step(1_000_000);
        assert_eq!(chart.selected, Some(10));
    }

    #[test]
    fn x_marks_are_evenly_spread_and_include_ends() {
        let chart = HistoryChart::prepare(points(&mock_history_values(365)));
        assert_eq!(chart.x_marks.len(), 5);
        assert_eq!(chart.x_marks[0].position, 0.0);
        assert_eq!(chart.x_marks[4].position, 1.0);
        assert_eq!(chart.x_marks[4].label, "21 Sep");
        let short = HistoryChart::prepare(points(&[10, 20, 30]));
        assert_eq!(short.x_marks.len(), 3);
    }

    #[test]
    fn percentages_always_sum_to_one_hundred() {
        for values in [
            vec![510, 280, 365, 145],
            vec![1, 1, 1],
            vec![1, 1],
            vec![3, 3, 3, 3, 3, 3, 3],
            vec![0, 5, 0],
            vec![7],
        ] {
            let sum: u32 = largest_remainder_percents(&values)
                .iter()
                .map(|p| *p as u32)
                .sum();
            assert_eq!(sum, 100, "{values:?}");
        }
        assert_eq!(largest_remainder_percents(&[0, 0]), vec![0, 0]);
        assert!(largest_remainder_percents(&[]).is_empty());
    }

    #[test]
    fn course_breakdown_handles_empty_and_zero_totals() {
        let empty = CourseBreakdown::prepare(Vec::new());
        assert!(empty.courses.is_empty());
        let zero = CourseBreakdown::prepare(vec![("A", "", 0, 0, 0), ("日本語", "", 0, 0, 1)]);
        assert!(zero
            .courses
            .iter()
            .all(|c| c.percent == 0 && c.fraction == 0.0 && c.start == 0.0));
        let split = CourseBreakdown::prepare(vec![("A", "", 30, 0, 0), ("B", "", 70, 0, 1)]);
        assert_eq!(split.courses[1].start, split.courses[0].fraction);
        assert!((split.courses[1].start + split.courses[1].fraction - 1.0).abs() < 1e-6);
    }

    #[test]
    fn streak_follows_production_semantics() {
        assert_eq!(current_streak(&points(&[])), 0);
        assert_eq!(current_streak(&points(&[5, 0, 5, 5, 5])), 3);
        // Today empty: count back from yesterday.
        assert_eq!(current_streak(&points(&[5, 0, 5, 5, 0])), 2);
        assert_eq!(current_streak(&points(&[0, 0, 0])), 0);
    }

    #[test]
    fn format_minutes_covers_boundaries() {
        assert_eq!(format_minutes(0), "0m");
        assert_eq!(format_minutes(59), "59m");
        assert_eq!(format_minutes(60), "1h");
        assert_eq!(format_minutes(155), "2h 35m");
    }

    #[test]
    fn typical_snapshot_is_internally_consistent() {
        let snapshot = DashboardSnapshot::build(DashboardScenario::Typical, 30);
        assert_eq!(snapshot.summary_cards.len(), 4);
        assert_eq!(snapshot.summary_cards[0].value, "2h 35m");
        assert_eq!(snapshot.summary_cards[1].value, "10h 20m");
        assert_eq!(snapshot.summary_cards[2].value, "5 days");
        assert_eq!(snapshot.sessions.len(), 48);
        // History tail agrees with the weekly chart.
        let tail: Vec<u32> = snapshot.history.points[23..]
            .iter()
            .map(|p| p.minutes)
            .collect();
        let weekly: Vec<u32> = snapshot.weekly.bars.iter().map(|b| b.minutes).collect();
        assert_eq!(tail, weekly);
        let percent: u32 = snapshot
            .courses
            .courses
            .iter()
            .map(|c| c.percent as u32)
            .sum();
        assert_eq!(percent, 100);
        assert!(snapshot
            .courses
            .courses
            .iter()
            .any(|c| c.name.chars().count() > 40));
        assert!(snapshot
            .courses
            .courses
            .iter()
            .any(|c| c.name.contains("日本語")));
        assert!(snapshot
            .sessions
            .iter()
            .any(|s| s.course.contains("日本語")));
    }

    #[test]
    fn every_scenario_and_dataset_size_builds_valid_geometry() {
        for scenario in DashboardScenario::ALL {
            for count in [0usize, 1, 2, 30, 365, 1_000] {
                let s = DashboardSnapshot::build(scenario, count);
                assert_finite_unit(&s.history.positions);
                assert_path_valid(&s.history.line_commands);
                assert_path_valid(&s.history.area_commands);
                assert_eq!(s.summary_cards.len(), 4);
                assert!(s
                    .weekly
                    .bars
                    .iter()
                    .all(|b| b.fraction.is_finite() && b.fraction <= 1.0));
                assert!(s.courses.courses.iter().all(|c| c.fraction.is_finite()));
            }
        }
    }

    #[test]
    fn empty_scenario_has_no_sessions_and_zero_cards() {
        let s = DashboardSnapshot::build(DashboardScenario::Empty, 30);
        assert!(s.sessions.is_empty() && s.courses.courses.is_empty());
        assert_eq!(s.summary_cards[0].value, "0m");
        assert_eq!(s.summary_cards[2].value, "0 days");
        assert_eq!(s.summary_cards[3].value, "0");
    }

    #[test]
    fn scenario_index_round_trips() {
        for scenario in DashboardScenario::ALL {
            assert_eq!(DashboardScenario::from_index(scenario.index()), scenario);
        }
        assert_eq!(
            DashboardScenario::from_index(99),
            DashboardScenario::Typical
        );
    }

    /// Manual timing report: `cargo test --release preparation_cost_report -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn preparation_cost_report() {
        use std::time::Instant;
        for n in [30usize, 365, 1_000, 10_000] {
            let start = Instant::now();
            let runs = 200;
            for _ in 0..runs {
                std::hint::black_box(DashboardSnapshot::build(DashboardScenario::Typical, n));
            }
            let build = start.elapsed() / runs;
            let mut chart = HistoryChart::prepare(points(&mock_history_values(n)));
            let start = Instant::now();
            for i in 0..runs {
                chart.set_plot_size(700.0 + i as f32, 320.0);
            }
            let resize = start.elapsed() / runs;
            let start = Instant::now();
            for i in 0..10_000 {
                chart.select_fraction((i % 100) as f32 / 100.0);
            }
            let select = start.elapsed() / 10_000;
            println!(
                "points={n:>6}: full snapshot build {build:>10.2?}, path re-emit on resize {resize:>10.2?}, hover lookup {select:>8.2?}, path bytes {}",
                chart.line_commands.len() + chart.area_commands.len()
            );
        }
    }

    #[test]
    fn preparation_is_deterministic() {
        assert_eq!(
            DashboardSnapshot::build(DashboardScenario::Typical, 365),
            DashboardSnapshot::build(DashboardScenario::Typical, 365)
        );
    }
}
