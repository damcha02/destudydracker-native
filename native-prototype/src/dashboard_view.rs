//! Production Dashboard presentation layer (Stage 17).
//!
//! ```text
//! AcademicController.state()  ──►  study_tracker_core::dashboard::DashboardMetrics   (pure, tested)
//!                                   └─ FocusTimeline (selected range)
//!                                              │
//!                                              ▼
//!                               DashboardController::data()  ──►  Slint `FnDashboardData`
//! ```
//!
//! Everything numeric comes from the core crate's ports of production's `metrics.ts`; this module
//! only (a) decides *when* to recompute, (b) turns metrics into display strings and chart
//! geometry, and (c) keeps the Dashboard's small amount of UI state (Quiet/Full, chart range,
//! selected row). Nothing here reads the filesystem and nothing is persisted: the Dashboard is
//! always derived from canonical academic data.
//!
//! ## Recomputation policy (Stage 17 brief §20)
//!
//! Metrics are cached against `(AcademicController::revision(), local date, daily goal)`. A Timer
//! display tick changes none of those, so it never reaches [`DashboardController::sync`]'s
//! recompute path; adding a session, ticking off a planned unit, or the local date rolling over
//! does. The chart timeline is additionally keyed by the selected range. Both recomputations are
//! counted ([`ComputeStats`]) so tests and the `STUDY_NATIVE_DASHBOARD_REPORT` hook can prove it.

use std::collections::HashMap;

use chrono::{Datelike, Local, LocalResult, TimeZone};
use slint::{Color, ModelRc, SharedString, VecModel};
use study_tracker_core::academic::Priority;
use study_tracker_core::dashboard::civil::js_date_only_as_local;
use study_tracker_core::dashboard::format::{
    format_fossil_date_label, format_minutes, format_month_day, format_unit_amount, js_number,
    month_short, to_fixed,
};
use study_tracker_core::dashboard::{
    focus_timeline, CivilDate, DashboardInput, DashboardMetrics, FocusRange, FocusTimeline,
    LocalClock, QueueEntry, SessionDays, DEFAULT_DAILY_GOAL_MINUTES, FOCUS_MILESTONES,
};
use study_tracker_core::timer::WallTimestamp;

use crate::{
    FnAheadRow, FnCourseRow, FnDashboardData, FnDiscovered, FnExamRow, FnFossilDay, FnFossilLayer,
    FnFossilStat, FnLegendItem, FnQueueRow, FnTooltipLayer,
};

/// The real-runtime [`LocalClock`]: applies the OS's timezone rules *per instant*, like the
/// JavaScript `Date` production uses (a session from last July and one from today each get their
/// own offset, DST included).
#[derive(Debug, Clone, Copy, Default)]
pub struct ChronoLocalClock;

impl LocalClock for ChronoLocalClock {
    fn local_date(&self, instant: WallTimestamp) -> CivilDate {
        match Local.timestamp_millis_opt(instant.unix_millis) {
            LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => {
                CivilDate::from_ymd(dt.year(), dt.month(), dt.day()).unwrap_or_else(|| {
                    CivilDate::from_days(instant.unix_millis.div_euclid(86_400_000))
                })
            }
            // Out-of-range instants: fall back to the UTC date rather than panic.
            LocalResult::None => CivilDate::from_days(instant.unix_millis.div_euclid(86_400_000)),
        }
    }

    fn local_midnight(&self, date: CivilDate) -> WallTimestamp {
        let (y, m, d) = date.ymd();
        let resolved = match Local.with_ymd_and_hms(y, m, d, 0, 0, 0) {
            LocalResult::Single(dt) => Some(dt),
            LocalResult::Ambiguous(first, _) => Some(first),
            // Midnight does not exist on this date (a DST jump over 00:00): JS `new Date` moves
            // forward; the first valid instant of the day is the closest equivalent.
            LocalResult::None => Local.with_ymd_and_hms(y, m, d, 1, 0, 0).earliest(),
        };
        WallTimestamp::from_unix_millis(
            resolved.map_or(date.days() * 86_400_000, |dt| dt.timestamp_millis()),
        )
    }
}

/// Dashboard-only UI state (production: `fieldDashboardLayout`, `focusRange`,
/// `selectedQuietDashboardEntryId`).
#[derive(Debug, Clone, PartialEq)]
pub struct DashboardUiState {
    pub full: bool,
    pub range: FocusRange,
    pub selected_entry: Option<String>,
}

