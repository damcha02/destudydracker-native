export type Priority = "low" | "medium" | "high";
export type SessionKind = "study" | "break" | "exam";
export type TimerPhase = "idle" | "study" | "break" | "exam" | "stopwatch";
export type TimerMode = "focus" | "exam" | "endless";
export type TabKey = "dashboard" | "planner" | "timer" | "vault" | "friends" | "break";
export type SocialLeaderboardScope = "global" | "friends" | "squad";
export type SocialLeaderboardPeriod = "daily" | "weekly" | "overall";
export type SocialSquadScorePeriod = "daily" | "season" | "overall";
export type SocialFeedScope = "global" | "friends";
export type SocialSubtab = "feed" | "leaderboard" | "friends" | "squad" | "profile";
export type SocialFriendRequestStatus = "pending" | "accepted" | "declined";
export type SocialSquadRole = "leader" | "co_leader" | "elder" | "member";
export type SocialAvatarStyle = "classic" | "serif" | "cursive" | "graffiti" | "pixel" | "mono";
export type SocialAvatar =
  | { kind: "letter"; letter: string; style: SocialAvatarStyle }
  | { kind: "icon"; icon: string }
  | { kind: "photo"; name: string; url: string; mimeType: string };

export type SemesterPhase = "semester" | "exam-prep";

export interface Semester {
  id: string;
  name: string;
  createdAt: string;
  startDate: string | null;
  endDate: string | null;
  phase: SemesterPhase;
  archived: boolean;
  archivedAt: string | null;
}

export interface Course {
  id: string;
  semesterId: string;
  name: string;
  color: string;
  targetGrade: number;
  createdAt: string;
  externalUrl: string | null;
}

export type TaskSubtype = "Lecture" | "Session" | "Sheet" | "Other";

export interface Task {
  id: string;
  semesterId: string;
  courseId: string;
  title: string;
  subtype: TaskSubtype;
  unitLabel: string;
  totalUnits: number;
  completedUnits: number;
  dueDate: string | null;
  priority: Priority;
  notes: string;
  createdAt: string;
}

export interface Exam {
  id: string;
  semesterId: string;
  courseId: string;
  title: string;
  examDate: string;
  weight: number;
  preparedness: number;
  location: string;
}

export type Weekday = 0 | 1 | 2 | 3 | 4 | 5 | 6;

export type TimetableEventKind = "occurrence" | "sheet-release" | "sheet-deadline";

export interface TimetableOccurrenceOverride {
  skipped?: true;
  date?: string;
  time?: string;
  endTime?: string | null;
}

export interface TimetableEvent {
  id: string;
  semesterId: string;
  courseId: string;
  kind: TimetableEventKind;
  taskId: string;
  label: string;
  date: string;
  time: string;
  endTime: string | null;
  repeatWeekly: boolean;
  recurrenceEndDate: string | null;
  occurrenceOverrides: Record<string, TimetableOccurrenceOverride>;
  url: string | null;
  completedOccurrences: string[];
  createdAt: string;
}

export interface Holiday {
  id: string;
  semesterId: string;
  startDate: string;
  endDate: string;
  label: string;
  createdAt: string;
}

export interface DailyTodo {
  id: string;
  date: string;
  time: string | null;
  endTime: string | null;
  title: string;
  notes: string;
  completed: boolean;
  completedAt: string | null;
  createdAt: string;
  // A weekly-repeating to-do keeps completed/completedAt for its own (anchor) date, but every
  // projected occurrence's completion is tracked separately here, by date - the same
  // completedOccurrences pattern TimetableEvent already uses for recurring items.
  repeatWeekly: boolean;
  completedOccurrences: string[];
  // Series controls for a repeating to-do, mirroring TimetableEvent: an inclusive last date, dates
  // dropped from the series, and per-date time overrides ("this occurrence only" moves).
  recurrenceEndDate: string | null;
  skippedOccurrences: string[];
  occurrenceTimes: Record<string, { time: string | null; endTime: string | null }>;
}

export interface CalendarEntry {
  id: string;
  taskId: string;
  date: string;
  unitAmount: 1 | 0.5 | 0.25;
  unitStart?: number;
  completed: boolean;
  completedAt: string | null;
  createdAt: string;
  startTime?: string;
  endTime?: string;
  adHocTitle?: string;
  adHocSemesterId?: string;
  adHocCourseId?: string;
}

export interface StudySession {
  id: string;
  semesterId: string | null;
  courseId: string | null;
  taskId: string | null;
  kind: SessionKind;
  goal: string;
  learned: string;
  blocker: string;
  nextStep: string;
  confidence: number;
  startedAt: string;
  endedAt: string;
  minutes: number;
  presetLabel: string;
}

export interface VaultExport {
  id: string;
  exportedAt: string;
  notePath: string;
  noteDate: string;
}

