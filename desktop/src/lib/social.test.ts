import { afterEach, describe, expect, it, vi } from "vitest";
import { getSquadScoreboard, getSyncSocialStats, MAX_SYNC_STAT_ROWS } from "./social";
import socialSource from "./social.ts?raw";
import type { SocialState, StudySession } from "../types";

function sessionForDate(date: string): StudySession {
  return {
    id: date,
    semesterId: null,
    courseId: null,
    taskId: null,
    kind: "study",
    goal: "",
    learned: "",
    blocker: "",
    nextStep: "",
    confidence: 3,
    startedAt: `${date}T08:00:00.000Z`,
    endedAt: `${date}T09:00:00.000Z`,
    minutes: 60,
    presetLabel: "Study",
  };
}

describe("social sync payload", () => {
  it("syncs only the newest stat rows", () => {
    const sessions = Array.from({ length: MAX_SYNC_STAT_ROWS + 5 }, (_, index) => {
      const date = new Date(Date.UTC(2025, 0, index + 1)).toISOString().slice(0, 10);
      return sessionForDate(date);
    });

    const stats = getSyncSocialStats(sessions);

    expect(stats).toHaveLength(MAX_SYNC_STAT_ROWS);
    expect(stats[0].date).toBe("2025-01-06");
    expect(stats.at(-1)?.date).toBe("2026-01-10");
  });
});

describe("getSquadScoreboard credential transport", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("sends userId/deviceSecret in the POST body, never in the URL", async () => {
    const fetchMock = vi.fn(async (_url: string, _init?: RequestInit) =>
      new Response(JSON.stringify({ entries: [] }), { status: 200, headers: { "content-type": "application/json" } })
    );
    vi.stubGlobal("fetch", fetchMock);

    const social = { userId: "test-user", deviceSecret: "super-secret-value" } as SocialState;
    await getSquadScoreboard(social, "season", true);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0];

    expect(url).toMatch(/\/squads\/scoreboard$/);
    expect(url).not.toContain("deviceSecret");
    expect(url).not.toContain("userId");
    expect(init?.method).toBe("POST");
    expect(JSON.parse(String(init?.body))).toMatchObject({ userId: "test-user", deviceSecret: "super-secret-value", period: "season" });
  });
});

describe("social.ts source — no credential-bearing URLs anywhere in the file", () => {
  it("never interpolates deviceSecret into a URL/query string", () => {
    expect(socialSource).not.toMatch(/deviceSecret=/);
  });
});
