import { useEffect, useMemo, useRef, useState } from "react";
import type { Dispatch, FormEvent, ReactNode, SetStateAction } from "react";
import { createPortal } from "react-dom";
import { formatDate, getCourseTasks, getSemesterCourses } from "../../lib/metrics";
import { durationBetween, endTimeFor, isValidIsoDate, getSemesterWeekNumber, makeTimetableEvent } from "../../lib/plannerSchedule";
import { TimeSpanFields } from "./TimeSpanFields";
import { displayTime } from "../../lib/timeInput";
import { formatSwissGrade, swissGrades } from "../../lib/grades";
import { makeId } from "../../lib/storage";
import type { AppState, Course, Holiday, Semester, Task, TimetableEvent } from "../../types";

const timetableEventKindLabel: Record<TimetableEvent["kind"], string> = {
  occurrence: "Occurrence",
  "sheet-release": "Released",
  "sheet-deadline": "Due",
};

type View = "main" | "wizard" | "archive";

const subtypeBadge: Record<Task["subtype"], string> = { Lecture: "Lecture", Session: "Exercise", Sheet: "Sheet", Other: "Other" };

/** A small "more actions" menu; closes on outside click, Escape, or choosing an item. */
function RowMenu({ label, children }: { label: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return undefined;
    const close = (event: MouseEvent) => { if (!ref.current?.contains(event.target as Node)) setOpen(false); };
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") setOpen(false); };
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", escape);
    return () => { document.removeEventListener("mousedown", close); document.removeEventListener("keydown", escape); };
  }, [open]);
  return (
    <div className="msm-menu" ref={ref}>
      <button type="button" className="msm-icon-button" aria-label={label} aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((current) => !current)}>&#8943;</button>
      {open ? <div className="msm-menu-list" role="menu" onClick={() => setOpen(false)}>{children}</div> : null}
    </div>
  );
}

type Props = {
  state: AppState;
  setState: Dispatch<SetStateAction<AppState>>;
  setMessage: (message: string) => void;
  onClose: () => void;
  onDeleteWithUndo: (label: string, updater: (current: AppState) => AppState) => void;
  onRemoveSemester: (semesterId: string) => void;
  onRemoveCourse: (courseId: string) => void;
  onRemoveTask: (taskId: string) => void;
  onAddTask: (semesterId: string, courseId: string) => void;
  onEditTask: (task: Task) => void;
  initialSemesterId?: string | null;
  initialCourseId?: string | null;
};

