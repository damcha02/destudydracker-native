import { APP_STATE_STORAGE_KEYS } from "./storage";
import { hasSeenWelcomeTour } from "./welcomeTour";

export type ReleaseNote = {
  title: string;
  items: { icon: string; heading: string; body: string }[];
};

/** One release worth announcing, as the popup shows it. */
export type ReleaseNoteEntry = ReleaseNote & { version: string };

// Add an entry for each release you want announced. Versions without an entry show nothing.
// Order items by what a user most wants to hear: new features first, then improvements, then fixes
// and polish. The list is read top-down and the first line is the one everyone reads.
export const RELEASE_NOTES: Record<string, ReleaseNote> = {
  "0.1.67": {
    title: "Nothing slips through",
    items: [
      { icon: "📌", heading: "The Pinwall catches what you skipped", body: "A new button on the Planner shows every lecture, exercise sheet, study block and to-do whose day has passed without being ticked off. Pick this week, the past month or the whole semester, group it by subject or by type, and tick things off right there - the counts, the calendar and your Dashboard all update with it. The Planner button carries this week's number, or a tick when the wall is clear." },
      { icon: "👋", heading: "A guided tour on your first run", body: "Opening Study Tracker for the first time now walks you through what each page is for, instead of dropping you into an empty Dashboard." },
      { icon: "❓", heading: "Every page explains itself", body: "The question mark next to the tab bar opens a guide for the page you are on: what it is for, the order to do things in, and a few things worth knowing. On the Dashboard it brings back the introduction." },
      { icon: "📓", heading: "The Vault starts quieter", body: "Before you have linked a folder, the Vault no longer shows drawers and setup controls you cannot use yet - just a line telling you what unlocks them. It also stops calling itself an Obsidian vault: the notes are plain markdown files in a folder you own, and any editor can read them." },
    ],
  },
  "0.1.66": {
    title: "Roomier arena, smoother pages",
    items: [
      { icon: "🏆", heading: "Solo squads can enter the Arena", body: "A squad no longer needs a second member to show up on the Squad Arena board. Start one and you are already on the scoreboard." },
      { icon: "🎨", heading: "Change style is its own menu item", body: "Styles sit next to Change theme in the menu, so switching the overall look is one click instead of a tab hidden inside the theme panel." },
      { icon: "✨", heading: "This popup", body: "After every update you get a short note like this one, so you know what moved." },
      { icon: "📖", heading: "The achievement book turns properly", body: "In Wabi-Sabi, the book's cover now swings open on the spine like a real page instead of squashing flat, and turning a page no longer flashes the spread you just left." },
      { icon: "🗓️", heading: "No more overlapping entries", body: "In the Wabi-Sabi achievement book, a long name used to let its date land on top of the line above it. Every entry now keeps to its own row." },
    ],
  },
};

const LAST_SEEN_KEY = "study-tracker-last-seen-version";

/** [major, minor, patch]; any `-rc1`/`+build` tail is ignored, and anything unparseable sorts last. */
function parseVersion(version: string): [number, number, number] | null {
  const match = /^(\d+)\.(\d+)\.(\d+)/.exec(version.trim());
  return match ? [Number(match[1]), Number(match[2]), Number(match[3])] : null;
}

/** Negative when a < b, positive when a > b, 0 when equal. Unparseable versions compare as equal. */
export function compareVersions(a: string, b: string): number {
  const left = parseVersion(a);
  const right = parseVersion(b);
  if (!left || !right) return 0;
  for (let index = 0; index < 3; index += 1) {
    if (left[index] !== right[index]) return left[index] - right[index];
  }
  return 0;
}

/**
 * Every announced release in (after, current], newest first - so someone who skipped four updates
 * gets all four, not just the newest. `after` being unparseable (or the range being empty) yields
 * nothing rather than the whole history.
 */
export function releaseNotesBetween(after: string, current: string): ReleaseNoteEntry[] {
  if (!parseVersion(after) || !parseVersion(current)) return [];
  return Object.keys(RELEASE_NOTES)
    .filter((version) => compareVersions(version, after) > 0 && compareVersions(version, current) <= 0)
    .sort((a, b) => compareVersions(b, a))
    .map((version) => ({ version, ...RELEASE_NOTES[version] }));
}

