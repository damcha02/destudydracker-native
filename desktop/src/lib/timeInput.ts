/** Parses what a person types into a time field ("9", "930", "9:30", "9:30pm", "21:30", "12 am") to "HH:MM", or null if it isn't a time. */
export function parseTimeInput(text: string): string | null {
  const match = text.trim().toLowerCase().match(/^(\d{1,2})(?:[:.h]?(\d{2}))?\s*(?:([ap])\.?\s*m?\.?)?$/);
  if (!match) return null;
  let hours = Number(match[1]);
  const minutes = match[2] === undefined ? 0 : Number(match[2]);
  const meridiem = match[3];
  if (minutes > 59) return null;
  if (meridiem) {
    if (hours < 1 || hours > 12) return null;
    hours = (hours % 12) + (meridiem === "p" ? 12 : 0);
  } else if (hours > 23) {
    return null;
  }
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}`;
}

/** Whether this machine's locale shows clocks as 12-hour am/pm. */
export function prefersTwelveHourClock(): boolean {
  try {
    return Boolean(new Intl.DateTimeFormat(undefined, { hour: "numeric" }).resolvedOptions().hour12);
  } catch {
    return false;
  }
}

/** "HH:MM" -> what the field should display ("9:30 PM" or "21:30"). */
export function formatTimeForDisplay(time: string, twelveHour: boolean): string {
  const parsed = parseTimeInput(time);
  if (!parsed) return "";
  if (!twelveHour) return parsed;
  const [hours, minutes] = parsed.split(":").map(Number);
  const suffix = hours >= 12 ? "PM" : "AM";
  return `${hours % 12 === 0 ? 12 : hours % 12}:${String(minutes).padStart(2, "0")} ${suffix}`;
}

export function shiftTime(time: string, deltaMinutes: number): string {
  const [hours, minutes] = (parseTimeInput(time) ?? "09:00").split(":").map(Number);
  const total = Math.min(Math.max(hours * 60 + minutes + deltaMinutes, 0), 23 * 60 + 59);
  return `${String(Math.floor(total / 60)).padStart(2, "0")}:${String(total % 60).padStart(2, "0")}`;
}

const twelveHourClock = prefersTwelveHourClock();

/** "HH:MM" shown the way this machine's locale writes clock times. */
export function displayTime(time: string): string {
  return formatTimeForDisplay(time, twelveHourClock) || time;
}
