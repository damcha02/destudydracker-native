import { useEffect, useRef, useState } from "react";
import { formatTimeForDisplay, parseTimeInput, prefersTwelveHourClock, shiftTime } from "../../lib/timeInput";

type Props = {
  value: string;
  onChange: (time: string) => void;
  autoFocus?: boolean;
  ariaLabel?: string;
};

/**
 * A plain text time field instead of <input type="time">: the native control differs per OS and
 * locale (am/pm segments, partial values that read as invalid, no picker at all in some Linux
 * webviews) and its controlled value snaps back while typing. This one accepts 24h or am/pm
 * typing, only ever hands the parent a clean "HH:MM" (or "" when empty/invalid), and never
 * rewrites what's being typed until the field loses focus.
 */
export function TimeField({ value, onChange, autoFocus, ariaLabel }: Props) {
  const [twelveHour] = useState(prefersTwelveHourClock);
  const [draft, setDraft] = useState(() => formatTimeForDisplay(value, twelveHour));
  const [invalid, setInvalid] = useState(false);
  const focused = useRef(false);

  useEffect(() => {
    if (focused.current) return;
    setDraft(formatTimeForDisplay(value, twelveHour));
    setInvalid(false);
  }, [value, twelveHour]);

  function handleChange(text: string) {
    setDraft(text);
    if (!text.trim()) {
      setInvalid(false);
      onChange("");
      return;
    }
    const parsed = parseTimeInput(text);
    setInvalid(false);
    onChange(parsed ?? "");
  }

  function handleBlur() {
    focused.current = false;
    if (!draft.trim()) {
      setInvalid(false);
      return;
    }
    const parsed = parseTimeInput(draft);
    if (parsed) {
      setDraft(formatTimeForDisplay(parsed, twelveHour));
      setInvalid(false);
      onChange(parsed);
    } else {
      setInvalid(true);
      onChange("");
    }
  }

  return (
    <input
      type="text"
      className={invalid ? "time-field invalid" : "time-field"}
      value={draft}
      inputMode="text"
      autoComplete="off"
      autoFocus={autoFocus}
      aria-label={ariaLabel ?? "Time"}
      aria-invalid={invalid || undefined}
      placeholder={twelveHour ? "9:30 AM" : "09:30"}
      title={invalid ? "Use a time like 9:30 AM or 21:30" : undefined}
      onFocus={() => { focused.current = true; }}
      onChange={(event) => handleChange(event.target.value)}
      onBlur={handleBlur}
      onKeyDown={(event) => {
        if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
        event.preventDefault();
        const next = shiftTime(parseTimeInput(draft) ?? "", event.key === "ArrowUp" ? 5 : -5);
        setDraft(formatTimeForDisplay(next, twelveHour));
        setInvalid(false);
        onChange(next);
      }}
    />
  );
}
