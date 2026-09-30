//! A synthetic, deterministic, entirely fabricated "heavy but plausible student profile" (Stage
//! 16, section 25 of the brief) - used only to measure real load/startup/memory behavior of the
//! native store at a realistic-to-heavy scale, never to claim anything about correctness (that is
//! `study-tracker-core::academic`'s and `migration_academic.rs`'s own, much smaller, deterministic
//! unit tests). No personal data of any kind; every name/label below is a generic placeholder.
//!
//! Reached only through `STUDY_NATIVE_GENERATE_SYNTHETIC_ACADEMIC_DATA=1` (see `main.rs`) - never
//! runs unless explicitly requested, matching every other diagnostic hook in this project.
//!
//! Chosen sizes (documented here, not just in the stage doc, so the two can never drift):
//!
//! | Entity | Count | Rationale |
//! |---|---|---|
//! | Semesters | 3 | A student a few years in - one archived, one current, one with only future dates |
//! | Courses | 8 per semester (24 total) | "dozens of courses" per the brief |
//! | Tasks | 15 per course (360 total) | "hundreds of tasks" |
//! | Exams | 3 per course (72 total) | One midterm-shaped, one final-shaped, one resit-shaped |
//! | Study sessions | 2000 | "hundreds/thousands of sessions"; kept under `SESSION_HISTORY_MAX` |
//! | Timetable events | 5 per course (120 total) | A realistic weekly lecture/exercise/sheet load |
//! | Holidays | 10 | Roughly one per semester's typical break count |
//! | Daily todos | 60 | About two months of ordinary day-to-day items |
//! | Calendar entries | 400 | A few scheduled work-units per task across the term |

use study_tracker_core::academic::{
    AcademicState, CalendarEntry, CalendarEntryId, Course, CourseId, DailyTodo, DailyTodoId, Exam,
    ExamId, Holiday, HolidayId, LocalDate, Priority, Semester, SemesterId, SessionId, SessionKind,
    StudySession, Task, TaskId, TaskSubtype, TimetableEvent, TimetableEventId, TimetableEventKind,
    UnitAmount,
};
use study_tracker_core::timer::WallTimestamp;

const SEMESTERS: usize = 3;
const COURSES_PER_SEMESTER: usize = 8;
const TASKS_PER_COURSE: usize = 15;
const EXAMS_PER_COURSE: usize = 3;
const SESSIONS_TOTAL: usize = 2000;
const TIMETABLE_EVENTS_PER_COURSE: usize = 5;
const HOLIDAYS_TOTAL: usize = 10;
const DAILY_TODOS_TOTAL: usize = 60;
const CALENDAR_ENTRIES_TOTAL: usize = 400;

fn date_from_day_offset(day_offset: i64) -> LocalDate {
    // A tiny fixed-epoch calendar helper - no calendar-correctness claim beyond "a plausible,
    // strictly increasing sequence of YYYY-MM-DD strings," which is all a load/startup
    // performance fixture needs (see `LocalDate::parse`'s own doc comment: it does not validate
    // real calendar correctness either).
    let base_year = 2024i64;
    let day_in_year = day_offset.rem_euclid(365);
    let year = base_year + day_offset.div_euclid(365);
    let month = (day_in_year / 30).min(11) + 1;
    let day = (day_in_year % 30) + 1;
    LocalDate::parse(&format!("{year:04}-{month:02}-{day:02}")).unwrap_or_else(|| {
        LocalDate::parse("2024-01-01").expect("a fixed fallback date is always well-formed")
    })
}