export function ManageSemestersModal({
  state, setState, setMessage, onClose, onDeleteWithUndo,
  onRemoveSemester, onRemoveCourse, onRemoveTask, onAddTask, onEditTask,
  initialSemesterId, initialCourseId,
}: Props) {
  const activeSemesters = useMemo(() => state.semesters.filter((semester) => !semester.archived), [state.semesters]);
  const archivedSemesters = useMemo(() => state.semesters.filter((semester) => semester.archived), [state.semesters]);
  const today = useMemo(() => new Date().toISOString().slice(0, 10), []);

  const [view, setView] = useState<View>("main");
  const [semesterId, setSemesterId] = useState(() => initialSemesterId ?? activeSemesters[0]?.id ?? "");
  const semester = activeSemesters.find((item) => item.id === semesterId) ?? null;

  const [renaming, setRenaming] = useState(false);
  const [nameDraft, setNameDraft] = useState("");
  const [startDraft, setStartDraft] = useState("");
  const [endDraft, setEndDraft] = useState("");
  const [removeConfirm, setRemoveConfirm] = useState(false);
  const [archiveConfirm, setArchiveConfirm] = useState(false);

  const [holidayDraft, setHolidayDraft] = useState({ label: "", startDate: "", endDate: "" });

  const [addingCourse, setAddingCourse] = useState(false);
  const [courseDraft, setCourseDraft] = useState({ name: "", color: "#8fb4ff", externalUrl: "", targetGrade: "4" });
  const [editingCourseId, setEditingCourseId] = useState<string | null>(null);
  const [courseEditDraft, setCourseEditDraft] = useState({ name: "", color: "#8fb4ff", externalUrl: "", targetGrade: "4" });
  const [courseRemoveConfirm, setCourseRemoveConfirm] = useState<string | null>(null);
  const [taskRemoveConfirm, setTaskRemoveConfirm] = useState<string | null>(null);
  const [openTaskId, setOpenTaskId] = useState<string | null>(null);
  const [scheduleDraft, setScheduleDraft] = useState({
    date: today, time: "10:00", duration: 90, repeatWeekly: true,
    releaseDate: today, releaseTime: "20:00", releaseDuration: 30, releaseWeekly: true,
    dueDate: today, dueTime: "23:00", dueDuration: 30, dueWeekly: true,
    sheetUrl: "",
  });

  const [editingEventId, setEditingEventId] = useState<string | null>(null);
  const [eventEditDraft, setEventEditDraft] = useState({ date: "", time: "", duration: 60, repeatWeekly: true });
  const [eventRemoveConfirm, setEventRemoveConfirm] = useState<string | null>(null);

  function startEditEvent(event: TimetableEvent) {
    setEditingEventId(event.id);
    setEventEditDraft({ date: event.date, time: event.time, duration: durationBetween(event.time, event.endTime, event.kind === "occurrence" ? 60 : 30), repeatWeekly: event.repeatWeekly });
  }

  function cancelEditEvent() {
    setEditingEventId(null);
  }

  function saveEditEvent() {
    if (!editingEventId || !isValidIsoDate(eventEditDraft.date) || !eventEditDraft.time) {
      setMessage("Pick a date and time first.");
      return;
    }
    const editedEnd = endTimeFor(eventEditDraft.time, eventEditDraft.duration);
    if (!editedEnd) {
      setMessage("That duration runs past midnight - shorten it or start earlier.");
      return;
    }
    setState((current) => ({
      ...current,
      timetableEvents: current.timetableEvents.map((event) =>
        event.id === editingEventId
          ? { ...event, date: eventEditDraft.date, time: eventEditDraft.time, endTime: editedEnd, repeatWeekly: eventEditDraft.repeatWeekly }
          : event,
      ),
    }));
    setEditingEventId(null);
    setMessage("Schedule updated.");
  }

  function removeScheduledEvent(eventId: string) {
    const event = state.timetableEvents.find((item) => item.id === eventId);
    onDeleteWithUndo(`"${event?.label ?? "Item"}" removed`, (current) => ({ ...current, timetableEvents: current.timetableEvents.filter((item) => item.id !== eventId) }));
    setEventRemoveConfirm(null);
    if (editingEventId === eventId) setEditingEventId(null);
  }

  const courses = semester ? getSemesterCourses(state, semester.id) : [];
  const weekNumber = semester ? getSemesterWeekNumber(semester, today) : null;
  const semesterHolidays = semester ? state.holidays.filter((holiday) => holiday.semesterId === semester.id) : [];

  function startEditCourse(course: Course) {
    setEditingCourseId(course.id);
    setCourseEditDraft({ name: course.name, color: course.color, externalUrl: course.externalUrl ?? "", targetGrade: String(course.targetGrade) });
  }

  useEffect(() => {
    if (!initialCourseId) return;
    const course = state.courses.find((item) => item.id === initialCourseId);
    if (!course) return;
    if (course.semesterId !== semesterId) setSemesterId(course.semesterId);
    startEditCourse(course);
    // Only ever act on the initial trigger - subsequent re-renders shouldn't re-open this.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialCourseId]);

  function startRename() {
    if (!semester) return;
    setNameDraft(semester.name);
    setStartDraft(semester.startDate ?? "");
    setEndDraft(semester.endDate ?? "");
    setRenaming(true);
  }

  function saveRename() {
    if (!semester || !nameDraft.trim()) {
      setMessage("Give the semester a name first.");
      return;
    }
    setState((current) => ({
      ...current,
      semesters: current.semesters.map((item) =>
        item.id === semester.id ? { ...item, name: nameDraft.trim(), startDate: startDraft || null, endDate: endDraft || null } : item,
      ),
    }));
    setRenaming(false);
  }

  function setPhase(phase: Semester["phase"]) {
    if (!semester) return;
    setState((current) => ({
      ...current,
      semesters: current.semesters.map((item) => (item.id === semester.id ? { ...item, phase } : item)),
    }));
    setMessage(phase === "exam-prep" ? "Switched to Exam Prep." : "Switched back to Semester phase.");
  }

  function archiveSemester() {
    if (!semester) return;
    setState((current) => ({
      ...current,
      semesters: current.semesters.map((item) =>
        item.id === semester.id ? { ...item, archived: true, archivedAt: new Date().toISOString() } : item,
      ),
    }));
    setArchiveConfirm(false);
    setMessage(`${semester.name} archived.`);
    setSemesterId(activeSemesters.find((item) => item.id !== semester.id)?.id ?? "");
  }

  function unarchiveSemester(id: string) {
    setState((current) => ({
      ...current,
      semesters: current.semesters.map((item) => (item.id === id ? { ...item, archived: false, archivedAt: null } : item)),
    }));
    setMessage("Semester restored to active semesters.");
  }

  function addHoliday() {
    if (!semester || !holidayDraft.label.trim() || !holidayDraft.startDate || !holidayDraft.endDate) {
      setMessage("A holiday needs a label, start date, and end date.");
      return;
    }
    const holiday: Holiday = {
      id: makeId(),
      semesterId: semester.id,
      label: holidayDraft.label.trim(),
      startDate: holidayDraft.startDate,
      endDate: holidayDraft.endDate < holidayDraft.startDate ? holidayDraft.startDate : holidayDraft.endDate,
      createdAt: new Date().toISOString(),
    };
    setState((current) => ({ ...current, holidays: [...current.holidays, holiday] }));
    setHolidayDraft({ label: "", startDate: "", endDate: "" });
  }

  function removeHoliday(holidayId: string) {
    const holiday = state.holidays.find((item) => item.id === holidayId);
    onDeleteWithUndo(`"${holiday?.label ?? "Holiday"}" removed`, (current) => ({ ...current, holidays: current.holidays.filter((item) => item.id !== holidayId) }));
  }

  function addCourse() {
    if (!semester || !courseDraft.name.trim()) {
      setMessage("Give the subject a name first.");
      return;
    }
    const course: Course = {
      id: makeId(),
      semesterId: semester.id,
      name: courseDraft.name.trim(),
      color: courseDraft.color,
      targetGrade: Number(courseDraft.targetGrade) || 4,
      createdAt: new Date().toISOString(),
      externalUrl: courseDraft.externalUrl.trim() || null,
    };
    setState((current) => ({ ...current, courses: [...current.courses, course] }));
    setCourseDraft({ name: "", color: "#8fb4ff", externalUrl: "", targetGrade: "4" });
    setAddingCourse(false);
    setMessage(`${course.name} added.`);
  }

  function saveEditCourse() {
    if (!editingCourseId || !courseEditDraft.name.trim()) {
      setMessage("Give the subject a name first.");
      return;
    }
    setState((current) => ({
      ...current,
      courses: current.courses.map((course) =>
        course.id === editingCourseId
          ? { ...course, name: courseEditDraft.name.trim(), color: courseEditDraft.color, targetGrade: Number(courseEditDraft.targetGrade) || course.targetGrade, externalUrl: courseEditDraft.externalUrl.trim() || null }
          : course,
      ),
    }));
    setEditingCourseId(null);
  }

  /** Used for every non-Sheet subtype (Lecture/Session/Other); Sheet units go through scheduleSheet below instead. */
  function scheduleTask(course: Course, task: Task) {
    if (!semester || !isValidIsoDate(scheduleDraft.date) || !scheduleDraft.time) {
      setMessage("Pick a date and time first.");
      return;
    }
    const end = endTimeFor(scheduleDraft.time, scheduleDraft.duration);
    if (!end) {
      setMessage("That duration runs past midnight - shorten it or start earlier.");
      return;
    }
    const event = makeTimetableEvent({
      id: makeId(),
      semesterId: semester.id,
      courseId: course.id,
      kind: "occurrence",
      taskId: task.id,
      label: task.title,
      date: scheduleDraft.date,
      time: scheduleDraft.time,
      endTime: end,
      repeatWeekly: scheduleDraft.repeatWeekly,
    });
    setState((current) => ({ ...current, timetableEvents: [...current.timetableEvents, event] }));
    setMessage(`${task.title} scheduled.`);
  }

  /**
   * Sheet-subtype tasks get a dedicated release + due scheduler instead of the generic
   * kind-dropdown flow: one submit creates both timetable events at once. Both share the task's
   * own title/taskId (so they always inherit the same course color via courseId at render time)
   * and the same URL, and each is independently checkable off via the normal per-occurrence
   * completedOccurrences logic - no special-casing needed there since both are ordinary events.
   */
  function scheduleSheet(course: Course, task: Task) {
    if (!semester || !isValidIsoDate(scheduleDraft.releaseDate) || !scheduleDraft.releaseTime || !isValidIsoDate(scheduleDraft.dueDate) || !scheduleDraft.dueTime) {
      setMessage("Pick a release and due date/time first.");
      return;
    }
    const releaseEnd = endTimeFor(scheduleDraft.releaseTime, scheduleDraft.releaseDuration);
    const dueEnd = endTimeFor(scheduleDraft.dueTime, scheduleDraft.dueDuration);
    if (!releaseEnd || !dueEnd) {
      setMessage("That duration runs past midnight - shorten it or start earlier.");
      return;
    }
    const url = scheduleDraft.sheetUrl.trim() || null;
    const createdAt = new Date().toISOString();
    const releaseEvent = makeTimetableEvent({
      id: makeId(), semesterId: semester.id, courseId: course.id, taskId: task.id,
      kind: "sheet-release", label: task.title,
      date: scheduleDraft.releaseDate, time: scheduleDraft.releaseTime, endTime: releaseEnd,
      repeatWeekly: scheduleDraft.releaseWeekly, url, createdAt,
    });
    const dueEvent = makeTimetableEvent({
      id: makeId(), semesterId: semester.id, courseId: course.id, taskId: task.id,
      kind: "sheet-deadline", label: task.title,
      date: scheduleDraft.dueDate, time: scheduleDraft.dueTime, endTime: dueEnd,
      repeatWeekly: scheduleDraft.dueWeekly, url, createdAt,
    });
    setState((current) => ({ ...current, timetableEvents: [...current.timetableEvents, releaseEvent, dueEvent] }));
    setMessage(`${task.title} scheduled - release and due added.`);
  }

  function submitWizard(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const formData = new FormData(form);
    const name = String(formData.get("name") ?? "").trim();
    const startDate = String(formData.get("startDate") ?? "") || null;
    const endDate = String(formData.get("endDate") ?? "") || null;
    if (!name) {
      setMessage("Give the semester a name first.");
      return;
    }
    const courseNames = formData.getAll("courseName").map((value) => String(value).trim());
    const courseUrls = formData.getAll("courseUrl").map((value) => String(value).trim());

    const newSemester: Semester = {
      id: makeId(),
      name,
      createdAt: new Date().toISOString(),
      startDate,
      endDate,
      phase: "semester",
      archived: false,
      archivedAt: null,
    };
    const newCourses: Course[] = courseNames
      .map((courseName, index) => ({ courseName, url: courseUrls[index] ?? "" }))
      .filter((entry) => entry.courseName)
      .map((entry) => ({
        id: makeId(),
        semesterId: newSemester.id,
        name: entry.courseName,
        color: "#8fb4ff",
        targetGrade: 4,
        createdAt: new Date().toISOString(),
        externalUrl: entry.url || null,
      }));

    setState((current) => ({
      ...current,
      semesters: [...current.semesters, newSemester],
      courses: [...current.courses, ...newCourses],
    }));
    setSemesterId(newSemester.id);
    setView("main");
    setMessage(`${newSemester.name} set up with ${newCourses.length} subject${newCourses.length === 1 ? "" : "s"}. Add course tasks below to start scheduling.`);
  }

  return createPortal(
    <div className="timetable-modal-backdrop manage-semesters-backdrop" onMouseDown={onClose}>
      <section className="timetable-modal manage-semesters-modal" role="dialog" aria-modal="true" aria-label="Manage semesters" onMouseDown={(event) => event.stopPropagation()}>
        <header className="timetable-modal-head">
          <strong>Manage Semesters</strong>
          <button type="button" className="ghost-button" onClick={onClose}>Close</button>
        </header>

        {view === "wizard" ? (
          <form onSubmit={submitWizard} className="timetable-modal-form">
            <button type="button" className="ghost-button" onClick={() => setView("main")}>&larr; Back</button>
            <label className="field">
              <span>Name</span>
              <input name="name" placeholder="Winter Semester 2026/2027" required />
            </label>
            <div className="timetable-modal-dates">
              <label className="field compact-field">
                <span>Start date</span>
                <input name="startDate" type="date" />
              </label>
              <label className="field compact-field">
                <span>End date</span>
                <input name="endDate" type="date" />
              </label>
            </div>
            <p className="section-note">Add subjects (optional link to the course page or LMS).</p>
            {[0, 1, 2, 3].map((index) => (
              <div key={index} className="timetable-modal-course-row">
                <input name="courseName" placeholder={`Subject ${index + 1} name`} />
                <input name="courseUrl" placeholder="https://... (optional)" />
              </div>
            ))}
            <button type="submit">Create</button>
          </form>
        ) : view === "archive" ? (
          <div className="timetable-modal-form">
            <button type="button" className="ghost-button" onClick={() => setView("main")}>&larr; Back</button>
            <div className="stack-list compact">
              {archivedSemesters.length ? archivedSemesters.map((item) => {
                const archivedCourses = state.courses.filter((course) => course.semesterId === item.id);
                const archivedTasks = state.tasks.filter((task) => task.semesterId === item.id);
                return (
                  <div key={item.id} className="overview-row detailed-overview">
                    <div>
                      <strong>{item.name}</strong>
                      <p className="section-note">
                        {item.startDate ? formatDate(item.startDate) : "No start date"} – {item.endDate ? formatDate(item.endDate) : "No end date"} · archived {item.archivedAt ? formatDate(item.archivedAt.slice(0, 10)) : ""}
                      </p>
                      <p className="section-note">{archivedCourses.length} subjects · {archivedTasks.length} tasks</p>
                    </div>
                    <button type="button" className="ghost-button" onClick={() => unarchiveSemester(item.id)}>Restore</button>
                  </div>
                );
              }) : <p className="empty-copy">Nothing archived yet.</p>}
            </div>
          </div>
        ) : (
          <div className="msm">
            <div className="msm-bar">
              <select className="msm-select" aria-label="Semester" value={semesterId} onChange={(event) => setSemesterId(event.target.value)}>
                {activeSemesters.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
              </select>
              {semester ? (
                <span className="msm-status">
                  <span className="msm-pill">{semester.phase === "exam-prep" ? "Exam prep" : "Active"}</span>
                  <span className="msm-muted">{weekNumber ? `Week ${weekNumber}` : semester.startDate ? "Not started" : "No dates"}</span>
                </span>
              ) : null}
              <span className="msm-bar-actions">
                <button type="button" className="ghost-button small-button" onClick={() => setView("wizard")}>+ New semester</button>
                <button type="button" className="ghost-button small-button" onClick={() => setView("archive")}>Archived ({archivedSemesters.length})</button>
                {semester ? (
                  <RowMenu label="Semester actions">
                    <button type="button" role="menuitem" onClick={startRename}>Rename &amp; dates</button>
                    {semester.phase === "semester" ? (
                      <button type="button" role="menuitem" onClick={() => setPhase("exam-prep")}>Start exam prep</button>
                    ) : (
                      <button type="button" role="menuitem" onClick={() => setPhase("semester")}>Resume semester</button>
                    )}
                    <button type="button" role="menuitem" onClick={() => setArchiveConfirm(true)}>Archive</button>
                    <button type="button" role="menuitem" className="danger" onClick={() => setRemoveConfirm(true)}>Delete semester</button>
                  </RowMenu>
                ) : null}
              </span>
            </div>

            {semester && renaming ? (
              <div className="msm-card msm-edit">
                <label className="field"><span>Name</span><input value={nameDraft} onChange={(event) => setNameDraft(event.target.value)} placeholder="Name" /></label>
                <label className="field"><span>Start</span><input type="date" value={startDraft} onChange={(event) => setStartDraft(event.target.value)} /></label>
                <label className="field"><span>End</span><input type="date" value={endDraft} onChange={(event) => setEndDraft(event.target.value)} /></label>
                <span className="msm-edit-actions">
                  <button type="button" onClick={saveRename}>Save</button>
                  <button type="button" className="ghost-button" onClick={() => setRenaming(false)}>Cancel</button>
                </span>
              </div>
            ) : null}

            {semester && archiveConfirm ? (
              <div className="msm-confirm">
                <span>Archive "{semester.name}"? It leaves your workload totals and can be restored later.</span>
                <button type="button" className="mini-danger" onClick={archiveSemester}>Archive</button>
                <button type="button" className="ghost-button small-button" onClick={() => setArchiveConfirm(false)}>Cancel</button>
              </div>
            ) : null}
            {semester && removeConfirm ? (
              <div className="msm-confirm">
                <span>Delete "{semester.name}" and everything in it? You can undo right after.</span>
                <button type="button" className="mini-danger" onClick={() => onRemoveSemester(semester.id)}>Delete</button>
                <button type="button" className="ghost-button small-button" onClick={() => setRemoveConfirm(false)}>Cancel</button>
              </div>
            ) : null}

            {semester ? (
              <>
                <details className="msm-card msm-holidays">
                  <summary>Holidays &amp; breaks <span className="msm-count">{semesterHolidays.length}</span></summary>
                  <div className="msm-holiday-form">
                    <input value={holidayDraft.label} onChange={(event) => setHolidayDraft((current) => ({ ...current, label: event.target.value }))} placeholder="Winter break" aria-label="Holiday name" />
                    <input type="date" value={holidayDraft.startDate} onChange={(event) => setHolidayDraft((current) => ({ ...current, startDate: event.target.value }))} aria-label="Holiday start" />
                    <input type="date" value={holidayDraft.endDate} onChange={(event) => setHolidayDraft((current) => ({ ...current, endDate: event.target.value }))} aria-label="Holiday end" />
                    <button type="button" onClick={addHoliday}>Add</button>
                  </div>
                  {semesterHolidays.map((holiday) => (
                    <div key={holiday.id} className="msm-line">
                      <span>{holiday.label} <span className="msm-muted">{formatDate(holiday.startDate)} – {formatDate(holiday.endDate)}</span></span>
                      <button type="button" className="ghost-button small-button danger" onClick={() => removeHoliday(holiday.id)}>Remove</button>
                    </div>
                  ))}
                </details>

                <div className="msm-section-head">
                  <h3>Subjects</h3>
                  <button type="button" className="ghost-button small-button" onClick={() => setAddingCourse((current) => !current)}>{addingCourse ? "Cancel" : "+ Add subject"}</button>
                </div>

                {addingCourse ? (
                  <div className="msm-card msm-edit">
                    <label className="field"><span>Name</span><input value={courseDraft.name} onChange={(event) => setCourseDraft((current) => ({ ...current, name: event.target.value }))} placeholder="Subject name" autoFocus /></label>
                    <label className="field"><span>Target grade</span>
                      <select value={courseDraft.targetGrade} onChange={(event) => setCourseDraft((current) => ({ ...current, targetGrade: event.target.value }))}>
                        {swissGrades.map((grade) => <option key={grade} value={String(grade)}>{formatSwissGrade(grade)}</option>)}
                      </select>
                    </label>
                    <label className="field"><span>Colour</span><input type="color" value={courseDraft.color} onChange={(event) => setCourseDraft((current) => ({ ...current, color: event.target.value }))} /></label>
                    <label className="field msm-wide"><span>Link (optional)</span><input value={courseDraft.externalUrl} onChange={(event) => setCourseDraft((current) => ({ ...current, externalUrl: event.target.value }))} placeholder="https://..." /></label>
                    <span className="msm-edit-actions"><button type="button" onClick={addCourse}>Add subject</button></span>
                  </div>
                ) : null}

                {courses.map((course) => {
                  const tasks = getCourseTasks(state, course.id);
                  return (
                    <section key={course.id} className="msm-card msm-subject" style={{ "--subject-color": course.color } as React.CSSProperties}>
                      {editingCourseId === course.id ? (
                        <div className="msm-edit">
                          <label className="field"><span>Name</span><input value={courseEditDraft.name} onChange={(event) => setCourseEditDraft((current) => ({ ...current, name: event.target.value }))} /></label>
                          <label className="field"><span>Target grade</span>
                            <select value={courseEditDraft.targetGrade} onChange={(event) => setCourseEditDraft((current) => ({ ...current, targetGrade: event.target.value }))}>
                              {swissGrades.map((grade) => <option key={grade} value={String(grade)}>{formatSwissGrade(grade)}</option>)}
                            </select>
                          </label>
                          <label className="field"><span>Colour</span><input type="color" value={courseEditDraft.color} onChange={(event) => setCourseEditDraft((current) => ({ ...current, color: event.target.value }))} /></label>
                          <label className="field msm-wide"><span>Link (optional)</span><input value={courseEditDraft.externalUrl} onChange={(event) => setCourseEditDraft((current) => ({ ...current, externalUrl: event.target.value }))} placeholder="https://..." /></label>
                          <span className="msm-edit-actions">
                            <button type="button" onClick={saveEditCourse}>Save</button>
                            <button type="button" className="ghost-button" onClick={() => setEditingCourseId(null)}>Cancel</button>
                          </span>
                        </div>
                      ) : (
                        <header className="msm-subject-head">
                          <span className="msm-dot" />
                          <strong className="msm-subject-name">{course.name}</strong>
                          <span className="msm-chip" title="Target grade">Target {formatSwissGrade(course.targetGrade)}</span>
                          <span className="msm-spacer" />
                          <button type="button" className="ghost-button small-button" onClick={() => startEditCourse(course)}>Edit</button>
                          <RowMenu label={`${course.name} actions`}>
                            <button type="button" role="menuitem" className="danger" onClick={() => setCourseRemoveConfirm(course.id)}>Remove subject</button>
                          </RowMenu>
                        </header>
                      )}

                      {courseRemoveConfirm === course.id ? (
                        <div className="msm-confirm">
                          <span>Remove "{course.name}" and its tasks?</span>
                          <button type="button" className="mini-danger" onClick={() => { onRemoveCourse(course.id); setCourseRemoveConfirm(null); }}>Remove</button>
                          <button type="button" className="ghost-button small-button" onClick={() => setCourseRemoveConfirm(null)}>Cancel</button>
                        </div>
                      ) : null}

                      <div className="msm-tasks">
                        {tasks.map((task) => {
                          const taskEvents = state.timetableEvents.filter((event) => event.taskId === task.id).sort((a, b) => (a.date + a.time).localeCompare(b.date + b.time));
                          const open = openTaskId === task.id;
                          const percent = task.totalUnits > 0 ? Math.min(100, Math.round((task.completedUnits / task.totalUnits) * 100)) : 0;
                          return (
                            <div key={task.id} className={`msm-task${open ? " open" : ""}`}>
                              <div className="msm-task-row">
                                <span className="msm-task-title">{task.title}</span>
                                <span className="msm-badge">{subtypeBadge[task.subtype]}</span>
                                <span className="msm-progress" title={task.totalUnits > 0 ? `${task.completedUnits} of ${task.totalUnits} ${task.unitLabel}` : "Nothing scheduled yet"}>
                                  <span className="msm-progress-track"><span style={{ width: `${percent}%` }} /></span>
                                  <span className="msm-muted">{task.totalUnits > 0 ? `${task.completedUnits}/${task.totalUnits}` : "not scheduled"}</span>
                                </span>
                                <button type="button" className="ghost-button small-button" aria-expanded={open} onClick={() => { setOpenTaskId(open ? null : task.id); setEditingEventId(null); }}>
                                  {open ? "Hide" : taskEvents.length ? `Schedule · ${taskEvents.length}` : "Schedule"}
                                </button>
                                <RowMenu label={`${task.title} actions`}>
                                  <button type="button" role="menuitem" onClick={() => onEditTask(task)}>Edit task</button>
                                  <button type="button" role="menuitem" className="danger" onClick={() => setTaskRemoveConfirm(task.id)}>Delete task</button>
                                </RowMenu>
                              </div>

                              {taskRemoveConfirm === task.id ? (
                                <div className="msm-confirm">
                                  <span>Delete "{task.title}"?</span>
                                  <button type="button" className="mini-danger" onClick={() => { onRemoveTask(task.id); setTaskRemoveConfirm(null); }}>Delete</button>
                                  <button type="button" className="ghost-button small-button" onClick={() => setTaskRemoveConfirm(null)}>Cancel</button>
                                </div>
                              ) : null}

                              {open ? (
                                <div className="msm-task-panel">
                                  {taskEvents.map((event) => (
                                    editingEventId === event.id ? (
                                      <div key={event.id} className="msm-slot-form">
                                        <input type="date" value={eventEditDraft.date} onChange={(edit) => setEventEditDraft((current) => ({ ...current, date: edit.target.value }))} aria-label="Date" />
                                        <TimeSpanFields time={eventEditDraft.time} duration={eventEditDraft.duration} onTimeChange={(time) => setEventEditDraft((current) => ({ ...current, time }))} onDurationChange={(duration) => setEventEditDraft((current) => ({ ...current, duration }))} />
                                        <label className="timetable-modal-toggle compact">
                                          <input type="checkbox" checked={eventEditDraft.repeatWeekly} onChange={(edit) => setEventEditDraft((current) => ({ ...current, repeatWeekly: edit.target.checked }))} />
                                          <span>Weekly</span>
                                        </label>
                                        <span className="msm-edit-actions">
                                          <button type="button" onClick={saveEditEvent}>Save</button>
                                          <button type="button" className="ghost-button small-button" onClick={cancelEditEvent}>Cancel</button>
                                        </span>
                                      </div>
                                    ) : (
                                      <div key={event.id} className="msm-slot">
                                        <span className="msm-slot-kind">{timetableEventKindLabel[event.kind]}</span>
                                        <span className="msm-slot-when">
                                          {formatDate(event.date)} · {displayTime(event.time)}{event.endTime ? `–${displayTime(event.endTime)}` : ""}{event.repeatWeekly ? " · weekly" : ""}
                                        </span>
                                        <span className="msm-spacer" />
                                        {eventRemoveConfirm === event.id ? (
                                          <>
                                            <button type="button" className="mini-danger" onClick={() => removeScheduledEvent(event.id)}>Remove</button>
                                            <button type="button" className="ghost-button small-button" onClick={() => setEventRemoveConfirm(null)}>Cancel</button>
                                          </>
                                        ) : (
                                          <>
                                            <button type="button" className="ghost-button small-button" onClick={() => startEditEvent(event)}>Edit</button>
                                            <button type="button" className="ghost-button small-button danger" onClick={() => setEventRemoveConfirm(event.id)}>Remove</button>
                                          </>
                                        )}
                                      </div>
                                    )
                                  ))}

                                  {task.subtype === "Sheet" ? (
                                    <div className="msm-slot-form msm-sheet-form">
                                      <div className="msm-sheet-part">
                                        <span className="msm-label">Release</span>
                                        <input type="date" value={scheduleDraft.releaseDate} onChange={(event) => setScheduleDraft((current) => ({ ...current, releaseDate: event.target.value }))} aria-label="Release date" />
                                        <TimeSpanFields time={scheduleDraft.releaseTime} duration={scheduleDraft.releaseDuration} onTimeChange={(releaseTime) => setScheduleDraft((current) => ({ ...current, releaseTime }))} onDurationChange={(releaseDuration) => setScheduleDraft((current) => ({ ...current, releaseDuration }))} />
                                        <label className="timetable-modal-toggle compact">
                                          <input type="checkbox" checked={scheduleDraft.releaseWeekly} onChange={(event) => setScheduleDraft((current) => ({ ...current, releaseWeekly: event.target.checked }))} />
                                          <span>Weekly</span>
                                        </label>
                                      </div>
                                      <div className="msm-sheet-part">
                                        <span className="msm-label">Due</span>
                                        <input type="date" value={scheduleDraft.dueDate} onChange={(event) => setScheduleDraft((current) => ({ ...current, dueDate: event.target.value }))} aria-label="Due date" />
                                        <TimeSpanFields time={scheduleDraft.dueTime} duration={scheduleDraft.dueDuration} onTimeChange={(dueTime) => setScheduleDraft((current) => ({ ...current, dueTime }))} onDurationChange={(dueDuration) => setScheduleDraft((current) => ({ ...current, dueDuration }))} />
                                        <label className="timetable-modal-toggle compact">
                                          <input type="checkbox" checked={scheduleDraft.dueWeekly} onChange={(event) => setScheduleDraft((current) => ({ ...current, dueWeekly: event.target.checked }))} />
                                          <span>Weekly</span>
                                        </label>
                                      </div>
                                      <input className="msm-wide" value={scheduleDraft.sheetUrl} onChange={(event) => setScheduleDraft((current) => ({ ...current, sheetUrl: event.target.value }))} placeholder="Sheet link (optional, shared by both)" />
                                      <button type="button" onClick={() => scheduleSheet(course, task)}>Add release + due</button>
                                    </div>
                                  ) : (
                                    <div className="msm-slot-form">
                                      <input type="date" value={scheduleDraft.date} onChange={(event) => setScheduleDraft((current) => ({ ...current, date: event.target.value }))} aria-label="Date" />
                                      <TimeSpanFields time={scheduleDraft.time} duration={scheduleDraft.duration} onTimeChange={(time) => setScheduleDraft((current) => ({ ...current, time }))} onDurationChange={(duration) => setScheduleDraft((current) => ({ ...current, duration }))} />
                                      <label className="timetable-modal-toggle compact">
                                        <input type="checkbox" checked={scheduleDraft.repeatWeekly} onChange={(event) => setScheduleDraft((current) => ({ ...current, repeatWeekly: event.target.checked }))} />
                                        <span>Weekly</span>
                                      </label>
                                      <button type="button" onClick={() => scheduleTask(course, task)}>Add to timetable</button>
                                    </div>
                                  )}
                                </div>
                              ) : null}
                            </div>
                          );
                        })}
                        <button type="button" className="msm-add-task" onClick={() => onAddTask(course.semesterId, course.id)}>+ Add task</button>
                      </div>
                    </section>
                  );
                })}
                {!courses.length ? <p className="empty-copy">No subjects yet. Add one above.</p> : null}
              </>
            ) : (
              <p className="empty-copy">No active semesters yet. Start one above.</p>
            )}
          </div>
        )}
      </section>
    </div>,
    document.body,
  );
}
