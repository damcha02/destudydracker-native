//! Wabi-Sabi presentation layer (Stage 19).
//!
//! ```text
//! AcademicController.state() + DashboardController.metrics()      (Stage 16/17, cached by revision)
//!            │
//!            ▼  study_tracker_core::dashboard::wabi::{wabi_dashboard, wabi_sidebar, wabi_quiet}
//! WabiController (this file): caches the result by (revision, day, selection, quiet pick),
//!            │                keeps the style's small UI state, maps a clicked row index back to
//!            ▼                its typed MarkTarget/StartTarget (Slint never sees ids)
//!   Slint WabiDashData / WabiSidebarData / WabiQuietData / WabiTimerData
//! ```
//!
//! Nothing here is persisted and nothing reads the clock directly: "now" and the local clock are
//! passed in, exactly like the Field Notebook Dashboard.

use std::collections::BTreeSet;

use slint::{Model, ModelRc, SharedString, VecModel};
use study_tracker_core::academic::AcademicState;
use study_tracker_core::dashboard::wabi::{
    wabi_dashboard, wabi_quiet, wabi_sidebar, MarkTarget, StartTarget, WabiDashboard, WabiInput,
    WabiQuiet, WabiSidebar,
};
use study_tracker_core::dashboard::{CivilDate, DashboardMetrics, LocalClock};

use crate::appearance_view::score_arc;
use crate::dashboard_view::parse_css_color;
use crate::{
    WabiCourseItem, WabiDashData, WabiDeadlineRow, WabiModeCard, WabiPlannedRow, WabiQuietData,
    WabiQuietOther, WabiSidebarData, WabiTaskItem, WabiTimerData,
};

/// Wabi-Sabi-only UI state (production: `selectedTaskId`, `wabiQuietMode`, `wabiQuietTaskId`,
/// `wabiTimerMenuOpen`, `wabiTimerSemesterOpen`, `wabiTimerOpenCourseIds`). Session-only, like
/// production (none of these are in localStorage).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WabiUi {
    pub selected_task: Option<String>,
    pub quiet: bool,
    pub quiet_task: Option<String>,
    pub timer_menu_open: bool,
    pub semester_open: bool,
    pub open_courses: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Key {
    revision: u64,
    today: CivilDate,
    selected: Option<String>,
    quiet_task: Option<String>,
    timer_goal: Option<String>,
}

#[derive(Debug, Clone)]
struct View {
    dash: WabiDashboard,
    sidebar: WabiSidebar,
    quiet: WabiQuiet,
}

pub struct WabiController {
    ui: WabiUi,
    key: Option<Key>,
    view: Option<View>,
    recomputes: u64,
    twelve_hour: bool,
}

/// What a click on a Wabi-Sabi row/button asks the application to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WabiAction {
    Toggle(MarkTarget),
    /// START / FOCUS: open the Timer (`focusTaskFromDashboard`). Linking the task to the Timer is
    /// not migrated yet; the target is kept for the log and for that later stage.
    Start(StartTarget),
    Select(String),
}

impl WabiController {
    pub fn new(twelve_hour: bool) -> Self {
        Self {
            ui: WabiUi {
                // Production opens the Timer submenu when you navigate to Timer.
                timer_menu_open: true,
                ..WabiUi::default()
            },
            key: None,
            view: None,
            recomputes: 0,
            twelve_hour,
        }
    }

    pub fn ui(&self) -> &WabiUi {
        &self.ui
    }

    pub fn ui_mut(&mut self) -> &mut WabiUi {
        &mut self.ui
    }

    pub fn recomputes(&self) -> u64 {
        self.recomputes
    }

    /// Recomputes when the academic revision, the day, or the selection inputs changed. Returns
    /// whether it did.
    pub fn sync(
        &mut self,
        state: &AcademicState,
        metrics: &DashboardMetrics,
        revision: u64,
        clock: &dyn LocalClock,
        timer_goal: Option<String>,
    ) -> bool {
        let key = Key {
            revision,
            today: metrics.today,
            selected: self.ui.selected_task.clone(),
            quiet_task: self.ui.quiet_task.clone(),
            timer_goal,
        };
        if self.key.as_ref() == Some(&key) && self.view.is_some() {
            return false;
        }
        let dash = wabi_dashboard(WabiInput {
            state,
            metrics,
            clock,
            selected_task_id: self.ui.selected_task.as_deref(),
            twelve_hour: self.twelve_hour,
        });
        let sidebar = wabi_sidebar(state, metrics);
        let quiet = wabi_quiet(
            state,
            metrics,
            clock,
            self.ui.quiet_task.as_deref(),
            self.ui.selected_task.as_deref(),
            key.timer_goal.as_deref(),
        );
        self.view = Some(View {
            dash,
            sidebar,
            quiet,
        });
        self.key = Some(key);
        self.recomputes += 1;
        true
    }

