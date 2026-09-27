import { useState } from "react";
import type { Dispatch, SetStateAction } from "react";
import { formatDate } from "../../lib/metrics";
import { durationBetween, endTimeFor, isValidIsoDate, makeTimetableEvent } from "../../lib/plannerSchedule";
import { TimeSpanFields } from "./TimeSpanFields";
import { displayTime } from "../../lib/timeInput";
import { makeId } from "../../lib/storage";
import type { AppState, Course, Task, TimetableEvent } from "../../types";

const kindLabel: Record<TimetableEvent["kind"], string> = {
  occurrence: "Occurrence",
  "sheet-release": "Released",
  "sheet-deadline": "Due",
};

type Props = {
  state: AppState;
  setState: Dispatch<SetStateAction<AppState>>;
  setMessage: (message: string) => void;
  onDeleteWithUndo: (label: string, updater: (current: AppState) => AppState) => void;
  task: Task;
  course: Course;
  /** Whether the "add to timetable" form is expanded (toggled by the Schedule button in the task editor). */
  open: boolean;
  onClose: () => void;
};

/** Schedule form + the task's scheduled occurrences (edit/remove), shown inside the Edit task window. */
export function TaskScheduleEditor({ state, setState, setMessage, onDeleteWithUndo, task, course, open, onClose }: Props) {
  const today = new Date().toISOString().slice(0, 10);
  const [draft, setDraft] = useState({
    date: today, time: "10:00", duration: 90, repeatWeekly: true,
    releaseDate: today, releaseTime: "", releaseDuration: 30, releaseWeekly: true,
    dueDate: today, dueTime: "", dueDuration: 30, dueWeekly: true,
    sheetUrl: "",
  });
  const [editingEventId, setEditingEventId] = useState<string | null>(null);
  const [eventDraft, setEventDraft] = useState({ date: "", time: "", duration: 60, repeatWeekly: true });
  const [removeConfirm, setRemoveConfirm] = useState<string | null>(null);

  const events = state.timetableEvents
    .filter((event) => event.taskId === task.id)
    .sort((a, b) => (a.date + a.time).localeCompare(b.date + b.time));

  function scheduleOccurrence() {
    if (!isValidIsoDate(draft.date) || !draft.time) {
      setMessage("Pick a date and time first.");
      return;
    }
    const end = endTimeFor(draft.time, draft.duration);
    if (!end) {
      setMessage("That duration runs past midnight - shorten it or start earlier.");
      return;
    }
    const event = makeTimetableEvent({
      id: makeId(), semesterId: task.semesterId, courseId: course.id, kind: "occurrence", taskId: task.id, label: task.title,
      date: draft.date, time: draft.time, endTime: end, repeatWeekly: draft.repeatWeekly,
    });
    setState((current) => ({ ...current, timetableEvents: [...current.timetableEvents, event] }));
    onClose();
    setMessage(`${task.title} scheduled.`);
  }

  function scheduleSheet() {
    // Times are optional: a date-only release/due is scheduled for that day, untimed.
    if (!isValidIsoDate(draft.releaseDate) || !isValidIsoDate(draft.dueDate)) {
      setMessage("Pick a release date and a due date first.");
      return;
    }
    // The duration isn't asked for: a fixed 30 min slot, or no end at all when untimed / too close to midnight.
    const releaseEnd = draft.releaseTime ? endTimeFor(draft.releaseTime, draft.releaseDuration) : null;
    const dueEnd = draft.dueTime ? endTimeFor(draft.dueTime, draft.dueDuration) : null;
    const url = draft.sheetUrl.trim() || null;
    const createdAt = new Date().toISOString();
    const releaseEvent = makeTimetableEvent({
      id: makeId(), semesterId: task.semesterId, courseId: course.id, taskId: task.id, kind: "sheet-release", label: task.title,
      date: draft.releaseDate, time: draft.releaseTime, endTime: releaseEnd, repeatWeekly: draft.releaseWeekly, url, createdAt,
    });
    const dueEvent = makeTimetableEvent({
      id: makeId(), semesterId: task.semesterId, courseId: course.id, taskId: task.id, kind: "sheet-deadline", label: task.title,
      date: draft.dueDate, time: draft.dueTime, endTime: dueEnd, repeatWeekly: draft.dueWeekly, url, createdAt,
    });
    setState((current) => ({ ...current, timetableEvents: [...current.timetableEvents, releaseEvent, dueEvent] }));
    onClose();
    setMessage(`${task.title} scheduled - release and due added.`);
  }

  function startEditEvent(event: TimetableEvent) {
    setEditingEventId(event.id);
    setEventDraft({ date: event.date, time: event.time, duration: durationBetween(event.time, event.endTime, event.kind === "occurrence" ? 60 : 30), repeatWeekly: event.repeatWeekly });
  }

  function saveEditEvent() {
    if (!editingEventId || !isValidIsoDate(eventDraft.date)) {
      setMessage("Pick a date first.");
      return;
    }
    // A blank time keeps (or makes) the item untimed.
    const end = eventDraft.time ? endTimeFor(eventDraft.time, eventDraft.duration) : null;
    if (eventDraft.time && !end) {
      setMessage("That duration runs past midnight - shorten it or start earlier.");
      return;
    }
    setState((current) => ({
      ...current,
      timetableEvents: current.timetableEvents.map((event) =>
        event.id === editingEventId ? { ...event, date: eventDraft.date, time: eventDraft.time, endTime: end, repeatWeekly: eventDraft.repeatWeekly } : event,
      ),
    }));
    setEditingEventId(null);
    setMessage("Schedule updated.");
  }

  function removeEvent(eventId: string) {
    const event = state.timetableEvents.find((item) => item.id === eventId);
    onDeleteWithUndo(`"${event?.label ?? "Item"}" removed`, (current) => ({ ...current, timetableEvents: current.timetableEvents.filter((item) => item.id !== eventId) }));
    setRemoveConfirm(null);
    if (editingEventId === eventId) setEditingEventId(null);
  }

  return (
    <div className="task-schedule-editor">
      {open ? (
        task.subtype === "Sheet" ? (
          <div className="manage-semesters-schedule-row manage-semesters-sheet-schedule">
            <div className="manage-semesters-sheet-schedule-section">
              <span className="section-note">Release</span>
              <input type="date" value={draft.releaseDate} onChange={(event) => setDraft((current) => ({ ...current, releaseDate: event.target.value }))} />
              <TimeSpanFields time={draft.releaseTime} duration={draft.releaseDuration} hideDuration onTimeChange={(releaseTime) => setDraft((current) => ({ ...current, releaseTime }))} onDurationChange={(releaseDuration) => setDraft((current) => ({ ...current, releaseDuration }))} />
              <label className="timetable-modal-toggle compact">
                <input type="checkbox" checked={draft.releaseWeekly} onChange={(event) => setDraft((current) => ({ ...current, releaseWeekly: event.target.checked }))} />
                <span>Weekly</span>
              </label>
            </div>
            <div className="manage-semesters-sheet-schedule-section">
              <span className="section-note">Due</span>
              <input type="date" value={draft.dueDate} onChange={(event) => setDraft((current) => ({ ...current, dueDate: event.target.value }))} />
              <TimeSpanFields time={draft.dueTime} duration={draft.dueDuration} hideDuration onTimeChange={(dueTime) => setDraft((current) => ({ ...current, dueTime }))} onDurationChange={(dueDuration) => setDraft((current) => ({ ...current, dueDuration }))} />
              <label className="timetable-modal-toggle compact">
                <input type="checkbox" checked={draft.dueWeekly} onChange={(event) => setDraft((current) => ({ ...current, dueWeekly: event.target.checked }))} />
                <span>Weekly</span>
              </label>
            </div>
            <input value={draft.sheetUrl} onChange={(event) => setDraft((current) => ({ ...current, sheetUrl: event.target.value }))} placeholder="https://... (optional, shared by both)" />
            <button type="button" onClick={scheduleSheet}>Add release + due to timetable</button>
          </div>
        ) : (
          <div className="manage-semesters-schedule-row">
            <input type="date" value={draft.date} onChange={(event) => setDraft((current) => ({ ...current, date: event.target.value }))} />
            <TimeSpanFields time={draft.time} duration={draft.duration} onTimeChange={(time) => setDraft((current) => ({ ...current, time }))} onDurationChange={(duration) => setDraft((current) => ({ ...current, duration }))} />
            <label className="timetable-modal-toggle compact">
              <input type="checkbox" checked={draft.repeatWeekly} onChange={(event) => setDraft((current) => ({ ...current, repeatWeekly: event.target.checked }))} />
              <span>Weekly</span>
            </label>
            <button type="button" onClick={scheduleOccurrence}>Add to timetable</button>
          </div>
        )
      ) : null}

      {events.length ? (
        <div className="manage-semesters-scheduled-list">
          <span className="section-note">Scheduled</span>
          {events.map((event) =>
            editingEventId === event.id ? (
              <div key={event.id} className="manage-semesters-schedule-row manage-semesters-scheduled-edit-row">
                <input type="date" value={eventDraft.date} onChange={(edit) => setEventDraft((current) => ({ ...current, date: edit.target.value }))} />
                <TimeSpanFields time={eventDraft.time} duration={eventDraft.duration} onTimeChange={(time) => setEventDraft((current) => ({ ...current, time }))} onDurationChange={(duration) => setEventDraft((current) => ({ ...current, duration }))} />
                <label className="timetable-modal-toggle compact">
                  <input type="checkbox" checked={eventDraft.repeatWeekly} onChange={(edit) => setEventDraft((current) => ({ ...current, repeatWeekly: edit.target.checked }))} />
                  <span>Weekly</span>
                </label>
                <button type="button" onClick={saveEditEvent}>Save</button>
                <button type="button" className="ghost-button small-button" onClick={() => setEditingEventId(null)}>Cancel</button>
              </div>
            ) : (
              <div key={event.id} className="manage-semesters-scheduled-row">
                <span className="section-note">
                  {kindLabel[event.kind]} · {formatDate(event.date)} · {event.time ? displayTime(event.time) : "any time"}{event.endTime ? `–${displayTime(event.endTime)}` : ""}{event.repeatWeekly ? " · weekly" : ""}
                </span>
                <button type="button" className="ghost-button small-button" onClick={() => startEditEvent(event)}>Edit</button>
                {removeConfirm === event.id ? (
                  <>
                    <button type="button" className="mini-danger" onClick={() => removeEvent(event.id)}>Confirm</button>
                    <button type="button" className="ghost-button small-button" onClick={() => setRemoveConfirm(null)}>Cancel</button>
                  </>
                ) : (
                  <button type="button" className="ghost-button small-button danger" onClick={() => setRemoveConfirm(event.id)}>Remove</button>
                )}
              </div>
            ),
          )}
        </div>
      ) : null}
    </div>
  );
}