/** Every announced release up to and including `current`, newest first. */
function releaseNotesUpTo(current: string): ReleaseNoteEntry[] {
  return releaseNotesBetween("0.0.0", current);
}

/**
 * What localStorage looked like before this run of the app touched it. `hadData` is the only thing
 * that distinguishes a first-ever run from an upgrade out of a version that predates the popup, and
 * the app starts saving state 700ms after mount - so it MUST be read at startup, before that first
 * save, not when the async version lookup happens to come back. See captureBootSnapshot.
 */
export type BootSnapshot = {
  /** The version last announced to this install, or null if none was ever recorded. */
  lastSeen: string | null;
  /** True when Study Tracker data was already present, i.e. this is not a first run. */
  hadData: boolean;
  /** True when the welcome tour has already been given. */
  sawWelcome: boolean;
  /** True when localStorage could not be read, so nothing can be recorded either. */
  storageBlocked?: boolean;
};

let bootSnapshot: BootSnapshot | null = null;

/**
 * Reads the pre-run state of this install and remembers it. Call once, as early as possible - see
 * main.tsx. Safe to call again; later calls are ignored so a remount cannot overwrite the real
 * snapshot with one taken after the app has already saved.
 */
export function captureBootSnapshot(): BootSnapshot {
  if (bootSnapshot) return bootSnapshot;
  try {
    bootSnapshot = {
      lastSeen: localStorage.getItem(LAST_SEEN_KEY),
      hadData: APP_STATE_STORAGE_KEYS.some((key) => localStorage.getItem(key) !== null),
      sawWelcome: hasSeenWelcomeTour(),
    };
  } catch {
    // Nothing can be read, so nothing can be recorded either: anything shown now would come back on
    // every single launch. Silence is the lesser evil.
    bootSnapshot = { lastSeen: null, hadData: true, sawWelcome: true, storageBlocked: true };
  }
  return bootSnapshot;
}

/** Test seam: forget the captured snapshot so the next capture reads storage again. */
export function resetBootSnapshotForTests(): void {
  bootSnapshot = null;
}

/** What to greet this boot with, if anything. At most one of the two ever applies. */
export type StartupAnnouncement =
  | { kind: "welcome" }
  | { kind: "notes"; releases: ReleaseNoteEntry[] };

/** Pure decision, given the pre-run state of the install. */
export function startupAnnouncement(snapshot: BootSnapshot, version: string): StartupAnnouncement | null {
  const { lastSeen, hadData, sawWelcome, storageBlocked } = snapshot;
  if (storageBlocked) return null;
  // A first-ever run gets the tour, not a changelog: nothing has "changed" from this user's point of
  // view, and they have not seen the app yet. An interrupted first run (tour never finished, still no
  // data) gets it again rather than losing it.
  if (!hadData && !sawWelcome) return { kind: "welcome" };
  if (lastSeen === version) return null;
  // No version on record but data present: an upgrade out of a version that predates this popup
  // (<= 0.1.65), which never wrote the key. We cannot know which version they came from, so show every
  // note we have rather than guess a range; the card scrolls if that is a lot. This stops mattering
  // once everyone has booted 0.1.66 once.
  const releases = lastSeen === null ? releaseNotesUpTo(version) : releaseNotesBetween(lastSeen, version);
  return releases.length ? { kind: "notes", releases } : null;
}

/**
 * The greeting for this boot, and records the version as seen so it does not come back. Reads the
 * install state captured at startup, not the live one.
 */
export function consumeStartupAnnouncement(version: string): StartupAnnouncement | null {
  const snapshot = captureBootSnapshot();
  try {
    localStorage.setItem(LAST_SEEN_KEY, version);
  } catch {
    // Unrecorded, so it may show again. Better than swallowing the announcement entirely.
  }
  return startupAnnouncement(snapshot, version);
}
