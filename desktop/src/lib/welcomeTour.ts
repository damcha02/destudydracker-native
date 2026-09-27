import type { TabKey } from "../types";

/** One stop on the tour: the overview lists these, then each gets a step of its own. */
export type WelcomeTourStop = {
  /** The tab this stop is about, or "menu" for the header menu, which is not a tab. */
  id: TabKey | "menu";
  icon: string;
  label: string;
  /** One line, for the overview list on the first step. */
  tagline: string;
  /** The fuller explanation, shown when the stop gets its own step. */
  body: string;
};

/**
 * In tab-bar order, with the menu last. Deliberately a short orientation, not a replacement for the
 * per-page tutorial behind the tab bar's question mark.
 *
 * Keep this free of anything a single app style owns - the dashboard's layout names (Focus, Cockpit)
 * only exist in some styles, so naming them here is wrong for everyone else. Describe what a page is
 * FOR. Note that App.tsx's pageHelp has drifted from the app in
 * places, so it is a starting point, not a source of truth.
 */
export const WELCOME_TOUR_STOPS: WelcomeTourStop[] = [
  {
    id: "dashboard",
    icon: "📊",
    label: "Dashboard",
    tagline: "See what is up next.",
    body: "Open this first each day to find out what to work on. It gathers what is due, which courses are falling behind, and how close your exams are, then points at the one task worth starting now. When you have picked it, send it straight to the Timer.",
  },
  {
    id: "planner",
    icon: "🗂️",
    label: "Planner",
    tagline: "Build your semester here.",
    body: "This is where the structure lives: semesters hold courses, courses hold tasks and exams. Work is counted in pieces - lectures, sheets, readings - rather than hours, so progress means something. You can also spread those pieces across days on the calendar.",
  },
  {
    id: "timer",
    icon: "⏱️",
    label: "Timer",
    tagline: "Where the studying happens.",
    body: "Pick a preset or your own length, link the session to a course or task, and press Start. Press Save when you finish - an unsaved session does not count anywhere. Linked sessions are what make the Dashboard and Planner useful.",
  },
  {
    id: "vault",
    icon: "📓",
    label: "Vault",
    tagline: "References, summaries and notes.",
    body: "Keep the useful links and material for each course under References, longer write-ups and imported PDFs under Summaries, and short notes as you go under Notes. Everything is saved as plain text files in a folder you pick, so your notes stay yours.",
  },
  {
    id: "break",
    icon: "🪨",
    label: "Break Room",
    tagline: "Breaks you have earned.",
    body: "Study time fills a bar that unlocks short games and activities, so a break feels deliberate instead of accidental. There is also a water tracker, a stretch prompt, achievements, and a pet rock that asks nothing of you.",
  },
  {
    id: "friends",
    icon: "🤝",
    label: "Social",
    tagline: "Optional company and competition.",
    body: "Share a finished session to the feed, compare focus time on the leaderboard, add friends by code, or join a squad. All of it is opt-in - the app works exactly the same if you never open this tab.",
  },
  {
    id: "menu",
    icon: "⚙️",
    label: "The menu",
    tagline: "Themes, styles, and your settings.",
    body: "The button in the header holds Change theme and Change style - Study Tracker looks quite different across styles, so it is worth a look. Personal sets your name and daily goal, Options controls effects and which tabs appear, and Settings has updates and backups. Back up before big changes.",
  },
];

const WELCOME_SEEN_KEY = "study-tracker-welcome-seen";

/** Marks the tour as seen so it does not greet this install again. */
export function markWelcomeTourSeen(): void {
  try {
    localStorage.setItem(WELCOME_SEEN_KEY, "1");
  } catch {
    // A tour that cannot be recorded is better shown again than not shown at all.
  }
}

export function hasSeenWelcomeTour(): boolean {
  try {
    return localStorage.getItem(WELCOME_SEEN_KEY) !== null;
  } catch {
    return false;
  }
}