export interface Settings {
  accent: string;
  userName: string;
  dailyGoalMinutes: number;
  themeFamily: "normal";
  backgroundEffect: boolean;
  hideFeedImages: boolean;
  hideProfilePictures: boolean;
  hideFeedPolls: boolean;
  showHelpButton: boolean;
  telemetryEnabled: boolean;
  vaultPath: string | null;
  vaultName: string;
  visibleTabs: Record<TabKey, boolean>;
}

export interface SocialLeaderboardEntry {
  userId: string;
  displayName: string;
  friendCode: string;
  avatar?: SocialAvatar;
  minutes: number;
  sessions: number;
  rank: number;
  lastActiveDate: string | null;
  isSelf?: boolean;
}

export interface SocialFriendRequest {
  id: string;
  fromUserId: string;
  toUserId: string;
  fromDisplayName: string;
  toDisplayName: string;
  fromFriendCode: string;
  toFriendCode: string;
  fromAvatar?: SocialAvatar;
  toAvatar?: SocialAvatar;
  status: SocialFriendRequestStatus;
  createdAt: string;
}

export interface SocialFriend {
  userId: string;
  displayName: string;
  friendCode: string;
  avatar?: SocialAvatar;
  friendsSince: string;
  lastSeenAt: string | null;
}

export interface SocialFeedPost {
  id: string;
  userId: string;
  displayName: string;
  friendCode: string;
  avatar?: SocialAvatar;
  type: "session" | "milestone";
  subject: string;
  detail: string;
  note: string;
  icon: string;
  minutes: number;
  presetLabel: string;
  createdAt: string;
  isSelf?: boolean;
  imageUrl?: string | null;
  imageMimeType?: string | null;
  imageExpiresAt?: string | null;
  imageExpiredAt?: string | null;
  poll?: SocialFeedPoll | null;
  reactions: Record<string, number>;
  reacted?: Record<string, boolean>;
  reactedBy?: Record<string, string[]>;
  comments?: SocialFeedComment[];
}

export interface SocialFeedPoll {
  question: string;
  multiple: boolean;
  options: SocialFeedPollOption[];
  totalVotes: number;
}

export interface SocialFeedPollOption {
  id: string;
  text: string;
  votes: number;
  selected?: boolean;
}

export interface SocialFeedComment {
  id: string;
  postId: string;
  userId: string;
  displayName: string;
  friendCode: string;
  avatar?: SocialAvatar;
  body: string;
  createdAt: string;
  isSelf?: boolean;
}

export interface SocialSquadMember {
  userId: string;
  displayName: string;
  friendCode: string;
  avatar?: SocialAvatar;
  role: SocialSquadRole;
  joinedAt: string;
  lastSeenAt: string | null;
  minutes: number;
  sessions: number;
  isSelf?: boolean;
}

export interface SocialSquad {
  id: string;
  name: string;
  isPrivate: boolean;
  createdByUserId: string;
  createdAt: string;
  totalMinutes: number;
  totalSessions: number;
  memberCount: number;
  myRole: SocialSquadRole;
  members: SocialSquadMember[];
}

export interface SocialSquadJoinRequest {
  id: string;
  squadId: string;
  squadName?: string;
  userId?: string;
  displayName?: string;
  friendCode?: string;
  avatar?: SocialAvatar;
  isPrivate?: boolean;
  status: SocialFriendRequestStatus;
  createdAt: string;
}

export interface SocialSquadMessage {
  id: string;
  squadId: string;
  userId: string;
  displayName: string;
  friendCode: string;
  avatar?: SocialAvatar;
  role: SocialSquadRole;
  body: string;
  createdAt: string;
  isSelf?: boolean;
}

export interface SocialSquadScoreEntry {
  squadId: string;
  squadName: string;
  isPrivate: boolean;
  memberCount: number;
  totalMinutes: number;
  totalSessions: number;
  averageMinutes: number;
  rank: number;
  points: number;
  scoredDays?: number;
}

export interface SocialSquadDetails extends Omit<SocialSquad, "myRole"> {
  maxMembers: number;
  statsDate?: string;
  previousDayDate?: string;
  previousDayTotalMinutes?: number;
  previousDayTotalSessions?: number;
  previousDayMemberCount?: number;
  previousDayAverageMinutes?: number;
  action: "join" | "request" | "pending" | "full" | "unavailable" | "current";
}

