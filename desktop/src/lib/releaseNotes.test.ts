import { beforeEach, describe, expect, it, vi } from "vitest";
import { captureBootSnapshot, compareVersions, consumeStartupAnnouncement, releaseNotesBetween, resetBootSnapshotForTests, startupAnnouncement, RELEASE_NOTES } from "./releaseNotes";
import type { BootSnapshot } from "./releaseNotes";

const LAST_SEEN_KEY = "study-tracker-last-seen-version";
/** One of APP_STATE_STORAGE_KEYS - its presence is how an existing install is recognised. */
const APP_STATE_KEY = "study-tracker-desktop-v3-core";

function fakeLocalStorage(initial: Record<string, string> = {}) {
  const store = new Map(Object.entries(initial));
  return {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => { store.set(key, value); },
    removeItem: (key: string) => { store.delete(key); },
    clear: () => { store.clear(); },
    key: (index: number) => [...store.keys()][index] ?? null,
    get length() { return store.size; },
  } as Storage;
}

const WELCOME_KEY = "study-tracker-welcome-seen";

/** A returning install that has already been announced `lastSeen`. */
const returning = (lastSeen: string | null): BootSnapshot => ({ lastSeen, hadData: true, sawWelcome: true });

function versionsOf(announcement: ReturnType<typeof startupAnnouncement>): string[] {
  return announcement?.kind === "notes" ? announcement.releases.map((entry) => entry.version) : [];
}

beforeEach(() => {
  vi.unstubAllGlobals();
  resetBootSnapshotForTests();
});

const note = (title: string) => ({ title, items: [{ icon: "*", heading: title, body: title }] });

function withNotes<T>(versions: string[], run: () => T): T {
  const original = { ...RELEASE_NOTES };
  for (const key of Object.keys(RELEASE_NOTES)) delete RELEASE_NOTES[key];
  for (const version of versions) RELEASE_NOTES[version] = note(version);
  try {
    return run();
  } finally {
    for (const key of Object.keys(RELEASE_NOTES)) delete RELEASE_NOTES[key];
    Object.assign(RELEASE_NOTES, original);
  }
}

describe("compareVersions", () => {
  it("orders by each component, not lexically", () => {
    expect(compareVersions("0.1.9", "0.1.10")).toBeLessThan(0);
    expect(compareVersions("0.2.0", "0.1.99")).toBeGreaterThan(0);
    expect(compareVersions("1.0.0", "0.9.9")).toBeGreaterThan(0);
    expect(compareVersions("0.1.66", "0.1.66")).toBe(0);
  });

  it("ignores a prerelease or build tail", () => {
    expect(compareVersions("0.1.66-rc1", "0.1.66")).toBe(0);
    expect(compareVersions("0.1.67+build9", "0.1.66")).toBeGreaterThan(0);
  });
});

describe("releaseNotesBetween", () => {
  it("stacks every skipped release, newest first", () => {
    withNotes(["0.1.64", "0.1.65", "0.1.66", "0.1.67"], () => {
      expect(releaseNotesBetween("0.1.64", "0.1.67").map((entry) => entry.version)).toEqual([
        "0.1.67",
        "0.1.66",
        "0.1.65",
      ]);
    });
  });

  it("includes releases with no note for the current version", () => {
    withNotes(["0.1.65", "0.1.66"], () => {
      expect(releaseNotesBetween("0.1.64", "0.1.68").map((entry) => entry.version)).toEqual([
        "0.1.66",
        "0.1.65",
      ]);
    });
  });

  it("excludes the version already seen and anything newer than this build", () => {
    withNotes(["0.1.65", "0.1.66", "0.1.70"], () => {
      expect(releaseNotesBetween("0.1.65", "0.1.66").map((entry) => entry.version)).toEqual(["0.1.66"]);
    });
  });

  it("returns nothing on a downgrade or an unparseable last-seen version", () => {
    withNotes(["0.1.65", "0.1.66"], () => {
      expect(releaseNotesBetween("0.1.66", "0.1.65")).toEqual([]);
      expect(releaseNotesBetween("nonsense", "0.1.66")).toEqual([]);
    });
  });
});

describe("shipped notes", () => {
  it("are keyed by a parseable version and have items", () => {
    for (const [version, entry] of Object.entries(RELEASE_NOTES)) {
      expect(version, `${version} must be x.y.z`).toMatch(/^\d+\.\d+\.\d+/);
      expect(entry.title.length, `${version} needs a title`).toBeGreaterThan(0);
      expect(entry.items.length, `${version} needs items`).toBeGreaterThan(0);
    }
  });
});