    /// Forces the next `sync` to recompute (a purely visual sidebar toggle changed).
    pub fn invalidate_ui(&mut self) {
        self.key = None;
    }

    pub fn dash(&self) -> Option<&WabiDashboard> {
        self.view.as_ref().map(|v| &v.dash)
    }

    pub fn one_thing_start(&self) -> Option<WabiAction> {
        self.dash()?.one_thing.start.clone().map(WabiAction::Start)
    }
    pub fn one_thing_mark(&self) -> Option<WabiAction> {
        self.dash()?
            .one_thing
            .mark_done
            .clone()
            .map(WabiAction::Toggle)
    }
    pub fn deadline_mark(&self, index: usize) -> Option<WabiAction> {
        Some(WabiAction::Toggle(
            self.dash()?.deadlines.get(index)?.mark.clone(),
        ))
    }
    pub fn deadline_focus(&self, index: usize) -> Option<WabiAction> {
        let task = self.dash()?.deadlines.get(index)?.task_id.clone()?;
        Some(WabiAction::Start(StartTarget::Task(task)))
    }
    pub fn deadline_select(&self, index: usize) -> Option<WabiAction> {
        let task = self.dash()?.deadlines.get(index)?.task_id.clone()?;
        Some(WabiAction::Select(task))
    }
    pub fn planned_mark(&self, index: usize) -> Option<WabiAction> {
        Some(WabiAction::Toggle(
            self.dash()?.planned.get(index)?.mark.clone(),
        ))
    }
    pub fn planned_select(&self, index: usize) -> Option<WabiAction> {
        let task = self.dash()?.planned.get(index)?.select_task.clone()?;
        Some(WabiAction::Select(task))
    }
    pub fn quiet_pick(&self, index: usize) -> Option<String> {
        self.view
            .as_ref()?
            .quiet
            .others
            .get(index)
            .map(|(id, _, _)| id.clone())
    }

    pub fn dash_data(&self) -> WabiDashData {
        let Some(view) = &self.view else {
            return WabiDashData::default();
        };
        let d = &view.dash;
        WabiDashData {
            today_label: d.today_label.as_str().into(),
            remaining: d.remaining_label.as_str().into(),
            one_title: d.one_thing.title.as_str().into(),
            one_meta: d.one_thing.meta.as_str().into(),
            one_can_start: d.one_thing.start.is_some(),
            one_can_mark: d.one_thing.mark_done.is_some(),
            deadlines: model(
                d.deadlines
                    .iter()
                    .map(|r| WabiDeadlineRow {
                        title: r.title.as_str().into(),
                        next: r.next,
                        status: r.status.as_str().into(),
                        due: r.due.as_str().into(),
                        relative: r.due_relative.as_str().into(),
                        has_task: r.task_id.is_some(),
                    })
                    .collect(),
            ),
            planned: model(
                d.planned
                    .iter()
                    .map(|r| WabiPlannedRow {
                        title: r.title.as_str().into(),
                        subject: r.subject.as_str().into(),
                        amount: r.amount.as_str().into(),
                        due: r.due.as_str().into(),
                        completed: r.completed,
                        selected: r.selected,
                    })
                    .collect(),
            ),
        }
    }

    pub fn sidebar_data(&self) -> WabiSidebarData {
        let Some(view) = &self.view else {
            return WabiSidebarData::default();
        };
        let s = &view.sidebar;
        WabiSidebarData {
            score: s.score as i32,
            label: s.label.into(),
            tone: i32::from(s.tone),
            arc: score_arc(s.score).into(),
            tended: s.tended_today.as_str().into(),
            goal_fraction: s.goal_percent as f32 / 100.0,
            has_semester: s.semester_name.is_some(),
            semester_name: s.semester_name.clone().unwrap_or_default().into(),
            semester_prep: s.semester_in_prep,
            semester_open: self.ui.semester_open,
            courses: model(
                s.courses
                    .iter()
                    .map(|c| WabiCourseItem {
                        name: c.name.as_str().into(),
                        course_color: parse_css_color(&c.color),
                        has_color: !c.color.trim().is_empty(),
                        quiet: c.quiet,
                        days: c.days_to_exam.clone().unwrap_or_default().into(),
                        open: self.ui.open_courses.contains(&c.id),
                        tasks: model(
                            c.tasks
                                .iter()
                                .map(|(_, title)| WabiTaskItem {
                                    title: title.as_str().into(),
                                    // `state.timer.taskId === task.id`: no Timer task linking yet.
                                    selected: false,
                                })
                                .collect(),
                        ),
                    })
                    .collect(),
            ),
        }
    }

