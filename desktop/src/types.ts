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

export interface Semester {
  id: string;
  name: string;
  createdAt: string;
}

export interface Course {
  id: string;
  semesterId: string;
  name: string;
  color: string;
  targetGrade: number;
  createdAt: string;
}

export interface Task {
  id: string;
  semesterId: string;
  courseId: string;
  title: string;
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
  durakPuzzle: DurakPuzzleState;
  wordlePuzzle: WordlePuzzleState;
  geodlePuzzle: GeodlePuzzleState;
  flagglePuzzle: FlagglePuzzleState;
  travlePuzzle: TravlePuzzleState;
}