/// Builds the whole synthetic dataset in one pass and returns it - never touches disk itself
/// (the caller, `main.rs`, persists it through the ordinary `AcademicController` path, exactly
/// like any other real data would be written).
pub fn build_synthetic_academic_state() -> AcademicState {
    let mut state = AcademicState::new();
    let base_time = WallTimestamp::from_unix_millis(1_700_000_000_000);

    for semester_index in 0..SEMESTERS {
        let semester_id = SemesterId::new(format!("synthetic-semester-{semester_index}"));
        let mut semester = Semester::new(
            semester_id.clone(),
            format!("Synthetic Semester {}", semester_index + 1),
            base_time,
        );
        semester.start_date = Some(date_from_day_offset((semester_index as i64) * 180));
        semester.end_date = Some(date_from_day_offset((semester_index as i64) * 180 + 120));
        semester.archived = semester_index == 0;
        state.add_semester(semester);

        for course_in_semester in 0..COURSES_PER_SEMESTER {
            let course_index = semester_index * COURSES_PER_SEMESTER + course_in_semester;
            let course_id = CourseId::new(format!("synthetic-course-{course_index}"));
            let course = Course::new(
                course_id.clone(),
                semester_id.clone(),
                format!("Synthetic Course {}", course_index + 1),
                ["blue", "green", "amber", "rose"][course_index % 4].to_string(),
                base_time,
            );
            state.add_course(course);

            for task_in_course in 0..TASKS_PER_COURSE {
                let task_index = course_index * TASKS_PER_COURSE + task_in_course;
                let mut task = Task::new(
                    TaskId::new(format!("synthetic-task-{task_index}")),
                    semester_id.clone(),
                    course_id.clone(),
                    format!("Synthetic Task {}", task_index + 1),
                    [
                        TaskSubtype::Lecture,
                        TaskSubtype::Sheet,
                        TaskSubtype::Session,
                        TaskSubtype::Other,
                    ][task_in_course % 4],
                    "Unit".to_string(),
                    base_time,
                );
                task.due_date = Some(date_from_day_offset((task_index as i64) % 300));
                task.priority =
                    [Priority::Low, Priority::Medium, Priority::High][task_in_course % 3];
                state.add_task(task);
            }

            for exam_in_course in 0..EXAMS_PER_COURSE {
                let exam_index = course_index * EXAMS_PER_COURSE + exam_in_course;
                state.add_exam(Exam::new(
                    ExamId::new(format!("synthetic-exam-{exam_index}")),
                    semester_id.clone(),
                    course_id.clone(),
                    format!("Synthetic Exam {}", exam_index + 1),
                    date_from_day_offset((exam_index as i64) * 17 % 300),
                ));
            }

            for event_in_course in 0..TIMETABLE_EVENTS_PER_COURSE {
                let event_index = course_index * TIMETABLE_EVENTS_PER_COURSE + event_in_course;
                state.add_timetable_event(TimetableEvent {
                    id: TimetableEventId::new(format!("synthetic-timetable-{event_index}")),
                    semester_id: semester_id.clone(),
                    course_id: course_id.clone(),
                    kind: TimetableEventKind::Occurrence,
                    task_id: TaskId::new(format!(
                        "synthetic-task-{}",
                        course_index * TASKS_PER_COURSE
                    )),
                    label: format!("Synthetic Lecture {}", event_index + 1),
                    date: date_from_day_offset((event_index as i64) % 300),
                    time: format!("{:02}:00", 8 + (event_in_course % 8)),
                    end_time: Some(format!("{:02}:00", 9 + (event_in_course % 8))),
                    repeat_weekly: true,
                    recurrence_end_date: None,
                    occurrence_overrides: Default::default(),
                    url: None,
                    completed_occurrences: Vec::new(),
                    created_at: base_time,
                });
            }
        }
    }

    for holiday_index in 0..HOLIDAYS_TOTAL {
        state.add_holiday(Holiday {
            id: HolidayId::new(format!("synthetic-holiday-{holiday_index}")),
            semester_id: SemesterId::new(format!(
                "synthetic-semester-{}",
                holiday_index % SEMESTERS
            )),
            start_date: date_from_day_offset((holiday_index as i64) * 30),
            end_date: date_from_day_offset((holiday_index as i64) * 30 + 5),
            label: format!("Synthetic Break {}", holiday_index + 1),
            created_at: base_time,
        });
    }

    for todo_index in 0..DAILY_TODOS_TOTAL {
        state.add_daily_todo(DailyTodo {
            id: DailyTodoId::new(format!("synthetic-todo-{todo_index}")),
            date: date_from_day_offset(todo_index as i64),
            time: None,
            end_time: None,
            title: format!("Synthetic To-do {}", todo_index + 1),
            notes: String::new(),
            completed: todo_index % 3 == 0,
            completed_at: None,
            created_at: base_time,
            repeat_weekly: false,
            completed_occurrences: Vec::new(),
            recurrence_end_date: None,
            skipped_occurrences: Vec::new(),
            occurrence_times: Default::default(),
        });
    }

    for entry_index in 0..CALENDAR_ENTRIES_TOTAL {
        let task_index = entry_index % (SEMESTERS * COURSES_PER_SEMESTER * TASKS_PER_COURSE);
        state.add_calendar_entry(CalendarEntry {
            id: CalendarEntryId::new(format!("synthetic-calendar-{entry_index}")),
            task_id: TaskId::new(format!("synthetic-task-{task_index}")),
            date: date_from_day_offset((entry_index as i64) % 300),
            unit_amount: UnitAmount::Whole,
            unit_start: None,
            completed: entry_index % 4 == 0,
            completed_at: None,
            created_at: base_time,
            start_time: None,
            end_time: None,
            ad_hoc_title: None,
            ad_hoc_semester_id: None,
            ad_hoc_course_id: None,
        });
    }

    let total_courses = SEMESTERS * COURSES_PER_SEMESTER;
    let mut sessions = Vec::with_capacity(SESSIONS_TOTAL);
    for session_index in 0..SESSIONS_TOTAL {
        let day = (SESSIONS_TOTAL - session_index) as i64; // oldest last, newest first-ish
        let started_at = base_time.plus_seconds((day as u64) * 86_400);
        let ended_at = started_at.plus_seconds(25 * 60);
        sessions.push(StudySession {
            id: SessionId::new(format!("synthetic-session-{session_index}")),
            semester_id: Some(SemesterId::new(format!(
                "synthetic-semester-{}",
                session_index % SEMESTERS
            ))),
            course_id: Some(CourseId::new(format!(
                "synthetic-course-{}",
                session_index % total_courses
            ))),
            task_id: None,
            kind: if session_index % 11 == 0 {
                SessionKind::Exam
            } else {
                SessionKind::Study
            },
            goal: String::new(),
            learned: String::new(),
            blocker: String::new(),
            next_step: String::new(),
            confidence: (session_index % 6) as u8,
            started_at,
            ended_at,
            minutes: 25,
            preset_label: "Pomodoro 25/5".to_string(),
        });
    }
    // Insert directly (not via `add_study_sessions`) - this is a one-shot bulk load of already-
    // historical data, not a live completion sequence, so the ordinary dedup/lifetime-accounting
    // path (correctly exercised elsewhere - see `academic::tests` and `migration_academic::tests`)
    // isn't the thing being measured here; this keeps dataset generation itself O(n), not O(n^2).
    state.sessions = sessions;
    state.lifetime_study_minutes = (SESSIONS_TOTAL as u64) * 25;
    state.lifetime_study_sessions = SESSIONS_TOTAL as u64;

    state
}
