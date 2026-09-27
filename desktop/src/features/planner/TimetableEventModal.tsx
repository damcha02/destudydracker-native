import { useMemo, useState } from "react";
import type { Dispatch, SetStateAction } from "react";
import { getCourseTasks, getSemesterCourses } from "../../lib/metrics";
import { convertTodoRepeat, countCompletedUnitOccurrences, unitDecrementFor } from "../../lib/plannerActions";
import { isValidIsoDate, makeTimetableEvent, parseIsoDate } from "../../lib/plannerSchedule";

/** The ISO date `days` after (or, if negative, before) `iso`. */
function shiftIso(iso: string, days: number) {
  if (!isValidIsoDate(iso)) return "";
  const date = parseIsoDate(iso);
  date.setDate(date.getDate() + days);
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}
import { makeId } from "../../lib/storage";
import { TimeField } from "./TimeField";
import type { AppState, DailyTodo, Task, TaskSubtype, TimetableEvent, TimetableEventKind } from "../../types";

export type TimetableModalPrefill = { date: string; time: string; semesterId: string | null };

export type TimetableModalTarget =
  | { mode: "create"; prefill: TimetableModalPrefill }
  | { mode: "edit"; entityKind: "event"; event: TimetableEvent }
  | { mode: "edit"; entityKind: "todo"; todo: DailyTodo };

export type TimetableModalState = TimetableModalTarget | null;

type Props = {
  state: AppState;
  setState: Dispatch<SetStateAction<AppState>>;
  setMessage: (message: string) => void;
  target: TimetableModalTarget;
  onClose: () => void;
  onOpenManageSemesters: () => void;
  /** Wabi-Sabi look for the daily to-do form. */
  wabi?: boolean;
  /** Offer "To-do" vs "Course task" (picking, or adding, an existing course task) on creation.
   * Independent of `wabi`: the option reuses generic layout classes, so it renders fine under
   * any app style, not just the Wabi-Sabi skin. */
  allowCourseTask?: boolean;
  /** Current app message, shown inside the window in Wabi-Sabi (the global banner sits behind the modal). */
  message?: string;
  onDeleteWithUndo: (label: string, updater: (current: AppState) => AppState) => void;
};

/**
 * Direct creation from the timetable grid is reserved for standalone Daily To-Dos - subject-bound
 * items (Lectures, Exercise Sessions, Exercise Sheets, and other course units) can only be
 * initialized and scheduled through the Manage Semesters portal, which is the single source of
 * truth for the stored TimetableEvent schema (see makeTimetableEvent). This modal still supports
 * editing an existing subject-bound occurrence in place, since that isn't a creation path.
 */
