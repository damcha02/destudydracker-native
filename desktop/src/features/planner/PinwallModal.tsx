import { useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { createPortal } from "react-dom";
import { collectPinwallItems, comparePinwallItems, groupPinwallItems, pinwallTypeLabels } from "../../lib/pinwall";
import type { PinwallGrouping, PinwallItem, PinwallRef, PinwallTimeframe } from "../../lib/pinwall";
import { parseIsoDate, toIsoDate } from "../../lib/plannerSchedule";
import type { AppState } from "../../types";

export type PinwallVariant = "notebook" | "modern" | "wabi";

type Props = {
  state: AppState;
  /** Which app style to dress the window in; the structure and behavior are the same for all three. */
  variant: PinwallVariant;
  /** Flips the item's completion through the planner's own handlers, so unit counts, the calendar and the dashboard stay in step. */
  onComplete: (ref: PinwallRef) => void;
  onClose: () => void;
};

const timeframes: { id: PinwallTimeframe; label: string }[] = [
  { id: "week", label: "This Week" },
  { id: "month", label: "Past Month" },
  { id: "semester", label: "All Semester" },
];

const groupings: { id: PinwallGrouping; label: string }[] = [
  { id: "subject", label: "By subject" },
  { id: "type", label: "By type" },
];

const copy: Record<PinwallVariant, { stamp: string; title: string; clear: string }> = {
  notebook: { stamp: "Pinwall of the past", title: "Still pinned up", clear: "The wall is clear." },
  modern: { stamp: "Pinwall", title: "Unchecked items", clear: "You're all caught up." },
  wabi: { stamp: "残 · Left behind", title: "What was left behind", clear: "Nothing left behind." },
};

/** How long a checked note stays pinned (struck through, fading) before it drops off the wall. */
const LEAVE_MS = 650;

function formatPinDate(iso: string, todayIso: string): { day: string; ago: string } {
  const day = parseIsoDate(iso).toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" });
  const days = Math.round((parseIsoDate(todayIso).getTime() - parseIsoDate(iso).getTime()) / 86400000);
  return { day, ago: days <= 0 ? "today" : days === 1 ? "yesterday" : `${days} days ago` };
}

export function PinwallModal({ state, variant, onComplete, onClose }: Props) {
  const [timeframe, setTimeframe] = useState<PinwallTimeframe>("week");
  const [grouping, setGrouping] = useState<PinwallGrouping>("subject");
  // Items just checked off keep their spot for a moment so the removal reads as an animation
  // rather than the card jumping; the real state has already been updated by then.
  const [leaving, setLeaving] = useState<Map<string, PinwallItem>>(() => new Map());
  const timers = useRef<number[]>([]);
  const [now] = useState(() => new Date());
  const todayIso = toIsoDate(now);

  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  });
  useEffect(() => {
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") onCloseRef.current();
    }
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, []);

  useEffect(() => () => timers.current.forEach((id) => window.clearTimeout(id)), []);

  const pending = useMemo(() => collectPinwallItems(state, timeframe, now), [state, timeframe, now]);
  const activeCourses = useMemo(() => {
    const activeSemesterIds = new Set(state.semesters.filter((semester) => !semester.archived).map((semester) => semester.id));
    return state.courses.filter((course) => activeSemesterIds.has(course.semesterId));
  }, [state.semesters, state.courses]);

  const groups = useMemo(() => {
    const pendingKeys = new Set(pending.map((item) => item.key));
    const shown = [...pending, ...[...leaving.values()].filter((item) => !pendingKeys.has(item.key))].sort(comparePinwallItems);
    return groupPinwallItems(shown, grouping, activeCourses);
  }, [pending, leaving, grouping, activeCourses]);

  const coursesById = useMemo(() => new Map(state.courses.map((course) => [course.id, course])), [state.courses]);

  function complete(item: PinwallItem) {
    if (leaving.has(item.key)) return;
    onComplete(item.ref);
    setLeaving((current) => new Map(current).set(item.key, item));
    const timer = window.setTimeout(() => {
      setLeaving((current) => {
        const next = new Map(current);
        next.delete(item.key);
        return next;
      });
    }, LEAVE_MS);
    timers.current.push(timer);
  }

  const openCount = pending.length;
  const oldest = pending[0];

  return createPortal(
    <div className="fn-task-modal-backdrop" onMouseDown={onClose}>
      <section className={`fn-pinwall pinwall--${variant}`} role="dialog" aria-modal="true" aria-labelledby="fn-pinwall-title" onMouseDown={(event) => event.stopPropagation()}>
        <header className="fn-pinwall-head">
          <div>
            <p className="fn-stamp-line">{copy[variant].stamp} · up to {formatPinDate(todayIso, todayIso).day}</p>
            <h2 id="fn-pinwall-title">{copy[variant].title}</h2>
            <p className="fn-pinwall-summary" aria-live="polite">
              {openCount
                ? `${openCount} unchecked item${openCount === 1 ? "" : "s"}${oldest ? ` · oldest from ${formatPinDate(oldest.date, todayIso).day}` : ""}`
                : copy[variant].clear}
            </p>
          </div>
          <button type="button" className="ghost-button small-button" onClick={onClose}>Close</button>
        </header>

        <div className="fn-pinwall-controls">
          <div className="fn-pinwall-segment" role="radiogroup" aria-label="Timeframe">
            {timeframes.map((option) => (
              <button
                key={option.id}
                type="button"
                role="radio"
                aria-checked={timeframe === option.id}
                className={timeframe === option.id ? "active" : ""}
                onClick={() => setTimeframe(option.id)}
              >
                {option.label}
              </button>
            ))}
          </div>
          <div className="fn-pinwall-segment fn-pinwall-segment--quiet" role="radiogroup" aria-label="Group items">
            {groupings.map((option) => (
              <button
                key={option.id}
                type="button"
                role="radio"
                aria-checked={grouping === option.id}
                className={grouping === option.id ? "active" : ""}
                onClick={() => setGrouping(option.id)}
              >
                {option.label}
              </button>
            ))}
          </div>
        </div>

        {groups.length ? (
          <div className="fn-pinwall-board">
            {groups.map((group, groupIndex) => (
              <article
                key={group.id}
                className="fn-pinwall-card"
                style={{ "--pin-color": group.color ?? "var(--ink-3)", "--pin-tilt": variant === "notebook" ? `${[-0.6, 0.4, -0.2, 0.7][groupIndex % 4]}deg` : "0deg" } as CSSProperties}
              >
                {variant === "notebook" ? <span className="fn-pinwall-pin" aria-hidden="true" /> : null}
                <header className="fn-pinwall-card-head">
                  <strong>{group.label}</strong>
                  <em>{group.items.filter((item) => !leaving.has(item.key)).length}</em>
                </header>
                <ul>
                  {group.items.map((item) => {
                    const course = item.courseId ? coursesById.get(item.courseId) : undefined;
                    const done = leaving.has(item.key);
                    const when = formatPinDate(item.date, todayIso);
                    return (
                      <li key={item.key} className={`fn-pinwall-item ${done ? "is-done" : ""}`}>
                        <label>
                          <input type="checkbox" checked={done} disabled={done} onChange={() => complete(item)} aria-label={`Mark "${item.title}" as done`} />
                          <span className="fn-pinwall-item-body">
                            <span className="fn-pinwall-item-title">
                              <b>{item.title}</b>
                              <span className={`fn-pinwall-badge fn-pinwall-badge--${item.type}`}>{pinwallTypeLabels[item.type]}</span>
                            </span>
                            <span className="fn-pinwall-item-meta">
                              {grouping === "type" ? (
                                <span className="fn-pinwall-subject" style={{ "--pin-color": course?.color ?? "var(--ink-4)" } as CSSProperties}>
                                  {course?.name ?? "Personal"}
                                </span>
                              ) : null}
                              <span>{when.day}{item.time ? ` · ${item.time}${item.endTime ? `–${item.endTime}` : ""}` : ""}</span>
                              <span className="fn-pinwall-ago">{when.ago}</span>
                            </span>
                          </span>
                        </label>
                      </li>
                    );
                  })}
                </ul>
              </article>
            ))}
          </div>
        ) : (
          <div className="fn-pinwall-empty">
            <strong>{copy[variant].clear}</strong>
            <p>No unchecked lectures, sessions, sheets or to-dos {timeframe === "week" ? "this week" : timeframe === "month" ? "in the past month" : "this semester"}.</p>
          </div>
        )}
      </section>
    </div>,
    document.body,
  );
}