export interface SocialState {
  userId: string;
  deviceSecret: string;
  friendCode: string;
  displayName: string;
  avatar: SocialAvatar;
  lastSyncedAt: string | null;
  lastSyncError: string | null;
  nextAutoSyncAt: string | null;
  isPrivate: boolean;
  autoPostSessions: boolean;
  showHoursToFriends: boolean;
  friends: SocialFriend[];
  incomingFriendRequests: SocialFriendRequest[];
  outgoingFriendRequests: SocialFriendRequest[];
  squad: SocialSquad | null;
  incomingSquadRequests: SocialSquadJoinRequest[];
  outgoingSquadRequests: SocialSquadJoinRequest[];
  squadMessages: SocialSquadMessage[];
  cachedFeeds: Record<SocialFeedScope, SocialFeedPost[]>;
  pendingFeedPosts: SocialFeedPost[];
  pendingFeedPostDeletions: string[];
  cachedLeaderboards: Record<SocialLeaderboardScope, Record<SocialLeaderboardPeriod, SocialLeaderboardEntry[]>>;
  cachedSquadScoreLeaderboards: Record<SocialSquadScorePeriod, SocialSquadScoreEntry[]>;
  /** The last point the server actually acknowledged a verified-session call — the boundary offline credit is measured from. */
  verifiedAnchor: { sessionId: string; confirmedAt: string } | null;
}

export interface TimerActiveSegment {
  startedAt: string;
  endedAt: string | null;
}

export interface TimerState {
  phase: TimerPhase;
  mode: TimerMode;
  remainingSeconds: number;
  loggedSplitSeconds: number;
  activeSegments: TimerActiveSegment[];
  running: boolean;
  studyMinutes: number;
  breakMinutes: number;
  examMinutes: number;
  startedAt: string | null;
  endsAt: string | null;
  semesterId: string | null;
  courseId: string | null;
  taskId: string | null;
  goal: string;
  learned: string;
  blocker: string;
  nextStep: string;
  confidence: number;
  presetLabel: string;
  lastAliveAt: string | null;
}

export interface PlayedBreak {
  name: string;
  playedAt: string;
}

export interface DurakPuzzleState {
  seed: string | null;
  hint: string;
  playerHand: string[];
  cpuHand: string[];
  trumpSuit: string;
  table: Array<{ attack: string; defense?: string; attackBy?: "player" | "cpu"; defenseBy?: "player" | "cpu" }>;
  discardPile: string[];
  phase: string;
  winner?: string;
  message: string;
  failures: number;
  completed: boolean;
  solvedCount: number;
}

export interface WordlePuzzleState {
  seedSalt: string;
  activeDate: string;
  puzzleId: string;
  answer: string;
  guesses: string[];
  completed: boolean;
  won: boolean;
  hardMode: boolean;
}

export interface GeodlePuzzleState {
  seedSalt: string;
  activeDate: string;
  puzzleId: string;
  answer: string;
  guesses: string[];
  completed: boolean;
  won: boolean;
}

export interface FlaggleGuess {
  country: string;
  similarity: number;
  maskedFlagDataUrl: string;
}

export interface FlagglePuzzleState {
  seedSalt: string;
  activeDate: string;
  puzzleId: string;
  answer: string;
  guesses: FlaggleGuess[];
  completed: boolean;
  won: boolean;
}

export interface TravlePuzzleState {
  seedSalt: string;
  activeDate: string;
  puzzleId: string;
  start: string;
  target: string;
  guesses: string[];
  completed: boolean;
  won: boolean;
}

export interface AppState {
  semesters: Semester[];
  courses: Course[];
  tasks: Task[];
  exams: Exam[];
  calendarEntries: CalendarEntry[];
  timetableEvents: TimetableEvent[];
  holidays: Holiday[];
  dailyTodos: DailyTodo[];
  sessions: StudySession[];
  lifetimeStudyMinutes: number;
  lifetimeStudySessions: number;
  exports: VaultExport[];
  settings: Settings;
  social: SocialState;
  timer: TimerState;
  activeTab: TabKey;
  unlockedGames: string[];
  unlockedGamesDate: string;
  playedBreaks: PlayedBreak[];
  playedBreaksDate: string;
  totalUnlocks: number;
  unlockStreak: number;
  lastUnlockDate: string;
  speedrunnerToday: boolean;
  playedGamesAllTime: string[];
  badgeCounts: Record<string, number>;
  badgeCountDates: Record<string, string>;
  waterGlasses: number;
  waterDate: string;
  petRockPats: number;
  /** Achievements hung on the Rest room board; x/y are the icon centre as a fraction of the board. */
  achievementBoard: Array<{ id: string; uid?: string; x: number; y: number; size?: number; color?: string; rot?: number; icon?: string; name?: string; how?: string }>;
  /** The day (YYYY-MM-DD) each achievement id was actually seen go from not-earned to earned, recorded
   * the moment that happens - never back-filled for one already earned when tracking began, since that
   * day is genuinely unknown. Powers the "book" achievement wall, which titles each entry by the day it
   * was earned and leaves entries with no recorded day untitled. */
  achievementEarnedOnDates: Record<string, string>;
  durakPuzzle: DurakPuzzleState;
  wordlePuzzle: WordlePuzzleState;
  geodlePuzzle: GeodlePuzzleState;
  flagglePuzzle: FlagglePuzzleState;
  travlePuzzle: TravlePuzzleState;
}
