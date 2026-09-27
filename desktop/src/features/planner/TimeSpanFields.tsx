import { endTimeFor } from "../../lib/plannerSchedule";
import { TimeField } from "./TimeField";
import { displayTime } from "../../lib/timeInput";

const durationPresets = [15, 30, 45, 60, 75, 90, 105, 120, 150, 180, 240];

function formatDuration(minutes: number) {
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (!hours) return `${rest} min`;
  return rest ? `${hours} h ${rest} min` : `${hours} h`;
}

/** Start time + duration instead of a free-form end time: the block always runs exactly start -> start + duration. */
export function TimeSpanFields({ label, time, duration, onTimeChange, onDurationChange, hideDuration }: {
  label?: string;
  /** Show only the start time (the caller keeps a fixed default duration). */
  hideDuration?: boolean;
  time: string;
  duration: number;
  onTimeChange: (time: string) => void;
  onDurationChange: (minutes: number) => void;
}) {
  const options = durationPresets.includes(duration) ? durationPresets : [...durationPresets, duration].sort((a, b) => a - b);
  const end = endTimeFor(time, duration);
  return (
    <span className="manage-semesters-timespan">
      {label ? <span className="section-note">{label}</span> : null}
      <label className="manage-semesters-timespan-field">
        <span>Start</span>
        <TimeField value={time} onChange={onTimeChange} ariaLabel="Start time" />
      </label>
      {hideDuration ? null : <><label className="manage-semesters-timespan-field">
        <span>Duration</span>
        <select value={duration} onChange={(event) => onDurationChange(Number(event.target.value))}>
          {options.map((minutes) => <option key={minutes} value={minutes}>{formatDuration(minutes)}</option>)}
        </select>
      </label>
      <span className="section-note manage-semesters-timespan-end">{time && end ? `ends ${displayTime(end)}` : time ? "runs past midnight" : ""}</span></>}
    </span>
  );
}
