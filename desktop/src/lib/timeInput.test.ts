import { describe, expect, it } from "vitest";
import { formatTimeForDisplay, parseTimeInput, shiftTime } from "./timeInput";
import { isValidIsoDate } from "./plannerSchedule";

describe("parseTimeInput", () => {
  it("accepts 24h and am/pm styles", () => {
    expect(parseTimeInput("9:30")).toBe("09:30");
    expect(parseTimeInput("930")).toBe("09:30");
    expect(parseTimeInput("21:05")).toBe("21:05");
    expect(parseTimeInput("9")).toBe("09:00");
    expect(parseTimeInput("9:30 PM")).toBe("21:30");
    expect(parseTimeInput("9:30pm")).toBe("21:30");
    expect(parseTimeInput("12 am")).toBe("00:00");
    expect(parseTimeInput("12:30 pm")).toBe("12:30");
    expect(parseTimeInput("9 a.m.")).toBe("09:00");
  });

  it("rejects things that aren't times", () => {
    for (const bad of ["", "abc", "25:00", "9:75", "13 pm", "0 am", "9:3"]) expect(parseTimeInput(bad)).toBeNull();
  });
});

describe("time display helpers", () => {
  it("formats for 12h and 24h clocks", () => {
    expect(formatTimeForDisplay("21:30", true)).toBe("9:30 PM");
    expect(formatTimeForDisplay("00:05", true)).toBe("12:05 AM");
    expect(formatTimeForDisplay("12:00", true)).toBe("12:00 PM");
    expect(formatTimeForDisplay("21:30", false)).toBe("21:30");
    expect(formatTimeForDisplay("", true)).toBe("");
  });

  it("steps in minutes and stays within the day", () => {
    expect(shiftTime("09:00", 5)).toBe("09:05");
    expect(shiftTime("00:00", -5)).toBe("00:00");
    expect(shiftTime("", 5)).toBe("09:05");
  });
});

describe("isValidIsoDate", () => {
  it("only accepts real YYYY-MM-DD dates", () => {
    expect(isValidIsoDate("2026-09-20")).toBe(true);
    expect(isValidIsoDate("2026-02-30")).toBe(false);
    expect(isValidIsoDate("20.09.2026")).toBe(false);
    expect(isValidIsoDate("")).toBe(false);
  });
});