    pub fn course_id(&self, index: usize) -> Option<String> {
        self.view
            .as_ref()?
            .sidebar
            .courses
            .get(index)
            .map(|c| c.id.clone())
    }

    pub fn quiet_data(&self) -> WabiQuietData {
        let Some(view) = &self.view else {
            return WabiQuietData::default();
        };
        let q = &view.quiet;
        WabiQuietData {
            title: q.title.as_str().into(),
            meta: q.meta.as_str().into(),
            others: model(
                q.others
                    .iter()
                    .map(|(_, title, course)| WabiQuietOther {
                        title: title.as_str().into(),
                        course: course.as_str().into(),
                    })
                    .collect(),
            ),
        }
    }
}

fn model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(rows))
}

/// The native Timer's presets, shown as production's Wabi-Sabi cards (`focusPresets` + Custom).
/// Production order and labels; Sprint and Custom do not exist in the native Timer yet and are
/// drawn unavailable, and the native "Demo" test preset is not shown in this style.
pub struct TimerCardSpec {
    pub title: &'static str,
    pub detail: &'static str,
    pub mode: Option<usize>,
}

pub const WABI_TIMER_CARDS: [TimerCardSpec; 6] = [
    TimerCardSpec {
        title: "Pomodoro",
        detail: "25 / 5",
        mode: Some(0),
    },
    TimerCardSpec {
        title: "Deep Work",
        detail: "52 / 17",
        mode: Some(1),
    },
    TimerCardSpec {
        title: "Sprint",
        detail: "90 / 20",
        mode: None,
    },
    TimerCardSpec {
        title: "Exam",
        detail: "120 min",
        mode: Some(2),
    },
    TimerCardSpec {
        title: "\u{221e} Endless",
        detail: "counts up",
        mode: Some(4),
    },
    TimerCardSpec {
        title: "Custom",
        detail: "set by hand",
        mode: None,
    },
];

/// The facts about the Timer the Wabi-Sabi Timer and Quiet mode show (from `AppModel`).
#[derive(Debug, Clone, PartialEq)]
pub struct TimerFacts {
    pub clock: String,
    pub idle: bool,
    pub running: bool,
    pub endless: bool,
    /// "WORK · 25 MIN" etc.
    pub phase_label: String,
    pub can_log: bool,
    pub selected_mode: usize,
    pub heading: String,
}

/// Builds the Timer surface data, reusing the cards model when the cards did not change so a
/// 100 ms refresh tick never replaces a model (the Stage 12 "Rule A").
pub fn timer_data(
    facts: &TimerFacts,
    cards: &ModelRc<WabiModeCard>,
) -> (WabiTimerData, Option<ModelRc<WabiModeCard>>) {
    let wanted: Vec<WabiModeCard> = WABI_TIMER_CARDS
        .iter()
        .map(|spec| WabiModeCard {
            title: spec.title.into(),
            detail: spec.detail.into(),
            active: spec.mode == Some(facts.selected_mode),
            available: spec.mode.is_some(),
            mode: spec.mode.map_or(-1, |m| m as i32),
        })
        .collect();
    let same = cards.row_count() == wanted.len()
        && wanted
            .iter()
            .enumerate()
            .all(|(i, card)| cards.row_data(i).as_ref() == Some(card));
    let (cards_model, replaced) = if same {
        (cards.clone(), None)
    } else {
        let m = ModelRc::new(VecModel::from(wanted));
        (m.clone(), Some(m))
    };
    let primary = if facts.idle {
        if facts.endless {
            "START TRACKING"
        } else {
            "START"
        }
    } else if facts.running {
        "PAUSE"
    } else {
        "RESUME"
    };
    (
        WabiTimerData {
            heading: SharedString::from(facts.heading.as_str()),
            subline: SharedString::default(),
            clock: facts.clock.as_str().into(),
            primary: primary.into(),
            can_log: facts.can_log,
            phase_label: facts.phase_label.as_str().into(),
            modes_enabled: facts.idle && !facts.running,
            modes: cards_model,
        },
        replaced,
    )
}

#[cfg(test)]
mod tests;