impl Default for DashboardUiState {
    fn default() -> Self {
        // Production defaults: the Quiet layout, and the chart opens on "This week".
        Self {
            full: false,
            range: FocusRange::Week,
            selected_entry: None,
        }
    }
}

/// How many times each expensive derivation actually ran.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComputeStats {
    pub metrics: u64,
    pub timelines: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CacheKey {
    revision: u64,
    today: CivilDate,
    goal: u32,
}

pub struct DashboardController {
    ui: DashboardUiState,
    daily_goal_minutes: u32,
    key: Option<CacheKey>,
    metrics: Option<DashboardMetrics>,
    timeline: Option<(CacheKey, FocusRange, FocusTimeline)>,
    now: WallTimestamp,
    stats: ComputeStats,
}

impl DashboardController {
    pub fn new(ui: DashboardUiState) -> Self {
        Self {
            ui,
            daily_goal_minutes: DEFAULT_DAILY_GOAL_MINUTES,
            key: None,
            metrics: None,
            timeline: None,
            now: WallTimestamp::from_unix_millis(0),
            stats: ComputeStats::default(),
        }
    }

    /// True when the cached metrics were computed for a different academic revision (cheap: one
    /// integer comparison, no clock or timezone work). Used on every Timer command/tick.
    pub fn is_stale(&self, revision: u64) -> bool {
        self.key.map_or(true, |k| k.revision != revision)
    }

    /// Like [`Self::is_stale`] but also notices the local date rolling over.
    pub fn is_stale_for(&self, revision: u64, now: WallTimestamp, clock: &dyn LocalClock) -> bool {
        self.key
            != Some(CacheKey {
                revision,
                today: clock.local_date(now),
                goal: self.daily_goal_minutes,
            })
    }

    pub fn ui_mut(&mut self) -> &mut DashboardUiState {
        &mut self.ui
    }

    pub fn stats(&self) -> ComputeStats {
        self.stats
    }

    pub fn metrics(&self) -> Option<&DashboardMetrics> {
        self.metrics.as_ref()
    }

    /// Brings the cached derivations up to date for `state`/`now`; returns `true` if the metrics
    /// were recomputed (so the caller knows the window needs fresh data). Cheap when nothing
    /// changed: one comparison of a `(revision, date, goal)` triple.
    pub fn sync(
        &mut self,
        state: &study_tracker_core::academic::AcademicState,
        revision: u64,
        now: WallTimestamp,
        clock: &dyn LocalClock,
    ) -> bool {
        let key = CacheKey {
            revision,
            today: clock.local_date(now),
            goal: self.daily_goal_minutes,
        };
        self.now = now;
        let mut recomputed = false;
        if self.key != Some(key) {
            self.metrics = Some(DashboardMetrics::compute(DashboardInput {
                state,
                now,
                clock,
                daily_goal_minutes: self.daily_goal_minutes,
            }));
            self.key = Some(key);
            self.stats.metrics += 1;
            recomputed = true;
        }
        let needs_timeline = self.ui.full
            && self
                .timeline
                .as_ref()
                .map_or(true, |(k, range, _)| *k != key || *range != self.ui.range);
        if needs_timeline {
            let sessions = SessionDays::new(&state.sessions, clock);
            self.timeline = Some((
                key,
                self.ui.range,
                focus_timeline(
                    self.ui.range,
                    key.today,
                    &sessions,
                    &state.courses,
                    state.lifetime_study_minutes,
                ),
            ));
            self.stats.timelines += 1;
            recomputed = true;
        }
        recomputed
    }

    /// Streak for the sidebar/diagnostics without rebuilding the whole view model.
    pub fn sidebar_text(&self) -> (String, String) {
        match &self.metrics {
            Some(m) => (
                format_minutes(m.today_minutes),
                format!("Streak {}", m.streak_days),
            ),
            None => ("0m".into(), "Streak 0".into()),
        }
    }

    /// Builds the Slint data for the current layout. Pure with respect to `self`.
    pub fn data(&self, clock: &dyn LocalClock) -> FnDashboardData {
        let Some(m) = &self.metrics else {
            return FnDashboardData::default();
        };
        build_data(m, self.timeline.as_ref().map(|t| &t.2), &self.ui, clock)
    }
}

// --- presentation helpers -------------------------------------------------------------------------