export function TimetableEventModal({ state, setState, setMessage, target, onClose, onOpenManageSemesters, onDeleteWithUndo, wabi = false, allowCourseTask = false, message = "" }: Props) {
  const isEdit = target.mode === "edit";
  const editingEvent = target.mode === "edit" && target.entityKind === "event" ? target.event : null;
  const editingTodo = target.mode === "edit" && target.entityKind === "todo" ? target.todo : null;
  const isSubjectItem = Boolean(editingEvent);

  const activeSemesters = useMemo(() => state.semesters.filter((semester) => !semester.archived), [state.semesters]);
  const initialSemesterId = editingEvent?.semesterId ?? (allowCourseTask && target.mode === "create" ? target.prefill.semesterId : null);
  const [semesterId] = useState(
    () => activeSemesters.find((semester) => semester.id === initialSemesterId)?.id ?? activeSemesters[0]?.id ?? "",
  );
  const semester = activeSemesters.find((item) => item.id === semesterId) ?? null;

  const courses = semester ? getSemesterCourses(state, semester.id) : [];
  const [courseId, setCourseId] = useState(editingEvent?.courseId ?? courses[0]?.id ?? "");
  const courseTasks = courseId ? getCourseTasks(state, courseId) : [];
  const [taskId, setTaskId] = useState(editingEvent?.taskId ?? courseTasks[0]?.id ?? "");
  const task = courseTasks.find((item) => item.id === taskId) ?? null;
  const [occurrenceKind, setOccurrenceKind] = useState<TimetableEventKind>(editingEvent?.kind ?? "occurrence");

  const [label, setLabel] = useState(editingEvent?.label ?? editingTodo?.title ?? "");
  const [date, setDate] = useState(editingEvent?.date ?? editingTodo?.date ?? (target.mode === "create" ? target.prefill.date : ""));
  const [time, setTime] = useState(editingEvent?.time ?? editingTodo?.time ?? (target.mode === "create" ? target.prefill.time : ""));
  const [endTime, setEndTime] = useState(editingEvent?.endTime ?? editingTodo?.endTime ?? "");
  const [url, setUrl] = useState(editingEvent?.url ?? "");
  const [repeatWeekly, setRepeatWeekly] = useState(editingEvent?.repeatWeekly ?? editingTodo?.repeatWeekly ?? false);
  const [notes, setNotes] = useState(editingTodo?.notes ?? "");
  const [recurrenceEnd, setRecurrenceEnd] = useState(editingTodo?.recurrenceEndDate ?? "");
  // To-dos default to unscheduled - the time picker only appears once the user explicitly opts
  // in via "+ Add Time", instead of implying every to-do needs a slot on the calendar.
  const [timeExpanded, setTimeExpanded] = useState(Boolean(editingTodo?.time));

  // A new item can be a plain to-do or a course task placed on the calendar.
  const [createKind, setCreateKind] = useState<"todo" | "course">("todo");
  const courseMode = allowCourseTask && !isEdit && createKind === "course";
  // Course task: pick an existing task, or make a new one right here (it is created together with its calendar entry).
  const NEW_TASK = "__new__";
  const [newTaskTitle, setNewTaskTitle] = useState("");
  const [newTaskSubtype, setNewTaskSubtype] = useState<TaskSubtype>("Lecture");
  const newTaskUnitLabel: Record<TaskSubtype, string> = { Lecture: "Lecture", Session: "Session", Sheet: "Sheet", Other: "Task" };

  const isSheetKind = occurrenceKind === "sheet-release" || occurrenceKind === "sheet-deadline";
  const makingNewTask = courseMode && (courseTasks.length === 0 || taskId === NEW_TASK);
  const courseSubtype: TaskSubtype | null = makingNewTask ? newTaskSubtype : task?.subtype ?? null;
  // A sheet is released on one date and due on another: the release defaults to a week before the due date until edited.
  const [releaseEdit, setReleaseEdit] = useState<string | null>(null);
  const releaseDate = releaseEdit ?? shiftIso(date, -7);

  function handleCourseChange(nextCourseId: string) {
    setCourseId(nextCourseId);
    setTaskId(nextCourseId ? getCourseTasks(state, nextCourseId)[0]?.id ?? "" : "");
  }

  function validateTimes(startValue: string, endValue: string, requireStart: boolean) {
    if (!isValidIsoDate(date)) {
      setMessage("Pick a valid date first.");
      return false;
    }
    if (requireStart && !startValue) {
      setMessage("Enter a valid start time first.");
      return false;
    }
    if (endValue && !startValue) {
      setMessage("Set a start time before an end time.");
      return false;
    }
    if (startValue && endValue && endValue <= startValue) {
      setMessage("End time must be after the start time.");
      return false;
    }
    return true;
  }

  function submit() {
    if (editingTodo) {
      const title = label.trim();
      if (!title) {
        setMessage("Give this task a title first.");
        return;
      }
      const nextTime = timeExpanded ? time : "";
      const nextEndTime = timeExpanded ? endTime : "";
      if (!validateTimes(nextTime, nextEndTime, false)) return;
      if (repeatWeekly && recurrenceEnd && recurrenceEnd < date) {
        setMessage("The repeat end date can't be before the start date.");
        return;
      }
      setState((current) => ({
        ...current,
        dailyTodos: current.dailyTodos.map((todo) => {
          if (todo.id !== editingTodo.id) return todo;
          const converted = convertTodoRepeat(todo, repeatWeekly);
          const weekdayChanged = converted.repeatWeekly && parseIsoDate(date).getDay() !== parseIsoDate(converted.date).getDay();
          const sameWeekday = (iso: string) => parseIsoDate(iso).getDay() === parseIsoDate(date).getDay();
          return {
            ...converted,
            title,
            date,
            time: nextTime || null,
            endTime: nextEndTime || null,
            notes,
            recurrenceEndDate: converted.repeatWeekly ? (recurrenceEnd || null) : null,
            skippedOccurrences: weekdayChanged ? converted.skippedOccurrences.filter(sameWeekday) : converted.skippedOccurrences,
            occurrenceTimes: weekdayChanged
              ? Object.fromEntries(Object.entries(converted.occurrenceTimes).filter(([occurrence]) => sameWeekday(occurrence)))
              : converted.occurrenceTimes,
          };
        }),
      }));
      onClose();
      return;
    }

    if (editingEvent) {
      if (!courseId || !taskId) {
        setMessage("Pick a course and a course task first.");
        return;
      }
      const nextEndTime = endTime;
      if (!validateTimes(time, nextEndTime, true)) return;
      setState((current) => {
        const previous = current.timetableEvents.find((event) => event.id === editingEvent.id);
        if (!previous) return current;
        const wasCounted = previous.kind !== "sheet-release";
        const isCounted = occurrenceKind !== "sheet-release";
        const completedHere = previous.completedOccurrences.length;
        const weekdayChanged = parseIsoDate(date).getDay() !== parseIsoDate(previous.date).getDay();
        const timetableEvents = current.timetableEvents.map((event) =>
          event.id === editingEvent.id
            ? {
                ...event,
                courseId,
                taskId,
                kind: occurrenceKind,
                label: label.trim() || task?.title || "Item",
                date,
                time,
                endTime: nextEndTime || null,
                repeatWeekly,
                url: isSheetKind ? (url.trim() || null) : null,
                occurrenceOverrides: weekdayChanged ? {} : event.occurrenceOverrides,
              }
            : event,
        );
        // Completed occurrences count toward their task's completed units - re-home that count if
        // the item moved to another task or crossed the sheet-release (never counted) boundary.
        let tasks = current.tasks;
        if ((previous.taskId !== taskId || wasCounted !== isCounted) && completedHere > 0) {
          const oldTask = current.tasks.find((item) => item.id === previous.taskId);
          const decrement = wasCounted && oldTask ? unitDecrementFor(countCompletedUnitOccurrences(current.timetableEvents, previous.taskId), oldTask.totalUnits, completedHere) : 0;
          tasks = current.tasks.map((item) => {
            let completedUnits = item.completedUnits;
            if (item.id === previous.taskId) completedUnits -= decrement;
            if (item.id === taskId && isCounted) completedUnits += completedHere;
            return completedUnits === item.completedUnits ? item : { ...item, completedUnits: Math.min(Math.max(completedUnits, 0), item.totalUnits) };
          });
        }
        return { ...current, timetableEvents, tasks };
      });
      onClose();
      return;
    }

    if (courseMode) {
      if (!courseId) {
        setMessage("Pick a course first.");
        return;
      }
      let target: Task | null = task;
      let created: Task | null = null;
      if (makingNewTask) {
        const title = newTaskTitle.trim();
        if (!title) {
          setMessage("Give the new task a name first.");
          return;
        }
        created = {
          id: makeId(), semesterId, courseId, title, subtype: newTaskSubtype, unitLabel: newTaskUnitLabel[newTaskSubtype],
          // Totals are counted from the calendar entries, so a new task starts empty.
          totalUnits: 0, completedUnits: 0, dueDate: null, priority: "medium", notes: "", createdAt: new Date().toISOString(),
        };
        target = created;
      }
      if (!target) {
        setMessage("Pick one of the course's tasks, or make a new one.");
        return;
      }
      const nextTime = timeExpanded ? time : "";
      const nextEndTime = timeExpanded ? endTime : "";
      if (!validateTimes(nextTime, nextEndTime, false)) return;
      // A sheet has a release date and a due date (two entries, both repeating if asked); everything else is one occurrence.
      const isSheet = target.subtype === "Sheet";
      if (isSheet && releaseDate && !isValidIsoDate(releaseDate)) {
        setMessage("Pick a valid release date, or clear it.");
        return;
      }
      if (isSheet && releaseDate && releaseDate > date) {
        setMessage("A sheet can't be released after it is due.");
        return;
      }
      const createdAt = new Date().toISOString();
      const sheetUrl = isSheet ? (url.trim() || null) : null;
      const events = [
        ...(isSheet && releaseDate
          ? [makeTimetableEvent({ id: makeId(), semesterId, courseId, taskId: target.id, kind: "sheet-release", label: target.title, date: releaseDate, time: "", endTime: null, repeatWeekly, url: sheetUrl, createdAt })]
          : []),
        makeTimetableEvent({
          id: makeId(), semesterId, courseId, taskId: target.id,
          kind: isSheet ? "sheet-deadline" : "occurrence",
          label: target.title, date, time: nextTime, endTime: nextEndTime || null, repeatWeekly, url: sheetUrl, createdAt,
        }),
      ];
      setState((current) => ({
        ...current,
        tasks: created ? [created, ...current.tasks] : current.tasks,
        timetableEvents: [...current.timetableEvents, ...events],
      }));
      onClose();
      return;
    }

    // Direct creation otherwise produces a Daily To-Do.
    const title = label.trim();
    if (!title) {
      setMessage("Give this to-do a title first.");
      return;
    }
    const nextTime = timeExpanded ? time : "";
    const nextEndTime = timeExpanded ? endTime : "";
    if (!validateTimes(nextTime, nextEndTime, false)) return;
    if (repeatWeekly && recurrenceEnd && recurrenceEnd < date) {
      setMessage("The repeat end date can't be before the start date.");
      return;
    }
    const todo: DailyTodo = {
      id: makeId(),
      date,
      time: nextTime || null,
      endTime: nextEndTime || null,
      title,
      notes,
      completed: false,
      completedAt: null,
      createdAt: new Date().toISOString(),
      repeatWeekly,
      completedOccurrences: [],
      recurrenceEndDate: repeatWeekly ? (recurrenceEnd || null) : null,
      skippedOccurrences: [],
      occurrenceTimes: {},
    };
    setState((current) => ({ ...current, dailyTodos: [...current.dailyTodos, todo] }));
    onClose();
  }

  function deleteSeries() {
    if (!editingTodo) return;
    onDeleteWithUndo(`"${editingTodo.title}" series removed`, (current) => ({ ...current, dailyTodos: current.dailyTodos.filter((todo) => todo.id !== editingTodo.id) }));
    onClose();
  }

  return (
    <div className="timetable-modal-backdrop" onMouseDown={onClose}>
      <section className={`timetable-modal ${wabi && !isSubjectItem ? "wabi-todo-modal" : ""}`} role="dialog" aria-modal="true" aria-label={isSubjectItem ? "Edit subject task" : "New daily to-do"} onMouseDown={(event) => event.stopPropagation()}>
        <header className="timetable-modal-head">
          <strong>{isSubjectItem ? "Edit subject task" : isEdit ? "Edit to-do" : courseMode ? "New course task" : "New daily to-do"}</strong>
          <button type="button" className="ghost-button" onClick={onClose}>Close</button>
        </header>

        <div className="timetable-modal-form">
          {allowCourseTask && !isEdit ? (
            <div className="wabi-todo-kind" role="radiogroup" aria-label="What to add">
              <button type="button" role="radio" aria-checked={createKind === "todo"} className={createKind === "todo" ? "active" : ""} onClick={() => setCreateKind("todo")}>To-do</button>
              <button type="button" role="radio" aria-checked={createKind === "course"} className={createKind === "course" ? "active" : ""} onClick={() => setCreateKind("course")}>Course task</button>
            </div>
          ) : null}

          {courseMode ? (
            <div className="wabi-todo-course">
              {courses.length ? (
                <>
                  <label className="field">
                    <span>Course</span>
                    <select value={courseId} onChange={(event) => handleCourseChange(event.target.value)}>
                      {courses.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
                    </select>
                  </label>
                  {courseTasks.length ? (
                    <label className="field">
                      <span>Task</span>
                      <select value={taskId} onChange={(event) => setTaskId(event.target.value)}>
                        {courseTasks.map((item) => <option key={item.id} value={item.id}>{item.title} · {item.subtype === "Session" ? "Exercise" : item.subtype}</option>)}
                        <option value={NEW_TASK}>+ New task…</option>
                      </select>
                    </label>
                  ) : (
                    <p className="section-note">This course has no tasks yet - make the first one here.</p>
                  )}
                  {makingNewTask ? (
                    <div className="wabi-todo-newtask">
                      <label className="field">
                        <span>New task</span>
                        <input value={newTaskTitle} onChange={(event) => setNewTaskTitle(event.target.value)} placeholder="e.g. Lecture, Exercise session, Serie" autoFocus />
                      </label>
                      <div className="wabi-todo-kind" role="radiogroup" aria-label="Task type">
                        {([["Lecture", "Lecture"], ["Session", "Exercise"], ["Sheet", "Sheet"], ["Other", "Other"]] as const).map(([value, name]) => (
                          <button key={value} type="button" role="radio" aria-checked={newTaskSubtype === value} className={newTaskSubtype === value ? "active" : ""} onClick={() => setNewTaskSubtype(value)}>{name}</button>
                        ))}
                      </div>
                    </div>
                  ) : null}
                  {courseSubtype === "Sheet" ? <p className="section-note">A sheet has a release date and a due date. Leave the release empty to schedule only the due date.</p> : null}
                </>
              ) : (
                <p className="section-note">No courses yet. Add a semester and courses from Plan first.</p>
              )}
            </div>
          ) : null}

          {isSubjectItem ? (
            <>
              <label className="field">
                <span>Semester</span>
                <select value={semesterId} disabled>
                  {activeSemesters.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
                </select>
              </label>

              <label className="field">
                <span>Course</span>
                <select value={courseId} onChange={(event) => handleCourseChange(event.target.value)}>
                  <option value="">Select a course...</option>
                  {courses.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
                </select>
              </label>

              {courseId ? (
                courseTasks.length ? (
                  <label className="field">
                    <span>Course task</span>
                    <select value={taskId} onChange={(event) => setTaskId(event.target.value)}>
                      {courseTasks.map((item) => <option key={item.id} value={item.id}>{item.title}</option>)}
                    </select>
                    {task ? <span className="timetable-modal-subtype-badge">{task.subtype}</span> : null}
                  </label>
                ) : (
                  <p className="section-note">No course tasks yet — add one first.</p>
                )
              ) : null}

              {taskId ? (
                <label className="field">
                  <span>Occurrence</span>
                  <select value={occurrenceKind} onChange={(event) => setOccurrenceKind(event.target.value as TimetableEventKind)}>
                    <option value="occurrence">Occurrence</option>
                    <option value="sheet-release">Released</option>
                    <option value="sheet-deadline">Due</option>
                  </select>
                </label>
              ) : null}
            </>
          ) : null}

          {courseMode ? null : <label className={`field ${wabi && !isSubjectItem ? "wabi-todo-title" : ""}`}>
            <span>{isSubjectItem ? "Label" : "Title"}</span>
            <input
              value={label}
              onChange={(event) => setLabel(event.target.value)}
              placeholder={isSubjectItem ? (task?.title ?? "Label") : wabi ? "What needs doing?" : "Title"}
              autoFocus={wabi && !isSubjectItem}
            />
          </label>}

          {isSubjectItem ? (
            <div className="timetable-modal-dates">
              <label className="field compact-field">
                <span>Date</span>
                <input type="date" value={date} onChange={(event) => setDate(event.target.value)} />
              </label>
              <label className="field compact-field">
                <span>Time</span>
                <TimeField value={time} onChange={setTime} />
              </label>
              <label className="field compact-field">
                <span>End time</span>
                <TimeField value={endTime} onChange={setEndTime} />
              </label>
            </div>
          ) : wabi ? (
            <div className="wabi-todo-when">
              {courseMode && courseSubtype === "Sheet" ? (
                <label className="field compact-field">
                  <span>Release date</span>
                  <input type="date" value={releaseDate} onChange={(event) => setReleaseEdit(event.target.value)} />
                </label>
              ) : null}
              <label className="field compact-field">
                <span>{courseMode && courseSubtype === "Sheet" ? "Due date" : "Date"}</span>
                <input type="date" value={date} onChange={(event) => setDate(event.target.value)} />
              </label>
              {timeExpanded ? (
                <>
                  <label className="field compact-field">
                    <span>Start</span>
                    <TimeField value={time} onChange={setTime} autoFocus />
                  </label>
                  <label className="field compact-field">
                    <span>End</span>
                    <TimeField value={endTime} onChange={setEndTime} />
                  </label>
                  <button type="button" className="wabi-todo-chip" onClick={() => { setTime(""); setEndTime(""); setTimeExpanded(false); }}>No time</button>
                </>
              ) : (
                <button type="button" className="wabi-todo-chip" onClick={() => setTimeExpanded(true)}>+ Add time</button>
              )}
            </div>
          ) : (
            <>
              {courseMode && courseSubtype === "Sheet" ? (
                <label className="field compact-field">
                  <span>Release date</span>
                  <input type="date" value={releaseDate} onChange={(event) => setReleaseEdit(event.target.value)} />
                </label>
              ) : null}
              <label className="field compact-field">
                <span>{courseMode && courseSubtype === "Sheet" ? "Due date" : "Date"}</span>
                <input type="date" value={date} onChange={(event) => setDate(event.target.value)} />
              </label>

              {timeExpanded ? (
                <div className="timetable-modal-dates">
                  <label className="field compact-field">
                    <span>Start time</span>
                    <TimeField value={time} onChange={setTime} autoFocus />
                  </label>
                  <label className="field compact-field">
                    <span>End time</span>
                    <TimeField value={endTime} onChange={setEndTime} />
                  </label>
                  <button
                    type="button"
                    className="ghost-button small-button timetable-modal-remove-time"
                    onClick={() => { setTime(""); setEndTime(""); setTimeExpanded(false); }}
                  >
                    Remove time
                  </button>
                </div>
              ) : (
                <div className="timetable-modal-unscheduled-notice">
                  <span>This To-Do will be unscheduled.</span>
                  <button type="button" className="ghost-button small-button" onClick={() => setTimeExpanded(true)}>
                    + Add Time
                  </button>
                </div>
              )}
            </>
          )}

          {courseMode && courseSubtype === "Sheet" ? (
            <label className="field">
              <span>Link (optional)</span>
              <input value={url} onChange={(event) => setUrl(event.target.value)} placeholder="https://..." />
            </label>
          ) : null}

          {isSubjectItem && isSheetKind ? (
            <label className="field">
              <span>Link</span>
              <input value={url} onChange={(event) => setUrl(event.target.value)} placeholder="https://..." />
            </label>
          ) : null}

          {!isSubjectItem && !courseMode ? (
            <label className="field">
              <span>Notes</span>
              <textarea value={notes} onChange={(event) => setNotes(event.target.value)} rows={2} />
            </label>
          ) : null}

          {isSubjectItem ? (
            <label className="timetable-modal-toggle">
              <input type="checkbox" checked={repeatWeekly} onChange={(event) => setRepeatWeekly(event.target.checked)} />
              <span>Repeat weekly until the semester ends</span>
            </label>
          ) : (
            <label className="timetable-modal-toggle">
              <input type="checkbox" checked={repeatWeekly} onChange={(event) => setRepeatWeekly(event.target.checked)} />
              <span>Repeat weekly</span>
            </label>
          )}

          {!isSubjectItem && !courseMode && repeatWeekly ? (
            <label className="field compact-field">
              <span>Repeat until (optional)</span>
              <input type="date" value={recurrenceEnd} min={date || undefined} onChange={(event) => setRecurrenceEnd(event.target.value)} />
            </label>
          ) : null}

          {wabi && message ? <p className="wabi-task-message" role="alert">{message}</p> : null}

          <button type="button" onClick={submit} disabled={(isSubjectItem || courseMode) && !taskId && !makingNewTask}>{isEdit ? "Save changes" : courseMode ? "Add to calendar" : "Create"}</button>

          {editingTodo?.repeatWeekly ? (
            <button type="button" className="ghost-button" onClick={deleteSeries}>Delete whole series</button>
          ) : null}

          {/* Once "Course task" is offered right here (allowCourseTask), this link is redundant - it only
              earns its place when the only way to schedule a course item is to leave this modal. */}
          {!isEdit && !wabi && !allowCourseTask ? (
            <button type="button" className="ghost-button timetable-modal-manage-link" onClick={onOpenManageSemesters}>
              Need to schedule a course item? Open Semester Manager
            </button>
          ) : null}
        </div>
      </section>
    </div>
  );
}