describe("startupAnnouncement", () => {
  it("greets a first-ever run with the tour, not a changelog", () => {
    withNotes(["0.1.66"], () => {
      expect(startupAnnouncement({ lastSeen: null, hadData: false, sawWelcome: false }, "0.1.66"))
        .toEqual({ kind: "welcome" });
    });
  });

  it("gives the tour again when a first run was interrupted before finishing it", () => {
    withNotes(["0.1.66"], () => {
      // No data written yet and the tour was never marked seen: they never got through it.
      expect(startupAnnouncement({ lastSeen: "0.1.66", hadData: false, sawWelcome: false }, "0.1.66"))
        .toEqual({ kind: "welcome" });
    });
  });

  it("does not re-greet someone who finished the tour", () => {
    withNotes(["0.1.66"], () => {
      expect(startupAnnouncement({ lastSeen: "0.1.66", hadData: false, sawWelcome: true }, "0.1.66")).toBeNull();
    });
  });

  it("never shows the tour and the notes at the same time", () => {
    withNotes(["0.1.65", "0.1.66"], () => {
      const fresh = startupAnnouncement({ lastSeen: null, hadData: false, sawWelcome: false }, "0.1.66");
      expect(fresh).toEqual({ kind: "welcome" });
      expect(versionsOf(fresh)).toEqual([]);
    });
  });

  it("announces every note to an install upgrading from before the popup existed", () => {
    // A 0.1.50 user has data but no last-seen key, because 0.1.50 never wrote one.
    withNotes(["0.1.65", "0.1.66"], () => {
      expect(versionsOf(startupAnnouncement(returning(null), "0.1.66"))).toEqual(["0.1.66", "0.1.65"]);
    });
  });

  it("stacks only what was missed when a version is on record", () => {
    withNotes(["0.1.64", "0.1.65", "0.1.66", "0.1.67"], () => {
      expect(versionsOf(startupAnnouncement(returning("0.1.65"), "0.1.67"))).toEqual(["0.1.67", "0.1.66"]);
    });
  });

  it("says nothing on a relaunch of the same version", () => {
    withNotes(["0.1.65", "0.1.66"], () => {
      expect(startupAnnouncement(returning("0.1.66"), "0.1.66")).toBeNull();
    });
  });

  it("says nothing rather than showing an empty card when the range has no notes", () => {
    withNotes(["0.1.66"], () => {
      expect(startupAnnouncement(returning("0.1.66"), "0.1.68")).toBeNull();
    });
  });
});

describe("captureBootSnapshot", () => {
  it("reads the pre-run state and ignores later writes", () => {
    vi.stubGlobal("localStorage", fakeLocalStorage());
    const first = captureBootSnapshot();
    expect(first).toEqual({ lastSeen: null, hadData: false, sawWelcome: false });
    // The app's own first save lands after this; the snapshot must not move.
    localStorage.setItem(APP_STATE_KEY, "{}");
    localStorage.setItem(WELCOME_KEY, "1");
    expect(captureBootSnapshot()).toEqual(first);
  });

  it("treats blocked storage as a returning install, so nobody is greeted every boot", () => {
    vi.stubGlobal("localStorage", {
      getItem: () => { throw new Error("denied"); },
      setItem: () => { throw new Error("denied"); },
    } as unknown as Storage);
    expect(captureBootSnapshot()).toEqual({ lastSeen: null, hadData: true, sawWelcome: true, storageBlocked: true });
  });
});

describe("consumeStartupAnnouncement", () => {
  it("records the version so the popup does not come back", () => {
    const storage = fakeLocalStorage({ [APP_STATE_KEY]: "{}", [LAST_SEEN_KEY]: "0.1.65", [WELCOME_KEY]: "1" });
    vi.stubGlobal("localStorage", storage);
    withNotes(["0.1.66"], () => {
      expect(versionsOf(consumeStartupAnnouncement("0.1.66"))).toEqual(["0.1.66"]);
    });
    expect(storage.getItem(LAST_SEEN_KEY)).toBe("0.1.66");
    // Same boot state, but the version is now on record.
    resetBootSnapshotForTests();
    withNotes(["0.1.66"], () => {
      expect(consumeStartupAnnouncement("0.1.66")).toBeNull();
    });
  });

  it("stays silent when storage throws", () => {
    vi.stubGlobal("localStorage", {
      getItem: () => { throw new Error("denied"); },
      setItem: () => { throw new Error("denied"); },
    } as unknown as Storage);
    withNotes(["0.1.66"], () => {
      expect(consumeStartupAnnouncement("0.1.66")).toBeNull();
    });
  });
});