fn ss(value: impl AsRef<str>) -> SharedString {
    SharedString::from(value.as_ref())
}

fn model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(rows))
}

/// CSS colour -> Slint colour. Production stores course colours as CSS strings (almost always
/// `#rrggbb` from its colour picker); a few named colours appear in hand-edited data and in the
/// sanitized fixture, so the common ones are understood too. Anything else falls back to the
/// course-blue of the Field Notebook palette rather than failing.
pub fn parse_css_color(value: &str) -> Color {
    let v = value.trim();
    if let Some(hex) = v.strip_prefix('#') {
        let digits: Option<Vec<u8>> = hex
            .chars()
            .map(|c| c.to_digit(16).map(|d| d as u8))
            .collect();
        if let Some(d) = digits {
            return match d.len() {
                3 => Color::from_rgb_u8(d[0] * 17, d[1] * 17, d[2] * 17),
                4 => Color::from_argb_u8(d[3] * 17, d[0] * 17, d[1] * 17, d[2] * 17),
                6 => Color::from_rgb_u8(d[0] * 16 + d[1], d[2] * 16 + d[3], d[4] * 16 + d[5]),
                8 => Color::from_argb_u8(
                    d[6] * 16 + d[7],
                    d[0] * 16 + d[1],
                    d[2] * 16 + d[3],
                    d[4] * 16 + d[5],
                ),
                _ => FALLBACK_COURSE_COLOR,
            };
        }
    }
    match v.to_ascii_lowercase().as_str() {
        "blue" => Color::from_rgb_u8(0, 0, 255),
        "red" => Color::from_rgb_u8(255, 0, 0),
        "green" => Color::from_rgb_u8(0, 128, 0),
        "orange" => Color::from_rgb_u8(255, 165, 0),
        "purple" => Color::from_rgb_u8(128, 0, 128),
        "yellow" => Color::from_rgb_u8(255, 255, 0),
        "pink" => Color::from_rgb_u8(255, 192, 203),
        "teal" => Color::from_rgb_u8(0, 128, 128),
        "gray" | "grey" => Color::from_rgb_u8(128, 128, 128),
        "black" => Color::from_rgb_u8(0, 0, 0),
        "white" => Color::from_rgb_u8(255, 255, 255),
        _ => FALLBACK_COURSE_COLOR,
    }
}

/// `--fn-course-blue` (dark palette), production's fallback when an entry has no course.
const FALLBACK_COURSE_COLOR: Color = Color::from_rgb_u8(0x71, 0x87, 0xb4);
/// `--ink-4` (dark): the colour of the "General" layer.
const GENERAL_LAYER_COLOR: Color = Color::from_rgb_u8(0x81, 0x78, 0x67);

fn px(value: f32) -> f32 {
    value
}

