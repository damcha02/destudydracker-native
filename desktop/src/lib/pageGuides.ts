import type { TabKey } from "../types";

export type PageGuideStep = {
  /** The short imperative that leads the line, e.g. "Add your courses". */
  lead: string;
  body: string;
};

export type PageGuide = {
  icon: string;
  title: string;
  /** One line under the title, same role as the welcome tour's tagline. */
  tagline: string;
  intro: string;
  /** Heading above the numbered list - name the thing being achieved, not "Steps". */
  stepsHeading: string;
  steps: PageGuideStep[];
  /** Short asides shown under the list. Things worth knowing that are not a step. */
  tips?: string[];
};

/**
 * The page guide behind each tab's question mark. Dashboard is deliberately absent: its question
 * mark reopens the welcome introduction instead (see App.tsx's openPageHelp).
 *
 * Keep these free of anything one app style owns - widget and layout names differ between Modern,
 * Field Notebook and Wabi-Sabi, so describe what a page is FOR and what order to do things in.
 */
export const PAGE_GUIDES: Record<Exclude<TabKey, "dashboard">, PageGuide> = {
  planner: {
    icon: "🗂️",
    title: "Planner",
    tagline: "Build your semester, one layer at a time.",
    intro: "Everything else in the app reads from what you set up here. It nests: a semester holds courses, a course holds tasks and exams, and a task holds the countable pieces you actually work through.",
    stepsHeading: "Setting up a semester",
    steps: [
      { lead: "Create the semester", body: "Start with one container for this term. You can give it a start and end date so the calendar and workload maths know the range." },
      { lead: "Add your courses", body: "One per real course. Give each a colour - that colour follows the course through the calendar, the timer and your stats." },
      { lead: "Add tasks and exams", body: "Tasks are the real work: lecture series, exercise sheets, readings, past papers, revision sets. Pick the subtype first, because it changes what the task tracks." },
      { lead: "Count the pieces, not the hours", body: "Say how many lectures or sheets there are in total. Progress is 4 of 12 lectures, never 'about two hours' - which is what makes the health scores mean something." },
      { lead: "Tick pieces off as you finish", body: "Use plus when a piece is done, minus if you counted one by mistake. Everything downstream updates from this." },
      { lead: "Spread the work across days", body: "Optionally place tasks on the calendar so you have a day-by-day plan instead of one intimidating pile." },
    ],
    tips: [
      "Falling behind shows up as a course's health dropping, well before the deadline does.",
      "A task with no due date is fine - it just will not push its way up your Dashboard.",
    ],
  },
  timer: {
    icon: "⏱️",
    title: "Timer",
    tagline: "Where the studying actually happens.",
    intro: "The Timer is what turns a plan into recorded work. A session that is linked to a course and task feeds your progress, your streak, your course health and your stats - an unlinked one only counts as time.",
    stepsHeading: "Running a session",
    steps: [
      { lead: "Pick a length", body: "Pomodoro for short bursts, Deep Work for long blocks, Exam for timed practice, Endless when you genuinely do not know. Custom sets your own focus and break minutes." },
      { lead: "Link it to your work", body: "Choose the semester, course and task. With a task linked, the timer shows exactly which piece is next, so there is no deciding left to do once it starts." },
      { lead: "Start, and leave it alone", body: "Pause and resume exist if you need them. The clock keeps its own time, so it survives the window being closed." },
      { lead: "Save when you stop", body: "This is the one that matters: an unsaved session is not recorded anywhere. Save writes the time and pushes the progress out into the rest of the app." },
      { lead: "Optionally, write it down", body: "The session log takes what you learned, what blocked you, and where to pick up next time. Future you will not remember otherwise." },
    ],
    tips: [
      "Send a task here straight from the Planner or Dashboard and it arrives already linked.",
      "A good goal names what should exist when the timer stops, not just the subject.",
    ],
  },
  vault: {
    icon: "📓",
    title: "Vault",
    tagline: "References, summaries and notes.",
    intro: "Three places to put things, kept as plain text files in a folder you choose. Nothing is trapped in the app - you can open, back up or move the whole lot with any editor.",
    stepsHeading: "Finding a home for things",
    steps: [
      { lead: "Choose a folder first", body: "Create a new one or point at a folder you already keep notes in. Nothing can be saved until this is set." },
      { lead: "References: the useful links", body: "One note per course for the things you keep needing - lecture recordings, the good textbook chapter, that one Stack Exchange answer, the professor's office hours." },
      { lead: "Summaries: the long-form stuff", body: "Per-course write-ups and imported PDFs. This is where a whole chapter gets condensed into something you can actually revise from." },
      { lead: "Notes: whatever is in your head now", body: "Short, dated notes as you go. Quick thoughts, a reflection after a session, a thing to check tomorrow." },
    ],
    tips: [
      "Just finished a session? Pull its reflection straight into a note instead of starting from a blank page.",
      "Files are plain text, so back them up the same way you back up anything else.",
    ],
  },
  break: {
    icon: "🪨",
    title: "Break Room",
    tagline: "Breaks you have actually earned.",
    intro: "A break you chose beats a break that happened to you. Studying earns XP, XP unlocks things to do in here, and the whole room is built to be easy to walk away from again.",
    stepsHeading: "Taking a proper break",
    steps: [
      { lead: "Earn it first", body: "Saved timer sessions fill the XP bar. The room deliberately rewards focus before it rewards resting." },
      { lead: "Unlock something", body: "Each card is a short activity. Locked ones tell you how much more study time they need, so there is always a next one in sight." },
      { lead: "Play, then leave", body: "Everything here is short on purpose. It is a reset, not a second app." },
      { lead: "Use the small cues", body: "Water tracker, a stretch prompt, and achievements for good break habits. One click each, no ceremony." },
      { lead: "Pat the rock", body: "It asks nothing of you and gives nothing back. That is the point." },
    ],
  },
  friends: {
    icon: "🤝",
    title: "Social",
    tagline: "Optional company and competition.",
    intro: "Some people study better with someone watching. All of this is opt-in - the rest of Study Tracker behaves identically if you never open this tab.",
    stepsHeading: "Finding your people",
    steps: [
      { lead: "Set a name people recognise", body: "You start with a generated one. Change it before adding anyone, or nobody will know which Student A3F9 you are." },
      { lead: "Add friends by code", body: "Swap codes with people you actually study with. Your friends feed stays small and quiet, which is the point of it." },
      { lead: "Share a finished session", body: "After saving a session you can post it, with a note, an image or a poll. Or post nothing and just read - that is a normal way to use this." },
      { lead: "Compare on the leaderboard", body: "Focus time across friends, squads, or everyone. Useful if competition motivates you, easy to ignore if it does not." },
      { lead: "Join or start a squad", body: "A group with a shared scoreboard, for motivation that lasts longer than one day. A squad of one is allowed - you are on the board straight away." },
    ],
    tips: ["Only what you choose to post is shared. Your tasks, notes and grades never leave your device."],
  },
};