/// Production's Field Notebook style applies `filter: grayscale(0.25) contrast(0.92)` to the whole
/// focus-history card (`:root[data-app-style="field-notebook"] .fossil-card`). Slint has no
/// per-subtree filters, so the chart's own colours are pre-filtered with the same two operations
/// (CSS `grayscale` uses the Rec.709 luma matrix; `contrast` pivots around mid-grey).
pub fn chart_filter(color: Color) -> Color {
    let (r, g, b) = (
        f64::from(color.red()),
        f64::from(color.green()),
        f64::from(color.blue()),
    );
    let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let apply = |c: f64| {
        ((c + (luma - c) * 0.25 - 127.5) * 0.92 + 127.5)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color::from_argb_u8(color.alpha(), apply(r), apply(g), apply(b))
}

/// `fossilRand`: a hash-based pseudo-random in `[0, 1)` so a given day/layer always gets the same
/// irregular "fossil" edge. Bit-exact port (`Math.imul` = wrapping 32-bit multiply; `>>>` =
/// unsigned shift).
pub fn fossil_rand(seed: i32) -> f64 {
    let mut s = seed.wrapping_mul(2_654_435_761u32 as i32);
    s = (((s as u32) >> 16) as i32 ^ s).wrapping_mul(0x45d9f3b);
    s = (((s as u32) >> 16) as i32 ^ s).wrapping_mul(0x45d9f3b);
    f64::from(((s as u32) >> 16) ^ (s as u32)) / 4_294_967_296.0
}

/// `FossilMilestoneIcon` picks its glyph by hours.
fn milestone_icon(hours: u32) -> i32 {
    match hours {
        0..=10 => 0,
        11..=25 => 1,
        26..=50 => 2,
        51..=100 => 3,
        _ => 4,
    }
}

fn priority_tone(priority: Priority) -> i32 {
    match priority {
        Priority::High => 0,
        Priority::Medium => 1,
        Priority::Low => 2,
    }
}

fn queue_row(entry: &QueueEntry, selected: bool) -> FnQueueRow {
    let due_or_time = entry
        .due_label
        .as_ref()
        .map_or_else(|| entry.time_range.clone(), |d| format!("due {d}"));
    let (chip, chip_tone) = if selected {
        (
            "selected".to_string(),
            priority_tone(if entry.completed {
                Priority::Low
            } else {
                entry.priority
            }),
        )
    } else if entry.completed {
        ("done".to_string(), 2)
    } else {
        (
            format_unit_amount(entry.amount),
            priority_tone(entry.priority),
        )
    };
    FnQueueRow {
        entry_id: ss(&entry.entry_id),
        title: ss(&entry.title),
        course: ss(entry.course_name.as_deref().unwrap_or("General")),
        course_color: entry
            .course_color
            .as_deref()
            .map_or(FALLBACK_COURSE_COLOR, parse_css_color),
        semester: ss(entry.semester_name.as_deref().unwrap_or("No semester")),
        unit_label: ss(&entry.unit_label),
        caption_upper: ss(due_or_time.to_uppercase()),
        due_or_time: ss(&due_or_time),
        chip: ss(chip),
        chip_tone,
        completed: entry.completed,
        has_task: entry.has_task,
        selected,
    }
}

/// Which entry is "selected" in a list: the UI's pick if it is in the visible list, else the
/// first (production: `selectedQuietEntry` / `selectedFullEntry`).
fn selected_index(entries: &[&QueueEntry], wanted: &Option<String>) -> Option<usize> {
    if entries.is_empty() {
        return None;
    }
    wanted
        .as_ref()
        .and_then(|id| entries.iter().position(|e| &e.entry_id == id))
        .or(Some(0))
}

fn entry_meta(entry: &QueueEntry) -> String {
    // `${unitLabel} · ${course ?? "General focus"} · ${due ? "due X" : timeRange}`
    format!(
        "{} · {} · {}",
        entry.unit_label,
        entry.course_name.as_deref().unwrap_or("General focus"),
        entry
            .due_label
            .as_ref()
            .map_or_else(|| entry.time_range.clone(), |d| format!("due {d}")),
    )
}

fn column_gap(range: FocusRange) -> f32 {
    match range {
        FocusRange::Days(365) => 0.0,
        FocusRange::Week | FocusRange::Days(7) => 4.0,
        FocusRange::Days(n) if n <= 14 => 3.0,
        FocusRange::Days(n) if n <= 30 => 2.0,
        _ => 1.0,
    }
}

/// The Full layout renders the chart with `heightClass = "short-weekly"`: a 105px plot.
const FULL_CHART_HEIGHT: f32 = 105.0;

fn build_fossil(
    m: &DashboardMetrics,
    timeline: &FocusTimeline,
    clock: &dyn LocalClock,
    data: &mut FnDashboardData,
) {
    let max_height = FULL_CHART_HEIGHT;
    let column_max = max_height - 5.0;
    let hits: HashMap<usize, usize> = {
        let mut map = HashMap::new();
        for hit in &timeline.milestone_hits {
            map.entry(hit.day_index).or_insert(hit.milestone); // production's `.find`: first hit per day
        }
        map
    };
    let week_or_short = matches!(timeline.range, FocusRange::Week)
        || matches!(timeline.range, FocusRange::Days(n) if n <= 14);

    let days: Vec<FnFossilDay> = timeline
        .days
        .iter()
        .enumerate()
        .map(|(index, day)| {
            let total = day.total_minutes as f64;
            let column_height = if day.total_minutes > 0 {
                (((total / timeline.max_day_minutes as f64).sqrt()) * f64::from(column_max))
                    .max(6.0)
            } else {
                0.0
            };
            let mut bottom = 0.0f64;
            let layers: Vec<FnFossilLayer> = day
                .layers
                .iter()
                .enumerate()
                .map(|(layer_index, layer)| {
                    let height = ((layer.minutes as f64 / total) * column_height - 1.0).max(4.0);
                    let seed = (index as i32) * 97 + (layer_index as i32) * 31;
                    let out = FnFossilLayer {
                        color: chart_filter(
                            layer
                                .color
                                .as_deref()
                                .map_or(GENERAL_LAYER_COLOR, parse_css_color),
                        ),
                        bottom: px(bottom as f32),
                        height: px(height as f32),
                        width_frac: ((82.0 + fossil_rand(seed) * 18.0) / 100.0) as f32,
                        radius_a: (1.5 + fossil_rand(seed + 1) * 4.0) as f32,
                        radius_b: (1.5 + fossil_rand(seed + 2) * 4.0) as f32,
                        radius_c: (0.5 + fossil_rand(seed + 3) * 2.5) as f32,
                        radius_d: (0.5 + fossil_rand(seed + 4) * 2.5) as f32,
                    };
                    bottom += height + 1.0; // `gap: 1px` between stacked layers
                    out
                })
                .collect();
            let date = day.date;
            let label = if week_or_short {
                ["S", "M", "T", "W", "T", "F", "S"][date.weekday() as usize].to_string()
            } else if index == 0 || date.day() == 1 {
                month_short(date).to_string()
            } else {
                String::new()
            };
            let (milestone, hours, icon, title, desc) = match hits.get(&index) {
                Some(&mi) => {
                    let (h, name, d) = FOCUS_MILESTONES[mi];
                    (
                        mi as i32,
                        format!("{h}h"),
                        milestone_icon(h),
                        name.to_string(),
                        d.to_string(),
                    )
                }
                None => (-1, String::new(), 0, String::new(), String::new()),
            };
            FnFossilDay {
                layers: model(layers),
                column_height: px(column_height as f32),
                empty: day.total_minutes == 0,
                is_today: day.is_today,
                label: ss(label),
                milestone,
                milestone_hours: ss(hours),
                milestone_icon: icon,
                milestone_title: ss(title),
                milestone_desc: ss(desc),
                tip_title: ss(format_fossil_date_label(date)),
                tip_layers: model(
                    day.layers
                        .iter()
                        .map(|l| FnTooltipLayer {
                            color: l
                                .color
                                .as_deref()
                                .map_or(GENERAL_LAYER_COLOR, parse_css_color),
                            name: ss(&l.name),
                            minutes: ss(format_minutes(l.minutes)),
                        })
                        .collect(),
                ),
                tip_total: ss(format_minutes(day.total_minutes)),
            }
        })
        .collect();

    data.days = model(days);
    data.column_gap = column_gap(timeline.range);
    data.chart_height = max_height;
    data.has_milestones = !timeline.milestone_hits.is_empty();
    data.range_labels = model(
        FocusRange::ALL
            .iter()
            .map(|r| ss(r.button_label().to_uppercase()))
            .collect(),
    );
    data.range_index = FocusRange::ALL
        .iter()
        .position(|r| *r == timeline.range)
        .unwrap_or(0) as i32;
    data.has_history = m.lifetime_minutes > 0;
    data.legend = model(
        timeline
            .active_courses
            .iter()
            .map(|c| FnLegendItem {
                color: c
                    .color
                    .as_deref()
                    .map_or(GENERAL_LAYER_COLOR, parse_css_color),
                name: ss(&c.name),
            })
            .collect(),
    );
    data.discovered = model(
        timeline
            .achieved_milestones
            .iter()
            .map(|&i| FnDiscovered {
                icon: milestone_icon(FOCUS_MILESTONES[i].0),
                label: ss(FOCUS_MILESTONES[i].1),
            })
            .collect(),
    );
    let biggest = &timeline.days[timeline
        .biggest_day
        .min(timeline.days.len().saturating_sub(1))];
    let mut stats = vec![
        FnFossilStat {
            label: ss("ACTIVE DAYS"),
            value: ss(timeline.active_days.to_string()),
            sub: ss(format!("/ {}", timeline.range.stats_denominator())),
        },
        FnFossilStat {
            label: ss("STREAK"),
            value: ss(m.streak_days.to_string()),
            sub: ss("days"),
        },
    ];
    if !timeline.days.is_empty() && biggest.total_minutes > 0 {
        stats.push(FnFossilStat {
            label: ss("BIGGEST DAY"),
            value: ss(format_minutes(biggest.total_minutes)),
            sub: ss(format_month_day(js_date_only_as_local(clock, biggest.date))),
        });
    }
    if let Some(&latest) = timeline.achieved_milestones.last() {
        stats.push(FnFossilStat {
            label: ss("LATEST FIND"),
            value: ss(FOCUS_MILESTONES[latest].1),
            sub: ss(format!("at {}h", FOCUS_MILESTONES[latest].0)),
        });
    }
    data.stats = model(stats);
}

/// Builds the complete Slint data for the current layout.
pub fn build_data(
    m: &DashboardMetrics,
    timeline: Option<&FocusTimeline>,
    ui: &DashboardUiState,
    clock: &dyn LocalClock,
) -> FnDashboardData {
    let mut d = FnDashboardData::default();
    d.full = ui.full;
    d.pill_tone = match m.overall_tone() {
        "strong" => 0,
        "steady" => 1,
        "watch" => 2,
        _ => 3,
    };
    d.pill_label = ss(m.overall_label);
    d.pill_score = ss(m.overall_score.to_string());
    d.focused = ss(format_minutes(m.today_minutes));
    d.goal = ss(format_minutes(u64::from(m.daily_goal_minutes)));
    d.goal_fill = m.goal_progress_percent as f32 / 100.0;

    let open: Vec<&QueueEntry> = m.queue.iter().filter(|e| !e.completed).collect();
    if !ui.full {
        // ---- Quiet ----
        let visible: Vec<&QueueEntry> = open.iter().copied().take(6).collect();
        let hidden = open.len().saturating_sub(visible.len());
        let selected = selected_index(&visible, &ui.selected_entry);
        d.stamp_line = ss(
            format!("{} · semester desk · week {}", m.today_label, m.week_number).to_uppercase(),
        );
        d.title_text = ss("On the desk");
        match selected.map(|i| visible[i]) {
            Some(entry) => {
                d.subtitle_strong = ss(&entry.title);
                d.subtitle_rest = ss(format!(" · {}", entry_meta(entry)));
                d.has_start = entry.has_task;
            }
            None => {
                d.subtitle_strong = ss("Choose a study block");
                d.subtitle_rest =
                    ss(" · Select a task below, or place one in the planner calendar.");
                d.has_start = false;
            }
        }
        d.today_line = ss(format!(
            "{} of {} done today.",
            format_minutes(m.today_minutes),
            format_minutes(u64::from(m.daily_goal_minutes))
        ));
        d.quiet_rows = model(
            visible
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let mut row = queue_row(e, Some(i) == selected);
                    row.chip = ss(&row.chip);
                    row
                })
                .collect(),
        );
        d.quiet_hidden = hidden as i32;
        // list height: rows (+2px margin under the selected one) + the list's own bottom rule
        d.quiet_list_height = if visible.is_empty() {
            7.0 + 21.6 + 1.0
        } else {
            let n = visible.len() as f32;
            let last_is_selected = selected == Some(visible.len() - 1);
            // selected 58 (56 + 2 margin), other rows 55, the last row has no rule (54)
            let mut h = 55.0 * n + if selected.is_some() { 3.0 } else { 0.0 };
            if !last_is_selected {
                h -= 1.0;
            }
            h + 1.0 + if hidden > 0 { 34.0 } else { 0.0 }
        };
        d.pace_lines = model(vec![
            ss(format!(
                "{} left",
                format_unit_amount(m.total_units_left as f64)
            )),
            ss(format!("{} units/day", to_fixed(m.units_per_day, 1))),
            ss(format!("{} open tasks", m.open_task_count)),
        ]);
        d.planned_today = ss(format!("{} planned today", m.queue_open_count));
        let mut ahead: Vec<FnAheadRow> = m
            .exams
            .iter()
            .take(4)
            .map(|e| FnAheadRow {
                label: ss(&e.title),
                value: ss(&e.date_label),
            })
            .collect();
        if let Some(label) = &m.nearest_deadline_label {
            ahead.push(FnAheadRow {
                label: ss("Nearest deadline"),
                value: ss(label),
            });
        }
        d.ahead = model(ahead);
        d.goal_caption =
            ss(format!("{}% · streak {} d", m.goal_progress_percent, m.streak_days).to_uppercase());
    } else {
        // ---- Full ----
        let queue: Vec<&QueueEntry> = m.queue.iter().take(5).collect();
        let selected = selected_index(&queue, &ui.selected_entry);
        d.stamp_line = ss(format!("{} · week {}", m.today_label, m.week_number).to_uppercase());
        d.title_text = ss("Today’s desk");
        d.subtitle_rest = ss(format!(
            "Semester work · {} open tasks · {} left{}",
            m.open_task_count,
            format_unit_amount(m.total_units_left as f64),
            m.nearest_deadline_label
                .as_ref()
                .map_or(String::new(), |l| format!(" · nearest deadline {l}")),
        ));
        d.goal_caption = ss(format!(
            "{}% of daily goal · streak {} d",
            m.goal_progress_percent, m.streak_days
        )
        .to_uppercase());
        d.queue_caption = ss(format!(
            "{} open · {} planned today",
            m.queue_open_count,
            m.queue.len()
        )
        .to_uppercase());
        match selected.map(|i| queue[i]) {
            Some(entry) => {
                d.next_code = ss(entry
                    .course_name
                    .as_deref()
                    .map_or("DESK".to_string(), |n| {
                        n.chars().take(4).collect::<String>().to_uppercase()
                    }));
                d.next_color = entry
                    .course_color
                    .as_deref()
                    .map_or(FALLBACK_COURSE_COLOR, parse_css_color);
                d.next_title = ss(&entry.title);
                d.next_meta = ss(entry_meta(entry));
                d.has_start = entry.has_task;
            }
            None => {
                d.next_code = ss("DESK");
                d.next_color = FALLBACK_COURSE_COLOR;
                d.next_title = ss("Choose a study block");
                d.next_meta = ss("Select one planned task from the queue.");
                d.has_start = false;
            }
        }
        d.queue_rows = model(
            queue
                .iter()
                .enumerate()
                .map(|(i, e)| queue_row(e, Some(i) == selected))
                .collect(),
        );
        d.courses = model(
            m.courses
                .iter()
                .map(|c| FnCourseRow {
                    name: ss(&c.name),
                    color: parse_css_color(&c.color),
                    score: ss(c.score.to_string()),
                    fill: c.score as f32 / 100.0,
                    detail: ss(format!(
                        "{} • {} tasks • {} • Target {}{}",
                        c.semester_name.as_deref().unwrap_or("No semester"),
                        c.task_count,
                        format_minutes(c.minutes),
                        c.target_label,
                        if c.overdue > 0 {
                            format!(" • {} overdue", c.overdue)
                        } else {
                            String::new()
                        },
                    )),
                })
                .collect(),
        );
        d.exams = model(
            m.exams
                .iter()
                .map(|e| FnExamRow {
                    days: ss(e.days_until.to_string()),
                    title: ss(&e.title),
                    meta: ss(format!(
                        "{} • {}% weight",
                        e.course_name.as_deref().unwrap_or("No course"),
                        e.weight_label
                    )),
                    prep_text: ss(format!("{}%", js_number(e.preparedness))),
                    prep_fill: (e.preparedness / 100.0).clamp(0.0, 1.0) as f32,
                    prep_tone: if e.preparedness >= 70.0 {
                        0
                    } else if e.preparedness >= 40.0 {
                        1
                    } else {
                        2
                    },
                })
                .collect(),
        );
        d.pace_left = ss(format_unit_amount(m.total_units_left as f64));
        d.pace_open = ss(m.open_task_count.to_string());
        d.pace_rate = ss(to_fixed(m.units_per_day, 1));
        d.pace_note = ss(if m.weekly_total_minutes > 0 {
            format!(
                "{} logged this week.",
                format_minutes(m.weekly_total_minutes)
            )
        } else {
            "No logged focus yet this week.".to_string()
        });
        d.margin_note = ss(match &m.earliest_exam_title {
            Some(title) => {
                format!("{title} is first. Block review time before chasing lower-pressure tasks.")
            }
            None => "Pin one task to today so the dashboard can answer the next-action question."
                .to_string(),
        });
        d.week_caption =
            ss(format!("{} this week", format_minutes(m.weekly_total_minutes)).to_uppercase());
        d.footer = ss(if m.has_sessions {
            format!(
                "{} logged across {} day{} since {}",
                format_minutes(m.lifetime_minutes),
                m.session_day_count,
                if m.session_day_count != 1 { "s" } else { "" },
                m.first_session_label.clone().unwrap_or_default(),
            )
            .to_uppercase()
        } else {
            "Your timeline begins when you complete your first session.".to_uppercase()
        });
        if let Some(timeline) = timeline {
            build_fossil(m, timeline, clock, &mut d);
        }
    }
    d
}

#[cfg(test)]
mod tests;
