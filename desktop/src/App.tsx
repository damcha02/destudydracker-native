import { useEffect, useEffectEvent, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { CSSProperties, ChangeEvent, DragEvent, FormEvent, KeyboardEvent, MouseEvent, ReactNode } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import type { Update } from "@tauri-apps/plugin-updater";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import type { PDFDocumentProxy } from "pdfjs-dist";
import "./App.css";
import { TimerClockDigits } from "./components/TimerClockDigits";
import { WabiRestFluidRing } from "./components/WabiRestFluidRing";
import { useTimerProgressRing } from "./hooks/useTimerProgressRing";
import { useTimerTick } from "./hooks/useTimerTick";
import {
  buildDailyNoteMarkdown,
  calculateAggregateWorkload,
  calculateDailyWork,
  clamp,
  daysUntil,
  formatDate,
  formatMinutes,
  getCourseHealthMap,
  getCourseMinutesMap,
  getCourseTasks,
  getFirstSessionDate,
  getFocusMomentum,
  getLocalMonthlyStats,
  getOverallHealth,
  getRemainingUnits,
  getSemesterCourses,
  getSemesterHealthMap,
  getSemesterTasks,
  getSessionDaySet,
  getStreakDays,
  getStudySessionCount,
  getTaskProgress,
  getTodayMinutes,
  getUnitsCompletedToday,
  getUpcomingExams,
  getWeeklyActivity,
  getWeeklyCourseCount,
  groupTasksByCourseId,
  groupTasksBySemesterId,
  isoDate,
} from "./lib/metrics";
import { createVault, deleteNote, importSummaryFiles, isTauriApp, linkVault, listNotes, listSummaryFiles, pickExistingVaultDirectory, pickSummaryFiles, pickVaultParentDirectory, readDailyNote, readNote, readReferenceNote, readSummaryPdf, writeDailyNote, writeNote, writeReferenceNote } from "./lib/obsidian";
import type { SummaryFile, VaultNoteFile } from "./lib/obsidian";
import { commentOnFeedPost, createFriendRequest, createSquad, deleteFeedPost, deleteFeedPostImage, deleteSquadMessage, finishVerifiedSession, getAdminUsage, getCurrentAnnouncement, getFriendStatus, getLeaderboardWithLocalSelf, getLocalLeaderboardEntry, getNextAutoSyncAt, getPlayerStats, getSocialFeed, getSocialLeaderboard, getSquadDetails, getSquadScoreboard, heartbeatVerifiedSession, isSocialApiConfigured, joinSquad, kickSquadMember, leaveSquad, notifyUsersAboutUpdate, presencePing, reactToFeedPost, reconcileOfflineVerifiedCredit, respondToFriendRequest, respondToSquadRequest, searchSquads, sendSquadMessage, sendTelemetryHeartbeat, setSquadMemberRole, shouldAutoSyncSocial, startVerifiedSession, syncSocialState, updateFeedPost, updateSquadSettings, uploadFeedPostImage, uploadProfileAvatar, voteOnFeedPoll } from "./lib/social";
import type { AdminUsageResponse, AppAnnouncement, AppMetadata, PlayerStatsResponse, R2UsageStatus, SquadSearchResult } from "./lib/social";
import { APP_STATE_STORAGE_KEYS, applyPersistedSections, createInitialPersistenceBaselines, defaultState, defaultTimer, downloadBackup, loadAppState, makeId, saveAppState } from "./lib/storage";
import type { PersistenceBaselines, PersistSection } from "./lib/storage";
import { startTimerPersistenceHeartbeat } from "./lib/timerPersistence";
import { closeTimerSegments, getDisplayRemainingSeconds, getIdleTimerSeconds, getTimerActiveSeconds } from "./lib/timerDisplay";
import { pauseCountdownTimer, pauseStopwatchTimer, resumeCountdownTimer, resumeStopwatchTimer } from "./lib/timerTransitions";
import type { AppState, CalendarEntry, Course, Exam, PlayedBreak, Priority, Semester, SocialAvatar, SocialAvatarStyle, SocialFeedComment, SocialFeedPost, SocialFeedScope, SocialFriend, SocialLeaderboardEntry, SocialLeaderboardPeriod, SocialLeaderboardScope, SocialSquadDetails, SocialSquadRole, SocialSquadScoreEntry, SocialSquadScorePeriod, SocialState, SocialSubtab, StudySession, TabKey, Task, TimerState } from "./types";
import type { Card, DurakGameState } from "./lib/durak";
import { canBeat, findDailyPuzzle, executePlayerAttack, executePlayerThrow, defendOneCard, playerPassThrow, playerPickUp, processCpuTurn, executeSlide, getAttackLimitAgainstCpu, getLegalSlideCards, gameStateToPuzzle, puzzleToGameState, SUIT_SYMBOL, SUIT_COLOR } from "./lib/durak";
import { filterFlaggleCountries, findFlaggleCountry, FLAGGLE_COUNTRY_COUNT, FLAGGLE_MAX_GUESSES, getFlaggleAnswerForDate, getFlagglePuzzleId, getFlagImageSrc, makeFlaggleSeedSalt, maskFlagByTargetColors, revealTargetFlagByGuesses } from "./lib/flaggle";
import { filterCountries, findCountryByName, GEODLE_COUNTRY_COUNT, GEODLE_MAX_GUESSES, getGeodleAnswerForDate, getGeodlePuzzleId, makeGeodleSeedSalt, scoreGeodleGuess } from "./lib/geodle";
import { TRAVLE_MAP_COUNTRIES } from "./lib/travleMapData";
import { filterTravleCountries, findTravleCountry, getTravleDisplayPath, getTravleGuessStates, getTravlePuzzleForDate, getTravlePuzzleId, getTravleShortestPath, getTravleSolvedPath, isTravleRouteSolved, makeTravleSeedSalt, TRAVLE_COUNTRY_COUNT, TRAVLE_MAX_GUESSES } from "./lib/travle";
import { getWordleAnswerForDate, getWordleHardModeViolation, getWordleKeyboardState, getWordlePuzzleId, isAcceptedWordleGuess, makeWordleSeedSalt, normalizeWordleGuess, scoreWordleGuess, WORDLE_ACCEPTED_GUESS_COUNT, WORDLE_ANSWER_COUNT, WORDLE_MAX_GUESSES, WORDLE_WORD_LENGTH } from "./lib/wordle";
import privacyPolicyText from "../../PRIVACY.md?raw";

type PdfJsModule = typeof import("pdfjs-dist");

type SocialProfileTarget = Pick<SocialFriend, "userId" | "displayName" | "friendCode" | "avatar"> & { lastSeenAt?: string | null };
type ProfileBadge = {
  id: string;
  icon: string;
  name: string;
  how: string;
  earned: boolean;
  daily?: boolean;
  count?: number;
};

type ProfileBadgeSubgroup = {
  category: string;
  source: string;
  badges: ProfileBadge[];
};

type ProfileBadgeGroup = ProfileBadgeSubgroup & {
  subgroups?: ProfileBadgeSubgroup[];
};

type FeedCommentNotice = {
  postId: string;
  scope: SocialFeedScope;
  commenterName: string;
  body: string;
};

type EndlessInactivityPrompt = {
  promptedAt: string;
};

const bellSound = new Audio("/bell.mp3");
bellSound.preload = "auto";
let bellAudioContext: AudioContext | null = null;
let bellAudioBuffer: AudioBuffer | null = null;
let bellAudioPromise: Promise<AudioBuffer | null> | null = null;
let bellAudioContextFailure: string | null = null;
let bellAudioContextFailureStage: BellStage | null = null;
let bellAudioFailure: Extract<BellResult, { ok: false }> | null = null;

type BellMethod = "web-audio" | "html-audio" | "prepared";
type BellStage =
  | "audio-context-unavailable"
  | "audio-context-create-failed"
  | "fetch-failed"
  | "decode-failed"
  | "context-suspended"
  | "web-audio-play-failed"
  | "html-audio-play-failed"
  | "html-audio-error";
type BellResult = { ok: true; method: BellMethod } | { ok: false; stage: BellStage; method?: BellMethod; detail: string };
type BellBufferResult = { ok: true; context: AudioContext; buffer: AudioBuffer } | Extract<BellResult, { ok: false }>;

function bellErrorDetail(error: unknown) {
  if (error instanceof Error) return `${error.name}: ${error.message}`;
  if (typeof error === "string") return error;
  return "Unknown audio error";
}

function bellFailure(stage: BellStage, detail: string, method?: BellMethod): Extract<BellResult, { ok: false }> {
  return { ok: false, stage, method, detail };
}

function getBellMediaError() {
  const error = bellSound.error;
  if (!error) return null;
  const labels: Record<number, string> = {
    [MediaError.MEDIA_ERR_ABORTED]: "aborted",
    [MediaError.MEDIA_ERR_NETWORK]: "network",
    [MediaError.MEDIA_ERR_DECODE]: "decode",
    [MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED]: "source-not-supported",
  };
  return `${labels[error.code] ?? "unknown"} (${error.code})${error.message ? `: ${error.message}` : ""}`;
}

function getBellContext() {
  if (bellAudioContext) return bellAudioContext;
  if (bellAudioContextFailure) return null;

  const audioWindow = window as Window & typeof globalThis & { webkitAudioContext?: typeof AudioContext };
  const BellAudioContext = audioWindow.AudioContext ?? audioWindow.webkitAudioContext;
  if (!BellAudioContext) {
    bellAudioContextFailure = "This WebView does not expose AudioContext or webkitAudioContext.";
    bellAudioContextFailureStage = "audio-context-unavailable";
    return null;
  }

  try {
    bellAudioContext = new BellAudioContext();
    return bellAudioContext;
  } catch (error: unknown) {
    bellAudioContextFailure = bellErrorDetail(error);
    bellAudioContextFailureStage = "audio-context-create-failed";
    console.warn("Bell audio context could not be created.", error);
    return null;
  }
}

async function loadBellBuffer(): Promise<BellBufferResult> {
  const context = getBellContext();
  if (!context) {
    return bellFailure(
      bellAudioContextFailureStage ?? "audio-context-unavailable",
      bellAudioContextFailure ?? "This WebView does not expose a usable AudioContext.",
    );
  }
  if (bellAudioBuffer) return { ok: true, context, buffer: bellAudioBuffer };
  if (bellAudioPromise) {
    const buffer = await bellAudioPromise;
    return buffer ? { ok: true, context, buffer } : bellFailure("decode-failed", "The previous MP3 decode attempt failed.", "web-audio");
  }

  bellAudioPromise = (async () => {
    let audioData: ArrayBuffer;

    try {
      const response = await fetch("/bell.mp3");
      if (!response.ok) throw new Error(`Could not load bell.mp3: ${response.status}`);
      audioData = await response.arrayBuffer();
    } catch (error: unknown) {
      bellAudioFailure = bellFailure("fetch-failed", bellErrorDetail(error), "web-audio");
      console.warn("Bell sound could not be fetched.", { result: bellAudioFailure, error });
      bellAudioPromise = null;
      return null;
    }

    try {
      bellAudioBuffer = await context.decodeAudioData(audioData);
      bellAudioFailure = null;
      return bellAudioBuffer;
    } catch (error: unknown) {
      bellAudioFailure = bellFailure("decode-failed", bellErrorDetail(error), "web-audio");
      console.warn("Bell sound could not be decoded.", { result: bellAudioFailure, error });
      bellAudioPromise = null;
      return null;
    }
  })();

  const buffer = await bellAudioPromise;
  if (!buffer) return bellAudioFailure ?? bellFailure("decode-failed", "The bell MP3 could not be fetched or decoded by the Web Audio backend.", "web-audio");
  return { ok: true, context, buffer };
}

async function playBellSound(): Promise<BellResult> {
  const webAudio = await loadBellBuffer();
  if (webAudio.ok) {
    try {
      const { context, buffer } = webAudio;
      if (context.state === "suspended") await context.resume();
      if (context.state === "running") {
        const source = context.createBufferSource();
        source.buffer = buffer;
        source.connect(context.destination);
        source.start();
        return { ok: true, method: "web-audio" };
      }
      console.warn("Bell Web Audio context is not running.", context.state);
    } catch (error: unknown) {
      console.warn("Bell sound failed through Web Audio.", error);
      return bellFailure("web-audio-play-failed", bellErrorDetail(error), "web-audio");
    }
  } else {
    console.warn("Bell Web Audio unavailable.", webAudio);
  }

  bellSound.currentTime = 0;
  try {
    await bellSound.play();
    return { ok: true, method: "html-audio" };
  } catch (error: unknown) {
    const mediaError = getBellMediaError();
    const result = bellFailure(
      mediaError ? "html-audio-error" : "html-audio-play-failed",
      mediaError ?? bellErrorDetail(error),
      "html-audio",
    );
    console.warn("Bell sound failed to play.", { result, error });
    return result;
  }
}

async function prepareBellSound(): Promise<BellResult> {
  bellSound.load();
  const context = getBellContext();
  if (!context) {
    return bellFailure(
      bellAudioContextFailureStage ?? "audio-context-unavailable",
      bellAudioContextFailure ?? "This WebView does not expose a usable AudioContext.",
    );
  }

  try {
    if (context.state === "suspended") await context.resume();
    if (context.state === "suspended") return bellFailure("context-suspended", "AudioContext stayed suspended after resume().", "web-audio");
    const buffer = await loadBellBuffer();
    if (!buffer.ok) return buffer;
    return { ok: true, method: "prepared" };
  } catch (error: unknown) {
    console.warn("Bell sound could not be prepared.", error);
    return bellFailure("context-suspended", bellErrorDetail(error), "web-audio");
  }
}

async function sendTimerNotification(title: string, body: string) {
  if (!isTauriApp()) return;

  try {
    const permissionGranted = await ensureTimerNotificationPermission();
    if (permissionGranted) sendNotification({ title, body });
  } catch (error: unknown) {
    console.warn("Timer notification could not be sent.", error);
  }
}

async function ensureTimerNotificationPermission() {
  if (!isTauriApp()) return false;
  let permissionGranted = await isPermissionGranted();
  if (!permissionGranted) {
    permissionGranted = await requestPermission() === "granted";
  }
  return permissionGranted;
}

function timerBrandPhase(timer: TimerState) {
  if (timer.phase === "stopwatch") return "study";
  if (timer.phase === "study" || timer.phase === "break" || timer.phase === "exam") return timer.phase;
  return "idle";
}

function TimerBrandMark({ phase }: { phase: ReturnType<typeof timerBrandPhase> }) {
  return (
    <div className={`brand-mark brand-mark--${phase}`} aria-hidden="true">
      {phase === "study" ? (
        <svg className="brand-mark-symbol brand-mark-symbol--snake" viewBox="0 0 32 32" fill="none">
          <path d="M22.7 7.8C19.5 5.8 13.3 5.7 10.4 8.9C7.3 12.3 10.5 15.3 15.7 16.5C21.6 17.9 23.5 21.5 19.5 24.2C16.4 26.3 10.9 25.5 8.9 22.2" />
          <circle cx="23.4" cy="7.6" r="1.15" />
        </svg>
      ) : phase === "break" ? (
        <svg className="brand-mark-symbol brand-mark-symbol--break" viewBox="0 0 32 32" fill="none">
          <path d="M11 8.5v15" />
          <path d="M21 8.5v15" />
        </svg>
      ) : phase === "exam" ? (
        <span className="brand-mark-letter">E</span>
      ) : (
        <svg className="brand-mark-symbol brand-mark-symbol--leaf" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
          <path d="M11 20A7 7 0 0 1 4 13c0-5 5-9 16-9 0 9-4 16-9 16z" />
          <path d="M11 20c0-5 2-9 7-13" />
        </svg>
      )}
    </div>
  );
}

const focusPresets = [
  { label: "Pomodoro 25/5", study: 25, breakMinutes: 5, mode: "focus" as const },
  { label: "Deep Work 52/17", study: 52, breakMinutes: 17, mode: "focus" as const },
  { label: "Sprint 90/20", study: 90, breakMinutes: 20, mode: "focus" as const },
  { label: "Exam", study: 120, breakMinutes: 0, mode: "exam" as const },
  { label: "∞ Endless", study: 0, breakMinutes: 0, mode: "endless" as const },
];

function formatCompletedUnits(units: number) {
  return Number.isInteger(units) ? String(units) : units.toFixed(2).replace(/0+$/, "").replace(/\.$/, "");
}

function formatCountdown(milliseconds: number) {
  const totalSeconds = Math.max(0, Math.ceil(milliseconds / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

const studyBreakGames = [
  { name: "Daily Durak", url: "", desc: "Solve today's Durak endgame puzzle" },
  { name: "Wordle", url: "https://www.nytimes.com/games/wordle", desc: "Guess the 5-letter word in 6 tries" },
  { name: "Travle", url: "", desc: "Build a border route between countries" },
  { name: "Flaggle", url: "", desc: "Guess the flag from shared colors" },
  { name: "Strands", url: "https://www.nytimes.com/games/strands", desc: "Find hidden words in a grid" },
  { name: "Geodle", url: "", desc: "Guess the country from geography clues" },
];
const STUDY_BREAK_GAME_COUNT = studyBreakGames.length;

const breakQuotes = [
  { text: "Almost everything will work again if you unplug it for a few minutes, including you.", author: "Anne Lamott" },
  { text: "Rest is not idleness, and to lie sometimes on the grass under trees on a summer's day, listening to the murmur of the water, or watching the clouds float across the sky, is by no means a waste of time.", author: "John Lubbock" },
  { text: "The time to relax is when you don't have time for it.", author: "Sydney J. Harris" },
  { text: "Sometimes the most productive thing you can do is relax.", author: "Mark Black" },
  { text: "Take rest; a field that has rested gives a bountiful crop.", author: "Ovid" },
  { text: "Tension is who you think you should be. Relaxation is who you are.", author: "Chinese Proverb" },
  { text: "Sleep is the best meditation.", author: "Dalai Lama" },
  { text: "If you can't change your fate, change your attitude.", author: "Amy Tan" },
  { text: "The mind is like a parachute — it works best when it's open.", author: "Frank Zappa" },
  { text: "What seems to us as bitter trials are often blessings in disguise.", author: "Oscar Wilde" },
  { text: "You should sit in nature for 20 minutes a day. Unless you're busy, then you should sit for an hour.", author: "Zen Proverb" },
  { text: "In the midst of movement and chaos, keep stillness inside of you.", author: "Deepak Chopra" },
  { text: "The greatest weapon against stress is our ability to choose one thought over another.", author: "William James" },
  { text: "A calm mind brings inner strength and self-confidence, so that's very important for good health.", author: "Dalai Lama" },
  { text: "Time you enjoy wasting was not wasted.", author: "John Lennon" },
  { text: "Give yourself a break. Stop and rest. Your mind will thank you.", author: "Unknown" },
  { text: "Do not confuse motion and progress. A rocking horse keeps moving but does not make any progress.", author: "Alfred A. Montapert" },
  { text: "The tree that is unbending is easily broken.", author: "Lao Tzu" },
  { text: "There is virtue in work and there is virtue in rest. Use both and overlook neither.", author: "Alan Cohen" },
  { text: "Relaxation is a form of meditation, and in the quiet of the mind, we find the solution to our problems.", author: "Unknown" },
  { text: "The soul always knows what to do to heal itself. The challenge is to silence the mind.", author: "Caroline Myss" },
  { text: "Stress is caused by being 'here' but wanting to be 'there.'", author: "Eckhart Tolle" },
  { text: "Now and then it's good to pause in our pursuit of happiness and just be happy.", author: "Guillaume Apollinaire" },
  { text: "Rest when you're weary. Refresh and renew yourself, your body, your mind, your spirit. Then get back to work.", author: "Ralph Marston" },
  { text: "It is not a luxury to rest. It is a necessity.", author: "Unknown" },
  { text: "Within you, there is a stillness and a sanctuary to which you can retreat at any time.", author: "Hermann Hesse" },
  { text: "Almost everything comes from nothing.", author: "Henri-Frédéric Amiel" },
  { text: "There is a certain kind of peace that comes from stepping away from the noise.", author: "Unknown" },
  { text: "If you get tired, learn to rest, not to quit.", author: "Banksy" },
  { text: "For fast-acting relief, try slowing down.", author: "Lily Tomlin" },
  { text: "The mind is not a vessel to be filled, but a fire to be kindled.", author: "Plutarch" },
  { text: "Rest and self-care are so important. When you take time to replenish your spirit, it allows you to serve others from the overflow.", author: "Eleanor Brown" },
  { text: "Breathe. Let go. And remind yourself that this very moment is the only one you know you have for sure.", author: "Oprah Winfrey" },
  { text: "Nothing can bring you peace but yourself.", author: "Ralph Waldo Emerson" },
  { text: "In the pause between thoughts, there is peace.", author: "Unknown" },
  { text: "The more you are motivated by love, the more fearless and free your action will be.", author: "Dalai Lama" },
  { text: "A step backward after making a wrong turn is a step in the right direction.", author: "Kurt Vonnegut" },
  { text: "Your mind will answer most questions if you learn to relax and wait for the answer.", author: "William S. Burroughs" },
  { text: "Every now and then go away, have a little relaxation, for when you come back to your work your judgment will be surer.", author: "Leonardo da Vinci" },
  { text: "Peace is not the absence of noise, trouble, or hard work. It is to be in the midst of those things and still be calm in your heart.", author: "Unknown" },
  { text: "The ability to be in the present moment is a major component of mental wellness.", author: "Abraham Maslow" },
  { text: "Slowing down is not a luxury, it's a necessity for a well-lived life.", author: "Unknown" },
  { text: "You don't have to be positive all the time. It's perfectly okay to feel sad, angry, annoyed, frustrated, scared, or anxious. Having feelings doesn't make you a negative person. It makes you human.", author: "Lori Deschene" },
  { text: "The best way to capture moments is to pay attention. This is how we cultivate mindfulness.", author: "Jon Kabat-Zinn" },
  { text: "Don't underestimate the value of doing nothing.", author: "A. A. Milne" },
  { text: "The pause between notes is what makes the music.", author: "Unknown" },
  { text: "Sometimes you need to step outside, get some air, and remind yourself of who you are and who you want to be.", author: "Unknown" },
  { text: "What lies behind us and what lies before us are tiny matters compared to what lies within us.", author: "Ralph Waldo Emerson" },
  { text: "All of humanity's problems stem from man's inability to sit quietly in a room alone.", author: "Blaise Pascal" },
  { text: "In the middle of difficulty lies opportunity.", author: "Albert Einstein" },
  { text: "To sit with the stillness of your own heart is the greatest gift you can give yourself.", author: "Unknown" },
  { text: "Sometime the most urgent thing you can possibly do is take a complete rest.", author: "Ashleigh Brilliant" },
  { text: "When you recover or discover something that nourishes your soul and brings joy, care enough about yourself to make room for it.", author: "Jean Shinoda Bolen" },
  { text: "Take a deep breath. It's just a bad day, not a bad life.", author: "Unknown" },
  { text: "You can't pour from an empty cup. Take care of yourself first.", author: "Unknown" },
  { text: "Let yourself be drawn by the stronger pull of that which you truly love.", author: "Rumi" },
  { text: "Worrying is like paying a debt you don't owe.", author: "Mark Twain" },
  { text: "The quieter you become, the more you can hear.", author: "Ram Dass" },
  { text: "Between stimulus and response there is a space. In that space is our power to choose our response. In our response lies our growth and our freedom.", author: "Viktor Frankl" },
  { text: "Nature does not hurry, yet everything is accomplished.", author: "Lao Tzu" },
  { text: "Be gentle with yourself. You are a child of the universe, no less than the trees and the stars.", author: "Max Ehrmann" },
  { text: "Your calm mind is the ultimate weapon against your challenges.", author: "Bryant McGill" },
  { text: "The mind is like water. When it's turbulent, it's difficult to see. When it's calm, everything becomes clear.", author: "Prasad Mahes" },
  { text: "Burnout is nature's way of telling you you've been going through the motions when your soul has departed.", author: "Unknown" },
  { text: "If you want to be creative, you have to rest and let your mind wander.", author: "Unknown" },
  { text: "Some of the most beautiful things in the world come from silence.", author: "Unknown" },
  { text: "Step outside. The air is still there, waiting for you.", author: "Unknown" },
  { text: "There's no need to fix everything. Sometimes just breathing is enough.", author: "Unknown" },
  { text: "Pause and remember: you are exactly where you need to be.", author: "Unknown" },
  { text: "Joy is not in things; it is in us.", author: "Richard Wagner" },
  { text: "Think of all the beauty still left around you and be happy.", author: "Anne Frank" },
  { text: "The sun is a daily reminder that we too can rise again from the darkness, that we too can shine.", author: "S. Ajna" },
  { text: "Be where you are, not where you think you should be.", author: "Unknown" },
  { text: "The best time to plant a tree was 20 years ago. The second best time is now.", author: "Chinese Proverb" },
  { text: "A seed grows with no sound, but a tree falls with a huge noise. Destruction has noise, but creation is quiet.", author: "Unknown" },
  { text: "Stillness is not about focusing on nothingness; it's about creating a clear space in which you can think.", author: "Unknown" },
  { text: "Sometimes, to stay alive, you have to kill the version of yourself that's trying to do everything.", author: "Unknown" },
  { text: "Not all those who wander are lost.", author: "J. R. R. Tolkien" },
  { text: "The most precious gift we can offer others is our presence. When mindfulness embraces those we love, they will bloom like flowers.", author: "Thich Nhat Hanh" },
  { text: "If you're overwhelmed, start small. Take a breath. Drink water. The rest will follow.", author: "Unknown" },
  { text: "Don't let your mind bully your body into believing it must carry the burden of its worries.", author: "Astrid Alauda" },
  { text: "An empty lantern provides no light. Rest is not a waste of time. It is fuel for the soul.", author: "Unknown" },
  { text: "Every breath we take, every step we make, is filled with peace and joy.", author: "Thich Nhat Hanh" },
  { text: "You are allowed to be both a masterpiece and a work in progress, simultaneously.", author: "Sophia Bush" },
  { text: "The act of resting is the act of reclaiming yourself.", author: "Unknown" },
  { text: "Turn your face to the sun and the shadows fall behind you.", author: "Maori Proverb" },
  { text: "This moment is the only moment that exists. Be here now.", author: "Ram Dass" },
  { text: "It does not matter how slowly you go as long as you do not stop.", author: "Confucius" },
  { text: "You are not your productivity. Your worth is not measured by what you produce.", author: "Unknown" },
  { text: "The magic you are looking for is in the work you are avoiding.", author: "Unknown" },
  { text: "Nothing can dim the light that shines from within.", author: "Maya Angelou" },
  { text: "The body achieves what the mind believes.", author: "Napoleon Hill" },
  { text: "We are what we repeatedly do. Excellence, then, is not an act, but a habit.", author: "Aristotle" },
  { text: "Courage doesn't always roar. Sometimes courage is the little voice at the end of the day that says, 'I'll try again tomorrow.'", author: "Mary Anne Radmacher" },
  { text: "To keep a lamp burning, we have to keep putting oil in it.", author: "Mother Teresa" },
  { text: "Nearly every great discovery came from someone who was taking a break from what they were supposed to be doing.", author: "Unknown" },
  { text: "Life moves pretty fast. If you don't stop and look around once in a while, you could miss it.", author: "Ferris Bueller" },
  { text: "If you want to go fast, go alone. If you want to go far, go together.", author: "African Proverb" },
  { text: "In the middle of the ordinary, there is always something extraordinary.", author: "Unknown" },
  { text: "You have been criticizing yourself for years, and it hasn't worked. Try approving of yourself and see what happens.", author: "Louise Hay" },
];

const stretchIdeas = [
  "Look away from screen, focus 20 ft away for 20s",
  "Roll your shoulders back 5 times",
  "Stand up, reach arms to the ceiling, side bend",
  "Clasp hands behind back, open chest, hold 10s",
  "Neck tilt: ear to shoulder, hold 10s each side",
  "Wrist stretch: palm up, pull fingers back gently",
  "Seated spinal twist: look over shoulder, hold 8s each side",
  "March in place for 15 seconds",
  "Shake out your hands and arms for 10s",
  "Take 3 deep belly breaths, exhale slowly",
];

const swissGrades = [4.0, 4.25, 4.5, 4.75, 5.0, 5.25, 5.5, 5.75, 6.0];
const TOTAL_WORKLOAD_ID = "__total_workload__";
const DASHBOARD_LAYOUT_KEY = "study-tracker-dashboard-layout";
const CUSTOM_DASHBOARD_LAYOUT_KEY = "study-tracker-dashboard-custom-layout";
const PALETTE_STORAGE_KEY = "study-tracker-palette";
const APP_STYLE_STORAGE_KEY = "study-tracker-style";
const FIELD_DASHBOARD_LAYOUT_KEY = "study-tracker-field-dashboard-layout";
const WABI_CIRCLE_COMPETITIVE_KEY = "study-tracker-wabi-circle-competitive";
const GARDEN_VARIANT_KEY = "study-tracker-garden-variant";
const DISMISSED_ANNOUNCEMENTS_KEY = "study-tracker-dismissed-announcements";
const TELEMETRY_INSTALL_ID_KEY = "study-tracker-telemetry-install-id";
const SESSION_HISTORY_DAYS = 365;
const SESSION_HISTORY_MAX = 3000;
const ENDLESS_INACTIVITY_PROMPT_MS = 2 * 60 * 60 * 1000;
const ENDLESS_INACTIVITY_GRACE_MS = 60 * 60 * 1000;
const ANNOUNCEMENT_POLL_INTERVAL_MS = 2 * 60 * 1000;
const TELEMETRY_HEARTBEAT_INTERVAL_MS = 60 * 60 * 1000;
const AUTO_UPDATE_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;
const RELEASES_PAGE_URL = "https://github.com/damcha02/destudydracker/releases/latest";
const SOURCE_LINUX_UPDATE_COMMAND = "cd /path/to/destudydracker\ngit pull\ncd desktop\nnpm install\nnpm run tauri:build\n./src-tauri/target/release/app";
const DEV_UPDATE_COMMAND = "cd /path/to/destudydracker\ngit pull\ncd desktop\nnpm install\nnpm run tauri:dev";
const DEFAULT_UPDATE_INSTALL_SUPPORT: UpdateInstallSupport = {
  canAutoInstall: false,
  packageHint: "unknown",
  runtimeChannel: "unknown",
  message: "Checking which update method this install supports...",
};

type ThemePalette =
  | "default"
  | "original"
  | "midnight"
  | "paper"
  | "cyberpunk"
  | "retrowave"
  | "forest"
  | "ocean"
  | "ume"
  | "copper"
  | "organs"
  | "lavender"
  | "gpt"
  | "claude"
  | "cute";
type AppStyle = "modern" | "field-notebook" | "wabi-sabi";
type ThemePanelView = "themes" | "styles";
type FieldDashboardLayout = "quiet" | "full";
type FieldPlannerTaskMode = "view" | "edit" | "create";
type FieldPlannerDeleteTarget = { type: "semester" | "course"; id: string; name: string };
type DashboardLayout = "focus" | "cockpit" | "analyst" | "custom";
type DashboardWidgetId = "today" | "urgentTasks" | "weeklyFocus" | "courseRadar" | "examRunway" | "garden" | "stats";
type DashboardWidgetWidth = "full" | "half" | "third";

type DashboardWidgetLayout = {
  id: DashboardWidgetId;
  width: DashboardWidgetWidth;
};

type CalendarView = "month" | "week";
type MenuPanel = "theme" | "personal" | "settings" | "options" | null;
type VaultSpace = "daily" | "references" | "summaries";
type PageHelp = {
  title: string;
  purpose: string;
  steps: string[];
  tour: TourStep[];
};

type TourStep = {
  target: string;
  title: string;
  body: string;
  tab?: TabKey;
};

type HelpExample = {
  task: string;
  unitLabel: string;
  total: number;
  done: number;
  next: number;
  caption: string;
  result: string;
  heading: string;
  tone: "c1" | "c2" | "c3";
};

type MenuHelpItem = {
  title: string;
  body: string;
};

const primaryTabs: Array<{ id: TabKey; label: string }> = [
  { id: "dashboard", label: "Dashboard" },
  { id: "planner", label: "Planner" },
  { id: "timer", label: "Timer" },
  { id: "vault", label: "Vault" },
  { id: "break", label: "Break Room" },
  { id: "friends", label: "Social" },
];

const menuHelpItems: MenuHelpItem[] = [
  { title: "Backup JSON", body: "Downloads a copy of your local Study Tracker data. Use it before big changes or when moving data manually." },
  { title: "Change theme", body: "Opens color palettes and the light/dark mode selector. The small sun/moon button is the fastest light/dark toggle." },
  { title: "Personal", body: "Set your display name and daily focus goal. The goal is used by Dashboard progress and daily study context." },
  { title: "Options", body: "Control visual effects, social feed images and polls, the help/info buttons, telemetry, and which tabs appear in navigation." },
  { title: "Settings", body: "Check app version, updates, install options, source update commands, and the dangerous delete-all-data action." },
  { title: "Main tab bar", body: "The row below the header is primary navigation. It switches between Dashboard, Planner, Timer, Vault, Break Room, and Social." },
  { title: "Page help", body: "The tab-bar question mark opens help for the current page. Its tutorial walks through that page step by step." },
];

const TUTORIAL_SEMESTER_ID = "tutorial-semester";
const TUTORIAL_COURSE_ID = "tutorial-course";
const TUTORIAL_TASK_ID = "tutorial-task";
const TUTORIAL_SEMESTER_NAME = "Tutorial Semester";
const TUTORIAL_COURSE_NAME = "Tutorial Course";
const TUTORIAL_TASK_TITLE = "Watch tutorial lectures";

const pageHelp: Record<TabKey, PageHelp> = {
  dashboard: {
    title: "Dashboard",
    purpose: "Start here when you are not sure what to study. It turns all your tasks, sessions, deadlines, and exams into a short daily overview.",
    steps: [
      "Pick a layout that feels clear to you.",
      "Check Today's Focus and urgent tasks first.",
      "Send one task to the Timer instead of trying to fix everything at once.",
    ],
    tour: [
      { target: "dashboard-layout", title: "Choose a dashboard view", body: "Start by picking the layout that feels easiest. Focus is the simplest, Cockpit shows everything, Analyst is stats-heavy, and Custom lets you arrange widgets." },
      { target: "dashboard-today", title: "Read today's focus", body: "This is the first place to look each day. It combines your progress, due dates, and study history into a quick recommendation." },
      { target: "dashboard-urgent", title: "Check planned work", body: "This list shows what is scheduled for today. If you are overwhelmed, pick only one item here and move it to the Timer." },
      { target: "dashboard-focus-button", title: "Send work to Timer", body: "The Focus button takes a planned task and links it to the Timer, so your study session counts toward the right course and task." },
      { target: "dashboard-weekly", title: "Review study rhythm", body: "Weekly Focus shows whether you studied consistently. Use it as feedback, not as a guilt chart." },
      { target: "dashboard-garden", title: "Understand the garden", body: "The garden is a visual reward for recent focus. More consistent study makes it grow." },
      { target: "dashboard-course-radar", title: "Scan course health", body: "Course Radar helps you spot courses that are falling behind before they become emergencies." },
      { target: "dashboard-exam-runway", title: "Watch exam pressure", body: "Exam Runway shows upcoming exams and preparedness so you know when revision should override normal tasks." },
    ],
  },
  planner: {
    title: "Planner",
    purpose: "Use this page to create the structure of your semester: semesters, courses, tasks, exams, and calendar work blocks. Units are the countable pieces of a task, not minutes or hours. The unit label tells the app what to call each piece.",
    steps: [
      "Create one semester.",
      "Add courses inside that semester.",
      "Add tasks or exams inside each course.",
      "Set the unit label, like Lecture, Sheet, or Exam.",
      "Set total units to the number of pieces you need to finish, then mark units complete as you work.",
      "Schedule units on the calendar if you want a day-by-day plan.",
    ],
    tour: [
      { target: "planner-add-semester", title: "Normally, start here", body: "A real setup starts with + Add semester. For this tutorial, Study Tracker has already created a Tutorial Semester so you can see the full workflow immediately." },
      { target: "planner-tutorial-semester", title: "Tutorial Semester", body: "This temporary semester was created for the tutorial. Semesters are containers for courses, tasks, exams, and calendar planning." },
      { target: "planner-tutorial-semester-health", title: "Semester health", body: "The score summarizes pressure inside this semester: task progress, overdue work, and upcoming exams." },
      { target: "planner-add-course", title: "Add courses here", body: "Normally you add each real course from this button. The tutorial has already created Tutorial Course inside Tutorial Semester." },
      { target: "planner-tutorial-course", title: "Tutorial Course", body: "Courses group related tasks and sessions. They also give Timer and Dashboard enough context to show where your study time went." },
      { target: "planner-add-task", title: "Add tasks here", body: "Tasks are the actual pieces of work: lectures, sheets, readings, old exams, projects, or revision sets." },
      { target: "planner-tutorial-task", title: "Tutorial task", body: "This task is Watch tutorial lectures. It has 10 total units, 3 already completed, and the next unit is Lecture 4 of 10." },
      { target: "planner-tutorial-task-meta", title: "Understand units", body: "The row shows completed units, total units, the next unit, and due date. Units are countable pieces, not hours." },
      { target: "planner-tutorial-task-progress", title: "Adjust progress", body: "After finishing a lecture, sheet, or exam, use plus. Use minus if you marked one by mistake. During the tutorial these controls are locked until Finish." },
      { target: "planner-tutorial-task-focus", title: "Focus from Planner", body: "Focus sends this task to the Timer already linked, so your session saves under the right semester, course, and task." },
      { target: "timer-links", title: "Arrive in Timer linked", body: "After focusing a task, the Timer can show the linked semester, course, and task so the session saves to the right place.", tab: "timer" },
      { target: "timer-presets", title: "Choose session length", body: "Pick a preset or custom timer after the task is linked. The task context stays attached." , tab: "timer"},
      { target: "timer-actions", title: "Start and save", body: "Start begins the focus block. Save records the session and pushes progress into the rest of the app.", tab: "timer" },
      { target: "planner-calendar", title: "Schedule units", body: "Back in Planner, use the calendar to spread task units across days or time blocks instead of guessing when to study.", tab: "planner" },
      { target: "planner-tutorial-task-remove", title: "Remove tutorial task", body: "At the end, press Finish first. Then you can click this Remove button yourself to delete the tutorial task." },
      { target: "planner-tutorial-course-remove", title: "Remove tutorial course", body: "After removing the task, remove Tutorial Course if you do not want to keep it." },
      { target: "planner-tutorial-semester-remove", title: "Remove tutorial semester", body: "Finally, remove Tutorial Semester. The tutorial cannot click this for you; you stay in control of deleting data." },
    ],
  },
  timer: {
    title: "Timer",
    purpose: "Use this page when you are ready to study. It tracks focus time and can link sessions back to courses and tasks.",
    steps: [
      "Choose a preset or Custom.",
      "Optionally link a semester, course, or task.",
      "Press Start. When done, press Save so the session counts in your stats.",
    ],
    tour: [
      { target: "timer-presets", title: "Pick a timer mode", body: "Presets are the fastest way to start. Use Pomodoro for short work, Deep Work for longer blocks, Exam for timed practice, and Endless when you do not know the duration." },
      { target: "timer-custom", title: "Custom timing", body: "Custom lets you choose your own focus and break minutes when the presets do not fit." },
      { target: "timer-links", title: "Link the session", body: "These fields connect the session to a semester, course, and task. Linked sessions make Dashboard, Planner, and Vault more useful." },
      { target: "timer-task-select", title: "Choose a task", body: "When a task is selected, the Timer shows the next unit, like Lecture 4 of 10, so you know exactly what to do." },
      { target: "timer-face", title: "Read the timer face", body: "The center shows remaining time, current mode, linked work, and logged minutes." },
      { target: "timer-start", title: "Start, pause, resume", body: "This button starts the block. During a session it becomes Pause, then Resume." },
      { target: "timer-save", title: "Save the session", body: "Save records the session. Without saving, the work will not appear in your history or stats." },
      { target: "timer-reset", title: "Reset when needed", body: "Reset clears the current timer if you started the wrong block or need to abandon it." },
      { target: "timer-advanced-toggle", title: "Optional session log", body: "Open this when you want to write what you learned, blockers, next steps, and confidence." },
      { target: "timer-goal", title: "Set the goal", body: "A good goal says what should exist when the timer ends, not just what subject you will study." },
      { target: "timer-learned", title: "Capture learning", body: "Write one sentence about what you learned. This can later become a Vault note." },
      { target: "timer-blocker", title: "Name the blocker", body: "If something is still weak, write it here so the next session knows what to fix." },
      { target: "timer-next-step", title: "Choose the next step", body: "This prevents the classic problem of ending a session and forgetting where to continue." },
      { target: "timer-confidence", title: "Rate confidence", body: "Confidence is a quick self-check. Low confidence means the task may need revision soon." },
      { target: "timer-session-log", title: "Recent sessions", body: "Saved sessions appear here and feed your Dashboard, statistics, and Vault draft context." },
    ],
  },
  vault: {
    title: "Vault",
    purpose: "Use this page for markdown notes that stay compatible with Obsidian or any normal text editor.",
    steps: [
      "Create or link a vault folder.",
      "Use Daily notes for session reflections.",
      "Use References and Summaries for course material.",
    ],
    tour: [
      { target: "vault-setup", title: "Open vault setup", body: "The Vault needs a folder before it can save notes. Setup is where you create or link that folder." },
      { target: "vault-name", title: "Vault name", body: "This is the display name for your markdown vault. It can match your Obsidian vault name." },
      { target: "vault-path", title: "Vault path", body: "This shows where notes will be written. If no path exists yet, create or link a vault." },
      { target: "vault-create", title: "Create vault", body: "Create new vault makes a fresh folder structure for Daily, References, and Summaries." },
      { target: "vault-link", title: "Link existing vault", body: "Use this if you already have an Obsidian vault or folder you want Study Tracker to write into." },
      { target: "vault-markdown", title: "Markdown cheatsheet", body: "Markdown help shows examples for headings, lists, links, quotes, and code blocks." },
      { target: "vault-spaces", title: "Switch vault spaces", body: "Daily is for study logs, References is for course notes, and Summaries is for PDFs or imported material." },
      { target: "vault-daily-date", title: "Choose daily note date", body: "Daily notes are date-based. Pick the day you want to load, edit, or save." },
      { target: "vault-load", title: "Load note", body: "Load reads the markdown file from disk if it already exists." },
      { target: "vault-session-draft", title: "Use session draft", body: "This pulls your recent timer reflection into the note so you do not start from a blank page." },
      { target: "vault-edit", title: "Edit note", body: "Edit opens the markdown editor. The preview lets you check how the note will read." },
      { target: "vault-editor", title: "Editor and preview", body: "Write markdown on one side and preview it on the other. Save when the note is ready." },
      { target: "vault-recent", title: "Recent exports", body: "Recently saved daily notes appear here so you can confirm what was written." },
    ],
  },
  friends: {
    title: "Social",
    purpose: "Use this page to share finished sessions, compete on leaderboards, manage friends, and participate in squads.",
    steps: [
      "Finish a timer session first.",
      "Share your latest session to the feed if you want accountability.",
      "Use Friends, Leaderboard, and Squad when you want social motivation.",
    ],
    tour: [
      { target: "social-nav", title: "Social sections", body: "The Social page is split into Feed, Leaderboard, Friends, Squad, and Profile. Each one supports a different motivation loop." },
      { target: "social-feed-scope", title: "Choose feed scope", body: "Friends Feed is quieter and more personal. Global Feed shows more activity." },
      { target: "social-refresh", title: "Refresh social data", body: "Refresh syncs feed posts, leaderboard data, friend requests, and squad updates." },
      { target: "social-stories", title: "Study circle", body: "Stories show your friends and recent activity so the page feels alive without opening every profile." },
      { target: "social-live", title: "Live activity", body: "This shows who was recently active. Use it as lightweight accountability." },
      { target: "social-composer", title: "Share latest session", body: "After saving a timer session, this composer lets you post it with a note, image, or poll." },
      { target: "social-note", title: "Write a short note", body: "One sentence is enough. You can also leave it blank and use the default post text." },
      { target: "social-image", title: "Attach image", body: "Use this when you want to share a screenshot, whiteboard, or study proof." },
      { target: "social-poll", title: "Create poll", body: "Polls are useful for quick questions, accountability checks, or deciding what to study next." },
      { target: "social-post", title: "Post session", body: "Post publishes the latest saved study session. Finish and save a Timer session first." },
      { target: "social-feed", title: "Read activity", body: "The feed shows sessions, milestones, comments, and reactions." },
      { target: "social-leaderboard-tab", title: "Leaderboard", body: "Use Leaderboard when competition helps you study. You can compare friends, squads, or global activity." },
      { target: "social-friends-tab", title: "Friends", body: "Use Friends to add people by code, accept requests, and build your study circle." },
      { target: "social-squad-tab", title: "Squad", body: "Squads are for group motivation and longer-term competition." },
      { target: "social-profile-tab", title: "Profile", body: "Profile controls how you appear to others, including avatar and badges." },
    ],
  },
  break: {
    title: "Break Room",
    purpose: "Use this page for intentional breaks. Study time unlocks small rewards so breaks feel earned instead of accidental.",
    steps: [
      "Study with the Timer to gain XP.",
      "Unlock or play a short break activity.",
      "Track water, stretches, and achievements as quick reset cues.",
    ],
    tour: [
      { target: "break-xp", title: "Break XP", body: "Study sessions fill this bar. The room rewards focus first, then breaks." },
      { target: "break-games", title: "Break activities", body: "Each card is a short break option. Locked cards show how much more study time is needed." },
      { target: "break-game-action", title: "Play or unlock", body: "Unlocked activities show Play. Locked activities show Unlock when you have earned enough XP." },
      { target: "break-stats", title: "Break stats", body: "These chips show how many activities are unlocked and how many you already played today." },
      { target: "break-water", title: "Water tracker", body: "Use this as a simple reset cue. One click adds a glass for today." },
      { target: "break-achievements", title: "Achievements", body: "Achievements reward good break habits and consistent use." },
      { target: "break-stretch", title: "Stretch prompt", body: "This gives you a quick physical reset idea. Refresh it if the current one does not fit." },
      { target: "break-rock", title: "Pet rock", body: "A tiny playful interaction. It is intentionally low-stakes: click it, smile, then return to focus." },
    ],
  },
};

const helpExamples: Record<TabKey, HelpExample[]> = {
  dashboard: [
    { task: "Pick today's focus", unitLabel: "Step", total: 4, done: 1, next: 2, caption: "Open Dashboard, then inspect the top recommendation", result: "Next: Check urgent tasks", heading: "Fast daily workflow", tone: "c1" },
    { task: "Read workload pressure", unitLabel: "Signal", total: 5, done: 3, next: 4, caption: "Health, urgent work, and exams tell you where pressure is building", result: "Next: Choose one task", heading: "Focus signals", tone: "c2" },
    { task: "Review weekly rhythm", unitLabel: "Day", total: 7, done: 4, next: 5, caption: "Use the focus chart to see consistency without overthinking one bad day", result: "Next: Fill the next gap", heading: "Weekly check", tone: "c3" },
  ],
  planner: [
    { task: "Watch lectures", unitLabel: "Lecture", total: 10, done: 3, next: 4, caption: "3 down, the glowing one is up next", result: "Next: Lecture 4 of 10", heading: "Three ways students use units", tone: "c1" },
    { task: "Exercise sheets", unitLabel: "Sheet", total: 14, done: 6, next: 7, caption: "Halfway through the pile", result: "Next: Sheet 7 of 14", heading: "Three ways students use units", tone: "c2" },
    { task: "Old exams", unitLabel: "Exam", total: 5, done: 1, next: 2, caption: "One solved, four to go", result: "Next: Exam 2 of 5", heading: "Three ways students use units", tone: "c3" },
  ],
  timer: [
    { task: "Start a focus block", unitLabel: "Step", total: 4, done: 2, next: 3, caption: "Choose a preset, link work if useful, then start", result: "Next: Start timer", heading: "Common timer flows", tone: "c1" },
    { task: "Link a task", unitLabel: "Field", total: 3, done: 1, next: 2, caption: "Semester, course, and task links make the session count in the right place", result: "Next: Pick course", heading: "Common timer flows", tone: "c2" },
    { task: "Save a session", unitLabel: "Step", total: 5, done: 4, next: 5, caption: "Saving turns your focus block into dashboard stats and vault context", result: "Next: Save session", heading: "Common timer flows", tone: "c3" },
  ],
  vault: [
    { task: "Create your vault", unitLabel: "Step", total: 3, done: 1, next: 2, caption: "Connect a folder once so notes save as normal markdown files", result: "Next: Link folder", heading: "Note-taking examples", tone: "c1" },
    { task: "Write a daily note", unitLabel: "Part", total: 4, done: 2, next: 3, caption: "Daily notes can collect what you learned, blockers, and next steps", result: "Next: Add reflection", heading: "Note-taking examples", tone: "c2" },
    { task: "Use markdown", unitLabel: "Format", total: 5, done: 3, next: 4, caption: "Headings, lists, links, and code blocks preview inside the app", result: "Next: Try a link", heading: "Note-taking examples", tone: "c3" },
  ],
  friends: [
    { task: "Share a session", unitLabel: "Step", total: 4, done: 2, next: 3, caption: "Finish a timer block, write one sentence, then post it", result: "Next: Add note", heading: "Social examples", tone: "c1" },
    { task: "Check accountability", unitLabel: "Space", total: 5, done: 1, next: 2, caption: "Feed, leaderboard, friends, squad, and profile each support a different kind of motivation", result: "Next: Open leaderboard", heading: "Social examples", tone: "c2" },
    { task: "Build your circle", unitLabel: "Step", total: 3, done: 1, next: 2, caption: "Use friend codes and squads when you want focused competition", result: "Next: Add friend", heading: "Social examples", tone: "c3" },
  ],
  break: [
    { task: "Earn a break", unitLabel: "Step", total: 4, done: 2, next: 3, caption: "Study time fills XP so breaks feel intentional, not accidental", result: "Next: Unlock activity", heading: "Break examples", tone: "c1" },
    { task: "Reset properly", unitLabel: "Cue", total: 5, done: 3, next: 4, caption: "Use water, stretch prompts, and short games as a real reset", result: "Next: Drink water", heading: "Break examples", tone: "c2" },
    { task: "Return to focus", unitLabel: "Step", total: 3, done: 2, next: 3, caption: "A good break ends by sending you back to the next focus block", result: "Next: Start timer", heading: "Break examples", tone: "c3" },
  ],
};

const vaultSpaces: Array<{ id: VaultSpace; label: string }> = [
  { id: "references", label: "References" },
  { id: "summaries", label: "Summaries" },
  { id: "daily", label: "Daily" },
];

const socialSubtabs: Array<{ id: SocialSubtab; label: string; badge?: string }> = [
  { id: "feed", label: "Feed" },
  { id: "leaderboard", label: "Leaderboard" },
  { id: "friends", label: "Friends" },
  { id: "squad", label: "Squad" },
  { id: "profile", label: "Profile" },
];

const LANDING_PAGE_URL = "https://damcha02.github.io/destudydracker/";

function makeFriendInviteLink(friendCode: string) {
  return `${LANDING_PAGE_URL}?invite=${encodeURIComponent(friendCode)}`;
}

const squadRoleLabels: Record<SocialSquadRole, string> = {
  leader: "Leader",
  co_leader: "Co-leader",
  elder: "Elder",
  member: "Member",
};

const squadRoles: SocialSquadRole[] = ["leader", "co_leader", "elder", "member"];
const SQUAD_TRACKING_START_LABEL = "29.07.2026";
const SQUAD_SEASON_NAME = "Summer Season 26";
const SQUAD_SEASON_RANGE_LABEL = "29.07.2026 - 31.08.2026";

function squadRoleRank(role: SocialSquadRole) {
  return role === "leader" ? 4 : role === "co_leader" ? 3 : role === "elder" ? 2 : 1;
}

function canManageSquadRequests(role?: SocialSquadRole) {
  return Boolean(role && squadRoleRank(role) > squadRoleRank("member"));
}

function canEditSquadMember(actorRole: SocialSquadRole, targetRole: SocialSquadRole) {
  if (actorRole === "leader") return targetRole !== "leader";
  if (actorRole === "co_leader") return squadRoleRank(targetRole) < squadRoleRank("co_leader");
  return false;
}

function canKickSquadMember(actorRole: SocialSquadRole, targetRole: SocialSquadRole) {
  if (actorRole === "leader") return targetRole !== "leader";
  if (actorRole === "co_leader") return squadRoleRank(targetRole) < squadRoleRank("co_leader");
  if (actorRole === "elder") return targetRole === "member";
  return false;
}

function getAssignableSquadRoles(actorRole: SocialSquadRole, targetRole: SocialSquadRole) {
  if (!canEditSquadMember(actorRole, targetRole)) return [];
  return squadRoles.filter((role) => role !== "leader" && (actorRole === "leader" || squadRoleRank(role) < squadRoleRank("co_leader")));
}

function pickSquadSuggestions(squads: SquadSearchResult[]) {
  const pool = [...squads];
  for (let i = pool.length - 1; i > 0; i -= 1) {
    const j = Math.floor(Math.random() * (i + 1));
    [pool[i], pool[j]] = [pool[j], pool[i]];
  }
  return pool.slice(0, 4);
}

const feedFallbackNotes = [
  "only 5 billion things to go...",
  "keeping up with the deadline",
  "not wasting time",
  "deleted instagram",
  "none of it is real...",
  "hustling",
  "grinding",
  "workaholic",
  "need a breather",
  "one more sesh",
  "fantasizing about my next break",
  "spending too much time in the breakroom",
  "cannot break into the vault",
  "slaying demons",
  "training dragons",
  "living in delusion",
  "code never sleeps",
  "brain.exe running",
  "fueled by caffeine",
  "in the zone",
  "closing tabs, opening minds",
  "debugging my life",
  "on the grindset",
  "minimum viable student",
  "late to the party, early to the library",
  "ctrl+s my sanity",
  "segfault in real life",
  "stack overflow of assignments",
];

const avatarStyles: Array<{ id: SocialAvatarStyle; label: string }> = [
  { id: "classic", label: "Classic" },
  { id: "serif", label: "Serif" },
  { id: "cursive", label: "Cursive" },
  { id: "graffiti", label: "Graffiti" },
  { id: "pixel", label: "Pixel" },
  { id: "mono", label: "Mono" },
];
const avatarIcons = ["✦", "★", "◆", "☘", "☾", "☀", "♜", "♞", "⚡", "☕", "📚", "🧠", "🔥", "🌊", "🌿", "🪐", "🚀", "🎯", "🏆", "🛡", "🦉", "🐢", "🐺", "🐱", "🍄", "🌙", "🌸", "🍀", "💎", "🎲", "🎧", "📝", "🔮", "🧩", "🕹", "📖", "🧪", "🛰", "🌌", "🦊"];
const alphabetLetters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ".split("");

type UpdateInfo = {
  status: "idle" | "available" | "current" | "installing" | "error";
  latestVersion?: string;
  releaseUrl?: string;
  message: string;
};

type UpdateInstallSupport = {
  canAutoInstall: boolean;
  packageHint: string;
  runtimeChannel: string;
  message: string;
};

type LinuxUpdateDownload = {
  version: string;
  packageType: string;
  filePath: string;
  installCommand: string;
  message: string;
};

type CheckForUpdatesOptions = {
  silent?: boolean;
  automatic?: boolean;
};

function loadDismissedAnnouncementIds() {
  try {
    const parsed = JSON.parse(localStorage.getItem(DISMISSED_ANNOUNCEMENTS_KEY) ?? "[]");
    return new Set(Array.isArray(parsed) ? parsed.filter((id): id is string => typeof id === "string") : []);
  } catch {
    return new Set<string>();
  }
}

function saveDismissedAnnouncementIds(ids: Set<string>) {
  localStorage.setItem(DISMISSED_ANNOUNCEMENTS_KEY, JSON.stringify([...ids].slice(-100)));
}

function getTelemetryInstallId() {
  const existing = localStorage.getItem(TELEMETRY_INSTALL_ID_KEY);
  if (existing) return existing;
  const next = crypto.randomUUID();
  localStorage.setItem(TELEMETRY_INSTALL_ID_KEY, next);
  return next;
}

function isValidAppVersion(version: string) {
  return /^\d+(?:\.\d+)*$/.test(version.trim());
}

type CalendarDay = {
  date: Date;
  iso: string;
  inCurrentMonth: boolean;
};

type CalendarUnitAmount = CalendarEntry["unitAmount"];
type CalendarTaskSource = "planner" | "new";
type CalendarAddDraft = {
  semesterId: string;
  courseId: string;
  source: CalendarTaskSource;
  taskId: string;
  title: string;
  unitAmount: CalendarUnitAmount;
  startTime: string;
  durationMinutes: string;
  noTime: boolean;
};
type CalendarEditDraft = {
  startTime: string;
  endTime: string;
};
type CalendarResizeState = {
  entryId: string;
  startY: number;
  startMinutes: number;
  endMinutes: number;
};
type CalendarMoveDragState = {
  entryId: string;
  durationMinutes: number;
  startX: number;
  startY: number;
  moved: boolean;
};
type CalendarMovePreview = {
  entryId: string;
  x: number;
  y: number;
  top: number;
  time: string;
};

type GardenPlantKind = "grass" | "sprout" | "leafy" | "flower" | "herb" | "mushroom" | "bush" | "fern" | "glow" | "tree" | "vine" | "crystal";

type GardenItem = {
  id: string;
  kind: GardenPlantKind;
  color?: string;
  maturity: number;
  label?: string;
  sub?: string;
  ambient?: boolean;
  fresh?: boolean;
  fixed?: boolean;
  wise?: boolean;
};

type PlacedGardenItem = GardenItem & { x: number; t: number };

type GardenSpeciesProps = {
  rng: () => number;
  color?: string;
  maturity?: number;
  wise?: boolean;
  detail?: number;
};

type JapaneseGardenPlantKind =
  | "shoot"
  | "susuki"
  | "moss"
  | "fern"
  | "bamboo"
  | "momiji"
  | "sakura"
  | "pine"
  | "azalea"
  | "iris"
  | "hagi"
  | "stone"
  | "teahouse"
  | "lantern"
  | "kiku";

type JapaneseGardenItem = {
  id: string;
  kind: JapaneseGardenPlantKind;
  color?: string;
  maturity: number;
  label?: string;
  sub?: string;
  ambient?: boolean;
  fresh?: boolean;
  wise?: boolean;
  anchor?: [number, number];
  minutes?: number;
};

type JapaneseGardenBuildResult = {
  heroes: JapaneseGardenItem[];
  features: JapaneseGardenItem[];
  sessions: JapaneseGardenItem[];
  milestones: JapaneseGardenItem[];
};

type JapaneseGardenCell = { x: number; t: number; cw: number; low: boolean; key: string };

type PlacedJapaneseGardenItem = JapaneseGardenItem & { x: number; t: number; cw: number; low?: boolean; growth: number };

const calendarTimelineHours = Array.from({ length: 18 }, (_, index) => index + 6);
const calendarDurationOptions = [15, 30, 45, 60, 90, 120, 180];
const calendarTimeStepMinutes = 5;
const calendarTimelineStartHour = 6;
const calendarTimelineHourHeight = 72;
const calendarTimelineStartMinutes = calendarTimelineStartHour * 60;
const calendarTimelineEndMinutes = 24 * 60;
const calendarTimelineTotalMinutes = calendarTimelineEndMinutes - calendarTimelineStartMinutes;

function gardenHash(value: string) {
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function gardenRng(seed: number) {
  let state = seed | 0;
  return function next() {
    state = (state + 0x6D2B79F5) | 0;
    let t = Math.imul(state ^ (state >>> 15), 1 | state);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function gardenGreen(rng: () => number) {
  return `color-mix(in oklab, var(--garden), var(--garden-deep) ${Math.round(15 + rng() * 65)}%)`;
}

function gardenMix(a: string, b: string, pct: number) {
  return `color-mix(in oklab, ${a}, ${b} ${pct}%)`;
}

function gardenLeaf(x: number, y: number, len: number, dir: number, lift = 0.7) {
  const tx = x + dir * len;
  const ty = y - len * lift;
  return `M${x} ${y} Q ${x + dir * len * 0.15} ${y - len * 0.8} ${tx} ${ty} Q ${x + dir * len * 0.72} ${y - len * 0.12} ${x} ${y} Z`;
}

const JG_LEAF = "oklch(0.60 0.10 148)";
const JG_LEAF_DEEP = "oklch(0.42 0.075 158)";
const JG_SOIL = "oklch(0.48 0.045 60)";
const JG_GOLD = "oklch(0.80 0.13 85)";
const JG_MOSS = "oklch(0.52 0.07 145)";
const JG_PINE = "oklch(0.42 0.055 155)";
const JG_MAPLE = "oklch(0.58 0.14 32)";
const JG_MAPLE2 = "oklch(0.66 0.13 55)";
const JG_SAKURA = "oklch(0.86 0.055 350)";
const JG_BAMBOO = "oklch(0.72 0.09 120)";
const JG_STONE = "oklch(0.62 0.012 250)";
const JG_WATER = "oklch(0.44 0.045 225)";

function jgGreen(rng: () => number) {
  return gardenMix(JG_LEAF, JG_LEAF_DEEP, Math.round(20 + rng() * 60));
}

/* 5-lobe maple leaf, drawn around 0,0 then placed by transform */
function gardenMapleLeaf(cx: number, cy: number, r: number, rot: number, fill: string, key: string | number, opacity?: number) {
  const p = `M0 ${-r} Q ${r * 0.34} ${-r * 0.62} ${r * 0.34} ${-r * 0.34} Q ${r * 0.74} ${-r * 0.6} ${r * 0.86} ${-r * 0.38} Q ${r * 0.66} ${-r * 0.06} ${r * 0.46} ${r * 0.14} Q ${r * 0.74} ${r * 0.5} ${r * 0.56} ${r * 0.66} Q ${r * 0.22} ${r * 0.42} 0 ${r * 0.36} Q ${-r * 0.22} ${r * 0.42} ${-r * 0.56} ${r * 0.66} Q ${-r * 0.74} ${r * 0.5} ${-r * 0.46} ${r * 0.14} Q ${-r * 0.66} ${-r * 0.06} ${-r * 0.86} ${-r * 0.38} Q ${-r * 0.74} ${-r * 0.6} ${-r * 0.34} ${-r * 0.34} Q ${-r * 0.34} ${-r * 0.62} 0 ${-r} Z`;
  return <path key={key} d={p} fill={fill} opacity={opacity == null ? 0.96 : opacity} transform={`translate(${cx} ${cy}) rotate(${rot})`} />;
}

/* one 5-petal blossom */
function gardenBlossom(cx: number, cy: number, r: number, pink: string, rot: number, key: string | number) {
  return (
    <g key={key}>
      {[0, 1, 2, 3, 4].map((index) => (
        <ellipse key={index} cx={cx} cy={cy - r * 0.62} rx={r * 0.44} ry={r * 0.62} fill={pink} transform={`rotate(${rot + index * 72} ${cx} ${cy})`} />
      ))}
      <circle cx={cx} cy={cy} r={r * 0.26} fill={gardenMix(JG_GOLD, pink, 35)} />
    </g>
  );
}

/* lancet leaf (bamboo, iris blade, susuki, fern) */
function gardenBlade(x: number, y: number, len: number, dir: number, droop: number, w = 0.22) {
  const tx = x + dir * len;
  const ty = y + droop;
  return `M${x} ${y} Q ${x + dir * len * 0.5} ${y - len * w} ${tx} ${ty} Q ${x + dir * len * 0.5} ${y + len * w * 0.6} ${x} ${y} Z`;
}

function GardenGrass({ rng }: GardenSpeciesProps) {
  const count = 4 + Math.floor(rng() * 3);
  return <g>{Array.from({ length: count }).map((_, index) => {
    const dx = (rng() - 0.5) * 26;
    const h = 15 + rng() * 22;
    return <path key={index} d={`M24 58 Q ${24 + dx * 0.25} ${58 - h * 0.55} ${24 + dx} ${58 - h}`} stroke={gardenGreen(rng)} strokeWidth={1.4 + rng()} fill="none" strokeLinecap="round" opacity={0.7 + rng() * 0.3} />;
  })}</g>;
}

function GardenSprout({ rng }: GardenSpeciesProps) {
  const green = gardenGreen(rng);
  return <g><path d="M24 58 Q 24.5 51 24 46" stroke="var(--garden-deep)" strokeWidth="1.8" fill="none" strokeLinecap="round" /><path d={gardenLeaf(24, 47, 8 + rng() * 3, -1)} fill={green} /><path d={gardenLeaf(24, 47, 7 + rng() * 3, 1)} fill={gardenMix(green, "var(--garden)", 40)} /></g>;
}

function GardenLeafy({ rng, color = "var(--accent)", maturity = 0.5 }: GardenSpeciesProps) {
  const h = 24 + rng() * 16 + maturity * 8;
  const bend = (rng() - 0.5) * 9;
  const topX = 24 + bend * 0.7;
  const topY = 58 - h;
  const leaves = 3 + Math.floor(rng() * 3);
  return <g><path d={`M24 58 Q ${24 + bend} ${58 - h * 0.55} ${topX} ${topY}`} stroke="var(--garden-deep)" strokeWidth="1.9" fill="none" strokeLinecap="round" />{Array.from({ length: leaves }).map((_, index) => {
    const f = 0.28 + 0.62 * (index / leaves);
    const lx = 24 + bend * f;
    const ly = 58 - h * f;
    return <path key={index} d={gardenLeaf(lx, ly, 6.5 + rng() * 5 + maturity * 2, index % 2 === 0 ? -1 : 1)} fill={gardenGreen(rng)} />;
  })}{maturity > 0.6 ? <circle cx={topX} cy={topY - 1.5} r="3" fill={color} opacity="0.95" /> : null}</g>;
}

function GardenFlower({ rng, color = "var(--accent)" }: GardenSpeciesProps) {
  const h = 22 + rng() * 14;
  const bend = (rng() - 0.5) * 7;
  const cx = 24 + bend * 0.7;
  const cy = 58 - h;
  const rot = rng() * 60;
  const petals = 5 + Math.floor(rng() * 2);
  return <g><path d={`M24 58 Q ${24 + bend} ${58 - h * 0.55} ${cx} ${cy}`} stroke="var(--garden-deep)" strokeWidth="1.7" fill="none" strokeLinecap="round" /><path d={gardenLeaf(24 + bend * 0.4, 58 - h * 0.42, 7 + rng() * 3, rng() > 0.5 ? 1 : -1)} fill={gardenGreen(rng)} />{Array.from({ length: petals }).map((_, index) => <ellipse key={index} cx={cx} cy={cy - 4.4} rx="2.9" ry="4.8" fill={color} opacity="0.92" transform={`rotate(${rot + index * (360 / petals)} ${cx} ${cy})`} />)}<circle cx={cx} cy={cy} r="2.5" fill={gardenMix(color, "white", 45)} /></g>;
}

function GardenHerb({ rng, color = "var(--accent)" }: GardenSpeciesProps) {
  const stems = 3 + Math.floor(rng() * 2);
  return <g>{Array.from({ length: stems }).map((_, index) => {
    const dx = (index - (stems - 1) / 2) * (5 + rng() * 3);
    const h = 20 + rng() * 14;
    const green = gardenGreen(rng);
    return <g key={index}><path d={`M24 58 Q ${24 + dx * 0.5} ${58 - h * 0.5} ${24 + dx} ${58 - h}`} stroke={green} strokeWidth="1.5" fill="none" strokeLinecap="round" />{[0.72, 0.86, 1].map((f, berryIndex) => <circle key={berryIndex} cx={24 + dx * f} cy={58 - h * f} r={1.7 - berryIndex * 0.25} fill={color} opacity="0.9" />)}</g>;
  })}</g>;
}

function GardenMushroom({ rng }: GardenSpeciesProps) {
  const capW = 10 + rng() * 4;
  const capH = 9 + rng() * 4;
  const cap = "oklch(0.63 0.075 40)";
  return <g><rect x="21.6" y="45" width="4.8" height="13" rx="2.4" fill={gardenMix("var(--garden-soil)", "white", 42)} /><path d={`M${24 - capW} 46.5 Q 24 ${46.5 - capH * 2} ${24 + capW} 46.5 Z`} fill={cap} /><circle cx={24 - capW * 0.4} cy="42.5" r="1.2" fill={gardenMix(cap, "white", 50)} /><circle cx={24 + capW * 0.35} cy="40.5" r="0.95" fill={gardenMix(cap, "white", 50)} /></g>;
}

function GardenBush({ rng, color = "var(--accent)", maturity = 0.5 }: GardenSpeciesProps) {
  const berries = maturity > 0.55 ? 3 : 0;
  return <g><ellipse cx="15" cy="50" rx={8 + rng() * 3} ry={7 + rng() * 2} fill={gardenGreen(rng)} /><ellipse cx="30" cy="47" rx={10 + rng() * 3} ry={9 + rng() * 3} fill={gardenGreen(rng)} /><ellipse cx="24" cy="52" rx={9 + rng() * 2} ry={6 + rng() * 2} fill={gardenMix(gardenGreen(rng), "var(--garden)", 30)} />{Array.from({ length: berries }).map((_, index) => <circle key={index} cx={12 + rng() * 24} cy={42 + rng() * 10} r="1.7" fill={color} opacity="0.95" />)}</g>;
}

function GardenFern({ rng }: GardenSpeciesProps) {
  const h = 26 + rng() * 14;
  const curl = 3 + rng() * 4;
  const green = gardenGreen(rng);
  const fronds: ReactNode[] = [];
  for (let index = 0; index < 8; index += 1) {
    const f = 0.18 + 0.78 * (index / 8);
    const x = 24 + curl * Math.sin(f * 2.4);
    const y = 58 - h * f;
    const len = (1 - f) * 9 + 2;
    fronds.push(<path key={`l${index}`} d={`M${x} ${y} Q ${x - len * 0.7} ${y - len * 0.4} ${x - len} ${y - len * 0.9}`} stroke={green} strokeWidth="1.3" fill="none" strokeLinecap="round" />);
    fronds.push(<path key={`r${index}`} d={`M${x} ${y} Q ${x + len * 0.7} ${y - len * 0.4} ${x + len} ${y - len * 0.9}`} stroke={green} strokeWidth="1.3" fill="none" strokeLinecap="round" />);
  }
  return <g><path d={`M24 58 Q ${24 + curl * 2} ${58 - h * 0.5} 24 ${58 - h}`} stroke="var(--garden-deep)" strokeWidth="1.7" fill="none" strokeLinecap="round" />{fronds}</g>;
}

function GardenGlowFlower({ rng }: GardenSpeciesProps) {
  const cx = 24;
  const cy = 26;
  const rot = rng() * 60;
  return <g style={{ filter: "drop-shadow(0 0 5px var(--warn))" }}><circle cx={cx} cy={cy} r="11" fill="var(--warn)" opacity="0.13" /><path d={`M24 58 Q 26 42 ${cx} ${cy}`} stroke="var(--garden-deep)" strokeWidth="1.8" fill="none" strokeLinecap="round" /><path d={gardenLeaf(25, 46, 8, -1)} fill={gardenGreen(rng)} /><path d={gardenLeaf(25.2, 42, 7, 1)} fill={gardenGreen(rng)} />{Array.from({ length: 6 }).map((_, index) => <ellipse key={index} cx={cx} cy={cy - 5} rx="3.1" ry="5.4" fill="var(--warn)" opacity="0.95" transform={`rotate(${rot + index * 60} ${cx} ${cy})`} />)}<circle cx={cx} cy={cy} r="2.8" fill={gardenMix("var(--warn)", "white", 55)} /></g>;
}

function GardenTree({ rng, wise }: GardenSpeciesProps) {
  const trunk = gardenMix("var(--garden-soil)", "black", 22);
  return <g style={wise ? { filter: "drop-shadow(0 0 6px color-mix(in oklab, var(--warn), transparent 45%))" } : undefined}><path d="M24 58 C 22.5 48 25.5 42 24 32" stroke={trunk} strokeWidth="3.4" fill="none" strokeLinecap="round" /><path d="M24 44 Q 30 40 33 36" stroke={trunk} strokeWidth="1.8" fill="none" strokeLinecap="round" /><path d="M24 40 Q 18 36 15.5 33" stroke={trunk} strokeWidth="1.8" fill="none" strokeLinecap="round" /><circle cx="24" cy="21" r="12.5" fill={gardenGreen(rng)} /><circle cx="13.5" cy="28" r="8" fill={gardenGreen(rng)} /><circle cx="34.5" cy="27" r="8.5" fill={gardenGreen(rng)} /><circle cx="24" cy="30" r="9" fill={gardenMix(gardenGreen(rng), "var(--garden)", 35)} />{wise ? Array.from({ length: 5 }).map((_, index) => <circle key={index} cx={12 + rng() * 24} cy={16 + rng() * 14} r="1.25" fill="var(--warn)" />) : null}</g>;
}

function GardenVine({ rng, color = "var(--accent)" }: GardenSpeciesProps) {
  const sway = 4 + rng() * 3;
  const buds = [0.32, 0.52, 0.72, 0.9];
  const hues = [color, gardenGreen(rng), gardenMix(color, "var(--garden)", 45)];
  return <g><path d={`M24 58 Q ${24 + sway} 40 ${24 - sway * 0.6} 24 Q ${24 + sway * 0.4} 12 24 4`} stroke="var(--garden-deep)" strokeWidth="1.6" fill="none" strokeLinecap="round" />{buds.map((f, index) => {
    const x = 24 + Math.sin(f * 6) * sway * (1 - f * 0.3);
    const y = 58 - f * 54;
    const dir = index % 2 === 0 ? -1 : 1;
    return <g key={index}><path d={gardenLeaf(x, y, 6 + rng() * 3, dir)} fill={gardenGreen(rng)} /><circle cx={x + dir * 4} cy={y - 1.5} r="2.1" fill={hues[index % hues.length]} opacity="0.92" /></g>;
  })}</g>;
}

function GardenCrystal({ rng }: GardenSpeciesProps) {
  const cx = 24;
  const cy = 30;
  const h = 20 + rng() * 8;
  return <g style={{ filter: "drop-shadow(0 0 6px color-mix(in oklab, var(--accent), transparent 35%))" }}><path d={`M24 58 Q 25.5 46 24 ${cy + 10}`} stroke="var(--garden-deep)" strokeWidth="1.7" fill="none" strokeLinecap="round" />{Array.from({ length: 5 }).map((_, index) => {
    const fx = cx + (index - 2) * 3.4;
    const fy = cy + 6 - Math.abs(index - 2) * 2.2;
    const fh = h * (0.55 + (1 - Math.abs(index - 2) / 3) * 0.5);
    return <path key={index} d={`M${fx} ${fy} L ${fx - 3.1} ${fy - fh * 0.42} L ${fx} ${fy - fh} L ${fx + 3.1} ${fy - fh * 0.42} Z`} fill={gardenMix("var(--accent)", index % 2 === 0 ? "white" : "var(--garden)", 20 + rng() * 20)} opacity="0.9" stroke="color-mix(in oklab, var(--accent), white 40%)" strokeWidth="0.4" />;
  })}<circle cx={cx} cy={cy - h * 0.55} r="1.6" fill="white" opacity="0.85" /></g>;
}

const gardenSpecies: Record<GardenPlantKind, (props: GardenSpeciesProps) => ReactNode> = {
  grass: GardenGrass,
  sprout: GardenSprout,
  leafy: GardenLeafy,
  flower: GardenFlower,
  herb: GardenHerb,
  mushroom: GardenMushroom,
  bush: GardenBush,
  fern: GardenFern,
  glow: GardenGlowFlower,
  tree: GardenTree,
  vine: GardenVine,
  crystal: GardenCrystal,
};

const gardenBaseWidth: Record<GardenPlantKind, number> = { grass: 0.82, sprout: 0.52, leafy: 0.95, flower: 0.92, herb: 0.9, mushroom: 0.58, bush: 1.12, fern: 0.95, glow: 1.05, tree: 1.95, vine: 1.1, crystal: 0.85 };

function gardenPickSpecies(rng: () => number, maturity: number): GardenPlantKind {
  if (maturity < 0.35) return rng() < 0.5 ? "sprout" : "grass";
  if (maturity < 0.7) return ["leafy", "herb", "fern", "mushroom"][Math.floor(rng() * 4)] as GardenPlantKind;
  return ["leafy", "bush", "flower", "fern"][Math.floor(rng() * 4)] as GardenPlantKind;
}

function buildGardenItems(courses: Course[], sessions: StudySession[], tasks: Task[], stage: number, weeklyMinutes: number, densityFactor: number): GardenItem[] {
  const courseMap = new Map(courses.map((course) => [course.id, course]));
  const items: GardenItem[] = [];
  const streak = getStreakDays({ sessions } as AppState);
  const todayTime = new Date(`${isoDate()}T00:00:00`).getTime();

  if (stage >= 5 || streak >= 10) {
    items.push({ id: "gk-tree", kind: "tree", wise: true, maturity: 1, label: "The Wise Tree", sub: "A flourishing stretch of study", fixed: true });
  }

  if (streak >= 5) {
    items.push({ id: "gk-streak", kind: "glow", maturity: 1, label: `${streak}-day streak`, sub: "Rare bloom. Keep it alive." });
  }

  if (stage >= 4) {
    items.push({ id: "gk-full-bloom", kind: "crystal", maturity: 1, label: "Full Bloom", sub: "The garden is blooming brilliantly" });
  }

  const weekCutoff = todayTime - 6 * 86400000;
  const weeklyCourseIds = new Set(
    sessions
      .filter((session) => (session.kind === "study" || session.kind === "exam") && session.courseId)
      .filter((session) => new Date(`${(session.endedAt || session.startedAt).slice(0, 10)}T00:00:00`).getTime() >= weekCutoff)
      .map((session) => session.courseId),
  );
  if (weeklyCourseIds.size >= 3) {
    items.push({ id: "gk-cross-pollinator", kind: "vine", maturity: 1, label: "Cross-Pollinator", sub: `${weeklyCourseIds.size} courses growing this week` });
  }

  const studySessionCount = sessions.filter((session) => session.kind === "study" || session.kind === "exam").length;
  if (studySessionCount >= 20) {
    items.push({ id: "gk-mushroom-ring", kind: "mushroom", maturity: 1, label: "Mushroom Ring", sub: `${studySessionCount} sessions logged and counting` });
  }

  sessions
    .filter((session) => (session.kind === "study" || session.kind === "exam") && session.courseId && courseMap.has(session.courseId))
    .slice(-90)
    .forEach((session) => {
      const course = courseMap.get(session.courseId ?? "");
      if (!course) return;
      const rng = gardenRng(gardenHash(`${session.id}-species`));
      const maturity = Math.min(1, session.minutes / 90);
      const sessionDate = (session.endedAt || session.startedAt).slice(0, 10);
      const ageDays = (todayTime - new Date(`${sessionDate}T00:00:00`).getTime()) / 86400000;
      items.push({
        id: session.id,
        kind: gardenPickSpecies(rng, maturity),
        color: course.color,
        maturity,
        fresh: ageDays <= 1.5,
        label: course.name,
        sub: `${session.presetLabel || session.kind} · ${formatMinutes(session.minutes)}${ageDays <= 1.5 ? " · freshly grown" : ""}`,
      });
    });

  tasks.forEach((task) => {
    if (task.totalUnits <= 0 || task.completedUnits < task.totalUnits) return;
    const course = courseMap.get(task.courseId);
    if (!course) return;
    items.push({ id: `task-${task.id}`, kind: "flower", color: course.color, maturity: 1, label: course.name, sub: `Milestone · ${task.title}` });
  });

  const ambientBase = [4, 8, 12, 16, 21, 25][stage] ?? 4;
  const ambient = Math.round((ambientBase + Math.min(8, Math.floor(weeklyMinutes / 90))) * densityFactor);
  for (let index = 0; index < ambient; index += 1) {
    const rng = gardenRng(gardenHash(`ambient-${stage}-${index}`));
    items.push({ id: `ambient-${stage}-${index}`, kind: rng() < 0.6 ? "grass" : rng() < 0.5 ? "sprout" : "fern", maturity: 0.25 + rng() * 0.3, ambient: true });
  }

  return items;
}

function placeGardenItems(items: GardenItem[], labelClearanceFraction: number): PlacedGardenItem[] {
  const placed: PlacedGardenItem[] = [];
  let leftCount = 0;
  let rightCount = 0;
  for (const item of items) {
    const rng = gardenRng(gardenHash(`${item.id}-pos`));
    if (item.fixed) {
      const x = 0.16 + rng() * 0.68;
      const t = 0.42 + rng() * 0.16;
      placed.push({ ...item, x, t });
      if (x < 0.5) leftCount += 1; else rightCount += 1;
      continue;
    }
    let best: { x: number; t: number } | null = null;
    let bestScore = -1;
    for (let attempt = 0; attempt < 9; attempt += 1) {
      const x = 0.03 + 0.94 * rng();
      const t = Math.pow(rng(), 0.82);
      // Keep the frontmost plants from growing directly behind the stats label — its
      // translucent backdrop only lightly diffuses whatever's behind it, it doesn't hide it.
      // Taper the exclusion by depth (a wedge, not a rectangle): only plants right at the
      // label's own height need the full measured width of clearance, so plants a little
      // further back — which render higher up, clear of the label — aren't banned from the
      // left edge too. labelClearanceFraction is sized from the label's actual rendered
      // pixel width, not a guess, so it scales correctly on both narrow and very wide cards.
      const labelClearance = labelClearanceFraction * Math.max(0, (t - 0.72) / 0.28);
      if (x < labelClearance) continue;
      let distance = Infinity;
      for (const plant of placed) distance = Math.min(distance, Math.hypot((x - plant.x) * 1.7, t - plant.t));
      const onLighterSide = x < 0.5 ? leftCount <= rightCount : rightCount <= leftCount;
      const score = distance + (onLighterSide ? 0.15 : 0);
      if (score > bestScore) {
        bestScore = score;
        best = { x, t };
      }
    }
    const placement = best ?? { x: 0.45 + rng() * 0.3, t: 0.3 + rng() * 0.25 };
    placed.push({ ...item, ...placement });
    if (placement.x < 0.5) leftCount += 1; else rightCount += 1;
  }
  return placed.sort((a, b) => a.t - b.t);
}

function KnowledgeGardenWidget({ appState, weeklyMinutes }: { appState: AppState; weeklyMinutes: number }) {
  const gardenStageNames = ["Dormant", "Sprouting", "Growing", "Leafy", "Blooming", "Flourishing"];
  const gardenStageThresholds = [0, 30, 90, 210, 420, 720];
  const stage = Math.max(0, gardenStageThresholds.filter((threshold) => weeklyMinutes >= threshold).length - 1);
  const wrapRef = useRef<HTMLDivElement | null>(null);
  const statsRef = useRef<HTMLDivElement | null>(null);
  const [containerWidth, setContainerWidth] = useState(380);
  const [statsWidth, setStatsWidth] = useState(0);
  const [tip, setTip] = useState<{ x: number; y: number; label?: string; sub?: string } | null>(null);
  const streak = useMemo(() => getStreakDays({ sessions: appState.sessions } as AppState), [appState.sessions]);
  const densityFactor = Math.max(0.55, Math.min(2.2, containerWidth / 380));
  // The label's own rendered width (it's fixed-pixel, driven by font-size and text length, not
  // container size) sized as a fraction of the container — so the placement keep-out below stays
  // accurate whether this widget is a ~300px cockpit column or a full-width hero row.
  const labelClearanceFraction = statsWidth > 0 ? Math.max(0.12, Math.min(0.55, (statsWidth + 24) / containerWidth)) : 0.46;
  const plants = useMemo(
    () => placeGardenItems(buildGardenItems(appState.courses, appState.sessions, appState.tasks, stage, weeklyMinutes, densityFactor), labelClearanceFraction),
    [appState.courses, appState.sessions, appState.tasks, stage, weeklyMinutes, densityFactor, labelClearanceFraction],
  );
  const fireflies = streak >= 5 ? Math.min(6, 2 + Math.floor(streak / 3)) : 0;

  useLayoutEffect(() => {
    const element = wrapRef.current;
    if (!element || typeof ResizeObserver === "undefined") return undefined;
    const observer = new ResizeObserver(() => {
      const width = Math.round(element.clientWidth / 12) * 12;
      setContainerWidth((prev) => (prev === width ? prev : width));
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useLayoutEffect(() => {
    const element = statsRef.current;
    if (!element || typeof ResizeObserver === "undefined") return undefined;
    const observer = new ResizeObserver(() => {
      const width = Math.round(element.clientWidth);
      setStatsWidth((prev) => (prev === width ? prev : width));
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const showTip = (event: MouseEvent<HTMLDivElement>, plant: PlacedGardenItem) => {
    const rect = wrapRef.current?.getBoundingClientRect();
    if (!rect) return;
    setTip({ x: Math.max(10, Math.min(rect.width - 10, event.clientX - rect.left)), y: Math.max(28, event.clientY - rect.top), label: plant.label, sub: plant.sub });
  };

  return (
    <div ref={wrapRef} className="garden-stage gk-stage" aria-label={`Knowledge Garden: ${gardenStageNames[stage]}, ${formatMinutes(weeklyMinutes)} last 7 days`}>
      <div className="gk-sun" aria-hidden="true" />
      <div className="gk-ground" aria-hidden="true">
        <svg preserveAspectRatio="none" viewBox="0 0 100 8"><path d="M0 8 Q 14 2.5 32 4.8 T 64 3.6 T 100 5.2 L 100 8 Z" /></svg>
      </div>

      {plants.map((plant, index) => {
        const Species = gardenSpecies[plant.kind];
        const rng = gardenRng(gardenHash(`${plant.id}-draw`));
        const depth = 0.45 + 0.55 * plant.t;
        const width = 76 * (gardenBaseWidth[plant.kind] || 1) * depth * (0.72 + 0.38 * (plant.maturity || 0.5));
        const bottomPct = 2 + (1 - plant.t) * 52;
        const interactive = !plant.ambient;
        return (
          <div
            key={plant.id}
            className={`gk-plant-in${plant.fresh ? " gk-fresh" : ""}`}
            onMouseEnter={interactive ? (event) => showTip(event, plant) : undefined}
            onMouseMove={interactive ? (event) => showTip(event, plant) : undefined}
            onMouseLeave={interactive ? () => setTip(null) : undefined}
            style={{
              position: "absolute",
              left: `${plant.x * 100}%`,
              bottom: `${bottomPct}%`,
              width,
              transform: "translateX(-50%)",
              zIndex: Math.round(plant.t * 40) + 3,
              opacity: plant.ambient ? 0.5 + 0.35 * plant.t : 0.72 + 0.28 * plant.t,
              animationDelay: `${(index % 12) * 0.05}s`,
              pointerEvents: interactive ? "auto" : "none",
            }}
          >
            <span className="gk-sway" style={{ "--gk-dur": `${4.5 + (gardenHash(plant.id) % 40) / 10}s`, "--gk-del": `${-(gardenHash(plant.id) % 60) / 10}s` } as CSSProperties}>
              <svg viewBox="0 0 48 58" aria-hidden="true"><Species rng={rng} color={plant.color} maturity={plant.maturity} wise={plant.wise} /></svg>
            </span>
          </div>
        );
      })}

      {Array.from({ length: fireflies }).map((_, index) => {
        const rng = gardenRng(gardenHash(`fly-${index}`));
        return <span key={index} className="gk-firefly" style={{ left: `${8 + rng() * 84}%`, top: `${12 + rng() * 38}%`, "--gk-dur": `${2.4 + rng() * 2.4}s`, "--gk-del": `${-rng() * 4}s` } as CSSProperties} />;
      })}

      {stage >= 4 ? <div className="gk-butterfly" aria-hidden="true"><svg viewBox="0 0 20 16"><ellipse className="gk-wing-l" cx="6" cy="7" rx="5" ry="4" /><ellipse className="gk-wing-r" cx="14" cy="7" rx="5" ry="4" /><rect x="9.3" y="3.5" width="1.4" height="9" rx="0.7" /></svg></div> : null}

      <div className="gk-stage-dots" aria-hidden="true">{Array.from({ length: 5 }).map((_, index) => <span key={index} className={index < stage ? "active" : ""} />)}</div>
      <div ref={statsRef} className="gk-stats"><span className="serif">{gardenStageNames[stage]}</span><span className="mono">{formatMinutes(weeklyMinutes)} last 7 days</span></div>
      {stage === 0 && weeklyMinutes === 0 ? <p className="gk-empty mono">log a session to plant your first sprout</p> : null}
      {tip ? <div className="gk-tooltip" style={{ left: tip.x, top: tip.y - 14 }}><strong>{tip.label}</strong>{tip.sub ? <span>{tip.sub}</span> : null}</div> : null}
    </div>
  );
}
function GardenShoot({ rng }: GardenSpeciesProps) {
  const green = jgGreen(rng);
  return (
    <g>
      <path d="M24 58 Q 24.6 51 24 45" stroke={JG_LEAF_DEEP} strokeWidth="1.7" fill="none" strokeLinecap="round" />
      <path d={gardenLeaf(24, 46, 7 + rng() * 3, -1)} fill={green} />
      <path d={gardenLeaf(24, 47.5, 6 + rng() * 3, 1)} fill={gardenMix(green, JG_LEAF, 40)} />
      <path d="M24 46 L 20 42.5" stroke={gardenMix(green, "white", 22)} strokeWidth="0.45" opacity="0.7" />
      <path d="M24 47.5 L 28 44.5" stroke={gardenMix(green, "white", 22)} strokeWidth="0.45" opacity="0.7" />
    </g>
  );
}

function GardenSusuki({ rng }: GardenSpeciesProps) {
  const count = 5 + Math.floor(rng() * 4);
  return (
    <g>
      {Array.from({ length: count }).map((_, index) => {
        const dx = (rng() - 0.5) * 28;
        const h = 18 + rng() * 26;
        const green = jgGreen(rng);
        const tipX = 24 + dx;
        const tipY = 58 - h;
        return (
          <g key={index}>
            <path d={`M23.4 58 Q ${24 + dx * 0.2} ${58 - h * 0.62} ${tipX} ${tipY} Q ${24 + dx * 0.3} ${58 - h * 0.6} 24.6 58 Z`} fill={green} opacity={0.55 + rng() * 0.35} />
            <path d={`M24 58 Q ${24 + dx * 0.2} ${58 - h * 0.6} ${tipX} ${tipY}`} stroke={gardenMix(green, "white", 14)} strokeWidth="0.5" fill="none" opacity="0.5" />
          </g>
        );
      })}
      {rng() > 0.4
        ? (() => {
            const px = 24 + (rng() - 0.5) * 12;
            const py = 22 + rng() * 6;
            return (
              <g>
                <path d={`M24 58 Q ${px - 2} ${py + 14} ${px} ${py + 4}`} stroke={jgGreen(rng)} strokeWidth="0.9" fill="none" strokeLinecap="round" />
                {Array.from({ length: 9 }).map((_, index) => (
                  <path
                    key={index}
                    d={`M${px} ${py + 5} q ${(index % 2 ? 1 : -1) * (1.4 + rng() * 2)} -2.4 ${(index % 2 ? 1 : -1) * (2.4 + rng() * 3)} -${4 + rng() * 4}`}
                    stroke={gardenMix(JG_GOLD, "white", 40)}
                    strokeWidth="0.7"
                    fill="none"
                    strokeLinecap="round"
                    opacity="0.7"
                  />
                ))}
              </g>
            );
          })()
        : null}
    </g>
  );
}

function GardenMossClump({ rng, detail }: GardenSpeciesProps) {
  return (
    <g>
      {Array.from({ length: 3 }).map((_, index) => (
        <ellipse key={index} cx={12 + rng() * 24} cy={55 - rng() * 3} rx={7 + rng() * 7} ry={2.6 + rng() * 2} fill={gardenMix(JG_MOSS, JG_LEAF, Math.round(rng() * 45))} opacity="0.75" />
      ))}
      {detail !== 0
        ? Array.from({ length: 7 }).map((_, index) => {
            const x = 8 + rng() * 32;
            const y = 51 + rng() * 6;
            return <path key={`t${index}`} d={`M${x} ${y} q ${(rng() - 0.5) * 2} -1.6 ${(rng() - 0.5) * 3} -${1.8 + rng() * 2}`} stroke={gardenMix(JG_MOSS, JG_LEAF, 40)} strokeWidth="0.55" fill="none" strokeLinecap="round" opacity="0.75" />;
          })
        : null}
    </g>
  );
}

function GardenBamboo({ rng, maturity = 0.5 }: GardenSpeciesProps) {
  const stalks = 2 + Math.floor(rng() * 3);
  return (
    <g>
      {Array.from({ length: stalks }).map((_, index) => {
        const x = 24 + (index - (stalks - 1) / 2) * (5 + rng() * 3);
        const h = 30 + rng() * 18 + maturity * 8;
        const segments = 4 + Math.floor(rng() * 3);
        const lean = (rng() - 0.5) * 5;
        const cane = gardenMix(JG_BAMBOO, index % 2 ? JG_PINE : "white", 8 + Math.round(rng() * 16));
        const top = x + lean;
        return (
          <g key={index}>
            <path
              d={`M${x - 1.1} 58 Q ${x + lean * 0.4 - 1.1} ${58 - h * 0.55} ${top - 0.85} ${58 - h} L ${top + 0.85} ${58 - h} Q ${x + lean * 0.4 + 1.1} ${58 - h * 0.55} ${x + 1.1} 58 Z`}
              fill={cane}
            />
            <path d={`M${x - 0.5} 58 Q ${x + lean * 0.4 - 0.5} ${58 - h * 0.55} ${top - 0.35} ${58 - h}`} stroke={gardenMix(cane, "white", 30)} strokeWidth="0.6" fill="none" opacity="0.7" />
            {Array.from({ length: segments }).map((_, seg) => {
              const f = (seg + 1) / segments;
              const y = 58 - h * f;
              const sx = x + lean * f;
              return (
                <g key={seg}>
                  <path d={`M${sx - 1.25} ${y} q 1.25 0.9 2.5 0`} stroke={gardenMix(cane, "black", 34)} strokeWidth="0.7" fill="none" />
                  {seg > segments - 4
                    ? Array.from({ length: 3 }).map((__, tuftIndex) => {
                        const dir = (seg + tuftIndex) % 2 ? 1 : -1;
                        const len = 7 + rng() * 6;
                        return <path key={tuftIndex} d={gardenBlade(sx, y - tuftIndex * 1.6, len, dir, 3 + rng() * 3, 0.17)} fill={gardenMix(JG_PINE, JG_BAMBOO, 30 + Math.round(rng() * 45))} opacity="0.92" />;
                      })
                    : null}
                </g>
              );
            })}
          </g>
        );
      })}
    </g>
  );
}

function GardenMomiji({ rng, color, maturity = 0.5, detail }: GardenSpeciesProps) {
  const h = 26 + rng() * 12 + maturity * 10;
  const red = rng() > 0.45 ? JG_MAPLE : JG_MAPLE2;
  const bark = gardenMix(JG_SOIL, "black", 32);
  const crown: number[][] = [
    [24, 58 - h, 10.2],
    [24 - 9, 58 - h * 0.84, 7.6],
    [24 + 9.5, 58 - h * 0.81, 8],
    [24, 58 - h * 0.66, 8.6],
    [24 - 4.5, 58 - h * 1.03, 5.2],
    [24 + 5, 58 - h * 1.0, 4.8],
  ];
  const leaves: ReactNode[] = [];
  const leafCount = detail === 0 ? 10 : 16 + Math.round(maturity * 8);
  for (let index = 0; index < leafCount; index += 1) {
    const b = crown[Math.floor(rng() * crown.length)];
    const angle = rng() * Math.PI * 2;
    const radius = Math.sqrt(rng()) * b[2] * 0.95;
    const tint = rng();
    const fill = gardenMix(tint < 0.34 ? JG_MAPLE : tint < 0.7 ? JG_MAPLE2 : gardenMix(red, JG_MOSS, 30), tint < 0.5 ? "black" : "white", 6 + Math.round(rng() * 14));
    leaves.push(gardenMapleLeaf(b[0] + Math.cos(angle) * radius, b[1] + Math.sin(angle) * radius * 0.8, 2 + rng() * 1.6, rng() * 360, fill, `l${index}`, 0.9 + rng() * 0.1));
  }
  return (
    <g>
      <path d={`M22.4 58 C 22.2 ${58 - h * 0.28} 24.6 ${58 - h * 0.44} 23.3 ${58 - h * 0.64} L 24.9 ${58 - h * 0.64} C 25.6 ${58 - h * 0.44} 25 ${58 - h * 0.28} 25.7 58 Z`} fill={bark} />
      <path d={`M24 ${58 - h * 0.44} Q 30 ${58 - h * 0.58} 32.6 ${58 - h * 0.72}`} stroke={bark} strokeWidth="1.3" fill="none" strokeLinecap="round" />
      <path d={`M24 ${58 - h * 0.5} Q 18 ${58 - h * 0.62} 15.4 ${58 - h * 0.76}`} stroke={bark} strokeWidth="1.2" fill="none" strokeLinecap="round" />
      <path d={`M24 ${58 - h * 0.6} Q 26.5 ${58 - h * 0.74} 27.5 ${58 - h * 0.86}`} stroke={bark} strokeWidth="0.9" fill="none" strokeLinecap="round" />
      {crown.map((b, index) => (
        <ellipse key={index} cx={b[0]} cy={b[1]} rx={b[2]} ry={b[2] * 0.78} fill={gardenMix(red, "black", 16 + Math.round(rng() * 12))} opacity="0.5" />
      ))}
      {leaves}
      {Array.from({ length: maturity > 0.7 ? 5 : 2 }).map((_, index) => gardenMapleLeaf(12 + rng() * 24, 53 - rng() * 4, 1.5 + rng(), rng() * 360, gardenMix(color || red, "black", 8), `g${index}`, 0.75))}
    </g>
  );
}

function GardenSakura({ rng, color, maturity = 0.5, detail }: GardenSpeciesProps) {
  const h = 26 + rng() * 12 + maturity * 10;
  const pink = color ? gardenMix(color, JG_SAKURA, 74) : JG_SAKURA;
  const deep = gardenMix(pink, JG_MAPLE, 22);
  const bark = gardenMix(JG_SOIL, "black", 34);
  const canopy: number[][] = [
    [24, 58 - h, 9.8],
    [24 - 9, 58 - h * 0.86, 7.4],
    [24 + 9.4, 58 - h * 0.83, 7.8],
    [24, 58 - h * 0.68, 8.4],
    [24 - 4, 58 - h * 1.04, 5.4],
    [24 + 5, 58 - h * 1.02, 5],
  ];
  const flowers: ReactNode[] = [];
  const flowerCount = detail === 0 ? 7 : 11 + Math.round(maturity * 7);
  for (let index = 0; index < flowerCount; index += 1) {
    const b = canopy[Math.floor(rng() * canopy.length)];
    const angle = rng() * Math.PI * 2;
    const radius = Math.sqrt(rng()) * b[2] * 0.92;
    flowers.push(gardenBlossom(b[0] + Math.cos(angle) * radius, b[1] + Math.sin(angle) * radius * 0.78, 2.1 + rng() * 1.5, rng() < 0.35 ? deep : pink, rng() * 72, `f${index}`));
  }
  return (
    <g>
      <path d={`M24 58 C 22.6 ${58 - h * 0.24} 25.6 ${58 - h * 0.44} 24 ${58 - h * 0.6}`} stroke={bark} strokeWidth="2.8" fill="none" strokeLinecap="round" />
      <path d={`M24 ${58 - h * 0.46} Q 29.5 ${58 - h * 0.6} 32 ${58 - h * 0.76}`} stroke={bark} strokeWidth="1.5" fill="none" strokeLinecap="round" />
      <path d={`M24 ${58 - h * 0.52} Q 18.5 ${58 - h * 0.66} 16 ${58 - h * 0.8}`} stroke={bark} strokeWidth="1.4" fill="none" strokeLinecap="round" />
      {canopy.map((p, index) => (
        <ellipse key={index} cx={p[0]} cy={p[1]} rx={p[2]} ry={p[2] * 0.74} fill={gardenMix(pink, index % 2 ? "white" : JG_MAPLE, 10 + Math.round(rng() * 10))} opacity="0.55" />
      ))}
      {flowers}
      {Array.from({ length: 5 }).map((_, index) => (
        <circle key={index} cx={10 + rng() * 28} cy={46 + rng() * 11} r="1" fill={pink} opacity="0.8" />
      ))}
    </g>
  );
}

function GardenPine({ rng, wise, detail }: GardenSpeciesProps) {
  const pads: number[][] = [
    [24, 22, 13, 4.6],
    [12, 31, 8.5, 3.4],
    [36, 29, 9, 3.6],
    [24, 36, 10, 3.6],
    [30, 44, 7, 2.8],
  ];
  const bark = gardenMix(JG_SOIL, "black", 28);
  return (
    <g>
      <path d="M22.2 58 C 18.6 48 25.6 42 21.6 34 C 19.8 30 22.6 27 22.8 24 L 25.2 24 C 25.4 27 22.4 30 24.4 34 C 28.4 42 21.6 48 25.8 58 Z" fill={bark} />
      {[0, 1, 2].map((index) => (
        <path key={index} d={`M${22.6 + index * 0.9} ${54 - index * 3} q 0.6 -6 -0.3 -11`} stroke={gardenMix(bark, "white", 16)} strokeWidth="0.35" fill="none" opacity="0.55" />
      ))}
      <path d="M23.6 38 Q 30 36 33 31" stroke={bark} strokeWidth="1.7" fill="none" strokeLinecap="round" />
      <path d="M23.2 34 Q 17 32 13.5 30" stroke={bark} strokeWidth="1.7" fill="none" strokeLinecap="round" />
      {pads.map((p, index) => (
        <g key={index}>
          <ellipse cx={p[0]} cy={p[1]} rx={p[2]} ry={p[3]} fill={gardenMix(JG_PINE, JG_LEAF, 12 + Math.round(rng() * 34))} opacity="0.95" />
          <ellipse cx={p[0] - p[2] * 0.25} cy={p[1] - p[3] * 0.45} rx={p[2] * 0.55} ry={p[3] * 0.5} fill={gardenMix(JG_PINE, "white", 12)} opacity="0.5" />
          {detail !== 0
            ? Array.from({ length: 8 }).map((_, tick) => {
                const angle = rng() * Math.PI * 2;
                const radius = Math.sqrt(rng());
                const nx = p[0] + Math.cos(angle) * p[2] * radius;
                const ny = p[1] + Math.sin(angle) * p[3] * radius;
                const dir = rng() < 0.5 ? -1 : 1;
                return (
                  <path
                    key={tick}
                    d={`M${nx} ${ny} l ${dir * (1.2 + rng() * 1.6)} ${-(1.4 + rng() * 1.8)}`}
                    stroke={gardenMix(JG_PINE, rng() < 0.5 ? "white" : "black", 10 + Math.round(rng() * 18))}
                    strokeWidth="0.5"
                    strokeLinecap="round"
                    opacity="0.8"
                  />
                );
              })
            : null}
        </g>
      ))}
      {wise ? Array.from({ length: 6 }).map((_, index) => <circle key={index} cx={11 + rng() * 26} cy={16 + rng() * 26} r="1.2" fill={JG_GOLD} />) : null}
    </g>
  );
}

function GardenAzalea({ rng, color, maturity = 0.5, detail }: GardenSpeciesProps) {
  const blooms = detail === 0 ? 4 : maturity > 0.5 ? 9 : 4;
  const petal = color || "oklch(0.68 0.13 12)";
  const mounds: number[][] = [
    [16, 51, 8 + rng() * 3, 6.5 + rng() * 2],
    [31, 49, 9 + rng() * 3, 7.5 + rng() * 2.5],
    [24, 52, 10 + rng() * 2, 6 + rng() * 2],
  ];
  return (
    <g>
      {mounds.map((m, index) => (
        <ellipse key={index} cx={m[0]} cy={m[1]} rx={m[2]} ry={m[3]} fill={index === 2 ? gardenMix(jgGreen(rng), JG_MOSS, 35) : jgGreen(rng)} />
      ))}
      {detail !== 0
        ? Array.from({ length: 10 }).map((_, index) => {
            const m = mounds[Math.floor(rng() * 3)];
            const angle = rng() * Math.PI * 2;
            const radius = Math.sqrt(rng());
            const x = m[0] + Math.cos(angle) * m[2] * radius;
            const y = m[1] + Math.sin(angle) * m[3] * radius * 0.9;
            return (
              <ellipse
                key={index}
                cx={x}
                cy={y}
                rx={1.5 + rng()}
                ry={0.9 + rng() * 0.5}
                fill={gardenMix(JG_MOSS, rng() < 0.5 ? "white" : "black", 8 + Math.round(rng() * 16))}
                opacity="0.6"
                transform={`rotate(${-40 + rng() * 80} ${x} ${y})`}
              />
            );
          })
        : null}
      {Array.from({ length: blooms }).map((_, index) => {
        const m = mounds[Math.floor(rng() * 3)];
        const angle = rng() * Math.PI * 2;
        const radius = Math.sqrt(rng()) * 0.85;
        return gardenBlossom(
          m[0] + Math.cos(angle) * m[2] * radius,
          m[1] - m[3] * 0.35 + Math.sin(angle) * m[3] * radius,
          1.7 + rng() * 0.9,
          gardenMix(petal, rng() < 0.4 ? "white" : "black", 6 + Math.round(rng() * 12)),
          rng() * 72,
          `b${index}`,
        );
      })}
    </g>
  );
}

function GardenIris({ rng, color }: GardenSpeciesProps) {
  const stems = 2 + Math.floor(rng() * 3);
  return (
    <g>
      {Array.from({ length: stems }).map((_, index) => {
        const dx = (index - (stems - 1) / 2) * (5 + rng() * 3);
        const h = 24 + rng() * 14;
        const cx = 24 + dx;
        const cy = 58 - h;
        const violet = color || "oklch(0.55 0.12 285)";
        const deep = gardenMix(violet, "black", 16);
        const pale = gardenMix(violet, "white", 22);
        return (
          <g key={index}>
            {[-1, 1, -1].map((dir, bladeIndex) => (
              <path key={bladeIndex} d={gardenBlade(24 + dx * 0.4, 58, 20 + rng() * 8, dir, -(16 + rng() * 10), 0.09)} fill={gardenMix(jgGreen(rng), "black", bladeIndex * 5)} opacity="0.85" />
            ))}
            <path d={`M${24 + dx * 0.4} 58 L ${cx} ${cy}`} stroke={jgGreen(rng)} strokeWidth="1.2" strokeLinecap="round" />
            <path d={`M${cx} ${cy + 1.6} q -3.6 0.4 -4.6 3.4 q 2.6 1.4 4.6 -0.6 Z`} fill={deep} />
            <path d={`M${cx} ${cy + 1.6} q 3.6 0.4 4.6 3.4 q -2.6 1.4 -4.6 -0.6 Z`} fill={deep} />
            <path d={`M${cx} ${cy + 2} q -1.4 3.4 0 5.4 q 1.4 -2 0 -5.4 Z`} fill={gardenMix(violet, "black", 24)} />
            <ellipse cx={cx - 1.5} cy={cy - 1.4} rx="1.5" ry="2.8" fill={pale} transform={`rotate(-22 ${cx} ${cy})`} />
            <ellipse cx={cx + 1.5} cy={cy - 1.4} rx="1.5" ry="2.8" fill={pale} transform={`rotate(22 ${cx} ${cy})`} />
            <path d={`M${cx - 1.4} ${cy + 3.2} q 1.4 0.6 2.8 0`} stroke={gardenMix(JG_GOLD, "white", 30)} strokeWidth="0.7" fill="none" strokeLinecap="round" />
          </g>
        );
      })}
    </g>
  );
}

function GardenHagi({ rng, color }: GardenSpeciesProps) {
  const arcs = 3 + Math.floor(rng() * 2);
  return (
    <g>
      {Array.from({ length: arcs }).map((_, index) => {
        const dir = index % 2 ? 1 : -1;
        const h = 22 + rng() * 12;
        const reach = 10 + rng() * 8;
        const green = jgGreen(rng);
        const bloom = color || "oklch(0.66 0.1 340)";
        const pt = (f: number): [number, number] => [24 + dir * reach * f, 58 - h * (0.55 + 0.45 * (1 - Math.abs(f - 0.45) * 1.4))];
        return (
          <g key={index}>
            <path d={`M24 58 Q ${24 + dir * reach * 0.4} ${58 - h} ${24 + dir * reach} ${58 - h * 0.7}`} stroke={green} strokeWidth="1.1" fill="none" strokeLinecap="round" />
            {[0.25, 0.45, 0.65, 0.85].map((f, leafIndex) => {
              const [x, y] = pt(f);
              return (
                <g key={`lf${leafIndex}`}>
                  <ellipse cx={x} cy={y - 1.6} rx="1.7" ry="1.1" fill={gardenMix(green, "white", 10)} opacity="0.9" transform={`rotate(${dir * -25} ${x} ${y})`} />
                  <ellipse cx={x} cy={y + 1.6} rx="1.7" ry="1.1" fill={gardenMix(green, "black", 8)} opacity="0.9" transform={`rotate(${dir * 25} ${x} ${y})`} />
                </g>
              );
            })}
            {[0.4, 0.62, 0.86].map((f, bloomIndex) => {
              const [x, y] = pt(f);
              return (
                <g key={`b${bloomIndex}`}>
                  <ellipse cx={x} cy={y + 0.4} rx="1.5" ry="1.1" fill={bloom} opacity="0.95" />
                  <ellipse cx={x} cy={y - 0.6} rx="1" ry="0.9" fill={gardenMix(bloom, "white", 30)} opacity="0.95" />
                </g>
              );
            })}
          </g>
        );
      })}
    </g>
  );
}

function GardenJapaneseFern({ rng }: GardenSpeciesProps) {
  const h = 24 + rng() * 14;
  const curl = 3 + rng() * 4;
  const green = jgGreen(rng);
  const fronds: ReactNode[] = [];
  for (let index = 0; index < 6; index += 1) {
    const f = 0.18 + 0.78 * (index / 6);
    const x = 24 + curl * Math.sin(f * 2.4);
    const y = 58 - h * f;
    const len = (1 - f) * 9 + 2;
    [-1, 1].forEach((dir) => {
      fronds.push(<path key={`${dir < 0 ? "l" : "r"}${index}`} d={gardenBlade(x, y, len, dir, -len * 0.85, 0.3)} fill={gardenMix(green, dir < 0 ? "black" : "white", 6)} opacity="0.92" />);
      for (let vein = 1; vein < 4; vein += 1) {
        const px = x + dir * len * (vein / 4);
        const py = y - len * 0.85 * (vein / 4);
        fronds.push(<path key={`${dir < 0 ? "lp" : "rp"}${index}${vein}`} d={`M${px} ${py} l ${dir * 1.6} -1.5`} stroke={gardenMix(green, "white", 14)} strokeWidth="0.4" opacity="0.6" />);
      }
    });
  }
  return (
    <g>
      <path d={`M24 58 Q ${24 + curl * 2} ${58 - h * 0.5} 24 ${58 - h}`} stroke={JG_LEAF_DEEP} strokeWidth="1.6" fill="none" strokeLinecap="round" />
      {fronds}
    </g>
  );
}

function GardenStone({ rng }: GardenSpeciesProps) {
  return (
    <g>
      <ellipse cx="24" cy="57" rx={12 + rng() * 4} ry="2.6" fill="black" opacity="0.16" />
      <ellipse cx="24" cy="55" rx={11 + rng() * 5} ry={5 + rng() * 3} fill={gardenMix(JG_STONE, "black", 18)} opacity="0.9" />
      <ellipse cx={22 + rng() * 3} cy={52 + rng() * 2} rx={7 + rng() * 3} ry={3.4 + rng() * 1.6} fill={gardenMix(JG_STONE, "white", 16)} opacity="0.75" />
      {Array.from({ length: 5 }).map((_, index) => (
        <path key={index} d={`M${15 + rng() * 18} ${50 + rng() * 6} q ${2 + rng() * 3} ${-1 + rng() * 2} ${4 + rng() * 4} ${-0.5 + rng()}`} stroke={gardenMix(JG_STONE, "black", 34)} strokeWidth="0.4" fill="none" opacity="0.5" />
      ))}
      <ellipse cx="30" cy="56" rx="4.5" ry="2.4" fill={gardenMix(JG_MOSS, "black", 10)} opacity="0.65" />
      <ellipse cx="17" cy="55.5" rx="3" ry="1.6" fill={gardenMix(JG_MOSS, "black", 20)} opacity="0.5" />
    </g>
  );
}

function GardenTeaHouse({ rng }: GardenSpeciesProps) {
  void rng;
  const wood = gardenMix(JG_SOIL, "black", 18);
  const woodLight = gardenMix(JG_SOIL, "white", 14);
  const roof = gardenMix(JG_STONE, "black", 46);
  const roofLight = gardenMix(JG_STONE, "black", 30);
  const shoji = gardenMix(JG_GOLD, "white", 62);
  return (
    <g>
      <ellipse cx="24" cy="57" rx="17" ry="2.6" fill="black" opacity="0.18" />
      <rect x="9" y="48" width="30" height="3.4" rx="0.8" fill={woodLight} />
      {[11, 20, 28, 36].map((x, index) => (
        <rect key={index} x={x} y="51" width="1.6" height="5" fill={wood} />
      ))}
      <rect x="11.5" y="34" width="25" height="14" fill={gardenMix(wood, "black", 12)} />
      <rect x="13.5" y="36" width="8.5" height="10" fill={shoji} opacity="0.75" />
      <rect x="24" y="36" width="8.5" height="10" fill={shoji} opacity="0.55" />
      <line x1="17.75" y1="36" x2="17.75" y2="46" stroke={wood} strokeWidth="0.6" />
      <line x1="28.25" y1="36" x2="28.25" y2="46" stroke={wood} strokeWidth="0.6" />
      <line x1="13.5" y1="41" x2="32.5" y2="41" stroke={wood} strokeWidth="0.6" />
      <path d="M24 17 L 43 33 Q 39 35.5 34.5 34.2 L 24 25.5 L 13.5 34.2 Q 9 35.5 5 33 Z" fill={roof} />
      <path d="M24 19.5 L 38.5 32 L 9.5 32 Z" fill={roofLight} opacity="0.85" />
      <path d="M24 17 L 24 25.5" stroke={gardenMix(roof, "white", 16)} strokeWidth="0.7" />
      <rect x="22.6" y="14.6" width="2.8" height="3" rx="0.6" fill={gardenMix(roof, "white", 10)} />
      <ellipse cx="24" cy="55.5" rx="5" ry="2" fill={gardenMix(JG_STONE, "black", 24)} />
    </g>
  );
}

function GardenStoneLantern({ rng }: GardenSpeciesProps) {
  void rng;
  return (
    <g>
      <ellipse cx="24" cy="57.5" rx="11" ry="2.6" fill="black" opacity="0.18" />
      <ellipse cx="24" cy="57" rx="9" ry="3" fill={gardenMix(JG_STONE, "black", 30)} opacity="0.8" />
      <rect x="21" y="42" width="6" height="14" fill={JG_STONE} />
      <rect x="21" y="42" width="1.6" height="14" fill={gardenMix(JG_STONE, "white", 18)} opacity="0.6" />
      <rect x="25.6" y="42" width="1.4" height="14" fill={gardenMix(JG_STONE, "black", 22)} opacity="0.6" />
      <rect x="17.5" y="38" width="13" height="4.5" rx="1" fill={gardenMix(JG_STONE, "white", 10)} />
      <rect x="19" y="29" width="10" height="9.5" rx="1" fill={gardenMix(JG_STONE, "white", 6)} />
      <rect x="21.2" y="31.5" width="5.6" height="5" rx="0.8" fill={JG_GOLD} opacity="0.8" />
      <path d="M14.5 29 L 33.5 29 L 28 23.5 L 20 23.5 Z" fill={gardenMix(JG_STONE, "black", 12)} />
      <circle cx="24" cy="21.5" r="2" fill={gardenMix(JG_STONE, "white", 14)} />
    </g>
  );
}

function GardenKiku({ rng }: GardenSpeciesProps) {
  const cx = 24;
  const cy = 26;
  const rot = rng() * 60;
  return (
    <g>
      <circle cx={cx} cy={cy} r="12" fill={JG_GOLD} opacity="0.12" />
      <path d={`M24 58 Q 26 42 ${cx} ${cy}`} stroke={JG_LEAF_DEEP} strokeWidth="1.7" fill="none" strokeLinecap="round" />
      <path d={gardenLeaf(25, 46, 8, -1)} fill={jgGreen(rng)} />
      <path d={gardenLeaf(25.2, 41, 7, 1)} fill={jgGreen(rng)} />
      {Array.from({ length: 12 }).map((_, index) => (
        <ellipse key={index} cx={cx} cy={cy - 6} rx="1.7" ry="6" fill={JG_GOLD} opacity={index % 2 ? 0.75 : 0.95} transform={`rotate(${rot + index * 30} ${cx} ${cy})`} />
      ))}
      <circle cx={cx} cy={cy} r="2.6" fill={gardenMix(JG_GOLD, "white", 55)} />
    </g>
  );
}

const japaneseGardenSpecies: Record<JapaneseGardenPlantKind, (props: GardenSpeciesProps) => ReactNode> = {
  shoot: GardenShoot,
  susuki: GardenSusuki,
  moss: GardenMossClump,
  fern: GardenJapaneseFern,
  bamboo: GardenBamboo,
  momiji: GardenMomiji,
  sakura: GardenSakura,
  pine: GardenPine,
  azalea: GardenAzalea,
  iris: GardenIris,
  hagi: GardenHagi,
  stone: GardenStone,
  teahouse: GardenTeaHouse,
  lantern: GardenStoneLantern,
  kiku: GardenKiku,
};

const japaneseGardenBaseWidth: Record<JapaneseGardenPlantKind, number> = {
  shoot: 0.5,
  susuki: 0.85,
  moss: 0.9,
  fern: 0.95,
  bamboo: 1.1,
  momiji: 1.75,
  sakura: 1.6,
  pine: 1.95,
  azalea: 1.15,
  iris: 0.95,
  hagi: 1,
  stone: 0.85,
  teahouse: 2,
  lantern: 0.95,
  kiku: 1.05,
};

function japanesePickSpecies(rng: () => number, maturity: number): JapaneseGardenPlantKind {
  if (maturity < 0.3) return rng() < 0.55 ? "shoot" : "susuki";
  if (maturity < 0.6) return ["fern", "hagi", "iris", "susuki", "azalea"][Math.floor(rng() * 5)] as JapaneseGardenPlantKind;
  if (maturity < 0.85) return ["bamboo", "azalea", "sakura", "iris", "momiji", "hagi"][Math.floor(rng() * 6)] as JapaneseGardenPlantKind;
  return ["sakura", "momiji", "sakura", "bamboo", "pine"][Math.floor(rng() * 5)] as JapaneseGardenPlantKind;
}

const JG_ROWS = [0.1, 0.36, 0.62, 0.88];
const JG_UPPER: JapaneseGardenPondBoxShape = { cx: 63, cy: 64, rx: 9.5, ry: 3.8 };
const JG_LOWER: JapaneseGardenPondBoxShape = { cx: 78, cy: 93, rx: 12.5, ry: 5 };
const JG_FLOW: [number, number][] = [
  [64.6, 65.8],
  [65.2, 74.5],
  [74.2, 81],
  [76.5, 91.5],
];

const JAPANESE_GARDEN_TALL_KINDS: Partial<Record<JapaneseGardenPlantKind, true>> = {
  momiji: true,
  sakura: true,
  pine: true,
  bamboo: true,
  kiku: true,
  lantern: true,
  teahouse: true,
};

const JAPANESE_GARDEN_FILL_RATIO = [0.12, 0.26, 0.38, 0.48, 0.56, 0.62, 0.68, 0.74, 0.8];

/* The river/bridge geometry was authored for a ~390x205 box. Dashboard layouts
   (e.g. the Focus layout's full-width hero card) can render this widget at far
   wider-than-tall aspect ratios; without correction the bezier/normal-vector
   math shears into a diagonal mess. Keep a single uniform scale tied to this
   reference aspect and letterbox/pillarbox the water drawing inside the real
   box instead of stretching it independently per axis. */
const JAPANESE_GARDEN_WATER_ASPECT = 390 / 205;

function japaneseGardenWaterFrame(boxW: number, boxH: number) {
  const scaleX = Math.min(boxW / 100, (boxH / 100) * JAPANESE_GARDEN_WATER_ASPECT);
  const scaleY = scaleX / JAPANESE_GARDEN_WATER_ASPECT;
  const effW = scaleX * 100;
  const effH = scaleY * 100;
  const offsetX = (boxW - effW) / 2;
  const offsetY = (boxH - effH) / 2;
  return { scaleX, scaleY, effW, effH, offsetX, offsetY };
}

function japaneseGardenOnWater(x: number, t: number, stage: number, containerWidth: number, containerHeight: number) {
  if (stage < 5) return false;
  const frame = japaneseGardenWaterFrame(containerWidth || 390, containerHeight || 205);
  const px = ((x * (containerWidth || 390) - frame.offsetX) / frame.effW) * 100;
  const py = 44 + t * 55;
  const inEllipse = (o: JapaneseGardenPondBoxShape, pad: number) => Math.pow((px - o.cx) / (o.rx + pad), 2) + Math.pow((py - o.cy) / (o.ry + pad * 0.5), 2) < 1;
  if (inEllipse(JG_UPPER, 3)) return true;
  if (stage >= 6) {
    if (inEllipse(JG_LOWER, 3)) return true;
    const [ax, ay] = JG_FLOW[0];
    const [bx, by] = JG_FLOW[1];
    const vx = bx - ax;
    const vy = by - ay;
    const u = Math.max(0, Math.min(1, ((px - ax) * vx + (py - ay) * vy) / (vx * vx + vy * vy)));
    const d = Math.hypot(px - (ax + vx * u), (py - (ay + vy * u)) * 1.9);
    if (d < 8) return true;
  }
  return false;
}

function japaneseGardenCells(stage: number, wide: boolean): JapaneseGardenCell[] {
  const cols = wide ? 8 : 5;
  const cw = 1 / cols;
  const upper = stage >= 5;
  const lower = stage >= 6;
  const cells: JapaneseGardenCell[] = [];
  for (let row = 0; row < JG_ROWS.length; row += 1) {
    for (let col = 0; col < cols; col += 1) {
      const x = (col + 0.5) / cols;
      const t = JG_ROWS[row];
      if (upper && t > 0.5 && t < 0.8 && x > 0.12 && x < 0.46) continue;
      if (lower && t > 0.5 && t < 0.8 && x >= 0.3 && x < 0.62) continue;
      if (lower && t > 0.8 && x > 0.46 && x < 0.92) continue;
      const low = t > 0.8 && x < 0.4;
      cells.push({ x, t, cw, low, key: `${row}-${col}` });
    }
  }
  return cells.sort((a, b) => (gardenHash(`cell${a.key}`) % 9973) - (gardenHash(`cell${b.key}`) % 9973));
}

function buildJapaneseGardenItems(courses: Course[], sessions: StudySession[], tasks: Task[], stage: number): JapaneseGardenBuildResult {
  const courseMap = new Map(courses.map((course) => [course.id, course]));
  const streak = getStreakDays({ sessions } as AppState);
  const todayTime = new Date(`${isoDate()}T00:00:00`).getTime();
  const heroes: JapaneseGardenItem[] = [];
  const features: JapaneseGardenItem[] = [];
  const sessionItems: JapaneseGardenItem[] = [];
  const milestones: JapaneseGardenItem[] = [];

  if (stage >= 7 || streak >= 14) {
    heroes.push({ id: "jg-great-pine", kind: "pine", wise: true, maturity: 1, anchor: [0.86, 0.36], label: "The Great Pine", sub: "Highest stage · a garden that keeps its shape" });
  }
  if (stage >= 5 || streak >= 10) {
    heroes.push({ id: "jg-old-maple", kind: "momiji", maturity: 1, anchor: [0.14, 0.36], label: "The Old Maple", sub: "Weeks of steady study" });
  }
  if (stage >= 3) {
    features.push({ id: "jg-teahouse", kind: "teahouse", maturity: 1, anchor: [0.3, 0.1], label: "Tea House", sub: "Built once the grasses came in" });
  }
  if (stage >= 4) {
    features.push({ id: "jg-lantern", kind: "lantern", maturity: 1, anchor: [0.16, 0.62], label: "Stone Lantern", sub: "Unlocked at the bamboo grove" });
  }
  if (streak >= 5) {
    features.push({ id: "jg-kiku", kind: "kiku", maturity: 1, anchor: [0.82, 0.62], label: `${streak}-day streak`, sub: "Golden kiku · keep it alive" });
  }

  sessions
    .filter((session) => (session.kind === "study" || session.kind === "exam") && session.courseId && courseMap.has(session.courseId))
    .forEach((session) => {
      const course = courseMap.get(session.courseId ?? "");
      if (!course) return;
      const rng = gardenRng(gardenHash(`${session.id}-jspecies`));
      const maturity = Math.min(1, session.minutes / 90);
      const sessionDate = (session.endedAt || session.startedAt).slice(0, 10);
      const ageDays = (todayTime - new Date(`${sessionDate}T00:00:00`).getTime()) / 86400000;
      sessionItems.push({
        id: session.id,
        kind: japanesePickSpecies(rng, maturity),
        color: course.color,
        maturity,
        minutes: session.minutes,
        fresh: ageDays <= 1.5,
        label: course.name,
        sub: `${session.presetLabel || session.kind} · ${formatMinutes(session.minutes)}${ageDays <= 1.5 ? " · freshly grown" : ""}`,
      });
    });
  sessionItems.sort((a, b) => (b.minutes || 0) - (a.minutes || 0));

  tasks.forEach((task) => {
    if (task.totalUnits <= 0 || task.completedUnits < task.totalUnits) return;
    const course = courseMap.get(task.courseId);
    if (!course) return;
    const rng = gardenRng(gardenHash(`${task.id}-ms`));
    milestones.push({ id: `task-${task.id}`, kind: rng() < 0.5 ? "sakura" : "azalea", color: course.color, maturity: 1, label: course.name, sub: `Milestone · ${task.title}` });
  });

  return { heroes, features, sessions: sessionItems, milestones };
}

function placeJapaneseGardenItems(build: JapaneseGardenBuildResult, stage: number, wide: boolean, containerWidth: number, containerHeight: number): PlacedJapaneseGardenItem[] {
  const cells = japaneseGardenCells(stage, wide);
  const free = cells.slice();
  const onWater = (x: number, t: number) => japaneseGardenOnWater(x, t, stage, containerWidth, containerHeight);

  const take = (anchor: [number, number] | undefined, rng: () => number, tall: boolean) => {
    const pool = tall ? free.filter((cell) => !cell.low) : free;
    if (!pool.length) return null;
    let pick = pool[0];
    if (anchor) {
      let best = Infinity;
      pool.forEach((cell) => {
        const d = Math.hypot((cell.x - anchor[0]) * 1.4, cell.t - anchor[1]);
        if (d < best) {
          best = d;
          pick = cell;
        }
      });
    }
    free.splice(free.indexOf(pick), 1);
    let x = Math.max(0.05, Math.min(0.95, pick.x + (rng() - 0.5) * pick.cw * 0.44));
    let t = pick.t + (rng() - 0.5) * 0.05;
    for (let attempt = 0; attempt < 4 && onWater(x, t); attempt += 1) {
      x = Math.max(0.05, Math.min(0.95, pick.x + (rng() - 0.5) * pick.cw * 0.44));
      t = pick.t + (rng() - 0.5) * 0.05;
    }
    if (onWater(x, t)) return null;
    return { x, t, cw: pick.cw, low: pick.low };
  };

  const placed: PlacedJapaneseGardenItem[] = [];
  const push = (item: JapaneseGardenItem, anchor?: [number, number]) => {
    const spot = take(anchor, gardenRng(gardenHash(`${item.id}-cell`)), !!JAPANESE_GARDEN_TALL_KINDS[item.kind]);
    if (!spot) return false;
    placed.push({ ...item, ...spot, growth: 1 });
    return true;
  };

  build.heroes.forEach((hero) => push(hero, hero.anchor));
  build.features.forEach((feature) => push(feature, feature.anchor));
  build.milestones.forEach((milestone) => push(milestone));

  let surplus = 0;
  build.sessions.forEach((session) => {
    if (!push(session)) surplus += 1;
  });

  const target = Math.round(free.length * (JAPANESE_GARDEN_FILL_RATIO[stage] ?? 0.2));
  for (let index = 0; index < target; index += 1) {
    const rng = gardenRng(gardenHash(`jamb-${index}`));
    const r = rng();
    let kind: JapaneseGardenPlantKind;
    if (stage <= 1) kind = r < 0.6 ? "moss" : "shoot";
    else if (stage <= 3) kind = r < 0.34 ? "moss" : r < 0.62 ? "susuki" : r < 0.84 ? "shoot" : "stone";
    else if (stage <= 5) kind = r < 0.26 ? "moss" : r < 0.46 ? "susuki" : r < 0.62 ? "fern" : r < 0.78 ? "hagi" : r < 0.92 ? "azalea" : "stone";
    else kind = r < 0.18 ? "moss" : r < 0.32 ? "susuki" : r < 0.44 ? "fern" : r < 0.58 ? "hagi" : r < 0.72 ? "azalea" : r < 0.84 ? "bamboo" : r < 0.93 ? "momiji" : "stone";
    const spot = free.find((cell) => cell.low);
    const finalKind: JapaneseGardenPlantKind = spot && index % 2 === 0 ? (r < 0.6 ? "moss" : "susuki") : kind;
    const ambientRng = gardenRng(gardenHash(`jamb-${index}`));
    push({ id: `jamb-${index}`, kind: finalKind, maturity: 0.24 + ambientRng() * 0.36, ambient: true });
  }

  const fullness = 1 - free.length / Math.max(1, cells.length);
  const growth = 1 + Math.min(0.32, surplus * 0.02) + Math.max(0, fullness - 0.7) * 0.35;
  return placed.map((plant) => ({ ...plant, growth: plant.ambient ? 1 + (growth - 1) * 0.3 : growth })).sort((a, b) => a.t - b.t);
}

function GardenKoi({ tone, left, top, dur, del, flip }: { tone: string; left: number; top: number; dur: number; del: number; flip?: number }) {
  return (
    <span className="jg-koi" style={{ position: "absolute", left: `${left}%`, top: `${top}%`, display: "block", "--jg-dur": `${dur}s`, "--jg-del": `${del}s` } as CSSProperties}>
      <svg viewBox="0 0 22 10" width="18" style={{ display: "block", opacity: 0.92, transform: `rotate(${flip || 0}deg)` }}>
        <ellipse cx="9" cy="5" rx="7" ry="2.6" fill={tone} />
        <path d="M16 5 L 21 2.2 L 20 5 L 21 7.8 Z" fill={tone} opacity="0.85" />
        <ellipse cx="7" cy="4.2" rx="2.2" ry="1.2" fill="rgba(255,255,255,.55)" />
      </svg>
    </span>
  );
}

type JapaneseGardenPondBoxShape = { cx: number; cy: number; rx: number; ry: number };

function GardenKoiSet({ box, fish }: { box: JapaneseGardenPondBoxShape; fish: [string, number, number, number][] }) {
  return (
    <>
      {fish.map((k, index) => (
        <GardenKoi key={index} tone={k[0]} left={box.cx - box.rx + k[1] * box.rx * 2} top={box.cy - box.ry * 0.4 + k[2] * box.ry} dur={12 + index * 4} del={-index * 3} flip={k[3]} />
      ))}
    </>
  );
}

function JapaneseGardenWater({ stage, boxWidth, boxHeight }: { stage: number; boxWidth: number; boxHeight: number }) {
  const boxW = boxWidth || 390;
  const boxH = boxHeight || 205;
  const lower = stage >= 6;
  const frame = japaneseGardenWaterFrame(boxW, boxH);
  const effW = frame.effW;
  const effH = frame.effH;
  const offsetX = frame.offsetX;
  const offsetY = frame.offsetY;
  const X = (p: number) => offsetX + p * frame.scaleX;
  const Y = (p: number) => offsetY + p * frame.scaleY;
  const SX = (r: number) => r * frame.scaleX;
  const SY = (r: number) => r * frame.scaleY;
  const rim = gardenMix(JG_STONE, "black", 46);
  const deep = gardenMix(JG_WATER, "black", 18);
  const lit = gardenMix(JG_WATER, "white", 16);

  const CP = JG_FLOW.map((p) => [X(p[0]), Y(p[1])]);
  const bez = (u: number, i: number) => {
    const v = 1 - u;
    return v * v * v * CP[0][i] + 3 * v * v * u * CP[1][i] + 3 * v * u * u * CP[2][i] + u * u * u * CP[3][i];
  };
  const dbez = (u: number, i: number) => {
    const v = 1 - u;
    return 3 * v * v * (CP[1][i] - CP[0][i]) + 6 * v * u * (CP[2][i] - CP[1][i]) + 3 * u * u * (CP[3][i] - CP[2][i]);
  };
  const at = (u: number): [number, number] => [bez(u, 0), bez(u, 1)];
  const norm = (u: number): [number, number, number, number] => {
    const tx = dbez(u, 0);
    const ty = dbez(u, 1);
    const m = Math.hypot(tx, ty) || 1;
    return [-ty / m, tx / m, tx / m, ty / m];
  };
  const halfW = (u: number) => effW * (0.011 + 0.015 * u) + (u > 0.76 ? effW * 0.032 * Math.pow((u - 0.76) / 0.24, 2) : 0);
  const SN = 28;
  const offs = (sgn: number, k?: number): [number, number][] =>
    Array.from({ length: SN + 1 }, (_, index) => {
      const u = index / SN;
      const p = at(u);
      const n = norm(u);
      const wob = sgn ? Math.sin(u * 9.1 + (sgn > 0 ? 0 : 2.3)) * effW * 0.005 : 0;
      const h = (halfW(u) + wob) * (sgn || k || 0);
      return [p[0] + n[0] * h, p[1] + n[1] * h];
    });
  const poly = (pts: [number, number][]) => pts.map((p, index) => `${index ? "L" : "M"}${p[0].toFixed(1)} ${p[1].toFixed(1)}`).join(" ");
  const eL = offs(1);
  const eR = offs(-1);
  const river = `${poly(eL)} ${poly(eR.slice().reverse()).replace("M", "L")} Z`;
  const thread = (k: number) => poly(offs(0, k));

  const uB = 0.46;
  const nB = norm(uB);
  const cross = at(uB);
  const nx = nB[0];
  const ny = nB[1];
  const fx = nB[2];
  const fy = nB[3];
  const hwMid = halfW(uB);

  const ell = (o: JapaneseGardenPondBoxShape, grow: number) => <ellipse cx={X(o.cx)} cy={Y(o.cy)} rx={SX(o.rx) + grow} ry={SY(o.ry) + grow} />;

  const foot = hwMid + Math.max(20, effW * 0.07);
  const plank = Math.max(13, effW * 0.038);
  const clampY = (y: number) => Math.max(offsetY + plank, Math.min(offsetY + effH - plank * 1.2, y));
  const L: [number, number] = [cross[0] - nx * foot - (fx * plank) / 2, clampY(cross[1] - ny * foot - (fy * plank) / 2)];
  const R: [number, number] = [cross[0] + nx * foot - (fx * plank) / 2, clampY(cross[1] + ny * foot - (fy * plank) / 2)];
  const spanLen = Math.hypot(R[0] - L[0], R[1] - L[1]);
  const rise = (Math.max(6, effH * 0.075) * Math.abs(R[0] - L[0])) / Math.max(1, spanLen);
  const arc = (p: [number, number], q: [number, number], dy: number) => {
    const c: [number, number] = [(p[0] + q[0]) / 2, (p[1] + q[1]) / 2 - rise * 2 + dy];
    return { d: `M${p[0]} ${p[1] + dy} Q ${c[0]} ${c[1]} ${q[0]} ${q[1] + dy}`, c };
  };
  const nearArc = arc(L, R, 0);
  const farL: [number, number] = [L[0] + fx * plank, L[1] + fy * plank];
  const farR: [number, number] = [R[0] + fx * plank, R[1] + fy * plank];
  const farArc = arc(farL, farR, 0);
  const deckBody = `${nearArc.d} L ${farR[0]} ${farR[1]} Q ${farArc.c[0]} ${farArc.c[1]} ${farL[0]} ${farL[1]} Z`;
  const onArc = (p: [number, number], q: [number, number], c: [number, number], u: number): [number, number] => [
    (1 - u) * (1 - u) * p[0] + 2 * (1 - u) * u * c[0] + u * u * q[0],
    (1 - u) * (1 - u) * p[1] + 2 * (1 - u) * u * c[1] + u * u * q[1],
  ];
  const postH = Math.max(9, effH * 0.055);
  const railC: [number, number] = [nearArc.c[0], nearArc.c[1] - postH];
  const railTop = `M${L[0]} ${L[1] - postH} Q ${railC[0]} ${railC[1]} ${R[0]} ${R[1] - postH}`;

  /* koi/lily-pad/ripple overlay below is positioned with plain CSS percentages
     of the full container, so pond boxes need to be expressed in that same
     container-relative percentage space (not the raw 0-100 water-authoring
     units the SVG geometry above uses via X()/Y()). */
  const pctX = (p: number) => (X(p) / boxW) * 100;
  const pctY = (p: number) => (Y(p) / boxH) * 100;
  const pctRX = (r: number) => ((r * frame.scaleX) / boxW) * 100;
  const pctRY = (r: number) => ((r * frame.scaleY) / boxH) * 100;
  const toPondPct = (o: JapaneseGardenPondBoxShape): JapaneseGardenPondBoxShape => ({ cx: pctX(o.cx), cy: pctY(o.cy), rx: pctRX(o.rx), ry: pctRY(o.ry) });
  const upperPct = toPondPct(JG_UPPER);
  const lowerPct = toPondPct(JG_LOWER);

  return (
    <>
      <svg viewBox={`0 0 ${boxW} ${boxH}`} preserveAspectRatio="none" style={{ position: "absolute", left: 0, top: 0, width: "100%", height: "100%", zIndex: 24, pointerEvents: "none" }}>
        <defs>
          <linearGradient id="jgWaterFill" gradientUnits="userSpaceOnUse" x1={offsetX + effW * 0.5} y1={offsetY + effH * 0.55} x2={offsetX + effW * 0.72} y2={offsetY + effH}>
            <stop offset="0%" stopColor={lit} />
            <stop offset="55%" stopColor={JG_WATER} />
            <stop offset="100%" stopColor={deep} />
          </linearGradient>
          <clipPath id="jgWaterClip">
            {ell(JG_UPPER, 0)}
            {lower ? ell(JG_LOWER, 0) : null}
            {lower ? <path d={river} /> : null}
          </clipPath>
        </defs>
        <g fill={gardenMix(JG_SOIL, "black", 46)} stroke={gardenMix(JG_SOIL, "black", 46)} strokeWidth="7" strokeLinejoin="round" opacity="0.55">
          {ell(JG_UPPER, 3)}
          {lower ? ell(JG_LOWER, 3) : null}
          {lower ? <path d={river} /> : null}
        </g>
        <g fill={rim} stroke={rim} strokeWidth="3" strokeLinejoin="round">
          {ell(JG_UPPER, 1)}
          {lower ? ell(JG_LOWER, 1) : null}
          {lower ? <path d={river} /> : null}
        </g>
        <g fill="url(#jgWaterFill)">
          {ell(JG_UPPER, 0)}
          {lower ? ell(JG_LOWER, 0) : null}
          {lower ? <path d={river} /> : null}
        </g>
        <ellipse cx={X(JG_UPPER.cx - 3)} cy={Y(JG_UPPER.cy - 1.2)} rx={SX(JG_UPPER.rx) * 0.44} ry={SY(JG_UPPER.ry) * 0.34} fill="rgba(255,255,255,.16)" />
        {lower ? <ellipse cx={X(JG_LOWER.cx - 4)} cy={Y(JG_LOWER.cy - 1.4)} rx={SX(JG_LOWER.rx) * 0.4} ry={SY(JG_LOWER.ry) * 0.32} fill="rgba(255,255,255,.14)" /> : null}
        {lower ? (
          <g fill="none" strokeLinecap="round">
            <path d={poly(eL)} stroke="rgba(0,0,0,.26)" strokeWidth="1.5" />
            <path d={poly(eR)} stroke="rgba(255,255,255,.2)" strokeWidth="1.1" />
            {[
              [-0.5, 7.5, 0],
              [0, 6.2, -1.4],
              [0.52, 8.4, -2.8],
            ].map((t, index) => (
              <path
                key={`th${index}`}
                className="jg-flow"
                d={thread(t[0])}
                stroke="rgba(255,255,255,.3)"
                strokeWidth="1.1"
                strokeDasharray="6 17"
                style={{ "--jg-dur": `${t[1]}s`, "--jg-del": `${t[2]}s`, "--jg-off": -92 } as CSSProperties}
              />
            ))}
            {[0.2, 0.35, 0.52, 0.68].map((u, index) => {
              const p = at(u);
              const n = norm(u);
              const h = halfW(u) * 0.62;
              return (
                <path
                  key={`rf${index}`}
                  stroke="rgba(255,255,255,.15)"
                  strokeWidth="1"
                  d={`M${p[0] - n[0] * h} ${p[1] - n[1] * h} Q ${p[0] + n[2] * 3.5} ${p[1] + n[3] * 3.5} ${p[0] + n[0] * h} ${p[1] + n[1] * h}`}
                />
              );
            })}
          </g>
        ) : null}
        {lower
          ? (() => {
              const hd = at(0.05);
              const hn = norm(0.05);
              const hw = halfW(0.05);
              const mo = at(0.95);
              const mn = norm(0.95);
              const mw = halfW(0.95);
              const junctionArc = (p: [number, number], n: [number, number, number, number], h: number, d: number) =>
                `M${p[0] - n[0] * h} ${p[1] - n[1] * h} Q ${p[0] + n[2] * d} ${p[1] + n[3] * d} ${p[0] + n[0] * h} ${p[1] + n[1] * h}`;
              return (
                <g fill="none" clipPath="url(#jgWaterClip)">
                  <path d={junctionArc(hd, hn, hw, 4)} stroke="rgba(255,255,255,.45)" strokeWidth="1.3" />
                  {[0.55, 0.85, 1.15].map((k, index) => (
                    <path key={`dl${index}`} d={junctionArc(mo, mn, mw * k, 5 * k)} strokeWidth="1" stroke={`rgba(255,255,255,${(0.28 - index * 0.07).toFixed(2)})`} />
                  ))}
                </g>
              );
            })()
          : null}
        {lower
          ? (() => {
              const hd = at(0.05);
              const hn = norm(0.05);
              const hw = halfW(0.05);
              return (
                <g fill="none">
                  <ellipse cx={hd[0] - hn[0] * (hw + 5.2)} cy={hd[1] - hn[1] * (hw + 5.2)} rx="5" ry="3" fill={gardenMix(JG_STONE, "black", 22)} />
                  <ellipse cx={hd[0] + hn[0] * (hw + 3.4)} cy={hd[1] + hn[1] * (hw + 3.4)} rx="4.3" ry="2.6" fill={gardenMix(JG_STONE, "black", 34)} />
                </g>
              );
            })()
          : null}

        {lower ? (
          <g>
            <ellipse cx={L[0] + (fx * plank) / 2} cy={L[1] + (fy * plank) / 2 + 2} rx={plank * 0.8} ry={plank * 0.34} fill={gardenMix(JG_STONE, "black", 48)} />
            <ellipse cx={R[0] + (fx * plank) / 2} cy={R[1] + (fy * plank) / 2 + 2} rx={plank * 0.8} ry={plank * 0.34} fill={gardenMix(JG_STONE, "black", 48)} />
            <path d={deckBody} fill={gardenMix(JG_MAPLE, "black", 18)} />
            <path d={farArc.d} stroke={gardenMix(JG_MAPLE, "black", 48)} strokeWidth="3" fill="none" />
            <path d={nearArc.d} stroke={gardenMix(JG_MAPLE, "white", 26)} strokeWidth="1.6" fill="none" />
            {[0.12, 0.5, 0.88].map((u, index) => {
              const p = onArc(L, R, nearArc.c, u);
              return <line key={`pl${index}`} x1={p[0]} y1={p[1]} x2={p[0] + fx * plank} y2={p[1] + fy * plank} stroke={gardenMix(JG_MAPLE, "black", 36)} strokeWidth="1.1" opacity="0.7" />;
            })}
            {[0.06, 0.28, 0.5, 0.72, 0.94].map((u, index) => {
              const p = onArc(L, R, nearArc.c, u);
              return <rect key={index} x={p[0] - 1.9} y={p[1] - postH} width="3.8" height={postH + 1} rx="1.3" fill={gardenMix(JG_MAPLE, "black", 34)} />;
            })}
            <path d={railTop} stroke={gardenMix(JG_MAPLE, "black", 20)} strokeWidth="3.4" fill="none" strokeLinecap="round" />
            <path d={railTop} stroke={gardenMix(JG_MAPLE, "white", 24)} strokeWidth="1.2" fill="none" strokeLinecap="round" />
          </g>
        ) : null}
      </svg>

      <div style={{ position: "absolute", left: 0, top: 0, width: "100%", height: "100%", zIndex: 25, pointerEvents: "none" }}>
        <GardenKoiSet
          box={upperPct}
          fish={[
            ["oklch(0.72 0.15 45)", 0.3, 0.4, 0],
            ["oklch(0.9 0.02 60)", 0.64, 1, -6],
          ]}
        />
        {lower ? (
          <GardenKoiSet
            box={lowerPct}
            fish={[
              ["oklch(0.66 0.16 30)", 0.24, 0.35, -8],
              ["oklch(0.9 0.02 60)", 0.54, 1.1, 5],
              ["oklch(0.72 0.15 45)", 0.78, 0.5, -4],
            ]}
          />
        ) : null}
        {(
          [[upperPct, 0.14, 1, 7] as [JapaneseGardenPondBoxShape, number, number, number]].concat(
            lower
              ? [
                  [lowerPct, 0.12, 0.95, 9] as [JapaneseGardenPondBoxShape, number, number, number],
                  [lowerPct, 0.84, 1.15, 7] as [JapaneseGardenPondBoxShape, number, number, number],
                ]
              : [],
          )
        ).map((p, index) => (
          <span
            key={index}
            style={{
              position: "absolute",
              left: `${p[0].cx - p[0].rx + p[1] * p[0].rx * 2}%`,
              top: `${p[0].cy - p[0].ry * 0.3 + p[2] * p[0].ry}%`,
              width: p[3],
              height: p[3] * 0.62,
              borderRadius: "50%",
              background: gardenMix(JG_MOSS, "black", 12),
              opacity: 0.85,
            }}
          />
        ))}
        <span
          className="jg-ripple"
          style={{ position: "absolute", left: `${upperPct.cx + 2}%`, top: `${upperPct.cy + 0.6}%`, width: 14, height: 5, borderRadius: "50%", border: "1px solid rgba(255,255,255,.32)", "--jg-dur": "5s" } as CSSProperties}
        />
        {lower ? (
          <span
            className="jg-ripple"
            style={{
              position: "absolute",
              left: `${lowerPct.cx - 2}%`,
              top: `${lowerPct.cy + 0.6}%`,
              width: 16,
              height: 6,
              borderRadius: "50%",
              border: "1px solid rgba(255,255,255,.28)",
              "--jg-dur": "6.5s",
              "--jg-del": "-2s",
            } as CSSProperties}
          />
        ) : null}
      </div>
    </>
  );
}

function JapaneseGardenWidget({ appState, weeklyMinutes }: { appState: AppState; weeklyMinutes: number }) {
  const japaneseGardenStageNames = ["Bare Ground", "First Shoots", "Moss Takes Hold", "Tea House", "Bamboo Grove", "Koi Pond", "River & Bridge", "Full Garden", "Long Tended"];
  const japaneseGardenStageThresholds = [0, 45, 120, 240, 400, 600, 850, 1150, 1500];
  const stage = Math.max(0, japaneseGardenStageThresholds.filter((threshold) => weeklyMinutes >= threshold).length - 1);
  const wrapRef = useRef<HTMLDivElement | null>(null);
  const [containerWidth, setContainerWidth] = useState(380);
  const [containerHeight, setContainerHeight] = useState(205);
  const [tip, setTip] = useState<{ x: number; y: number; label?: string; sub?: string } | null>(null);
  const streak = useMemo(() => getStreakDays({ sessions: appState.sessions } as AppState), [appState.sessions]);
  const wide = containerWidth > 430;
  const plants = useMemo(
    () => placeJapaneseGardenItems(buildJapaneseGardenItems(appState.courses, appState.sessions, appState.tasks, stage), stage, wide, containerWidth, containerHeight),
    [appState.courses, appState.sessions, appState.tasks, stage, wide, containerWidth, containerHeight],
  );
  const fireflies = streak >= 5 ? Math.min(7, 2 + Math.floor(streak / 3)) : 0;
  const petals = stage >= 6 ? 7 : stage >= 5 ? 4 : 0;

  useLayoutEffect(() => {
    const element = wrapRef.current;
    if (!element || typeof ResizeObserver === "undefined") return undefined;
    const observer = new ResizeObserver(() => {
      const width = Math.round(element.clientWidth / 12) * 12;
      const height = Math.round(element.clientHeight);
      setContainerWidth((prev) => (prev === width ? prev : width));
      setContainerHeight((prev) => (prev === height ? prev : height));
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const showTip = (event: MouseEvent<HTMLDivElement>, plant: PlacedJapaneseGardenItem) => {
    const rect = wrapRef.current?.getBoundingClientRect();
    if (!rect) return;
    setTip({ x: Math.max(10, Math.min(rect.width - 10, event.clientX - rect.left)), y: Math.max(28, event.clientY - rect.top), label: plant.label, sub: plant.sub });
  };

  return (
    <div ref={wrapRef} className="garden-stage jg-stage" aria-label={`Japanese Garden of Knowledge: ${japaneseGardenStageNames[stage]}, ${formatMinutes(weeklyMinutes)} last 7 days`}>
      <div className="jg-moon" aria-hidden="true" />
      <svg className="jg-hills" preserveAspectRatio="none" viewBox="0 0 100 20" aria-hidden="true">
        <path d="M0 20 Q 22 3 44 11 Q 62 17 78 6 Q 90 -1 100 9 L 100 20 Z" />
      </svg>
      <div className={`gk-ground jg-ground${stage >= 2 ? " jg-ground-moss" : ""}`} aria-hidden="true">
        <svg preserveAspectRatio="none" viewBox="0 0 100 8"><path d="M0 8 Q 14 2.5 32 4.8 T 64 3.6 T 100 5.2 L 100 8 Z" /></svg>
      </div>

      {stage >= 5 ? <JapaneseGardenWater stage={stage} boxWidth={containerWidth} boxHeight={containerHeight} /> : null}

      {plants.map((plant, index) => {
        const Species = japaneseGardenSpecies[plant.kind];
        const rng = gardenRng(gardenHash(`${plant.id}-jdraw`));
        const depth = 0.62 + 0.38 * plant.t;
        const bed = containerWidth * (plant.cw || 0.125);
        const bottomPct = 1 + (1 - plant.t) * 55;
        const headroom = (containerHeight * (1 - bottomPct / 100) * 0.94) / 1.21;
        let width = Math.min(bed * 0.66 * (japaneseGardenBaseWidth[plant.kind] || 1) * depth * (0.8 + 0.25 * (plant.maturity || 0.5)) * plant.growth, headroom, containerHeight * 0.46);
        if (plant.low) width = Math.min(width, containerHeight * 0.17);
        const interactive = !plant.ambient;
        return (
          <div
            key={plant.id}
            className={`gk-plant-in${plant.fresh ? " gk-fresh" : ""}`}
            onMouseEnter={interactive ? (event) => showTip(event, plant) : undefined}
            onMouseMove={interactive ? (event) => showTip(event, plant) : undefined}
            onMouseLeave={interactive ? () => setTip(null) : undefined}
            style={{
              position: "absolute",
              left: `${plant.x * 100}%`,
              bottom: `${bottomPct}%`,
              width,
              transform: "translateX(-50%)",
              zIndex: Math.round(plant.t * 40) + 3,
              opacity: plant.ambient ? 0.62 + 0.32 * plant.t : 0.82 + 0.18 * plant.t,
              animationDelay: `${(index % 14) * 0.045}s`,
              pointerEvents: interactive ? "auto" : "none",
            }}
          >
            <span
              className={plant.ambient || index > 8 ? undefined : "gk-sway"}
              style={{ display: "block", "--gk-dur": `${5 + (gardenHash(plant.id) % 40) / 10}s`, "--gk-del": `${-(gardenHash(plant.id) % 60) / 10}s` } as CSSProperties}
            >
              <svg viewBox="0 0 48 58" aria-hidden="true">
                <Species rng={rng} color={plant.color} maturity={plant.maturity} wise={plant.wise} detail={plant.ambient ? 0 : 1} />
              </svg>
            </span>
          </div>
        );
      })}

      {Array.from({ length: petals }).map((_, index) => {
        const rng = gardenRng(gardenHash(`jg-petal-${index}`));
        return (
          <span
            key={index}
            className="jg-petal"
            style={{
              position: "absolute",
              left: `${10 + rng() * 80}%`,
              top: `${-4 + rng() * 20}%`,
              width: 5,
              height: 4,
              borderRadius: "60% 40% 60% 40%",
              background: JG_SAKURA,
              opacity: 0.85,
              zIndex: 44,
              pointerEvents: "none",
              "--jg-dur": `${7 + rng() * 6}s`,
              "--jg-del": `${-rng() * 8}s`,
            } as CSSProperties}
          />
        );
      })}

      {Array.from({ length: fireflies }).map((_, index) => {
        const rng = gardenRng(gardenHash(`jg-fly-${index}`));
        return <span key={index} className="gk-firefly" style={{ left: `${8 + rng() * 84}%`, top: `${14 + rng() * 36}%`, "--gk-dur": `${2.4 + rng() * 2.4}s`, "--gk-del": `${-rng() * 4}s` } as CSSProperties} />;
      })}

      <div className="gk-stage-dots" aria-hidden="true">{Array.from({ length: 9 }).map((_, index) => <span key={index} className={index < stage ? "active" : ""} />)}</div>
      <div className="gk-stats"><span className="serif">{japaneseGardenStageNames[stage]}</span><span className="mono">{formatMinutes(weeklyMinutes)} last 7 days</span></div>
      {stage === 0 && weeklyMinutes === 0 ? <p className="gk-empty mono">log a session to set the first stone</p> : null}
      {tip ? <div className="gk-tooltip" style={{ left: tip.x, top: tip.y - 14 }}><strong>{tip.label}</strong>{tip.sub ? <span>{tip.sub}</span> : null}</div> : null}
    </div>
  );
}

const calendarTimelineHeight = calendarTimelineHours.length * calendarTimelineHourHeight;

const themePalettes: { id: ThemePalette; name: string; desc: string; swatch: string }[] = [
  { id: "default", name: "Default", desc: "Editorial blue-grey with soft study accents.", swatch: "oklch(0.70 0.10 245)" },
  { id: "original", name: "Original", desc: "Slate, cyan ink, and coral-red accents.", swatch: "#e06c75" },
  { id: "midnight", name: "Midnight", desc: "GitHub-dark ink with a bright red accent.", swatch: "#f85149" },
  { id: "paper", name: "Paper", desc: "Warm cream paper with mustard ink.", swatch: "#c5ac4a" },
  { id: "cyberpunk", name: "Cyberpunk", desc: "Neon cyan, electric purple, and magenta.", swatch: "#e040fb" },
  { id: "retrowave", name: "Retrowave", desc: "Synthwave indigo, purple, and hot pink.", swatch: "#e94560" },
  { id: "forest", name: "Forest", desc: "Deep woodland greens for quiet focus.", swatch: "#7cb871" },
  { id: "ocean", name: "Ocean", desc: "Deep-sea navy with bright blue focus.", swatch: "#4facfe" },
  { id: "ume", name: "Ume", desc: "Plum-blossom aubergine and pink.", swatch: "#f5a0c0" },
  { id: "copper", name: "Copper", desc: "Burnished copper and espresso warmth.", swatch: "#d4764e" },
  { id: "organs", name: "Organs", desc: "Cream on near-black with oxblood red.", swatch: "#c83240" },
  { id: "lavender", name: "Lavender", desc: "Soft violet on lilac white.", swatch: "#9b6dcc" },
  { id: "gpt", name: "GPT", desc: "Monochrome graphite and grey.", swatch: "#949494" },
  { id: "claude", name: "Claude", desc: "Clay-orange on warm charcoal.", swatch: "#c6613f" },
  { id: "cute", name: "Cute", desc: "Kawaii pastel pinks.", swatch: "#ff6b9d" },
];

const appStyles: { id: AppStyle; name: string; desc: string; swatch: string }[] = [
  { id: "modern", name: "Modern", desc: "Current rounded study dashboard with palette themes.", swatch: "linear-gradient(135deg, #8fb4ff, #98c379)" },
  { id: "field-notebook", name: "Field Notebook", desc: "Paper, ink, course tabs, ruled ledgers, and study-circle social styling.", swatch: "linear-gradient(135deg, #fbf8f0 0 45%, #23211d 45% 55%, #9c5a34 55%)" },
  { id: "wabi-sabi", name: "Wabi-Sabi 侘寂", desc: "Quiet paper and ink: one thing at a time, a sidebar of kanji, and a circle with no ranks unless you want them.", swatch: "linear-gradient(135deg, #e9e5d8 0 45%, #4f6b4a 45% 80%, #b0472e 80%)" },
];

function isAppStyle(value: string | null): value is AppStyle {
  return appStyles.some((style) => style.id === value);
}

function isThemePalette(value: string | null): value is ThemePalette {
  return themePalettes.some((palette) => palette.id === value);
}

function loadThemePalette(): ThemePalette {
  const saved = localStorage.getItem(PALETTE_STORAGE_KEY);
  if (isThemePalette(saved)) return saved;
  if (saved === "parchment") return "paper";
  if (saved === "cosmic") return "retrowave";
  if (saved === "grove") return "forest";
  return "default";
}

function loadAppStyle(): AppStyle {
  const saved = localStorage.getItem(APP_STYLE_STORAGE_KEY);
  return isAppStyle(saved) ? saved : "modern";
}

function loadFieldDashboardLayout(): FieldDashboardLayout {
  return localStorage.getItem(FIELD_DASHBOARD_LAYOUT_KEY) === "full" ? "full" : "quiet";
}

function loadWabiCircleCompetitive(): boolean {
  return localStorage.getItem(WABI_CIRCLE_COMPETITIVE_KEY) === "true";
}

type GardenVariant = "western" | "japanese";

function loadGardenVariant(): GardenVariant {
  return localStorage.getItem(GARDEN_VARIANT_KEY) === "japanese" ? "japanese" : "western";
}

const dashboardWidgetIds: DashboardWidgetId[] = [
  "today",
  "urgentTasks",
  "weeklyFocus",
  "courseRadar",
  "examRunway",
  "garden",
  "stats",
];

const defaultCustomDashboardLayout: DashboardWidgetLayout[] = [
  { id: "today", width: "half" },
  { id: "urgentTasks", width: "half" },
  { id: "weeklyFocus", width: "full" },
  { id: "courseRadar", width: "half" },
  { id: "examRunway", width: "half" },
  { id: "garden", width: "third" },
  { id: "stats", width: "third" },
];

function isDashboardLayout(value: string | null): value is DashboardLayout {
  return value === "focus" || value === "cockpit" || value === "analyst" || value === "custom";
}

function isDashboardWidgetId(value: unknown): value is DashboardWidgetId {
  return typeof value === "string" && dashboardWidgetIds.includes(value as DashboardWidgetId);
}

function isDashboardWidgetWidth(value: unknown): value is DashboardWidgetWidth {
  return value === "full" || value === "half" || value === "third";
}

function loadDashboardLayout() {
  const saved = localStorage.getItem(DASHBOARD_LAYOUT_KEY);
  return isDashboardLayout(saved) ? saved : "cockpit";
}

function normalizeCustomDashboardLayout(value: unknown): DashboardWidgetLayout[] {
  if (!Array.isArray(value)) return defaultCustomDashboardLayout;

  const seen = new Set<DashboardWidgetId>();
  const safeLayout = value.reduce<DashboardWidgetLayout[]>((items, item) => {
    if (!item || typeof item !== "object") return items;
    const record = item as Record<string, unknown>;
    if (!isDashboardWidgetId(record.id) || seen.has(record.id)) return items;
    seen.add(record.id);
    items.push({ id: record.id, width: isDashboardWidgetWidth(record.width) ? record.width : "half" });
    return items;
  }, []);

  defaultCustomDashboardLayout.forEach((item) => {
    if (!seen.has(item.id)) safeLayout.push(item);
  });

  return safeLayout;
}

function loadCustomDashboardLayout() {
  try {
    return normalizeCustomDashboardLayout(JSON.parse(localStorage.getItem(CUSTOM_DASHBOARD_LAYOUT_KEY) ?? "null"));
  } catch {
    return defaultCustomDashboardLayout;
  }
}

type CourseDraft = {
  semesterId: string;
  name: string;
  targetGrade: string;
  color: string;
};

type TaskDraft = {
  semesterId: string;
  courseId: string;
  title: string;
  unitLabel: string;
  totalUnits: string;
  completedUnits: string;
  dueDate: string;
  priority: Priority;
  notes: string;
};

function inferUnitLabel(title: string) {
  const normalized = title.toLowerCase();
  if (normalized.includes("lecture")) return "Lecture";
  if (normalized.includes("sheet") || normalized.includes("exercise")) return "Sheet";
  if (normalized.includes("exam")) return "Exam";
  if (normalized.includes("chapter")) return "Chapter";
  if (normalized.includes("reading")) return "Reading";
  return "Unit";
}

function cleanUnitLabel(label: string, title: string) {
  return label.trim() || inferUnitLabel(title);
}

function formatUnitLabel(label: string, count: number) {
  const clean = label.trim() || "Unit";
  return count === 1 || clean.endsWith("s") ? clean.toLowerCase() : `${clean.toLowerCase()}s`;
}

function getNextUnitLabel(task: Task) {
  const next = Math.min(task.totalUnits, Math.max(1, Math.floor(task.completedUnits) + 1));
  const label = task.unitLabel || "Unit";
  return task.completedUnits >= task.totalUnits ? `All ${formatUnitLabel(label, task.totalUnits)} done` : `Next: ${label} ${next} of ${task.totalUnits}`;
}

type ExamDraft = {
  semesterId: string;
  courseId: string;
  title: string;
  examDate: string;
  weight: string;
  preparedness: string;
};

function formatClock(totalSeconds: number) {
  const seconds = Math.max(0, Math.floor(totalSeconds));
  const minutes = String(Math.floor(seconds / 60)).padStart(2, "0");
  const remaining = String(seconds % 60).padStart(2, "0");
  return `${minutes}:${remaining}`;
}

function formatSwissGrade(grade: number) {
  const fixed = grade.toFixed(2);
  if (fixed.endsWith("00")) return fixed.slice(0, -1);
  if (fixed.endsWith("0")) return fixed.slice(0, -1);
  return fixed;
}

function toggleId(list: string[], id: string) {
  return list.includes(id) ? list.filter((item) => item !== id) : [...list, id];
}

function localIsoDate(date = new Date()) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function parseCalendarDate(iso: string) {
  const [year, month, day] = iso.split("-").map(Number);
  return new Date(year, month - 1, day);
}

function addCalendarDays(date: Date, days: number) {
  const next = new Date(date);
  next.setDate(next.getDate() + days);
  return next;
}

function getMonday(date: Date) {
  const start = new Date(date);
  const day = start.getDay();
  const diff = day === 0 ? -6 : 1 - day;
  start.setDate(start.getDate() + diff);
  start.setHours(0, 0, 0, 0);
  return start;
}

function buildMonthDays(cursor: Date): CalendarDay[] {
  const monthStart = new Date(cursor.getFullYear(), cursor.getMonth(), 1);
  const gridStart = getMonday(monthStart);

  return Array.from({ length: 42 }, (_, index) => {
    const date = addCalendarDays(gridStart, index);
    return {
      date,
      iso: localIsoDate(date),
      inCurrentMonth: date.getMonth() === cursor.getMonth(),
    };
  });
}

function buildWeekDays(cursor: Date): CalendarDay[] {
  const weekStart = getMonday(cursor);
  return Array.from({ length: 7 }, (_, index) => {
    const date = addCalendarDays(weekStart, index);
    return {
      date,
      iso: localIsoDate(date),
      inCurrentMonth: true,
    };
  });
}

function formatCalendarTitle(cursor: Date, view: CalendarView) {
  if (view === "month") {
    return new Intl.DateTimeFormat("en", { month: "long", year: "numeric" }).format(cursor);
  }

  const weekStart = getMonday(cursor);
  const weekEnd = addCalendarDays(weekStart, 6);
  const sameYear = weekStart.getFullYear() === weekEnd.getFullYear();
  const startLabel = new Intl.DateTimeFormat("en", {
    month: "short",
    day: "numeric",
    year: sameYear ? undefined : "numeric",
  }).format(weekStart);
  const endLabel = new Intl.DateTimeFormat("en", { month: "short", day: "numeric", year: "numeric" }).format(weekEnd);
  return `${startLabel} - ${endLabel}`;
}

function carryOverCalendarEntries(state: AppState, today = localIsoDate()): AppState {
  let changed = false;
  const taskLookup = new Map(state.tasks.map((task) => [task.id, task]));
  const calendarEntries = state.calendarEntries.map((entry) => {
    if (entry.completed || entry.date >= today) return entry;

    const task = taskLookup.get(entry.taskId);
    if (!task) return entry;

    const nextDate = task.dueDate && task.dueDate < today ? task.dueDate : today;
    if (entry.date === nextDate) return entry;
    changed = true;
    return { ...entry, date: nextDate };
  });

  return changed ? { ...state, calendarEntries } : state;
}

function getCalendarEntryAmount(entry: CalendarEntry) {
  return entry.unitAmount ?? 1;
}

function getCompletedCalendarWholeUnits(entries: CalendarEntry[], taskId: string) {
  const amount = entries.reduce((sum, entry) => {
    if (!entry.completed || entry.taskId !== taskId) return sum;
    return sum + getCalendarEntryAmount(entry);
  }, 0);
  return Math.floor(amount + 0.0001);
}

function compareCalendarEntriesForUnits(a: CalendarEntry, b: CalendarEntry) {
  const dateCompare = a.date.localeCompare(b.date);
  if (dateCompare) return dateCompare;
  const timeCompare = (a.startTime ?? "").localeCompare(b.startTime ?? "");
  if (timeCompare) return timeCompare;
  return a.createdAt.localeCompare(b.createdAt);
}

function formatUnitAmount(amount: number) {
  if (amount === 1) return "1 unit";
  if (amount === 0.5) return "1/2 unit";
  if (amount === 0.25) return "1/4 unit";
  return `${Number.isInteger(amount) ? amount : amount.toFixed(2)} units`;
}

function timeToMinutes(time: string) {
  const [hours, minutes] = time.split(":").map(Number);
  if (!Number.isFinite(hours) || !Number.isFinite(minutes)) return 0;
  return hours * 60 + minutes;
}

function isValidCalendarTime(time: string) {
  if (!/^\d{2}:\d{2}$/.test(time)) return false;
  const [hours, minutes] = time.split(":").map(Number);
  return hours >= 0 && hours <= 23 && minutes >= 0 && minutes <= 59;
}

function snapCalendarMinutes(minutes: number) {
  return Math.round(minutes / calendarTimeStepMinutes) * calendarTimeStepMinutes;
}

function minutesToTime(totalMinutes: number) {
  const safeTotal = clamp(totalMinutes, 0, 23 * 60 + 59);
  const hours = Math.floor(safeTotal / 60);
  const minutes = safeTotal % 60;
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}`;
}

function addMinutesToTime(time: string, minutes: number) {
  return minutesToTime(clamp(timeToMinutes(time) + minutes, 0, 23 * 60 + 59));
}

function formatTimeRange(entry: CalendarEntry) {
  if (!entry.startTime) return "Unscheduled";
  return entry.endTime ? `${entry.startTime} - ${entry.endTime}` : entry.startTime;
}

function getTimeGreeting(date = new Date()) {
  const hour = date.getHours();
  if (hour >= 5 && hour < 11) return "Good morning";
  if (hour >= 11 && hour < 13) return "Good day";
  if (hour >= 13 && hour < 17) return "Good afternoon";
  if (hour >= 17 && hour < 22) return "Good evening";
  return "Good night";
}

function getTimerMinutes(timer: TimerState) {
  const activeSeconds = getTimerActiveSeconds(timer);
  if (activeSeconds <= 0) return 0;
  return Math.max(1, Math.round(activeSeconds / 60));
}

function keepTimerContext(timer: TimerState) {
  return {
    studyMinutes: timer.studyMinutes,
    breakMinutes: timer.breakMinutes,
    examMinutes: timer.examMinutes,
    semesterId: timer.semesterId,
    courseId: timer.courseId,
    taskId: timer.taskId,
    goal: timer.goal,
    learned: timer.learned,
    blocker: timer.blocker,
    nextStep: timer.nextStep,
    confidence: timer.confidence,
    mode: timer.mode,
    presetLabel: timer.presetLabel,
  };
}

function getErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return fallback;
}

function formatFileSize(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${value >= 10 || unitIndex === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unitIndex]}`;
}

function buildReferenceNoteMarkdown(course: Course) {
  return `# ${course.name} References

> Useful links, docs, recordings, exercises, and exam prep for ${course.name}.

## Official Links
- [Course page](https://example.com)
- [Moodle / LMS](https://example.com)

## Lecture Resources
- [Lecture recordings](https://example.com)
- [Slides folder](https://example.com)

## Exercises
- [Exercise sheets](https://example.com)
- [Solutions](https://example.com)

## Exam Prep
- [Past exams](https://example.com)
- [Formula sheet](https://example.com)
`;
}

function stripMarkdownFrontmatter(markdown: string) {
  return markdown.replace(/^---\n[\s\S]*?\n---\n+/, "");
}

function isExternalWebUrl(url: string) {
  return /^https?:\/\//i.test(url.trim());
}

function openExternalLink(url: string) {
  const target = url.trim();
  if (!isExternalWebUrl(target)) return;

  if (isTauriApp()) {
    void openUrl(target);
    return;
  }

  window.open(target, "_blank", "noreferrer");
}

function renderExternalLink(label: ReactNode, url: string, key: string) {
  return (
    <a
      key={key}
      href={url}
      onClick={(event: MouseEvent<HTMLAnchorElement>) => {
        event.preventDefault();
        openExternalLink(url);
      }}
      target="_blank"
      rel="noreferrer"
    >
      {label}
    </a>
  );
}

function renderInlineMarkdown(text: string, keyPrefix: string) {
  const parts = text.split(/(`[^`]+`|\*\*[^*]+\*\*|\*[^*]+\*|\[[^\]]+\]\([^)]+\)|https?:\/\/[^\s<)]+)/g).filter(Boolean);
  return parts.map((part, index) => {
    const key = `${keyPrefix}-${index}`;
    if (part.startsWith("`") && part.endsWith("`")) return <code key={key}>{part.slice(1, -1)}</code>;
    if (part.startsWith("**") && part.endsWith("**")) return <strong key={key}>{part.slice(2, -2)}</strong>;
    if (part.startsWith("*") && part.endsWith("*")) return <em key={key}>{part.slice(1, -1)}</em>;

    const link = part.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
    if (link) {
      return renderExternalLink(link[1], link[2], key);
    }

    if (isExternalWebUrl(part)) return renderExternalLink(part, part, key);

    return part;
  });
}

function renderMarkdownPreview(markdown: string) {
  const lines = markdown.split("\n");
  const elements: ReactNode[] = [];
  let listItems: string[] = [];
  let orderedItems: string[] = [];
  let codeLines: string[] = [];
  let inCode = false;

  function flushLists(key: string) {
    if (listItems.length) {
      elements.push(
        <ul key={`${key}-ul`}>
          {listItems.map((item, index) => <li key={index}>{renderInlineMarkdown(item, `${key}-ul-${index}`)}</li>)}
        </ul>,
      );
      listItems = [];
    }
    if (orderedItems.length) {
      elements.push(
        <ol key={`${key}-ol`}>
          {orderedItems.map((item, index) => <li key={index}>{renderInlineMarkdown(item, `${key}-ol-${index}`)}</li>)}
        </ol>,
      );
      orderedItems = [];
    }
  }

  lines.forEach((line, index) => {
    const key = `md-${index}`;
    const trimmed = line.trim();

    if (trimmed.startsWith("```")) {
      if (inCode) {
        elements.push(<pre key={key}><code>{codeLines.join("\n")}</code></pre>);
        codeLines = [];
        inCode = false;
      } else {
        flushLists(key);
        inCode = true;
      }
      return;
    }

    if (inCode) {
      codeLines.push(line);
      return;
    }

    if (!trimmed) {
      flushLists(key);
      return;
    }

    const heading = trimmed.match(/^(#{1,3})\s+(.+)$/);
    if (heading) {
      flushLists(key);
      const content = renderInlineMarkdown(heading[2], key);
      if (heading[1].length === 1) elements.push(<h1 key={key}>{content}</h1>);
      if (heading[1].length === 2) elements.push(<h2 key={key}>{content}</h2>);
      if (heading[1].length === 3) elements.push(<h3 key={key}>{content}</h3>);
      return;
    }

    if (trimmed.startsWith(">")) {
      flushLists(key);
      elements.push(<blockquote key={key}>{renderInlineMarkdown(trimmed.replace(/^>\s?/, ""), key)}</blockquote>);
      return;
    }

    const bullet = trimmed.match(/^[-*]\s+(.+)$/);
    if (bullet) {
      orderedItems = [];
      listItems.push(bullet[1]);
      return;
    }

    const ordered = trimmed.match(/^\d+\.\s+(.+)$/);
    if (ordered) {
      listItems = [];
      orderedItems.push(ordered[1]);
      return;
    }

    flushLists(key);
    elements.push(<p key={key}>{renderInlineMarkdown(trimmed, key)}</p>);
  });

  flushLists("end");
  if (inCode && codeLines.length) elements.push(<pre key="end-code"><code>{codeLines.join("\n")}</code></pre>);

  return elements;
}

function buildSessionFromTimer(timer: TimerState, endedAt: string, minutes: number, startedAt = timer.startedAt ?? endedAt): StudySession {
  return {
    id: makeId(),
    semesterId: timer.semesterId,
    courseId: timer.courseId,
    taskId: timer.taskId,
    kind: timer.phase === "exam" ? "exam" : "study",
    goal: timer.goal.trim(),
    learned: timer.learned.trim(),
    blocker: timer.blocker.trim(),
    nextStep: timer.nextStep.trim(),
    confidence: timer.confidence,
    startedAt,
    endedAt,
    minutes,
    presetLabel: timer.presetLabel,
  };
}

function nextLocalMidnightAfter(date: Date) {
  const next = new Date(date);
  next.setHours(24, 0, 0, 0);
  return next;
}

function buildSessionsFromTimerRange(timer: TimerState, endedAt: string) {
  const buckets = new Map<string, { startedAt: string; endedAt: string; seconds: number }>();
  closeTimerSegments(timer, endedAt).forEach((segment) => {
    let cursorMs = new Date(segment.startedAt).getTime();
    const endMs = new Date(segment.endedAt).getTime();
    if (!Number.isFinite(cursorMs) || !Number.isFinite(endMs) || endMs <= cursorMs) return;

    while (cursorMs < endMs) {
      const midnight = nextLocalMidnightAfter(new Date(cursorMs)).getTime();
      const segmentEndMs = Math.min(endMs, midnight);
      const bucketEndMs = segmentEndMs === midnight ? segmentEndMs - 1 : segmentEndMs;
      const startedAt = new Date(cursorMs).toISOString();
      const bucketEndedAt = new Date(bucketEndMs).toISOString();
      const key = isoDate(new Date(cursorMs));
      const current = buckets.get(key);
      const seconds = Math.max(0, (segmentEndMs - cursorMs) / 1000);
      buckets.set(key, current ? {
        startedAt: current.startedAt,
        endedAt: bucketEndedAt,
        seconds: current.seconds + seconds,
      } : { startedAt, endedAt: bucketEndedAt, seconds });
      cursorMs = segmentEndMs;
    }
  });

  return [...buckets.values()].map((bucket) => buildSessionFromTimer(timer, bucket.endedAt, Math.max(1, Math.round(bucket.seconds / 60)), bucket.startedAt));
}

/**
 * Study/exam time overlapping `(sinceIso, nowIso]`, drawn from already-finalized sessions plus
 * whatever's still open on the live timer (truncated to `nowIso` via `closeTimerSegments`, the
 * same helper crash-recovery already uses) — the candidate claim for offline-credit reconciliation.
 * The server clamps/plausibility-caps this independently; this is just "what could plausibly apply."
 */
function computeOfflineIntervals(state: AppState, sinceIso: string, nowIso: string) {
  const sinceMs = new Date(sinceIso).getTime();
  const fromHistory = state.sessions
    .filter((session) => (session.kind === "study" || session.kind === "exam") && new Date(session.endedAt).getTime() > sinceMs)
    .map((session) => ({ startedAt: session.startedAt, endedAt: session.endedAt }));
  const fromActiveTimer = state.timer.phase === "study" || state.timer.phase === "exam"
    ? closeTimerSegments(state.timer, nowIso).filter((segment) => new Date(segment.endedAt).getTime() > sinceMs)
    : [];
  return [...fromHistory, ...fromActiveTimer];
}

function getNewLifetimeTotals(state: AppState, sessions: StudySession[]) {
  return sessions
    .filter((session) => session.kind === "study" || session.kind === "exam")
    .reduce((totals, session) => ({
      minutes: totals.minutes + Math.max(0, Math.round(session.minutes)),
      sessions: totals.sessions + 1,
    }), { minutes: state.lifetimeStudyMinutes, sessions: state.lifetimeStudySessions });
}

function prependSessionsToState(state: AppState, sessions: StudySession[], postSession?: StudySession) {
  const socialState = postSession && state.social.autoPostSessions
    ? queueFeedPost(state, postSession, getSessionCourseName(state, postSession))
    : state;
  const lifetimeTotals = getNewLifetimeTotals(socialState, sessions);
  return {
    ...socialState,
    lifetimeStudyMinutes: lifetimeTotals.minutes,
    lifetimeStudySessions: lifetimeTotals.sessions,
    sessions: pruneSessionHistory([...sessions, ...socialState.sessions]),
  };
}

function parseSocialTimestamp(value: string | null | undefined) {
  if (!value) return null;
  const normalized = /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/.test(value) ? `${value.replace(" ", "T")}Z` : value;
  const timestamp = new Date(normalized).getTime();
  return Number.isNaN(timestamp) ? null : timestamp;
}

function formatProfileSeenAt(value: string) {
  const timestamp = parseSocialTimestamp(value);
  if (timestamp === null) return formatDate(value);

  const recent = Date.now() - timestamp < 2 * 24 * 60 * 60 * 1000;
  return new Intl.DateTimeFormat("en", {
    month: "short",
    day: "numeric",
    ...(recent ? { hour: "numeric", minute: "2-digit" } : {}),
  }).format(new Date(timestamp));
}

function formatFeedPostedAt(value: string) {
  const timestamp = parseSocialTimestamp(value);
  if (timestamp === null) return formatDate(value);

  return new Intl.DateTimeFormat("en", {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  }).format(new Date(timestamp));
}

const WABI_ATTENDANCE_WINDOW_MS = 48 * 60 * 60 * 1000;
// Must match VERIFIED_SESSION_HEARTBEAT_MS / VERIFIED_SESSION_GRACE_MS in cloudflare/src/index.ts —
// used client-side only to decide when a gap is worth reconciling, not as an authoritative bound
// (the server independently derives and enforces its own boundary).
const VERIFIED_SESSION_HEARTBEAT_MS = 15 * 60 * 1000;
const VERIFIED_SESSION_GRACE_MS = 5 * 60 * 1000;

function isRecentlyActive(value: string | null | undefined, maxAgeMs = 45 * 60 * 1000) {
  const timestamp = parseSocialTimestamp(value);
  return timestamp !== null && Date.now() - timestamp < maxAgeMs;
}

function pickFeedFallbackNote(seed: string) {
  const total = [...seed].reduce((sum, char) => sum + char.charCodeAt(0), 0);
  return feedFallbackNotes[total % feedFallbackNotes.length];
}

const FEED_IMAGE_MAX_BYTES = 5 * 1024 * 1024;
const FEED_IMAGE_MAX_DIMENSION = 1280;
const FEED_IMAGE_TYPES = new Set(["image/png", "image/jpeg", "image/webp", "image/gif"]);
const R2_OWNER_FRIEND_CODE = "ZRWL-WKNF";

interface PreparedFeedImage {
  blob: Blob;
  previewUrl: string;
  name: string;
}

interface FeedPollDraft {
  question: string;
  multiple: boolean;
  options: string[];
}

const emptyFeedPollDraft = (): FeedPollDraft => ({ question: "", multiple: false, options: ["", ""] });
const MAX_FEED_POLL_OPTIONS = 12;

function canvasToBlob(canvas: HTMLCanvasElement, type: string, quality: number) {
  return new Promise<Blob>((resolve, reject) => {
    canvas.toBlob((blob) => blob ? resolve(blob) : reject(new Error("Could not prepare image.")), type, quality);
  });
}

async function prepareFeedImage(file: File): Promise<PreparedFeedImage> {
  if (!FEED_IMAGE_TYPES.has(file.type)) throw new Error("Use PNG, JPEG, WebP, or GIF images.");
  if (file.size > FEED_IMAGE_MAX_BYTES) throw new Error("Image is too large. Use an image under 5 MB.");
  if (file.type === "image/gif") {
    return { blob: file, previewUrl: URL.createObjectURL(file), name: file.name };
  }

  const bitmap = await createImageBitmap(file);
  try {
    const scale = Math.min(1, FEED_IMAGE_MAX_DIMENSION / Math.max(bitmap.width, bitmap.height));
    const width = Math.max(1, Math.round(bitmap.width * scale));
    const height = Math.max(1, Math.round(bitmap.height * scale));
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Could not prepare image.");
    context.drawImage(bitmap, 0, 0, width, height);
    const blob = await canvasToBlob(canvas, "image/webp", 0.82);
    if (blob.size > FEED_IMAGE_MAX_BYTES) throw new Error("Image is still too large after compression.");
    return { blob, previewUrl: URL.createObjectURL(blob), name: file.name };
  } finally {
    bitmap.close();
  }
}

const AVATAR_SOURCE_IMAGE_MAX_BYTES = 1024 * 1024;
const AVATAR_IMAGE_MAX_BYTES = 96 * 1024;
const AVATAR_IMAGE_MAX_DIMENSION = 160;
const AVATAR_IMAGE_COMPRESSION_ATTEMPTS = [
  { dimension: AVATAR_IMAGE_MAX_DIMENSION, quality: 0.72 },
  { dimension: AVATAR_IMAGE_MAX_DIMENSION, quality: 0.58 },
  { dimension: AVATAR_IMAGE_MAX_DIMENSION, quality: 0.45 },
  { dimension: 128, quality: 0.45 },
];
const AVATAR_IMAGE_TYPES = new Set(["image/png", "image/jpeg", "image/webp"]);
const AVATAR_CROP_VIEWPORT_PX = 300;
const AVATAR_CROP_MAX_ZOOM = 4;

interface AvatarCropSource {
  url: string;
  width: number;
  height: number;
  name: string;
}

interface AvatarCropState {
  x: number;
  y: number;
  zoom: number;
}

function blobToDataUrl(blob: Blob) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error("Could not read photo."));
    reader.readAsDataURL(blob);
  });
}

async function dataUrlToBlob(dataUrl: string) {
  const response = await fetch(dataUrl);
  return response.blob();
}

function loadImageElement(dataUrl: string) {
  return new Promise<HTMLImageElement>((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("Could not load photo."));
    image.src = dataUrl;
  });
}

function clampAvatarCrop(crop: AvatarCropState, source: AvatarCropSource): AvatarCropState {
  const scale = Math.max(AVATAR_CROP_VIEWPORT_PX / source.width, AVATAR_CROP_VIEWPORT_PX / source.height) * crop.zoom;
  const halfW = AVATAR_CROP_VIEWPORT_PX / (scale * 2 * source.width);
  const halfH = AVATAR_CROP_VIEWPORT_PX / (scale * 2 * source.height);
  return {
    ...crop,
    x: Math.min(1 - halfW, Math.max(halfW, crop.x)),
    y: Math.min(1 - halfH, Math.max(halfH, crop.y)),
  };
}

async function cropAvatarToDataUrl(source: AvatarCropSource, crop: AvatarCropState): Promise<string> {
  const image = await loadImageElement(source.url);
  const scale = Math.max(AVATAR_CROP_VIEWPORT_PX / source.width, AVATAR_CROP_VIEWPORT_PX / source.height) * crop.zoom;
  const cropSize = AVATAR_CROP_VIEWPORT_PX / scale;
  const cropX = crop.x * source.width - cropSize / 2;
  const cropY = crop.y * source.height - cropSize / 2;
  const canvas = document.createElement("canvas");
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Could not prepare photo.");
  context.imageSmoothingEnabled = true;
  context.imageSmoothingQuality = "high";

  for (const attempt of AVATAR_IMAGE_COMPRESSION_ATTEMPTS) {
    canvas.width = attempt.dimension;
    canvas.height = attempt.dimension;
    context.clearRect(0, 0, attempt.dimension, attempt.dimension);
    context.drawImage(image, cropX, cropY, cropSize, cropSize, 0, 0, attempt.dimension, attempt.dimension);
    const blob = await canvasToBlob(canvas, "image/webp", attempt.quality);
    if (blob.size <= AVATAR_IMAGE_MAX_BYTES) return blobToDataUrl(blob);
  }

  throw new Error("Photo is still too large after compression. Try a simpler or smaller photo.");
}

function formatBytes(value: number) {
  if (value >= 1024 * 1024 * 1024) return `${(value / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  if (value >= 1024 * 1024) return `${Math.round(value / (1024 * 1024))} MB`;
  if (value >= 1024) return `${Math.round(value / 1024)} KB`;
  return `${value} B`;
}

function formatCompactNumber(value: number) {
  return new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

function buildFeedPostFromSession(session: StudySession, state: AppState, courseName: string, note?: string): SocialFeedPost {
  const subject = courseName || session.goal || (session.kind === "exam" ? "Exam session" : "Study session");
  const detail = `${formatMinutes(session.minutes)} · ${session.presetLabel || (session.kind === "exam" ? "Exam" : "Focus")}`;
  return {
    id: session.id,
    userId: state.social.userId,
    displayName: state.social.displayName,
    friendCode: state.social.friendCode,
    type: "session",
    subject,
    detail,
    note: (note ?? "").trim() || pickFeedFallbackNote(session.id),
    icon: session.kind === "exam" ? "⚔" : "✦",
    minutes: session.minutes,
    presetLabel: session.presetLabel,
    createdAt: session.endedAt,
    isSelf: true,
    reactions: { fire: 0, brain: 0, clap: 0 },
    reacted: {},
  };
}

function prepareFeedPollDraft(draft: FeedPollDraft) {
  const question = draft.question.trim().replace(/\s+/g, " ").slice(0, 180);
  const seen = new Set<string>();
  const options = draft.options.flatMap((option) => {
    const text = option.trim().replace(/\s+/g, " ").slice(0, 100);
    const key = text.toLocaleLowerCase();
    if (!text || seen.has(key)) return [];
    seen.add(key);
    return [{ id: makeId(), text, votes: 0, selected: false }];
  }).slice(0, MAX_FEED_POLL_OPTIONS);
  if (!question && options.length === 0) return null;
  if (!question || options.length < 2) throw new Error("A poll needs a question and at least two different options.");
  return { question, multiple: draft.multiple, options, totalVotes: 0 };
}

function getSessionCourseName(state: AppState, session: StudySession) {
  return state.courses.find((course) => course.id === session.courseId)?.name ?? "";
}

function queueFeedPost(state: AppState, session: StudySession, courseName: string, note?: string, poll?: SocialFeedPost["poll"]) {
  const post = { ...buildFeedPostFromSession(session, state, courseName, note), poll: poll ?? null };
  const alreadyQueued = state.social.pendingFeedPosts.some((item) => item.id === post.id);
  const cachedLocally = [...state.social.cachedFeeds.global, ...state.social.cachedFeeds.friends].some((item) => item.id === post.id);
  if (alreadyQueued || cachedLocally) return state;
  return {
    ...state,
    social: {
      ...state.social,
      pendingFeedPosts: [post, ...state.social.pendingFeedPosts].slice(0, 25),
      cachedFeeds: {
        global: [post, ...state.social.cachedFeeds.global].slice(0, 50),
        friends: [post, ...state.social.cachedFeeds.friends].slice(0, 50),
      },
    },
  };
}

function pruneSessionHistory(sessions: StudySession[], today = new Date()) {
  const cutoff = new Date(today);
  cutoff.setDate(cutoff.getDate() - SESSION_HISTORY_DAYS);
  cutoff.setHours(0, 0, 0, 0);

  return sessions
    .filter((session) => new Date(session.endedAt) >= cutoff)
    .slice(0, SESSION_HISTORY_MAX);
}

const focusMilestones = [
  { hours: 10, label: "Seed Fossil", desc: "10 hours of study unearthed" },
  { hours: 25, label: "Shell Fragment", desc: "25 hours - patterns forming" },
  { hours: 50, label: "Ammonite", desc: "50 hours - taking shape" },
  { hours: 100, label: "Crystal Cluster", desc: "100 hours crystallized" },
  { hours: 250, label: "Complete Specimen", desc: "250 hours - a rare find" },
  { hours: 500, label: "Ancient Artifact", desc: "500 hours - legendary" },
  { hours: 1000, label: "Golden Record", desc: "1000 hours - transcendent" },
];

type FocusRange = "week" | 7 | 14 | 30 | 60 | 365;
type FocusTip = { idx: number; x: number; y: number };

function fossilRand(seed: number) {
  let s = Math.imul(seed | 0, 2654435761);
  s = Math.imul((s >>> 16) ^ s, 0x45d9f3b);
  s = Math.imul((s >>> 16) ^ s, 0x45d9f3b);
  return (((s >>> 16) ^ s) >>> 0) / 4294967296;
}

function fossilDateLabel(date: string) {
  return new Intl.DateTimeFormat("en-GB", { weekday: "short", day: "numeric", month: "short" }).format(new Date(`${date}T00:00:00`));
}

function StrataIcon({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 14 14" fill="none" aria-hidden="true">
      <rect x="1" y="1.5" width="12" height="2.8" rx="1.2" fill="currentColor" opacity="0.3" />
      <rect x="1" y="5.5" width="12" height="2.8" rx="1.2" fill="currentColor" opacity="0.5" />
      <rect x="1" y="9.5" width="12" height="2.8" rx="1.2" fill="currentColor" opacity="0.75" />
    </svg>
  );
}

function FossilMilestoneIcon({ hours, size = 14 }: { hours: number; size?: number }) {
  const style = { width: size, height: size, display: "block" };
  if (hours <= 10) return (
    <svg viewBox="0 0 16 16" fill="none" style={style} aria-hidden="true">
      <circle cx="8" cy="8" r="5" stroke="currentColor" strokeWidth="1.4" />
      <circle cx="8" cy="8" r="2" fill="currentColor" />
    </svg>
  );
  if (hours <= 25) return (
    <svg viewBox="0 0 16 16" fill="none" style={style} aria-hidden="true">
      <path d="M10.5 3C7 2.5 4 6 4 9.5S6 14 8 14s4-2 4-5.5S11.5 3 10.5 3z" stroke="currentColor" strokeWidth="1.3" />
      <path d="M6.5 9.5c1-2 2.5-3 3.5-2.5" stroke="currentColor" strokeWidth="0.9" opacity="0.5" />
    </svg>
  );
  if (hours <= 50) return (
    <svg viewBox="0 0 16 16" fill="none" style={style} aria-hidden="true">
      <circle cx="8" cy="8" r="5.5" stroke="currentColor" strokeWidth="1.2" />
      <path d="M8 5a3 3 0 1 0 0 6" stroke="currentColor" strokeWidth="1.1" />
      <circle cx="8" cy="8" r="1.3" fill="currentColor" />
    </svg>
  );
  if (hours <= 100) return (
    <svg viewBox="0 0 16 16" fill="none" style={style} aria-hidden="true">
      <path d="M8 1L12.5 6 8 15 3.5 6z" stroke="currentColor" strokeWidth="1.3" />
      <path d="M3.5 6h9" stroke="currentColor" strokeWidth="0.9" opacity="0.6" />
    </svg>
  );
  return (
    <svg viewBox="0 0 16 16" fill="none" style={style} aria-hidden="true">
      <path d="M8 1.5l2 4.2h4.5l-3.5 2.8 1.2 4.3L8 10.5l-4.2 2.3 1.2-4.3L1.5 5.7H6z" stroke="currentColor" strokeWidth="1.1" fill="currentColor" fillOpacity="0.2" />
    </svg>
  );
}

async function startWindowDrag() {
  if (!isTauriApp()) return;
  await getCurrentWindow().startDragging();
}

async function minimizeWindow() {
  if (!isTauriApp()) return;
  await getCurrentWindow().minimize();
}

async function toggleMaximizeWindow() {
  if (!isTauriApp()) return;
  await getCurrentWindow().toggleMaximize();
}

async function toggleFullscreenWindow() {
  if (!isTauriApp()) return;
  const window = getCurrentWindow();
  await window.setFullscreen(!(await window.isFullscreen()));
}

async function closeWindow() {
  if (!isTauriApp()) return;
  await getCurrentWindow().close();
}

function getInitials(name: string) {
  const initials = name
    .trim()
    .split(/[\s_-]+/)
    .map((part) => part[0])
    .join("")
    .slice(0, 2)
    .toUpperCase();
  return initials || "ST";
}

function getFirstAvatarLetter(name: string) {
  return (name.trim()[0] || "S").toUpperCase();
}

function getAvatarDisplayName(avatar: SocialAvatar | undefined, name: string) {
  if (!avatar) return getInitials(name);
  if (avatar.kind === "icon") return avatar.icon;
  if (avatar.kind === "photo") return avatar.url ? "" : getInitials(name);
  return avatar.letter || getFirstAvatarLetter(name);
}

function getAvatarClass(avatar: SocialAvatar | undefined) {
  if (!avatar) return "";
  if (avatar.kind === "icon") return "arena-avatar--icon";
  if (avatar.kind === "photo") return avatar.url ? "arena-avatar--photo" : "";
  return `arena-avatar--letter arena-avatar--letter-${avatar.style}`;
}

function getArenaHue(name: string) {
  return [...name].reduce((sum, character) => sum + character.charCodeAt(0), 0) % 360;
}

function ArenaBg() {
  return (
    <div className="arena-bg" aria-hidden="true">
      <div className="arena-bg-orb arena-bg-orb--1" />
      <div className="arena-bg-orb arena-bg-orb--2" />
      <div className="arena-bg-orb arena-bg-orb--3" />
    </div>
  );
}

function ArenaAvatar({ name, avatar, self = false, size = "md" }: { name: string; avatar?: SocialAvatar; self?: boolean; size?: "sm" | "md" | "lg" }) {
  return (
    <span
      className={`arena-avatar arena-avatar--${size} ${getAvatarClass(avatar)} ${self ? "arena-avatar--self" : ""}`}
      style={{ "--avatar-hue": getArenaHue(name) } as CSSProperties}
      aria-hidden="true"
    >
      {avatar?.kind === "photo" && avatar.url ? (
        <img className="arena-avatar-photo" src={avatar.url} alt="" draggable={false} />
      ) : (
        getAvatarDisplayName(avatar, name)
      )}
    </span>
  );
}

function ArenaRankBadge({ rank, large = false }: { rank: number; large?: boolean }) {
  return (
    <span className={`arena-rank-badge arena-rank-badge--${rank <= 3 ? rank : "plain"} ${large ? "arena-rank-badge--lg" : ""}`}>
      {rank === 1 ? "1" : rank === 2 ? "2" : rank === 3 ? "3" : `#${rank}`}
    </span>
  );
}

function ArenaLeaderboardRow({ entry, selfAvatar, onProfile }: { entry: SocialLeaderboardEntry; selfAvatar?: SocialAvatar; onProfile?: () => void }) {
  return (
    <div className={`arena-lb-row ${entry.isSelf ? "arena-lb-row--self" : ""}`} onClick={onProfile} role={onProfile ? "button" : undefined} tabIndex={onProfile ? 0 : undefined} onKeyDown={onProfile ? (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onProfile(); } } : undefined}>
      <ArenaRankBadge rank={entry.rank} />
      <ArenaAvatar name={entry.displayName} avatar={entry.isSelf ? selfAvatar : entry.avatar} self={entry.isSelf} size="sm" />
      <div className="arena-lb-person">
        <strong>{entry.displayName}{entry.isSelf ? " (You)" : ""}</strong>
        <span>{entry.friendCode}</span>
      </div>
      <div className="arena-lb-score">
        <strong>{formatMinutes(entry.minutes)}</strong>
        <span>{entry.sessions} sess.</span>
      </div>
    </div>
  );
}

function SquadArenaRow({ entry, period, isSelf, onOpen }: { entry: SocialSquadScoreEntry; period: SocialSquadScorePeriod; isSelf: boolean; onOpen?: () => void }) {
  return (
    <div className={`squad-score-row ${isSelf ? "squad-score-row--self" : ""}`} onClick={onOpen} role={onOpen ? "button" : undefined} tabIndex={onOpen ? 0 : undefined} onKeyDown={onOpen ? (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onOpen(); } } : undefined}>
      <ArenaRankBadge rank={entry.rank} />
      <div className="squad-score-main">
        <strong>{entry.squadName}{isSelf ? " (Your squad)" : ""}</strong>
        <span>{entry.isPrivate ? "Private" : "Public"} · {entry.memberCount} members · avg {formatMinutes(Math.round(entry.averageMinutes))}</span>
      </div>
      <div className="squad-score-points">
        <strong>{period === "daily" ? formatMinutes(Math.round(entry.averageMinutes)) : `${entry.points} pts`}</strong>
        <span>{period === "daily" ? `${entry.points} pts if day ends now` : `${entry.scoredDays ?? 0} scored days`}</span>
      </div>
    </div>
  );
}

function SummaryPdfViewer({ vaultPath, path, title }: { vaultPath: string; path: string; title: string }) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const renderTaskRef = useRef<{ cancel: () => void } | null>(null);
  const [documentProxy, setDocumentProxy] = useState<PDFDocumentProxy | null>(null);
  const [pageNumber, setPageNumber] = useState(1);
  const [pageCount, setPageCount] = useState(0);
  const [scale, setScale] = useState(1.25);
  const [loading, setLoading] = useState(false);
  const [rendering, setRendering] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let loadingTask: ReturnType<PdfJsModule["getDocument"]> | null = null;
    setLoading(true);
    setError(null);
    setDocumentProxy(null);
    setPageNumber(1);
    setPageCount(0);

    void Promise.all([
      import("pdfjs-dist"),
      import("pdfjs-dist/build/pdf.worker.mjs?url"),
      readSummaryPdf(vaultPath, path),
    ])
      .then(([pdfjsLib, workerModule, bytes]) => {
        if (cancelled) return null;
        pdfjsLib.GlobalWorkerOptions.workerSrc = workerModule.default;
        loadingTask = pdfjsLib.getDocument({ data: new Uint8Array(bytes) });
        return loadingTask.promise;
      })
      .then((pdf) => {
        if (!pdf) return;
        return pdf;
      })
      .then((pdf) => {
        if (!pdf) return;
        if (cancelled) {
          void pdf.cleanup();
          return;
        }
        setDocumentProxy(pdf);
        setPageCount(pdf.numPages);
      })
      .catch((loadError: unknown) => {
        if (!cancelled) setError(getErrorMessage(loadError, "Could not load this PDF."));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
      renderTaskRef.current?.cancel();
      renderTaskRef.current = null;
      void loadingTask?.destroy();
    };
  }, [path, vaultPath]);

  useEffect(() => {
    if (!documentProxy || !canvasRef.current) return undefined;
    let cancelled = false;
    setRendering(true);
    setError(null);
    renderTaskRef.current?.cancel();

    void documentProxy.getPage(pageNumber)
      .then((page) => {
        if (cancelled || !canvasRef.current) return;
        const canvas = canvasRef.current;
        const context = canvas.getContext("2d");
        if (!context) throw new Error("Could not prepare PDF canvas.");

        const viewport = page.getViewport({ scale });
        const pixelRatio = window.devicePixelRatio || 1;
        canvas.width = Math.floor(viewport.width * pixelRatio);
        canvas.height = Math.floor(viewport.height * pixelRatio);
        canvas.style.width = `${viewport.width}px`;
        canvas.style.height = `${viewport.height}px`;
        context.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
        context.clearRect(0, 0, viewport.width, viewport.height);

        const renderTask = page.render({ canvas, canvasContext: context, viewport });
        renderTaskRef.current = renderTask;
        return renderTask.promise;
      })
      .catch((renderError: unknown) => {
        if (cancelled) return;
        if (renderError instanceof Error && renderError.name === "RenderingCancelledException") return;
        setError(getErrorMessage(renderError, "Could not render this PDF page."));
      })
      .finally(() => {
        if (!cancelled) setRendering(false);
      });

    return () => {
      cancelled = true;
      renderTaskRef.current?.cancel();
      renderTaskRef.current = null;
    };
  }, [documentProxy, pageNumber, scale]);

  return (
    <div className="summary-pdf-viewer">
      <div className="summary-pdf-controls">
        <button type="button" className="ghost-button small-button" onClick={() => setPageNumber((current) => Math.max(1, current - 1))} disabled={!documentProxy || pageNumber <= 1}>Prev</button>
        <span>Page {pageCount ? pageNumber : "-"} / {pageCount || "-"}</span>
        <button type="button" className="ghost-button small-button" onClick={() => setPageNumber((current) => Math.min(pageCount, current + 1))} disabled={!documentProxy || pageNumber >= pageCount}>Next</button>
        <button type="button" className="ghost-button small-button" onClick={() => setScale((current) => Math.max(0.7, Number((current - 0.15).toFixed(2))))} disabled={!documentProxy}>-</button>
        <span>{Math.round(scale * 100)}%</span>
        <button type="button" className="ghost-button small-button" onClick={() => setScale((current) => Math.min(2.5, Number((current + 0.15).toFixed(2))))} disabled={!documentProxy}>+</button>
      </div>
      <div className="summary-pdf-canvas-wrap" aria-busy={loading || rendering}>
        {loading ? <p className="summary-pdf-status">Loading PDF...</p> : null}
        {error ? <p className="summary-pdf-error">{error}</p> : null}
        <canvas ref={canvasRef} aria-label={title} />
        {rendering && !loading ? <p className="summary-pdf-status floating">Rendering page...</p> : null}
      </div>
    </div>
  );
}

function GuidedTourOverlay({ help, stepIndex, onBack, onNext, onClose }: { help: PageHelp; stepIndex: number; onBack: () => void; onNext: () => void; onClose: () => void }) {
  const step = help.tour[stepIndex];
  const [rect, setRect] = useState<DOMRect | null>(null);

  useLayoutEffect(() => {
    if (!step) return undefined;

    let frame = 0;
    const updateRect = () => {
      const target = document.querySelector<HTMLElement>(`[data-tour="${step.target}"]`);
      if (!target) {
        setRect(null);
        return;
      }
      frame = window.requestAnimationFrame(() => setRect(target.getBoundingClientRect()));
    };

    document.querySelector<HTMLElement>(`[data-tour="${step.target}"]`)?.scrollIntoView({ block: "center", inline: "center", behavior: "smooth" });
    updateRect();
    window.addEventListener("resize", updateRect);
    window.addEventListener("scroll", updateRect, true);
    return () => {
      window.cancelAnimationFrame(frame);
      window.removeEventListener("resize", updateRect);
      window.removeEventListener("scroll", updateRect, true);
    };
  }, [step]);

  if (!step) return null;

  const highlightStyle = rect
    ? ({ top: rect.top - 8, left: rect.left - 8, width: rect.width + 16, height: rect.height + 16 } as CSSProperties)
    : undefined;
  const bubbleStyle = rect
    ? ({
      top: Math.min(window.innerHeight - 220, Math.max(18, rect.bottom + 18)),
      left: Math.min(window.innerWidth - 340, Math.max(18, rect.left)),
    } as CSSProperties)
    : undefined;
  const arrowStyle = rect
    ? ({ top: rect.bottom + 2, left: Math.min(window.innerWidth - 42, Math.max(26, rect.left + rect.width / 2)) } as CSSProperties)
    : undefined;

  return (
    <div className="tour-layer" role="dialog" aria-modal="true" aria-label={`${help.title} tutorial`}>
      <div className="tour-dim" />
      {rect ? <div className="tour-highlight" style={highlightStyle} /> : null}
      {rect ? <div className="tour-arrow" style={arrowStyle} /> : null}
      <section className="tour-bubble" style={bubbleStyle}>
        <div className="tour-progress">Step {stepIndex + 1} of {help.tour.length}</div>
        <h3>{step.title}</h3>
        <p>{step.body}</p>
        {!rect ? <small className="tour-missing">This feature is not visible right now. Try opening the relevant section, or continue.</small> : null}
        <div className="tour-actions">
          <button type="button" className="ghost-button small-button" onClick={onClose}>Finish</button>
          <button type="button" className="ghost-button small-button" onClick={onBack} disabled={stepIndex === 0}>Back</button>
          <button type="button" onClick={onNext}>{stepIndex === help.tour.length - 1 ? "Done" : "Next"}</button>
        </div>
      </section>
    </div>
  );
}

function HelpExampleCards({ examples }: { examples: HelpExample[] }) {
  const heading = examples[0]?.heading ?? "Examples";
  return (
    <div className="planner-unit-examples">
      <div className="help-example-heading">
        <span>{heading}</span>
        <hr />
      </div>
      <div className="hp-stack">
        {examples.map((example) => {
          const progress = Math.round((example.done / example.total) * 100);
          return (
            <article key={example.task} className={`hp-card ${example.tone}`}>
              <div className="hp-head">
                <span className="hp-task">{example.task}</span>
                <span className="hp-unit">{example.unitLabel.toLowerCase()} · <b>{example.next}</b> next · {example.total} total</span>
              </div>
              <div className="hp-row" aria-label={`${example.done} completed, ${example.next} next, ${example.total} total`}>
                {Array.from({ length: example.total }, (_, index) => {
                  const unit = index + 1;
                  const stateClass = unit <= example.done ? "done" : unit === example.next ? "next" : "";
                  return <span key={unit} className={`hp-cell ${stateClass}`}>{unit}</span>;
                })}
              </div>
              <p className="hp-caption">{example.caption} <span aria-hidden="true">-&gt;</span> <b>{example.result}</b></p>
              <div className="hp-progress" aria-label={`${progress}% complete`}>
                <span className="hp-pct">{progress}%</span>
                <div className="hp-track"><div className="hp-fill" style={{ width: `${progress}%` }} /></div>
              </div>
            </article>
          );
        })}
      </div>
    </div>
  );
}

const TIMER_ONLY_SECTION: ReadonlySet<PersistSection> = new Set(["timer"]);
const ALL_PERSIST_SECTIONS: ReadonlySet<PersistSection> = new Set(["timer", "social", "core"]);

function App() {
  const [state, setState] = useState<AppState>(() => {
    const loaded = loadAppState();
    return { ...loaded, sessions: pruneSessionHistory(loaded.sessions) };
  });
  const [theme, setTheme] = useState(() => localStorage.getItem("study-tracker-theme") || "dark");
  const [palette, setPalette] = useState<ThemePalette>(loadThemePalette);
  const [appStyle, setAppStyle] = useState<AppStyle>(loadAppStyle);
  const [themePanelView, setThemePanelView] = useState<ThemePanelView>("themes");
  const [wabiQuietMode, setWabiQuietMode] = useState(false);
  const [wabiQuietTaskId, setWabiQuietTaskId] = useState<string | null>(null);
  const [wabiQuietTaskMenuOpen, setWabiQuietTaskMenuOpen] = useState(false);
  const [wabiCircleCompetitive, setWabiCircleCompetitive] = useState<boolean>(loadWabiCircleCompetitive);
  const [gardenVariant, setGardenVariant] = useState<GardenVariant>(loadGardenVariant);
  const [breakTimerMinutes, setBreakTimerMinutes] = useState(5);
  const [breakTimerRemaining, setBreakTimerRemaining] = useState(5 * 60);
  const [breakTimerRunning, setBreakTimerRunning] = useState(false);
  const [breakTimerEditing, setBreakTimerEditing] = useState(false);
  const [breakTimerDraft, setBreakTimerDraft] = useState("");
  const [wabiRestRoom, setWabiRestRoom] = useState<"games" | "meditation">("games");
  const [breathOn, setBreathOn] = useState(false);
  const [breathElapsedSeconds, setBreathElapsedSeconds] = useState(0);
  const [fieldDashboardLayout, setFieldDashboardLayout] = useState<FieldDashboardLayout>(loadFieldDashboardLayout);
  const [dashboardLayout, setDashboardLayout] = useState<DashboardLayout>(loadDashboardLayout);
  const [customDashboardLayout, setCustomDashboardLayout] = useState<DashboardWidgetLayout[]>(loadCustomDashboardLayout);
  const [dashboardEditing, setDashboardEditing] = useState(false);
  const [draggingWidgetId, setDraggingWidgetId] = useState<DashboardWidgetId | null>(null);
  const [focusRange, setFocusRange] = useState<FocusRange>("week");
  const [focusTip, setFocusTip] = useState<FocusTip | null>(null);
  const [activeFocusMilestone, setActiveFocusMilestone] = useState<number | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [endlessInactivityPrompt, setEndlessInactivityPrompt] = useState<EndlessInactivityPrompt | null>(null);
  const [timerInactivityNoticeVisible, setTimerInactivityNoticeVisible] = useState(false);
  const [countdownNowMs, setCountdownNowMs] = useState(() => Date.now());
  const endlessContinuousStartedAtRef = useRef<string | null>(null);
  const endlessInactivityPromptRef = useRef<EndlessInactivityPrompt | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [wabiMenuPosition, setWabiMenuPosition] = useState<{ top: number; left: number } | null>(null);
  const wabiMenuButtonRef = useRef<HTMLButtonElement | null>(null);
  const [activeMenuPanel, setActiveMenuPanel] = useState<MenuPanel>(null);
  const [visibleTabsOptionsOpen, setVisibleTabsOptionsOpen] = useState(false);
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(TOTAL_WORKLOAD_ID);
  const [selectedQuietDashboardEntryId, setSelectedQuietDashboardEntryId] = useState<string | null>(null);
  const [fieldPlannerTaskId, setFieldPlannerTaskId] = useState<string | null>(null);
  const [fieldPlannerTaskMode, setFieldPlannerTaskMode] = useState<FieldPlannerTaskMode | null>(null);
  const [fieldPlannerDeleteTarget, setFieldPlannerDeleteTarget] = useState<FieldPlannerDeleteTarget | null>(null);
  const [fieldPlannerWorkloadId, setFieldPlannerWorkloadId] = useState<string>(TOTAL_WORKLOAD_ID);
  const [helpTab, setHelpTab] = useState<TabKey | null>(null);
  const [menuHelpOpen, setMenuHelpOpen] = useState(false);
  const [tourState, setTourState] = useState<{ tab: TabKey; step: number } | null>(null);
  const [semesterName, setSemesterName] = useState("");
  const [courseDraft, setCourseDraft] = useState<CourseDraft>({
    semesterId: "",
    name: "",
    targetGrade: "4.0",
    color: "#8fb4ff",
  });
  const [taskDraft, setTaskDraft] = useState<TaskDraft>({
    semesterId: "",
    courseId: "",
    title: "",
    unitLabel: "Unit",
    totalUnits: "10",
    completedUnits: "0",
    dueDate: "",
    priority: "medium",
    notes: "",
  });
  const [examDraft, setExamDraft] = useState<ExamDraft>({
    semesterId: "",
    courseId: "",
    title: "",
    examDate: "",
    weight: "40",
    preparedness: "35",
  });
  const [showSemesterForm, setShowSemesterForm] = useState(false);
  const [expandedSemesterIds, setExpandedSemesterIds] = useState<string[]>([]);
  const [expandedCourseIds, setExpandedCourseIds] = useState<string[]>([]);
  const [addingCourseSemesterId, setAddingCourseSemesterId] = useState<string | null>(null);
  const [addingTaskCourseId, setAddingTaskCourseId] = useState<string | null>(null);
  const [addingExamCourseId, setAddingExamCourseId] = useState<string | null>(null);
  const [editingSemesterId, setEditingSemesterId] = useState<string | null>(null);
  const [semesterEditName, setSemesterEditName] = useState("");
  const [editingCourseId, setEditingCourseId] = useState<string | null>(null);
  const [courseEditDraft, setCourseEditDraft] = useState<CourseDraft>({
    semesterId: "",
    name: "",
    targetGrade: "4.0",
    color: "#8fb4ff",
  });
  const [editingTaskId, setEditingTaskId] = useState<string | null>(null);
  const [taskEditDraft, setTaskEditDraft] = useState<TaskDraft>({
    semesterId: "",
    courseId: "",
    title: "",
    unitLabel: "Unit",
    totalUnits: "10",
    completedUnits: "0",
    dueDate: "",
    priority: "medium",
    notes: "",
  });
  const [calculatorOpen, setCalculatorOpen] = useState(true);
  const [timerAdvancedOpen, setTimerAdvancedOpen] = useState(false);
  const [timerFaceEditing, setTimerFaceEditing] = useState(false);
  const [timerFaceDraft, setTimerFaceDraft] = useState("");
  const endlessInactivityRemainingMs = endlessInactivityPrompt
    ? Math.max(0, new Date(endlessInactivityPrompt.promptedAt).getTime() + ENDLESS_INACTIVITY_GRACE_MS - countdownNowMs)
    : ENDLESS_INACTIVITY_GRACE_MS;
  const endlessInactivityCountdownLabel = formatCountdown(endlessInactivityRemainingMs);
  const [fullscreen, setFullscreen] = useState(false);
  const [windowMaximized, setWindowMaximized] = useState(false);
  const [calendarView, setCalendarView] = useState<CalendarView>("week");
  const [calendarCursorDate, setCalendarCursorDate] = useState(() => new Date());
  const [selectedCalendarDate, setSelectedCalendarDate] = useState<string | null>(null);
  const [calendarAddOpen, setCalendarAddOpen] = useState(false);
  const [calendarAddDraft, setCalendarAddDraft] = useState<CalendarAddDraft>({
    semesterId: "",
    courseId: "",
    source: "planner",
    taskId: "",
    title: "",
    unitAmount: 1,
    startTime: "09:00",
    durationMinutes: "60",
    noTime: false,
  });
  const [calendarEditEntryId, setCalendarEditEntryId] = useState<string | null>(null);
  const [calendarEditDraft, setCalendarEditDraft] = useState<CalendarEditDraft>({ startTime: "09:00", endTime: "10:00" });
  const [calendarDragEntryId, setCalendarDragEntryId] = useState<string | null>(null);
  const [calendarMovePreview, setCalendarMovePreview] = useState<CalendarMovePreview | null>(null);
  const [calendarResizeEntryId, setCalendarResizeEntryId] = useState<string | null>(null);
  const calendarTimelineRef = useRef<HTMLDivElement | null>(null);
  const calendarMoveDragRef = useRef<CalendarMoveDragState | null>(null);
  const calendarResizeRef = useRef<CalendarResizeState | null>(null);
  const [calendarToday, setCalendarToday] = useState(localIsoDate);
  const [personalNameDraft, setPersonalNameDraft] = useState(() => state.settings.userName);
  const [personalDailyGoalHoursDraft, setPersonalDailyGoalHoursDraft] = useState(() => String((state.settings.dailyGoalMinutes ?? 120) / 60));
  const [vaultNoteDate, setVaultNoteDate] = useState(localIsoDate);
  const [vaultNoteContent, setVaultNoteContent] = useState("");
  const [vaultNotePath, setVaultNotePath] = useState<string | null>(null);
  const [vaultNotes, setVaultNotes] = useState<VaultNoteFile[]>([]);
  const [vaultNoteTitle, setVaultNoteTitle] = useState("");
  const [vaultNoteSavedAt, setVaultNoteSavedAt] = useState<number | null>(null);
  const [vaultNotesLoading, setVaultNotesLoading] = useState(false);
  const [vaultNoteDirty, setVaultNoteDirty] = useState(false);
  const [vaultNoteLoading, setVaultNoteLoading] = useState(false);
  const [vaultSetupOpen, setVaultSetupOpen] = useState(() => !state.settings.vaultPath);
  const [vaultDailyEditing, setVaultDailyEditing] = useState(false);
  const [markdownCheatsheetOpen, setMarkdownCheatsheetOpen] = useState(false);
  const [vaultSpace, setVaultSpace] = useState<VaultSpace>("daily");
  const [referenceSemesterId, setReferenceSemesterId] = useState("");
  const [referenceCourseId, setReferenceCourseId] = useState("");
  const [referenceContent, setReferenceContent] = useState("");
  const [referencePath, setReferencePath] = useState<string | null>(null);
  const [referenceDirty, setReferenceDirty] = useState(false);
  const [referenceLoading, setReferenceLoading] = useState(false);
  const [referenceEditing, setReferenceEditing] = useState(false);
  const [referencePathVisible, setReferencePathVisible] = useState(false);
  const referenceLoadRequestRef = useRef(0);
  const [summarySemesterId, setSummarySemesterId] = useState("");
  const [summaryCourseId, setSummaryCourseId] = useState("");
  const [summaryFiles, setSummaryFiles] = useState<SummaryFile[]>([]);
  const [selectedSummaryPath, setSelectedSummaryPath] = useState<string | null>(null);
  const [summaryLoading, setSummaryLoading] = useState(false);
  const summaryLoadRequestRef = useRef(0);
  const pendingUpdateRef = useRef<Update | null>(null);
  const latestStateRef = useRef(state);
  const persistenceBaselinesRef = useRef<PersistenceBaselines>(createInitialPersistenceBaselines(state));
  const timerFaceRingRef = useRef<HTMLDivElement | null>(null);
  const examFaceRingRef = useRef<HTMLDivElement | null>(null);
  const saveStateTimeoutRef = useRef<number | null>(null);
  const sessionSavePendingRef = useRef(false);
  const verifiedSessionRef = useRef<string | null>(null);
  const timerRunningRef = useRef(state.timer.running);
  timerRunningRef.current = state.timer.running;
  const attemptVerifiedSyncRef = useRef<() => void>(() => {});
  const reconcilingOfflineCreditRef = useRef(false);
  const pendingSaveRef = useRef<AppState | null>(null);
  const socialSyncInProgressRef = useRef(false);
  const seenFeedCommentIdsRef = useRef<Set<string> | null>(null);
  const wordleModalRef = useRef<HTMLDivElement | null>(null);
  const [currentAppVersion, setCurrentAppVersion] = useState("loading...");
  const [updateInstallSupport, setUpdateInstallSupport] = useState<UpdateInstallSupport>(DEFAULT_UPDATE_INSTALL_SUPPORT);
  const [linuxUpdateDownload, setLinuxUpdateDownload] = useState<LinuxUpdateDownload | null>(null);
  const [linuxPackageDownloading, setLinuxPackageDownloading] = useState(false);
  const [updateNoticeVisible, setUpdateNoticeVisible] = useState(false);
  const [appAnnouncement, setAppAnnouncement] = useState<AppAnnouncement | null>(null);
  const [updateNoticeSending, setUpdateNoticeSending] = useState(false);
  const [adminUsage, setAdminUsage] = useState<AdminUsageResponse | null>(null);
  const [adminUsageLoading, setAdminUsageLoading] = useState(false);
  const [adminUsageOpen, setAdminUsageOpen] = useState(false);
  const [privacyOpen, setPrivacyOpen] = useState(false);
  const [feedCommentNotice, setFeedCommentNotice] = useState<FeedCommentNotice | null>(null);
  const [updateChecking, setUpdateChecking] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo>({
    status: "idle",
    releaseUrl: RELEASES_PAGE_URL,
    message: "Check whether a newer release is available.",
  });
  const [hasUnreadSocial, setHasUnreadSocial] = useState(false);
  const [emojiPickerPostId, setEmojiPickerPostId] = useState<string | null>(null);
  const [socialSubtab, setSocialSubtab] = useState<SocialSubtab>("feed");
  const [feedScope, setFeedScope] = useState<SocialFeedScope>("friends");
  const [feedNoteDraft, setFeedNoteDraft] = useState("");
  const [feedImageDraft, setFeedImageDraft] = useState<PreparedFeedImage | null>(null);
  const [feedPollDraft, setFeedPollDraft] = useState<FeedPollDraft>(() => emptyFeedPollDraft());
  const [feedPollPanelOpen, setFeedPollPanelOpen] = useState(false);
  const [r2UsageStatus, setR2UsageStatus] = useState<R2UsageStatus | null>(null);
  const [failedFeedImages, setFailedFeedImages] = useState<Set<string>>(() => new Set());
  const [expandedFeedImageId, setExpandedFeedImageId] = useState<string | null>(null);
  const [feedLoading, setFeedLoading] = useState(false);
  const [expandedFeedComments, setExpandedFeedComments] = useState<Set<string>>(() => new Set());
  const [feedCommentDrafts, setFeedCommentDrafts] = useState<Record<string, string>>({});
  const [feedCommentSavingId, setFeedCommentSavingId] = useState<string | null>(null);
  const [socialScope, setSocialScope] = useState<SocialLeaderboardScope>("friends");
  const [socialPeriod, setSocialPeriod] = useState<SocialLeaderboardPeriod>("weekly");
  const [squadScorePeriod, setSquadScorePeriod] = useState<SocialSquadScorePeriod>("season");
  const [friendCodeDraft, setFriendCodeDraft] = useState("");
  const [squadNameDraft, setSquadNameDraft] = useState("");
  const [squadPrivateDraft, setSquadPrivateDraft] = useState(false);
  const [squadSearchDraft, setSquadSearchDraft] = useState("");
  const [squadSearchResults, setSquadSearchResults] = useState<SquadSearchResult[]>([]);
  const [squadSuggestionPool, setSquadSuggestionPool] = useState<SquadSearchResult[]>([]);
  const [squadSuggestions, setSquadSuggestions] = useState<SquadSearchResult[]>([]);
  const [squadSearching, setSquadSearching] = useState(false);
  const [squadSuggestionsLoading, setSquadSuggestionsLoading] = useState(false);
  const [squadChatDraft, setSquadChatDraft] = useState("");
  const [expandedSquadMemberId, setExpandedSquadMemberId] = useState<string | null>(null);
  const [squadSettingsEditing, setSquadSettingsEditing] = useState(false);
  const [squadSettingsNameDraft, setSquadSettingsNameDraft] = useState("");
  const [squadSettingsPrivateDraft, setSquadSettingsPrivateDraft] = useState(false);
  const [socialSyncing, setSocialSyncing] = useState(false);
  const [socialNameEditing, setSocialNameEditing] = useState(false);
  const [socialNameDraft, setSocialNameDraft] = useState(() => state.social.displayName);
  const [profileAvatarEditorOpen, setProfileAvatarEditorOpen] = useState(false);
  const [profileAvatarDraft, setProfileAvatarDraft] = useState<SocialAvatar>(() => state.social.avatar);
  const [profileAvatarLetterPickerOpen, setProfileAvatarLetterPickerOpen] = useState(false);
  const profileAvatarFileInputRef = useRef<HTMLInputElement | null>(null);
  const [avatarCropOpen, setAvatarCropOpen] = useState(false);
  const [avatarCropSource, setAvatarCropSource] = useState<AvatarCropSource | null>(null);
  const [avatarCrop, setAvatarCrop] = useState<AvatarCropState>({ x: 0.5, y: 0.5, zoom: 1 });
  const [avatarCropDragging, setAvatarCropDragging] = useState(false);
  const avatarCropDragRef = useRef<{ pointerId: number; startX: number; startY: number; cropX: number; cropY: number } | null>(null);
  const [viewingFriend, setViewingFriend] = useState<SocialProfileTarget | null>(null);
  const [viewingFriendStats, setViewingFriendStats] = useState<PlayerStatsResponse | null>(null);
  const [viewingFriendLoading, setViewingFriendLoading] = useState(false);
  const [viewingSquadEntry, setViewingSquadEntry] = useState<SocialSquadScoreEntry | null>(null);
  const [viewingSquadDetails, setViewingSquadDetails] = useState<SocialSquadDetails | null>(null);
  const [viewingSquadLoading, setViewingSquadLoading] = useState(false);
  const [editingFeedPostId, setEditingFeedPostId] = useState<string | null>(null);
  const [editingFeedPostNote, setEditingFeedPostNote] = useState("");
  const [editingFeedPostImage, setEditingFeedPostImage] = useState<PreparedFeedImage | null>(null);
  const [editingFeedPostRemoveImage, setEditingFeedPostRemoveImage] = useState(false);
  const [feedPostSaving, setFeedPostSaving] = useState(false);
  const [badgesOpen, setBadgesOpen] = useState(false);
  const canViewR2Usage = state.social.friendCode === R2_OWNER_FRIEND_CODE;
  const [showDurakPuzzle, setShowDurakPuzzle] = useState(false);
  const [showWordlePuzzle, setShowWordlePuzzle] = useState(false);
  const [showGeodlePuzzle, setShowGeodlePuzzle] = useState(false);
  const [showFlagglePuzzle, setShowFlagglePuzzle] = useState(false);
  const [showTravlePuzzle, setShowTravlePuzzle] = useState(false);
  const [durakGameState, setDurakGameState] = useState<DurakGameState | null>(null);
  const [durakSelected, setDurakSelected] = useState<number[]>([]);
  const [wordleDraft, setWordleDraft] = useState("");
  const [wordleMessage, setWordleMessage] = useState("");
  const [geodleDraft, setGeodleDraft] = useState("");
  const [geodleDropdownOpen, setGeodleDropdownOpen] = useState(false);
  const [geodleMessage, setGeodleMessage] = useState("");
  const [flaggleDraft, setFlaggleDraft] = useState("");
  const [flaggleDropdownOpen, setFlaggleDropdownOpen] = useState(false);
  const [flaggleMessage, setFlaggleMessage] = useState("");
  const [flaggleProcessing, setFlaggleProcessing] = useState(false);
  const [flagglePreviewDataUrl, setFlagglePreviewDataUrl] = useState("");
  const [travleDraft, setTravleDraft] = useState("");
  const [travleDropdownOpen, setTravleDropdownOpen] = useState(false);
  const [travleMessage, setTravleMessage] = useState("");
  const [travleMapZoom, setTravleMapZoom] = useState(1);
  const canSendUpdateNotice = canViewR2Usage && isValidAppVersion(currentAppVersion);

  function getAppMetadata(): AppMetadata {
    return {
      version: currentAppVersion && currentAppVersion !== "loading..." ? currentAppVersion : "unknown",
      platform: navigator.platform || "unknown",
      runtimeChannel: updateInstallSupport.runtimeChannel || "unknown",
    };
  }

  // Only sections whose reference actually changed since the last successful persist get
  // written (see storage.ts's getChangedSections) - a section's baseline only advances once
  // its own write succeeds, so a failed write is retried on the next call instead of being
  // silently treated as done. See lib/storage.ts for the full section-aware design.
  function persistState(nextState: AppState, options?: { forceSections?: ReadonlySet<PersistSection> }) {
    const succeeded = saveAppState(nextState, persistenceBaselinesRef.current, options);
    persistenceBaselinesRef.current = applyPersistedSections(persistenceBaselinesRef.current, nextState, succeeded);
  }

  useEffect(() => {
    latestStateRef.current = state;
    if (saveStateTimeoutRef.current !== null) window.clearTimeout(saveStateTimeoutRef.current);
    saveStateTimeoutRef.current = window.setTimeout(() => {
      persistState(latestStateRef.current);
      saveStateTimeoutRef.current = null;
    }, 700);
  }, [state]);

  useEffect(() => {
    if (!state.timer.running) return undefined;

    return startTimerPersistenceHeartbeat(
      () => latestStateRef.current,
      (s) => persistState(s, { forceSections: TIMER_ONLY_SECTION }),
    );
  }, [state.timer.running]);
  useEffect(() => {
    const flushState = () => {
      if (saveStateTimeoutRef.current !== null) {
        window.clearTimeout(saveStateTimeoutRef.current);
        saveStateTimeoutRef.current = null;
      }
      persistState(latestStateRef.current, { forceSections: ALL_PERSIST_SECTIONS });
    };
    const handleVisibilityChange = () => {
      if (document.visibilityState === "hidden") flushState();
    };
    window.addEventListener("beforeunload", flushState);
    window.addEventListener("pagehide", flushState);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => {
      window.removeEventListener("beforeunload", flushState);
      window.removeEventListener("pagehide", flushState);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      flushState();
    };
  }, []);

  useEffect(() => () => {
    if (feedImageDraft) URL.revokeObjectURL(feedImageDraft.previewUrl);
  }, [feedImageDraft]);

  useEffect(() => () => {
    if (editingFeedPostImage) URL.revokeObjectURL(editingFeedPostImage.previewUrl);
  }, [editingFeedPostImage]);

  useEffect(() => {
    if (!expandedFeedImageId) return;
    function closeOnEscape(event: globalThis.KeyboardEvent) {
      if (event.key === "Escape") setExpandedFeedImageId(null);
    }
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [expandedFeedImageId]);

  useEffect(() => {
    if (state.settings.hideFeedImages) setExpandedFeedImageId(null);
  }, [state.settings.hideFeedImages]);

  useLayoutEffect(() => {
    document.documentElement.dataset.backgroundEffect = state.settings.backgroundEffect === false ? "off" : "on";
  }, [state.settings.backgroundEffect]);

  useEffect(() => {
    if (!isTauriApp()) {
      setCurrentAppVersion("browser preview");
      setUpdateInstallSupport({
        canAutoInstall: false,
        packageHint: "browser",
        runtimeChannel: "browser",
        message: "Automatic updates are only available in the installed desktop app.",
      });
      return;
    }

    void getVersion()
      .then(setCurrentAppVersion)
      .catch((error: unknown) => {
        console.warn("Could not read app version.", error);
        setCurrentAppVersion("unknown");
      });

    void invoke<UpdateInstallSupport>("get_update_install_support")
      .then(setUpdateInstallSupport)
      .catch((error: unknown) => {
        console.warn("Could not detect update install support.", error);
        setUpdateInstallSupport({
          canAutoInstall: false,
          packageHint: "unknown",
          runtimeChannel: "unknown",
          message: "Could not detect whether this install supports automatic updates. Use the release page instead.",
        });
      });
  }, []);

  useEffect(() => {
    setState((current) => carryOverCalendarEntries(current, calendarToday));
  }, [calendarToday]);

  useEffect(() => {
    const interval = window.setInterval(() => {
      const nextToday = localIsoDate();
      setCalendarToday((current) => (current === nextToday ? current : nextToday));
    }, 60000);

    return () => window.clearInterval(interval);
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem("study-tracker-theme", theme);
  }, [theme]);

  useEffect(() => {
    document.documentElement.dataset.appStyle = appStyle;
    localStorage.setItem(APP_STYLE_STORAGE_KEY, appStyle);
  }, [appStyle]);

  function updateWabiMenuPosition() {
    const button = wabiMenuButtonRef.current;
    if (!button) return;

    const rect = button.getBoundingClientRect();
    const menuWidth = 200;
    const viewportPadding = 8;
    setWabiMenuPosition({
      top: rect.bottom + 8,
      left: Math.min(rect.left, window.innerWidth - menuWidth - viewportPadding),
    });
  }

  useLayoutEffect(() => {
    if (!menuOpen || appStyle !== "wabi-sabi") return undefined;

    updateWabiMenuPosition();
    window.addEventListener("resize", updateWabiMenuPosition);
    return () => window.removeEventListener("resize", updateWabiMenuPosition);
  }, [appStyle, menuOpen]);

  useEffect(() => {
    localStorage.setItem(WABI_CIRCLE_COMPETITIVE_KEY, String(wabiCircleCompetitive));
  }, [wabiCircleCompetitive]);

  useEffect(() => {
    localStorage.setItem(GARDEN_VARIANT_KEY, gardenVariant);
  }, [gardenVariant]);

  useEffect(() => {
    if (!breakTimerRunning) return;
    const id = setInterval(() => {
      setBreakTimerRemaining((current) => {
        if (current <= 1) {
          setBreakTimerRunning(false);
          return 0;
        }
        return current - 1;
      });
    }, 1000);
    return () => clearInterval(id);
  }, [breakTimerRunning]);

  useEffect(() => {
    if (!breathOn) return;
    const id = setInterval(() => {
      setBreathElapsedSeconds((current) => {
        if (current >= 75) {
          setBreathOn(false);
          return 0;
        }
        return current + 1;
      });
    }, 1000);
    return () => clearInterval(id);
  }, [breathOn]);

  useEffect(() => {
    if (palette === "default") {
      delete document.documentElement.dataset.palette;
    } else {
      document.documentElement.dataset.palette = palette;
    }
    localStorage.setItem(PALETTE_STORAGE_KEY, palette);
  }, [palette]);

  useEffect(() => {
    localStorage.setItem(DASHBOARD_LAYOUT_KEY, dashboardLayout);
  }, [dashboardLayout]);

  useEffect(() => {
    localStorage.setItem(FIELD_DASHBOARD_LAYOUT_KEY, fieldDashboardLayout);
  }, [fieldDashboardLayout]);

  useEffect(() => {
    if (appStyle === "field-notebook" && state.activeTab === "planner") {
      setCalendarView("month");
    }
  }, [appStyle, state.activeTab]);

  useEffect(() => {
    localStorage.setItem(CUSTOM_DASHBOARD_LAYOUT_KEY, JSON.stringify(customDashboardLayout));
  }, [customDashboardLayout]);

  useEffect(() => {
    if (dashboardLayout !== "custom") setDashboardEditing(false);
  }, [dashboardLayout]);

  useEffect(() => {
    if (!endlessInactivityPrompt) return undefined;
    setCountdownNowMs(Date.now());
    const interval = window.setInterval(() => setCountdownNowMs(Date.now()), 1000);
    return () => window.clearInterval(interval);
  }, [endlessInactivityPrompt]);

  useEffect(() => {
    if (!message) return undefined;
    const timeout = window.setTimeout(() => setMessage(null), 3200);
    return () => window.clearTimeout(timeout);
  }, [message]);

  useEffect(() => {
    if (!markdownCheatsheetOpen) return undefined;
    function closeOnEscape(event: globalThis.KeyboardEvent) {
      if (event.key === "Escape") setMarkdownCheatsheetOpen(false);
    }

    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [markdownCheatsheetOpen]);

  useEffect(() => {
    if (!state.timer.running) return undefined;

    function ringBell() {
      playBellSound().then((result) => {
        if (!result.ok) console.warn("Bell sound could not play.", result);
      });
    }

    const interval = window.setInterval(() => {
      setState((current) => {
        const timer = current.timer;
        if (!timer.running) return current;
        if (!timer.endsAt && timer.phase !== "stopwatch") return current;

        if (timer.phase === "stopwatch") {
          if (!timer.startedAt) return current;
          const now = new Date();
          const prompt = endlessInactivityPromptRef.current;
          if (prompt) {
            if (now.getTime() - new Date(prompt.promptedAt).getTime() >= ENDLESS_INACTIVITY_GRACE_MS) {
              const sessions = buildSessionsFromTimerRange(timer, prompt.promptedAt);
              const socialState = prependSessionsToState(current, sessions, sessions[sessions.length - 1]);
              endlessContinuousStartedAtRef.current = null;
              endlessInactivityPromptRef.current = null;
              setEndlessInactivityPrompt(null);
              setTimerInactivityNoticeVisible(true);
              void sendTimerNotification("Timer stopped", "Timer was stopped due to inactivity.");
              const next = {
                ...socialState,
                timer: {
                  ...defaultTimer,
                  ...keepTimerContext(timer),
                  running: false,
                  startedAt: null,
                  endsAt: null,
                  phase: "idle" as const,
                  remainingSeconds: getIdleTimerSeconds(timer),
                  activeSegments: [],
                },
              };
              pendingSaveRef.current = next;
              return next;
            }

            // Display value is derived on demand (getDisplayRemainingSeconds) instead of
            // being written to state every tick; this tick only needs to poll for the
            // inactivity-grace-period transition above.
            return current;
          }

          if (!endlessContinuousStartedAtRef.current) {
            endlessContinuousStartedAtRef.current = now.toISOString();
          } else if (now.getTime() - new Date(endlessContinuousStartedAtRef.current).getTime() >= ENDLESS_INACTIVITY_PROMPT_MS) {
            const nextPrompt = { promptedAt: now.toISOString() };
            endlessInactivityPromptRef.current = nextPrompt;
            setEndlessInactivityPrompt(nextPrompt);
            ringBell();
            void sendTimerNotification("Are you still here?", "Confirm that you are still studying to keep the endless timer running.");
          }

          return current;
        }

        const endsAt = timer.endsAt;
        if (!endsAt) return current;

        const now = new Date();
        const diff = Math.ceil((new Date(endsAt).getTime() - now.getTime()) / 1000);
        if (diff > 0) {
          // Same reasoning as the stopwatch branch above: this tick only needs to detect
          // whether endsAt has been crossed yet, not keep remainingSeconds fresh in state.
          return current;
        }

        const endedAt = new Date(new Date(endsAt).getTime()).toISOString();
        if (timer.phase === "study") {
          const sessions = buildSessionsFromTimerRange(timer, endedAt);
          const socialState = prependSessionsToState(current, sessions, sessions[sessions.length - 1]);
          if (timer.mode === "focus" && timer.breakMinutes > 0) {
            ringBell();
            void sendTimerNotification("Focus session finished", "Nice work. Time for a break.");
            const next = {
              ...socialState,
              timer: {
                ...timer,
                phase: "break" as const,
                running: true,
                  startedAt: endedAt,
                  endsAt: new Date(Date.now() + timer.breakMinutes * 60000).toISOString(),
                  remainingSeconds: timer.breakMinutes * 60,
                  loggedSplitSeconds: 0,
                  activeSegments: [],
                },
              };
            pendingSaveRef.current = next;
            return next;
          }

          ringBell();
          void sendTimerNotification("Focus session finished", "Your study timer is complete.");
          const next = {
            ...socialState,
            timer: { ...defaultTimer, ...keepTimerContext(timer) },
          };
          pendingSaveRef.current = next;
          return next;
        }

        if (timer.phase === "exam") {
          const sessions = buildSessionsFromTimerRange(timer, endedAt);
          const socialState = prependSessionsToState(current, sessions, sessions[sessions.length - 1]);
          ringBell();
          void sendTimerNotification("Exam timer finished", "Your exam timer is complete.");
          const next = {
            ...socialState,
            timer: { ...defaultTimer, ...keepTimerContext(timer) },
          };
          pendingSaveRef.current = next;
          return next;
        }

        ringBell();
        if (timer.phase === "break") {
          void sendTimerNotification("Break finished", "Break is over. Ready for the next focus session?");
        }
        const next = {
          ...current,
          timer: { ...defaultTimer, ...keepTimerContext(timer) },
        };
        pendingSaveRef.current = next;
        return next;
      });
      window.setTimeout(() => {
        const pending = pendingSaveRef.current;
        if (pending) {
          pendingSaveRef.current = null;
          persistState(pending);
        }
      }, 0);
    }, 500);

    return () => window.clearInterval(interval);
  }, [state.timer.running]);

  // Ticks on its own (via useTimerTick, capped at 1/sec) instead of depending on a
  // per-tick-written remainingSeconds field, and only ever calls invoke() - never setState -
  // so it cannot itself trigger an App() re-render.
  useTimerTick(state.timer, (timer) => {
    if (!isTauriApp()) return;
    const verifiedSessionActive = isSocialApiConfigured()
      && timer.running
      && timer.phase !== "idle"
      && timer.phase !== "break";
    void invoke("set_timer_tray_state", {
      phase: timer.phase,
      running: timer.running,
      remainingSeconds: Math.max(0, Math.floor(getDisplayRemainingSeconds(timer))),
      verifiedSessionActive,
    }).catch((error: unknown) => {
      console.warn("Timer tray state could not be updated.", error);
    });
  }, 1000);

  useEffect(() => {
    if (state.timer.phase === "idle") {
      setFullscreen(false);
    }
  }, [state.timer.phase]);

  useEffect(() => {
    if (!isTauriApp()) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    getCurrentWindow().isMaximized().then((value) => { if (!cancelled) setWindowMaximized(value); }).catch(() => undefined);
    getCurrentWindow().onResized(() => {
      getCurrentWindow().isMaximized().then((value) => { if (!cancelled) setWindowMaximized(value); }).catch(() => undefined);
    }).then((fn) => { if (cancelled) fn(); else unlisten = fn; }).catch(() => undefined);
    return () => { cancelled = true; unlisten?.(); };
  }, []);

  useEffect(() => {
    if (
      selectedTaskId === TOTAL_WORKLOAD_ID ||
      (selectedTaskId && state.tasks.some((task) => task.id === selectedTaskId))
    ) {
      return;
    }
    const nextTask = state.tasks.find((task) => getRemainingUnits(task) > 0) ?? state.tasks[0] ?? null;
    setSelectedTaskId(nextTask?.id ?? null);
  }, [selectedTaskId, state.tasks]);

  useEffect(() => {
    if (!calendarAddDraft.semesterId && state.semesters[0]?.id) {
      setCalendarAddDraft((current) => ({ ...current, semesterId: state.semesters[0].id }));
    }
  }, [calendarAddDraft.semesterId, state.semesters]);

  useEffect(() => {
    function handleMoveDrag(event: globalThis.MouseEvent) {
      const drag = calendarMoveDragRef.current;
      const timeline = calendarTimelineRef.current;
      if (!drag || !timeline) return;

      const movedEnough = Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY) > 4;
      if (!movedEnough && !drag.moved) return;
      drag.moved = true;

      const target = getTimelineTimeFromClientY(timeline, event.clientY);
      setCalendarMovePreview({ entryId: drag.entryId, x: event.clientX, y: event.clientY, top: target.top, time: target.time });
    }

    function handleMoveEnd(event: globalThis.MouseEvent) {
      const drag = calendarMoveDragRef.current;
      const timeline = calendarTimelineRef.current;
      if (!drag || !timeline) return;

      if (!drag.moved) {
        calendarMoveDragRef.current = null;
        setCalendarDragEntryId(null);
        setCalendarMovePreview(null);
        return;
      }

      const target = getTimelineTimeFromClientY(timeline, event.clientY);
      updateCalendarEntryTime(drag.entryId, target.time, addMinutesToTime(target.time, drag.durationMinutes));
      calendarMoveDragRef.current = null;
      setCalendarDragEntryId(null);
      setCalendarMovePreview(null);
    }

    function handleResizeMove(event: globalThis.MouseEvent) {
      const resize = calendarResizeRef.current;
      if (!resize) return;

      const minuteDelta = snapCalendarMinutes(((event.clientY - resize.startY) / 72) * 60);
      const endMinutes = clamp(resize.endMinutes + minuteDelta, resize.startMinutes + calendarTimeStepMinutes, 23 * 60 + 59);
      const endTime = minutesToTime(endMinutes);
      setState((current) => ({
        ...current,
        calendarEntries: current.calendarEntries.map((entry) =>
          entry.id === resize.entryId ? { ...entry, endTime } : entry,
        ),
      }));
    }

    function handleResizeEnd() {
      calendarResizeRef.current = null;
      setCalendarResizeEntryId(null);
    }

    window.addEventListener("mousemove", handleResizeMove);
    window.addEventListener("mouseup", handleResizeEnd);
    window.addEventListener("mousemove", handleMoveDrag);
    window.addEventListener("mouseup", handleMoveEnd);
    return () => {
      window.removeEventListener("mousemove", handleResizeMove);
      window.removeEventListener("mouseup", handleResizeEnd);
      window.removeEventListener("mousemove", handleMoveDrag);
      window.removeEventListener("mouseup", handleMoveEnd);
    };
  }, []);

  useEffect(() => {
    setCalendarAddDraft((current) => {
      if (!current.semesterId) return current;
      if (current.courseId && state.courses.some((course) => course.id === current.courseId && course.semesterId === current.semesterId)) return current;
      return { ...current, courseId: "", taskId: "" };
    });
  }, [calendarAddDraft.semesterId, state.courses]);

  useEffect(() => {
    setCalendarAddDraft((current) => {
      if (current.source !== "planner") return current;
      const taskMatches = current.taskId && state.tasks.some((task) => {
        if (task.id !== current.taskId) return false;
        if (current.courseId) return task.courseId === current.courseId;
        if (current.semesterId) return task.semesterId === current.semesterId;
        return true;
      });
      if (taskMatches) return current;
      const nextTask = state.tasks.find((task) => {
        if (current.courseId) return task.courseId === current.courseId;
        if (current.semesterId) return task.semesterId === current.semesterId;
        return true;
      });
      return { ...current, taskId: nextTask?.id ?? "" };
    });
  }, [calendarAddDraft.courseId, calendarAddDraft.semesterId, state.tasks]);

  useEffect(() => {
    const firstSemester = state.semesters[0]?.id ?? "";
    if (!courseDraft.semesterId && firstSemester) {
      setCourseDraft((current) => ({ ...current, semesterId: firstSemester }));
    }
    if (!taskDraft.semesterId && firstSemester) {
      const firstCourse = state.courses.find((course) => course.semesterId === firstSemester);
      setTaskDraft((current) => ({
        ...current,
        semesterId: firstSemester,
        courseId: firstCourse?.id ?? current.courseId,
      }));
    }
    if (!examDraft.semesterId && firstSemester) {
      const firstCourse = state.courses.find((course) => course.semesterId === firstSemester);
      setExamDraft((current) => ({
        ...current,
        semesterId: firstSemester,
        courseId: firstCourse?.id ?? current.courseId,
      }));
    }
  }, [courseDraft.semesterId, examDraft.semesterId, state.courses, state.semesters, taskDraft.semesterId]);

  const semesterLookup = useMemo(
    () => new Map(state.semesters.map((semester) => [semester.id, semester])),
    [state.semesters],
  );
  const courseLookup = useMemo(
    () => new Map(state.courses.map((course) => [course.id, course])),
    [state.courses],
  );
  const referenceCourses = useMemo(
    () => referenceSemesterId ? state.courses.filter((course) => course.semesterId === referenceSemesterId) : [],
    [referenceSemesterId, state.courses],
  );
  const summaryCourses = useMemo(
    () => summarySemesterId ? state.courses.filter((course) => course.semesterId === summarySemesterId) : [],
    [summarySemesterId, state.courses],
  );
  const selectedReferenceSemester = referenceSemesterId ? semesterLookup.get(referenceSemesterId) ?? null : null;
  const selectedReferenceCourse = referenceCourseId ? courseLookup.get(referenceCourseId) ?? null : null;
  const selectedSummarySemester = summarySemesterId ? semesterLookup.get(summarySemesterId) ?? null : null;
  const selectedSummaryCourse = summaryCourseId ? courseLookup.get(summaryCourseId) ?? null : null;
  const selectedSummaryFile = summaryFiles.find((file) => file.path === selectedSummaryPath) ?? summaryFiles[0] ?? null;
  const selectedSummaryUrl = selectedSummaryFile ? convertFileSrc(selectedSummaryFile.path) : null;
  const taskLookup = useMemo(() => new Map(state.tasks.map((task) => [task.id, task])), [state.tasks]);
  // Single-pass grouping/aggregation maps shared by the dashboard course radar and the planner
  // semester/course lists, instead of each course/semester re-scanning the full tasks/exams/
  // sessions arrays on every render (see metrics.ts for the O(N*tasks) -> O(tasks) reasoning).
  const courseTasksByCourseId = useMemo(() => groupTasksByCourseId(state.tasks), [state.tasks]);
  const semesterTasksBySemesterId = useMemo(() => groupTasksBySemesterId(state.tasks), [state.tasks]);
  const courseMinutesByCourseId = useMemo(() => getCourseMinutesMap(state.sessions), [state.sessions]);
  // calendarToday is included only to force a recompute at the local day boundary - the
  // callback doesn't read its value. calculateWorkloadHealth (via daysUntil/getExamPressure)
  // reads the real wall clock internally, so without this the "overdue"/"due soon" status
  // would silently freeze past midnight until courses/tasks/exams changed for some other
  // reason. Mirrors the same pattern already used by weeklyActivity/todayMinutes/streakDays/
  // focusMomentum below.
  const courseHealthByCourseId = useMemo(
    () => getCourseHealthMap(state.courses, courseTasksByCourseId, state.exams),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- calendarToday forces day-boundary invalidation, see comment above
    [state.courses, courseTasksByCourseId, state.exams, calendarToday],
  );
  const semesterHealthBySemesterId = useMemo(
    () => getSemesterHealthMap(state.semesters, semesterTasksBySemesterId, state.exams),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- calendarToday forces day-boundary invalidation, see comment above
    [state.semesters, semesterTasksBySemesterId, state.exams, calendarToday],
  );
  const calendarAddCourses = useMemo(
    () => calendarAddDraft.semesterId ? state.courses.filter((course) => course.semesterId === calendarAddDraft.semesterId) : [],
    [calendarAddDraft.semesterId, state.courses],
  );
  const calendarAddTasks = useMemo(
    () => state.tasks.filter((task) => {
      if (calendarAddDraft.courseId) return task.courseId === calendarAddDraft.courseId;
      if (calendarAddDraft.semesterId) return task.semesterId === calendarAddDraft.semesterId;
      return true;
    }),
    [calendarAddDraft.courseId, calendarAddDraft.semesterId, state.tasks],
  );

  useEffect(() => {
    const firstSemester = state.semesters[0]?.id ?? "";
    if (!referenceSemesterId && firstSemester) {
      setReferenceSemesterId(firstSemester);
      return;
    }
    if (referenceSemesterId && !state.semesters.some((semester) => semester.id === referenceSemesterId)) {
      setReferenceSemesterId(firstSemester);
      setReferenceCourseId("");
    }
  }, [referenceSemesterId, state.semesters]);

  useEffect(() => {
    const firstCourse = referenceCourses[0]?.id ?? "";
    if (!referenceCourseId && firstCourse) {
      setReferenceCourseId(firstCourse);
      return;
    }
    if (referenceCourseId && !referenceCourses.some((course) => course.id === referenceCourseId)) {
      setReferenceCourseId(firstCourse);
    }
  }, [referenceCourseId, referenceCourses]);

  useEffect(() => {
    setReferenceContent("");
    setReferencePath(null);
    setReferenceDirty(false);
    setReferenceEditing(false);
    setReferencePathVisible(false);
  }, [referenceSemesterId, referenceCourseId]);

  useEffect(() => {
    if (vaultSpace !== "references" || !state.settings.vaultPath || !selectedReferenceSemester || !selectedReferenceCourse) return;
    void loadReferenceNote(state.settings.vaultPath, selectedReferenceSemester, selectedReferenceCourse, { silent: true });
  }, [selectedReferenceCourse, selectedReferenceSemester, state.settings.vaultPath, vaultSpace]);

  useEffect(() => {
    const firstSemester = state.semesters[0]?.id ?? "";
    if (!summarySemesterId && firstSemester) {
      setSummarySemesterId(firstSemester);
      return;
    }
    if (summarySemesterId && !state.semesters.some((semester) => semester.id === summarySemesterId)) {
      setSummarySemesterId(firstSemester);
      setSummaryCourseId("");
    }
  }, [state.semesters, summarySemesterId]);

  useEffect(() => {
    const firstCourse = summaryCourses[0]?.id ?? "";
    if (!summaryCourseId && firstCourse) {
      setSummaryCourseId(firstCourse);
      return;
    }
    if (summaryCourseId && !summaryCourses.some((course) => course.id === summaryCourseId)) {
      setSummaryCourseId(firstCourse);
    }
  }, [summaryCourseId, summaryCourses]);

  useEffect(() => {
    setSummaryFiles([]);
    setSelectedSummaryPath(null);
  }, [summarySemesterId, summaryCourseId]);

  useEffect(() => {
    if (vaultSpace !== "summaries" || !state.settings.vaultPath || !selectedSummarySemester || !selectedSummaryCourse) return;
    void loadSummaryFileList(state.settings.vaultPath, selectedSummarySemester, selectedSummaryCourse, { silent: true });
  }, [selectedSummaryCourse, selectedSummarySemester, state.settings.vaultPath, vaultSpace]);

  useEffect(() => {
    if (appStyle !== "wabi-sabi" || state.activeTab !== "vault" || !state.settings.vaultPath) return;
    void loadVaultNotes(state.settings.vaultPath);
  }, [appStyle, state.activeTab, state.settings.vaultPath]);

  useEffect(() => {
    if (appStyle !== "wabi-sabi" || vaultNoteTitle || !vaultNotes.length) return;
    void openVaultNote(vaultNotes[0]);
  }, [appStyle, vaultNoteTitle, vaultNotes]);
  const selectedTask = useMemo(
    () => state.tasks.find((task) => task.id === selectedTaskId) ?? null,
    [selectedTaskId, state.tasks],
  );
  const totalWorkload = useMemo(() => {
    return calculateAggregateWorkload(state.tasks);
  }, [state.tasks]);
  const isTotalWorkloadSelected = selectedTaskId === TOTAL_WORKLOAD_ID;
  const fieldPlannerWorkload = useMemo(() => {
    if (fieldPlannerWorkloadId === TOTAL_WORKLOAD_ID) return { ...totalWorkload, label: "Total workload" };

    const course = state.courses.find((item) => item.id === fieldPlannerWorkloadId) ?? null;
    const courseTasks = course ? state.tasks.filter((task) => task.courseId === course.id) : [];
    if (!course) {
      return {
        ...calculateAggregateWorkload([]),
        label: "Course workload",
        message: "Choose a course.",
      };
    }

    return { ...calculateAggregateWorkload(courseTasks), label: course.name };
  }, [fieldPlannerWorkloadId, state.courses, state.tasks, totalWorkload]);

  const weeklyActivity = useMemo(() => getWeeklyActivity(state.sessions, new Date(`${calendarToday}T00:00:00`)), [state.sessions, calendarToday]);
  const upcomingExams = useMemo(() => getUpcomingExams({ exams: state.exams } as AppState), [state.exams]);
  const overallHealth = useMemo(() => getOverallHealth({ tasks: state.tasks, exams: state.exams } as AppState), [state.exams, state.tasks]);
  const notePreview = useMemo(() => buildDailyNoteMarkdown({ courses: state.courses, sessions: state.sessions } as AppState, vaultNoteDate), [state.courses, state.sessions, vaultNoteDate]);
  const dailyPreviewContent = stripMarkdownFrontmatter(vaultNoteContent || notePreview);
  const referencePreview = selectedReferenceSemester && selectedReferenceCourse
    ? stripMarkdownFrontmatter(referenceContent || buildReferenceNoteMarkdown(selectedReferenceCourse))
    : "";
  const healthLabel = overallHealth >= 75 ? "Strong" : overallHealth >= 55 ? "Steady" : overallHealth >= 35 ? "Watch" : "Critical";
  const healthState = overallHealth >= 75 ? "strong" : overallHealth >= 55 ? "steady" : overallHealth >= 35 ? "watch" : "critical";
  const scoreColor = overallHealth >= 75 ? "var(--ok)" : overallHealth >= 55 ? "var(--steady)" : overallHealth >= 35 ? "var(--watch)" : "var(--critical)";
  const greetingName = state.settings.userName.trim();
  const dashboardGreeting = `${getTimeGreeting()}${greetingName ? `, ${greetingName}` : ""}`;
  const selectedTaskCalc = selectedTask ? calculateDailyWork(selectedTask) : null;
  const selectedTaskProgress = isTotalWorkloadSelected ? totalWorkload.progress : selectedTask ? getTaskProgress(selectedTask) : 0;
  const weeklyTotalMinutes = weeklyActivity.reduce((sum, entry) => sum + entry.minutes, 0);
  const gardenStage = Math.max(0, [0, 30, 90, 210, 420, 720].filter((threshold) => weeklyTotalMinutes >= threshold).length - 1);
  const todayMinutes = useMemo(() => getTodayMinutes({ sessions: state.sessions } as AppState, calendarToday), [state.sessions, calendarToday]);
  const unitsCompletedToday = useMemo(() => getUnitsCompletedToday({ calendarEntries: state.calendarEntries } as AppState, calendarToday), [state.calendarEntries, calendarToday]);
  const streakDays = useMemo(() => getStreakDays({ sessions: state.sessions } as AppState, calendarToday), [state.sessions, calendarToday]);
  const focusMomentum = useMemo(() => getFocusMomentum({ sessions: state.sessions } as AppState, calendarToday), [state.sessions, calendarToday]);
  const studyBreakTokens = Math.min(STUDY_BREAK_GAME_COUNT, Math.floor(todayMinutes / 45));
  const todayStr = calendarToday;
  const effectiveUnlocked = state.unlockedGamesDate === todayStr ? state.unlockedGames : [];
  const canUnlockMore = effectiveUnlocked.length < studyBreakTokens;
  const minsUntilNext = studyBreakTokens < STUDY_BREAK_GAME_COUNT ? Math.max(1, 45 - (todayMinutes % 45)) : 0;
  const xpProgress = todayMinutes % 45;
  const xpPercent = (xpProgress / 45) * 100;
  const effectivePlayed: PlayedBreak[] = state.playedBreaksDate === todayStr ? state.playedBreaks : [];
  const todayPlayedNames = [...new Set(effectivePlayed.map(p => p.name))];
  const waterCount = state.waterDate === todayStr ? state.waterGlasses : 0;
  const badgeFullHouse = effectiveUnlocked.length === STUDY_BREAK_GAME_COUNT;
  const badgeFirstBreak = state.totalUnlocks >= 1;
  const badgeOnFire = state.unlockStreak >= 3;
  const badgeEarlyBird = effectivePlayed.some(p => new Date(p.playedAt).getHours() < 9);
  const badgeNightOwl = effectivePlayed.some(p => new Date(p.playedAt).getHours() >= 22);
  const badgeSpeedrunnerToday = state.speedrunnerToday && state.lastUnlockDate === todayStr;
  const badgeSpeedrunner = badgeSpeedrunnerToday || (state.badgeCounts.speedrunner ?? 0) > 0;
  const badgeExplorer = state.playedGamesAllTime.length >= STUDY_BREAK_GAME_COUNT;
  const badgePerfectionist = badgeFullHouse && todayPlayedNames.length === STUDY_BREAK_GAME_COUNT;
  const badgeVeteran = state.totalUnlocks >= 10;
  const socialLeaderboard = getLeaderboardWithLocalSelf(state, socialScope, socialPeriod);
  const socialLeaderboardTopMinutes = Math.max(1, ...socialLeaderboard.map((entry) => entry.minutes));
  const squadMemberLeaderboard = getLeaderboardWithLocalSelf(state, "squad", socialPeriod);
  const squadScoreLeaderboard = state.social.cachedSquadScoreLeaderboards[squadScorePeriod] ?? [];
  const socialArenaTitle = socialScope === "global" ? "World Arena" : socialScope === "squad" ? "Squad Arena" : "Friends Arena";
  const socialArenaSubtitle = socialScope === "squad"
    ? squadScorePeriod === "daily" ? "Daily Sprint" : squadScorePeriod === "season" ? "Seasonal Points" : "Overall Points"
    : socialPeriod === "daily" ? "Daily Sprint" : socialPeriod === "weekly" ? "Weekly League" : "Hall of Focus";
  const socialGlobalWeekly = getLeaderboardWithLocalSelf(state, "global", "weekly");
  const socialFriendsWeekly = getLeaderboardWithLocalSelf(state, "friends", "weekly");
  const localSocialDaily = getLocalLeaderboardEntry(state, "daily");
  const localSocialWeekly = getLocalLeaderboardEntry(state, "weekly");
  const localSocialOverall = getLocalLeaderboardEntry(state, "overall");
  // calendarToday forces a recompute at the local day boundary (needed for month-boundary
  // correctness) - getLocalMonthlyStats itself still reads the real wall clock internally at
  // whatever moment this memo actually recomputes, same as the original inline code did.
  // eslint-disable-next-line react-hooks/exhaustive-deps -- calendarToday forces day/month-boundary invalidation, see comment above
  const localSocialMonthly = useMemo(() => getLocalMonthlyStats(state.sessions), [state.sessions, calendarToday]);
  const myGlobalRank = state.social.isPrivate ? undefined : socialGlobalWeekly.find((entry) => entry.userId === state.social.userId)?.rank;
  const myFriendRank = socialFriendsWeekly.find((entry) => entry.userId === state.social.userId)?.rank;
  const socialFeed = state.social.cachedFeeds[feedScope] ?? [];
  const displayedSocialFeed = socialFeed;
  const feedImagesVisible = !state.settings.hideFeedImages;
  const feedPollsVisible = !state.settings.hideFeedPolls;
  const expandedFeedImage = feedImagesVisible && expandedFeedImageId ? socialFeed.find((post) => post.id === expandedFeedImageId && post.imageUrl) : null;
  const friendInviteLink = makeFriendInviteLink(state.social.friendCode);
  const incomingFriendRequestCount = state.social.incomingFriendRequests.length;
  const latestFeedSession = state.sessions.find((session) => session.kind === "study" || session.kind === "exam") ?? null;
  const latestFeedSessionPosted = latestFeedSession
    ? [...state.social.pendingFeedPosts, ...state.social.cachedFeeds.global, ...state.social.cachedFeeds.friends].some((post) => post.id === latestFeedSession.id)
    : false;
  const feedPollHasDraft = Boolean(feedPollDraft.question.trim() || feedPollDraft.options.some((option) => option.trim()));
  const liveFriends = state.social.friends.filter((friend) => {
    return isRecentlyActive(friend.lastSeenAt);
  });
  const wabiAttendanceFriends = state.social.friends
    .filter((friend) => isRecentlyActive(friend.lastSeenAt, WABI_ATTENDANCE_WINDOW_MS))
    .sort((a, b) => (parseSocialTimestamp(b.lastSeenAt) ?? 0) - (parseSocialTimestamp(a.lastSeenAt) ?? 0));
  const weekCompareEntries = socialFriendsWeekly.slice(0, 6);
  const socialConfigured = isSocialApiConfigured();
  useEffect(() => {
    const eligible = isTauriApp()
      && socialConfigured
      && state.timer.running
      && state.timer.phase !== "idle"
      && state.timer.phase !== "break";
    let disposed = false;
    let heartbeatInterval: number | undefined;
    const social = state.social;

    // Offline credit is measured from the PREVIOUS anchor (the last point the server actually
    // acknowledged) forward to the moment a call succeeds again — i.e. only triggers once a real
    // gap (longer than the heartbeat+grace window) is confirmed to have closed, never on the
    // ordinary 15-minute cadence.
    const maybeReconcileOffline = (previousAnchor: SocialState["verifiedAnchor"], confirmedAt: string) => {
      if (!previousAnchor || reconcilingOfflineCreditRef.current) return;
      const staleMs = new Date(confirmedAt).getTime() - new Date(previousAnchor.confirmedAt).getTime();
      if (staleMs <= VERIFIED_SESSION_HEARTBEAT_MS + VERIFIED_SESSION_GRACE_MS) return;

      const intervals = computeOfflineIntervals(latestStateRef.current, previousAnchor.confirmedAt, confirmedAt);
      if (!intervals.length) return;

      reconcilingOfflineCreditRef.current = true;
      void reconcileOfflineVerifiedCredit(social, { anchorSessionId: previousAnchor.sessionId, intervals })
        .then((result) => {
          if (result.cappedFromClaimedMinutes > 0) {
            setMessage(`${result.cappedFromClaimedMinutes} offline minute${result.cappedFromClaimedMinutes === 1 ? "" : "s"} could not be verified.`);
          }
        })
        .catch(() => undefined)
        .finally(() => {
          reconcilingOfflineCreditRef.current = false;
        });
    };

    const confirmAnchor = (sessionId: string) => {
      const previousAnchor = latestStateRef.current.social.verifiedAnchor;
      const confirmedAt = new Date().toISOString();
      setState((current) => ({ ...current, social: { ...current.social, verifiedAnchor: { sessionId, confirmedAt } } }));
      maybeReconcileOffline(previousAnchor, confirmedAt);
    };

    const attemptHeartbeat = (sessionId: string) => {
      void heartbeatVerifiedSession(social, sessionId)
        .then(() => confirmAnchor(sessionId))
        .catch(() => undefined);
    };

    const scheduleHeartbeats = (sessionId: string) => {
      heartbeatInterval = window.setInterval(() => attemptHeartbeat(sessionId), VERIFIED_SESSION_HEARTBEAT_MS);
    };

    const attemptStart = () => {
      void startVerifiedSession(social)
        .then(({ sessionId }) => {
          if (disposed || !timerRunningRef.current) {
            return finishVerifiedSession(social, sessionId).catch(() => undefined);
          }
          verifiedSessionRef.current = sessionId;
          confirmAnchor(sessionId);
          scheduleHeartbeats(sessionId);
          return undefined;
        })
        .catch(() => undefined);
    };

    // Ground-truth reconnect detection is "the next verified-session call actually succeeds" —
    // the `online` listener below just calls this sooner as a latency optimization; it never
    // substitutes for it (`navigator.onLine` only reflects the network interface, not whether
    // the Worker is actually reachable).
    attemptVerifiedSyncRef.current = () => {
      if (!eligible || disposed) return;
      if (verifiedSessionRef.current) attemptHeartbeat(verifiedSessionRef.current);
      else attemptStart();
    };

    if (!eligible) {
      const sessionId = verifiedSessionRef.current;
      verifiedSessionRef.current = null;
      if (sessionId) void finishVerifiedSession(social, sessionId).catch(() => undefined);
      return undefined;
    }

    if (verifiedSessionRef.current) {
      scheduleHeartbeats(verifiedSessionRef.current);
    } else {
      attemptStart();
    }

    return () => {
      disposed = true;
      if (heartbeatInterval !== undefined) window.clearInterval(heartbeatInterval);
    };
  }, [socialConfigured, state.social.deviceSecret, state.social.userId, state.timer.phase, state.timer.running]);

  useEffect(() => {
    const handleOnline = () => attemptVerifiedSyncRef.current();
    window.addEventListener("online", handleOnline);
    return () => window.removeEventListener("online", handleOnline);
  }, []);
  const lastSocialSyncLabel = state.social.lastSyncedAt ? `${formatDate(state.social.lastSyncedAt)} ${new Date(state.social.lastSyncedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}` : "Never";
  const socialFriendIds = new Set(state.social.friends.map((friend) => friend.userId));
  const outgoingFriendRequestCodes = new Set(state.social.outgoingFriendRequests.map((request) => request.toFriendCode));
  const currentSquad = state.social.squad;
  const currentSquadRole = currentSquad?.myRole;
  const canManageCurrentSquadRequests = canManageSquadRequests(currentSquadRole);
  const isLastSquadMember = Boolean(currentSquad && currentSquad.memberCount <= 1);
  const viewingIsSelf = viewingFriend?.userId === state.social.userId;
  const viewingIsFriend = viewingFriend ? socialFriendIds.has(viewingFriend.userId) : false;
  const viewingRequestPending = viewingFriend ? outgoingFriendRequestCodes.has(viewingFriend.friendCode) : false;
  const viewingSquadAction = viewingSquadDetails?.action;
  const rockStage = state.petRockPats >= 8888888 ? { plant: "\u{1F47C}", label: "Guardian Angel" }
    : state.petRockPats >= 6666666 ? { plant: "\u{1F47F}", label: "Demon" }
    : state.petRockPats >= 1000000 ? { plant: "\u{1F5FF}", label: "Rock God" }
    : state.petRockPats >= 888888 ? { plant: "\u{1F54A}\uFE0F", label: "Saint" }
    : state.petRockPats >= 666666 ? { plant: "\u{1F608}", label: "Hell's Diplomat" }
    : state.petRockPats >= 500000 ? { plant: "\u{1F48E}", label: "Ancient Starstone" }
    : state.petRockPats >= 100000 ? { plant: "\u{1F320}", label: "Celestial Rock" }
    : state.petRockPats >= 50000 ? { plant: "\u{1FA90}", label: "Planetary Rock" }
    : state.petRockPats >= 20000 ? { plant: "\u{2604}\uFE0F", label: "Meteoric Rock" }
    : state.petRockPats >= 10000 ? { plant: "\u{1FAA8}", label: "Eternal Rock" }
    : state.petRockPats >= 5000 ? { plant: "\u{1F30C}", label: "Galactic Rock" }
    : state.petRockPats >= 1000 ? { plant: "\u{1F31F}", label: "Cosmic Rock" }
    : state.petRockPats >= 888 ? { plant: "\u{1F607}", label: "Heavenly Rock" }
    : state.petRockPats >= 666 ? { plant: "\u{1F525}", label: "Hellish Rock" }
    : state.petRockPats >= 500 ? { plant: "\u{1F451}", label: "Royal Rock" }
    : state.petRockPats >= 250 ? { plant: "\u{1F98B}", label: "Blooming Rock" }
    : state.petRockPats >= 100 ? { plant: "\u{1F333}", label: "Flourished Rock" }
    : state.petRockPats >= 50 ? { plant: "\u{1F33F}", label: "Growing Rock" }
    : state.petRockPats >= 10 ? { plant: "\u{1F331}", label: "Sprouting Rock" }
    : { plant: "", label: "Pet Rock" };
  const streakEmoji = state.unlockStreak >= 7 ? "\u{1F525}\u{1F525}\u{1F525}" : state.unlockStreak >= 3 ? "\u{1F525}\u{1F525}" : state.unlockStreak >= 1 ? "\u{1F525}" : "";
  const petRockMilestones = [
    { id: "rock-sprouting", icon: "\u{1F331}", name: "Sprouting Rock", threshold: 10, label: "10" },
    { id: "rock-growing", icon: "\u{1F33F}", name: "Growing Rock", threshold: 50, label: "50" },
    { id: "rock-flourished", icon: "\u{1F333}", name: "Flourished Rock", threshold: 100, label: "100" },
    { id: "rock-blooming", icon: "\u{1F98B}", name: "Blooming Rock", threshold: 250, label: "250" },
    { id: "rock-royal", icon: "\u{1F451}", name: "Royal Rock", threshold: 500, label: "500" },
    { id: "rock-hellish", icon: "\u{1F525}", name: "Hellish Rock", threshold: 666, label: "666" },
    { id: "rock-heavenly", icon: "\u{1F607}", name: "Heavenly Rock", threshold: 888, label: "888" },
    { id: "rock-cosmic", icon: "\u{1F31F}", name: "Cosmic Rock", threshold: 1000, label: "1k" },
    { id: "rock-galactic", icon: "\u{1F30C}", name: "Galactic Rock", threshold: 5000, label: "5k" },
    { id: "rock-eternal", icon: "\u{1FAA8}", name: "Eternal Rock", threshold: 10000, label: "10k" },
    { id: "rock-meteoric", icon: "\u{2604}\uFE0F", name: "Meteoric Rock", threshold: 20000, label: "20k" },
    { id: "rock-planetary", icon: "\u{1FA90}", name: "Planetary Rock", threshold: 50000, label: "50k" },
    { id: "rock-celestial", icon: "\u{1F320}", name: "Celestial Rock", threshold: 100000, label: "100k" },
    { id: "rock-starstone", icon: "\u{1F48E}", name: "Ancient Starstone", threshold: 500000, label: "500k" },
    { id: "rock-hells-diplomat", icon: "\u{1F608}", name: "Hell's Diplomat", threshold: 666666, label: "666,666" },
    { id: "rock-saint", icon: "\u{1F54A}\uFE0F", name: "Saint", threshold: 888888, label: "888,888" },
    { id: "rock-god", icon: "\u{1F5FF}", name: "Rock God", threshold: 1000000, label: "1M" },
    { id: "rock-demon", icon: "\u{1F47F}", name: "Demon", threshold: 6666666, label: "6,666,666" },
    { id: "rock-guardian-angel", icon: "\u{1F47C}", name: "Guardian Angel", threshold: 8888888, label: "8,888,888" },
  ];
  const currentRockBadge = petRockMilestones.findLast((badge) => state.petRockPats >= badge.threshold);
  const nextRockBadge = petRockMilestones.find((badge) => state.petRockPats < badge.threshold);
  const badges: ProfileBadge[] = [
    { id: "full-house", icon: "\u{1F3C6}", name: "Full House", earned: badgeFullHouse || (state.badgeCounts["full-house"] ?? 0) > 0, daily: true, how: `Unlock all ${STUDY_BREAK_GAME_COUNT} break games in one day.` },
    { id: "first-break", icon: "\u{2B50}", name: "First Break", earned: badgeFirstBreak, how: "Unlock any Break Room game once." },
    { id: "on-fire", icon: "\u{1F525}", name: "On Fire", earned: badgeOnFire, how: "Unlock at least one break game on 3 consecutive days." },
    { id: "early-bird", icon: "\u{1F305}", name: "Early Bird", earned: badgeEarlyBird || (state.badgeCounts["early-bird"] ?? 0) > 0, daily: true, how: "Play an unlocked break game before 9:00 AM." },
    { id: "night-owl", icon: "\u{1F989}", name: "Night Owl", earned: badgeNightOwl || (state.badgeCounts["night-owl"] ?? 0) > 0, daily: true, how: "Play an unlocked break game at or after 10:00 PM." },
    { id: "speedrunner", icon: "\u{26A1}", name: "Speedrunner", earned: badgeSpeedrunner, daily: true, how: "Unlock your first break after earning a 45-minute break token from one single study or exam session." },
    { id: "explorer", icon: "\u{1F5FA}\uFE0F", name: "Explorer", earned: badgeExplorer, how: `Play all ${STUDY_BREAK_GAME_COUNT} different break games at least once.` },
    { id: "perfectionist", icon: "\u{1F3AF}", name: "Perfectionist", earned: badgePerfectionist || (state.badgeCounts.perfectionist ?? 0) > 0, daily: true, how: `Unlock all ${STUDY_BREAK_GAME_COUNT} games and play all ${STUDY_BREAK_GAME_COUNT} games on the same day.` },
    { id: "veteran", icon: "\u{1F48E}", name: "Veteran", earned: badgeVeteran, how: "Unlock break games 10 total times." },
    { id: "rock-current", icon: currentRockBadge?.icon ?? "\u{1FAA8}", name: currentRockBadge?.name ?? "Pet Rock", earned: Boolean(currentRockBadge), how: nextRockBadge ? `Pat the pet rock ${nextRockBadge.label} times.` : "Reach the final pet rock form." },
  ];
  const petRockBadges: ProfileBadge[] = petRockMilestones.map((badge) => ({
    id: badge.id,
    icon: badge.icon,
    name: badge.name,
    earned: state.petRockPats >= badge.threshold,
    how: `Pat the pet rock ${badge.label} times.`,
  }));
  const wordleKeyboardState = getWordleKeyboardState(state.wordlePuzzle.guesses, state.wordlePuzzle.answer);
  const scoredWordleRows = state.wordlePuzzle.guesses.map((guess) => scoreWordleGuess(guess, state.wordlePuzzle.answer));
  const draftWordleRows = !state.wordlePuzzle.completed ? [Array.from({ length: WORDLE_WORD_LENGTH }, (_, colIndex) => ({ letter: wordleDraft[colIndex] ?? "", state: null }))] : [];
  const wordleRows = Array.from({ length: WORDLE_MAX_GUESSES }, (_, rowIndex) => [...scoredWordleRows, ...draftWordleRows][rowIndex] ?? Array.from({ length: WORDLE_WORD_LENGTH }, () => ({ letter: "", state: null })));
  const wordleHardModeLocked = state.wordlePuzzle.guesses.length > 0 && !state.wordlePuzzle.completed;
  const geodleOptions = filterCountries(geodleDraft).slice(0, 80);
  const geodleRows = state.geodlePuzzle.guesses.toReversed().map((guess) => ({ guess, clues: scoreGeodleGuess(guess, state.geodlePuzzle.answer) }));
  const geodleGuessesLeft = Math.max(0, GEODLE_MAX_GUESSES - state.geodlePuzzle.guesses.length);
  const flaggleOptions = filterFlaggleCountries(flaggleDraft).slice(0, 80);
  const flaggleRows = state.flagglePuzzle.guesses.toReversed();
  const flaggleGuessesLeft = Math.max(0, FLAGGLE_MAX_GUESSES - state.flagglePuzzle.guesses.length);
  const travleOptions = filterTravleCountries(travleDraft).slice(0, 80);
  const travleRoute = [state.travlePuzzle.start, ...state.travlePuzzle.guesses];
  const travleCurrent = state.travlePuzzle.guesses.at(-1) ?? state.travlePuzzle.start;
  const travleGuessesLeft = Math.max(0, TRAVLE_MAX_GUESSES - state.travlePuzzle.guesses.length);
  const travleShortestPath = getTravleShortestPath(state.travlePuzzle.start, state.travlePuzzle.target);
  const travleSolvedPath = getTravleSolvedPath(state.travlePuzzle.start, state.travlePuzzle.target, state.travlePuzzle.guesses);
  const travleDisplayPath = getTravleDisplayPath(state.travlePuzzle.start, state.travlePuzzle.target, state.travlePuzzle.guesses);
  const travleGuessStates = getTravleGuessStates(state.travlePuzzle.start, state.travlePuzzle.target, state.travlePuzzle.guesses);
  const travleMapCountryByCode = useMemo(() => new Map(TRAVLE_MAP_COUNTRIES.map((country) => [country.code, country])), []);
  const travleCountryNameByCode = useMemo(() => new Map(filterTravleCountries("").map((country) => [country.code, country.name])), []);
  const travleRouteCodes = travleRoute.flatMap((countryName) => {
    const country = findTravleCountry(countryName);
    return country ? [country.code] : [];
  });
  const travleTargetCode = findTravleCountry(state.travlePuzzle.target)?.code ?? "";
  const travleCurrentCode = findTravleCountry(travleCurrent)?.code ?? "";
  const travleRouteCodeSet = new Set(travleRouteCodes);
  const travleSolvedCodeSet = new Set(travleSolvedPath.flatMap((countryName) => {
    const country = findTravleCountry(countryName);
    return country ? [country.code] : [];
  }));
  const travleDisplayPathCodes = travleDisplayPath.flatMap((countryName) => {
    const country = findTravleCountry(countryName);
    return country ? [country.code] : [];
  });
  const travleMapRoutePoints = travleDisplayPathCodes.flatMap((code) => {
    const country = travleMapCountryByCode.get(code);
    if (!country) return [];
    const point = country.point ?? [(country.bounds[0] + country.bounds[2]) / 2, (country.bounds[1] + country.bounds[3]) / 2];
    return [`${point[0]},${point[1]}`];
  }).join(" ");
  const travleMapViewBox = useMemo(() => {
    const focusCodes = new Set<string>([...travleRouteCodes, travleTargetCode].filter(Boolean));
    const focusCountries = [...focusCodes].flatMap((code) => {
      const country = travleMapCountryByCode.get(code);
      return country ? [country] : [];
    });
    if (!focusCountries.length) return "0 0 1000 500";
    const bounds = focusCountries.reduce((box, country) => ({
      minX: Math.min(box.minX, country.bounds[0]),
      minY: Math.min(box.minY, country.bounds[1]),
      maxX: Math.max(box.maxX, country.bounds[2]),
      maxY: Math.max(box.maxY, country.bounds[3]),
    }), { minX: Infinity, minY: Infinity, maxX: -Infinity, maxY: -Infinity });
    const centerX = (bounds.minX + bounds.maxX) / 2;
    const centerY = (bounds.minY + bounds.maxY) / 2;
    const fitWidth = Math.max(120, bounds.maxX - bounds.minX + 80);
    const fitHeight = Math.max(90, bounds.maxY - bounds.minY + 70);
    const width = Math.min(1000, fitWidth / travleMapZoom);
    const height = Math.min(500, fitHeight / travleMapZoom);
    const x = Math.max(0, Math.min(1000 - width, centerX - width / 2));
    const y = Math.max(0, Math.min(500 - height, centerY - height / 2));
    return `${x} ${y} ${width} ${height}`;
  }, [travleMapCountryByCode, travleMapZoom, travleRouteCodes, travleTargetCode]);
  const wordlePuzzleIsToday = Boolean(state.wordlePuzzle.seedSalt && state.wordlePuzzle.activeDate === todayStr && state.wordlePuzzle.puzzleId === getWordlePuzzleId(todayStr, state.wordlePuzzle.seedSalt));
  const geodlePuzzleIsToday = Boolean(state.geodlePuzzle.seedSalt && state.geodlePuzzle.activeDate === todayStr && state.geodlePuzzle.puzzleId === getGeodlePuzzleId(todayStr, state.geodlePuzzle.seedSalt));
  const flagglePuzzleIsToday = Boolean(state.flagglePuzzle.seedSalt && state.flagglePuzzle.activeDate === todayStr && state.flagglePuzzle.puzzleId === getFlagglePuzzleId(todayStr, state.flagglePuzzle.seedSalt));
  const travlePuzzleIsToday = Boolean(state.travlePuzzle.seedSalt && state.travlePuzzle.activeDate === todayStr && state.travlePuzzle.puzzleId === getTravlePuzzleId(todayStr, state.travlePuzzle.seedSalt));

  useEffect(() => {
    const dailyHits = [
      ["full-house", badgeFullHouse],
      ["early-bird", badgeEarlyBird],
      ["night-owl", badgeNightOwl],
      ["speedrunner", badgeSpeedrunnerToday],
      ["perfectionist", badgePerfectionist],
    ] as const;

    const earnedToday = dailyHits.filter(([id, earned]) => earned && state.badgeCountDates[id] !== todayStr);
    if (!earnedToday.length) return;

    setState((current) => {
      const nextCounts = { ...current.badgeCounts };
      const nextDates = { ...current.badgeCountDates };
      let changed = false;

      earnedToday.forEach(([id]) => {
        if (nextDates[id] === todayStr) return;
        nextCounts[id] = (nextCounts[id] ?? 0) + 1;
        nextDates[id] = todayStr;
        changed = true;
      });

      return changed ? { ...current, badgeCounts: nextCounts, badgeCountDates: nextDates } : current;
    });
  }, [badgeEarlyBird, badgeFullHouse, badgeNightOwl, badgePerfectionist, badgeSpeedrunnerToday, state.badgeCountDates, todayStr]);
  const openTaskCount = useMemo(() => state.tasks.filter((task) => getRemainingUnits(task) > 0).length, [state.tasks]);
  const completionRadius = 58;
  const completionCircumference = 2 * Math.PI * completionRadius;
  const completionOffset = completionCircumference - (selectedTaskProgress / 100) * completionCircumference;

  const timerCourses = state.timer.semesterId ? getSemesterCourses(state, state.timer.semesterId) : state.courses;
  const timerTasks = state.timer.courseId ? getCourseTasks(state, state.timer.courseId) : [];
  const timerSelectableTasks = state.timer.courseId
    ? timerTasks
    : state.timer.semesterId
      ? getSemesterTasks(state, state.timer.semesterId)
      : state.tasks;
  const timerCourse = state.timer.courseId ? courseLookup.get(state.timer.courseId) : null;
  const timerTask = state.timer.taskId ? taskLookup.get(state.timer.taskId) : null;
  const hasKnownTimerPreset = focusPresets.some((preset) => preset.label === state.timer.presetLabel);
  const isCustomTimerPreset = !hasKnownTimerPreset || state.timer.presetLabel === "Custom";
  // The progress ring's --timer-progress CSS variable is mutated directly on these two
  // elements by useTimerProgressRing's own interval, instead of flowing through a per-tick
  // root AppState field and a declarative style prop - see hooks/useTimerProgressRing.ts.
  useTimerProgressRing(timerFaceRingRef, state.timer);
  useTimerProgressRing(examFaceRingRef, state.timer);

  const calendarDays = useMemo(
    () => (calendarView === "month" ? buildMonthDays(calendarCursorDate) : buildWeekDays(calendarCursorDate)),
    [calendarCursorDate, calendarView],
  );
  const calendarTitle = useMemo(
    () => formatCalendarTitle(calendarCursorDate, calendarView),
    [calendarCursorDate, calendarView],
  );
  const calendarEntriesByDate = useMemo(() => {
    const map = new Map<string, CalendarEntry[]>();
    state.calendarEntries.forEach((entry) => {
      const entries = map.get(entry.date) ?? [];
      entries.push(entry);
      map.set(entry.date, entries);
    });
    map.forEach((entries) => entries.sort((a, b) => a.createdAt.localeCompare(b.createdAt)));
    return map;
  }, [state.calendarEntries]);
  const examsByDate = useMemo(() => {
    const map = new Map<string, Exam[]>();
    state.exams.forEach((exam) => {
      const exams = map.get(exam.examDate) ?? [];
      exams.push(exam);
      map.set(exam.examDate, exams);
    });
    return map;
  }, [state.exams]);
  const deadlinesByDate = useMemo(() => {
    const map = new Map<string, Task[]>();
    state.tasks.forEach((task) => {
      if (!task.dueDate) return;
      const tasks = map.get(task.dueDate) ?? [];
      tasks.push(task);
      map.set(task.dueDate, tasks);
    });
    return map;
  }, [state.tasks]);
  const scheduledIncompleteByTask = useMemo(() => {
    const map = new Map<string, number>();
    state.calendarEntries.forEach((entry) => {
      if (entry.completed) return;
      map.set(entry.taskId, (map.get(entry.taskId) ?? 0) + getCalendarEntryAmount(entry));
    });
    return map;
  }, [state.calendarEntries]);
  const completedCalendarRemainderByTask = useMemo(() => {
    const totals = new Map<string, number>();
    state.calendarEntries.forEach((entry) => {
      if (!entry.completed) return;
      totals.set(entry.taskId, (totals.get(entry.taskId) ?? 0) + getCalendarEntryAmount(entry));
    });
    const remainders = new Map<string, number>();
    totals.forEach((amount, taskId) => remainders.set(taskId, amount - Math.floor(amount + 0.0001)));
    return remainders;
  }, [state.calendarEntries]);
  const todayCalendarEntries = useMemo(
    () =>
      state.calendarEntries
        .filter((entry) => entry.date === calendarToday && (taskLookup.has(entry.taskId) || entry.adHocTitle))
        .sort((a, b) => {
          if (a.completed !== b.completed) return a.completed ? 1 : -1;
          return (a.startTime ?? a.createdAt).localeCompare(b.startTime ?? b.createdAt);
        }),
    [calendarToday, state.calendarEntries, taskLookup],
  );

  const totalAllTimeMinutes = state.lifetimeStudyMinutes;
  const sessionDays = useMemo(() => getSessionDaySet(state.sessions), [state.sessions]);
  const firstSessionDate = useMemo(() => getFirstSessionDate(state.sessions), [state.sessions]);

  const focusTimeline = useMemo(() => {
    type FossilLayer = { id: string; name: string; color: string; minutes: number };
    type FossilDay = { date: string; isToday: boolean; totalMinutes: number; layers: FossilLayer[]; cumulative: number };

    const sortedSessions = [...state.sessions].sort((a, b) => a.endedAt.localeCompare(b.endedAt));
    const dayMap = new Map<string, StudySession[]>();
    for (const session of sortedSessions) {
      const key = isoDate(new Date(session.endedAt));
      const sessions = dayMap.get(key) ?? [];
      sessions.push(session);
      dayMap.set(key, sessions);
    }

    const days: FossilDay[] = [];
    const today = isoDate();
    const rangeStartDate = new Date();
    const rangeDayCount = focusRange === "week" ? 7 : focusRange;
    if (focusRange === "week") {
      const day = rangeStartDate.getDay();
      const mondayOffset = day === 0 ? -6 : 1 - day;
      rangeStartDate.setDate(rangeStartDate.getDate() + mondayOffset);
    } else {
      rangeStartDate.setDate(rangeStartDate.getDate() - focusRange + 1);
    }
    const rangeStart = isoDate(rangeStartDate);
    let cumulative = sortedSessions
      .filter((session) => isoDate(new Date(session.endedAt)) < rangeStart)
      .reduce((sum, session) => sum + session.minutes, 0);
    let maxDayMinutes = 30;
    const milestoneHits: Array<(typeof focusMilestones)[number] & { dayIndex: number; date: string }> = [];

    for (let i = 0; i < rangeDayCount; i++) {
      const d = new Date(rangeStartDate);
      d.setDate(rangeStartDate.getDate() + i);
      const key = isoDate(d);
      const sessions = dayMap.get(key) ?? [];
      const totalMinutes = sessions.reduce((sum, session) => sum + session.minutes, 0);
      const previousCumulative = cumulative;
      cumulative += totalMinutes;
      maxDayMinutes = Math.max(maxDayMinutes, totalMinutes);

      focusMilestones.forEach((milestone) => {
        const threshold = milestone.hours * 60;
        if (previousCumulative < threshold && cumulative >= threshold) {
          milestoneHits.push({ ...milestone, dayIndex: days.length, date: key });
        }
      });

      const layerMap = new Map<string, FossilLayer>();
      sessions.forEach((session) => {
        const course = session.courseId ? courseLookup.get(session.courseId) : null;
        const id = course?.id ?? "general";
        const current = layerMap.get(id) ?? {
          id,
          name: course?.name ?? "General",
          color: course?.color ?? "var(--ink-4)",
          minutes: 0,
        };
        current.minutes += session.minutes;
        layerMap.set(id, current);
      });

      days.push({
        date: key,
        isToday: key === today,
        totalMinutes,
        layers: [...layerMap.values()].sort((a, b) => b.minutes - a.minutes),
        cumulative,
      });
    }

    const activeDays = days.filter((day) => day.totalMinutes > 0).length;
    const biggestDay = days.reduce((biggest, day) => (day.totalMinutes > biggest.totalMinutes ? day : biggest), days[0] ?? { date: "", isToday: false, totalMinutes: 0, layers: [], cumulative: 0 });
    const activeCourseMap = new Map<string, FossilLayer>();
    days.forEach((day) => {
      day.layers.forEach((layer) => {
        const current = activeCourseMap.get(layer.id) ?? { ...layer, minutes: 0 };
        current.minutes += layer.minutes;
        activeCourseMap.set(layer.id, current);
      });
    });

    const achievedMilestones = focusMilestones.filter((milestone) => totalAllTimeMinutes >= milestone.hours * 60);
    const visibleMinutes = days.reduce((sum, day) => sum + day.totalMinutes, 0);

    return {
      days,
      milestoneHits,
      today,
      activeDays,
      biggestDay,
      maxDayMinutes,
      activeCourses: [...activeCourseMap.values()].sort((a, b) => b.minutes - a.minutes),
      achievedMilestones,
      latestMilestone: achievedMilestones.at(-1) ?? null,
      visibleMinutes,
    };
  }, [courseLookup, focusRange, state.sessions, totalAllTimeMinutes]);

  const studySessionCount = useMemo(() => getStudySessionCount(state.sessions), [state.sessions]);
  const weeklyCourseCount = useMemo(() => getWeeklyCourseCount(state.sessions, calendarToday), [state.sessions, calendarToday]);
  const completedTaskCount = useMemo(() => state.tasks.filter((task) => task.totalUnits > 0 && task.completedUnits >= task.totalUnits).length, [state.tasks]);

  const profileBadgeGroups: ProfileBadgeGroup[] = [
    {
      category: "Break Room",
      source: "Unlock and play break games from the Break Room.",
      badges: badges.map((badge) => ({
        ...badge,
        count: badge.daily ? state.badgeCounts[badge.id] ?? 0 : undefined,
      })),
      subgroups: [
        {
          category: "Pet Rock",
          source: "Earn these by patting the Break Room pet rock.",
          badges: petRockBadges,
        },
      ],
    },
    {
      category: "Focus Fossil",
      source: "Earn these by building all-time study hours.",
      badges: focusMilestones.map((milestone) => ({
        id: `fossil-${milestone.hours}`,
        icon: "◆",
        name: milestone.label,
        how: milestone.desc,
        earned: totalAllTimeMinutes >= milestone.hours * 60,
      })),
    },
    {
      category: "Garden of Knowledge",
      source: "Earn these by keeping your study garden alive.",
      badges: [
        {
          id: "garden-first-sprout",
          icon: "❀",
          name: "First Sprout",
          how: "Log your first study session.",
          earned: studySessionCount >= 1,
        },
        {
          id: "garden-streak-bloom",
          icon: "✺",
          name: "Streak Bloom",
          how: "Maintain a 5-day study streak.",
          earned: streakDays >= 5,
        },
        {
          id: "garden-mushroom-ring",
          icon: "❁",
          name: "Mushroom Ring",
          how: "Log 20 study sessions.",
          earned: studySessionCount >= 20,
        },
        {
          id: "garden-cross-pollinator",
          icon: "✤",
          name: "Cross-Pollinator",
          how: "Study 3 or more different courses in the last 7 days.",
          earned: weeklyCourseCount >= 3,
        },
        {
          id: "garden-full-bloom",
          icon: "◇",
          name: "Full Bloom",
          how: "Reach the Blooming garden stage (420+ minutes in 7 days).",
          earned: gardenStage >= 4,
        },
        {
          id: "garden-harvest-season",
          icon: "❋",
          name: "Harvest Season",
          how: "Fully complete 10 tasks.",
          earned: completedTaskCount >= 10,
        },
        {
          id: "garden-wise-tree",
          icon: "♣",
          name: "The Wise Tree",
          how: "Reach 720 minutes in the last 7 days, or maintain a 10-day study streak.",
          earned: gardenStage >= 5 || streakDays >= 10,
        },
      ],
    },
  ];

  const renderProfileBadgeCard = (badge: ProfileBadge) => (
    <button key={badge.id} type="button" className={`profile-badge-card ${badge.earned ? "earned" : "locked"}`}>
      <span className="profile-badge-icon">{badge.icon}</span>
      <span className="profile-badge-name">{badge.name}</span>
      {badge.daily ? <span className="profile-badge-count">×{badge.count ?? 0}</span> : null}
      <span className="profile-badge-tip" role="tooltip">
        <strong>{badge.earned ? "Unlocked" : "Locked"}</strong>
        <small>{badge.how}</small>
      </span>
    </button>
  );

  function setActiveTab(activeTab: TabKey) {
    if (activeTab === "friends") setHasUnreadSocial(false);
    setTourState(null);
    setState((current) => ({ ...current, activeTab }));
  }

  function openPageHelp(tab: TabKey) {
    setHelpTab(tab);
    setMenuHelpOpen(false);
    setTourState(null);
  }

  function openMenuHelp() {
    setMenuHelpOpen(true);
    setHelpTab(null);
    setTourState(null);
  }

  function ensurePlannerTutorialData() {
    const createdAt = new Date().toISOString();
    setState((current) => {
      const hasSemester = current.semesters.some((semester) => semester.id === TUTORIAL_SEMESTER_ID);
      const hasCourse = current.courses.some((course) => course.id === TUTORIAL_COURSE_ID);
      const hasTask = current.tasks.some((task) => task.id === TUTORIAL_TASK_ID);

      if (hasSemester && hasCourse && hasTask) return current;

      const semesters = hasSemester
        ? current.semesters
        : [{ id: TUTORIAL_SEMESTER_ID, name: TUTORIAL_SEMESTER_NAME, createdAt }, ...current.semesters];
      const courses = hasCourse
        ? current.courses
        : [{ id: TUTORIAL_COURSE_ID, semesterId: TUTORIAL_SEMESTER_ID, name: TUTORIAL_COURSE_NAME, targetGrade: 5, color: "#8fb4ff", createdAt }, ...current.courses];
      const tasks = hasTask
        ? current.tasks
        : [{
            id: TUTORIAL_TASK_ID,
            semesterId: TUTORIAL_SEMESTER_ID,
            courseId: TUTORIAL_COURSE_ID,
            title: TUTORIAL_TASK_TITLE,
            unitLabel: "Lecture",
            totalUnits: 10,
            completedUnits: 3,
            dueDate: null,
            priority: "medium" as Priority,
            notes: "This task was created by the Planner tutorial. Remove it when you are done.",
            createdAt,
          }, ...current.tasks];

      return { ...current, semesters, courses, tasks };
    });
    setExpandedSemesterIds((current) => (current.includes(TUTORIAL_SEMESTER_ID) ? current : [...current, TUTORIAL_SEMESTER_ID]));
    setExpandedCourseIds((current) => (current.includes(TUTORIAL_COURSE_ID) ? current : [...current, TUTORIAL_COURSE_ID]));
    setAddingCourseSemesterId(null);
    setAddingTaskCourseId(null);
    setSelectedTaskId(TUTORIAL_TASK_ID);
  }

  function startPageTour(tab: TabKey) {
    setHelpTab(null);
    if (tab === "planner") ensurePlannerTutorialData();
    setActiveTab(tab);
    setTourState({ tab, step: 0 });
  }

  const ensurePlannerTutorialDataForEffect = useEffectEvent(() => {
    ensurePlannerTutorialData();
  });

  useEffect(() => {
    if (!tourState) return;
    const step = pageHelp[tourState.tab].tour[tourState.step];
    if (!step) return;

    const desiredTab = step.tab ?? tourState.tab;
    if (state.activeTab !== desiredTab) {
      setState((current) => ({ ...current, activeTab: desiredTab }));
    }

    if (desiredTab === "planner") {
      if (tourState.tab === "planner") ensurePlannerTutorialDataForEffect();

      if (step.target === "planner-semester-name" || step.target === "planner-create-semester") {
        setShowSemesterForm(true);
      }

      const firstSemester = state.semesters.find((semester) => semester.id === TUTORIAL_SEMESTER_ID) ?? state.semesters[0];
      const firstCourse = state.courses.find((course) => course.id === TUTORIAL_COURSE_ID) ?? (firstSemester ? state.courses.find((course) => course.semesterId === firstSemester.id) : null);
      if (firstSemester && (step.target.startsWith("planner-add-course") || step.target.startsWith("planner-course"))) {
        setExpandedSemesterIds((current) => (current.includes(firstSemester.id) ? current : [...current, firstSemester.id]));
        setAddingCourseSemesterId(firstSemester.id);
      }
      if (firstSemester && firstCourse && (step.target.startsWith("planner-add-task") || step.target.startsWith("planner-task"))) {
        setExpandedSemesterIds((current) => (current.includes(firstSemester.id) ? current : [...current, firstSemester.id]));
        setExpandedCourseIds((current) => (current.includes(firstCourse.id) ? current : [...current, firstCourse.id]));
        setAddingTaskCourseId(firstCourse.id);
      }
      if (firstSemester?.id === TUTORIAL_SEMESTER_ID) {
        setExpandedSemesterIds((current) => (current.includes(TUTORIAL_SEMESTER_ID) ? current : [...current, TUTORIAL_SEMESTER_ID]));
        setExpandedCourseIds((current) => (current.includes(TUTORIAL_COURSE_ID) ? current : [...current, TUTORIAL_COURSE_ID]));
      }
    }

    if (desiredTab === "timer" && tourState.tab === "planner") {
      setState((current) => ({
        ...current,
        timer: {
          ...current.timer,
          semesterId: TUTORIAL_SEMESTER_ID,
          courseId: TUTORIAL_COURSE_ID,
          taskId: TUTORIAL_TASK_ID,
          goal: current.timer.goal || TUTORIAL_TASK_TITLE,
        },
      }));
    }

    if (desiredTab === "timer" && step.target === "timer-custom") {
      setState((current) => ({
        ...current,
        timer: {
          ...current.timer,
          mode: "focus",
          presetLabel: "Custom",
        },
      }));
    }

    if (desiredTab === "timer" && step.target.startsWith("timer-") && step.target !== "timer-presets" && step.target !== "timer-links" && step.target !== "timer-face" && step.target !== "timer-actions") {
      setTimerAdvancedOpen(true);
    }

    if (desiredTab === "vault") {
      if (step.target.startsWith("vault-setup") || step.target === "vault-name" || step.target === "vault-path" || step.target === "vault-create" || step.target === "vault-link") {
        setVaultSetupOpen(true);
      }
      if (step.target.startsWith("vault-daily") || step.target === "vault-load" || step.target === "vault-session-draft" || step.target === "vault-edit" || step.target === "vault-editor" || step.target === "vault-recent") {
        setVaultSpace("daily");
      }
      if (step.target === "vault-editor") setVaultDailyEditing(true);
    }

    if (desiredTab === "friends") {
      if (step.target === "social-leaderboard-tab") setSocialSubtab("leaderboard");
      else if (step.target === "social-friends-tab") setSocialSubtab("friends");
      else if (step.target === "social-squad-tab") setSocialSubtab("squad");
      else if (step.target === "social-profile-tab") setSocialSubtab("profile");
      else setSocialSubtab("feed");
    }
  }, [tourState, state.activeTab, state.courses, state.semesters]);

  function updateFeedCommentNoticeFromFeeds(feedsByScope: Array<{ scope: SocialFeedScope; feed: SocialFeedPost[] }>) {
    const currentState = latestStateRef.current;
    let seenCommentIds = seenFeedCommentIdsRef.current;
    if (!seenCommentIds) {
      seenCommentIds = new Set(
        [...currentState.social.cachedFeeds.global, ...currentState.social.cachedFeeds.friends]
          .flatMap((post) => post.comments ?? [])
          .map((comment) => comment.id),
      );
      seenFeedCommentIdsRef.current = seenCommentIds;
    }

    let nextNotice: FeedCommentNotice | null = null;
    for (const { scope, feed } of feedsByScope) {
      for (const post of feed) {
        const isOwnPost = post.userId === currentState.social.userId || post.isSelf;
        for (const comment of post.comments ?? []) {
          const isNewComment = !seenCommentIds.has(comment.id);
          if (isNewComment && isOwnPost && comment.userId !== currentState.social.userId && !comment.isSelf && !nextNotice) {
            nextNotice = {
              postId: post.id,
              scope,
              commenterName: comment.displayName,
              body: comment.body,
            };
          }
          seenCommentIds.add(comment.id);
        }
      }
    }

    if (nextNotice) setFeedCommentNotice(nextNotice);
  }

  function openFeedCommentNotice(notice: FeedCommentNotice) {
    setActiveTab("friends");
    setSocialSubtab("feed");
    setFeedScope(notice.scope);
    setExpandedFeedComments((current) => new Set(current).add(notice.postId));
    setFeedCommentNotice(null);
  }

  function toggleTabVisibility(tab: TabKey) {
    setState((current) => {
      const visibleTabs = current.settings.visibleTabs ?? defaultState.settings.visibleTabs;
      const isVisible = visibleTabs[tab] !== false;
      const visibleCount = primaryTabs.filter(({ id }) => visibleTabs[id] !== false).length;

      if (isVisible && visibleCount <= 1) return current;

      const nextVisibleTabs = { ...visibleTabs, [tab]: !isVisible };
      const nextActiveTab = nextVisibleTabs[current.activeTab] !== false
        ? current.activeTab
        : primaryTabs.find(({ id }) => nextVisibleTabs[id] !== false)?.id ?? "dashboard";

      return {
        ...current,
        activeTab: nextActiveTab,
        settings: {
          ...current.settings,
          visibleTabs: nextVisibleTabs,
        },
      };
    });
  }

  function toggleBackgroundEffect() {
    setState((current) => ({
      ...current,
      settings: {
        ...current.settings,
        backgroundEffect: current.settings.backgroundEffect === false,
      },
    }));
  }

  function toggleFeedImages() {
    setState((current) => ({
      ...current,
      settings: {
        ...current.settings,
        hideFeedImages: !current.settings.hideFeedImages,
      },
    }));
    if (!state.settings.hideFeedImages) setExpandedFeedImageId(null);
  }

  function toggleFeedPolls() {
    setState((current) => ({
      ...current,
      settings: {
        ...current.settings,
        hideFeedPolls: !current.settings.hideFeedPolls,
      },
    }));
  }

  function toggleHelpButton() {
    setState((current) => ({
      ...current,
      settings: {
        ...current.settings,
        showHelpButton: current.settings.showHelpButton === false,
      },
    }));
  }

  function toggleTelemetry() {
    setState((current) => ({
      ...current,
      settings: {
        ...current.settings,
        telemetryEnabled: !current.settings.telemetryEnabled,
      },
    }));
  }

  async function copyFriendCode() {
    try {
      await navigator.clipboard.writeText(state.social.friendCode);
      setMessage("Friend code copied.");
    } catch (error: unknown) {
      console.warn("Could not copy friend code.", error);
      setMessage("Could not copy the friend code. Select it manually instead.");
    }
  }

  async function copyFriendInviteLink() {
    try {
      await navigator.clipboard.writeText(makeFriendInviteLink(state.social.friendCode));
      setMessage("Friend invite link copied.");
    } catch (error: unknown) {
      console.warn("Could not copy friend invite link.", error);
      setMessage("Could not copy the invite link. Select it manually instead.");
    }
  }

  useEffect(() => {
    if (!emojiPickerPostId) return;
    function handleClick(event: globalThis.MouseEvent) {
      const target = event.target as HTMLElement;
      if (!target.closest(".emoji-picker") && !target.closest(".reaction-btn--add")) {
        setEmojiPickerPostId(null);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [emojiPickerPostId]);

  async function runSocialSync(options: { silent?: boolean; stateOverride?: AppState } = {}) {
    const { silent = false, stateOverride } = options;
    const syncState = stateOverride ?? state;
    if (!isSocialApiConfigured()) {
      if (!silent) setMessage("Social sync is not configured yet. Add the Cloudflare Worker URL first.");
      setState((current) => ({
        ...current,
        social: {
          ...current.social,
          lastSyncError: "Social sync is not configured yet.",
        },
      }));
      return;
    }

    if (socialSyncInProgressRef.current) return;
    socialSyncInProgressRef.current = true;
    setSocialSyncing(true);
    try {
      const result = await syncSocialState(syncState, getAppMetadata());
      const deletedPostIds: string[] = [];
      for (const postId of syncState.social.pendingFeedPostDeletions) {
        try {
          await deleteFeedPost(syncState.social, postId);
          deletedPostIds.push(postId);
        } catch (error) {
          console.warn("Could not delete queued feed post.", postId, error);
        }
      }
      const syncedAt = new Date().toISOString();
      setState((current) => ({
        ...current,
        social: {
          ...current.social,
          pendingFeedPosts: current.social.pendingFeedPosts.filter((post) => !result.sentFeedPostIds.includes(post.id)),
          pendingFeedPostDeletions: current.social.pendingFeedPostDeletions.filter((id) => !deletedPostIds.includes(id)),
          lastSyncedAt: syncedAt,
          lastSyncError: null,
          nextAutoSyncAt: getNextAutoSyncAt(),
        },
      }));
      void (async () => {
        try {
          const status = await getFriendStatus(syncState.social);
          setState((current) => ({
            ...current,
            social: {
              ...current.social,
              ...status.social,
              pendingFeedPosts: current.social.pendingFeedPosts,
            },
          }));
        } catch (refreshError: unknown) {
          console.warn("Social status refresh after sync failed.", refreshError);
        }
      })();
      setHasUnreadSocial(true);
      if (!silent) setMessage("Social data synced.");
    } catch (error: unknown) {
      console.warn("Social sync failed.", error);
      setState((current) => ({
        ...current,
        social: {
          ...current.social,
          lastSyncError: getErrorMessage(error, "Could not sync social data."),
          nextAutoSyncAt: getNextAutoSyncAt(),
        },
      }));
      if (!silent) setMessage(getErrorMessage(error, "Could not sync social data."));
    } finally {
      socialSyncInProgressRef.current = false;
      setSocialSyncing(false);
    }
  }

  function startEditingSocialName() {
    setSocialNameDraft(state.social.displayName);
    setSocialNameEditing(true);
  }

  function cancelEditingSocialName() {
    setSocialNameDraft(state.social.displayName);
    setSocialNameEditing(false);
  }

  async function saveSocialName(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const displayName = socialNameDraft.trim().slice(0, 48);
    if (!displayName) {
      setMessage("Give your player a name first.");
      return;
    }

    const nextState = {
      ...state,
      social: {
        ...state.social,
        displayName,
      },
    };
    setState(nextState);
    setSocialNameEditing(false);
    setMessage("Player name saved.");
    if (socialConfigured) await runSocialSync({ silent: true, stateOverride: nextState });
  }

  function openProfileAvatarEditor() {
    setProfileAvatarDraft(state.social.avatar);
    setProfileAvatarLetterPickerOpen(false);
    setProfileAvatarEditorOpen(true);
  }

  function closeProfileAvatarEditor() {
    setProfileAvatarDraft(state.social.avatar);
    setProfileAvatarLetterPickerOpen(false);
    setProfileAvatarEditorOpen(false);
  }

  function applySelfAvatarToCachedSocial(appState: AppState, avatar: SocialAvatar): AppState {
    return {
      ...appState,
      social: {
        ...appState.social,
        avatar,
      },
    };
  }

  async function saveProfileAvatar() {
    if (profileAvatarDraft.kind === "photo" && !profileAvatarDraft.url) {
      setMessage("Choose a photo before saving.");
      return;
    }
    let avatar = profileAvatarDraft;
    if (avatar.kind === "photo" && socialConfigured && avatar.url.startsWith("data:image/")) {
      try {
        const image = await dataUrlToBlob(avatar.url);
        const result = await uploadProfileAvatar(state.social, image, avatar.name || "avatar.webp");
        avatar = result.avatar;
      } catch (error: unknown) {
        setMessage(getErrorMessage(error, "Could not upload profile photo."));
        return;
      }
    }
    const nextState = applySelfAvatarToCachedSocial(state, avatar);
    setState(nextState);
    setProfileAvatarEditorOpen(false);
    setProfileAvatarLetterPickerOpen(false);
    setMessage(socialConfigured ? "Profile avatar saved and synced." : "Profile avatar saved locally.");
    if (socialConfigured) await runSocialSync({ silent: true, stateOverride: nextState });
  }

  async function handleProfileAvatarPhotoChange(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    try {
      if (!AVATAR_IMAGE_TYPES.has(file.type)) throw new Error("Use PNG, JPEG, or WebP photos.");
      if (file.size > AVATAR_SOURCE_IMAGE_MAX_BYTES) throw new Error("Photo is too large. Use an image under 1 MB.");
      const url = await blobToDataUrl(file);
      const element = await loadImageElement(url);
      setAvatarCropSource({ url, width: element.naturalWidth, height: element.naturalHeight, name: file.name });
      setAvatarCrop({ x: 0.5, y: 0.5, zoom: 1 });
      avatarCropDragRef.current = null;
      setAvatarCropOpen(true);
    } catch (error: unknown) {
      setMessage(getErrorMessage(error, "Could not prepare photo."));
    }
  }

  function cancelAvatarCrop() {
    avatarCropDragRef.current = null;
    setAvatarCropDragging(false);
    setAvatarCropSource(null);
    setAvatarCropOpen(false);
  }

  async function confirmAvatarCrop() {
    if (!avatarCropSource) return;
    try {
      const url = await cropAvatarToDataUrl(avatarCropSource, avatarCrop);
      setProfileAvatarDraft({ kind: "photo", name: avatarCropSource.name, url, mimeType: "image/webp" });
      avatarCropDragRef.current = null;
      setAvatarCropDragging(false);
      setAvatarCropSource(null);
      setAvatarCropOpen(false);
    } catch (error: unknown) {
      setMessage(getErrorMessage(error, "Could not crop photo."));
    }
  }

  function handleAvatarCropPointerDown(event: React.PointerEvent<HTMLDivElement>) {
    if (!avatarCropSource) return;
    event.preventDefault();
    avatarCropDragRef.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      cropX: avatarCrop.x,
      cropY: avatarCrop.y,
    };
    setAvatarCropDragging(true);
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function handleAvatarCropPointerMove(event: React.PointerEvent<HTMLDivElement>) {
    const drag = avatarCropDragRef.current;
    if (!avatarCropSource || !drag || drag.pointerId !== event.pointerId) return;
    const { width, height } = avatarCropSource;
    const scriptScale = Math.max(AVATAR_CROP_VIEWPORT_PX / width, AVATAR_CROP_VIEWPORT_PX / height) * avatarCrop.zoom;
    const dispW = width * scriptScale;
    const dispH = height * scriptScale;
    const dx = event.clientX - drag.startX;
    const dy = event.clientY - drag.startY;
    const next = clampAvatarCrop({
      x: drag.cropX - dx / dispW,
      y: drag.cropY - dy / dispH,
      zoom: avatarCrop.zoom,
    }, avatarCropSource);
    setAvatarCrop(next);
  }

  function handleAvatarCropPointerEnd(event: React.PointerEvent<HTMLDivElement>) {
    if (avatarCropDragRef.current?.pointerId === event.pointerId) {
      avatarCropDragRef.current = null;
      setAvatarCropDragging(false);
    }
  }

  function handleAvatarCropWheel(event: React.WheelEvent<HTMLDivElement>) {
    if (!avatarCropSource) return;
    const factor = event.deltaY < 0 ? 1.08 : 1 / 1.08;
    setAvatarCrop((current) => clampAvatarCrop({
      ...current,
      zoom: Math.min(AVATAR_CROP_MAX_ZOOM, Math.max(1, current.zoom * factor)),
    }, avatarCropSource));
  }

  function handleAvatarCropZoomChange(value: number) {
    if (!avatarCropSource) return;
    setAvatarCrop((current) => clampAvatarCrop({
      ...current,
      zoom: Math.min(AVATAR_CROP_MAX_ZOOM, Math.max(1, value)),
    }, avatarCropSource));
  }

  function applyFeedPostImage(postId: string, image: Pick<SocialFeedPost, "imageUrl" | "imageMimeType" | "imageExpiresAt" | "imageExpiredAt">) {
    const updatePost = (post: SocialFeedPost) => post.id === postId ? { ...post, ...image } : post;
    setState((current) => ({
      ...current,
      social: {
        ...current.social,
        cachedFeeds: {
          global: current.social.cachedFeeds.global.map(updatePost),
          friends: current.social.cachedFeeds.friends.map(updatePost),
        },
        pendingFeedPosts: current.social.pendingFeedPosts.map(updatePost),
      },
    }));
    setFailedFeedImages((current) => {
      if (!current.has(postId)) return current;
      const next = new Set(current);
      next.delete(postId);
      return next;
    });
  }

  async function uploadImageForFeedPost(postId: string, image: PreparedFeedImage) {
    const result = await uploadFeedPostImage(state.social, postId, image.blob);
    if (canViewR2Usage && result.r2Usage) setR2UsageStatus(result.r2Usage);
    applyFeedPostImage(postId, {
      imageUrl: result.imageUrl,
      imageMimeType: result.imageMimeType,
      imageExpiresAt: result.imageExpiresAt,
      imageExpiredAt: result.imageExpiredAt,
    });
  }

  async function handleFeedImageDraftChange(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    if (canViewR2Usage && r2UsageStatus?.paused) {
      setMessage("Image uploads are paused to stay below the free R2 limits.");
      return;
    }
    try {
      const image = await prepareFeedImage(file);
      setFeedImageDraft((current) => {
        if (current) URL.revokeObjectURL(current.previewUrl);
        return image;
      });
    } catch (error: unknown) {
      setMessage(getErrorMessage(error, "Could not prepare image."));
    }
  }

  function updateFeedPollOption(index: number, value: string) {
    setFeedPollDraft((current) => ({
      ...current,
      options: current.options.map((option, optionIndex) => optionIndex === index ? value : option),
    }));
  }

  function addFeedPollOption() {
    setFeedPollDraft((current) => current.options.length >= MAX_FEED_POLL_OPTIONS ? current : { ...current, options: [...current.options, ""] });
  }

  function removeFeedPollOption(index: number) {
    setFeedPollDraft((current) => current.options.length <= 2 ? current : { ...current, options: current.options.filter((_, optionIndex) => optionIndex !== index) });
  }

  function clearFeedPollDraft() {
    setFeedPollDraft(emptyFeedPollDraft());
    setFeedPollPanelOpen(false);
  }

  async function handleEditingFeedPostImageChange(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    if (canViewR2Usage && r2UsageStatus?.paused) {
      setMessage("Image uploads are paused to stay below the free R2 limits.");
      return;
    }
    try {
      const image = await prepareFeedImage(file);
      setEditingFeedPostImage((current) => {
        if (current) URL.revokeObjectURL(current.previewUrl);
        return image;
      });
      setEditingFeedPostRemoveImage(false);
    } catch (error: unknown) {
      setMessage(getErrorMessage(error, "Could not prepare image."));
    }
  }

  async function postLatestSessionToFeed(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!latestFeedSession) {
      setMessage("Finish a study session before posting to the feed.");
      return;
    }
    if (latestFeedSessionPosted) {
      setMessage("That session is already queued for the feed.");
      return;
    }

    let poll: SocialFeedPost["poll"] = null;
    try {
      poll = prepareFeedPollDraft(feedPollDraft);
    } catch (error: unknown) {
      setMessage(getErrorMessage(error, "Could not prepare poll."));
      return;
    }

    let nextState: AppState | null = null;
    setState((current) => {
      nextState = queueFeedPost(current, latestFeedSession, getSessionCourseName(current, latestFeedSession), feedNoteDraft, poll);
      return nextState;
    });
    const image = feedImageDraft;
    setFeedNoteDraft("");
    setFeedImageDraft(null);
    setFeedPollDraft(emptyFeedPollDraft());
    setFeedPollPanelOpen(false);
    if (!image) {
      setMessage(socialConfigured ? "Post queued. Sync to publish it to the feed." : "Post queued locally. Configure sync to publish it.");
      return;
    }
    if (!socialConfigured || !nextState) {
      setMessage("Post queued locally. Configure sync before adding images.");
      return;
    }
    setMessage("Publishing post image...");
    await runSocialSync({ silent: true, stateOverride: nextState });
    try {
      await uploadImageForFeedPost(latestFeedSession.id, image);
      setMessage("Post published with image.");
    } catch (error: unknown) {
      console.warn("Could not upload feed image.", error);
      setMessage(getErrorMessage(error, "Post queued, but image upload failed."));
    } finally {
      URL.revokeObjectURL(image.previewUrl);
    }
  }

  function toggleProfilePrivacy() {
    const nextState = {
      ...state,
      social: {
        ...state.social,
        isPrivate: !state.social.isPrivate,
      },
    };
    setState(nextState);
    setMessage(nextState.social.isPrivate ? "Profile set to private." : "Profile set to public.");
    if (socialConfigured) void runSocialSync({ silent: true, stateOverride: nextState });
  }

  function toggleAutoPostSessions() {
    setState((current) => ({
      ...current,
      social: {
        ...current.social,
        autoPostSessions: !current.social.autoPostSessions,
      },
    }));
    setMessage(state.social.autoPostSessions ? "Auto-post disabled." : "Auto-post enabled.");
  }

  function toggleShowHoursToFriends() {
    const nextState = {
      ...state,
      social: {
        ...state.social,
        showHoursToFriends: !state.social.showHoursToFriends,
      },
    };
    setState(nextState);
    setMessage(nextState.social.showHoursToFriends ? "Friends can see your study hours." : "Your study hours are hidden from friends.");
    if (socialConfigured) void runSocialSync({ silent: true, stateOverride: nextState });
  }

  async function toggleLocalFeedReaction(postId: string, emoji: string) {
    setState((current) => {
      const myName = current.social.displayName;
      const updatePost = (post: SocialFeedPost) => {
        if (post.id !== postId) return post;
        const reacted = Boolean(post.reacted?.[emoji]);
        const prevNames = post.reactedBy?.[emoji] ?? [];
        const ownNameIndex = prevNames.indexOf(myName);
        const nextNames = reacted && ownNameIndex >= 0
          ? [...prevNames.slice(0, ownNameIndex), ...prevNames.slice(ownNameIndex + 1)]
          : reacted
            ? prevNames
            : [...prevNames, myName];
        return {
          ...post,
          reactions: {
            ...post.reactions,
            [emoji]: Math.max(0, (post.reactions?.[emoji] ?? 0) + (reacted ? -1 : 1)),
          },
          reacted: {
            ...post.reacted,
            [emoji]: !reacted,
          },
          reactedBy: {
            ...post.reactedBy,
            [emoji]: nextNames,
          },
        };
      };
      return {
        ...current,
        social: {
          ...current.social,
          cachedFeeds: {
            global: current.social.cachedFeeds.global.map(updatePost),
            friends: current.social.cachedFeeds.friends.map(updatePost),
          },
          pendingFeedPosts: current.social.pendingFeedPosts.map(updatePost),
        },
      };
    });
    if (!socialConfigured || state.social.pendingFeedPosts.some((post) => post.id === postId)) return;
    try {
      await reactToFeedPost(state.social, postId, emoji);
    } catch (error: unknown) {
      console.warn("Could not sync feed reaction.", error);
      setMessage(getErrorMessage(error, "Could not sync reaction."));
    }
  }

  async function voteFeedPoll(post: SocialFeedPost, optionId: string) {
    if (!post.poll) return;
    if (state.social.pendingFeedPosts.some((item) => item.id === post.id)) {
      setMessage("Sync this post before voting on its poll.");
      return;
    }
    if (!socialConfigured) {
      setMessage("Social sync is required for poll voting.");
      return;
    }

    const applyPoll = (poll: SocialFeedPost["poll"]) => {
      const updatePost = (item: SocialFeedPost) => item.id === post.id ? { ...item, poll } : item;
      setState((current) => ({
        ...current,
        social: {
          ...current.social,
          cachedFeeds: {
            global: current.social.cachedFeeds.global.map(updatePost),
            friends: current.social.cachedFeeds.friends.map(updatePost),
          },
          pendingFeedPosts: current.social.pendingFeedPosts.map(updatePost),
        },
      }));
    };

    const wasSelected = post.poll.options.find((option) => option.id === optionId)?.selected ?? false;
    const optimisticOptions = post.poll.options.map((option) => {
      if (post.poll?.multiple) {
        if (option.id !== optionId) return option;
        return { ...option, selected: !wasSelected, votes: Math.max(0, option.votes + (wasSelected ? -1 : 1)) };
      }
      if (option.id === optionId) return { ...option, selected: !wasSelected, votes: Math.max(0, option.votes + (wasSelected ? -1 : 1)) };
      if (option.selected && !wasSelected) return { ...option, selected: false, votes: Math.max(0, option.votes - 1) };
      return option;
    });
    applyPoll({ ...post.poll, options: optimisticOptions, totalVotes: optimisticOptions.reduce((sum, option) => sum + option.votes, 0) });

    try {
      const result = await voteOnFeedPoll(state.social, post.id, optionId);
      applyPoll(result.poll);
    } catch (error: unknown) {
      console.warn("Could not sync poll vote.", error);
      applyPoll(post.poll);
      setMessage(getErrorMessage(error, "Could not sync poll vote."));
    }
  }

  function toggleFeedComments(postId: string) {
    setExpandedFeedComments((current) => {
      const next = new Set(current);
      if (next.has(postId)) next.delete(postId);
      else next.add(postId);
      return next;
    });
  }

  async function submitFeedComment(event: FormEvent<HTMLFormElement>, post: SocialFeedPost) {
    event.preventDefault();
    const body = (feedCommentDrafts[post.id] ?? "").trim();
    if (!body) {
      setMessage("Write a comment first.");
      return;
    }
    if (state.social.pendingFeedPosts.some((item) => item.id === post.id)) {
      setMessage("Sync this post before adding comments.");
      return;
    }
    if (!socialConfigured) {
      setMessage("Social sync is required for comments.");
      return;
    }

    setFeedCommentSavingId(post.id);
    try {
      const result = await commentOnFeedPost(state.social, post.id, body);
      const addComment = (item: SocialFeedPost) => {
        if (item.id !== post.id) return item;
        const comments = item.comments ?? [];
        if (comments.some((comment) => comment.id === result.comment.id)) return item;
        return { ...item, comments: [...comments, result.comment] };
      };
      setState((current) => ({
        ...current,
        social: {
          ...current.social,
          cachedFeeds: {
            global: current.social.cachedFeeds.global.map(addComment),
            friends: current.social.cachedFeeds.friends.map(addComment),
          },
        },
      }));
      setFeedCommentDrafts((current) => ({ ...current, [post.id]: "" }));
      setExpandedFeedComments((current) => new Set(current).add(post.id));
    } catch (error: unknown) {
      console.warn("Could not sync feed comment.", error);
      setMessage(getErrorMessage(error, "Could not post comment."));
    } finally {
      setFeedCommentSavingId(null);
    }
  }

  async function sendFriendRequestToCode(friendCodeValue: string) {
    const friendCode = friendCodeValue.trim().toUpperCase();
    if (!friendCode) {
      setMessage("Enter a friend code first.");
      return false;
    }
    if (friendCode === state.social.friendCode) {
      setMessage("That is your own friend code.");
      return false;
    }
    if (state.social.friends.some((friend) => friend.friendCode === friendCode)) {
      setMessage("You are already friends.");
      return false;
    }
    if (state.social.outgoingFriendRequests.some((request) => request.toFriendCode === friendCode)) {
      setMessage("Friend request already pending.");
      return false;
    }

    setSocialSyncing(true);
    try {
      await createFriendRequest(state.social, friendCode);
      await runSocialSync({ silent: true });
      setMessage("Friend request sent.");
      return true;
    } catch (error: unknown) {
      console.warn("Could not send friend request.", error);
      setMessage(getErrorMessage(error, "Could not send friend request."));
      return false;
    } finally {
      setSocialSyncing(false);
    }
  }

  async function submitFriendRequest(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const sent = await sendFriendRequestToCode(friendCodeDraft);
    if (sent) setFriendCodeDraft("");
  }

  function startEditingFeedPost(post: SocialFeedPost) {
    setEditingFeedPostId(post.id);
    setEditingFeedPostNote(post.note ?? "");
    setEditingFeedPostImage((current) => {
      if (current) URL.revokeObjectURL(current.previewUrl);
      return null;
    });
    setEditingFeedPostRemoveImage(false);
  }

  function cancelEditingFeedPost() {
    setEditingFeedPostId(null);
    setEditingFeedPostNote("");
    setEditingFeedPostImage((current) => {
      if (current) URL.revokeObjectURL(current.previewUrl);
      return null;
    });
    setEditingFeedPostRemoveImage(false);
  }

  async function saveFeedPostEdit(postId: string) {
    const note = editingFeedPostNote.trim();
    setFeedPostSaving(true);
    const updatePost = (post: SocialFeedPost) => (post.id === postId ? { ...post, note } : post);
    try {
      if (socialConfigured && !state.social.pendingFeedPosts.some((post) => post.id === postId)) {
        await updateFeedPost(state.social, postId, note);
        if (editingFeedPostImage) {
          await uploadImageForFeedPost(postId, editingFeedPostImage);
        } else if (editingFeedPostRemoveImage) {
          const result = await deleteFeedPostImage(state.social, postId);
          if (canViewR2Usage && result.r2Usage) setR2UsageStatus(result.r2Usage);
        }
      }
      setState((current) => ({
        ...current,
        social: {
          ...current.social,
          cachedFeeds: {
            global: current.social.cachedFeeds.global.map(updatePost).map((post) => post.id === postId && editingFeedPostRemoveImage ? { ...post, imageUrl: null, imageMimeType: null, imageExpiresAt: null, imageExpiredAt: null } : post),
            friends: current.social.cachedFeeds.friends.map(updatePost).map((post) => post.id === postId && editingFeedPostRemoveImage ? { ...post, imageUrl: null, imageMimeType: null, imageExpiresAt: null, imageExpiredAt: null } : post),
          },
          pendingFeedPosts: current.social.pendingFeedPosts.map(updatePost).map((post) => post.id === postId && editingFeedPostRemoveImage ? { ...post, imageUrl: null, imageMimeType: null, imageExpiresAt: null, imageExpiredAt: null } : post),
        },
      }));
      cancelEditingFeedPost();
      setMessage("Post updated.");
    } catch (error: unknown) {
      console.warn("Could not update feed post.", error);
      setMessage(getErrorMessage(error, "Could not update feed post."));
    } finally {
      setFeedPostSaving(false);
    }
  }

  async function deleteOwnFeedPost(postId: string) {
    setFeedPostSaving(true);
    try {
      if (socialConfigured && !state.social.pendingFeedPosts.some((post) => post.id === postId)) {
        await deleteFeedPost(state.social, postId);
      }
      setState((current) => ({
        ...current,
        social: {
          ...current.social,
          cachedFeeds: {
            global: current.social.cachedFeeds.global.filter((post) => post.id !== postId),
            friends: current.social.cachedFeeds.friends.filter((post) => post.id !== postId),
          },
          pendingFeedPosts: current.social.pendingFeedPosts.filter((post) => post.id !== postId),
        },
      }));
      cancelEditingFeedPost();
      setMessage("Post deleted. You can repost that session now.");
    } catch (error: unknown) {
      console.warn("Could not delete feed post.", error);
      setMessage(getErrorMessage(error, "Could not delete feed post."));
    } finally {
      setFeedPostSaving(false);
    }
  }

  async function answerFriendRequest(requestId: string, response: "accepted" | "declined") {
    setSocialSyncing(true);
    try {
      const result = await respondToFriendRequest(state.social, requestId, response);
      setState((current) => ({
        ...current,
        social: {
          ...current.social,
          ...result.social,
          pendingFeedPosts: current.social.pendingFeedPosts,
          lastSyncedAt: new Date().toISOString(),
          lastSyncError: null,
          nextAutoSyncAt: getNextAutoSyncAt(),
        },
      }));
      setMessage(response === "accepted" ? "Friend request accepted." : "Friend request declined.");
    } catch (error: unknown) {
      console.warn("Could not respond to friend request.", error);
      setMessage(getErrorMessage(error, "Could not update friend request."));
    } finally {
      setSocialSyncing(false);
    }
  }

  function applySocialSnapshot(result: { social: Partial<AppState["social"]> }) {
    setState((current) => ({
      ...current,
      social: {
        ...current.social,
        ...result.social,
        pendingFeedPosts: current.social.pendingFeedPosts,
        lastSyncedAt: new Date().toISOString(),
        lastSyncError: null,
        nextAutoSyncAt: getNextAutoSyncAt(),
      },
    }));
  }

  async function submitSquadCreate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = squadNameDraft.trim();
    if (!name) {
      setMessage("Name your squad first.");
      return;
    }
    setSocialSyncing(true);
    try {
      const result = await createSquad(state.social, name, squadPrivateDraft);
      applySocialSnapshot(result);
      setSquadNameDraft("");
      setMessage("Squad created.");
    } catch (error: unknown) {
      console.warn("Could not create squad.", error);
      setMessage(getErrorMessage(error, "Could not create squad."));
    } finally {
      setSocialSyncing(false);
    }
  }

  async function submitSquadSearch(event?: FormEvent<HTMLFormElement>) {
    event?.preventDefault();
    if (!socialConfigured) {
      setMessage("Social sync is required for squads.");
      return;
    }
    setSquadSearching(true);
    try {
      const result = await searchSquads(state.social, squadSearchDraft.trim());
      setSquadSearchResults(result.squads);
    } catch (error: unknown) {
      console.warn("Could not search squads.", error);
      setMessage(getErrorMessage(error, "Could not search squads."));
    } finally {
      setSquadSearching(false);
    }
  }

  async function loadSquadSuggestions(options: { forceFetch?: boolean } = {}) {
    if (!socialConfigured || currentSquad) return;
    if (!options.forceFetch && squadSuggestionPool.length) {
      setSquadSuggestions(pickSquadSuggestions(squadSuggestionPool));
      return;
    }
    setSquadSuggestionsLoading(true);
    try {
      const result = await searchSquads(state.social, "");
      setSquadSuggestionPool(result.squads);
      setSquadSuggestions(pickSquadSuggestions(result.squads));
    } catch (error: unknown) {
      console.warn("Could not load squad suggestions.", error);
      setMessage(getErrorMessage(error, "Could not load squad suggestions."));
    } finally {
      setSquadSuggestionsLoading(false);
    }
  }

  const loadInitialSquadSuggestions = useEffectEvent(() => {
    void loadSquadSuggestions({ forceFetch: true });
  });

  async function joinOrRequestSquad(squad: SquadSearchResult) {
    setSocialSyncing(true);
    try {
      const result = await joinSquad(state.social, squad.id);
      applySocialSnapshot(result);
      if (squad.isPrivate) {
        const markPending = (item: SquadSearchResult) => item.id === squad.id ? { ...item, action: "pending" as const } : item;
        setSquadSuggestions((current) => current.map(markPending));
        setSquadSuggestionPool((current) => current.map(markPending));
        setSquadSearchResults((current) => current.map(markPending));
      }
      await submitSquadSearch();
      setMessage(squad.isPrivate ? "Join request sent." : "Joined squad.");
    } catch (error: unknown) {
      console.warn("Could not join squad.", error);
      setMessage(getErrorMessage(error, "Could not join squad."));
    } finally {
      setSocialSyncing(false);
    }
  }

  async function answerSquadRequest(requestId: string, response: "accepted" | "declined") {
    setSocialSyncing(true);
    try {
      const result = await respondToSquadRequest(state.social, requestId, response);
      applySocialSnapshot(result);
      setMessage(response === "accepted" ? "Squad request accepted." : "Squad request declined.");
    } catch (error: unknown) {
      console.warn("Could not update squad request.", error);
      setMessage(getErrorMessage(error, "Could not update squad request."));
    } finally {
      setSocialSyncing(false);
    }
  }

  async function leaveCurrentSquad() {
    if (isLastSquadMember && !window.confirm("You are the last member. Leaving will delete this squad. Continue?")) return;
    setSocialSyncing(true);
    try {
      const result = await leaveSquad(state.social);
      applySocialSnapshot(result);
      setMessage(isLastSquadMember ? "Squad deleted." : "You left the squad.");
    } catch (error: unknown) {
      console.warn("Could not leave squad.", error);
      setMessage(getErrorMessage(error, "Could not leave squad."));
    } finally {
      setSocialSyncing(false);
    }
  }

  async function submitSquadChat(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const body = squadChatDraft.trim();
    if (!body) return;
    setSquadChatDraft("");
    try {
      const result = await sendSquadMessage(state.social, body);
      applySocialSnapshot(result);
    } catch (error: unknown) {
      console.warn("Could not send squad message.", error);
      setSquadChatDraft(body);
      setMessage(getErrorMessage(error, "Could not send message."));
    }
  }

  async function deleteOwnSquadMessage(messageId: string) {
    if (!window.confirm("Delete this message?")) return;
    try {
      const result = await deleteSquadMessage(state.social, messageId);
      applySocialSnapshot(result);
      setMessage("Message deleted.");
    } catch (error: unknown) {
      console.warn("Could not delete squad message.", error);
      setMessage(getErrorMessage(error, "Could not delete message."));
    }
  }

  async function changeSquadMemberRole(targetUserId: string, role: SocialSquadRole) {
    setSocialSyncing(true);
    try {
      const result = await setSquadMemberRole(state.social, targetUserId, role);
      applySocialSnapshot(result);
      setExpandedSquadMemberId(null);
      setMessage("Squad rank updated.");
    } catch (error: unknown) {
      console.warn("Could not update squad rank.", error);
      setMessage(getErrorMessage(error, "Could not update squad rank."));
    } finally {
      setSocialSyncing(false);
    }
  }

  async function kickFromSquad(targetUserId: string, displayName: string) {
    if (!window.confirm(`Kick ${displayName} from the squad?`)) return;
    setSocialSyncing(true);
    try {
      const result = await kickSquadMember(state.social, targetUserId);
      applySocialSnapshot(result);
      setExpandedSquadMemberId(null);
      setMessage("Member kicked.");
    } catch (error: unknown) {
      console.warn("Could not kick squad member.", error);
      setMessage(getErrorMessage(error, "Could not kick member."));
    } finally {
      setSocialSyncing(false);
    }
  }

  function startSquadSettingsEdit() {
    if (!currentSquad) return;
    setSquadSettingsNameDraft(currentSquad.name);
    setSquadSettingsPrivateDraft(currentSquad.isPrivate);
    setSquadSettingsEditing(true);
  }

  async function submitSquadSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = squadSettingsNameDraft.trim();
    if (!name) {
      setMessage("Name your squad first.");
      return;
    }
    setSocialSyncing(true);
    try {
      const result = await updateSquadSettings(state.social, name, squadSettingsPrivateDraft);
      applySocialSnapshot(result);
      setSquadSettingsEditing(false);
      setMessage("Squad settings saved.");
    } catch (error: unknown) {
      console.warn("Could not update squad settings.", error);
      setMessage(getErrorMessage(error, "Could not update squad settings."));
    } finally {
      setSocialSyncing(false);
    }
  }

  function openMenuPanel(panel: Exclude<MenuPanel, null>) {
    setActiveMenuPanel(panel);
    setMenuOpen(false);
    setDeleteConfirmOpen(false);
    if (panel === "personal") {
      setPersonalNameDraft(state.settings.userName);
      setPersonalDailyGoalHoursDraft(String((state.settings.dailyGoalMinutes ?? 120) / 60));
    }
  }

  function closeMenuPanel() {
    setActiveMenuPanel(null);
    setDeleteConfirmOpen(false);
  }

  function openUpdateSettingsFromNotice() {
    setUpdateNoticeVisible(false);
    openMenuPanel("settings");
  }

  function dismissAppAnnouncement(announcementId: string) {
    const dismissedIds = loadDismissedAnnouncementIds();
    dismissedIds.add(announcementId);
    saveDismissedAnnouncementIds(dismissedIds);
    setAppAnnouncement(null);
  }

  async function sendUpdateNoticeToUsers() {
    const targetVersion = currentAppVersion.trim();
    if (!isValidAppVersion(targetVersion)) {
      setMessage("Wait until the current app version is loaded before notifying users.");
      return;
    }
    if (!window.confirm(`Notify users below version ${targetVersion} about the update?`)) return;
    setUpdateNoticeSending(true);
    try {
      const result = await notifyUsersAboutUpdate(state.social, targetVersion);
      setAppAnnouncement(null);
      setMessage(`Update notice sent to users below ${result.targetVersion}.`);
    } catch (error: unknown) {
      console.warn("Could not send update notice.", error);
      setMessage(getErrorMessage(error, "Could not notify users about the update."));
    } finally {
      setUpdateNoticeSending(false);
    }
  }

  async function loadAdminUsage() {
    setAdminUsageLoading(true);
    try {
      setAdminUsage(await getAdminUsage(state.social));
      setAdminUsageOpen(true);
    } catch (error: unknown) {
      console.warn("Could not load usage data.", error);
      setMessage(getErrorMessage(error, "Could not load usage data."));
    } finally {
      setAdminUsageLoading(false);
    }
  }

  function shortAdminId(value?: string | null) {
    return value ? value.slice(0, 8) : "unknown";
  }

  function formatAdminTimestamp(value?: string | null) {
    if (!value) return "Never";
    const parsed = new Date(value.includes("T") ? value : `${value.replace(" ", "T")}Z`);
    if (Number.isNaN(parsed.getTime())) return value;
    return `${formatDate(parsed.toISOString())} ${parsed.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
  }

  function formatLatestDevice(user: AdminUsageResponse["users"][number]) {
    const label = user.deviceLabel || user.appPlatform || "unknown device";
    return `${label} · device ${shortAdminId(user.deviceFingerprintHash)}`;
  }

  function savePersonalSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const userName = personalNameDraft.trim();
    const dailyGoalHours = Number(personalDailyGoalHoursDraft);
    const dailyGoalMinutes = clamp(Math.round((Number.isFinite(dailyGoalHours) ? dailyGoalHours : 2) * 60), 15, 1440);
    setState((current) => ({ ...current, settings: { ...current.settings, userName, dailyGoalMinutes } }));
    setMessage("Personal settings saved.");
    closeMenuPanel();
  }

  async function checkForUpdates(options: CheckForUpdatesOptions = {}) {
    const { silent = false, automatic = false } = options;

    if (!isTauriApp()) {
      if (!silent) {
        setUpdateInfo({
          status: "idle",
          releaseUrl: RELEASES_PAGE_URL,
          message: "Automatic updates are only available in the installed desktop app.",
        });
      }
      return;
    }

    if (updateInstallSupport.packageHint === "development") {
      pendingUpdateRef.current = null;
      if (!silent) {
        setUpdateInfo({
          status: "idle",
          releaseUrl: "https://github.com/damcha02/destudydracker",
          message: updateInstallSupport.message,
        });
      }
      return;
    }

    if (updateInstallSupport.packageHint === "source-build") {
      pendingUpdateRef.current = null;
      if (!silent) {
        setUpdateInfo({
          status: "available",
          releaseUrl: "https://github.com/damcha02/destudydracker",
          message: updateInstallSupport.message,
        });
      }
      return;
    }

    if (!silent) setUpdateChecking(true);
    pendingUpdateRef.current = null;
    if (!silent) setUpdateInfo({ status: "idle", releaseUrl: RELEASES_PAGE_URL, message: "Checking for updates..." });

    try {
      const update = await check({ timeout: 30000 });

      if (update) {
        pendingUpdateRef.current = update;
        if (automatic) setUpdateNoticeVisible(true);
        setUpdateInfo({
          status: "available",
          latestVersion: update.version,
          releaseUrl: RELEASES_PAGE_URL,
          message: updateInstallSupport.canAutoInstall ? `Version ${update.version} is available and can be installed automatically.` : `Version ${update.version} is available. ${updateInstallSupport.message}`,
        });
      } else if (!silent) {
        setUpdateInfo({
          status: "current",
          releaseUrl: RELEASES_PAGE_URL,
          message: "You are up to date.",
        });
      }
    } catch (error) {
      if (silent) {
        console.warn("Automatic update check failed.", error);
      } else {
        setUpdateInfo({
          status: "error",
          releaseUrl: RELEASES_PAGE_URL,
          message: `${getErrorMessage(error, "Could not check for updates. Check your internet connection.")} You can still download installers from the release page.`,
        });
      }
    } finally {
      if (!silent) setUpdateChecking(false);
    }
  }

  const runAutomaticUpdateCheck = useEffectEvent(() => {
    void checkForUpdates({ silent: true, automatic: true });
  });

  const refreshAppAnnouncement = useEffectEvent(async () => {
    if (!isSocialApiConfigured()) return;
    try {
      const result = await getCurrentAnnouncement(getAppMetadata());
      const announcement = result.announcement;
      if (!announcement) {
        setAppAnnouncement(null);
        return;
      }

      setAppAnnouncement(loadDismissedAnnouncementIds().has(announcement.id) ? null : announcement);
    } catch (error: unknown) {
      console.warn("Could not refresh app announcement.", error);
    }
  });

  const sendOptInTelemetryHeartbeat = useEffectEvent(async () => {
    if (!state.settings.telemetryEnabled || !isSocialApiConfigured()) return;
    try {
      await sendTelemetryHeartbeat(getTelemetryInstallId(), getAppMetadata());
    } catch (error: unknown) {
      console.warn("Could not send telemetry heartbeat.", error);
    }
  });

  const runAutomaticSocialSync = useEffectEvent(() => {
    if (shouldAutoSyncSocial(state.social)) void runSocialSync({ silent: true });
  });

  const syncAfterSessionChange = useEffectEvent(() => {
    if (socialConfigured) void runSocialSync({ silent: true });
  });

  const presencePingEffect = useEffectEvent(() => {
    if (socialConfigured) void presencePing(state.social, getAppMetadata()).catch(() => {});
  });

  async function refreshFriendStatusNow() {
    if (!socialConfigured || state.activeTab !== "friends") return;
    try {
      const result = await getFriendStatus(state.social);
      setState((current) => ({
        ...current,
        social: {
          ...current.social,
          friends: result.social.friends,
          incomingFriendRequests: result.social.incomingFriendRequests,
          outgoingFriendRequests: result.social.outgoingFriendRequests,
          squad: result.social.squad,
          incomingSquadRequests: result.social.incomingSquadRequests,
          outgoingSquadRequests: result.social.outgoingSquadRequests,
          squadMessages: result.social.squadMessages,
          pendingFeedPosts: current.social.pendingFeedPosts,
          lastSyncError: null,
        },
      }));
    } catch (error: unknown) {
      console.warn("Could not refresh friend status.", error);
    }
  }

  const refreshFriendStatus = useEffectEvent(async () => {
    await refreshFriendStatusNow();
  });

  const refreshSocialLeaderboard = useEffectEvent(async () => {
    if (!socialConfigured || state.activeTab !== "friends" || socialSubtab !== "leaderboard") return;
    try {
      const result = await getSocialLeaderboard(state.social, socialScope, socialPeriod);
      setState((current) => ({
        ...current,
        social: {
          ...current.social,
          cachedLeaderboards: {
            ...current.social.cachedLeaderboards,
            [socialScope]: {
              ...current.social.cachedLeaderboards[socialScope],
              [socialPeriod]: result.entries,
            },
          },
        },
      }));
    } catch (error: unknown) {
      console.warn("Could not refresh social leaderboard.", error);
    }
  });

  const refreshSquadScoreboard = useEffectEvent(async () => {
    if (!socialConfigured || state.activeTab !== "friends" || socialSubtab !== "leaderboard" || socialScope !== "squad") return;
    try {
      const result = await getSquadScoreboard(state.social, squadScorePeriod);
      if (!result) return;
      setState((current) => ({
        ...current,
        social: {
          ...current.social,
          cachedSquadScoreLeaderboards: {
            ...current.social.cachedSquadScoreLeaderboards,
            [squadScorePeriod]: result.entries,
          },
        },
      }));
    } catch (error: unknown) {
      console.warn("Could not refresh squad scoreboard.", error);
    }
  });

  async function openFriendProfile(friend: SocialProfileTarget) {
    setViewingFriend(friend.userId === state.social.userId ? { ...friend, avatar: state.social.avatar } : friend);
    setViewingFriendStats(null);
    setViewingFriendLoading(true);
    if (friend.userId === state.social.userId) {
      const daily = getLocalLeaderboardEntry(state, "daily");
      const weekly = getLocalLeaderboardEntry(state, "weekly");
      const overall = getLocalLeaderboardEntry(state, "overall");
      setViewingFriendStats({
        displayName: state.social.displayName,
        friendCode: state.social.friendCode,
        avatar: state.social.avatar,
        lastSeenAt: state.social.lastSyncedAt,
        hoursVisible: true,
        daily: { minutes: daily.minutes, sessions: daily.sessions, lastActiveDate: daily.lastActiveDate },
        weekly: { minutes: weekly.minutes, sessions: weekly.sessions, lastActiveDate: weekly.lastActiveDate },
        overall: { minutes: overall.minutes, sessions: overall.sessions, lastActiveDate: overall.lastActiveDate },
      });
      setViewingFriendLoading(false);
      return;
    }
    try {
      const stats = await getPlayerStats(state.social, friend.userId);
      setViewingFriendStats(stats);
      setViewingFriend((current) => current && current.userId === friend.userId ? { ...current, avatar: stats.avatar } : current);
    } catch (error: unknown) {
      console.warn("Could not load friend stats.", error);
      setMessage(getErrorMessage(error, "Could not load profile."));
    } finally {
      setViewingFriendLoading(false);
    }
  }

  async function openSquadDetails(entry: SocialSquadScoreEntry) {
    setViewingSquadEntry(entry);
    setViewingSquadDetails(null);
    setViewingSquadLoading(true);
    try {
      const result = await getSquadDetails(state.social, entry.squadId);
      setViewingSquadDetails(result.squad);
    } catch (error: unknown) {
      console.warn("Could not load squad details.", error);
      setMessage(getErrorMessage(error, "Could not load squad."));
    } finally {
      setViewingSquadLoading(false);
    }
  }

  async function joinOrRequestViewedSquad() {
    if (!viewingSquadDetails || (viewingSquadDetails.action !== "join" && viewingSquadDetails.action !== "request")) return;
    setSocialSyncing(true);
    try {
      const wasPrivate = viewingSquadDetails.isPrivate;
      const result = await joinSquad(state.social, viewingSquadDetails.id);
      applySocialSnapshot(result);
      try {
        const refreshed = await getSquadDetails(state.social, viewingSquadDetails.id);
        setViewingSquadDetails(refreshed.squad);
      } catch (refreshError: unknown) {
        console.warn("Could not refresh squad details.", refreshError);
        setViewingSquadDetails((current) => current ? { ...current, action: wasPrivate ? "pending" : "current" } : current);
      }
      setMessage(wasPrivate ? "Join request sent." : "Joined squad.");
    } catch (error: unknown) {
      console.warn("Could not join squad.", error);
      setMessage(getErrorMessage(error, "Could not join squad."));
    } finally {
      setSocialSyncing(false);
    }
  }

  async function refreshSocialFeedNow() {
    if (!socialConfigured || state.activeTab !== "friends" || socialSubtab !== "feed") return;
    setFeedLoading(true);
    try {
      const result = await getSocialFeed(state.social, feedScope);
      updateFeedCommentNoticeFromFeeds([{ scope: feedScope, feed: result.feed }]);
      if (canViewR2Usage && result.r2Usage) setR2UsageStatus(result.r2Usage);
      else if (!canViewR2Usage) setR2UsageStatus(null);
      setFailedFeedImages((current) => {
        if (!current.size) return current;
        const liveImageIds = new Set(result.feed.filter((post) => post.imageUrl).map((post) => post.id));
        const next = new Set([...current].filter((id) => liveImageIds.has(id)));
        return next.size === current.size ? current : next;
      });
      setState((current) => ({
        ...current,
        social: {
          ...current.social,
          cachedFeeds: {
            ...current.social.cachedFeeds,
            [feedScope]: result.feed,
          },
          lastSyncError: null,
        },
      }));
    } catch (error: unknown) {
      console.warn("Could not refresh social feed.", error);
      setState((current) => ({
        ...current,
        social: {
          ...current.social,
          lastSyncError: getErrorMessage(error, "Could not refresh the feed."),
        },
      }));
    } finally {
      setFeedLoading(false);
    }
  }

  const refreshSocialFeed = useEffectEvent(async () => {
    await refreshSocialFeedNow();
  });

  useEffect(() => {
    if (!isTauriApp()) return undefined;
    if (updateInstallSupport.packageHint === "unknown" && updateInstallSupport.runtimeChannel === "unknown") return undefined;
    if (updateInstallSupport.packageHint === "development" || updateInstallSupport.packageHint === "source-build") return undefined;

    runAutomaticUpdateCheck();
    const interval = window.setInterval(runAutomaticUpdateCheck, AUTO_UPDATE_CHECK_INTERVAL_MS);
    return () => window.clearInterval(interval);
  }, [updateInstallSupport.packageHint, updateInstallSupport.runtimeChannel]);

  useEffect(() => {
    void refreshAppAnnouncement();
    const interval = window.setInterval(() => void refreshAppAnnouncement(), ANNOUNCEMENT_POLL_INTERVAL_MS);
    return () => window.clearInterval(interval);
  }, []);

  useEffect(() => {
    if (!state.settings.telemetryEnabled) return undefined;
    void sendOptInTelemetryHeartbeat();
    const interval = window.setInterval(() => void sendOptInTelemetryHeartbeat(), TELEMETRY_HEARTBEAT_INTERVAL_MS);
    return () => window.clearInterval(interval);
  }, [state.settings.telemetryEnabled]);

  useEffect(() => {
    // Hygiene fix, not a security control: this only stops the app's own accidental
    // browser-preview/dev sessions from silently creating sync accounts. A script hitting
    // the Worker's /sync endpoint directly bypasses the client entirely, so this check
    // does nothing to prevent that — see PRIVACY.md and the abuse-detection changes on
    // the server side for the actual anti-abuse controls.
    if (!isTauriApp()) return undefined;
    presencePingEffect();
    runAutomaticSocialSync();
    const interval = window.setInterval(runAutomaticSocialSync, 60 * 60 * 1000);
    return () => window.clearInterval(interval);
  }, []);

  useEffect(() => {
    if (state.activeTab !== "friends") return undefined;
    void refreshFriendStatus();
    const interval = window.setInterval(() => void refreshFriendStatus(), 2 * 60 * 1000);
    return () => window.clearInterval(interval);
  }, [state.activeTab]);

  useEffect(() => {
    if (!socialConfigured || !state.sessions.length) return;
    const timeout = window.setTimeout(syncAfterSessionChange, 2000);
    return () => window.clearTimeout(timeout);
  }, [socialConfigured, state.sessions.length]);

  useEffect(() => {
    void refreshSocialFeed();
  }, [feedScope, socialSubtab, state.activeTab]);

  useEffect(() => {
    void refreshSocialLeaderboard();
  }, [socialScope, socialPeriod, socialSubtab, state.activeTab]);

  useEffect(() => {
    void refreshSquadScoreboard();
  }, [socialScope, squadScorePeriod, socialSubtab, state.activeTab]);

  useEffect(() => {
    if (!socialConfigured || state.activeTab !== "friends" || socialSubtab !== "feed") return undefined;
    const interval = window.setInterval(() => void refreshSocialFeed(), 2 * 60 * 1000);
    return () => window.clearInterval(interval);
  }, [socialConfigured, socialSubtab, state.activeTab]);

  useEffect(() => {
    if (state.activeTab === "friends" && (socialSubtab === "leaderboard" || socialSubtab === "feed" || socialSubtab === "profile" || socialSubtab === "squad")) {
      presencePingEffect();
      if (socialSubtab === "leaderboard" && socialConfigured) {
        void runSocialSync({ silent: true });
      } else {
        void refreshFriendStatus();
      }
    }
  }, [state.activeTab, socialSubtab]);

  useEffect(() => {
    if (!socialConfigured || state.activeTab !== "friends" || socialSubtab !== "squad" || currentSquad || squadSuggestions.length || squadSuggestionsLoading) return;
    loadInitialSquadSuggestions();
  }, [socialConfigured, state.activeTab, socialSubtab, currentSquad, squadSuggestions.length, squadSuggestionsLoading]);

  const initDurakPuzzle = useEffectEvent(() => {
    const today = isoDate();
    const savedSeed = state.durakPuzzle.seed;
    let solvedCount = state.durakPuzzle.solvedCount || 0;
    if (savedSeed && !savedSeed.startsWith(today)) {
      solvedCount = 0;
      setState((s) => ({ ...s, durakPuzzle: { ...s.durakPuzzle, solvedCount: 0, seed: null, failures: 0 } }));
    }
    if (solvedCount >= 3) return;
    const seedIndex = solvedCount;
    const seed = `${today}_${seedIndex}`;
    if (durakGameState && state.durakPuzzle.seed === seed) return;
    if (state.durakPuzzle.seed === seed) {
      const saved = puzzleToGameState(state.durakPuzzle);
      if (saved) {
        setDurakGameState(saved);
        setDurakSelected([]);
        return;
      }
    }
    const puzzle = findDailyPuzzle(seed);
    if (!puzzle) return;
    setDurakGameState(puzzle.initialState);
    const serialized = gameStateToPuzzle(puzzle.initialState, 0, false, puzzle.hint, seed);
    setState((s) => ({ ...s, durakPuzzle: { ...s.durakPuzzle, ...serialized } }));
  });

  useEffect(() => {
    if (showDurakPuzzle) {
      initDurakPuzzle();
    }
  }, [showDurakPuzzle]);

  const initWordlePuzzle = useEffectEvent(() => {
    const activeDate = localIsoDate();
    const seedSalt = state.wordlePuzzle.seedSalt || makeWordleSeedSalt();
    const puzzleId = getWordlePuzzleId(activeDate, seedSalt);
    if (state.wordlePuzzle.activeDate === activeDate && state.wordlePuzzle.puzzleId === puzzleId && state.wordlePuzzle.answer) return;
    setState((current) => ({
      ...current,
      wordlePuzzle: {
        seedSalt,
        activeDate,
        puzzleId,
        answer: getWordleAnswerForDate(activeDate, seedSalt),
        guesses: [],
        completed: false,
        won: false,
        hardMode: current.wordlePuzzle.hardMode,
      },
    }));
    setWordleDraft("");
    setWordleMessage("");
  });

  useEffect(() => {
    if (showWordlePuzzle) initWordlePuzzle();
  }, [showWordlePuzzle]);

  useEffect(() => {
    if (!showWordlePuzzle) return;
    window.setTimeout(() => wordleModalRef.current?.focus(), 0);
  }, [showWordlePuzzle]);

  const initGeodlePuzzle = useEffectEvent(() => {
    const activeDate = localIsoDate();
    const seedSalt = state.geodlePuzzle.seedSalt || makeGeodleSeedSalt();
    const puzzleId = getGeodlePuzzleId(activeDate, seedSalt);
    if (state.geodlePuzzle.activeDate === activeDate && state.geodlePuzzle.puzzleId === puzzleId && state.geodlePuzzle.answer) return;
    setState((current) => ({
      ...current,
      geodlePuzzle: {
        seedSalt,
        activeDate,
        puzzleId,
        answer: getGeodleAnswerForDate(activeDate, seedSalt),
        guesses: [],
        completed: false,
        won: false,
      },
    }));
    setGeodleDraft("");
    setGeodleMessage("");
    setGeodleDropdownOpen(false);
  });

  useEffect(() => {
    if (showGeodlePuzzle) initGeodlePuzzle();
  }, [showGeodlePuzzle]);

  const initFlagglePuzzle = useEffectEvent(() => {
    const activeDate = localIsoDate();
    const seedSalt = state.flagglePuzzle.seedSalt || makeFlaggleSeedSalt();
    const puzzleId = getFlagglePuzzleId(activeDate, seedSalt);
    if (state.flagglePuzzle.activeDate === activeDate && state.flagglePuzzle.puzzleId === puzzleId && state.flagglePuzzle.answer) return;
    setState((current) => ({
      ...current,
      flagglePuzzle: {
        seedSalt,
        activeDate,
        puzzleId,
        answer: getFlaggleAnswerForDate(activeDate, seedSalt),
        guesses: [],
        completed: false,
        won: false,
      },
    }));
    setFlaggleDraft("");
    setFlaggleMessage("");
    setFlaggleDropdownOpen(false);
  });

  useEffect(() => {
    if (showFlagglePuzzle) initFlagglePuzzle();
  }, [showFlagglePuzzle]);

  const initTravlePuzzle = useEffectEvent(() => {
    const activeDate = localIsoDate();
    const seedSalt = state.travlePuzzle.seedSalt || makeTravleSeedSalt();
    const puzzleId = getTravlePuzzleId(activeDate, seedSalt);
    if (state.travlePuzzle.activeDate === activeDate && state.travlePuzzle.puzzleId === puzzleId && state.travlePuzzle.start && state.travlePuzzle.target) return;
    const puzzle = getTravlePuzzleForDate(activeDate, seedSalt);
    setState((current) => ({
      ...current,
      travlePuzzle: {
        seedSalt,
        activeDate,
        puzzleId,
        start: puzzle.start,
        target: puzzle.target,
        guesses: [],
        completed: false,
        won: false,
      },
    }));
    setTravleDraft("");
    setTravleMessage("");
    setTravleDropdownOpen(false);
    setTravleMapZoom(1);
  });

  useEffect(() => {
    if (showTravlePuzzle) initTravlePuzzle();
  }, [showTravlePuzzle]);

  async function refreshFlagglePreview(answer = state.flagglePuzzle.answer, guesses = state.flagglePuzzle.guesses.map((guess) => guess.country), revealFull = state.flagglePuzzle.completed) {
    if (!answer || !guesses.length) {
      setFlagglePreviewDataUrl("");
      return;
    }
    try {
      setFlagglePreviewDataUrl(await revealTargetFlagByGuesses(answer, guesses, revealFull));
    } catch (error) {
      console.warn("Could not render Flaggle preview.", error);
      setFlagglePreviewDataUrl("");
    }
  }

  useEffect(() => {
    if (!showFlagglePuzzle) return;
    void refreshFlagglePreview();
  }, [showFlagglePuzzle, state.flagglePuzzle.answer, state.flagglePuzzle.guesses]);

  async function installPendingUpdate() {
    if (!isTauriApp()) {
      openExternalLink(RELEASES_PAGE_URL);
      return;
    }

    if (updateInstallSupport.runtimeChannel !== "official-release" || !updateInstallSupport.canAutoInstall) {
      setUpdateInfo((current) => ({
        ...current,
        status: "available",
        releaseUrl: RELEASES_PAGE_URL,
        message: updateInstallSupport.message,
      }));
      openExternalLink(RELEASES_PAGE_URL);
      return;
    }

    const update = pendingUpdateRef.current;
    if (!update) {
      setUpdateInfo({
        status: "error",
        releaseUrl: RELEASES_PAGE_URL,
        message: "No checked update is ready to install. Check for updates again or open the release page.",
      });
      return;
    }

    let downloadedBytes = 0;
    let totalBytes: number | undefined;
    setUpdateChecking(true);
    setUpdateInfo({
      status: "installing",
      latestVersion: update.version,
      releaseUrl: RELEASES_PAGE_URL,
      message: `Downloading version ${update.version}...`,
    });

    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          downloadedBytes = 0;
          totalBytes = event.data.contentLength;
          setUpdateInfo((current) => ({ ...current, message: totalBytes ? `Downloading update: 0 / ${formatFileSize(totalBytes)}` : "Downloading update..." }));
        }
        if (event.event === "Progress") {
          downloadedBytes += event.data.chunkLength;
          setUpdateInfo((current) => ({
            ...current,
            message: totalBytes ? `Downloading update: ${formatFileSize(downloadedBytes)} / ${formatFileSize(totalBytes)}` : `Downloading update: ${formatFileSize(downloadedBytes)}`,
          }));
        }
        if (event.event === "Finished") {
          setUpdateInfo((current) => ({ ...current, message: "Installing update..." }));
        }
      });

      pendingUpdateRef.current = null;
      setUpdateInfo({
        status: "current",
        latestVersion: update.version,
        releaseUrl: RELEASES_PAGE_URL,
        message: "Update installed. Restarting Study Tracker...",
      });
      await relaunch();
    } catch (error) {
      setUpdateInfo({
        status: "error",
        latestVersion: update.version,
        releaseUrl: RELEASES_PAGE_URL,
        message: `${getErrorMessage(error, "Could not install the update.")} If you installed with .deb or .rpm, download the new installer from the release page.`,
      });
    } finally {
      setUpdateChecking(false);
    }
  }

  async function downloadManualLinuxUpdate() {
    if (!isTauriApp()) {
      openExternalLink(RELEASES_PAGE_URL);
      return;
    }

    setLinuxPackageDownloading(true);
    setLinuxUpdateDownload(null);
    setUpdateInfo((current) => ({ ...current, status: "installing", message: "Downloading Linux package..." }));

    try {
      const download = await invoke<LinuxUpdateDownload>("download_linux_update_package");
      setLinuxUpdateDownload(download);
      setUpdateInfo((current) => ({
        ...current,
        status: "available",
        latestVersion: download.version,
        message: `${download.message} The install command is shown below.`,
      }));
    } catch (error) {
      setUpdateInfo((current) => ({
        ...current,
        status: "error",
        message: `${getErrorMessage(error, "Could not download the Linux package.")} You can still use the release page.`,
      }));
    } finally {
      setLinuxPackageDownloading(false);
    }
  }

  async function copyLinuxInstallCommand() {
    if (!linuxUpdateDownload) return;
    try {
      await navigator.clipboard.writeText(linuxUpdateDownload.installCommand);
      setMessage("Install command copied.");
    } catch (error) {
      console.warn("Could not copy install command.", error);
      setMessage("Could not copy command. Select it manually instead.");
    }
  }

  async function copySourceLinuxUpdateCommand() {
    try {
      await navigator.clipboard.writeText(SOURCE_LINUX_UPDATE_COMMAND);
      setMessage("Source update commands copied.");
    } catch (error) {
      console.warn("Could not copy source update commands.", error);
      setMessage("Could not copy commands. Select them manually instead.");
    }
  }

  async function copyDevUpdateCommand() {
    try {
      await navigator.clipboard.writeText(DEV_UPDATE_COMMAND);
      setMessage("Development update commands copied.");
    } catch (error) {
      console.warn("Could not copy development update commands.", error);
      setMessage("Could not copy commands. Select them manually instead.");
    }
  }

  async function revealLinuxPackage() {
    if (!linuxUpdateDownload) return;
    try {
      await revealItemInDir(linuxUpdateDownload.filePath);
    } catch (error) {
      console.warn("Could not open downloaded package location.", error);
      setMessage("Could not open the downloads folder.");
    }
  }

  function deleteAllData() {
    const cleanState: AppState = {
      ...defaultState,
      settings: {
        ...defaultState.settings,
        themeFamily: "normal",
      },
    };
    setState(cleanState);
    setDashboardLayout("cockpit");
    setCustomDashboardLayout(defaultCustomDashboardLayout);
    setSelectedTaskId(TOTAL_WORKLOAD_ID);
    setSemesterName("");
    setCourseDraft({ semesterId: "", name: "", targetGrade: "4.0", color: "#8fb4ff" });
    setTaskDraft({ semesterId: "", courseId: "", title: "", unitLabel: "Unit", totalUnits: "10", completedUnits: "0", dueDate: "", priority: "medium", notes: "" });
    setExamDraft({ semesterId: "", courseId: "", title: "", examDate: "", weight: "40", preparedness: "35" });
    setShowSemesterForm(false);
    setExpandedSemesterIds([]);
    setExpandedCourseIds([]);
    setAddingCourseSemesterId(null);
    setAddingTaskCourseId(null);
    setAddingExamCourseId(null);
    setEditingSemesterId(null);
    setEditingCourseId(null);
    setEditingTaskId(null);
    setSelectedCalendarDate(null);
    setPersonalNameDraft("");
    APP_STATE_STORAGE_KEYS.forEach((key) => localStorage.removeItem(key));
    localStorage.removeItem(DASHBOARD_LAYOUT_KEY);
    localStorage.removeItem(CUSTOM_DASHBOARD_LAYOUT_KEY);
    localStorage.removeItem(DISMISSED_ANNOUNCEMENTS_KEY);
    localStorage.removeItem(TELEMETRY_INSTALL_ID_KEY);
    closeMenuPanel();
    setMessage("All study tracker data deleted.");
  }

  function addSemester(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = semesterName.trim();
    if (!name) {
      setMessage("Give the semester a name first.");
      return;
    }

    const semester: Semester = {
      id: makeId(),
      name,
      createdAt: new Date().toISOString(),
    };

    setState((current) => ({ ...current, semesters: [...current.semesters, semester] }));
    setSemesterName("");
    setCourseDraft((current) => ({ ...current, semesterId: semester.id }));
    setTaskDraft((current) => ({ ...current, semesterId: semester.id, courseId: "" }));
    setExamDraft((current) => ({ ...current, semesterId: semester.id, courseId: "" }));
    setShowSemesterForm(false);
    setExpandedSemesterIds((current) => (current.includes(semester.id) ? current : [...current, semester.id]));
    setAddingCourseSemesterId(semester.id);
    setMessage(`${semester.name} added.`);
  }

  function addCourse(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!courseDraft.semesterId || !courseDraft.name.trim()) {
      setMessage("Pick a semester and give the course a name.");
      return;
    }

    const course: Course = {
      id: makeId(),
      semesterId: courseDraft.semesterId,
      name: courseDraft.name.trim(),
      color: courseDraft.color,
      targetGrade: Number(courseDraft.targetGrade) || 4,
      createdAt: new Date().toISOString(),
    };

    setState((current) => ({ ...current, courses: [...current.courses, course] }));
    setCourseDraft((current) => ({ ...current, name: "", targetGrade: "4.0" }));
    setTaskDraft((current) => ({ ...current, semesterId: course.semesterId, courseId: course.id }));
    setExamDraft((current) => ({ ...current, semesterId: course.semesterId, courseId: course.id }));
    setExpandedSemesterIds((current) => (current.includes(course.semesterId) ? current : [...current, course.semesterId]));
    setExpandedCourseIds((current) => (current.includes(course.id) ? current : [...current, course.id]));
    setAddingCourseSemesterId(null);
    setAddingTaskCourseId(course.id);
    setMessage(`${course.name} added to ${semesterLookup.get(course.semesterId)?.name ?? "semester"}.`);
  }

  function startEditingSemester(semester: Semester) {
    setSemesterEditName(semester.name);
    setEditingSemesterId(semester.id);
    setAddingCourseSemesterId(null);
  }

  function updateSemester(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = semesterEditName.trim();
    if (!editingSemesterId || !name) {
      setMessage("Give the semester a name first.");
      return;
    }

    setState((current) => ({
      ...current,
      semesters: current.semesters.map((semester) => (semester.id === editingSemesterId ? { ...semester, name } : semester)),
    }));
    setEditingSemesterId(null);
    setMessage(`${name} updated.`);
  }

  function addTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!taskDraft.semesterId || !taskDraft.courseId || !taskDraft.title.trim()) {
      setMessage("A task needs a semester, course, and title.");
      return;
    }

    const task: Task = {
      id: makeId(),
      semesterId: taskDraft.semesterId,
      courseId: taskDraft.courseId,
      title: taskDraft.title.trim(),
      unitLabel: cleanUnitLabel(taskDraft.unitLabel, taskDraft.title),
      totalUnits: Math.max(1, Number(taskDraft.totalUnits) || 1),
      completedUnits: clamp(Number(taskDraft.completedUnits) || 0, 0, Number(taskDraft.totalUnits) || 1),
      dueDate: taskDraft.dueDate || null,
      priority: taskDraft.priority,
      notes: taskDraft.notes.trim(),
      createdAt: new Date().toISOString(),
    };

    setState((current) => ({ ...current, tasks: [task, ...current.tasks] }));
    setTaskDraft((current) => ({
      ...current,
      title: "",
      unitLabel: "Unit",
      totalUnits: "10",
      completedUnits: "0",
      dueDate: "",
      notes: "",
    }));
    setExpandedCourseIds((current) => (current.includes(task.courseId) ? current : [...current, task.courseId]));
    setAddingTaskCourseId(null);
    setSelectedTaskId(task.id);
    if (appStyle === "field-notebook") {
      setFieldPlannerTaskId(task.id);
      setFieldPlannerTaskMode("view");
    }
    setMessage(`${task.title} is now tracked.`);
  }

  function startEditingCourse(course: Course) {
    setCourseEditDraft({
      semesterId: course.semesterId,
      name: course.name,
      targetGrade: course.targetGrade.toString(),
      color: course.color,
    });
    setEditingCourseId(course.id);
    setAddingCourseSemesterId(null);
  }

  function updateCourse(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editingCourseId || !courseEditDraft.name.trim()) {
      setMessage("Give the course a name first.");
      return;
    }

    const name = courseEditDraft.name.trim();
    setState((current) => ({
      ...current,
      courses: current.courses.map((course) =>
        course.id === editingCourseId
          ? {
              ...course,
              name,
              targetGrade: Number(courseEditDraft.targetGrade) || 4,
              color: courseEditDraft.color,
            }
          : course,
      ),
    }));
    setEditingCourseId(null);
    setMessage(`${name} updated.`);
  }

  function startEditingTask(task: Task) {
    setTaskEditDraft({
      semesterId: task.semesterId,
      courseId: task.courseId,
      title: task.title,
      unitLabel: task.unitLabel || inferUnitLabel(task.title),
      totalUnits: task.totalUnits.toString(),
      completedUnits: task.completedUnits.toString(),
      dueDate: task.dueDate ?? "",
      priority: task.priority,
      notes: task.notes,
    });
    setEditingTaskId(task.id);
    setAddingTaskCourseId(null);
  }

  function updateTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editingTaskId || !taskEditDraft.semesterId || !taskEditDraft.courseId || !taskEditDraft.title.trim()) {
      setMessage("A task needs a semester, course, and title.");
      return;
    }

    const title = taskEditDraft.title.trim();
    const unitLabel = cleanUnitLabel(taskEditDraft.unitLabel, title);
    const totalUnits = Math.max(1, Number(taskEditDraft.totalUnits) || 1);
    const completedUnits = clamp(Number(taskEditDraft.completedUnits) || 0, 0, totalUnits);
    setState((current) => ({
      ...current,
      tasks: current.tasks.map((task) =>
        task.id === editingTaskId
          ? {
              ...task,
              title,
              unitLabel,
              totalUnits,
              completedUnits,
              dueDate: taskEditDraft.dueDate || null,
              priority: taskEditDraft.priority,
              notes: taskEditDraft.notes.trim(),
            }
          : task,
      ),
    }));
    setEditingTaskId(null);
    setMessage(`${title} updated.`);
  }

  function confirmTaskDueDate(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key !== "Enter") return;
    event.preventDefault();
    event.currentTarget.blur();
  }

  function addExam(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!examDraft.semesterId || !examDraft.courseId || !examDraft.title.trim() || !examDraft.examDate) {
      setMessage("An exam needs a semester, course, title, and date.");
      return;
    }

    const exam: Exam = {
      id: makeId(),
      semesterId: examDraft.semesterId,
      courseId: examDraft.courseId,
      title: examDraft.title.trim(),
      examDate: examDraft.examDate,
      weight: clamp(Number(examDraft.weight) || 0, 0, 100),
      preparedness: clamp(Number(examDraft.preparedness) || 0, 0, 100),
    };

    setState((current) => ({ ...current, exams: [exam, ...current.exams] }));
    setExamDraft((current) => ({ ...current, title: "", examDate: "", weight: "40", preparedness: "35" }));
    setAddingExamCourseId(null);
    setMessage(`${exam.title} added to the runway.`);
  }

  function adjustTask(taskId: string, delta: number) {
    setState((current) => ({
      ...current,
      tasks: current.tasks.map((task) =>
        task.id === taskId ? { ...task, completedUnits: clamp(task.completedUnits + delta, 0, task.totalUnits) } : task,
      ),
    }));
  }

  function removeTask(taskId: string) {
    setState((current) => ({
      ...current,
      tasks: current.tasks.filter((task) => task.id !== taskId),
      calendarEntries: current.calendarEntries.filter((entry) => entry.taskId !== taskId),
      timer: current.timer.taskId === taskId ? { ...current.timer, taskId: null } : current.timer,
    }));
    setEditingTaskId((current) => (current === taskId ? null : current));
    setFieldPlannerTaskId((current) => (current === taskId ? null : current));
    setFieldPlannerTaskMode((current) => (fieldPlannerTaskId === taskId ? null : current));
  }

  function removeCourse(courseId: string) {
    setState((current) => ({
      ...current,
      calendarEntries: current.calendarEntries.filter((entry) => {
        const task = current.tasks.find((item) => item.id === entry.taskId);
        return task?.courseId !== courseId;
      }),
      courses: current.courses.filter((course) => course.id !== courseId),
      tasks: current.tasks.filter((task) => task.courseId !== courseId),
      exams: current.exams.filter((exam) => exam.courseId !== courseId),
      timer:
        current.timer.courseId === courseId
          ? { ...current.timer, courseId: null, taskId: null }
          : current.timer,
    }));
    setExpandedCourseIds((current) => current.filter((item) => item !== courseId));
    setAddingTaskCourseId((current) => (current === courseId ? null : current));
    setAddingExamCourseId((current) => (current === courseId ? null : current));
    setEditingCourseId((current) => (current === courseId ? null : current));
    setEditingTaskId((current) => {
      const removedTaskIds = state.tasks.filter((task) => task.courseId === courseId).map((task) => task.id);
      return current && removedTaskIds.includes(current) ? null : current;
    });
  }

  function removeSemester(semesterId: string) {
    const courseIds = state.courses.filter((course) => course.semesterId === semesterId).map((course) => course.id);
    setState((current) => ({
      ...current,
      calendarEntries: current.calendarEntries.filter((entry) => {
        const task = current.tasks.find((item) => item.id === entry.taskId);
        return task?.semesterId !== semesterId;
      }),
      semesters: current.semesters.filter((semester) => semester.id !== semesterId),
      courses: current.courses.filter((course) => course.semesterId !== semesterId),
      tasks: current.tasks.filter((task) => task.semesterId !== semesterId),
      exams: current.exams.filter((exam) => exam.semesterId !== semesterId),
      timer:
        current.timer.semesterId === semesterId || courseIds.includes(current.timer.courseId ?? "")
          ? { ...current.timer, semesterId: null, courseId: null, taskId: null }
          : current.timer,
    }));
    setExpandedSemesterIds((current) => current.filter((item) => item !== semesterId));
    setExpandedCourseIds((current) => current.filter((item) => !courseIds.includes(item)));
    setAddingCourseSemesterId((current) => (current === semesterId ? null : current));
    setAddingTaskCourseId((current) => (current && courseIds.includes(current) ? null : current));
    setAddingExamCourseId((current) => (current && courseIds.includes(current) ? null : current));
    setEditingSemesterId((current) => (current === semesterId ? null : current));
    setEditingCourseId((current) => (current && courseIds.includes(current) ? null : current));
    setEditingTaskId((current) => {
      const removedTaskIds = state.tasks.filter((task) => task.semesterId === semesterId).map((task) => task.id);
      return current && removedTaskIds.includes(current) ? null : current;
    });
  }

  function removeExam(examId: string) {
    setState((current) => ({ ...current, exams: current.exams.filter((exam) => exam.id !== examId) }));
  }

  function removeSession(sessionId: string) {
    const session = state.sessions.find((item) => item.id === sessionId);
    if (!session) return;
    const countsTowardLifetime = session.kind === "study" || session.kind === "exam";
    const publishedPost = [...state.social.cachedFeeds.global, ...state.social.cachedFeeds.friends].some((post) => post.id === sessionId);
    if (!window.confirm("Delete this session? This removes its time from your history and statistics.")) return;
    setState((current) => {
      const remaining = current.sessions.filter((item) => item.id !== sessionId);
      const removedMinutes = countsTowardLifetime ? Math.max(0, Math.round(session.minutes)) : 0;
      const removedSessions = countsTowardLifetime ? 1 : 0;
      return {
        ...current,
        sessions: remaining,
        lifetimeStudyMinutes: Math.max(0, (current.lifetimeStudyMinutes || 0) - removedMinutes),
        lifetimeStudySessions: Math.max(0, (current.lifetimeStudySessions || 0) - removedSessions),
        social: {
          ...current.social,
           pendingFeedPosts: current.social.pendingFeedPosts.filter((post) => post.id !== sessionId),
           pendingFeedPostDeletions: publishedPost && !current.social.pendingFeedPostDeletions.includes(sessionId)
             ? [...current.social.pendingFeedPostDeletions, sessionId]
             : current.social.pendingFeedPostDeletions,
          cachedFeeds: {
            global: current.social.cachedFeeds.global.filter((post) => post.id !== sessionId),
            friends: current.social.cachedFeeds.friends.filter((post) => post.id !== sessionId),
          },
        },
      };
    });
    setMessage("Session deleted.");
  }

  function unlockGame(name: string) {
    const t = isoDate();
    setState((current) => {
      const freshUnlocked = current.unlockedGamesDate === t ? current.unlockedGames : [];
      const isFirstUnlockToday = freshUnlocked.length === 0;
      const alreadySpeedrunnerToday = current.lastUnlockDate === t && current.speedrunnerToday;
      const earnedFirstTokenInOneSession = current.sessions.some((session) => {
        if (session.kind !== "study" && session.kind !== "exam") return false;
        return isoDate(new Date(session.endedAt)) === t && session.minutes >= 45;
      });

      const newStreak = (() => {
        if (!current.lastUnlockDate) return 1;
        const prev = new Date(current.lastUnlockDate);
        const today = new Date(t + "T00:00:00");
        const diff = (today.getTime() - prev.getTime()) / (1000 * 60 * 60 * 24);
        if (diff === 1) return current.unlockStreak + 1;
        if (diff === 0) return current.unlockStreak;
        return 1;
      })();

      return {
        ...current,
        unlockedGamesDate: t,
        unlockedGames: [...freshUnlocked, name],
        totalUnlocks: current.totalUnlocks + 1,
        speedrunnerToday: alreadySpeedrunnerToday || (isFirstUnlockToday && earnedFirstTokenInOneSession),
        unlockStreak: newStreak,
        lastUnlockDate: t,
      };
    });
  }

  function logPlayedBreak(name: string) {
    const t = isoDate();
    setState(s => ({
      ...s,
      playedBreaksDate: t,
      playedBreaks: [
        ...(s.playedBreaksDate === t ? s.playedBreaks : []),
        { name, playedAt: new Date().toISOString() },
      ],
      playedGamesAllTime: s.playedGamesAllTime.includes(name)
        ? s.playedGamesAllTime
        : [...s.playedGamesAllTime, name],
    }));
  }

  function saveDurakState(gs: DurakGameState, failures?: number, completed?: boolean) {
    setDurakGameState(gs);
    const today = state.durakPuzzle.seed ?? isoDate();
    const hint = state.durakPuzzle.hint;
    const serialized = gameStateToPuzzle(gs, failures ?? state.durakPuzzle.failures, completed ?? state.durakPuzzle.completed, hint, today);
    setState((s) => ({ ...s, durakPuzzle: { ...s.durakPuzzle, ...serialized } }));
  }

  function handleDurakCardClick(idx: number) {
    if (!durakGameState) return;
    const card = durakGameState.playerHand[idx];

    if (durakGameState.phase === "player_defense") {
      const target = durakGameState.table.find((e) => !e.defense);
      const canBeatCard = target ? canBeat(card, target.attack, durakGameState.trumpSuit) : false;
      const canSlideCard = getLegalSlideCards(durakGameState, "player").some((slideCard) => slideCard.suit === card.suit && slideCard.rank === card.rank);
      if (!canBeatCard && !canSlideCard) return;
      setDurakSelected((prev) => prev.includes(idx) ? [] : [idx]);
      return;
    }

    if (durakGameState.phase === "player_attack" || durakGameState.phase === "player_throw") {
      const tableRanks = durakGameState.phase === "player_throw"
        ? new Set(durakGameState.table.flatMap((e) => [e.attack.rank, e.defense?.rank].filter(Boolean)))
        : null;

      if (durakGameState.phase === "player_throw" && tableRanks && !tableRanks.has(card.rank)) return;

      setDurakSelected((prev) => {
        if (prev.includes(idx)) return prev.filter((i) => i !== idx);
        if (prev.length === 0) return [idx];
        const firstCard = durakGameState.playerHand[prev[0]];
        if (firstCard.rank !== card.rank) return [idx];

        const maxCards = durakGameState.phase === "player_attack" ? Math.min(6, durakGameState.cpuHand.length) : Math.max(0, getAttackLimitAgainstCpu(durakGameState) - durakGameState.table.length);
        if (prev.length >= maxCards) return prev;

        return [...prev, idx];
      });
    }
  }

  function resetDurakAfterFail() {
    const failures = state.durakPuzzle.failures + 1;
    const seed = state.durakPuzzle.seed ?? isoDate();
    const puzzle = findDailyPuzzle(seed);
    if (puzzle) {
      setDurakGameState(puzzle.initialState);
      const serialized = gameStateToPuzzle(puzzle.initialState, failures, false, puzzle.hint, seed);
      setState((s) => ({ ...s, durakPuzzle: { ...s.durakPuzzle, ...serialized } }));
    }
    setDurakSelected([]);
  }

  function handleDurakFinished(next: DurakGameState) {
    if (next.phase !== "finished") return saveDurakState(next);
    if (next.winner === "player") {
      saveDurakState(next, state.durakPuzzle.failures, true);
      setState((s) => ({ ...s, durakPuzzle: { ...s.durakPuzzle, solvedCount: (s.durakPuzzle.solvedCount || 0) + 1 } }));
    } else {
      setDurakGameState(next);
      saveDurakState(next, state.durakPuzzle.failures, false);
    }
  }

  function handleDurakAttack() {
    if (!durakGameState || durakSelected.length === 0 || durakGameState.phase !== "player_attack") return;
    const maxAttack = Math.min(6, durakGameState.cpuHand.length);
    const cards = durakSelected.map((i) => durakGameState.playerHand[i]).slice(0, maxAttack);
    handleDurakFinished(processCpuTurn(executePlayerAttack(durakGameState, cards)));
    setDurakSelected([]);
  }

  function handleDurakThrow(forcePass = false) {
    if (!durakGameState || durakGameState.phase !== "player_throw") return;
    if (!forcePass && durakSelected.length > 0) {
      const maxThrow = Math.max(0, getAttackLimitAgainstCpu(durakGameState) - durakGameState.table.length);
      const cards = durakSelected.map((i) => durakGameState.playerHand[i]).slice(0, maxThrow);
      handleDurakFinished(processCpuTurn(executePlayerThrow(durakGameState, cards)));
    } else {
      handleDurakFinished(processCpuTurn(playerPassThrow(durakGameState)));
    }
    setDurakSelected([]);
  }

  function handleDurakDefend() {
    if (!durakGameState || durakSelected.length === 0 || durakGameState.phase !== "player_defense") return;
    const card = durakGameState.playerHand[durakSelected[0]];
    handleDurakFinished(processCpuTurn(defendOneCard(durakGameState, card)));
    setDurakSelected([]);
  }

  function handleDurakPickUp() {
    if (!durakGameState || durakGameState.phase !== "player_defense") return;
    setDurakSelected([]);
    handleDurakFinished(processCpuTurn(playerPickUp(durakGameState)));
  }

  function handleDurakSlide(card: Card) {
    if (!durakGameState || durakGameState.phase !== "player_defense") return;
    const canSlide = getLegalSlideCards(durakGameState, "player").some((slideCard) => slideCard.suit === card.suit && slideCard.rank === card.rank);
    if (!canSlide) return;
    handleDurakFinished(processCpuTurn(executeSlide(durakGameState, card)));
    setDurakSelected([]);
  }

  function submitWordleGuess() {
    if (state.wordlePuzzle.completed) return;
    const guess = normalizeWordleGuess(wordleDraft);
    if (guess.length !== WORDLE_WORD_LENGTH) {
      setWordleMessage("Enter 5 letters.");
      return;
    }
    if (!isAcceptedWordleGuess(guess)) {
      setWordleMessage("Not in the word list.");
      return;
    }
    if (state.wordlePuzzle.hardMode) {
      const violation = getWordleHardModeViolation(guess, state.wordlePuzzle.guesses, state.wordlePuzzle.answer);
      if (violation) {
        setWordleMessage(violation);
        return;
      }
    }

    setState((current) => {
      if (current.wordlePuzzle.completed || current.wordlePuzzle.guesses.includes(guess)) return current;
      const guesses = [...current.wordlePuzzle.guesses, guess];
      const won = guess === current.wordlePuzzle.answer;
      const completed = won || guesses.length >= WORDLE_MAX_GUESSES;
      return {
        ...current,
        wordlePuzzle: {
          ...current.wordlePuzzle,
          guesses,
          won,
          completed,
        },
      };
    });

    if (guess === state.wordlePuzzle.answer) {
      setWordleMessage(`Solved in ${state.wordlePuzzle.guesses.length + 1}.`);
    } else if (state.wordlePuzzle.guesses.length + 1 >= WORDLE_MAX_GUESSES) {
      setWordleMessage(`Answer: ${state.wordlePuzzle.answer.toUpperCase()}`);
    } else {
      setWordleMessage("");
    }
    setWordleDraft("");
  }

  function addWordleLetter(letter: string) {
    if (state.wordlePuzzle.completed) return;
    setWordleDraft((current) => normalizeWordleGuess(`${current}${letter}`).slice(0, WORDLE_WORD_LENGTH));
    setWordleMessage("");
  }

  function handleWordleBackspace() {
    if (state.wordlePuzzle.completed) return;
    setWordleDraft((current) => current.slice(0, -1));
    setWordleMessage("");
  }

  function handleWordleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Enter") {
      event.preventDefault();
      submitWordleGuess();
      return;
    }
    if (event.key === "Backspace") {
      event.preventDefault();
      handleWordleBackspace();
      return;
    }
    if (/^[a-z]$/i.test(event.key)) {
      event.preventDefault();
      addWordleLetter(event.key.toLowerCase());
    }
  }

  function toggleWordleHardMode() {
    if (wordleHardModeLocked) return;
    setState((current) => ({
      ...current,
      wordlePuzzle: {
        ...current.wordlePuzzle,
        hardMode: !current.wordlePuzzle.hardMode,
      },
    }));
    setWordleMessage("");
  }

  function selectGeodleCountry(name: string) {
    setGeodleDraft(name);
    setGeodleDropdownOpen(false);
    setGeodleMessage("");
  }

  function submitGeodleGuess() {
    if (state.geodlePuzzle.completed) return;
    const country = findCountryByName(geodleDraft);
    if (!country) {
      setGeodleMessage("Select a country from the list.");
      setGeodleDropdownOpen(true);
      return;
    }
    if (state.geodlePuzzle.guesses.includes(country.name)) {
      setGeodleMessage("You already guessed that country.");
      return;
    }

    setState((current) => {
      if (current.geodlePuzzle.completed || current.geodlePuzzle.guesses.includes(country.name)) return current;
      const guesses = [...current.geodlePuzzle.guesses, country.name];
      const won = country.name === current.geodlePuzzle.answer;
      const completed = won || guesses.length >= GEODLE_MAX_GUESSES;
      return {
        ...current,
        geodlePuzzle: {
          ...current.geodlePuzzle,
          guesses,
          won,
          completed,
        },
      };
    });

    if (country.name === state.geodlePuzzle.answer) {
      setGeodleMessage(`Solved in ${state.geodlePuzzle.guesses.length + 1}.`);
    } else if (state.geodlePuzzle.guesses.length + 1 >= GEODLE_MAX_GUESSES) {
      setGeodleMessage(`Answer: ${state.geodlePuzzle.answer}.`);
    } else {
      setGeodleMessage("");
    }
    setGeodleDraft("");
    setGeodleDropdownOpen(false);
  }

  function selectFlaggleCountry(name: string) {
    setFlaggleDraft(name);
    setFlaggleDropdownOpen(false);
    setFlaggleMessage("");
  }

  async function submitFlaggleGuess() {
    if (state.flagglePuzzle.completed || flaggleProcessing) return;
    const country = findFlaggleCountry(flaggleDraft);
    if (!country) {
      setFlaggleMessage("Select a country from the list.");
      setFlaggleDropdownOpen(true);
      return;
    }
    if (state.flagglePuzzle.guesses.some((guess) => guess.country === country.name)) {
      setFlaggleMessage("You already guessed that country.");
      return;
    }

    setFlaggleProcessing(true);
    try {
      const result = await maskFlagByTargetColors(country.name, state.flagglePuzzle.answer);
      const nextGuessNames = [...state.flagglePuzzle.guesses.map((guess) => guess.country), country.name];
      setState((current) => {
        if (current.flagglePuzzle.completed || current.flagglePuzzle.guesses.some((guess) => guess.country === country.name)) return current;
        const guesses = [...current.flagglePuzzle.guesses, { country: country.name, similarity: result.similarity, maskedFlagDataUrl: result.maskedFlagDataUrl }];
        const won = country.name === current.flagglePuzzle.answer;
        const completed = won || guesses.length >= FLAGGLE_MAX_GUESSES;
        return {
          ...current,
          flagglePuzzle: {
            ...current.flagglePuzzle,
            guesses,
            won,
            completed,
          },
        };
      });

      if (country.name === state.flagglePuzzle.answer) {
        setFlaggleMessage(`Solved in ${state.flagglePuzzle.guesses.length + 1}.`);
      } else if (state.flagglePuzzle.guesses.length + 1 >= FLAGGLE_MAX_GUESSES) {
        setFlaggleMessage(`Answer: ${state.flagglePuzzle.answer}.`);
      } else {
        setFlaggleMessage("");
      }
      void refreshFlagglePreview(state.flagglePuzzle.answer, nextGuessNames, country.name === state.flagglePuzzle.answer || state.flagglePuzzle.guesses.length + 1 >= FLAGGLE_MAX_GUESSES);
      setFlaggleDraft("");
      setFlaggleDropdownOpen(false);
    } catch (error) {
      console.warn("Could not process Flaggle guess.", error);
      setFlaggleMessage("Could not render that flag. Try another guess.");
    } finally {
      setFlaggleProcessing(false);
    }
  }

  function selectTravleCountry(name: string) {
    setTravleDraft(name);
    setTravleDropdownOpen(false);
    setTravleMessage("");
  }

  function submitTravleGuess() {
    if (state.travlePuzzle.completed) return;
    const country = findTravleCountry(travleDraft);
    if (!country) {
      setTravleMessage("Select a country from the list.");
      setTravleDropdownOpen(true);
      return;
    }

    if (country.name === state.travlePuzzle.start || state.travlePuzzle.guesses.includes(country.name)) {
      setTravleMessage("That country is already in your route.");
      return;
    }

    const nextGuesses = [...state.travlePuzzle.guesses, country.name];
    const solved = isTravleRouteSolved(state.travlePuzzle.start, state.travlePuzzle.target, nextGuesses);
    const guessStates = getTravleGuessStates(state.travlePuzzle.start, state.travlePuzzle.target, nextGuesses);
    const guessState = guessStates[country.name];

    setState((current) => {
      if (current.travlePuzzle.completed || current.travlePuzzle.guesses.includes(country.name)) return current;
      const guesses = [...current.travlePuzzle.guesses, country.name];
      const won = isTravleRouteSolved(current.travlePuzzle.start, current.travlePuzzle.target, guesses);
      const completed = won || guesses.length >= TRAVLE_MAX_GUESSES;
      return {
        ...current,
        travlePuzzle: {
          ...current.travlePuzzle,
          guesses,
          won,
          completed,
        },
      };
    });

    if (solved) {
      const route = getTravleSolvedPath(state.travlePuzzle.start, state.travlePuzzle.target, nextGuesses).join(" -> ");
      setTravleMessage(`Route complete: ${route}.`);
    } else if (nextGuesses.length >= TRAVLE_MAX_GUESSES) {
      const shortest = getTravleShortestPath(state.travlePuzzle.start, state.travlePuzzle.target).join(" -> ");
      setTravleMessage(`Route closed. Shortest path: ${shortest}.`);
    } else if (guessState === "route") {
      setTravleMessage(`${country.name} is on the route.`);
    } else if (guessState === "possible") {
      setTravleMessage(`${country.name} could help connect the route.`);
    } else {
      setTravleMessage(`${country.name} is off route.`);
    }
    setTravleDraft("");
    setTravleDropdownOpen(false);
  }

  function addWater() {
    const t = isoDate();
    setState(s => ({
      ...s,
      waterDate: t,
      waterGlasses: (s.waterDate === t ? s.waterGlasses : 0) + 1,
    }));
  }

  const [rockBounce, setRockBounce] = useState(false);
  const [rockCelebrating, setRockCelebrating] = useState(false);

  function patRock() {
    setState(s => {
      const next = s.petRockPats + 1;
      if (next === 1000) setRockCelebrating(true);
      return { ...s, petRockPats: next };
    });
    setRockBounce(true);
    setTimeout(() => setRockBounce(false), 350);
  }

  function toggleBreakTimerRun() {
    setBreakTimerRunning((current) => {
      if (!current && breakTimerRemaining <= 0) setBreakTimerRemaining(breakTimerMinutes * 60);
      return !current;
    });
  }

  function resetBreakTimer() {
    setBreakTimerRunning(false);
    setBreakTimerRemaining(breakTimerMinutes * 60);
  }

  function toggleBreathingExercise() {
    setBreathOn((current) => !current);
    setBreathElapsedSeconds(0);
  }

  function startBreakTimerEditing() {
    if (breakTimerRunning) return;
    setBreakTimerDraft(formatClock(breakTimerRemaining));
    setBreakTimerEditing(true);
  }

  function applyBreakTimerDraft() {
    const seconds = parseTimerFaceInput(breakTimerDraft);
    if (seconds === null) {
      setBreakTimerEditing(false);
      setBreakTimerDraft(formatClock(breakTimerRemaining));
      return;
    }
    setBreakTimerMinutes(Math.max(1, Math.round(seconds / 60)));
    setBreakTimerRemaining(seconds);
    setBreakTimerEditing(false);
  }

  const [celebrating, setCelebrating] = useState<string | null>(null);
  const [quoteIndex] = useState(() => Math.floor(Math.random() * breakQuotes.length));
  const quote = breakQuotes[quoteIndex];
  const [stretchIndex, setStretchIndex] = useState(() => Math.floor(Math.random() * stretchIdeas.length));
  const stretch = stretchIdeas[stretchIndex];

  function addCalendarEntryFromDrawer() {
    if (!selectedCalendarDate) return;

    const durationMinutes = Number(calendarAddDraft.durationMinutes);
    const startTime = calendarAddDraft.startTime;
    if (!calendarAddDraft.noTime && !isValidCalendarTime(startTime)) {
      setMessage("Use 24-hour time like 09:00 or 17:30.");
      return;
    }
    const endTime = addMinutesToTime(startTime, Number.isFinite(durationMinutes) ? durationMinutes : 60);
    const timeOptions = calendarAddDraft.noTime ? {} : { startTime, endTime };

    if (calendarAddDraft.source === "planner") {
      const task = taskLookup.get(calendarAddDraft.taskId);
      if (!task) {
        setMessage("Pick a planner task first.");
        return;
      }
      if (task.dueDate && selectedCalendarDate > task.dueDate) {
        setMessage(`"${task.title}" cannot be scheduled after ${formatDate(task.dueDate)}.`);
        return;
      }

      const unscheduledUnits = Math.max(
        0,
        getRemainingUnits(task) - (scheduledIncompleteByTask.get(task.id) ?? 0) - (completedCalendarRemainderByTask.get(task.id) ?? 0),
      );
      if (unscheduledUnits < calendarAddDraft.unitAmount) {
        setMessage(`All remaining units for "${task.title}" are already planned.`);
        return;
      }

      const entry: CalendarEntry = {
        id: makeId(),
        taskId: task.id,
        date: selectedCalendarDate,
        unitAmount: calendarAddDraft.unitAmount,
        unitStart: Math.min(
          task.totalUnits,
          Math.max(
            1,
            Math.floor(
              task.completedUnits
              + (scheduledIncompleteByTask.get(task.id) ?? 0)
              + (completedCalendarRemainderByTask.get(task.id) ?? 0),
            ) + 1,
          ),
        ),
        completed: false,
        completedAt: null,
        createdAt: new Date().toISOString(),
        ...timeOptions,
      };
      setState((current) => ({ ...current, calendarEntries: [...current.calendarEntries, entry] }));
      setCalendarAddOpen(false);
      return;
    }

    const title = calendarAddDraft.title.trim();
    if (!title) {
      setMessage("Type a task title first.");
      return;
    }

    const entry: CalendarEntry = {
      id: makeId(),
      taskId: "",
      date: selectedCalendarDate,
      unitAmount: calendarAddDraft.unitAmount,
      completed: false,
      completedAt: null,
      createdAt: new Date().toISOString(),
      adHocTitle: title,
      adHocSemesterId: calendarAddDraft.semesterId || undefined,
      adHocCourseId: calendarAddDraft.courseId || undefined,
      ...timeOptions,
    };

    setState((current) => ({ ...current, calendarEntries: [...current.calendarEntries, entry] }));
    setCalendarAddDraft((current) => ({ ...current, title: "" }));
    setCalendarAddOpen(false);
  }

  function toggleCalendarEntry(entryId: string) {
    setState((current) => {
      const entry = current.calendarEntries.find((item) => item.id === entryId);
      if (!entry) return current;

      const completing = !entry.completed;
      const beforeWholeUnits = getCompletedCalendarWholeUnits(current.calendarEntries, entry.taskId);
      const calendarEntries = current.calendarEntries.map((item) =>
        item.id === entryId
          ? { ...item, completed: completing, completedAt: completing ? new Date().toISOString() : null }
          : item,
      );
      const afterWholeUnits = getCompletedCalendarWholeUnits(calendarEntries, entry.taskId);
      const wholeUnitDelta = afterWholeUnits - beforeWholeUnits;

      return {
        ...current,
        calendarEntries,
        tasks: current.tasks.map((task) =>
          task.id === entry.taskId
            ? { ...task, completedUnits: clamp(task.completedUnits + wholeUnitDelta, 0, task.totalUnits) }
            : task,
        ),
      };
    });
  }

  function removeCalendarEntry(entryId: string) {
    setState((current) => ({
      ...current,
      calendarEntries: current.calendarEntries.filter((entry) => entry.id !== entryId || entry.completed),
    }));
  }

  function updateCalendarEntryTime(entryId: string, startTime: string, endTime: string) {
    setState((current) => ({
      ...current,
      calendarEntries: current.calendarEntries.map((entry) =>
        entry.id === entryId ? { ...entry, startTime, endTime } : entry,
      ),
    }));
  }

  function unscheduleCalendarEntry(entryId: string) {
    setState((current) => ({
      ...current,
      calendarEntries: current.calendarEntries.map((entry) =>
        entry.id === entryId ? { ...entry, startTime: undefined, endTime: undefined } : entry,
      ),
    }));
    setCalendarEditEntryId(null);
  }

  function getEntryDurationMinutes(entry: CalendarEntry) {
    if (!entry.startTime || !entry.endTime) return 60;
    return Math.max(calendarTimeStepMinutes, timeToMinutes(entry.endTime) - timeToMinutes(entry.startTime));
  }

  function getTimelineTimeFromClientY(timeline: HTMLDivElement, clientY: number) {
    const rect = timeline.getBoundingClientRect();
    const ratio = clamp((clientY - rect.top) / Math.max(1, rect.height), 0, 0.99);
    const minuteOffset = snapCalendarMinutes(ratio * calendarTimelineTotalMinutes);
    const minutes = clamp(calendarTimelineStartMinutes + minuteOffset, calendarTimelineStartMinutes, calendarTimelineEndMinutes - calendarTimeStepMinutes);
    return {
      minutes,
      time: minutesToTime(minutes),
      top: ((minutes - calendarTimelineStartMinutes) / calendarTimelineTotalMinutes) * calendarTimelineHeight,
    };
  }

  function startCalendarEntryMove(event: MouseEvent<HTMLDivElement>, entry: CalendarEntry) {
    if (calendarEditEntryId || calendarResizeRef.current) return;
    const target = event.target as HTMLElement;
    if (target.closest("button,input,.calendar-resize-handle")) return;
    event.preventDefault();
    setCalendarEditEntryId(null);
    calendarMoveDragRef.current = { entryId: entry.id, durationMinutes: getEntryDurationMinutes(entry), startX: event.clientX, startY: event.clientY, moved: false };
    setCalendarDragEntryId(entry.id);
  }

  function startCalendarEntryEdit(entry: CalendarEntry) {
    const startTime = entry.startTime ?? "09:00";
    setCalendarEditEntryId(entry.id);
    setCalendarEditDraft({ startTime, endTime: entry.endTime ?? addMinutesToTime(startTime, 60) });
  }

  function saveCalendarEntryEdit(entryId: string) {
    if (!isValidCalendarTime(calendarEditDraft.startTime) || !isValidCalendarTime(calendarEditDraft.endTime)) {
      setMessage("Use 24-hour time like 09:00 or 17:30.");
      return;
    }
    const startMinutes = timeToMinutes(calendarEditDraft.startTime);
    const endMinutes = timeToMinutes(calendarEditDraft.endTime);
    if (endMinutes <= startMinutes) {
      setMessage("End time must be after start time.");
      return;
    }

    updateCalendarEntryTime(entryId, calendarEditDraft.startTime, calendarEditDraft.endTime);
    setCalendarEditEntryId(null);
  }

  function startCalendarEntryResize(event: MouseEvent<HTMLDivElement>, entry: CalendarEntry) {
    if (!entry.startTime) return;
    event.preventDefault();
    event.stopPropagation();
    const startMinutes = timeToMinutes(entry.startTime);
    const endMinutes = timeToMinutes(entry.endTime ?? addMinutesToTime(entry.startTime, 60));
    calendarResizeRef.current = { entryId: entry.id, startY: event.clientY, startMinutes, endMinutes };
    setCalendarResizeEntryId(entry.id);
  }

  function shiftCalendar(direction: -1 | 1) {
    setCalendarCursorDate((current) => {
      if (calendarView === "month") {
        return new Date(current.getFullYear(), current.getMonth() + direction, 1);
      }
      return addCalendarDays(current, direction * 7);
    });
  }

  function jumpCalendarToToday() {
    const today = new Date();
    setCalendarCursorDate(today);
    setSelectedCalendarDate(localIsoDate(today));
  }

  function setPlannerCalendarView(view: CalendarView) {
    setCalendarView(view);
    if (view === "week") {
      setCalendarCursorDate(new Date());
    }
  }

  function openCalendarDrawer(date: string) {
    setSelectedCalendarDate(date);
    setCalendarAddOpen(false);
    setCalendarEditEntryId(null);
  }

  function closeCalendarDrawer() {
    setSelectedCalendarDate(null);
    setCalendarAddOpen(false);
    setCalendarEditEntryId(null);
    setCalendarDragEntryId(null);
    calendarMoveDragRef.current = null;
    setCalendarMovePreview(null);
  }

  function openCalendarAddDrawer() {
    setCalendarAddOpen(true);
  }

  function openTodayTodoDrawer() {
    const today = localIsoDate();
    setCalendarCursorDate(parseCalendarDate(today));
    setSelectedCalendarDate(today);
    setCalendarEditEntryId(null);
    setCalendarAddDraft((current) => ({ ...current, source: "new", noTime: true }));
    setCalendarAddOpen(true);
  }

  function closeCalendarAddDrawer() {
    setCalendarAddOpen(false);
  }

  function openNextCalendarDay() {
    setSelectedCalendarDate((current) => {
      const nextDate = addCalendarDays(current ? parseCalendarDate(current) : new Date(), 1);
      setCalendarCursorDate(nextDate);
      setCalendarAddOpen(false);
      return localIsoDate(nextDate);
    });
  }

  function toggleSemester(semesterId: string) {
    setExpandedSemesterIds((current) => toggleId(current, semesterId));
  }

  function toggleCourse(courseId: string) {
    setExpandedCourseIds((current) => toggleId(current, courseId));
  }

  function applyPreset(label: string, study: number, breakMinutes: number, mode: "focus" | "exam" | "endless") {
    setTimerFaceEditing(false);
    setState((current) => ({
      ...current,
      timer: {
        ...current.timer,
        mode,
        studyMinutes: mode === "exam" ? current.timer.studyMinutes : study,
        breakMinutes,
        examMinutes: mode === "exam" ? study : current.timer.examMinutes,
        presetLabel: label,
        phase: current.timer.running ? current.timer.phase : "idle",
        remainingSeconds: current.timer.running
          ? getDisplayRemainingSeconds(current.timer)
          : getIdleTimerSeconds({
              mode,
              studyMinutes: mode === "exam" ? current.timer.studyMinutes : study,
              examMinutes: mode === "exam" ? study : current.timer.examMinutes,
        }),
        startedAt: current.timer.running ? current.timer.startedAt : null,
        endsAt: current.timer.running ? current.timer.endsAt : null,
        loggedSplitSeconds: current.timer.running ? current.timer.loggedSplitSeconds : 0,
        activeSegments: current.timer.running ? current.timer.activeSegments : [],
      },
    }));
  }

  function parseTimerFaceInput(value: string) {
    const trimmed = value.trim();
    if (!trimmed) return null;
    if (/^\d+$/.test(trimmed)) return Math.max(1, Number(trimmed) * 60);
    const match = trimmed.match(/^(\d{1,4}):(\d{1,2})$/);
    if (!match) return null;
    const minutes = Number(match[1]);
    const seconds = Number(match[2]);
    if (!Number.isFinite(minutes) || !Number.isFinite(seconds) || seconds > 59) return null;
    return Math.max(1, minutes * 60 + seconds);
  }

  function applyTimerFaceDraft() {
    const seconds = parseTimerFaceInput(timerFaceDraft);
    if (!seconds) {
      setMessage("Use minutes or MM:SS, for example 90 or 90:00.");
      setTimerFaceDraft(formatClock(state.timer.remainingSeconds));
      setTimerFaceEditing(false);
      return;
    }

    const minutes = Math.max(1, Math.ceil(seconds / 60));
    setState((current) => ({
      ...current,
      timer: {
        ...current.timer,
        studyMinutes: current.timer.mode === "exam" ? current.timer.studyMinutes : minutes,
        examMinutes: current.timer.mode === "exam" ? minutes : current.timer.examMinutes,
        remainingSeconds: seconds,
        presetLabel: current.timer.mode === "exam" ? current.timer.presetLabel : "Custom",
      },
    }));
    setTimerFaceEditing(false);
  }

  function startTimerFaceEditing() {
    if (appStyle === "modern" || state.timer.running || state.timer.phase !== "idle" || (state.timer.mode !== "exam" && !isCustomTimerPreset)) return;
    setTimerFaceDraft(formatClock(state.timer.remainingSeconds));
    setTimerFaceEditing(true);
  }

  function clearEndlessInactivityPrompt() {
    endlessInactivityPromptRef.current = null;
    setEndlessInactivityPrompt(null);
  }

  function acknowledgeEndlessInactivityPrompt() {
    clearEndlessInactivityPrompt();
    endlessContinuousStartedAtRef.current = new Date().toISOString();
  }

  function startTimer() {
    const isEndless = state.timer.mode === "endless";
    const isExam = state.timer.mode === "exam";
    const totalSeconds = state.timer.phase === "idle" ? Math.max(1, state.timer.remainingSeconds) : (isExam ? state.timer.examMinutes : state.timer.studyMinutes) * 60;
    const startedAt = new Date().toISOString();
    void prepareBellSound().then((result) => {
      if (!result.ok) console.warn("Bell sound could not be prepared.", result);
    });
    void ensureTimerNotificationPermission().catch((error: unknown) => {
      console.warn("Timer notification permission could not be prepared.", error);
    });

    if (isEndless) {
      endlessContinuousStartedAtRef.current = startedAt;
      clearEndlessInactivityPrompt();
      setState((current) => ({
        ...current,
        activeTab: "timer",
        timer: {
          ...current.timer,
          running: true,
          phase: "stopwatch",
          startedAt,
          endsAt: null,
          remainingSeconds: 0,
          loggedSplitSeconds: 0,
          activeSegments: [{ startedAt, endedAt: null }],
        },
      }));
      return;
    }

    setState((current) => ({
      ...current,
      activeTab: "timer",
      timer: {
        ...current.timer,
        running: true,
        phase: isExam ? "exam" : "study",
        startedAt,
        endsAt: new Date(Date.now() + totalSeconds * 1000).toISOString(),
        remainingSeconds: totalSeconds,
        loggedSplitSeconds: 0,
        activeSegments: [{ startedAt, endedAt: null }],
      },
    }));

    setFullscreen(true);
  }

  function pauseTimer() {
    if (!state.timer.running) {
      void prepareBellSound().then((result) => {
        if (!result.ok) console.warn("Bell sound could not be prepared.", result);
      });
    }

    setState((current) => {
      const timer = current.timer;
      if (timer.phase === "idle") return current;

      if (timer.phase === "stopwatch") {
        if (timer.running) {
          endlessContinuousStartedAtRef.current = null;
          clearEndlessInactivityPrompt();
          return { ...current, timer: pauseStopwatchTimer(timer) };
        }
        endlessContinuousStartedAtRef.current = new Date().toISOString();
        clearEndlessInactivityPrompt();
        return { ...current, timer: resumeStopwatchTimer(timer) };
      }

      if (timer.running && timer.endsAt) {
        return { ...current, timer: pauseCountdownTimer(timer, timer.endsAt) };
      }

      return { ...current, timer: resumeCountdownTimer(timer) };
    });
  }

  function resetTimer() {
    endlessContinuousStartedAtRef.current = null;
    clearEndlessInactivityPrompt();
    setState((current) => ({
      ...current,
      timer: {
        ...defaultTimer,
        ...keepTimerContext(current.timer),
        running: false,
        startedAt: null,
        endsAt: null,
        remainingSeconds: getIdleTimerSeconds(current.timer),
        phase: "idle",
      },
    }));
  }

  function completeSessionManually() {
    if (sessionSavePendingRef.current) return;
    if (state.timer.phase !== "study" && state.timer.phase !== "exam" && state.timer.phase !== "stopwatch") {
      setMessage("There is no active study block to save.");
      return;
    }
    if (getTimerMinutes(state.timer) <= 0) {
      setMessage("Start the timer before saving a session.");
      return;
    }

    sessionSavePendingRef.current = true;
    const timer = state.timer;
    const endedAt = new Date().toISOString();
    const sessions = buildSessionsFromTimerRange(timer, endedAt);
    const socialState = prependSessionsToState(state, sessions, sessions[sessions.length - 1]);
    const nextState = {
      ...socialState,
      timer: {
        ...defaultTimer,
        ...keepTimerContext(timer),
        running: false,
        startedAt: null,
        endsAt: null,
        phase: "idle" as const,
        remainingSeconds: getIdleTimerSeconds(timer),
      },
    };
    playBellSound();
    endlessContinuousStartedAtRef.current = null;
    clearEndlessInactivityPrompt();
    setState(nextState);
    persistState(nextState);
    window.setTimeout(() => {
      sessionSavePendingRef.current = false;
    }, 750);
    setMessage("Session saved.");
  }

  async function handleCreateVault() {
    if (!isTauriApp()) {
      setMessage("Vault creation works inside the desktop build.");
      return;
    }

    try {
      const parent = await pickVaultParentDirectory();
      if (!parent) return;
      const vaultPath = await createVault(parent, state.settings.vaultName || "StudyTrackerVault");
      setState((current) => ({
        ...current,
        activeTab: "vault",
        settings: { ...current.settings, vaultPath },
      }));
      await loadVaultNote(vaultPath, vaultNoteDate);
      setMessage(`Vault created at ${vaultPath}`);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not create the vault."));
    }
  }

  async function handleLinkVault() {
    if (!isTauriApp()) {
      setMessage("Linking a vault works inside the desktop build.");
      return;
    }

    try {
      const selected = await pickExistingVaultDirectory();
      if (!selected) return;
      const vaultPath = await linkVault(selected);
      setState((current) => ({
        ...current,
        activeTab: "vault",
        settings: { ...current.settings, vaultPath },
      }));
      await loadVaultNote(vaultPath, vaultNoteDate);
      setMessage(`Vault linked at ${vaultPath}`);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not link the vault."));
    }
  }

  async function loadVaultNotes(vaultPath = state.settings.vaultPath) {
    if (!vaultPath) return;

    setVaultNotesLoading(true);
    try {
      setVaultNotes(await listNotes(vaultPath));
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not load notes."));
    } finally {
      setVaultNotesLoading(false);
    }
  }

  async function openVaultNote(note: VaultNoteFile) {
    if (!state.settings.vaultPath) return;

    setVaultNotesLoading(true);
    try {
      const content = await readNote(state.settings.vaultPath, note.title);
      setVaultNoteTitle(note.title);
      setVaultNoteContent(stripMarkdownFrontmatter(content ?? ""));
      setVaultNotePath(note.path);
      setVaultNoteSavedAt(note.savedAt);
      setVaultNoteDirty(false);
      setVaultDailyEditing(false);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not open note."));
    } finally {
      setVaultNotesLoading(false);
    }
  }

  function startNewVaultNote() {
    setVaultNoteTitle("Untitled note");
    setVaultNoteContent("");
    setVaultNotePath(null);
    setVaultNoteSavedAt(null);
    setVaultNoteDirty(false);
    setVaultDailyEditing(true);
  }

  async function handleSaveWabiVaultNote() {
    if (!state.settings.vaultPath) return;
    const title = vaultNoteTitle.trim();
    if (!title) {
      setMessage("Give the note a title before saving.");
      return;
    }

    setVaultNotesLoading(true);
    try {
      const notePath = await writeNote(state.settings.vaultPath, title, vaultNoteContent);
      const savedAt = Date.now();
      setVaultNotePath(notePath);
      setVaultNoteSavedAt(savedAt);
      setVaultNoteDirty(false);
      setVaultDailyEditing(false);
      await loadVaultNotes(state.settings.vaultPath);
      setMessage(`Saved ${title}.`);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not save note."));
    } finally {
      setVaultNotesLoading(false);
    }
  }

  async function handleDeleteWabiVaultNote() {
    if (!state.settings.vaultPath || !vaultNotePath || !vaultNoteTitle) return;
    if (!window.confirm(`Delete "${vaultNoteTitle}"? This cannot be undone.`)) return;

    setVaultNotesLoading(true);
    try {
      await deleteNote(state.settings.vaultPath, vaultNoteTitle);
      const remaining = (await listNotes(state.settings.vaultPath)).filter((note) => note.title !== vaultNoteTitle);
      setVaultNotes(remaining);
      const nextNote = remaining[0];
      if (nextNote) {
        await openVaultNote(nextNote);
      } else {
        startNewVaultNote();
        setVaultDailyEditing(false);
      }
      setMessage(`Deleted ${vaultNoteTitle}.`);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not delete note."));
    } finally {
      setVaultNotesLoading(false);
    }
  }

  async function loadVaultNote(vaultPath = state.settings.vaultPath, noteDate = vaultNoteDate) {
    if (!vaultPath) {
      setMessage("Create or link an Obsidian vault first.");
      return;
    }

    setVaultNoteLoading(true);
    try {
      const existing = await readDailyNote(vaultPath, noteDate);
      setVaultNoteContent(existing ? stripMarkdownFrontmatter(existing) : buildDailyNoteMarkdown(state, noteDate));
      setVaultNotePath(`${vaultPath}/Daily/${noteDate}.md`);
      setVaultNoteDirty(false);
      setVaultDailyEditing(false);
      setMessage(existing ? `Loaded ${noteDate}.md` : `Created an unsaved draft for ${noteDate}.`);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not load the daily note."));
    } finally {
      setVaultNoteLoading(false);
    }
  }

  async function handleSaveVaultNote() {
    if (!state.settings.vaultPath) {
      setMessage("Create or link an Obsidian vault first.");
      return;
    }

    setVaultNoteLoading(true);
    try {
      const notePath = await writeDailyNote(state.settings.vaultPath, vaultNoteDate, vaultNoteContent || notePreview);
      setVaultNotePath(notePath);
      setVaultNoteDirty(false);
      setVaultDailyEditing(false);
      setState((current) => ({
        ...current,
        exports: [
          {
            id: makeId(),
            exportedAt: new Date().toISOString(),
            noteDate: vaultNoteDate,
            notePath,
          },
          ...current.exports,
        ].slice(0, 20),
      }));
      setMessage(`Saved daily note to ${notePath}`);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not save the daily note."));
    } finally {
      setVaultNoteLoading(false);
    }
  }

  async function loadReferenceNote(
    vaultPath = state.settings.vaultPath,
    semester = selectedReferenceSemester,
    course = selectedReferenceCourse,
    options: { silent?: boolean } = {},
  ) {
    if (!vaultPath) {
      setMessage("Create or link an Obsidian vault first.");
      return;
    }
    if (!semester || !course) {
      setMessage("Choose a semester and course first.");
      return;
    }

    const requestId = referenceLoadRequestRef.current + 1;
    referenceLoadRequestRef.current = requestId;
    setReferenceLoading(true);
    try {
      const existing = await readReferenceNote(vaultPath, semester.name, course.name);
      if (referenceLoadRequestRef.current !== requestId) return;
      setReferenceContent(existing ? stripMarkdownFrontmatter(existing) : buildReferenceNoteMarkdown(course));
      setReferencePath(`${vaultPath}/References/${semester.name}/${course.name}.md`);
      setReferenceDirty(false);
      setReferenceEditing(false);
      if (!options.silent) setMessage(existing ? `Loaded references for ${course.name}.` : `Created an unsaved references draft for ${course.name}.`);
    } catch (error) {
      if (referenceLoadRequestRef.current !== requestId) return;
      setMessage(getErrorMessage(error, "Could not load course references."));
    } finally {
      if (referenceLoadRequestRef.current === requestId) setReferenceLoading(false);
    }
  }

  async function handleSaveReferenceNote() {
    if (!state.settings.vaultPath) {
      setMessage("Create or link an Obsidian vault first.");
      return;
    }
    if (!selectedReferenceSemester || !selectedReferenceCourse) {
      setMessage("Choose a semester and course first.");
      return;
    }

    setReferenceLoading(true);
    try {
      const content = referenceContent || buildReferenceNoteMarkdown(selectedReferenceCourse);
      const notePath = await writeReferenceNote(
        state.settings.vaultPath,
        selectedReferenceSemester.name,
        selectedReferenceCourse.name,
        content,
      );
      setReferenceContent(content);
      setReferencePath(notePath);
      setReferenceDirty(false);
      setReferenceEditing(false);
      setMessage(`Saved references to ${notePath}`);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not save course references."));
    } finally {
      setReferenceLoading(false);
    }
  }

  function handleEditReferenceNote() {
    if (!selectedReferenceSemester || !selectedReferenceCourse) {
      setMessage("Choose a semester and course first.");
      return;
    }
    setReferenceContent((current) => current || buildReferenceNoteMarkdown(selectedReferenceCourse));
    setReferenceEditing(true);
  }

  async function loadSummaryFileList(
    vaultPath = state.settings.vaultPath,
    semester = selectedSummarySemester,
    course = selectedSummaryCourse,
    options: { silent?: boolean } = {},
  ) {
    if (!vaultPath) {
      setMessage("Create or link an Obsidian vault first.");
      return;
    }
    if (!semester || !course) {
      setMessage("Choose a semester and course first.");
      return;
    }

    const requestId = summaryLoadRequestRef.current + 1;
    summaryLoadRequestRef.current = requestId;
    setSummaryLoading(true);
    try {
      const files = await listSummaryFiles(vaultPath, semester.name, course.name);
      if (summaryLoadRequestRef.current !== requestId) return;
      setSummaryFiles(files);
      setSelectedSummaryPath((current) => (current && files.some((file) => file.path === current) ? current : files[0]?.path ?? null));
      if (!options.silent) setMessage(files.length ? `Loaded ${files.length} summary file${files.length === 1 ? "" : "s"}.` : `No summaries found for ${course.name}.`);
    } catch (error) {
      if (summaryLoadRequestRef.current !== requestId) return;
      setMessage(getErrorMessage(error, "Could not load summaries."));
    } finally {
      if (summaryLoadRequestRef.current === requestId) setSummaryLoading(false);
    }
  }

  async function handleAddSummaryFiles() {
    if (!state.settings.vaultPath) {
      setMessage("Create or link an Obsidian vault first.");
      return;
    }
    if (!selectedSummarySemester || !selectedSummaryCourse) {
      setMessage("Choose a semester and course first.");
      return;
    }
    if (!isTauriApp()) {
      setMessage("Adding summary files works inside the desktop build.");
      return;
    }

    const selected = await pickSummaryFiles();
    if (!selected.length) return;

    setSummaryLoading(true);
    try {
      const files = await importSummaryFiles(
        state.settings.vaultPath,
        selectedSummarySemester.name,
        selectedSummaryCourse.name,
        selected,
      );
      setSummaryFiles(files);
      const imported = new Set(selected.map((path) => path.split(/[\\/]/).pop()));
      const firstImported = files.find((file) => imported.has(file.name));
      setSelectedSummaryPath(firstImported?.path ?? files[0]?.path ?? null);
      setMessage(`Added ${selected.length} file${selected.length === 1 ? "" : "s"} to Summaries.`);
    } catch (error) {
      setMessage(getErrorMessage(error, "Could not add summary files."));
    } finally {
      setSummaryLoading(false);
    }
  }

  function handleUseGeneratedNote() {
    setVaultNoteContent(notePreview);
    setVaultNoteDirty(true);
    setVaultDailyEditing(true);
    setMessage("Session draft copied into the editor. Save it when you are ready.");
  }

  function healthClass(score: number) {
    if (score >= 75) return "strong";
    if (score >= 55) return "steady";
    if (score >= 35) return "watch";
    return "critical";
  }

  function focusTaskFromDashboard(task: Task) {
    setSelectedTaskId(task.id);
    setState((current) => ({
      ...current,
      activeTab: "timer",
      timer: {
        ...current.timer,
        semesterId: task.semesterId,
        courseId: task.courseId,
        taskId: task.id,
        goal: current.timer.goal || task.title,
      },
    }));
    setMessage(`"${task.title}" sent to timer.`);
  }

  function selectTaskForNow(task: Task) {
    setSelectedTaskId(task.id);
    setState((current) => ({
      ...current,
      timer: {
        ...current.timer,
        semesterId: task.semesterId,
        courseId: task.courseId,
        taskId: task.id,
        goal: task.title,
      },
    }));
    setMessage(`"${task.title}" selected for now.`);
  }

  function focusTaskInQuietMode(task: Task) {
    setSelectedTaskId(task.id);
    setWabiQuietTaskId(task.id);
    setWabiQuietTaskMenuOpen(false);
    setState((current) => ({
      ...current,
      timer: {
        ...current.timer,
        semesterId: task.semesterId,
        courseId: task.courseId,
        taskId: task.id,
        goal: task.title,
      },
    }));
  }

  const getCalendarEntryTask = (entry: CalendarEntry) => taskLookup.get(entry.taskId) ?? null;
  const getCalendarEntryTitle = (entry: CalendarEntry) => getCalendarEntryTask(entry)?.title ?? entry.adHocTitle ?? "Calendar task";
  const getCalendarEntryUnitStart = (entry: CalendarEntry, task: Task) => {
    if (typeof entry.unitStart === "number" && Number.isFinite(entry.unitStart)) {
      return clamp(Math.floor(entry.unitStart), 1, task.totalUnits);
    }

    const taskEntries = state.calendarEntries
      .filter((item) => item.taskId === task.id)
      .sort(compareCalendarEntriesForUnits);
    const sequenceBase = clamp(task.completedUnits - getCompletedCalendarWholeUnits(state.calendarEntries, task.id), 0, task.totalUnits);
    let amountBefore = 0;

    for (const item of taskEntries) {
      if (item.id === entry.id) break;
      amountBefore += getCalendarEntryAmount(item);
    }

    return clamp(Math.floor(sequenceBase + amountBefore) + 1, 1, task.totalUnits);
  };
  const getCalendarEntryUnitLabel = (entry: CalendarEntry) => {
    const task = getCalendarEntryTask(entry);
    if (!task) return formatUnitAmount(getCalendarEntryAmount(entry));
    return entry.unitAmount === 1
      ? `${cleanUnitLabel(task.unitLabel, task.title)} ${getCalendarEntryUnitStart(entry, task)} of ${task.totalUnits}`
      : `${formatUnitAmount(entry.unitAmount)} ${formatUnitLabel(task.unitLabel, 2)}`;
  };
  const getCalendarEntryCourse = (entry: CalendarEntry) => {
    const task = getCalendarEntryTask(entry);
    return task ? courseLookup.get(task.courseId) ?? null : entry.adHocCourseId ? courseLookup.get(entry.adHocCourseId) ?? null : null;
  };
  const getCalendarEntrySemester = (entry: CalendarEntry) => {
    const task = getCalendarEntryTask(entry);
    return task ? semesterLookup.get(task.semesterId) ?? null : entry.adHocSemesterId ? semesterLookup.get(entry.adHocSemesterId) ?? null : null;
  };

  function renderCalendarTimelineEntry(entry: CalendarEntry) {
    const task = getCalendarEntryTask(entry);
    const course = getCalendarEntryCourse(entry);
    const semester = getCalendarEntrySemester(entry);
    const isEditing = calendarEditEntryId === entry.id;
    const isDragging = calendarDragEntryId === entry.id;
    const isResizing = calendarResizeEntryId === entry.id;
    const entryStartMinutes = entry.startTime ? timeToMinutes(entry.startTime) : calendarTimelineStartMinutes;
    const entryEndMinutes = entry.endTime ? timeToMinutes(entry.endTime) : entryStartMinutes + 60;
    const entryTop = ((entryStartMinutes - calendarTimelineStartMinutes) / calendarTimelineTotalMinutes) * calendarTimelineHeight;
    const entryHeight = Math.max(58, ((entryEndMinutes - entryStartMinutes) / calendarTimelineTotalMinutes) * calendarTimelineHeight - 8);
    const entryStyle = {
      "--entry-color": course?.color ?? "var(--accent)",
      ...(entry.startTime ? { top: `${Math.max(4, entryTop + 4)}px`, height: `${entryHeight}px` } : {}),
    } as CSSProperties;
    return (
      <div
        key={entry.id}
        className={`calendar-timeline-entry ${entry.startTime ? "scheduled" : "unscheduled"} ${entry.completed ? "done" : ""} ${isDragging ? "dragging" : ""} ${isResizing ? "resizing" : ""}`}
        onMouseDown={(event) => !isEditing && startCalendarEntryMove(event, entry)}
        style={entryStyle}
      >
        {isEditing ? (
          <div className="calendar-edit-inline">
            <label>
              <span>Start</span>
              <input
                type="text"
                inputMode="numeric"
                pattern="[0-2][0-9]:[0-5][0-9]"
                placeholder="09:00"
                value={calendarEditDraft.startTime}
                onChange={(event) => setCalendarEditDraft((current) => ({ ...current, startTime: event.target.value }))}
              />
            </label>
            <label>
              <span>End</span>
              <input
                type="text"
                inputMode="numeric"
                pattern="[0-2][0-9]:[0-5][0-9]"
                placeholder="10:00"
                value={calendarEditDraft.endTime}
                onChange={(event) => setCalendarEditDraft((current) => ({ ...current, endTime: event.target.value }))}
              />
            </label>
            <button type="button" className="calendar-primary-button" onClick={() => saveCalendarEntryEdit(entry.id)}>
              Save
            </button>
            <button type="button" className="ghost-button small-button" onClick={() => setCalendarEditEntryId(null)}>
              Cancel
            </button>
            <button type="button" className="ghost-button small-button" onClick={() => unscheduleCalendarEntry(entry.id)}>
              Unschedule
            </button>
          </div>
        ) : (
          <>
            <div className="calendar-timeline-entry-copy">
              <strong>{getCalendarEntryTitle(entry)}</strong>
              <span>{getCalendarEntryUnitLabel(entry)} · {course?.name ?? "General"}{semester ? ` · ${semester.name}` : ""}</span>
              <small>{formatTimeRange(entry)}{task ? "" : " · calendar only"}</small>
            </div>
            {entry.startTime ? (
              <>
                <input className="calendar-timeline-checkbox" type="checkbox" checked={entry.completed} onChange={() => toggleCalendarEntry(entry.id)} title="Mark scheduled unit done" />
                <div className="calendar-timeline-entry-actions">
                  <button type="button" className="ghost-button small-button" onClick={() => startCalendarEntryEdit(entry)}>
                    Edit
                  </button>
                  <button type="button" className="ghost-button small-button" disabled={entry.completed} onClick={() => removeCalendarEntry(entry.id)}>
                    Remove
                  </button>
                </div>
              </>
            ) : (
              <div className="calendar-timeline-entry-footer">
                <input className="calendar-timeline-checkbox" type="checkbox" checked={entry.completed} onChange={() => toggleCalendarEntry(entry.id)} title="Mark scheduled unit done" />
                <div className="calendar-timeline-entry-actions">
                  <button type="button" className="ghost-button small-button" onClick={() => startCalendarEntryEdit(entry)}>
                    Edit
                  </button>
                  <button type="button" className="ghost-button small-button" disabled={entry.completed} onClick={() => removeCalendarEntry(entry.id)}>
                    Remove
                  </button>
                </div>
              </div>
            )}
            {entry.startTime ? (
              <div className="calendar-resize-handle" onMouseDown={(event) => startCalendarEntryResize(event, entry)} title="Drag to change duration" />
            ) : null}
          </>
        )}
      </div>
    );
  }

  function renderCalendarDayOverlay() {
    const selectedDate = selectedCalendarDate ? parseCalendarDate(selectedCalendarDate) : null;
    if (!selectedCalendarDate || !selectedDate) return null;

    const selectedEntries = selectedCalendarDate ? (calendarEntriesByDate.get(selectedCalendarDate) ?? []) : [];
    const selectedExams = selectedCalendarDate ? (examsByDate.get(selectedCalendarDate) ?? []) : [];
    const selectedDeadlines = selectedCalendarDate ? (deadlinesByDate.get(selectedCalendarDate) ?? []) : [];
    const timedEntries = selectedEntries
      .filter((entry) => entry.startTime)
      .sort((a, b) => (a.startTime ?? "").localeCompare(b.startTime ?? ""));
    const unscheduledEntries = selectedEntries.filter((entry) => !entry.startTime);

    return (
      <div className="calendar-drawer-backdrop calendar-day-view-backdrop" onMouseDown={closeCalendarDrawer}>
        <aside className="calendar-day-view" onMouseDown={(event) => event.stopPropagation()} aria-label="Calendar day view">
          <div className="calendar-day-view-head">
            <div>
              <p className="eyebrow">Day view</p>
              <h3>{new Intl.DateTimeFormat("en", { weekday: "long", month: "long", day: "numeric", year: "numeric" }).format(selectedDate)}</h3>
              <p className="section-note">Click + to assign planner tasks or create calendar-only tasks.</p>
            </div>
            <div className="calendar-day-view-actions">
              <button type="button" className="calendar-primary-button" onClick={openCalendarAddDrawer}>
                + Add task
              </button>
              <button type="button" className="ghost-button small-button" onClick={closeCalendarDrawer}>
                Done
              </button>
            </div>
          </div>

          <div className="calendar-day-view-body">
            <div className="calendar-timeline">
              <div
                ref={calendarTimelineRef}
                className={`calendar-timeline-canvas ${calendarMovePreview ? "drag-over" : ""}`}
                style={{ height: `${calendarTimelineHeight}px` }}
              >
                {calendarTimelineHours.map((hour) => (
                  <div key={hour} className="calendar-timeline-hour" style={{ height: `${calendarTimelineHourHeight}px` }}>
                    <div className="calendar-timeline-time">{String(hour).padStart(2, "0")}:00</div>
                  </div>
                ))}
                {calendarMovePreview ? (
                  <div className="calendar-drop-guide" style={{ top: `${calendarMovePreview.top}px` }}>
                    <span>{calendarMovePreview.time}</span>
                  </div>
                ) : null}
                {timedEntries.map(renderCalendarTimelineEntry)}
              </div>
            </div>

            <div className="calendar-day-side-panel">
              <section className="calendar-drawer-section">
                <div>
                  <strong>Unscheduled</strong>
                  <small>Entries without a time stay here.</small>
                </div>
                <div className="calendar-expanded-list drawer-list">
                  {unscheduledEntries.length ? unscheduledEntries.map(renderCalendarTimelineEntry) : (
                    <p className="empty-copy compact-empty">No unscheduled calendar tasks.</p>
                  )}
                </div>
              </section>

              {selectedExams.length || selectedDeadlines.length ? (
                <section className="calendar-drawer-section">
                  <div>
                    <strong>Fixed markers</strong>
                    <small>Exams and deadlines are pulled from the planner.</small>
                  </div>
                  <div className="calendar-expanded-list drawer-list">
                    {selectedExams.map((exam) => (
                      <div key={exam.id} className="calendar-marker-row exam-marker">
                        <strong>Exam</strong>
                        <span>{exam.title} · {courseLookup.get(exam.courseId)?.name ?? "No course"}</span>
                      </div>
                    ))}
                    {selectedDeadlines.map((task) => (
                      <div key={task.id} className="calendar-marker-row deadline-marker">
                        <strong>Deadline</strong>
                        <span>{task.title} · {courseLookup.get(task.courseId)?.name ?? "No course"}</span>
                      </div>
                    ))}
                  </div>
                </section>
              ) : null}

              <section className="calendar-drawer-section">
                <div>
                  <strong>Day controls</strong>
                  <small>Move through days or add a new calendar block.</small>
                </div>
                <div className="calendar-day-side-actions">
                  <button type="button" className="ghost-button" onClick={openNextCalendarDay}>
                    Next day
                  </button>
                  <button type="button" className="calendar-primary-button" onClick={openCalendarAddDrawer}>
                    + Add task
                  </button>
                </div>
              </section>
            </div>

            {calendarAddOpen ? (
              <aside className="calendar-drawer calendar-add-drawer" aria-label="Add calendar task">
                <div className="calendar-drawer-head">
                  <div>
                    <p className="eyebrow">Add task</p>
                    <h3>{calendarAddDraft.noTime ? "Add today's TODO" : "Plan a time block"}</h3>
                  </div>
                  <button type="button" className="ghost-button small-button" onClick={closeCalendarAddDrawer}>
                    Close
                  </button>
                </div>

                <div className="calendar-add-form">
                  <label>
                    <span>Semester</span>
                    <select
                      value={calendarAddDraft.semesterId}
                      onChange={(event) => setCalendarAddDraft((current) => ({ ...current, semesterId: event.target.value, courseId: "", taskId: "" }))}
                    >
                      <option value="">No semester</option>
                      {state.semesters.map((semester) => (
                        <option key={semester.id} value={semester.id}>{semester.name}</option>
                      ))}
                    </select>
                  </label>

                  <label>
                    <span>Subject</span>
                    <select
                      value={calendarAddDraft.courseId}
                      onChange={(event) => setCalendarAddDraft((current) => ({ ...current, courseId: event.target.value, taskId: "" }))}
                    >
                      <option value="">General</option>
                      {calendarAddCourses.map((course) => (
                        <option key={course.id} value={course.id}>{course.name}</option>
                      ))}
                    </select>
                  </label>

                  <div className="calendar-source-toggle" aria-label="Task source">
                    {(["planner", "new"] as CalendarTaskSource[]).map((source) => (
                      <button
                        key={source}
                        type="button"
                        className={calendarAddDraft.source === source ? "active" : ""}
                        onClick={() => setCalendarAddDraft((current) => ({ ...current, source }))}
                      >
                        {source === "planner" ? "From planner" : "New task"}
                      </button>
                    ))}
                  </div>

                  {calendarAddDraft.source === "planner" ? (
                    <label>
                      <span>Planner task</span>
                      <select
                        value={calendarAddDraft.taskId}
                        onChange={(event) => setCalendarAddDraft((current) => ({ ...current, taskId: event.target.value }))}
                      >
                        <option value="">Pick a task</option>
                        {calendarAddTasks.map((task) => (
                          <option key={task.id} value={task.id}>{task.title}</option>
                        ))}
                      </select>
                    </label>
                  ) : (
                    <label>
                      <span>New calendar task</span>
                      <input
                        value={calendarAddDraft.title}
                        onChange={(event) => setCalendarAddDraft((current) => ({ ...current, title: event.target.value }))}
                        placeholder="Watch Youtube video"
                      />
                    </label>
                  )}

                  <div className="calendar-unit-toggle" aria-label="Calendar unit amount">
                    {([1, 0.5, 0.25] as CalendarUnitAmount[]).map((amount) => (
                      <button
                        key={amount}
                        type="button"
                        className={calendarAddDraft.unitAmount === amount ? "active" : ""}
                        onClick={() => setCalendarAddDraft((current) => ({ ...current, unitAmount: amount }))}
                      >
                        {amount === 1 ? "1" : amount === 0.5 ? "1/2" : "1/4"}
                      </button>
                    ))}
                  </div>

                  {calendarAddDraft.noTime ? (
                    <div className="calendar-schedule-panel">
                      <p className="calendar-time-preview">This task will appear in Unscheduled.</p>
                      <button type="button" className="ghost-button" onClick={() => setCalendarAddDraft((current) => ({ ...current, noTime: false }))}>
                        Schedule
                      </button>
                    </div>
                  ) : (
                    <>
                      <div className="calendar-time-form-grid">
                        <label>
                          <span>Start</span>
                          <input
                            type="text"
                            inputMode="numeric"
                            pattern="[0-2][0-9]:[0-5][0-9]"
                            placeholder="09:00"
                            value={calendarAddDraft.startTime}
                            onChange={(event) => setCalendarAddDraft((current) => ({ ...current, startTime: event.target.value }))}
                          />
                        </label>
                        <label>
                          <span>Duration</span>
                          <select
                            value={calendarAddDraft.durationMinutes}
                            onChange={(event) => setCalendarAddDraft((current) => ({ ...current, durationMinutes: event.target.value }))}
                          >
                            {calendarDurationOptions.map((minutes) => (
                              <option key={minutes} value={minutes}>{minutes < 60 ? `${minutes} min` : `${minutes / 60} h`}</option>
                            ))}
                          </select>
                        </label>
                      </div>

                      <div className="calendar-schedule-panel scheduled">
                        <p className="calendar-time-preview">
                          {calendarAddDraft.startTime} - {addMinutesToTime(calendarAddDraft.startTime, Number(calendarAddDraft.durationMinutes) || 60)}
                        </p>
                        <button type="button" className="ghost-button" onClick={() => setCalendarAddDraft((current) => ({ ...current, noTime: true }))}>
                          Unschedule
                        </button>
                </div>
              </>
            )}
          </div>

                <div className="calendar-drawer-actions">
                  <button type="button" className="ghost-button" onClick={closeCalendarAddDrawer}>
                    Cancel
                  </button>
                  <button type="button" className="calendar-primary-button" onClick={addCalendarEntryFromDrawer}>
                    Add to calendar
                  </button>
                </div>
              </aside>
            ) : null}
            {calendarMovePreview ? (() => {
              const previewEntry = selectedEntries.find((entry) => entry.id === calendarMovePreview.entryId);
              return (
                <div className="calendar-drag-preview" style={{ left: calendarMovePreview.x + 14, top: calendarMovePreview.y + 14 }}>
                  <strong>{previewEntry ? getCalendarEntryTitle(previewEntry) : "Calendar task"}</strong>
                  <span>{calendarMovePreview.time}</span>
                </div>
              );
            })() : null}
          </div>
        </aside>
      </div>
    );
  }

  function renderPlannerCalendar() {
    const weekdayLabels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

    return (
      <article className="panel-card planner-calendar-card" data-tour="planner-calendar">
        <div className="section-head planner-calendar-head">
          <div>
            <p className="eyebrow">Calendar</p>
            <h2>{calendarTitle}</h2>
            <p className="section-note">Plan one task unit at a time. Exams and task deadlines appear automatically.</p>
          </div>
          <div className="planner-calendar-controls">
            <div className="calendar-view-toggle" aria-label="Calendar view">
              {(["month", "week"] as CalendarView[]).map((view) => (
                <button
                  key={view}
                  type="button"
                  className={calendarView === view ? "active" : ""}
                  onClick={() => setPlannerCalendarView(view)}
                >
                  {view === "month" ? "Month" : "Week"}
                </button>
              ))}
            </div>
            <div className="calendar-nav-buttons">
              <button type="button" className="ghost-button small-button" onClick={() => shiftCalendar(-1)}>
                Prev
              </button>
              <button type="button" className="ghost-button small-button" onClick={jumpCalendarToToday}>
                Today
              </button>
              <button type="button" className="ghost-button small-button" onClick={() => shiftCalendar(1)}>
                Next
              </button>
            </div>
          </div>
        </div>

        <div className="calendar-weekday-row" aria-hidden="true">
          {weekdayLabels.map((label) => (
            <span key={label}>{label}</span>
          ))}
        </div>

        <div className={`calendar-grid ${calendarView === "week" ? "week-view" : "month-view"}`}>
          {calendarDays.map((day) => {
            const entries = calendarEntriesByDate.get(day.iso) ?? [];
            const exams = examsByDate.get(day.iso) ?? [];
            const deadlines = deadlinesByDate.get(day.iso) ?? [];
            const isSelected = selectedCalendarDate === day.iso;
            const isToday = day.iso === calendarToday;
            const fieldNotebookWeek = appStyle === "field-notebook" && calendarView === "week";
            const wabiWeek = appStyle === "wabi-sabi" && calendarView === "week";
            const visibleEntryLimit = wabiWeek ? entries.length : fieldNotebookWeek ? 8 : 3;
            const visibleExamLimit = wabiWeek ? exams.length : fieldNotebookWeek ? 4 : 2;
            const visibleDeadlineLimit = wabiWeek ? deadlines.length : fieldNotebookWeek ? 4 : 2;
            const visibleItemCount = Math.min(entries.length, visibleEntryLimit) + Math.min(exams.length, visibleExamLimit) + Math.min(deadlines.length, visibleDeadlineLimit);
            const hiddenItemCount = entries.length + exams.length + deadlines.length - visibleItemCount;

            return (
              <section
                key={day.iso}
                role="button"
                tabIndex={0}
                className={`calendar-day-card ${day.inCurrentMonth ? "" : "muted-month"} ${isToday ? "today" : ""} ${isSelected ? "selected" : ""}`}
                onClick={() => openCalendarDrawer(day.iso)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    openCalendarDrawer(day.iso);
                  }
                }}
              >
                <div className="calendar-day-top">
                  <span>{day.date.getDate()}</span>
                  <small>{new Intl.DateTimeFormat("en", { month: "short", year: "numeric" }).format(day.date)}</small>
                </div>

                <div className="calendar-day-items">
                  {entries.slice(0, visibleEntryLimit).map((entry) => {
                    const course = getCalendarEntryCourse(entry);
                    return (
                      <div key={entry.id} className={`calendar-pill task-pill ${entry.completed ? "completed" : ""}`} style={{ "--pill-color": course?.color ?? "var(--accent)" } as CSSProperties}>
                        <input
                          className="calendar-task-checkbox"
                          type="checkbox"
                          checked={entry.completed}
                          onChange={() => toggleCalendarEntry(entry.id)}
                          onClick={(event) => event.stopPropagation()}
                        />
                        <span className="calendar-task-copy">
                          <span className="calendar-task-title">{getCalendarEntryTitle(entry)}</span>
                          <span className="calendar-task-course">{getCalendarEntryUnitLabel(entry)} · {course?.name ?? "No course"}</span>
                        </span>
                      </div>
                    );
                  })}
                  {exams.slice(0, visibleExamLimit).map((exam) => {
                    const course = courseLookup.get(exam.courseId);
                    return (
                      <div key={exam.id} className="calendar-pill exam-pill" style={{ "--pill-color": course?.color ?? "var(--danger)" } as CSSProperties}>
                        <span>Exam: {exam.title}</span>
                      </div>
                    );
                  })}
                  {deadlines.slice(0, visibleDeadlineLimit).map((task) => {
                    const course = courseLookup.get(task.courseId);
                    return (
                      <div key={task.id} className="calendar-pill deadline-pill" style={{ "--pill-color": course?.color ?? "var(--warn)" } as CSSProperties}>
                        <span>Due: {task.title}</span>
                      </div>
                    );
                  })}
                  {hiddenItemCount > 0 ? (
                    <span className="calendar-more">+{hiddenItemCount} more</span>
                  ) : null}
                </div>
              </section>
            );
          })}
        </div>
      </article>
    );
  }
  function renderTodayCard() {
    const todayLabel = new Intl.DateTimeFormat("en", { weekday: "long", day: "numeric", month: "long" }).format(new Date(`${calendarToday}T00:00:00`));
    const dailyGoalMinutes = Math.max(1, state.settings.dailyGoalMinutes ?? 120);
    const goalProgress = clamp(Math.round((todayMinutes / dailyGoalMinutes) * 100), 0, 100);
    return (
      <article className="panel-card design-card design-today-card" data-tour="dashboard-today">
        <div className="design-today-top">
          <div className="design-today-focus-stat">
            <p className="eyebrow">Today · {todayLabel}</p>
            <strong className="design-big-stat">{formatMinutes(todayMinutes)}</strong>
            <span>focused</span>
          </div>
          <div className="design-icon-badge">⌖</div>
        </div>

        <div className="design-mini-metrics">
          <div>
            <span>Units done</span>
            <strong>{formatCompletedUnits(unitsCompletedToday)}</strong>
          </div>
          <div>
            <span>Day streak</span>
            <strong>{streakDays}</strong>
          </div>
          <div>
            <span>Momentum</span>
            <strong>{focusMomentum}</strong>
          </div>
        </div>

        <div className="design-goal-meter">
          <div className="design-goal-head">
            <span>Daily goal</span>
            <strong>{formatMinutes(todayMinutes)} / {formatMinutes(dailyGoalMinutes)}</strong>
          </div>
          <div className="design-goal-track" aria-label={`Daily goal ${goalProgress}% complete`}>
            <span style={{ width: `${goalProgress}%` }} />
          </div>
          <small>{goalProgress}% complete</small>
        </div>
      </article>
    );
  }

  function renderWeeklyChart(heightClass = "") {
    const { days, milestoneHits, activeDays, biggestDay, maxDayMinutes, activeCourses, achievedMilestones, latestMilestone, visibleMinutes } = focusTimeline;
    const hasData = totalAllTimeMinutes > 0;
    const hoveredDay = focusTip ? days[focusTip.idx] : null;
    const columnGap = focusRange === 365 ? 0 : focusRange === "week" || focusRange === 7 ? 4 : focusRange <= 14 ? 3 : focusRange <= 30 ? 2 : 1;
    const chartHeightByRange: Record<FocusRange, number> = {
      week: 132,
      7: 132,
      14: 124,
      30: 116,
      60: 124,
      365: 124,
    };
    const maxHeight = heightClass.includes("short-weekly") ? 105 : chartHeightByRange[focusRange];
    const columnMaxHeight = maxHeight - 5;
    const rangeDensityClass = focusRange === "week" ? "range-week" : focusRange <= 14 ? "range-short" : focusRange <= 30 ? "range-medium" : focusRange === 365 ? "range-year" : "range-long";
    const rangeLabel = focusRange === "week" ? "this week" : focusRange === 365 ? "past year" : `last ${focusRange} days`;
    const updateFocusTip = (event: MouseEvent<HTMLDivElement>, idx: number) => {
      const anchor = event.currentTarget.querySelector(".fossil-column")
        ?? event.currentTarget.querySelector(".fossil-eroded-day")
        ?? event.currentTarget;
      const rect = anchor.getBoundingClientRect();
      setFocusTip({ idx, x: rect.left + rect.width / 2, y: rect.top });
    };

    return (
      <article className={`panel-card design-card design-weekly-card fossil-card ${rangeDensityClass} ${heightClass}`} data-tour="dashboard-weekly">
        <div className="fossil-card-grain" aria-hidden="true" />

        <div className="fossil-head">
          <div>
            <p className="fossil-eyebrow"><StrataIcon size={13} /> Focus history</p>
            <h3>Weekly focus</h3>
          </div>
          <div className="fossil-total">
            <strong>{formatMinutes(visibleMinutes)}</strong>
            <span>{rangeLabel}</span>
          </div>
        </div>

        <div className="fossil-range-toggle" aria-label="Weekly Focus range">
          {(["week", 7, 14, 30, 60, 365] as FocusRange[]).map((range) => (
            <button
              key={range}
              type="button"
              className={range === focusRange ? "active" : ""}
              onClick={() => {
                setFocusRange(range);
                setFocusTip(null);
                setActiveFocusMilestone(null);
              }}
            >
              {range === "week" ? "This week" : range === 365 ? "1y" : `${range}d`}
            </button>
          ))}
        </div>

        {hasData ? (
          <>
            <div className="fossil-strata-shell" style={{ "--focus-day-count": days.length, "--focus-col-gap": `${columnGap}px` } as CSSProperties}>
              <div className="fossil-depth-lines" aria-hidden="true" />
              <div className="fossil-scroll">
                <div className="fossil-track">
                <div className="fossil-strata" style={{ height: maxHeight }}>
                  {days.map((day, dayIndex) => {
                    const columnHeight = day.totalMinutes > 0 ? Math.max(6, Math.sqrt(day.totalMinutes / maxDayMinutes) * columnMaxHeight) : 0;
                    const isHovered = focusTip?.idx === dayIndex;
                    const hasHover = focusTip !== null;
                    return (
                      <div
                        key={day.date}
                        className={`fossil-day ${day.isToday ? "today" : ""} ${isHovered ? "hovered" : ""} ${hasHover && !isHovered ? "dimmed" : ""}`}
                        onMouseEnter={(event: MouseEvent<HTMLDivElement>) => {
                          updateFocusTip(event, dayIndex);
                        }}
                        onMouseMove={(event: MouseEvent<HTMLDivElement>) => {
                          updateFocusTip(event, dayIndex);
                        }}
                        onMouseLeave={() => setFocusTip(null)}
                      >
                        {day.totalMinutes === 0 ? (
                          <div className="fossil-eroded-day" />
                        ) : (
                          <div className="fossil-column" style={{ height: columnHeight }} title={`${fossilDateLabel(day.date)}: ${formatMinutes(day.totalMinutes)}`}>
                            {day.layers.map((layer, layerIndex) => {
                              const layerHeight = Math.max(4, (layer.minutes / day.totalMinutes) * columnHeight - 1);
                              const seed = dayIndex * 97 + layerIndex * 31;
                              return (
                                <div
                                  key={layer.id}
                                  className="fossil-layer"
                                  style={{
                                    "--layer-color": layer.color,
                                    "--layer-height": `${layerHeight}px`,
                                    "--layer-width": `${82 + fossilRand(seed) * 18}%`,
                                    "--layer-radius-a": `${1.5 + fossilRand(seed + 1) * 4}px`,
                                    "--layer-radius-b": `${1.5 + fossilRand(seed + 2) * 4}px`,
                                    "--layer-radius-c": `${0.5 + fossilRand(seed + 3) * 2.5}px`,
                                    "--layer-radius-d": `${0.5 + fossilRand(seed + 4) * 2.5}px`,
                                  } as CSSProperties}
                                  title={`${layer.name} · ${formatMinutes(layer.minutes)}`}
                                />
                              );
                            })}
                          </div>
                        )}

                        {day.isToday ? (
                          <span className="fossil-survey-flag" aria-label="Today">
                            <svg width="16" height="22" viewBox="0 0 16 22" fill="none" aria-hidden="true">
                              <line x1="2.5" y1="1" x2="2.5" y2="21" stroke="currentColor" strokeWidth="1.8" opacity="0.5" />
                              <polygon points="3,1 14,5 3,9" fill="currentColor" />
                            </svg>
                          </span>
                        ) : null}
                      </div>
                    );
                  })}
                </div>
                <div className="fossil-bedrock" />
                {milestoneHits.length ? (
                  <div className="fossil-milestones-row">
                    {days.map((day, dayIndex) => {
                      const milestone = milestoneHits.find((item) => item.dayIndex === dayIndex);
                      if (!milestone) return <span key={day.date} className="fossil-milestone-spacer" />;
                      const active = activeFocusMilestone === milestone.hours;
                      return (
                        <button
                          key={`${day.date}-${milestone.hours}`}
                          type="button"
                          className={`fossil-milestone ${active ? "active" : ""}`}
                          onClick={() => setActiveFocusMilestone(active ? null : milestone.hours)}
                        >
                          <FossilMilestoneIcon hours={milestone.hours} size={13} />
                          <span>{milestone.hours}h</span>
                          {active ? (
                            <span className="fossil-milestone-popover">
                              <strong>{milestone.label}</strong>
                              <small>{milestone.desc}</small>
                            </span>
                          ) : null}
                        </button>
                      );
                    })}
                  </div>
                ) : null}
                <div className="fossil-date-row">
                  {days.map((day, dayIndex) => {
                    const date = new Date(`${day.date}T00:00:00`);
                    const label = focusRange === "week" || focusRange <= 14
                      ? ["S", "M", "T", "W", "T", "F", "S"][date.getDay()]
                      : dayIndex === 0 || date.getDate() === 1
                        ? new Intl.DateTimeFormat("en", { month: "short" }).format(date)
                        : "";
                    return <span key={day.date} className={day.isToday ? "today" : ""}>{label}</span>;
                  })}
                </div>
                </div>
              </div>
            </div>

            {focusTip && hoveredDay ? (
              <div className="fossil-tooltip" style={{ left: focusTip.x, top: focusTip.y - 8 }}>
                <div className="fossil-tooltip-head">
                  <strong>{fossilDateLabel(hoveredDay.date)}</strong>
                  {hoveredDay.isToday ? <span>TODAY</span> : null}
                </div>
                {hoveredDay.totalMinutes > 0 ? (
                  <>
                    <div className="fossil-tooltip-layers">
                      {hoveredDay.layers.map((layer) => (
                        <div key={layer.id}>
                          <span style={{ background: layer.color }} />
                          <em>{layer.name}</em>
                          <strong>{formatMinutes(layer.minutes)}</strong>
                        </div>
                      ))}
                    </div>
                    <div className="fossil-tooltip-total"><span>Total</span><strong>{formatMinutes(hoveredDay.totalMinutes)}</strong></div>
                  </>
                ) : (
                  <p>No study - rest day</p>
                )}
              </div>
            ) : null}

            <div className="fossil-legend">
              <span>Courses</span>
              {activeCourses.map((course) => (
                <div key={course.id}>
                  <i style={{ background: course.color }} />
                  <span>{course.name}</span>
                </div>
              ))}
            </div>

            {achievedMilestones.length ? (
              <div className="fossil-discovered">
                <span>Discovered</span>
                {achievedMilestones.map((milestone) => (
                  <div key={milestone.hours}>
                    <FossilMilestoneIcon hours={milestone.hours} size={11} />
                    <span>{milestone.label}</span>
                  </div>
                ))}
              </div>
            ) : null}

            <div className="fossil-stats">
              <div><span>Active days</span><strong>{activeDays}</strong><em>/ {focusRange === "week" ? "week" : focusRange === 365 ? "1y" : focusRange}</em></div>
              <div><span>Streak</span><strong>{streakDays}</strong><em>days</em></div>
              {biggestDay.totalMinutes > 0 ? <div><span>Biggest day</span><strong>{formatMinutes(biggestDay.totalMinutes)}</strong><em>{formatDate(biggestDay.date)}</em></div> : null}
              {latestMilestone ? <div><span>Latest find</span><strong>{latestMilestone.label}</strong><em>at {latestMilestone.hours}h</em></div> : null}
            </div>
          </>
        ) : (
          <div className="fossil-empty">
            <StrataIcon size={40} />
            <strong>No focus history yet</strong>
            <p>Complete your first study block to begin building your focus history.</p>
          </div>
        )}
      </article>
    );
  }

  function renderGardenCard(heightClass = "") {
    return (
      <article className={`panel-card design-card design-garden-card ${heightClass}`} data-tour="dashboard-garden">
        <div className="section-head compact-headline">
          <div>
            <p className="eyebrow">Knowledge Garden</p>
            <h3>{gardenVariant === "japanese" ? "Japanese Garden of Knowledge" : "Growth from focus"}</h3>
          </div>
          <button
            type="button"
            className="dashboard-todo-button"
            onClick={() => setGardenVariant((variant) => (variant === "japanese" ? "western" : "japanese"))}
          >
            {gardenVariant === "japanese" ? "switch to Garden" : "switch to Japanese Garden"}
          </button>
        </div>
        {gardenVariant === "japanese" ? (
          <JapaneseGardenWidget appState={state} weeklyMinutes={weeklyTotalMinutes} />
        ) : (
          <KnowledgeGardenWidget appState={state} weeklyMinutes={weeklyTotalMinutes} />
        )}
      </article>
    );
  }

  function renderUrgentTasks(limit = 5, options?: { selectedEntryId?: string | null; onSelectEntry?: (entry: CalendarEntry) => void }) {
    const entries = todayCalendarEntries.slice(0, limit);
    return (
      <article className="panel-card design-card design-priority-card" data-tour="dashboard-urgent">
        <div className="section-head compact-headline">
          <div>
            <p className="eyebrow">Do this next</p>
            <h3>Planned today</h3>
            <p className="section-note">From your calendar schedule.</p>
          </div>
          <button type="button" className="dashboard-todo-button" onClick={openTodayTodoDrawer}>
            TODO
          </button>
        </div>

        <div className="design-task-list">
          {entries.length ? (
            entries.map((entry) => {
              const task = taskLookup.get(entry.taskId);
              const course = task ? courseLookup.get(task.courseId) : entry.adHocCourseId ? courseLookup.get(entry.adHocCourseId) : null;
              const semester = task ? semesterLookup.get(task.semesterId) : entry.adHocSemesterId ? semesterLookup.get(entry.adHocSemesterId) : null;
              const title = task?.title ?? entry.adHocTitle ?? "Calendar task";
              const isSelectedEntry = options?.selectedEntryId === entry.id;
              return (
                <div key={entry.id} className={`design-task-item ${task && selectedTaskId === task.id ? "selected" : ""} ${isSelectedEntry ? "fn-selected" : ""}`}>
                  <button type="button" className="design-task-main" onClick={() => {
                    if (options?.onSelectEntry) {
                      options.onSelectEntry(entry);
                      if (task) setSelectedTaskId(task.id);
                    } else if (task) {
                      setSelectedTaskId(task.id);
                    } else {
                      openCalendarDrawer(entry.date);
                    }
                  }}>
                    <span className="design-course-stripe" style={{ background: course?.color ?? "var(--accent)" }} />
                    <span className="design-task-copy">
                      <span className="design-task-title-line">
                        <strong>{title}</strong>
                        <em className={`priority-chip ${entry.completed ? "low" : task?.priority ?? "medium"}`}>{isSelectedEntry ? "selected" : entry.completed ? "done" : formatUnitAmount(getCalendarEntryAmount(entry))}</em>
                      </span>
                      <span className="design-task-meta">
                        <span>{getCalendarEntryUnitLabel(entry)}</span>
                        <span>{course?.name ?? "General"}</span>
                        <span>{semester?.name ?? "No semester"}</span>
                        <span>{task?.dueDate ? `due ${formatDate(task.dueDate)}` : formatTimeRange(entry)}</span>
                      </span>
                    </span>
                  </button>
                  <div className="design-task-side">
                    <input
                      className="dashboard-task-check"
                      type="checkbox"
                      checked={entry.completed}
                      onChange={() => toggleCalendarEntry(entry.id)}
                      title="Mark scheduled unit done"
                    />
                    {task ? (
                      <button type="button" className="ghost-button small-button" data-tour="dashboard-focus-button" onClick={() => focusTaskFromDashboard(task)}>
                        Focus
                      </button>
                    ) : null}
                  </div>
                </div>
              );
            })
          ) : (
            <p className="empty-copy">Nothing planned for today. Add tasks from the planner calendar.</p>
          )}
        </div>
      </article>
    );
  }

  function renderCourseRadar() {
    return (
      <article className="panel-card design-card design-course-radar" data-tour="dashboard-course-radar">
        <div className="section-head compact-headline">
          <div>
            <p className="eyebrow">Course radar</p>
            <h3>Where you stand</h3>
          </div>
        </div>

        <div className="design-course-list">
          {state.courses.length ? (
            state.courses.map((course) => {
              // Non-null: courseHealthByCourseId/courseTasksByCourseId are built by iterating
              // this same state.courses/state.tasks, so every course.id is guaranteed a key.
              const health = courseHealthByCourseId.get(course.id)!;
              const semester = semesterLookup.get(course.semesterId);
              return (
                <div className="design-course-row" key={course.id}>
                  <span className="design-course-dot" style={{ background: course.color }} />
                  <div>
                    <div className="design-course-title-line">
                      <strong>{course.name}</strong>
                      <span className={healthClass(health.score)}>{health.score}</span>
                    </div>
                    <div className="health-track tight">
                      <div className="health-fill" style={{ width: `${health.score}%`, background: course.color }} />
                    </div>
                    <small>
                      {semester?.name ?? "No semester"} • {(courseTasksByCourseId.get(course.id) ?? []).length} tasks • {formatMinutes(courseMinutesByCourseId.get(course.id) ?? 0)} • Target {formatSwissGrade(course.targetGrade)}
                      {health.overdue ? ` • ${health.overdue} overdue` : ""}
                    </small>
                  </div>
                </div>
              );
            })
          ) : (
            <p className="empty-copy">Courses will appear here once you start filling a semester.</p>
          )}
        </div>
      </article>
    );
  }

  function renderExamRunway() {
    return (
      <article className="panel-card design-card design-exam-runway" data-tour="dashboard-exam-runway">
        <div className="section-head compact-headline">
          <div>
            <p className="eyebrow">Exam runway</p>
            <h3>What's coming</h3>
          </div>
        </div>

        <div className="design-exam-list">
          {upcomingExams.length ? (
            upcomingExams.map((exam) => {
              const course = courseLookup.get(exam.courseId);
              const remainingDays = daysUntil(exam.examDate);
              return (
                <div key={exam.id} className="design-exam-item">
                  <div className="design-exam-days">
                    <strong>{remainingDays}</strong>
                    <span>days</span>
                  </div>
                  <div className="design-exam-copy">
                    <strong>{exam.title}</strong>
                    <span>{course?.name ?? "No course"} • {exam.weight}% weight</span>
                  </div>
                  <div className="design-exam-prep">
                    <strong>{exam.preparedness}%</strong>
                    <div className="health-track tight">
                      <div className="health-fill" style={{ width: `${exam.preparedness}%`, background: exam.preparedness >= 70 ? "var(--ok)" : exam.preparedness >= 40 ? "var(--warn)" : "var(--danger)" }} />
                    </div>
                  </div>
                </div>
              );
            })
          ) : (
            <p className="empty-copy">Add exams so the dashboard can see grade pressure early.</p>
          )}
        </div>
      </article>
    );
  }

  function renderStatCard(label: string, value: string | number, detail: string, tone = "") {
    return (
      <article className={`panel-card design-card design-stat-card ${tone}`}>
        <p className="eyebrow">{label}</p>
        <strong>{value}</strong>
        <span>{detail}</span>
      </article>
    );
  }

  function renderStatsWidget() {
    return (
      <article className="panel-card design-card design-stats-widget">
        <div className="section-head compact-headline">
          <div>
            <p className="eyebrow">Snapshot</p>
            <h3>Study health</h3>
          </div>
          <span className={`design-chip ${healthState}`}>{healthLabel}</span>
        </div>
        <div className="design-stats-mini-grid">
          <div>
            <span>Focused today</span>
            <strong>{formatMinutes(todayMinutes)}</strong>
          </div>
          <div>
            <span>This week</span>
            <strong>{formatMinutes(weeklyTotalMinutes)}</strong>
          </div>
          <div>
            <span>Streak</span>
            <strong>{streakDays}</strong>
          </div>
          <div>
            <span>Open tasks</span>
            <strong>{openTaskCount}</strong>
          </div>
        </div>
      </article>
    );
  }

  function renderDashboardWidgetContent(id: DashboardWidgetId) {
    switch (id) {
      case "today":
        return renderTodayCard();
      case "urgentTasks":
        return renderUrgentTasks(5);
      case "weeklyFocus":
        return renderWeeklyChart();
      case "courseRadar":
        return renderCourseRadar();
      case "examRunway":
        return renderExamRunway();
      case "garden":
        return renderGardenCard("custom-garden");
      case "stats":
        return renderStatsWidget();
    }
  }

  function moveDashboardWidget(fromId: DashboardWidgetId, toId: DashboardWidgetId) {
    if (fromId === toId) return;
    setCustomDashboardLayout((current) => {
      const fromIndex = current.findIndex((item) => item.id === fromId);
      const toIndex = current.findIndex((item) => item.id === toId);
      if (fromIndex < 0 || toIndex < 0 || fromIndex === toIndex) return current;
      const next = [...current];
      const [moved] = next.splice(fromIndex, 1);
      next.splice(toIndex, 0, moved);
      return next;
    });
  }

  function changeDashboardWidgetWidth(id: DashboardWidgetId, width: DashboardWidgetWidth) {
    setCustomDashboardLayout((current) => current.map((item) => (item.id === id ? { ...item, width } : item)));
  }

  function resetCustomDashboardLayout() {
    setCustomDashboardLayout(defaultCustomDashboardLayout);
    setDraggingWidgetId(null);
    setMessage("Custom dashboard reset.");
  }

  function startWidgetDrag(event: DragEvent<HTMLElement>, id: DashboardWidgetId) {
    setDraggingWidgetId(id);
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", id);
  }

  function dropWidget(event: DragEvent<HTMLElement>, toId: DashboardWidgetId) {
    event.preventDefault();
    const fromId = event.dataTransfer.getData("text/plain") as DashboardWidgetId;
    if (isDashboardWidgetId(fromId)) moveDashboardWidget(fromId, toId);
    setDraggingWidgetId(null);
  }

  function renderDashboardWidgetShell(widget: DashboardWidgetLayout) {
    const labels: Record<DashboardWidgetId, string> = {
      today: "Today",
      urgentTasks: "Urgent tasks",
      weeklyFocus: "Weekly focus",
      courseRadar: "Course radar",
      examRunway: "Exam runway",
      garden: "Knowledge garden",
      stats: "Stats",
    };

    return (
      <div
        key={widget.id}
        className={`dashboard-widget ${widget.width} ${dashboardEditing ? "editing" : ""} ${draggingWidgetId === widget.id ? "dragging" : ""}`}
        onDragOver={(event) => {
          if (!dashboardEditing) return;
          event.preventDefault();
          event.dataTransfer.dropEffect = "move";
        }}
        onDrop={(event) => dropWidget(event, widget.id)}
      >
        {dashboardEditing ? (
          <div className="widget-edit-bar">
            <span
              className="widget-drag-handle"
              draggable
              onDragStart={(event) => startWidgetDrag(event, widget.id)}
              onDragEnd={() => setDraggingWidgetId(null)}
              title={`Drag ${labels[widget.id]}`}
            >
              ⋮⋮ {labels[widget.id]}
            </span>
            <div className="widget-size-control" aria-label={`${labels[widget.id]} size`}>
              {([
                ["full", "Full"],
                ["half", "Half"],
                ["third", "Third"],
              ] as [DashboardWidgetWidth, string][]).map(([width, label]) => (
                <button
                  key={width}
                  type="button"
                  className={widget.width === width ? "active" : ""}
                  onClick={() => changeDashboardWidgetWidth(widget.id, width)}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
        ) : null}
        <div className="dashboard-widget-content" aria-hidden={dashboardEditing ? true : undefined}>
          {renderDashboardWidgetContent(widget.id)}
        </div>
      </div>
    );
  }

  function renderCustomDashboard() {
    return (
      <div className="custom-dashboard-wrap">
        <div className="custom-dashboard-toolbar">
          <div>
            <p className="eyebrow">Custom dashboard</p>
            <strong>{dashboardEditing ? "Drag widgets, then choose Full, Half, or Third." : "Your saved widget layout."}</strong>
          </div>
          <div className="custom-dashboard-actions">
            <button type="button" className="ghost-button" onClick={resetCustomDashboardLayout}>
              Reset
            </button>
            <button type="button" className="ghost-button" onClick={() => setDashboardEditing((current) => !current)}>
              {dashboardEditing ? "Done" : "Edit Dashboard"}
            </button>
          </div>
        </div>
        <div className={`custom-dashboard-grid ${dashboardEditing ? "editing" : ""}`}>
          {customDashboardLayout.map((widget) => renderDashboardWidgetShell(widget))}
        </div>
      </div>
    );
  }

  function getFieldDashboardData() {
    const todayLabel = new Intl.DateTimeFormat("en", { weekday: "long", day: "2-digit", month: "short", year: "numeric" }).format(new Date(`${calendarToday}T00:00:00`));
    const yearStart = new Date(new Date().getFullYear(), 0, 1);
    const weekNumber = Math.ceil((((new Date().getTime() - yearStart.getTime()) / 86400000) + yearStart.getDay() + 1) / 7);
    const dailyGoalMinutes = Math.max(1, state.settings.dailyGoalMinutes ?? 120);
    const goalProgress = clamp(Math.round((todayMinutes / dailyGoalMinutes) * 100), 0, 100);
    const totalUnitsLeft = state.tasks.reduce((sum, task) => sum + getRemainingUnits(task), 0);
    const selectedEntry = selectedTaskId ? todayCalendarEntries.find((entry) => entry.taskId === selectedTaskId) ?? null : null;
    const nextEntry = selectedEntry ?? todayCalendarEntries.find((entry) => !entry.completed) ?? todayCalendarEntries[0] ?? null;
    const nextTask = nextEntry?.taskId ? taskLookup.get(nextEntry.taskId) ?? null : null;
    const nextCourse = nextTask ? courseLookup.get(nextTask.courseId) ?? null : nextEntry?.adHocCourseId ? courseLookup.get(nextEntry.adHocCourseId) ?? null : null;
    const nextTitle = nextTask?.title ?? nextEntry?.adHocTitle ?? "Plan the next study block";
    const nextMeta = nextEntry
      ? `${getCalendarEntryUnitLabel(nextEntry)} · ${nextCourse?.name ?? "General focus"} · ${nextTask?.dueDate ? `due ${formatDate(nextTask.dueDate)}` : formatTimeRange(nextEntry)}`
      : "No task pinned for today · open the planner calendar to place one unit.";
    const queueCount = todayCalendarEntries.filter((entry) => !entry.completed).length;
    const nearestTaskDeadline = state.tasks
      .filter((task): task is Task & { dueDate: string } => Boolean(task.dueDate) && getRemainingUnits(task) > 0)
      .sort((a, b) => a.dueDate.localeCompare(b.dueDate))[0]?.dueDate ?? null;
    const nearestDeadline = nearestTaskDeadline ?? upcomingExams[0]?.examDate ?? null;
    const earliestExam = upcomingExams[0] ?? null;

    return { todayLabel, weekNumber, dailyGoalMinutes, goalProgress, totalUnitsLeft, nextEntry, nextTask, nextCourse, nextTitle, nextMeta, queueCount, nearestDeadline, earliestExam };
  }

  function renderFieldDashboardSwitch() {
    return (
      <div className="fn-view-switch" aria-label="Field Notebook dashboard layout">
        <span>View</span>
        {(["quiet", "full"] as FieldDashboardLayout[]).map((layout) => (
          <button key={layout} type="button" className={fieldDashboardLayout === layout ? "active" : ""} onClick={() => setFieldDashboardLayout(layout)}>
            {layout === "quiet" ? "Quiet" : "Full"}
          </button>
        ))}
      </div>
    );
  }

  function renderFieldNotebookQuietDashboard() {
    const { todayLabel, weekNumber, dailyGoalMinutes, goalProgress, totalUnitsLeft, queueCount, nearestDeadline } = getFieldDashboardData();
    const allQuietTasks = todayCalendarEntries.filter((entry) => !entry.completed);
    const quietTasks = allQuietTasks.slice(0, 6);
    const hiddenQuietTaskCount = Math.max(0, allQuietTasks.length - quietTasks.length);
    const selectedQuietEntry = quietTasks.find((entry) => entry.id === selectedQuietDashboardEntryId) ?? quietTasks[0] ?? null;
    const selectedQuietTask = selectedQuietEntry ? taskLookup.get(selectedQuietEntry.taskId) ?? null : null;
    const selectedQuietCourse = selectedQuietTask
      ? courseLookup.get(selectedQuietTask.courseId) ?? null
      : selectedQuietEntry?.adHocCourseId
        ? courseLookup.get(selectedQuietEntry.adHocCourseId) ?? null
        : null;
    const selectedQuietTitle = selectedQuietTask?.title ?? selectedQuietEntry?.adHocTitle ?? "Choose a study block";
    const selectedQuietMeta = selectedQuietEntry
      ? `${getCalendarEntryUnitLabel(selectedQuietEntry)} · ${selectedQuietCourse?.name ?? "General focus"} · ${selectedQuietTask?.dueDate ? `due ${formatDate(selectedQuietTask.dueDate)}` : formatTimeRange(selectedQuietEntry)}`
      : "Select a task below, or place one in the planner calendar.";
    const quietExams = upcomingExams.slice(0, 4);

    return (
      <section className="fn-dashboard fn-quiet-dashboard fade-up">
        <header className="fn-quiet-topbar">
          <div>
            <p className="fn-stamp-line">{todayLabel} · semester desk · week {weekNumber}</p>
            <h2>On the desk</h2>
            <p><strong>{selectedQuietTitle}</strong> · {selectedQuietMeta}</p>
          </div>
          {renderFieldDashboardSwitch()}
        </header>

        <div className="fn-quiet-grid">
          <main className="fn-quiet-main">
            <div className="fn-quiet-action-row">
              {selectedQuietTask ? <button type="button" onClick={() => focusTaskFromDashboard(selectedQuietTask)}>Start focus</button> : null}
              <button type="button" className="ghost-button" onClick={openTodayTodoDrawer}>Something else</button>
              <span>{formatMinutes(todayMinutes)} of {formatMinutes(dailyGoalMinutes)} done today.</span>
            </div>

            <div className="fn-quiet-rule" />

            <div className="fn-quiet-list" data-tour="dashboard-urgent">
              {quietTasks.length ? quietTasks.map((entry) => {
                const task = taskLookup.get(entry.taskId);
                const course = task ? courseLookup.get(task.courseId) : entry.adHocCourseId ? courseLookup.get(entry.adHocCourseId) : null;
                const active = selectedQuietEntry?.id === entry.id;
                return (
                  <div
                    key={entry.id}
                    className={`fn-quiet-row ${active ? "active" : ""}`}
                    style={{ "--fn-course": course?.color ?? "var(--fn-course-blue)" } as CSSProperties}
                  >
                    <input
                      className="dashboard-task-check"
                      type="checkbox"
                      checked={entry.completed}
                      onChange={() => toggleCalendarEntry(entry.id)}
                      title="Mark done"
                    />
                    <button
                      type="button"
                      className="fn-quiet-row-body"
                      onClick={() => setSelectedQuietDashboardEntryId(entry.id)}
                    >
                      <i />
                      <span>{task?.title ?? entry.adHocTitle ?? "Calendar task"} <em>{course?.name ?? "General"}</em></span>
                      <strong>{active ? "selected" : task?.dueDate ? `due ${formatDate(task.dueDate)}` : formatTimeRange(entry)}</strong>
                    </button>
                  </div>
                );
              }) : (
                <p className="fn-quiet-empty">No scheduled units today. Place one task in the planner calendar.</p>
              )}
              {hiddenQuietTaskCount > 0 ? <button type="button" className="fn-quiet-more" onClick={() => setFieldDashboardLayout("full")}>+{hiddenQuietTaskCount} more planned · open Full</button> : null}
            </div>

            <p className="fn-quiet-note">Course ledger, weekly rhythm, vault links, social, and squads all still exist under Full or their tabs. Quiet shows one decision and nothing you can scroll past.</p>
          </main>

          <aside className="fn-quiet-side">
            <section>
              <div className="fn-section-rule"><span>Pace</span></div>
              <p>{formatUnitAmount(totalUnitsLeft)} left<br />{totalWorkload.unitsPerDay.toFixed(1)} units/day<br />{openTaskCount} open tasks<br /><span>{queueCount} planned today</span></p>
            </section>
            <section>
              <div className="fn-section-rule"><span>Ahead</span></div>
              {quietExams.length ? quietExams.map((exam) => (
                <div key={exam.id} className="fn-quiet-exam-row"><span>{exam.title}</span><strong>{formatDate(exam.examDate)}</strong></div>
              )) : <p className="fn-quiet-empty">No upcoming exams.</p>}
              {nearestDeadline ? <div className="fn-quiet-exam-row"><span>Nearest deadline</span><strong>{formatDate(nearestDeadline)}</strong></div> : null}
            </section>
            <section className="fn-focus-ledger quiet">
              <span>Focused today</span>
              <strong>{formatMinutes(todayMinutes)} <em>/ {formatMinutes(dailyGoalMinutes)}</em></strong>
              <div className="fn-ledger-track"><i style={{ width: `${goalProgress}%` }} /></div>
              <small>{goalProgress}% · streak {streakDays} d</small>
            </section>
          </aside>
        </div>
      </section>
    );
  }

  function renderFieldNotebookFullDashboard() {
    const { todayLabel, weekNumber, dailyGoalMinutes, goalProgress, totalUnitsLeft, queueCount, nearestDeadline, earliestExam } = getFieldDashboardData();
    const fullQueueEntries = todayCalendarEntries.slice(0, 5);
    const selectedFullEntry = fullQueueEntries.find((entry) => entry.id === selectedQuietDashboardEntryId) ?? fullQueueEntries[0] ?? null;
    const selectedFullTask = selectedFullEntry ? taskLookup.get(selectedFullEntry.taskId) ?? null : null;
    const selectedFullCourse = selectedFullTask
      ? courseLookup.get(selectedFullTask.courseId) ?? null
      : selectedFullEntry?.adHocCourseId
        ? courseLookup.get(selectedFullEntry.adHocCourseId) ?? null
        : null;
    const selectedFullTitle = selectedFullTask?.title ?? selectedFullEntry?.adHocTitle ?? "Choose a study block";
    const selectedFullMeta = selectedFullEntry
      ? `${getCalendarEntryUnitLabel(selectedFullEntry)} · ${selectedFullCourse?.name ?? "General focus"} · ${selectedFullTask?.dueDate ? `due ${formatDate(selectedFullTask.dueDate)}` : formatTimeRange(selectedFullEntry)}`
      : "Select one planned task from the queue.";

    return (
      <section className="fn-dashboard fn-full-dashboard fade-up">
        <header className="fn-dashboard-head">
          <div>
            <p className="fn-stamp-line">{todayLabel} · week {weekNumber}</p>
            <h2>Today’s desk</h2>
            <p>Semester work · {openTaskCount} open tasks · {formatUnitAmount(totalUnitsLeft)} left{nearestDeadline ? ` · nearest deadline ${formatDate(nearestDeadline)}` : ""}</p>
          </div>
          {renderFieldDashboardSwitch()}
          <div className="fn-focus-ledger">
            <span>Focused today</span>
            <strong>{formatMinutes(todayMinutes)} <em>/ {formatMinutes(dailyGoalMinutes)}</em></strong>
            <div className="fn-ledger-track"><i style={{ width: `${goalProgress}%` }} /></div>
            <small>{goalProgress}% of daily goal · streak {streakDays} d</small>
          </div>
        </header>

        <div className="fn-dashboard-grid">
          <main className="fn-main-ledger">
            <section className="fn-next-sheet" data-tour="dashboard-today">
              <div className="fn-section-rule">
                <span>Study queue · ordered by pressure</span>
                <em>{queueCount} open · {todayCalendarEntries.length} planned today</em>
              </div>
              <article className="fn-next-task" style={{ "--fn-course": selectedFullCourse?.color ?? "var(--fn-course-blue)" } as CSSProperties}>
                <div>
                  <span className="fn-course-code">{selectedFullCourse ? selectedFullCourse.name.slice(0, 4).toUpperCase() : "DESK"}</span>
                  <span className="fn-pressure">Selected block</span>
                  <h3>{selectedFullTitle}</h3>
                  <p>{selectedFullMeta}</p>
                </div>
                <div className="fn-next-actions">
                  {selectedFullTask ? <button type="button" onClick={() => focusTaskFromDashboard(selectedFullTask)}>Start focus</button> : null}
                  <button type="button" className="ghost-button" onClick={openTodayTodoDrawer}>Something else</button>
                </div>
              </article>
              {renderUrgentTasks(5, { selectedEntryId: selectedFullEntry?.id ?? null, onSelectEntry: (entry) => setSelectedQuietDashboardEntryId(entry.id) })}
            </section>
          </main>

          <aside className="fn-side-ledger">
            <section>
              <div className="fn-section-rule"><span>Course ledger</span></div>
              {renderCourseRadar()}
            </section>
            <section className="fn-pace-note">
              <p className="eyebrow">Pace ledger · total workload</p>
              <div><strong>{formatUnitAmount(totalUnitsLeft)}</strong><span>left</span></div>
              <div><strong>{openTaskCount}</strong><span>open tasks</span></div>
              <div><strong>{totalWorkload.unitsPerDay.toFixed(1)}</strong><span>units/day</span></div>
              <p>{weeklyTotalMinutes ? `${formatMinutes(weeklyTotalMinutes)} logged this week.` : "No logged focus yet this week."}</p>
            </section>
          </aside>

          <aside className="fn-margin-ledger">
            <section data-tour="dashboard-exam-runway">
              <div className="fn-section-rule"><span>Exam runway</span></div>
              {renderExamRunway()}
            </section>
            <section className="fn-margin-note">
              <span>Margin note</span>
              <p>{earliestExam ? `${earliestExam.title} is first. Block review time before chasing lower-pressure tasks.` : "Pin one task to today so the dashboard can answer the next-action question."}</p>
            </section>
          </aside>

          <section className="fn-weekly-sheet" data-tour="dashboard-weekly">
            <div className="fn-section-rule">
              <span>Weekly rhythm · hours focused</span>
              <em>{formatMinutes(weeklyTotalMinutes)} this week</em>
            </div>
            {renderWeeklyChart("short-weekly")}
          </section>
        </div>

        {state.sessions.length > 0 ? (
          <p className="fn-footer-note">{formatMinutes(totalAllTimeMinutes)} logged across {sessionDays.size} day{sessionDays.size !== 1 ? "s" : ""} since {formatDate(firstSessionDate!)}</p>
        ) : (
          <p className="fn-footer-note dim">Your timeline begins when you complete your first session.</p>
        )}
      </section>
    );
  }

  function renderFieldNotebookDashboard() {
    return fieldDashboardLayout === "quiet" ? renderFieldNotebookQuietDashboard() : renderFieldNotebookFullDashboard();
  }

  function renderTimerLinkStrip() {
    return (
      <div className="timer-link-strip" data-tour="timer-links">
        <label className="field compact-field">
          <span>Semester</span>
          <select
            data-tour="timer-semester-select"
            value={state.timer.semesterId ?? ""}
            onChange={(event) => {
              const semesterId = event.target.value || null;
              const firstCourse = state.courses.find((course) => course.semesterId === semesterId);
              setState((current) => ({
                ...current,
                timer: { ...current.timer, semesterId, courseId: firstCourse?.id ?? null, taskId: null },
              }));
            }}
          >
            <option value="">Any semester</option>
            {state.semesters.map((semester) => (
              <option key={semester.id} value={semester.id}>{semester.name}</option>
            ))}
          </select>
        </label>
        <label className="field compact-field">
          <span>Course</span>
          <select
            data-tour="timer-course-select"
            value={state.timer.courseId ?? ""}
            onChange={(event) => {
              const courseId = event.target.value || null;
              const course = courseId ? courseLookup.get(courseId) : null;
              setState((current) => ({
                ...current,
                timer: { ...current.timer, semesterId: course?.semesterId ?? current.timer.semesterId, courseId, taskId: null },
              }));
            }}
          >
            <option value="">No linked course</option>
            {timerCourses.map((course) => (
              <option key={course.id} value={course.id}>{course.name}</option>
            ))}
          </select>
        </label>
        <label className="field compact-field task-switch-field">
          <span>Task</span>
          <select
            data-tour="timer-task-select"
            value={state.timer.taskId ?? ""}
            onChange={(event) => {
              const taskId = event.target.value || null;
              const task = taskId ? taskLookup.get(taskId) : null;
              setSelectedTaskId(taskId);
              setWabiQuietTaskId(taskId);
              setState((current) => ({
                ...current,
                timer: {
                  ...current.timer,
                  semesterId: task?.semesterId ?? current.timer.semesterId,
                  courseId: task?.courseId ?? current.timer.courseId,
                  taskId,
                  goal: task ? task.title : current.timer.goal,
                },
              }));
            }}
          >
            <option value="">No linked task</option>
            {timerSelectableTasks.map((task) => (
              <option key={task.id} value={task.id}>{task.title}</option>
            ))}
          </select>
        </label>
      </div>
    );
  }

  function renderTimerAdvancedPanel() {
    return (
      <>
        <button type="button" className="timer-advanced-toggle ghost-button" data-tour="timer-advanced-toggle" onClick={() => setTimerAdvancedOpen((current) => !current)}>
          <span>✎ Session log & links</span>
          <em>optional</em>
          <strong>{timerAdvancedOpen ? "⌄" : "›"}</strong>
        </button>

        {timerAdvancedOpen ? (
          <div className="timer-advanced-panel roomy-top">
            <div className="section-head compact-headline">
              <div>
                <p className="eyebrow">Advanced</p>
                <h3>Session details and logging</h3>
              </div>
              <span className="section-note">Link a course or task, tune minutes, and write notes for the session log.</span>
            </div>

            <div className="timer-input-grid">
              <label className="field">
                <span>Semester</span>
                <select
                  value={state.timer.semesterId ?? ""}
                  onChange={(event) => {
                    const semesterId = event.target.value || null;
                    const firstCourse = state.courses.find((course) => course.semesterId === semesterId);
                    setState((current) => ({
                      ...current,
                      timer: {
                        ...current.timer,
                        semesterId,
                        courseId: firstCourse?.id ?? null,
                        taskId: null,
                      },
                    }));
                  }}
                >
                  <option value="">Any semester</option>
                  {state.semesters.map((semester) => (
                    <option key={semester.id} value={semester.id}>
                      {semester.name}
                    </option>
                  ))}
                </select>
              </label>
              <label className="field">
                <span>Course</span>
                <select
                  value={state.timer.courseId ?? ""}
                  onChange={(event) => {
                    const courseId = event.target.value || null;
                    const course = courseId ? courseLookup.get(courseId) : null;
                    setState((current) => ({
                      ...current,
                      timer: {
                        ...current.timer,
                        semesterId: course?.semesterId ?? current.timer.semesterId,
                        courseId,
                        taskId: null,
                      },
                    }));
                  }}
                >
                  <option value="">No linked course</option>
                  {timerCourses.map((course) => (
                    <option key={course.id} value={course.id}>
                      {course.name}
                    </option>
                  ))}
                </select>
              </label>
              <label className="field">
                <span>Task</span>
                <select
            value={state.timer.taskId ?? ""}
            onChange={(event) => {
              const taskId = event.target.value || null;
              const task = taskId ? taskLookup.get(taskId) : null;
              setSelectedTaskId(taskId);
              setWabiQuietTaskId(taskId);
              setState((current) => ({
                      ...current,
                      timer: {
                        ...current.timer,
                        semesterId: task?.semesterId ?? current.timer.semesterId,
                        courseId: task?.courseId ?? current.timer.courseId,
                        taskId,
                        goal: task ? task.title : current.timer.goal,
                      },
                    }));
                  }}
                >
                  <option value="">No linked task</option>
                  {timerTasks.map((task) => (
                    <option key={task.id} value={task.id}>
                      {task.title}
                    </option>
                  ))}
                </select>
              </label>
              <label className="field">
                <span>{state.timer.mode === "exam" ? "Exam minutes" : "Focus minutes"}</span>
                <input
                  type="number"
                  min="1"
                  value={state.timer.mode === "exam" ? state.timer.examMinutes : state.timer.studyMinutes}
                  onChange={(event) => {
                    const next = Math.max(1, Number(event.target.value) || 1);
                    setState((current) => {
                      const timer = {
                        ...current.timer,
                        studyMinutes: current.timer.mode === "exam" ? current.timer.studyMinutes : next,
                        examMinutes: current.timer.mode === "exam" ? next : current.timer.examMinutes,
                        presetLabel: current.timer.mode === "exam" ? current.timer.presetLabel : "Custom",
                      };
                      return {
                        ...current,
                        timer: {
                          ...timer,
                          remainingSeconds: current.timer.running ? getDisplayRemainingSeconds(current.timer) : getIdleTimerSeconds(timer),
                        },
                      };
                    });
                  }}
                />
              </label>
              <label className="field">
                <span>Break minutes</span>
                <input
                  type="number"
                  min="0"
                  disabled={state.timer.mode === "exam"}
                  value={state.timer.breakMinutes}
                  onChange={(event) =>
                    setState((current) => ({
                      ...current,
                      timer: { ...current.timer, breakMinutes: Math.max(0, Number(event.target.value) || 0), presetLabel: "Custom" },
                    }))
                  }
                />
              </label>
              <label className="field wide">
                <span>Goal for this block</span>
                <input
                  data-tour="timer-goal"
                  value={state.timer.goal}
                  onChange={(event) => setState((current) => ({ ...current, timer: { ...current.timer, goal: event.target.value } }))}
                  placeholder="What should exist when the timer ends?"
                />
              </label>
              <label className="field wide">
                <span>What did you learn?</span>
                <textarea
                  data-tour="timer-learned"
                  value={state.timer.learned}
                  onChange={(event) => setState((current) => ({ ...current, timer: { ...current.timer, learned: event.target.value } }))}
                  placeholder="Short reflection that can go straight into Obsidian later."
                />
              </label>
              <label className="field">
                <span>What is still weak?</span>
                <input
                  data-tour="timer-blocker"
                  value={state.timer.blocker}
                  onChange={(event) => setState((current) => ({ ...current, timer: { ...current.timer, blocker: event.target.value } }))}
                  placeholder="Weak topic or blocker"
                />
              </label>
              <label className="field">
                <span>Next step</span>
                <input
                  data-tour="timer-next-step"
                  value={state.timer.nextStep}
                  onChange={(event) => setState((current) => ({ ...current, timer: { ...current.timer, nextStep: event.target.value } }))}
                  placeholder="What comes next?"
                />
              </label>
              <label className="field">
                <span>Confidence</span>
                <input
                  data-tour="timer-confidence"
                  type="range"
                  min="1"
                  max="5"
                  step="1"
                  value={state.timer.confidence}
                  onChange={(event) => setState((current) => ({ ...current, timer: { ...current.timer, confidence: Number(event.target.value) } }))}
                />
                <span className="range-value">{state.timer.confidence}/5</span>
              </label>
            </div>
          </div>
        ) : null}
      </>
    );
  }

  function renderWabiTimer() {
    const phaseWord = state.timer.phase === "break" ? "BREAK"
      : state.timer.phase === "stopwatch" ? "STOPWATCH"
      : state.timer.mode === "exam" ? "EXAM"
      : "WORK";
    const phaseMinutes = state.timer.phase === "break" ? state.timer.breakMinutes
      : state.timer.mode === "exam" ? state.timer.examMinutes
      : state.timer.studyMinutes;
    const phaseDetail = state.timer.phase === "stopwatch" ? "COUNTING UP" : `${phaseMinutes} MIN`;
    const todaySessions = state.sessions.filter((session) => isoDate(new Date(session.endedAt)) === calendarToday);

    return (
      <section className="wabi-timer-grid fade-up">
        <div className="wabi-timer-head">
          <p className="wabi-eyebrow">TIMER</p>
          <h2>{timerTask?.title ?? timerCourse?.name ?? "General focus"}</h2>
        </div>

        {renderTimerLinkStrip()}

        <div className="wabi-timer-body">
          <div className="wabi-timer-clock-col">
            {timerFaceEditing ? (
              <input
                className="wabi-timer-clock-input"
                value={timerFaceDraft}
                onChange={(event) => setTimerFaceDraft(event.target.value)}
                onBlur={applyTimerFaceDraft}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    applyTimerFaceDraft();
                  } else if (event.key === "Escape") {
                    setTimerFaceEditing(false);
                    setTimerFaceDraft(formatClock(state.timer.remainingSeconds));
                  }
                }}
                inputMode="numeric"
                aria-label="Timer duration"
                autoFocus
              />
            ) : (
              <button
                type="button"
                className="wabi-timer-clock"
                onClick={startTimerFaceEditing}
                disabled={state.timer.running || state.timer.phase !== "idle" || (state.timer.mode !== "exam" && !isCustomTimerPreset)}
              >
                <TimerClockDigits timer={state.timer} formatClock={formatClock} />
              </button>
            )}
          </div>

          <div className="wabi-timer-modes-col">
            <p className="wabi-timer-phase-label">{phaseWord} · {phaseDetail}</p>
            <div className="wabi-timer-mode-grid" data-tour="timer-presets">
              {focusPresets.map((preset) => (
                <button
                  key={preset.label}
                  type="button"
                  className={`wabi-timer-mode-card ${state.timer.presetLabel === preset.label ? "active" : ""}`}
                  disabled={state.timer.running}
                  onClick={() => applyPreset(preset.label, preset.study, preset.breakMinutes, preset.mode)}
                >
                  <strong>{preset.label.replace(" 25/5", "").replace(" 52/17", "").replace(" 90/20", "")}</strong>
                  <span>{preset.mode === "endless" ? "counts up" : preset.mode === "exam" ? `${preset.study} min` : `${preset.study} / ${preset.breakMinutes}`}</span>
                </button>
              ))}
              <button
                type="button"
                className={`wabi-timer-mode-card ${isCustomTimerPreset ? "active" : ""}`}
                disabled={state.timer.running}
                onClick={() =>
                  setState((current) => ({
                    ...current,
                    timer: {
                      ...current.timer,
                      presetLabel: "Custom",
                      mode: "focus",
                      studyMinutes: defaultTimer.studyMinutes,
                      breakMinutes: defaultTimer.breakMinutes,
                      phase: "idle",
                      running: false,
                      startedAt: null,
                      endsAt: null,
                      remainingSeconds: defaultTimer.studyMinutes * 60,
                      loggedSplitSeconds: 0,
                      activeSegments: [],
                    },
                  }))
                }
              >
                <strong>Custom</strong>
                <span>set by hand</span>
              </button>
            </div>
            {isCustomTimerPreset || state.timer.mode === "exam" ? (
              <div className="wabi-timer-custom-row">
                <label>
                  <span>{state.timer.mode === "exam" ? "Exam minutes" : "Focus minutes"}</span>
                  <input
                    type="number"
                    min="1"
                    value={state.timer.mode === "exam" ? state.timer.examMinutes : state.timer.studyMinutes}
                    disabled={state.timer.running}
                    onChange={(event) => {
                      const next = Math.max(1, Number(event.target.value) || 1);
                      setState((current) => {
                        const timer = {
                          ...current.timer,
                          studyMinutes: current.timer.mode === "exam" ? current.timer.studyMinutes : next,
                          examMinutes: current.timer.mode === "exam" ? next : current.timer.examMinutes,
                          presetLabel: current.timer.mode === "exam" ? current.timer.presetLabel : "Custom",
                        };
                        return {
                          ...current,
                          timer: { ...timer, remainingSeconds: current.timer.running ? getDisplayRemainingSeconds(current.timer) : getIdleTimerSeconds(timer) },
                        };
                      });
                    }}
                  />
                </label>
                {state.timer.mode !== "exam" ? (
                  <label>
                    <span>Break minutes</span>
                    <input
                      type="number"
                      min="0"
                      disabled={state.timer.running}
                      value={state.timer.breakMinutes}
                      onChange={(event) =>
                        setState((current) => ({
                          ...current,
                          timer: { ...current.timer, breakMinutes: Math.max(0, Number(event.target.value) || 0), presetLabel: "Custom" },
                        }))
                      }
                    />
                  </label>
                ) : null}
              </div>
            ) : null}
          </div>
        </div>

        <div className="wabi-timer-actions">
          <button type="button" className="wabi-btn-solid" onClick={state.timer.phase === "idle" ? startTimer : pauseTimer}>
            {state.timer.phase === "idle" ? (state.timer.mode === "endless" ? "START TRACKING" : "START") : state.timer.running ? "PAUSE" : "RESUME"}
          </button>
          <button type="button" className="wabi-btn-outline" onClick={resetTimer}>RESET</button>
          <button type="button" className="wabi-btn-outline" onClick={completeSessionManually} disabled={state.timer.phase === "idle" || state.timer.phase === "break"}>
            LOG AND CLOSE
          </button>
          {state.timer.mode !== "exam" && state.timer.mode !== "endless" ? (
            <button
              type="button"
              className="wabi-text-link"
              onClick={() => {
                if (state.timer.phase === "study" && getTimerMinutes(state.timer) > 0) {
                  setMessage("Save or reset the current study session before switching to break.");
                  return;
                }
                setState((current) => ({
                  ...current,
                  timer: {
                    ...current.timer,
                    running: false,
                    phase: current.timer.phase === "break" ? "study" : "break",
                    remainingSeconds: (current.timer.phase === "break" ? current.timer.studyMinutes : current.timer.breakMinutes) * 60,
                    startedAt: null,
                    endsAt: null,
                    loggedSplitSeconds: 0,
                    activeSegments: [],
                  },
                }));
              }}
            >
              Switch to {state.timer.phase === "break" ? "study" : "break"} →
            </button>
          ) : null}
        </div>

        {renderTimerAdvancedPanel()}

        <div className="wabi-timer-log">
          <p className="wabi-eyebrow">LOGGED TODAY</p>
          {todaySessions.length ? (
            todaySessions.map((session) => (
              <div className="wabi-timer-log-row" key={session.id}>
                <span className="wabi-timer-log-time">{new Intl.DateTimeFormat("en", { hour: "2-digit", minute: "2-digit", hour12: false }).format(new Date(session.endedAt))}</span>
                <span className="wabi-timer-log-title">{session.goal || session.presetLabel}</span>
                <span className="wabi-timer-log-mins">{formatMinutes(session.minutes)}</span>
                <button type="button" className="wabi-timer-log-remove" aria-label={`Delete session ${session.goal || session.presetLabel}`} onClick={() => removeSession(session.id)}>×</button>
              </div>
            ))
          ) : (
            <p className="wabi-empty">Nothing logged yet today.</p>
          )}
        </div>
      </section>
    );
  }

  function renderWabiBreakRoom() {
    const breakMM = String(Math.floor(breakTimerRemaining / 60)).padStart(2, "0");
    const breakSS = String(breakTimerRemaining % 60).padStart(2, "0");
    const breakTotalSeconds = Math.max(1, breakTimerMinutes * 60);
    const breakProgress = clamp(1 - breakTimerRemaining / breakTotalSeconds, 0, 1);
    const breathStep = breathElapsedSeconds % 19;
    const breathPhaseIndex = breathStep < 4 ? 0 : breathStep < 11 ? 1 : 2;
    const breathPhases = [
      { label: "Breathe in", seconds: 4, instruction: "Through the nose, quietly." },
      { label: "Hold", seconds: 7, instruction: "Still, shoulders down." },
      { label: "Breathe out", seconds: 8, instruction: "Through the mouth, slow and audible." },
    ];
    const breathPhase = breathPhases[breathPhaseIndex];
    const breathPhaseElapsed = breathPhaseIndex === 0 ? breathStep : breathPhaseIndex === 1 ? breathStep - 4 : breathStep - 11;
    const breathRemaining = breathPhase.seconds - breathPhaseElapsed;
    const breathSize = breathPhaseIndex === 0
      ? 60 + (breathStep / 4) * 110
      : breathPhaseIndex === 1
        ? 170
        : 170 - ((breathStep - 11) / 8) * 110;
    return (
      <section className="wabi-break-grid">
        <div className="wabi-rest-room-tabs" role="tablist" aria-label="Break room">
          <button type="button" role="tab" aria-selected={wabiRestRoom === "games"} className={wabiRestRoom === "games" ? "active" : ""} onClick={() => setWabiRestRoom("games")}>GAMES</button>
          <button type="button" role="tab" aria-selected={wabiRestRoom === "meditation"} className={wabiRestRoom === "meditation" ? "active" : ""} onClick={() => setWabiRestRoom("meditation")}>MEDITATION</button>
        </div>

        {wabiRestRoom === "games" ? <>
          <div className="wabi-rest-card">
            <WabiRestFluidRing progress={breakProgress} running={breakTimerRunning} dark={theme === "dark"} />
            <h2>Rest, {breakTimerMinutes} minute{breakTimerMinutes === 1 ? "" : "s"}</h2>
            <p>Water the plant on the sill, or play one round of something small.</p>
            {breakTimerEditing ? (
              <input className="wabi-rest-clock-input" value={breakTimerDraft} onChange={(event) => setBreakTimerDraft(event.target.value)} onBlur={applyBreakTimerDraft} onKeyDown={(event) => {
                if (event.key === "Enter") { event.preventDefault(); applyBreakTimerDraft(); }
                else if (event.key === "Escape") { setBreakTimerEditing(false); setBreakTimerDraft(formatClock(breakTimerRemaining)); }
              }} inputMode="numeric" aria-label="Break timer duration" autoFocus />
            ) : (
              <button type="button" className="wabi-rest-clock" onClick={startBreakTimerEditing} disabled={breakTimerRunning}>{breakMM}:{breakSS}</button>
            )}
            <div className="wabi-rest-actions">
              <button type="button" className="wabi-btn-solid" onClick={toggleBreakTimerRun}>{breakTimerRunning ? "PAUSE" : "START"}</button>
              <button type="button" className="wabi-text-link" onClick={resetBreakTimer}>RESET</button>
            </div>
          </div>

          <div className="wabi-break-lower">
            <div className="wabi-xp-water" data-tour="break-xp" title={`XP ${xpProgress} / 45 — ~${minsUntilNext} min to next unlock`}>
              <div className="wabi-xp-water-track"><div className="wabi-xp-water-fill" style={{ height: `${xpPercent}%` }} /></div>
              <span className="wabi-xp-water-label">{effectiveUnlocked.length}/{STUDY_BREAK_GAME_COUNT}</span>
            </div>
            <div className="wabi-games-col">
              <div className="wabi-eyebrow-row"><span className="wabi-eyebrow">GAMES</span><span className="wabi-faint-text">{effectiveUnlocked.length} of {STUDY_BREAK_GAME_COUNT} available</span></div>
              <div className="wabi-games-grid" data-tour="break-games">
                {studyBreakGames.map((game) => {
                  const unlocked = effectiveUnlocked.includes(game.name);
                  return <div key={game.name} className={`wabi-game-card ${unlocked ? "unlocked" : "locked"} ${celebrating === game.name ? "celebrating" : ""}`} onAnimationEnd={() => setCelebrating(null)}>
                    <div className="wabi-game-head"><strong>{game.name}</strong>{unlocked ? <span className="wabi-game-tag">READY</span> : null}</div>
                    <span className="wabi-game-desc">{game.desc}</span>
                    <div className="wabi-game-foot">
                      {unlocked ? (
                        game.name === "Daily Durak" ? <><span className="wabi-game-progress">{(state.durakPuzzle.solvedCount || 0)}/3</span><button type="button" className="wabi-text-link" data-tour="break-game-action" onClick={() => { logPlayedBreak(game.name); setShowDurakPuzzle(true); }}>PLAY</button></>
                          : game.name === "Wordle" ? <>{wordlePuzzleIsToday && state.wordlePuzzle.completed ? <span className="wabi-game-progress">{state.wordlePuzzle.won ? "Solved" : "Failed"}</span> : <span />}<button type="button" className="wabi-text-link" data-tour="break-game-action" onClick={() => { logPlayedBreak(game.name); setShowWordlePuzzle(true); }}>PLAY</button></>
                            : game.name === "Travle" ? <>{travlePuzzleIsToday && state.travlePuzzle.completed ? <span className="wabi-game-progress">{state.travlePuzzle.won ? "Solved" : "Failed"}</span> : <span />}<button type="button" className="wabi-text-link" data-tour="break-game-action" onClick={() => { logPlayedBreak(game.name); setShowTravlePuzzle(true); }}>PLAY</button></>
                              : game.name === "Geodle" ? <>{geodlePuzzleIsToday && state.geodlePuzzle.completed ? <span className="wabi-game-progress">{state.geodlePuzzle.won ? "Solved" : "Failed"}</span> : <span />}<button type="button" className="wabi-text-link" data-tour="break-game-action" onClick={() => { logPlayedBreak(game.name); setShowGeodlePuzzle(true); }}>PLAY</button></>
                                : game.name === "Flaggle" ? <>{flagglePuzzleIsToday && state.flagglePuzzle.completed ? <span className="wabi-game-progress">{state.flagglePuzzle.won ? "Solved" : "Failed"}</span> : <span />}<button type="button" className="wabi-text-link" data-tour="break-game-action" onClick={() => { logPlayedBreak(game.name); setShowFlagglePuzzle(true); }}>PLAY</button></>
                                  : <a href={game.url} target="_blank" rel="noreferrer" className="wabi-text-link" data-tour="break-game-action" onClick={() => logPlayedBreak(game.name)}>PLAY</a>
                      ) : canUnlockMore ? <button type="button" className="wabi-text-link" data-tour="break-game-action" onClick={() => { unlockGame(game.name); setCelebrating(game.name); }}>UNLOCK</button> : <span className="wabi-game-progress">~{minsUntilNext} min</span>}
                    </div>
                  </div>;
                })}
              </div>
            </div>
          </div>
          <div className="wabi-pet-rock" data-tour="break-rock" onClick={patRock} role="button" tabIndex={0} onKeyDown={(event) => { if (!event.repeat && (event.key === "Enter" || event.key === " ")) { event.preventDefault(); patRock(); } }}>
            <div className="rock-area"><span className={`rock ${rockBounce ? "bounce" : ""} ${rockCelebrating ? "celebrate" : ""}`} onAnimationEnd={() => setRockCelebrating(false)}>{'\u{1FAA8}'}</span>{rockStage.plant ? <span className="rock-plant">{rockStage.plant}</span> : null}</div>
            <span className="wabi-faint-text">{rockStage.label} · {state.petRockPats} pat{state.petRockPats !== 1 ? "s" : ""}</span>
          </div>
        </> : (
          <div className="wabi-meditation-room">
            <div className="wabi-breath-orbit" aria-live="polite">
              <span className="wabi-breath-ring" style={{ width: `${breathOn ? breathSize : 60}px`, height: `${breathOn ? breathSize : 60}px` }} />
              <span className="wabi-breath-count">{breathOn ? breathRemaining : ""}</span>
            </div>
            <h2>{breathOn ? breathPhase.label : "Four, seven, eight"}</h2>
            <p>{breathOn ? "Follow the ring. Count with it, do not rush the hold." : "In for four, hold for seven, out for eight. Four rounds and the break is over."}</p>
            <button type="button" className="wabi-btn-solid" onClick={toggleBreathingExercise}>{breathOn ? "STOP" : "BEGIN"}</button>
            <span className="wabi-breath-round">{breathOn ? `Round ${Math.floor(breathElapsedSeconds / 19) + 1} of 4` : "Four rounds, about eighty seconds."}</span>
            <div className="wabi-breath-guide">
              {breathPhases.map((phase, index) => <div key={phase.label} className={breathOn && breathPhaseIndex === index ? "active" : ""}>
                <strong>{phase.seconds}</strong><span>SECONDS</span><h3>{phase.label}</h3><p>{phase.instruction}</p>
              </div>)}
            </div>
            <p className="wabi-breath-note">Nose in, tongue behind the front teeth, mouth out. Sitting up, not lying down.</p>
          </div>
        )}
      </section>
    );
  }

  function renderWabiQuietMode() {
    const { nextTask, nextEntry, nextTitle, nextMeta } = getFieldDashboardData();
    // The active Timer task is authoritative; a quiet-mode pick is only a fallback.
    const quietTask = timerTask ?? (wabiQuietTaskId ? taskLookup.get(wabiQuietTaskId) ?? null : null);
    const quietEntry = quietTask ? todayCalendarEntries.find((entry) => entry.taskId === quietTask.id) ?? null : null;
    const useQuietTask = quietTask !== null && (quietTask.id === timerTask?.id || !quietEntry?.completed);
    const focusTask = useQuietTask ? quietTask : nextTask;
    const focusEntry = useQuietTask ? quietEntry : nextEntry;
    const focusTitle = useQuietTask && quietTask ? quietTask.title : nextTitle;
    const focusMeta = useQuietTask && quietTask
      ? `${focusEntry ? getCalendarEntryUnitLabel(focusEntry) + " · " : ""}${courseLookup.get(quietTask.courseId)?.name ?? "General focus"}${quietTask.dueDate ? ` · due ${formatDate(quietTask.dueDate)}` : ""}`
      : nextMeta;
    const otherOpenTasks = todayCalendarEntries
      .filter((entry) => !entry.completed && entry.taskId !== focusTask?.id)
      .map((entry) => ({ entry, task: taskLookup.get(entry.taskId) }))
      .filter((row): row is { entry: CalendarEntry; task: Task } => Boolean(row.task));

    return (
      <div className="wabi-quiet-overlay">
        <div className="wabi-quiet-inner">
          <span className="wabi-eyebrow">NOW</span>
          <div className="wabi-quiet-task-picker">
            <button type="button" className="wabi-quiet-title" onClick={() => setWabiQuietTaskMenuOpen((open) => !open)}>
              {focusTitle}
            </button>
            {wabiQuietTaskMenuOpen ? (
              <div className="wabi-quiet-dropdown" role="listbox">
                {otherOpenTasks.length ? otherOpenTasks.map(({ entry, task }) => (
                  <button key={entry.id} type="button" className="wabi-quiet-dropdown-item" onClick={() => focusTaskInQuietMode(task)}>
                    <span>{task.title}</span>
                    <em>{courseLookup.get(task.courseId)?.name ?? "General"}</em>
                  </button>
                )) : (
                  <span className="wabi-quiet-dropdown-empty">Nothing else planned for today.</span>
                )}
              </div>
            ) : null}
          </div>
          <p className="wabi-quiet-meta">{focusMeta}</p>
          <div className="wabi-quiet-clock"><TimerClockDigits timer={state.timer} formatClock={formatClock} /></div>
          <div className="wabi-quiet-actions">
            <button type="button" className="wabi-btn-solid" onClick={state.timer.phase === "idle" ? startTimer : pauseTimer}>
              {state.timer.phase === "idle" ? (state.timer.mode === "endless" ? "START TRACKING" : "START") : state.timer.running ? "PAUSE" : "RESUME"}
            </button>
            <button type="button" className="wabi-btn-outline" onClick={completeSessionManually}>DONE, LOG IT</button>
          </div>
          <button type="button" className="wabi-quiet-leave" onClick={() => setWabiQuietMode(false)}>LEAVE QUIET MODE</button>
        </div>
      </div>
    );
  }

  function renderWabiToolbarMenu() {
    if (!menuOpen || !wabiMenuPosition) return null;

    return createPortal(
      <div
        className="topbar-menu wabi-toolbar-menu"
        role="menu"
        style={{ position: "fixed", top: wabiMenuPosition.top, left: wabiMenuPosition.left, right: "auto" }}
      >
        <button
          type="button"
          role="menuitem"
          onClick={() => {
            downloadBackup(state);
            setMenuOpen(false);
          }}
        >
          Backup JSON
        </button>
        <button type="button" role="menuitem" onClick={() => openMenuPanel("theme")}>Change theme</button>
        <button type="button" role="menuitem" onClick={() => openMenuPanel("personal")}>Personal</button>
        <button type="button" role="menuitem" onClick={() => openMenuPanel("options")}>Options</button>
        <button type="button" role="menuitem" onClick={() => openMenuPanel("settings")}>Settings</button>
      </div>,
      document.body,
    );
  }

  function renderWabiSidebar() {
    const wabiTabLabels: Record<TabKey, string> = { dashboard: "Today", planner: "Plan", timer: "Timer", vault: "Notes", break: "Rest", friends: "Circle" };
    const wabiTabKanji: Record<TabKey, string> = { dashboard: "今日", planner: "計画", timer: "時計", vault: "記録", break: "休み", friends: "仲間" };
    const { goalProgress } = getFieldDashboardData();
    const visibleTabs = state.settings.visibleTabs ?? defaultState.settings.visibleTabs;
    return (
      <aside className="wabi-sidebar">
        <div className="wabi-brand">
          <span className="wabi-brand-mark" />
          <span className="wabi-brand-name">Kokoro</span>
        </div>
        <div className="wabi-health" title="Heuristic score based on task progress, overdue work, and exam pressure.">
          <div className="wabi-health-ring">
            <svg width="36" height="36" style={{ transform: "rotate(-90deg)" }}>
              <circle cx="18" cy="18" r="15.5" fill="none" stroke="var(--ring-track)" strokeWidth="4" />
              <circle
                cx="18" cy="18" r="15.5" fill="none" stroke={scoreColor} strokeWidth="4"
                strokeDasharray="97.39"
                strokeDashoffset={97.39 - (overallHealth / 100) * 97.39}
                strokeLinecap="round"
                style={{ transition: "stroke-dashoffset 0.7s cubic-bezier(0.2,0.7,0.3,1)" }}
              />
            </svg>
            <div className="wabi-health-ring-value">{overallHealth}</div>
          </div>
          <div>
            <div className="wabi-eyebrow">Overall</div>
            <span className="wabi-health-label" style={{ color: scoreColor }}>{healthLabel}</span>
          </div>
        </div>

        <div className="wabi-toolbar">
          <button
            type="button"
            className="wabi-toolbar-button"
            onClick={() => setTheme((current) => (current === "dark" ? "light" : "dark"))}
            title="Toggle theme"
          >
            {theme === "dark" ? (
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="4" />
                <path d="M12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.5 1.5M17.5 17.5 19 19M19 5l-1.5 1.5M6.5 17.5 5 19" />
              </svg>
            ) : (
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                <path d="M21 12.8A8.5 8.5 0 1 1 11.2 3 6.5 6.5 0 0 0 21 12.8z" />
              </svg>
            )}
          </button>
          {state.settings.showHelpButton !== false ? (
            <button type="button" className="wabi-toolbar-button" onClick={openMenuHelp} aria-label="Open menu help" title="Menu help">
              ?
            </button>
          ) : null}
          <div className="wabi-toolbar-menu-wrap">
            <button
              ref={wabiMenuButtonRef}
              type="button"
              className="wabi-toolbar-button"
              aria-label="Open menu"
              aria-expanded={menuOpen}
              onClick={() => setMenuOpen((current) => {
                if (!current) updateWabiMenuPosition();
                return !current;
              })}
            >
              <span className="hamburger-button-icon">
                <span />
                <span />
                <span />
              </span>
            </button>
            {renderWabiToolbarMenu()}
          </div>
        </div>
        <nav className="wabi-nav" aria-label="Primary navigation">
          {primaryTabs.filter(({ id }) => visibleTabs[id] !== false).map(({ id: key }) => {
            const active = state.activeTab === key && !wabiQuietMode;
            return (
              <button
                key={key}
                type="button"
                className={`wabi-nav-item ${active ? "active" : ""}`}
                onClick={() => {
                  setWabiQuietMode(false);
                  setActiveTab(key);
                }}
              >
                <span className="wabi-nav-label">
                  <span>{wabiTabLabels[key]}</span>
                  {active ? <em>{wabiTabKanji[key]}</em> : null}
                  {key === "friends" && hasUnreadSocial ? <span className="nav-badge" /> : null}
                </span>
                <span className="wabi-nav-sub">{key}</span>
              </button>
            );
          })}
        </nav>
        <span className="wabi-sidebar-spacer" />
        <div className="wabi-sidebar-foot">
          <div className="wabi-eyebrow">TENDED TODAY</div>
          <div className="wabi-foot-time">{formatMinutes(todayMinutes)}</div>
          <div className="wabi-foot-track"><div style={{ width: `${goalProgress}%` }} /></div>
          <button type="button" className="wabi-quiet-link" onClick={() => {
            if (!wabiQuietMode) {
              setWabiQuietTaskId(state.timer.taskId);
              setWabiQuietTaskMenuOpen(false);
            }
            setWabiQuietMode((current) => !current);
          }}>
            {wabiQuietMode ? "← BACK TO THE APP" : "QUIET MODE →"}
          </button>
        </div>
      </aside>
    );
  }

  function renderWabiSabiDashboard() {
    const { todayLabel, dailyGoalMinutes, nextEntry, nextTask, nextTitle, nextMeta, queueCount } = getFieldDashboardData();
    const minutesToGoal = Math.max(0, dailyGoalMinutes - todayMinutes);
    const remainingLabel = queueCount > 0 ? `${queueCount} open · ${minutesToGoal > 0 ? `${formatMinutes(minutesToGoal)} to today's goal` : "goal met"}` : "goal met";

    return (
      <section className="wabi-dashboard fade-up">
        <div className="wabi-today-head">
          <div>
            <p className="wabi-eyebrow">TODAY</p>
            <h2>{todayLabel}</h2>
          </div>
          <span className="wabi-remaining">{remainingLabel}</span>
        </div>

        <div className="wabi-one-thing">
          <p className="wabi-eyebrow accent">ONE THING</p>
          <h3>{nextTitle}</h3>
          <p className="wabi-meta">{nextMeta}</p>
          <div className="wabi-one-thing-actions">
            {nextTask ? (
              <button type="button" className="wabi-btn-solid" onClick={() => focusTaskFromDashboard(nextTask)}>START</button>
            ) : null}
            {nextEntry ? (
              <button type="button" className="wabi-btn-outline" onClick={() => toggleCalendarEntry(nextEntry.id)}>MARK DONE</button>
            ) : null}
            <button type="button" className="wabi-text-link" onClick={openTodayTodoDrawer}>SOMETHING ELSE</button>
          </div>
        </div>

        <div className="wabi-task-list-head">
          <span className="wabi-eyebrow">PLANNED TODAY</span>
        </div>
        <div className="wabi-task-list" data-tour="dashboard-urgent">
          {todayCalendarEntries.length ? (
            todayCalendarEntries.map((entry) => {
              const task = taskLookup.get(entry.taskId);
              const course = task ? courseLookup.get(task.courseId) : entry.adHocCourseId ? courseLookup.get(entry.adHocCourseId) : null;
              const title = task?.title ?? entry.adHocTitle ?? "Calendar task";
              return (
                <div className={`wabi-task-row ${task?.id === selectedTaskId ? "selected" : ""}`} key={entry.id}>
                  <button
                    type="button"
                    className={`wabi-task-dot ${entry.completed ? "done" : ""}`}
                    onClick={() => toggleCalendarEntry(entry.id)}
                    aria-label={entry.completed ? "Mark not done" : "Mark done"}
                  />
                  <button
                    type="button"
                    className={`wabi-task-title ${entry.completed ? "done" : ""}`}
                    onClick={() => (task ? selectTaskForNow(task) : openCalendarDrawer(entry.date))}
                  >
                    {title}
                  </button>
                  <span className="wabi-task-subject">{course?.name ?? "General"}</span>
                  <span className="wabi-task-amount">{formatUnitAmount(getCalendarEntryAmount(entry))}</span>
                  <span className="wabi-task-due">{task?.dueDate ? formatDate(task.dueDate) : formatTimeRange(entry)}</span>
                </div>
              );
            })
          ) : (
            <p className="wabi-empty">Nothing planned today. Add tasks from the planner calendar.</p>
          )}
        </div>
        <p className="wabi-hint">Click a title to make it the current task. Click the mark to close it out.</p>
      </section>
    );
  }

  function renderWabiVaultCourseShelf(space: "references" | "summaries") {
    const selectedCourseId = space === "references" ? referenceCourseId : summaryCourseId;
    return (
      <aside className="wabi-vault-shelf" aria-label={`${space} by semester`}>
        {state.semesters.map((semester) => {
          const courses = getSemesterCourses(state, semester.id);
          const expanded = expandedSemesterIds.includes(semester.id);
          return (
            <div key={semester.id} className={`wabi-vault-semester ${expanded ? "open" : ""}`}>
              <button type="button" className="wabi-vault-semester-toggle" onClick={() => toggleSemester(semester.id)}>
                <span>{semester.name}</span>
                <small>{courses.length} courses</small>
              </button>
              {expanded ? (
                <div className="wabi-vault-course-list">
                  {courses.map((course) => (
                    <button
                      key={course.id}
                      type="button"
                      className={course.id === selectedCourseId ? "active" : ""}
                      onClick={() => {
                        if (space === "references") {
                          if (referenceDirty) {
                            setMessage("Save the current reference note before switching courses.");
                            return;
                          }
                          setReferenceSemesterId(semester.id);
                          setReferenceCourseId(course.id);
                          setReferenceEditing(false);
                        } else {
                          setSummarySemesterId(semester.id);
                          setSummaryCourseId(course.id);
                          setSelectedSummaryPath(null);
                        }
                      }}
                    >
                      <span style={{ background: course.color }} />{course.name}
                    </button>
                  ))}
                </div>
              ) : null}
            </div>
          );
        })}
      </aside>
    );
  }

  function renderFieldPlannerTaskModal() {
    if (!fieldPlannerTaskMode) return null;

    const task = fieldPlannerTaskId ? taskLookup.get(fieldPlannerTaskId) ?? null : null;
    const creating = fieldPlannerTaskMode === "create";
    if (!creating && !task) return null;

    const modalCourse = creating ? courseLookup.get(taskDraft.courseId) ?? null : task ? courseLookup.get(task.courseId) ?? null : null;
    const modalSemester = creating ? semesterLookup.get(taskDraft.semesterId) ?? null : task ? semesterLookup.get(task.semesterId) ?? null : null;
    const calc = task ? calculateDailyWork(task) : null;
    const progress = task ? getTaskProgress(task) : 0;
    const editing = fieldPlannerTaskMode === "edit" && task;
    const closeModal = () => {
      setFieldPlannerTaskMode(null);
      setFieldPlannerTaskId(null);
      setEditingTaskId(null);
    };
    const renderTaskForm = () => {
      const draft = creating ? taskDraft : taskEditDraft;
      const setDraft = creating ? setTaskDraft : setTaskEditDraft;
      const submit = (event: FormEvent<HTMLFormElement>) => {
        if (creating) {
          addTask(event);
        } else {
          updateTask(event);
          setFieldPlannerTaskMode("view");
        }
      };

      return (
        <form className="fn-task-form" onSubmit={submit}>
          <div className="fn-task-form-grid">
            <label className="field fn-task-form-title">
              <span>Task title</span>
              <input value={draft.title} onChange={(event) => {
                const title = event.target.value;
                setDraft((current) => {
                  const shouldInfer = !current.unitLabel.trim() || current.unitLabel === "Unit" || current.unitLabel === inferUnitLabel(current.title);
                  return { ...current, title, unitLabel: shouldInfer ? inferUnitLabel(title) : current.unitLabel };
                });
              }} placeholder="Lecture, exercise sheet, reading..." autoFocus />
            </label>
            <label className="field">
              <span>Unit label</span>
              <input value={draft.unitLabel} onChange={(event) => setDraft((current) => ({ ...current, unitLabel: event.target.value }))} />
            </label>
            <label className="field">
              <span>Total units</span>
              <input type="number" min="1" value={draft.totalUnits} onChange={(event) => setDraft((current) => ({ ...current, totalUnits: event.target.value }))} />
            </label>
            <label className="field">
              <span>Done</span>
              <input type="number" min="0" value={draft.completedUnits} onChange={(event) => setDraft((current) => ({ ...current, completedUnits: event.target.value }))} />
            </label>
            <label className="field">
              <span>Priority</span>
              <select value={draft.priority} onChange={(event) => setDraft((current) => ({ ...current, priority: event.target.value as Priority }))}>
                <option value="high">High</option>
                <option value="medium">Medium</option>
                <option value="low">Low</option>
              </select>
            </label>
            <label className="field">
              <span>Due date</span>
              <input type="date" value={draft.dueDate} onChange={(event) => setDraft((current) => ({ ...current, dueDate: event.target.value }))} onKeyDown={confirmTaskDueDate} />
            </label>
            <label className="field fn-task-form-notes">
              <span>Notes</span>
              <textarea value={draft.notes} onChange={(event) => setDraft((current) => ({ ...current, notes: event.target.value }))} placeholder="Definition of done, rubric hints, professor notes..." />
            </label>
          </div>
          <footer className="fn-task-modal-footer">
            <button type="submit">{creating ? "Create task" : "Save task"}</button>
            <button type="button" className="ghost-button" onClick={creating ? closeModal : () => setFieldPlannerTaskMode("view")}>Cancel</button>
            {draft.dueDate ? <button type="button" className="ghost-button" onClick={() => setDraft((current) => ({ ...current, dueDate: "" }))}>Clear due date</button> : null}
          </footer>
        </form>
      );
    };

    return (
      <div className="fn-task-modal-backdrop" onMouseDown={closeModal}>
        <section className="fn-task-modal" role="dialog" aria-modal="true" aria-labelledby="fn-task-modal-title" onMouseDown={(event) => event.stopPropagation()}>
          <header className="fn-task-modal-head">
            <div>
              <p className="fn-stamp-line">Task file · {modalSemester?.name ?? "No semester"} · {modalCourse?.name ?? "No course"}</p>
              <h2 id="fn-task-modal-title">{creating ? "Add task" : editing ? "Edit task" : task?.title}</h2>
            </div>
            <button type="button" className="ghost-button small-button" onClick={closeModal}>Close</button>
          </header>

          {creating || editing ? renderTaskForm() : task && calc ? (
            <>
              <div className="fn-task-modal-ledger">
                <div><span>Progress</span><strong>{task.completedUnits}/{task.totalUnits}</strong><em>{formatUnitLabel(task.unitLabel, task.totalUnits)}</em></div>
                <div><span>Next</span><strong>{getNextUnitLabel(task).replace("Next: ", "")}</strong><em>{task.dueDate ? `due ${formatDate(task.dueDate)}` : "no due date"}</em></div>
                <div><span>Completion</span><strong>{progress}%</strong><em>{calc.unitsPerDay.toFixed(1)} / day</em></div>
              </div>
              <div className="fn-task-modal-progress"><div className="health-track tight wide"><div className="health-fill" style={{ width: `${progress}%`, background: modalCourse?.color ?? "var(--fn-course-blue)" }} /></div></div>
              {task.notes ? <p className="fn-task-modal-note">{task.notes}</p> : null}
              <div className="fn-task-modal-actions">
                <button type="button" onClick={() => adjustTask(task.id, -1)}>-</button>
                <button type="button" onClick={() => adjustTask(task.id, 1)}>+</button>
                <button type="button" className="ghost-button" onClick={() => { startEditingTask(task); setFieldPlannerTaskMode("edit"); }}>Edit</button>
                <button type="button" className="ghost-button" onClick={() => focusTaskFromDashboard(task)}>Focus</button>
                <button type="button" className="mini-danger" onClick={() => removeTask(task.id)}>Remove</button>
              </div>
            </>
          ) : null}
        </section>
      </div>
    );
  }

  function renderFieldPlannerDeleteDialog() {
    if (!fieldPlannerDeleteTarget) return null;

    const title = fieldPlannerDeleteTarget.type === "semester" ? "Deleting semester" : "Deleting course";
    const confirmDelete = () => {
      if (fieldPlannerDeleteTarget.type === "semester") {
        removeSemester(fieldPlannerDeleteTarget.id);
      } else {
        removeCourse(fieldPlannerDeleteTarget.id);
      }
      setFieldPlannerDeleteTarget(null);
    };

    return (
      <div className="fn-delete-dialog-backdrop" onMouseDown={() => setFieldPlannerDeleteTarget(null)}>
        <section className="fn-delete-dialog" role="alertdialog" aria-modal="true" aria-labelledby="fn-delete-title" onMouseDown={(event) => event.stopPropagation()}>
          <p className="fn-stamp-line">{fieldPlannerDeleteTarget.name}</p>
          <h2 id="fn-delete-title">{title}</h2>
          <p>No recovery possible.<br />Are you sure?</p>
          <div className="fn-delete-dialog-actions">
            <button type="button" className="fn-delete-yes" onClick={confirmDelete}>YES</button>
            <button type="button" className="ghost-button" onClick={() => setFieldPlannerDeleteTarget(null)}>NO</button>
          </div>
        </section>
      </div>
    );
  }

  function renderMenuPanel() {
    if (!activeMenuPanel) return null;

    return (
      <div className="settings-panel-backdrop" onMouseDown={closeMenuPanel}>
        <section className="settings-panel" onMouseDown={(event) => event.stopPropagation()}>
          <div className="settings-panel-head">
            <div>
              <p className="eyebrow">Menu</p>
              <h2>{activeMenuPanel === "theme" ? "Theme" : activeMenuPanel === "personal" ? "Personal" : activeMenuPanel === "options" ? "Options" : "Settings"}</h2>
            </div>
            <button type="button" className="ghost-button small-button" onClick={closeMenuPanel}>Close</button>
          </div>

          {activeMenuPanel === "theme" ? (
            <div className="settings-panel-body">
              <div className="theme-panel-switch" aria-label="Appearance picker">
                {(["themes", "styles"] as const).map((view) => (
                  <button key={view} type="button" className={themePanelView === view ? "active" : ""} onClick={() => setThemePanelView(view)}>
                    {view === "themes" ? "Themes" : "Styles"}
                  </button>
                ))}
              </div>
              {themePanelView === "themes" ? (
                appStyle !== "modern" ? (
                  <div className="arena-empty">
                    <strong>Coming soon</strong>
                    <span>{appStyle === "field-notebook" ? "Field Notebook" : "Wabi-Sabi"} doesn't support color themes yet. Switch back to Modern in Styles to pick a palette.</span>
                  </div>
                ) : (
                  <div className="theme-choice-grid">
                    {themePalettes.map((p) => (
                      <button
                        key={p.id}
                        type="button"
                        className={`theme-choice-card ${palette === p.id ? "active" : ""}`}
                        onClick={() => setPalette(p.id)}
                      >
                        <div className="theme-choice-main">
                          <span className="theme-choice-swatch" style={{ background: p.swatch }} />
                          <div>
                            <strong>{p.name}</strong>
                            <span>{p.desc}</span>
                          </div>
                        </div>
                        {palette === p.id ? <span className="design-chip">Active</span> : null}
                      </button>
                    ))}
                  </div>
                )
              ) : (
                <div className="theme-choice-grid style-choice-grid">
                  {appStyles.map((styleOption) => (
                    <button
                      key={styleOption.id}
                      type="button"
                      className={`theme-choice-card style-choice-card ${appStyle === styleOption.id ? "active" : ""}`}
                      onClick={() => setAppStyle(styleOption.id)}
                    >
                      <div className="theme-choice-main">
                        <span className="theme-choice-swatch style-choice-swatch" style={{ background: styleOption.swatch }} />
                        <div>
                          <strong>{styleOption.name}</strong>
                          <span>{styleOption.desc}</span>
                        </div>
                      </div>
                      {appStyle === styleOption.id ? <span className="design-chip">Active</span> : null}
                    </button>
                  ))}
                </div>
              )}
              <div className="theme-mode-row" aria-label="Theme mode">
                {(["light", "dark"] as const).map((mode) => (
                  <button
                    key={mode}
                    type="button"
                    className={theme === mode ? "active" : ""}
                    onClick={() => setTheme(mode)}
                  >
                    {mode === "light" ? "Light" : "Dark"}
                  </button>
                ))}
              </div>
            </div>
          ) : null}

          {activeMenuPanel === "personal" ? (
            <form className="settings-panel-body" onSubmit={savePersonalSettings}>
              <label className="field">
                <span>Your name</span>
                <input
                  value={personalNameDraft}
                  onChange={(event) => setPersonalNameDraft(event.target.value)}
                  placeholder="e.g. Damcha"
                />
              </label>
              <label className="field">
                <span>Daily focus goal (hours)</span>
                <input
                  type="number"
                  min="0.25"
                  max="24"
                  step="0.25"
                  value={personalDailyGoalHoursDraft}
                  onChange={(event) => setPersonalDailyGoalHoursDraft(event.target.value)}
                  placeholder="2"
                />
              </label>
              <div className="inline-form-actions">
                <button type="submit">Save personal settings</button>
                <button type="button" className="ghost-button" onClick={() => setPersonalNameDraft("")}>Clear name</button>
              </div>
            </form>
          ) : null}

          {activeMenuPanel === "options" ? (
            <div className="settings-panel-body options-panel-body">
              <div className="tab-toggle-list">
                <label className="tab-toggle-row">
                  <span>
                    <strong>Background effect</strong>
                    <small>{state.settings.backgroundEffect === false ? "Off" : "On"} · ambient gradients and animated glows</small>
                  </span>
                  <input
                    type="checkbox"
                    checked={state.settings.backgroundEffect !== false}
                    onChange={toggleBackgroundEffect}
                  />
                  <span className="ios-switch" aria-hidden="true" />
                </label>
                <label className="tab-toggle-row">
                  <span>
                    <strong>Feed images</strong>
                    <small>{state.settings.hideFeedImages ? "Hidden" : "Shown"} · social feed post images on this device</small>
                  </span>
                  <input
                    type="checkbox"
                    checked={!state.settings.hideFeedImages}
                    onChange={toggleFeedImages}
                  />
                  <span className="ios-switch" aria-hidden="true" />
                </label>
                <label className="tab-toggle-row">
                  <span>
                    <strong>Feed polls</strong>
                    <small>{state.settings.hideFeedPolls ? "Hidden" : "Shown"} · social feed polls on this device</small>
                  </span>
                  <input
                    type="checkbox"
                    checked={!state.settings.hideFeedPolls}
                    onChange={toggleFeedPolls}
                  />
                  <span className="ios-switch" aria-hidden="true" />
                </label>
                <label className="tab-toggle-row">
                  <span>
                    <strong>Info button</strong>
                    <small>{state.settings.showHelpButton === false ? "Hidden" : "Shown"} · the ? help button in the tab bar</small>
                  </span>
                  <input
                    type="checkbox"
                    checked={state.settings.showHelpButton !== false}
                    onChange={toggleHelpButton}
                  />
                  <span className="ios-switch" aria-hidden="true" />
                </label>
              </div>
              <button
                type="button"
                className="options-intro-card options-dropdown-trigger"
                aria-expanded={visibleTabsOptionsOpen}
                onClick={() => setVisibleTabsOptionsOpen((current) => !current)}
              >
                <span>
                  <strong>Choose visible tabs</strong>
                  <span>Turn off any tab you do not want in the main navigation. The app adapts as if it was never there.</span>
                </span>
                <span className="options-dropdown-chevron" aria-hidden="true">›</span>
              </button>
              {visibleTabsOptionsOpen ? (
                <div className="tab-toggle-list">
                  {primaryTabs.map((tab) => {
                    const visibleTabs = state.settings.visibleTabs ?? defaultState.settings.visibleTabs;
                    const checked = visibleTabs[tab.id] !== false;
                    const visibleCount = primaryTabs.filter(({ id }) => visibleTabs[id] !== false).length;
                    const disabled = checked && visibleCount <= 1;

                    return (
                      <label key={tab.id} className={`tab-toggle-row ${disabled ? "disabled" : ""}`}>
                        <span>
                          <strong>{tab.label}</strong>
                          <small>{checked ? "Shown in tabs" : "Hidden from tabs"}</small>
                        </span>
                        <input
                          type="checkbox"
                          checked={checked}
                          disabled={disabled}
                          onChange={() => toggleTabVisibility(tab.id)}
                        />
                        <span className="ios-switch" aria-hidden="true" />
                      </label>
                    );
                  })}
                </div>
              ) : null}
            </div>
          ) : null}

          {activeMenuPanel === "settings" ? (
            <div className="settings-panel-body settings-danger-zone">
              <div className="tab-toggle-list">
                <label className="tab-toggle-row">
                  <span>
                    <strong>Anonymous usage telemetry</strong>
                    <small>{state.settings.telemetryEnabled ? "On" : "Off"} · shares only app version, platform, runtime channel, and last-opened time</small>
                  </span>
                  <input
                    type="checkbox"
                    checked={state.settings.telemetryEnabled}
                    onChange={toggleTelemetry}
                  />
                  <span className="ios-switch" aria-hidden="true" />
                </label>
              </div>
              <div className="linux-update-command-card">
                <div>
                  <strong>Privacy</strong>
                  <span>What social sync collects for abuse prevention, and how long it's kept.</span>
                </div>
                <div className="update-actions">
                  <button type="button" className="ghost-button" onClick={() => setPrivacyOpen(true)}>View privacy policy</button>
                </div>
              </div>
              <div className={`settings-update-card ${updateInfo.status}`}>
                <div>
                  <strong>Updates</strong>
                  <span>Current version: {currentAppVersion}</span>
                  <p>{updateInfo.message}</p>
                </div>
                <div className="update-actions">
                  <button type="button" className="ghost-button" onClick={() => void checkForUpdates()} disabled={updateChecking}>
                    {updateChecking && updateInfo.status !== "installing" ? "Checking..." : "Check for updates"}
                  </button>
                  {updateInfo.status === "available" && updateInstallSupport.canAutoInstall ? (
                    <button type="button" onClick={() => void installPendingUpdate()} disabled={updateChecking}>
                      Install update
                    </button>
                  ) : null}
                  {updateInfo.status === "available" && !updateInstallSupport.canAutoInstall && updateInstallSupport.packageHint === "manual-linux" ? (
                    <button type="button" onClick={() => void downloadManualLinuxUpdate()} disabled={linuxPackageDownloading}>
                      {linuxPackageDownloading ? "Downloading..." : "Download package"}
                    </button>
                  ) : null}
                  <button type="button" className="ghost-button" onClick={() => openExternalLink(updateInfo.releaseUrl ?? RELEASES_PAGE_URL)}>
                    {updateInfo.status === "available" && (updateInstallSupport.packageHint === "source-linux" || updateInstallSupport.packageHint === "source-build") ? "Open repository" : updateInfo.status === "available" && !updateInstallSupport.canAutoInstall ? "Download manually" : "Open release page"}
                  </button>
                  {canViewR2Usage ? (
                    <button type="button" className="ghost-button" onClick={() => void sendUpdateNoticeToUsers()} disabled={updateNoticeSending || !canSendUpdateNotice}>
                      {updateNoticeSending ? "Notifying..." : canSendUpdateNotice ? `Notify users below ${currentAppVersion}` : "Version not ready"}
                    </button>
                  ) : null}
                </div>
              </div>
              {canViewR2Usage ? (
                <div className="linux-update-command-card">
                  <div>
                    <strong>Usage and versions</strong>
                    <span>Owner-only view of synced users plus anonymous opt-in telemetry installs.</span>
                    {adminUsage?.summary ? (
                      <span>{Number(adminUsage.summary.userCount ?? 0)} synced users · {Number(adminUsage.summary.active24h ?? 0)} active 24h · {Number(adminUsage.summary.active7d ?? 0)} active 7d · {adminUsage.telemetry.length} opt-in installs</span>
                    ) : null}
                  </div>
                  <div className="update-actions">
                    <button type="button" onClick={() => void loadAdminUsage()} disabled={adminUsageLoading}>
                      {adminUsageLoading ? "Loading..." : "Load usage data"}
                    </button>
                  </div>
                </div>
              ) : null}
              {linuxUpdateDownload ? (
                <div className="linux-update-command-card">
                  <div>
                    <strong>Install downloaded update</strong>
                    <span>{linuxUpdateDownload.message}</span>
                    <span>Saved to: {linuxUpdateDownload.filePath}</span>
                  </div>
                  <textarea className="linux-update-command" value={linuxUpdateDownload.installCommand} readOnly rows={linuxUpdateDownload.installCommand.includes("\n") ? 2 : 1} />
                  <div className="update-actions">
                    <button type="button" onClick={() => void copyLinuxInstallCommand()}>Copy command</button>
                    <button type="button" className="ghost-button" onClick={() => void revealLinuxPackage()}>Open downloads folder</button>
                    <button type="button" className="ghost-button" onClick={() => setLinuxUpdateDownload(null)}>Close</button>
                  </div>
                </div>
              ) : null}
              {updateInstallSupport.packageHint === "development" ? (
                <div className="linux-update-command-card">
                  <div>
                    <strong>Development build updates disabled</strong>
                    <span>This prevents a dev app from installing a release app and switching localStorage locations.</span>
                    <span>Replace <code>/path/to/destudydracker</code> with your local repository path.</span>
                  </div>
                  <textarea className="linux-update-command" value={DEV_UPDATE_COMMAND} readOnly rows={5} />
                  <div className="update-actions">
                    <button type="button" onClick={() => void copyDevUpdateCommand()}>Copy commands</button>
                    <button type="button" className="ghost-button" onClick={() => openExternalLink("https://github.com/damcha02/destudydracker")}>Open repository</button>
                  </div>
                </div>
              ) : null}
              {updateInfo.status === "available" && (updateInstallSupport.packageHint === "source-linux" || updateInstallSupport.packageHint === "source-build") ? (
                <div className="linux-update-command-card">
                  <div>
                    <strong>Update from source</strong>
                    <span>{updateInstallSupport.packageHint === "source-build" ? "This source build cannot install release updates automatically." : "Recommended for Arch, Hyprland-heavy setups, and unsupported Linux distributions."}</span>
                    <span>Replace <code>/path/to/destudydracker</code> with your local repository path.</span>
                  </div>
                  <textarea className="linux-update-command" value={SOURCE_LINUX_UPDATE_COMMAND} readOnly rows={6} />
                  <div className="update-actions">
                    <button type="button" onClick={() => void copySourceLinuxUpdateCommand()}>Copy commands</button>
                    <button type="button" className="ghost-button" onClick={() => openExternalLink("https://github.com/damcha02/destudydracker")}>Open repository</button>
                  </div>
                </div>
              ) : null}
              <div className="delete-data-card">
                <div>
                  <strong>Delete all data</strong>
                  <span>This clears semesters, courses, tasks, calendar entries, exams, sessions, exports, and personal info.</span>
                </div>
                {deleteConfirmOpen ? (
                  <div className="delete-confirm-row">
                    <span>Are you sure?</span>
                    <button type="button" className="ghost-button small-button" onClick={() => setDeleteConfirmOpen(false)}>No</button>
                    <button type="button" className="danger-button" onClick={deleteAllData}>Yes, delete</button>
                  </div>
                ) : (
                  <button type="button" className="danger-button" onClick={() => setDeleteConfirmOpen(true)}>Delete all data</button>
                )}
              </div>
            </div>
          ) : null}
        </section>
      </div>
    );
  }

  // Keep macOS traffic lights native; Linux and Windows use the compact app bar.
  const showWindowTitlebar = isTauriApp() && !/Macintosh|Mac OS X/.test(navigator.userAgent);
  const openHelp = helpTab ? pageHelp[helpTab] : null;
  const activeTourHelp = tourState ? pageHelp[tourState.tab] : null;

  return (
    <>
      {showWindowTitlebar ? (
        <div className="window-titlebar" onMouseDown={() => void startWindowDrag()}>
          <div className="window-titlebar-title">Study Tracker</div>
          <div className="window-titlebar-controls" onMouseDown={(event) => event.stopPropagation()}>
            <button type="button" onClick={() => void minimizeWindow()} aria-label="Minimize window" title="Minimize">
              <span aria-hidden="true">-</span>
            </button>
            <button type="button" onClick={() => void toggleMaximizeWindow()} aria-label={windowMaximized ? "Restore window" : "Maximize window"} title={windowMaximized ? "Restore window" : "Maximize window"}>
              <span aria-hidden="true">{windowMaximized ? "❐" : "□"}</span>
            </button>
            <button type="button" onClick={() => void toggleFullscreenWindow()} aria-label="Enter or exit fullscreen" title="Enter or exit fullscreen">
              <span aria-hidden="true">⛶</span>
            </button>
            <button type="button" className="window-close-button" onClick={() => void closeWindow()} aria-label="Close window" title="Close">
              <span aria-hidden="true">×</span>
            </button>
          </div>
        </div>
      ) : null}

      <div className={`shell ${showWindowTitlebar ? "with-window-titlebar" : ""}`} style={{ "--accent": state.settings.accent } as CSSProperties}>
      {appStyle === "wabi-sabi" ? renderWabiSidebar() : null}
      {appStyle === "wabi-sabi" && wabiQuietMode ? renderWabiQuietMode() : null}
      <header className="topbar">
        <div className="brand-cluster">
          <TimerBrandMark phase={timerBrandPhase(state.timer)} />
          <div>
            <h1>Study Tracker</h1>
            <p className="subtitle">Semester planning, focus tracking, workload clarity, and study notes.</p>
          </div>
        </div>

        <div className="topbar-actions">
          <div
            className={`health-pill ${healthState}`}
            title="Heuristic score based on task progress, overdue work, and exam pressure."
            style={{ display: "flex", alignItems: "center", gap: 12, padding: "7px 8px 7px 14px", minWidth: 0 }}
          >
            <div>
              <div className="eyebrow" style={{ fontSize: 8.5, marginBottom: 1 }}>Overall score</div>
              <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
                <span style={{ width: 7, height: 7, borderRadius: 99, background: scoreColor, flexShrink: 0, display: "inline-block" }} />
                <span style={{ fontSize: 12.5, fontWeight: 700, color: scoreColor }}>{healthLabel}</span>
              </div>
            </div>
            <div style={{ position: "relative", width: 42, height: 42, flexShrink: 0 }}>
              <svg width="42" height="42" style={{ transform: "rotate(-90deg)" }}>
                <circle cx="21" cy="21" r="18.5" fill="none" stroke="var(--ring-track)" strokeWidth="5" />
                <circle
                  cx="21" cy="21" r="18.5" fill="none" stroke={scoreColor} strokeWidth="5"
                  strokeDasharray="116.24"
                  strokeDashoffset={116.24 - (overallHealth / 100) * 116.24}
                  strokeLinecap="round"
                  style={{ transition: "stroke-dashoffset 0.7s cubic-bezier(0.2,0.7,0.3,1)" }}
                />
              </svg>
              <div style={{ position: "absolute", inset: 0, display: "flex", alignItems: "center", justifyContent: "center" }}>
                <span style={{ fontFamily: "var(--font-mono)", fontSize: 14, fontWeight: 700, color: "var(--ink)" }}>{overallHealth}</span>
              </div>
            </div>
          </div>
          <button
            className="theme-toggle"
            onClick={() => setTheme((current) => (current === "dark" ? "light" : "dark"))}
            type="button"
            title="Toggle theme"
            style={{ display: "grid", placeItems: "center" }}
          >
            {theme === "dark" ? (
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="4"/>
                <path d="M12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.5 1.5M17.5 17.5 19 19M19 5l-1.5 1.5M6.5 17.5 5 19"/>
              </svg>
            ) : (
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                <path d="M21 12.8A8.5 8.5 0 1 1 11.2 3 6.5 6.5 0 0 0 21 12.8z"/>
              </svg>
            )}
          </button>
          {state.settings.showHelpButton !== false ? (
            <button type="button" className="page-help-button menu-help-button" onClick={openMenuHelp} aria-label="Open menu help" title="Menu help">
              ?
            </button>
          ) : null}
          <div className="topbar-menu-wrap">
            <button
              type="button"
              className="hamburger-button"
              aria-label="Open menu"
              aria-expanded={menuOpen}
              onClick={() => setMenuOpen((current) => !current)}
            >
              <span />
              <span />
              <span />
            </button>
            {menuOpen ? (
              <div className="topbar-menu" role="menu">
                <button
                  type="button"
                  role="menuitem"
                  onClick={() => {
                    downloadBackup(state);
                    setMenuOpen(false);
                  }}
                >
                  Backup JSON
                </button>
                <button type="button" role="menuitem" onClick={() => openMenuPanel("theme")}>Change theme</button>
                <button type="button" role="menuitem" onClick={() => openMenuPanel("personal")}>Personal</button>
                <button type="button" role="menuitem" onClick={() => openMenuPanel("options")}>Options</button>
                <button type="button" role="menuitem" onClick={() => openMenuPanel("settings")}>Settings</button>
              </div>
            ) : null}
          </div>
        </div>
      </header>

      {renderMenuPanel()}

      {updateNoticeVisible || appAnnouncement || feedCommentNotice || endlessInactivityPrompt || timerInactivityNoticeVisible ? (
        <div className="notice-stack" aria-live="polite">
          {updateNoticeVisible ? (
            <aside className="update-notice" role="status">
              <div>
                <strong>New update available.</strong>
                <span>
                  Go to <button type="button" onClick={openUpdateSettingsFromNotice}>Settings</button> to update the app.
                </span>
              </div>
              <button type="button" className="update-notice-close" onClick={() => setUpdateNoticeVisible(false)} aria-label="Dismiss update notice">
                X
              </button>
            </aside>
          ) : null}

          {appAnnouncement ? (
            <aside className="update-notice" role="status">
              <div>
                <strong>{appAnnouncement.title}</strong>
                <span>{appAnnouncement.body}</span>
              </div>
              <button type="button" className="update-notice-close" onClick={() => dismissAppAnnouncement(appAnnouncement.id)} aria-label="Dismiss announcement">
                X
              </button>
            </aside>
          ) : null}

          {feedCommentNotice ? (
            <aside className="update-notice comment-notice" role="status">
              <div>
                <strong>New comment on your post.</strong>
                <span>
                  {feedCommentNotice.commenterName}: "{feedCommentNotice.body.length > 72 ? `${feedCommentNotice.body.slice(0, 72)}...` : feedCommentNotice.body}" <button type="button" onClick={() => openFeedCommentNotice(feedCommentNotice)}>View</button>
                </span>
              </div>
              <button type="button" className="update-notice-close" onClick={() => setFeedCommentNotice(null)} aria-label="Dismiss comment notice">
                X
              </button>
            </aside>
          ) : null}

          {endlessInactivityPrompt ? (
            <aside className="update-notice timer-inactivity-warning" role="alert" aria-live="assertive">
              <div>
                <strong>Are you still here?</strong>
                <span>The endless timer will stop in <b>{endlessInactivityCountdownLabel}</b> unless you confirm. It will save only the time before this warning. <button type="button" onClick={acknowledgeEndlessInactivityPrompt}>I'm here</button></span>
              </div>
              <button type="button" className="update-notice-close" onClick={acknowledgeEndlessInactivityPrompt} aria-label="Confirm you are still studying">
                X
              </button>
            </aside>
          ) : null}

          {timerInactivityNoticeVisible ? (
            <aside className="update-notice timer-inactivity-notice" role="alert" aria-live="assertive">
              <div>
                <strong>Timer was stopped due to inactivity.</strong>
                <span>The endless timer saved only the time before the inactivity warning.</span>
              </div>
              <button type="button" className="update-notice-close" onClick={() => setTimerInactivityNoticeVisible(false)} aria-label="Dismiss inactivity notice">
                X
              </button>
            </aside>
          ) : null}
        </div>
      ) : null}

      <nav className="tab-row" aria-label="Primary navigation">
        <div className="tab-row-scroll">
          {primaryTabs.filter(({ id }) => (state.settings.visibleTabs ?? defaultState.settings.visibleTabs)[id] !== false).map(({ id: key, label }) => (
            <button
              key={key}
              className={`tab-button ${state.activeTab === key ? "active" : ""}`}
              onClick={() => setActiveTab(key)}
              type="button"
            >
              <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                {key === "dashboard" && <path d="M4 13h6V4H4zM14 20h6v-9h-6zM14 7h6V4h-6zM4 20h6v-3H4z" />}
                {key === "planner" && <><rect x="3" y="4" width="18" height="17" rx="2" /><path d="M3 9h18M8 2v4M16 2v4M7 14h5M7 17h8" /></>}
                {key === "timer" && <><circle cx="12" cy="13" r="8" /><path d="M12 13V9M12 5V3M9 3h6" /></>}
                {key === "vault" && <><path d="M12 2 4 6v6c0 5 3.4 8 8 10 4.6-2 8-5 8-10V6z" /><path d="M9 12l2 2 4-4" /></>}
                {key === "friends" && <><circle cx="9" cy="8" r="3" /><path d="M3 20c.8-3.4 3-5 6-5s5.2 1.6 6 5" /><path d="M16 11a2.5 2.5 0 1 0 0-5" /><path d="M17 15c2.1.5 3.4 2 4 5" /></>}
                {key === "break" && <><path d="M6 12h4M14 12h4M8 10V8M16 10V8" /><rect x="2" y="7" width="20" height="12" rx="3" /></>}
              </svg>
              {label}{key === "friends" && hasUnreadSocial ? <span className="nav-badge" /> : null}
            </button>
          ))}
        </div>
        {state.settings.showHelpButton !== false ? (
          <button type="button" className="page-help-button tab-help-button" onClick={() => openPageHelp(state.activeTab)} aria-label={`Open ${pageHelp[state.activeTab].title} help`} title={`${pageHelp[state.activeTab].title} help`}>
            ?
          </button>
        ) : null}
      </nav>

      {message ? <div className="message-banner">{message}</div> : null}

      {renderCalendarDayOverlay()}

      {openHelp ? (
        <div className="help-modal-backdrop" onMouseDown={() => setHelpTab(null)}>
          <section className="help-modal" onMouseDown={(event) => event.stopPropagation()} role="dialog" aria-modal="true" aria-label={`${openHelp.title} help`}>
            <div className="help-modal-head">
              <div>
                <p className="eyebrow">Page guide</p>
                <h2>{openHelp.title}</h2>
              </div>
              <button type="button" className="ghost-button small-button" onClick={() => setHelpTab(null)} aria-label="Close page guide">X</button>
            </div>
            <p className="help-purpose">{openHelp.purpose}</p>
            <div className="help-section">
              <h3>Start here</h3>
              <ol>
                {openHelp.steps.map((step) => <li key={step}>{step}</li>)}
              </ol>
            </div>
            {helpTab === "planner" ? <HelpExampleCards examples={helpExamples.planner} /> : null}
            <div className="help-modal-actions">
              <button type="button" className="ghost-button" onClick={() => setHelpTab(null)}>Close</button>
              <button type="button" className="help-tutorial-button" onClick={() => startPageTour(helpTab!)}>Start tutorial <span aria-hidden="true">-&gt;</span></button>
            </div>
          </section>
        </div>
      ) : null}

      {menuHelpOpen ? (
        <div className="help-modal-backdrop" onMouseDown={() => setMenuHelpOpen(false)}>
          <section className="help-modal" onMouseDown={(event) => event.stopPropagation()} role="dialog" aria-modal="true" aria-label="Menu help">
            <div className="help-modal-head">
              <div>
                <p className="eyebrow">App menu</p>
                <h2>What the menu does</h2>
              </div>
              <button type="button" className="ghost-button small-button" onClick={() => setMenuHelpOpen(false)} aria-label="Close menu help">X</button>
            </div>
            <p className="help-purpose">Use the top-right menu for app-wide settings, backups, appearance, updates, and optional simplification. Page-specific help lives in the tab-bar question mark.</p>
            <div className="menu-help-list">
              {menuHelpItems.map((item) => (
                <article key={item.title} className="menu-help-item">
                  <strong>{item.title}</strong>
                  <span>{item.body}</span>
                </article>
              ))}
            </div>
            <div className="help-modal-actions">
              <button type="button" className="ghost-button" onClick={() => setMenuHelpOpen(false)}>Close</button>
            </div>
          </section>
        </div>
      ) : null}

      {adminUsageOpen && adminUsage ? (
        <div className="help-modal-backdrop" onMouseDown={() => setAdminUsageOpen(false)}>
          <section className="help-modal admin-usage-modal" onMouseDown={(event) => event.stopPropagation()} role="dialog" aria-modal="true" aria-label="Usage and versions report">
            <div className="help-modal-head">
              <div>
                <p className="eyebrow">Owner report</p>
                <h2>Usage and versions</h2>
              </div>
              <button type="button" className="ghost-button small-button" onClick={() => setAdminUsageOpen(false)} aria-label="Close usage report">X</button>
            </div>
            <p className="help-purpose">Synced users, latest devices, app versions, and anonymous opt-in telemetry installs.</p>
            <div className="admin-usage-summary-grid">
              <article className="admin-usage-stat">
                <span>Synced users</span>
                <strong>{Number(adminUsage.summary?.userCount ?? 0)}</strong>
              </article>
              <article className="admin-usage-stat">
                <span>Active 24h</span>
                <strong>{Number(adminUsage.summary?.active24h ?? 0)}</strong>
              </article>
              <article className="admin-usage-stat">
                <span>Active 7d</span>
                <strong>{Number(adminUsage.summary?.active7d ?? 0)}</strong>
              </article>
              <article className="admin-usage-stat">
                <span>Opt-in installs</span>
                <strong>{adminUsage.telemetry.length}</strong>
              </article>
              <article className="admin-usage-stat admin-usage-stat-alert">
                <span>Flagged</span>
                <strong>{Number(adminUsage.summary?.flaggedCount ?? 0)}</strong>
              </article>
            </div>
            <div className="admin-usage-section">
              <div className="admin-usage-section-head">
                <h3>Synced Users</h3>
                <span>{adminUsage.users.length} latest rows</span>
              </div>
              <div className="admin-usage-table-wrap">
                <table className="admin-usage-table">
                  <thead>
                    <tr>
                      <th>User</th>
                      <th>Last Seen</th>
                      <th>Version</th>
                      <th>Latest Device</th>
                      <th>Channel</th>
                    </tr>
                  </thead>
                  <tbody>
                    {adminUsage.users.map((user) => (
                      <tr key={user.friendCode}>
                        <td data-label="User">
                          <strong>{user.displayName || "Unknown"}</strong>
                          <span>{user.friendCode}</span>
                          {user.isFlagged ? <span className="admin-usage-flag-badge" title={user.flaggedReason ?? ""}>Flagged</span> : null}
                        </td>
                        <td data-label="Last Seen">{formatAdminTimestamp(user.lastSeenAt)}</td>
                        <td data-label="Version">{user.appVersion || "unknown"}</td>
                        <td data-label="Latest Device">
                          <span className="admin-usage-device-pill">{formatLatestDevice(user)}</span>
                        </td>
                        <td data-label="Channel">{user.appRuntimeChannel || "unknown"}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
            <div className="admin-usage-section">
              <div className="admin-usage-section-head">
                <h3>Flagged Accounts</h3>
                <span>{adminUsage.flaggedUsers.length} rows</span>
              </div>
              <div className="admin-usage-table-wrap">
                <table className="admin-usage-table">
                  <thead>
                    <tr>
                      <th>User</th>
                      <th>Reason</th>
                      <th>Country</th>
                      <th>Network</th>
                      <th>Last Seen</th>
                    </tr>
                  </thead>
                  <tbody>
                    {adminUsage.flaggedUsers.map((user) => (
                      <tr key={user.friendCode}>
                        <td data-label="User">
                          <strong>{user.displayName || "Unknown"}</strong>
                          <span>{user.friendCode}</span>
                        </td>
                        <td data-label="Reason">{user.flaggedReason || "—"}</td>
                        <td data-label="Country">{user.signupCountry || "unknown"}</td>
                        <td data-label="Network">{user.signupAsOrganization || "unknown"}</td>
                        <td data-label="Last Seen">{formatAdminTimestamp(user.lastSeenAt)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
            <div className="admin-usage-section">
              <div className="admin-usage-section-head">
                <h3>Opt-In Telemetry Installs</h3>
                <span>{adminUsage.telemetry.length} latest rows</span>
              </div>
              <div className="admin-usage-table-wrap">
                <table className="admin-usage-table">
                  <thead>
                    <tr>
                      <th>Install</th>
                      <th>Last Seen</th>
                      <th>First Seen</th>
                      <th>Version</th>
                      <th>Platform</th>
                      <th>Channel</th>
                    </tr>
                  </thead>
                  <tbody>
                    {adminUsage.telemetry.map((install) => (
                      <tr key={install.installId}>
                        <td data-label="Install"><span className="admin-usage-device-pill">{shortAdminId(install.installId)}</span></td>
                        <td data-label="Last Seen">{formatAdminTimestamp(install.lastSeenAt)}</td>
                        <td data-label="First Seen">{formatAdminTimestamp(install.createdAt)}</td>
                        <td data-label="Version">{install.appVersion || "unknown"}</td>
                        <td data-label="Platform">{install.appPlatform || "unknown"}</td>
                        <td data-label="Channel">{install.appRuntimeChannel || "unknown"}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
            <div className="admin-usage-section">
              <div className="admin-usage-section-head">
                <h3>Recent Abuse Events</h3>
                <span>{adminUsage.abuseEvents.length} rows</span>
              </div>
              <div className="admin-usage-table-wrap">
                <table className="admin-usage-table">
                  <thead>
                    <tr>
                      <th>Type</th>
                      <th>Path</th>
                      <th>Country</th>
                      <th>Network</th>
                      <th>User</th>
                      <th>When</th>
                    </tr>
                  </thead>
                  <tbody>
                    {adminUsage.abuseEvents.map((event, index) => (
                      <tr key={`${event.createdAt}-${index}`}>
                        <td data-label="Type">{event.eventType}</td>
                        <td data-label="Path">{event.path || "—"}</td>
                        <td data-label="Country">{event.country || "unknown"}</td>
                        <td data-label="Network">{event.asOrganization || "unknown"}</td>
                        <td data-label="User">{event.userId || "—"}</td>
                        <td data-label="When">{formatAdminTimestamp(event.createdAt)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
            <div className="help-modal-actions">
              <button type="button" className="ghost-button" onClick={() => void loadAdminUsage()} disabled={adminUsageLoading}>{adminUsageLoading ? "Refreshing..." : "Refresh"}</button>
              <button type="button" onClick={() => setAdminUsageOpen(false)}>Close</button>
            </div>
          </section>
        </div>
      ) : null}

      {privacyOpen ? (
        <div className="help-modal-backdrop" onMouseDown={() => setPrivacyOpen(false)}>
          <section className="help-modal" onMouseDown={(event) => event.stopPropagation()} role="dialog" aria-modal="true" aria-label="Privacy policy">
            <div className="help-modal-head">
              <div>
                <p className="eyebrow">Social sync</p>
                <h2>Privacy</h2>
              </div>
              <button type="button" className="ghost-button small-button" onClick={() => setPrivacyOpen(false)} aria-label="Close privacy policy">X</button>
            </div>
            <pre className="privacy-policy-text">{privacyPolicyText}</pre>
            <div className="help-modal-actions">
              <button type="button" onClick={() => setPrivacyOpen(false)}>Close</button>
            </div>
          </section>
        </div>
      ) : null}

      {tourState && activeTourHelp ? (
        <GuidedTourOverlay
          help={activeTourHelp}
          stepIndex={tourState.step}
          onBack={() => setTourState((current) => current ? { ...current, step: Math.max(0, current.step - 1) } : current)}
          onNext={() => setTourState((current) => {
            if (!current) return current;
            const nextStep = current.step + 1;
            return nextStep >= pageHelp[current.tab].tour.length ? null : { ...current, step: nextStep };
          })}
          onClose={() => setTourState(null)}
        />
      ) : null}

      {state.activeTab === "dashboard" && appStyle === "field-notebook" ? renderFieldNotebookDashboard() : null}

      {state.activeTab === "dashboard" && appStyle === "wabi-sabi" ? renderWabiSabiDashboard() : null}

      {state.activeTab === "dashboard" && appStyle === "modern" ? (
        <section className="dashboard-design fade-up">
          <div className="dashboard-design-head">
            <div>
              <h2>{dashboardGreeting}</h2>
              <p>Here's what deserves your focus today.</p>
            </div>
            <div className="layout-control" aria-label="Dashboard layout" data-tour="dashboard-layout">
              <span className="eyebrow">Layout</span>
              {([
                ["focus", "Focus"],
                ["cockpit", "Cockpit"],
                ["analyst", "Analyst"],
                ["custom", "Custom"],
              ] as [DashboardLayout, string][]).map(([key, label]) => (
                <button
                  key={key}
                  type="button"
                  className={dashboardLayout === key ? "active" : ""}
                  onClick={() => setDashboardLayout(key)}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>

          {dashboardLayout === "cockpit" ? (<>
            <div className="dash-3col">
              <div className="design-stack">
                {renderTodayCard()}
                {renderGardenCard()}
              </div>
              <div className="design-stack">
                {renderUrgentTasks(4)}
                {renderWeeklyChart()}
              </div>
              <div className="design-stack">
                {renderCourseRadar()}
                {renderExamRunway()}
              </div>
            </div>
          </>) : null}

          {dashboardLayout === "focus" ? (<>
            <div className="dash-focus-claude">
              <div className="dash-focus-top-row">
                {renderTodayCard()}
                {renderUrgentTasks(4)}
              </div>
              {renderGardenCard("hero-garden")}
              <div className="dash-focus-bottom-row">
                {renderWeeklyChart("short-weekly")}
                {renderExamRunway()}
              </div>
            </div>
          </>) : null}

          {dashboardLayout === "analyst" ? (
            <div className="design-stack">
              <div className="dash-stats-grid">
                {renderStatCard("Focused today", formatMinutes(todayMinutes), "Logged study time")}
                {renderStatCard("This week", formatMinutes(weeklyTotalMinutes), "Last 7 days")}
                {renderStatCard("Day streak", streakDays, "Consecutive active days", "warm")}
                {renderStatCard("Open tasks", openTaskCount, "Still in progress")}
              </div>
              <div className="dash-analyst-grid">
                <div className="design-stack">
                  {renderWeeklyChart()}
                  {renderUrgentTasks(4)}
                </div>
                <div className="design-stack">
                  {renderCourseRadar()}
                  {renderExamRunway()}
                </div>
              </div>
            </div>
          ) : null}

          {dashboardLayout === "custom" ? renderCustomDashboard() : null}

          {state.sessions.length > 0 ? (
            <p className="long-now">{formatMinutes(totalAllTimeMinutes)} across {sessionDays.size} day{sessionDays.size !== 1 ? "s" : ""} since {formatDate(firstSessionDate!)}</p>
          ) : (
            <p className="long-now dim">Your timeline begins when you complete your first session.</p>
          )}
        </section>
      ) : null}

      {state.activeTab === "planner" ? (
        <section className={`planner-stack ${appStyle === "field-notebook" ? "fn-planner" : appStyle === "wabi-sabi" ? "wabi-planner" : ""}`}>
          {appStyle === "field-notebook" ? (
            <aside className="fn-planner-rail" aria-label="Planner folders">
              <div className="fn-rail-section">
                <div className="fn-rail-label">Semesters</div>
                {state.semesters.map((semester) => {
                  const courses = getSemesterCourses(state, semester.id);
                  const tasks = getSemesterTasks(state, semester.id);
                  const active = expandedSemesterIds.includes(semester.id);
                  return (
                    <div key={semester.id} className="fn-semester-tree">
                      <button type="button" className={`fn-rail-row fn-semester-folder ${active ? "active" : ""}`} onClick={() => toggleSemester(semester.id)}>
                        <strong>{semester.name}</strong>
                        <span>{courses.length} courses · {tasks.length} tasks</span>
                      </button>
                      {active ? (
                        <div className="fn-course-tree">
                          {courses.map((course) => {
                            const courseTasks = getCourseTasks(state, course.id);
                            const courseActive = expandedCourseIds.includes(course.id);
                            return (
                              <div key={course.id} className="fn-course-tree-item">
                                <button type="button" className={`fn-course-folder ${courseActive ? "active" : ""}`} style={{ "--fn-course": course.color } as CSSProperties} onClick={() => toggleCourse(course.id)}>
                                  <strong>{course.name}</strong>
                                  <span>{courseTasks.length}</span>
                                </button>
                                {courseActive ? (
                                  <div className="fn-task-tree">
                                    {courseTasks.length ? courseTasks.map((task) => (
                                      <button key={task.id} type="button" className={`fn-sidebar-task ${fieldPlannerTaskId === task.id ? "active" : ""}`} onClick={() => {
                                        setSelectedTaskId(task.id);
                                        setFieldPlannerTaskId(task.id);
                                        setFieldPlannerTaskMode("view");
                                      }}>
                                        <span>{task.title}</span>
                                        <em>{task.completedUnits}/{task.totalUnits}</em>
                                      </button>
                                    )) : <p className="fn-sidebar-empty">No tasks yet.</p>}
                                    <button type="button" className="fn-rail-link fn-sidebar-add-task" onClick={() => {
                                      setTaskDraft((current) => ({ ...current, semesterId: semester.id, courseId: course.id, title: "", unitLabel: "Unit", totalUnits: "10", completedUnits: "0", dueDate: "", priority: "medium", notes: "" }));
                                      setFieldPlannerTaskId(null);
                                      setFieldPlannerTaskMode("create");
                                    }}>+ add task</button>
                                    <div className="fn-sidebar-actions">
                                      <button type="button" onClick={() => startEditingCourse(course)}>edit course</button>
                                      <button type="button" className="danger" onClick={() => setFieldPlannerDeleteTarget({ type: "course", id: course.id, name: course.name })}>remove course</button>
                                    </div>
                                    {editingCourseId === course.id ? (
                                      <form className="fn-sidebar-form" onSubmit={updateCourse}>
                                        <input value={courseEditDraft.name} onChange={(event) => setCourseEditDraft((current) => ({ ...current, name: event.target.value }))} placeholder="Course name" />
                                        <div className="fn-sidebar-form-grid">
                                          <select value={courseEditDraft.targetGrade} onChange={(event) => setCourseEditDraft((current) => ({ ...current, targetGrade: event.target.value }))}>
                                            {swissGrades.map((grade) => <option key={grade} value={grade.toString()}>{formatSwissGrade(grade)}</option>)}
                                          </select>
                                          <input aria-label="Course color" type="color" value={courseEditDraft.color} onChange={(event) => setCourseEditDraft((current) => ({ ...current, color: event.target.value }))} />
                                        </div>
                                        <div className="fn-sidebar-form-actions">
                                          <button type="submit">Save</button>
                                          <button type="button" onClick={() => setEditingCourseId(null)}>Cancel</button>
                                        </div>
                                      </form>
                                    ) : null}
                                  </div>
                                ) : null}
                              </div>
                            );
                          })}
                          {addingCourseSemesterId === semester.id ? (
                            <form className="fn-sidebar-form" onSubmit={addCourse}>
                              <input value={courseDraft.name} onChange={(event) => setCourseDraft((current) => ({ ...current, semesterId: semester.id, name: event.target.value }))} placeholder="Course name" />
                              <div className="fn-sidebar-form-grid">
                                <select value={courseDraft.targetGrade} onChange={(event) => setCourseDraft((current) => ({ ...current, semesterId: semester.id, targetGrade: event.target.value }))}>
                                  {swissGrades.map((grade) => <option key={grade} value={grade.toString()}>{formatSwissGrade(grade)}</option>)}
                                </select>
                                <input aria-label="Course color" type="color" value={courseDraft.color} onChange={(event) => setCourseDraft((current) => ({ ...current, semesterId: semester.id, color: event.target.value }))} />
                              </div>
                              <div className="fn-sidebar-form-actions">
                                <button type="submit">Add</button>
                                <button type="button" onClick={() => setAddingCourseSemesterId(null)}>Cancel</button>
                              </div>
                            </form>
                          ) : null}
                          <button type="button" className="fn-rail-link fn-sidebar-add-course" onClick={() => {
                            setCourseDraft((current) => ({ ...current, semesterId: semester.id }));
                            setAddingCourseSemesterId((current) => (current === semester.id ? null : semester.id));
                          }}>+ add course</button>
                          <div className="fn-sidebar-actions fn-semester-actions">
                            <button type="button" onClick={() => startEditingSemester(semester)}>edit semester</button>
                            <button type="button" className="danger" onClick={() => setFieldPlannerDeleteTarget({ type: "semester", id: semester.id, name: semester.name })}>remove semester</button>
                          </div>
                          {editingSemesterId === semester.id ? (
                            <form className="fn-sidebar-form" onSubmit={updateSemester}>
                              <input value={semesterEditName} onChange={(event) => setSemesterEditName(event.target.value)} placeholder="Semester name" />
                              <div className="fn-sidebar-form-actions">
                                <button type="submit">Save</button>
                                <button type="button" onClick={() => setEditingSemesterId(null)}>Cancel</button>
                              </div>
                            </form>
                          ) : null}
                        </div>
                      ) : null}
                    </div>
                  );
                })}
                {showSemesterForm ? (
                  <form className="fn-sidebar-form" onSubmit={addSemester}>
                    <input value={semesterName} onChange={(event) => setSemesterName(event.target.value)} placeholder="Semester name" />
                    <div className="fn-sidebar-form-actions">
                      <button type="submit">Add</button>
                      <button type="button" onClick={() => setShowSemesterForm(false)}>Cancel</button>
                    </div>
                  </form>
                ) : null}
                <button type="button" className="fn-rail-link" onClick={() => setShowSemesterForm((current) => !current)}>+ new semester</button>
                <section className="fn-sidebar-workload" aria-label="Workload calculator">
                  <div className="fn-rail-label">Workload</div>
                  <label>
                    <span>Selection</span>
                    <select value={fieldPlannerWorkloadId} onChange={(event) => setFieldPlannerWorkloadId(event.target.value)}>
                      <option value={TOTAL_WORKLOAD_ID}>Total workload</option>
                      {state.courses.map((course) => (
                        <option key={course.id} value={course.id}>{course.name}</option>
                      ))}
                    </select>
                  </label>
                  <div className="fn-workload-card">
                    <strong>{fieldPlannerWorkload.label}</strong>
                    <p>{fieldPlannerWorkload.message}</p>
                    <div className="fn-workload-track" aria-label={`${fieldPlannerWorkload.progress}% complete`}>
                      <i style={{ width: `${fieldPlannerWorkload.progress}%` }} />
                    </div>
                    <div className="fn-workload-ledger">
                      <span>Left <b>{fieldPlannerWorkload.remainingUnits}</b></span>
                      <span>Done <b>{fieldPlannerWorkload.progress}%</b></span>
                      <span>Days <b>{fieldPlannerWorkload.daysLeft ?? "set"}</b></span>
                      <span>Pace <b>{fieldPlannerWorkload.unitsPerDay.toFixed(1)}/d</b></span>
                    </div>
                    {fieldPlannerWorkload.nearestDueDate ? <em>Nearest deadline {formatDate(fieldPlannerWorkload.nearestDueDate)}</em> : null}
                  </div>
                </section>
              </div>
            </aside>
          ) : null}
          {appStyle === "wabi-sabi" ? renderPlannerCalendar() : null}
          <article className="panel-card planner-board-panel" data-tour="planner-semesters">
            <div className="section-head planner-header">
              <div>
                <p className="eyebrow">{appStyle === "wabi-sabi" ? "Plan" : "Planner"}</p>
                <h2>{appStyle === "wabi-sabi" ? `${openTaskCount} thing${openTaskCount === 1 ? "" : "s"} growing` : "Semesters, courses, and tasks"}</h2>
                <p className="section-note">{appStyle === "wabi-sabi" ? "Open a semester to manage its courses. Select a course to edit it." : "Click a semester or course to expand it. Click it again to collapse."}</p>
              </div>
              <div className="page-head-actions">
                <button type="button" className="ghost-button" data-tour="planner-add-semester" onClick={() => setShowSemesterForm((current) => !current)}>
                  {showSemesterForm ? "Close" : "+ Add semester"}
                </button>
              </div>
            </div>

            {showSemesterForm ? (
              <form className="inline-form-card" onSubmit={addSemester}>
                <label className="field">
                  <span>Semester name</span>
                  <input data-tour="planner-semester-name" value={semesterName} onChange={(event) => setSemesterName(event.target.value)} placeholder="e.g. Semester 1 2026" />
                </label>
                <button type="submit" data-tour="planner-create-semester">Create semester</button>
              </form>
            ) : null}

            <div className="semester-board roomy-top">
              {state.semesters.length ? (
                state.semesters.map((semester) => {
                  const courses = getSemesterCourses(state, semester.id);
                  const tasks = semesterTasksBySemesterId.get(semester.id) ?? [];
                  // Non-null: semesterHealthBySemesterId is built by iterating this same
                  // state.semesters, so every semester.id is guaranteed a key.
                  const semesterHealth = semesterHealthBySemesterId.get(semester.id)!;
                  const semesterExpanded = expandedSemesterIds.includes(semester.id);
                  const semesterExams = state.exams.filter((exam) => exam.semesterId === semester.id);

                  return (
                    <section key={semester.id} className={`semester-card ${semesterExpanded ? "open" : ""}`} data-tour={semester.id === TUTORIAL_SEMESTER_ID ? "planner-tutorial-semester" : undefined}>
                      <div className="semester-header-row">
                        <button type="button" className="accordion-toggle semester-toggle" onClick={() => toggleSemester(semester.id)}>
                          <span className="accordion-title-group">
                            <strong>{semester.name}</strong>
                            <small>
                              {courses.length} courses • {tasks.length} tasks • {tasks.filter((task) => getRemainingUnits(task) > 0).length} active • {semesterHealth.label}
                            </small>
                          </span>
                        </button>

                        <div className="accordion-actions">
                          <div className="mini-health" data-tour={semester.id === TUTORIAL_SEMESTER_ID ? "planner-tutorial-semester-health" : undefined}>
                            <strong>{semesterHealth.score}</strong>
                            <span>{semesterHealth.label}</span>
                          </div>
                          <button
                            type="button"
                            className="ghost-button small-button"
                            data-tour="planner-add-course"
                            onClick={() => {
                              setExpandedSemesterIds((current) => (current.includes(semester.id) ? current : [...current, semester.id]));
                              setCourseDraft((current) => ({ ...current, semesterId: semester.id }));
                              setAddingCourseSemesterId((current) => (current === semester.id ? null : semester.id));
                            }}
                          >
                            + Course
                          </button>
                          <button type="button" className="ghost-button small-button" onClick={() => startEditingSemester(semester)}>
                            Edit
                          </button>
                          <button type="button" className="mini-danger" data-tour={semester.id === TUTORIAL_SEMESTER_ID ? "planner-tutorial-semester-remove" : undefined} onClick={() => removeSemester(semester.id)}>
                            Remove
                          </button>
                        </div>
                      </div>

                      {editingSemesterId === semester.id ? (
                        <form className="inline-form-card nested-form semester-edit-form" onSubmit={updateSemester}>
                          <label className="field">
                            <span>Semester name</span>
                            <input value={semesterEditName} onChange={(event) => setSemesterEditName(event.target.value)} />
                          </label>
                          <div className="inline-form-actions">
                            <button type="submit">Save semester</button>
                            <button type="button" className="ghost-button" onClick={() => setEditingSemesterId(null)}>
                              Cancel
                            </button>
                          </div>
                        </form>
                      ) : null}

                      {semesterExpanded ? (
                        <div className="accordion-body">
                          {addingCourseSemesterId === semester.id ? (
                            <form className="inline-form-card nested-form" onSubmit={addCourse}>
                              <div className="inline-form-grid inline-form-grid-course">
                                <label className="field">
                                  <span>Course name</span>
                                  <input
                                    data-tour="planner-course-name"
                                    value={courseDraft.name}
                                    onChange={(event) => setCourseDraft((current) => ({ ...current, name: event.target.value, semesterId: semester.id }))}
                                    placeholder="e.g. Numerical Methods"
                                  />
                                </label>
                                <label className="field">
                                  <span>Target grade</span>
                                  <select data-tour="planner-course-target" value={courseDraft.targetGrade} onChange={(event) => setCourseDraft((current) => ({ ...current, semesterId: semester.id, targetGrade: event.target.value }))}>
                                    {swissGrades.map((grade) => (
                                      <option key={grade} value={grade.toString()}>
                                        {formatSwissGrade(grade)}
                                      </option>
                                    ))}
                                  </select>
                                </label>
                                <label className="field">
                                  <span>Color</span>
                                  <input type="color" value={courseDraft.color} onChange={(event) => setCourseDraft((current) => ({ ...current, color: event.target.value, semesterId: semester.id }))} />
                                </label>
                              </div>
                              <div className="inline-form-actions">
                                <button type="submit" data-tour="planner-course-submit">Add course</button>
                                <button type="button" className="ghost-button" onClick={() => setAddingCourseSemesterId(null)}>
                                  Cancel
                                </button>
                              </div>
                            </form>
                          ) : null}

                          <div className="course-stack">
                            {courses.length ? (
                              courses.map((course) => {
                                const courseTasks = courseTasksByCourseId.get(course.id) ?? [];
                                // Non-null: courseHealthByCourseId is built by iterating this
                                // same state.courses, so every course.id is guaranteed a key.
                                const health = courseHealthByCourseId.get(course.id)!;
                                const courseExpanded = expandedCourseIds.includes(course.id);
                                const completedCourseTasks = courseTasks.filter((task) => getRemainingUnits(task) === 0).length;
                                const courseTotalUnits = courseTasks.reduce((sum, task) => sum + Math.max(1, task.totalUnits), 0);
                                const courseCompletedUnits = courseTasks.reduce((sum, task) => sum + Math.min(Math.max(0, task.completedUnits), Math.max(1, task.totalUnits)), 0);
                                const courseProgress = courseTotalUnits ? Math.round((courseCompletedUnits / courseTotalUnits) * 100) : 0;

                                return (
                                  <article key={course.id} className={`course-sheet ${courseExpanded ? "open" : ""}`} data-tour={course.id === TUTORIAL_COURSE_ID ? "planner-tutorial-course" : undefined}>
                                    <div className="course-header-row">
                                      <button type="button" className="accordion-toggle course-toggle" onClick={() => toggleCourse(course.id)}>
                                        <div className="course-toggle-main">
                                          <div className="course-chip" style={{ background: course.color }} />
                                           <span className="accordion-title-group">
                                             <strong>{course.name}</strong>
                                             <small>
                                              {appStyle === "wabi-sabi" ? `${completedCourseTasks} of ${courseTasks.length} done` : `Target ${formatSwissGrade(course.targetGrade)} • ${courseTasks.length} tasks • ${health.label}`}
                                             </small>
                                           </span>
                                           {appStyle === "wabi-sabi" ? <span className="wabi-course-progress" aria-label={`${courseProgress}% complete`}><i style={{ width: `${courseProgress}%`, background: course.color }} /></span> : null}
                                         </div>
                                      </button>

                                      <div className="accordion-actions course-actions">
                                        <div className="course-action-row">
                                          <button
                                            type="button"
                                            className="ghost-button small-button"
                                            data-tour="planner-add-task"
                                            onClick={() => {
                                              setExpandedSemesterIds((current) => (current.includes(semester.id) ? current : [...current, semester.id]));
                                              setExpandedCourseIds((current) => (current.includes(course.id) ? current : [...current, course.id]));
                                              setTaskDraft((current) => ({ ...current, semesterId: semester.id, courseId: course.id }));
                                              setAddingTaskCourseId((current) => (current === course.id ? null : course.id));
                                            }}
                                          >
                                            + Task
                                          </button>
                                          <button
                                            type="button"
                                            className="ghost-button small-button"
                                            onClick={() => {
                                              setExpandedSemesterIds((current) => (current.includes(semester.id) ? current : [...current, semester.id]));
                                              setExpandedCourseIds((current) => (current.includes(course.id) ? current : [...current, course.id]));
                                              setExamDraft((current) => ({ ...current, semesterId: semester.id, courseId: course.id }));
                                              setAddingExamCourseId((current) => (current === course.id ? null : course.id));
                                            }}
                                          >
                                            + Exam
                                          </button>
                                        </div>
                                        <div className="course-action-row">
                                          <button type="button" className="ghost-button small-button" onClick={() => startEditingCourse(course)}>
                                            Edit
                                          </button>
                                          <button type="button" className="mini-danger" data-tour={course.id === TUTORIAL_COURSE_ID ? "planner-tutorial-course-remove" : undefined} onClick={() => removeCourse(course.id)}>
                                            Remove
                                          </button>
                                        </div>
                                      </div>
                                    </div>

                                    {editingCourseId === course.id ? (
                                      <form className="inline-form-card nested-form course-edit-form" onSubmit={updateCourse}>
                                        <div className="inline-form-grid inline-form-grid-course">
                                          <label className="field">
                                            <span>Course name</span>
                                            <input value={courseEditDraft.name} onChange={(event) => setCourseEditDraft((current) => ({ ...current, name: event.target.value }))} />
                                          </label>
                                          <label className="field">
                                            <span>Target grade</span>
                                            <select value={courseEditDraft.targetGrade} onChange={(event) => setCourseEditDraft((current) => ({ ...current, targetGrade: event.target.value }))}>
                                              {swissGrades.map((grade) => (
                                                <option key={grade} value={grade.toString()}>
                                                  {formatSwissGrade(grade)}
                                                </option>
                                              ))}
                                            </select>
                                          </label>
                                          <label className="field">
                                            <span>Color</span>
                                            <input type="color" value={courseEditDraft.color} onChange={(event) => setCourseEditDraft((current) => ({ ...current, color: event.target.value }))} />
                                          </label>
                                        </div>
                                        <div className="inline-form-actions">
                                          <button type="submit">Save course</button>
                                          <button type="button" className="ghost-button" onClick={() => setEditingCourseId(null)}>
                                            Cancel
                                          </button>
                                        </div>
                                      </form>
                                    ) : null}

                                    {courseExpanded ? (
                                      <div className="accordion-body nested-body">
                                        {addingTaskCourseId === course.id ? (
                                          <form className="inline-form-card nested-form" onSubmit={addTask}>
                                            <div className="inline-form-grid inline-form-grid-task">
                                              <label className="field task-title-field">
                                                <span>Task title</span>
                                                <input
                                                  data-tour="planner-task-title"
                                                  value={taskDraft.title}
                                                  onChange={(event) => setTaskDraft((current) => {
                                                    const title = event.target.value;
                                                    const shouldInfer = !current.unitLabel.trim() || current.unitLabel === "Unit" || current.unitLabel === inferUnitLabel(current.title);
                                                    return { ...current, semesterId: semester.id, courseId: course.id, title, unitLabel: shouldInfer ? inferUnitLabel(title) : current.unitLabel };
                                                  })}
                                                  placeholder="Sheet 4, reading pack, chapter summary..."
                                                />
                                              </label>
                                              <label className="field">
                                                <span>Unit label</span>
                                                <input data-tour="planner-task-unit-label" value={taskDraft.unitLabel} onChange={(event) => setTaskDraft((current) => ({ ...current, semesterId: semester.id, courseId: course.id, unitLabel: event.target.value }))} placeholder="Lecture, Sheet, Exam..." />
                                              </label>
                                              <label className="field">
                                                <span>Total units</span>
                                                <input data-tour="planner-task-total" type="number" min="1" value={taskDraft.totalUnits} onChange={(event) => setTaskDraft((current) => ({ ...current, semesterId: semester.id, courseId: course.id, totalUnits: event.target.value }))} />
                                              </label>
                                              <label className="field">
                                                <span>Done</span>
                                                <input data-tour="planner-task-done" type="number" min="0" value={taskDraft.completedUnits} onChange={(event) => setTaskDraft((current) => ({ ...current, semesterId: semester.id, courseId: course.id, completedUnits: event.target.value }))} />
                                              </label>
                                              <label className="field">
                                                <span>Priority</span>
                                                <select data-tour="planner-task-priority" value={taskDraft.priority} onChange={(event) => setTaskDraft((current) => ({ ...current, semesterId: semester.id, courseId: course.id, priority: event.target.value as Priority }))}>
                                                  <option value="high">High</option>
                                                  <option value="medium">Medium</option>
                                                  <option value="low">Low</option>
                                                </select>
                                              </label>
                                              <label className="field">
                                                <span>Due date (optional)</span>
                                                <input
                                                  type="date"
                                                  data-tour="planner-task-due"
                                                  value={taskDraft.dueDate}
                                                  onChange={(event) => setTaskDraft((current) => ({ ...current, semesterId: semester.id, courseId: course.id, dueDate: event.target.value }))}
                                                  onKeyDown={confirmTaskDueDate}
                                                />
                                              </label>
                                              <label className="field task-notes-field">
                                                <span>Notes</span>
                                                <textarea data-tour="planner-task-notes" value={taskDraft.notes} onChange={(event) => setTaskDraft((current) => ({ ...current, semesterId: semester.id, courseId: course.id, notes: event.target.value }))} placeholder="Definition of done, rubric hints, professor notes..." />
                                              </label>
                                            </div>
                                            <p className="unit-label-hint">One unit means one piece of work: Lecture 1 of 10, Sheet 3 of 14, Exam 2 of 5.</p>
                                            <div className="inline-form-actions">
                                              <button type="submit" data-tour="planner-task-submit">Add task</button>
                                              <button type="button" className="ghost-button" onClick={() => setAddingTaskCourseId(null)}>
                                                Cancel
                                              </button>
                                              {taskDraft.dueDate ? (
                                                <button type="button" className="ghost-button" onClick={() => setTaskDraft((current) => ({ ...current, dueDate: "" }))}>
                                                  Clear due date
                                                </button>
                                              ) : null}
                                            </div>
                                          </form>
                                        ) : null}

                                        {addingExamCourseId === course.id ? (
                                          <form className="inline-form-card nested-form" onSubmit={addExam}>
                                            <div className="inline-form-grid inline-form-grid-exam course-exam-form-grid">
                                              <label className="field">
                                                <span>Exam title</span>
                                                <input value={examDraft.title} onChange={(event) => setExamDraft((current) => ({ ...current, semesterId: semester.id, courseId: course.id, title: event.target.value }))} placeholder="Midterm, final, oral..." />
                                              </label>
                                              <label className="field">
                                                <span>Date</span>
                                                <input type="date" value={examDraft.examDate} onChange={(event) => setExamDraft((current) => ({ ...current, semesterId: semester.id, courseId: course.id, examDate: event.target.value }))} />
                                              </label>
                                              <label className="field">
                                                <span>Weight %</span>
                                                <input type="number" min="0" max="100" value={examDraft.weight} onChange={(event) => setExamDraft((current) => ({ ...current, semesterId: semester.id, courseId: course.id, weight: event.target.value }))} />
                                              </label>
                                              <label className="field">
                                                <span>Preparedness %</span>
                                                <input type="number" min="0" max="100" value={examDraft.preparedness} onChange={(event) => setExamDraft((current) => ({ ...current, semesterId: semester.id, courseId: course.id, preparedness: event.target.value }))} />
                                              </label>
                                            </div>
                                            <div className="inline-form-actions">
                                              <button type="submit">Add exam</button>
                                              <button type="button" className="ghost-button" onClick={() => setAddingExamCourseId(null)}>
                                                Cancel
                                              </button>
                                            </div>
                                          </form>
                                        ) : null}

                                        <div className="task-table">
                                          {courseTasks.length ? (
                                            courseTasks.map((task) => {
                                              const calc = calculateDailyWork(task);
                                              const progress = getTaskProgress(task);
                                              return (
                                                <div key={task.id}>
                                                  <div className={`task-row-card ${selectedTaskId === task.id ? "selected" : ""} ${getRemainingUnits(task) === 0 ? "complete" : ""}`} data-tour={task.id === TUTORIAL_TASK_ID ? "planner-tutorial-task" : "planner-task-row"}>
                                                    <div className="task-row-main">
                                                      <button type="button" className="link-button task-title-button" onClick={() => setSelectedTaskId(task.id)}>
                                                        <strong>{task.title}</strong>
                                                      </button>
                                                      <p data-tour={task.id === TUTORIAL_TASK_ID ? "planner-tutorial-task-meta" : undefined}>
                                                        {task.completedUnits}/{task.totalUnits} {formatUnitLabel(task.unitLabel, task.totalUnits)} • {getNextUnitLabel(task)} • {task.dueDate ? `due ${formatDate(task.dueDate)}` : "no due date"}
                                                      </p>
                                                    </div>
                                                    <div className="task-row-progress">
                                                      <div className="progress-pill-row">
                                                        <span>{progress}%</span>
                                                        <span>{calc.unitsPerDay.toFixed(1)} / day</span>
                                                      </div>
                                                      <div className="health-track tight wide">
                                                        <div className="health-fill" style={{ width: `${progress}%`, background: course.color }} />
                                                      </div>
                                                    </div>
                                                    <div className="task-row-actions" data-tour={task.id === TUTORIAL_TASK_ID ? "planner-tutorial-task-progress" : "planner-task-progress"}>
                                                      <button type="button" onClick={() => adjustTask(task.id, -1)}>
                                                        -
                                                      </button>
                                                      <button type="button" onClick={() => adjustTask(task.id, 1)}>
                                                        +
                                                      </button>
                                                      <button type="button" className="ghost-button small-button" onClick={() => startEditingTask(task)}>
                                                        Edit
                                                      </button>
                                                      <button
                                                        type="button"
                                                        className="ghost-button small-button"
                                                        data-tour={task.id === TUTORIAL_TASK_ID ? "planner-tutorial-task-focus" : "planner-task-focus"}
                                                        onClick={() => {
                                                          setSelectedTaskId(task.id);
                                                          setState((current) => ({
                                                            ...current,
                                                            activeTab: "timer",
                                                            timer: {
                                                              ...current.timer,
                                                              semesterId: task.semesterId,
                                                              courseId: task.courseId,
                                                              taskId: task.id,
                                                              goal: current.timer.goal || task.title,
                                                            },
                                                          }));
                                                        }}
                                                      >
                                                        Focus
                                                      </button>
                                                      <button type="button" className="mini-danger" data-tour={task.id === TUTORIAL_TASK_ID ? "planner-tutorial-task-remove" : undefined} onClick={() => removeTask(task.id)}>
                                                        Remove
                                                      </button>
                                                    </div>
                                                  </div>
                                                  {editingTaskId === task.id ? (
                                                    <form className="inline-form-card nested-form task-edit-form" onSubmit={updateTask}>
                                                      <div className="inline-form-grid inline-form-grid-task">
                                                        <label className="field task-title-field">
                                                          <span>Task title</span>
                                                          <input value={taskEditDraft.title} onChange={(event) => setTaskEditDraft((current) => {
                                                            const title = event.target.value;
                                                            const shouldInfer = !current.unitLabel.trim() || current.unitLabel === "Unit" || current.unitLabel === inferUnitLabel(current.title);
                                                            return { ...current, title, unitLabel: shouldInfer ? inferUnitLabel(title) : current.unitLabel };
                                                          })} />
                                                        </label>
                                                        <label className="field">
                                                          <span>Unit label</span>
                                                          <input value={taskEditDraft.unitLabel} onChange={(event) => setTaskEditDraft((current) => ({ ...current, unitLabel: event.target.value }))} placeholder="Lecture, Sheet, Exam..." />
                                                        </label>
                                                        <label className="field">
                                                          <span>Total units</span>
                                                          <input type="number" min="1" value={taskEditDraft.totalUnits} onChange={(event) => setTaskEditDraft((current) => ({ ...current, totalUnits: event.target.value }))} />
                                                        </label>
                                                        <label className="field">
                                                          <span>Done</span>
                                                          <input type="number" min="0" value={taskEditDraft.completedUnits} onChange={(event) => setTaskEditDraft((current) => ({ ...current, completedUnits: event.target.value }))} />
                                                        </label>
                                                        <label className="field">
                                                          <span>Priority</span>
                                                          <select value={taskEditDraft.priority} onChange={(event) => setTaskEditDraft((current) => ({ ...current, priority: event.target.value as Priority }))}>
                                                            <option value="high">High</option>
                                                            <option value="medium">Medium</option>
                                                            <option value="low">Low</option>
                                                          </select>
                                                        </label>
                                                        <label className="field">
                                                          <span>Due date (optional)</span>
                                                          <input
                                                            type="date"
                                                            value={taskEditDraft.dueDate}
                                                            onChange={(event) => setTaskEditDraft((current) => ({ ...current, dueDate: event.target.value }))}
                                                            onKeyDown={confirmTaskDueDate}
                                                          />
                                                        </label>
                                                        <label className="field task-notes-field">
                                                          <span>Notes</span>
                                                          <textarea value={taskEditDraft.notes} onChange={(event) => setTaskEditDraft((current) => ({ ...current, notes: event.target.value }))} />
                                                        </label>
                                                      </div>
                                                      <p className="unit-label-hint">This controls labels like {cleanUnitLabel(taskEditDraft.unitLabel, taskEditDraft.title)} {Math.min(Math.max(1, Number(taskEditDraft.completedUnits) + 1 || 1), Math.max(1, Number(taskEditDraft.totalUnits) || 1))} of {Math.max(1, Number(taskEditDraft.totalUnits) || 1)}.</p>
                                                      <div className="inline-form-actions">
                                                        <button type="submit">Save task</button>
                                                        <button type="button" className="ghost-button" onClick={() => setEditingTaskId(null)}>
                                                          Cancel
                                                        </button>
                                                        {taskEditDraft.dueDate ? (
                                                          <button type="button" className="ghost-button" onClick={() => setTaskEditDraft((current) => ({ ...current, dueDate: "" }))}>
                                                            Clear due date
                                                          </button>
                                                        ) : null}
                                                      </div>
                                                    </form>
                                                  ) : null}
                                                </div>
                                              );
                                            })
                                          ) : (
                                            <p className="empty-copy compact-empty">No tasks in this course yet.</p>
                                          )}
                                        </div>
                                      </div>
                                    ) : null}
                                  </article>
                                );
                              })
                            ) : (
                              <p className="empty-copy">No courses yet inside this semester.</p>
                            )}
                          </div>

                          <div className="semester-exam-panel">
                            <div className="section-head compact-head">
                              <div>
                                <p className="eyebrow">Assessment</p>
                                <h3>Exams in this semester</h3>
                              </div>
                            </div>

                            <div className="stack-list compact">
                              {semesterExams.length ? (
                                semesterExams.map((exam) => (
                                  <div key={exam.id} className="exam-row detailed">
                                    <div>
                                      <strong>{exam.title}</strong>
                                      <p>{courseLookup.get(exam.courseId)?.name ?? "No course"}</p>
                                    </div>
                                    <div className="exam-side">
                                      <span>{daysUntil(exam.examDate)} days</span>
                                      <small>{exam.weight}% of grade</small>
                                    </div>
                                    <button type="button" className="mini-danger" onClick={() => removeExam(exam.id)}>
                                      Remove
                                    </button>
                                  </div>
                                ))
                              ) : (
                                <p className="empty-copy compact-empty">No exams added for this semester.</p>
                              )}
                            </div>
                          </div>
                        </div>
                      ) : null}
                    </section>
                  );
                })
              ) : (
                <p className="empty-copy">Use the + button above to add your first semester.</p>
              )}
            </div>
          </article>

          {appStyle !== "wabi-sabi" ? renderPlannerCalendar() : null}

          <div className="planner-support-grid">
            <article className="panel-card calculator-card">
              <button type="button" className="accordion-toggle support-toggle" onClick={() => setCalculatorOpen((current) => !current)}>
                <span className="accordion-title-group">
                  <strong>Workload Calculator</strong>
                  <small>{calculatorOpen ? "Click to collapse" : "Click to expand"}</small>
                </span>
              </button>

              {calculatorOpen ? (
                <div className="accordion-body nested-body">
                  <label className="field">
                    <span>Selection</span>
                    <select value={selectedTaskId ?? ""} onChange={(event) => setSelectedTaskId(event.target.value)}>
                      <option value="">Select task</option>
                      <option value={TOTAL_WORKLOAD_ID}>Total workload</option>
                      {state.tasks.map((task) => (
                        <option key={task.id} value={task.id}>
                          {task.title}
                        </option>
                      ))}
                    </select>
                  </label>

                  {isTotalWorkloadSelected ? (
                    <div className="calculator-result roomy-box">
                      <strong>Total workload</strong>
                      <p>{totalWorkload.message}</p>
                      <div className="calculator-grid single-column-metrics">
                        <div>
                          <span>Remaining</span>
                          <strong>{totalWorkload.remainingUnits} units</strong>
                        </div>
                        <div>
                          <span>Days left</span>
                          <strong>{totalWorkload.daysLeft ?? "set due dates"}</strong>
                        </div>
                        <div>
                          <span>Target pace</span>
                          <strong>{totalWorkload.unitsPerDay.toFixed(1)} / day</strong>
                        </div>
                      </div>
                    </div>
                  ) : selectedTask && selectedTaskCalc ? (
                    <div className="calculator-result roomy-box">
                      <strong>{selectedTask.title}</strong>
                      <p>{selectedTaskCalc.message}</p>
                      <div className="calculator-grid single-column-metrics">
                        <div>
                          <span>Remaining</span>
                          <strong>{getRemainingUnits(selectedTask)} {formatUnitLabel(selectedTask.unitLabel, getRemainingUnits(selectedTask))}</strong>
                        </div>
                        <div>
                          <span>Days left</span>
                          <strong>{selectedTaskCalc.daysLeft ?? "set due date"}</strong>
                        </div>
                        <div>
                          <span>Target pace</span>
                          <strong>{selectedTaskCalc.unitsPerDay.toFixed(1)} / day</strong>
                        </div>
                      </div>
                    </div>
                  ) : (
                    <p className="empty-copy">Choose a task and the app will estimate the daily pace needed to finish it.</p>
                  )}
                </div>
              ) : null}
            </article>

            <article className="panel-card completion-card">
              <div className="section-head compact-headline">
                <div>
                  <p className="eyebrow">Completion</p>
                  <h2>Percent done</h2>
                </div>
              </div>

              {isTotalWorkloadSelected || selectedTask ? (
                <div className="completion-ring-panel">
                  <div className="completion-ring-wrap">
                    <svg className="completion-ring" viewBox="0 0 140 140" role="img" aria-label={`${selectedTaskProgress}% complete`}>
                      <circle className="completion-ring-track" cx="70" cy="70" r={completionRadius} />
                      <circle
                        className="completion-ring-value"
                        cx="70"
                        cy="70"
                        r={completionRadius}
                        strokeDasharray={completionCircumference}
                        strokeDashoffset={completionOffset}
                      />
                    </svg>
                    <div className="completion-ring-label">
                      <strong>{selectedTaskProgress}%</strong>
                      <span>done</span>
                    </div>
                  </div>

                  <div className="completion-copy">
                    {isTotalWorkloadSelected ? (
                      <>
                        <strong>Total workload</strong>
                        <p>{totalWorkload.completedUnits}/{totalWorkload.totalUnits} units complete</p>
                        <small>
                          {totalWorkload.nearestDueDate ? `Nearest deadline ${formatDate(totalWorkload.nearestDueDate)}` : "No shared due date"}
                        </small>
                      </>
                    ) : selectedTask ? (
                      <>
                        <strong>{selectedTask.title}</strong>
                        <p>{selectedTask.completedUnits}/{selectedTask.totalUnits} units complete</p>
                        <small>
                          {courseLookup.get(selectedTask.courseId)?.name ?? "No course"} • {selectedTask.dueDate ? `Due ${formatDate(selectedTask.dueDate)}` : "No due date"}
                        </small>
                      </>
                    ) : null}
                  </div>
                </div>
              ) : (
                <p className="empty-copy">Pick a task in the calculator to see its completion ring.</p>
              )}
            </article>
          </div>

          <article className="panel-card semester-overview-card">
            <div className="section-head compact-headline">
              <div>
                <p className="eyebrow">Semester Overview</p>
                <h2>Exams by semester</h2>
              </div>
            </div>

            <div className="stack-list compact">
              {state.semesters.length ? (
                state.semesters.map((semester) => {
                  const courses = getSemesterCourses(state, semester.id);
                  const exams = state.exams.filter((exam) => exam.semesterId === semester.id);
                  const nextExam = [...exams].filter((exam) => daysUntil(exam.examDate) >= 0).sort((a, b) => daysUntil(a.examDate) - daysUntil(b.examDate))[0] ?? null;
                  return (
                    <div key={semester.id} className="overview-row detailed-overview">
                      <div>
                        <strong>{semester.name}</strong>
                        <p>{courses.length} courses • {exams.length} exams</p>
                        {nextExam ? <small>Next: {nextExam.title} in {daysUntil(nextExam.examDate)} days</small> : <small>No upcoming exams</small>}
                      </div>
                      <div className="overview-side exam-overview-side">
                        {exams.length ? exams.slice(0, 3).map((exam) => (
                          <span key={exam.id}>{exam.title}</span>
                        )) : <span>No exams yet</span>}
                      </div>
                    </div>
                  );
                })
              ) : (
                <p className="empty-copy">Your semesters will appear here once you add them.</p>
              )}
            </div>
          </article>
          {appStyle === "field-notebook" ? renderFieldPlannerTaskModal() : null}
          {appStyle === "field-notebook" ? renderFieldPlannerDeleteDialog() : null}
        </section>
      ) : null}

      {state.activeTab === "timer" && appStyle === "wabi-sabi" ? renderWabiTimer() : null}

      {state.activeTab === "timer" && appStyle !== "wabi-sabi" ? (
        <section className={`timer-grid ${appStyle === "field-notebook" ? "fn-timer" : ""}`}>
          <article className="panel-card timer-main-card">
            <div className="section-head">
              <div>
                <p className="eyebrow">Focus Engine</p>
                <h2>Start a focus block</h2>
              </div>
              <div className="page-head-actions">
                <span className="section-note">Keep the default view simple. Add course, task, and reflection only when you need it.</span>
              </div>
            </div>

            <div className="timer-board roomy-top">
              <div className="timer-preset-cards" data-tour="timer-presets">
                {focusPresets.map((preset) => (
                  <button
                    key={preset.label}
                    type="button"
                    className={`timer-preset-card ${state.timer.presetLabel === preset.label ? "active" : ""}`}
                    disabled={state.timer.running}
                    onClick={() => applyPreset(preset.label, preset.study, preset.breakMinutes, preset.mode)}
                  >
                    <strong>{preset.label.replace(" 25/5", "").replace(" 52/17", "").replace(" 90/20", "")}</strong>
                    <span>{preset.mode === "endless" ? "∞" : preset.mode === "exam" ? `${preset.study} min` : `${preset.study} / ${preset.breakMinutes}`}</span>
                  </button>
                ))}
                <button
                  type="button"
                  className={`timer-preset-card ${isCustomTimerPreset ? "active" : ""}`}
                  disabled={state.timer.running}
                  onClick={() =>
                    setState((current) => ({
                      ...current,
                      timer: {
                        ...current.timer,
                        presetLabel: "Custom",
                        mode: "focus",
                        studyMinutes: defaultTimer.studyMinutes,
                        breakMinutes: defaultTimer.breakMinutes,
                        phase: "idle",
                        running: false,
                        startedAt: null,
                        endsAt: null,
                        remainingSeconds: defaultTimer.studyMinutes * 60,
                        loggedSplitSeconds: 0,
                        activeSegments: [],
                      },
                    }))
                  }
                >
                  <strong>Custom</strong>
                  <span>Custom</span>
                </button>
              </div>

              {renderTimerLinkStrip()}

              {isCustomTimerPreset || state.timer.mode === "exam" ? (
                <div className="timer-custom-row" data-tour="timer-custom">
                  <label className="field compact-field">
                    <span>{state.timer.mode === "exam" ? "Exam minutes" : "Focus minutes"}</span>
                    <input
                      type="number"
                      min="1"
                      value={state.timer.mode === "exam" ? state.timer.examMinutes : state.timer.studyMinutes}
                      disabled={state.timer.running}
                      onChange={(event) => {
                        const next = Math.max(1, Number(event.target.value) || 1);
                        setState((current) => {
                          const timer = {
                            ...current.timer,
                            studyMinutes: current.timer.mode === "exam" ? current.timer.studyMinutes : next,
                            examMinutes: current.timer.mode === "exam" ? next : current.timer.examMinutes,
                            presetLabel: current.timer.mode === "exam" ? current.timer.presetLabel : "Custom",
                          };
                          return {
                            ...current,
                            timer: {
                              ...timer,
                              remainingSeconds: current.timer.running ? getDisplayRemainingSeconds(current.timer) : getIdleTimerSeconds(timer),
                            },
                          };
                        });
                      }}
                    />
                  </label>
                  {state.timer.mode !== "exam" ? (
                    <label className="field compact-field">
                      <span>Break minutes</span>
                      <input
                        type="number"
                        min="0"
                        disabled={state.timer.running}
                        value={state.timer.breakMinutes}
                        onChange={(event) =>
                          setState((current) => ({
                            ...current,
                            timer: { ...current.timer, breakMinutes: Math.max(0, Number(event.target.value) || 0), presetLabel: "Custom" },
                          }))
                        }
                      />
                    </label>
                  ) : null}
                </div>
              ) : null}

              <div ref={timerFaceRingRef} className={`timer-face spacious-face ${state.timer.running ? "running" : state.timer.phase !== "idle" ? "paused" : "idle"}`} data-tour="timer-face" style={{ "--aura-color": timerCourse?.color ?? "var(--accent)" } as CSSProperties}>
                <div className="timer-phase-row">
                  <span className="timer-phase-pill">{state.timer.phase === "stopwatch" ? "Stopwatch" : state.timer.phase === "break" ? "Break" : state.timer.mode === "exam" ? "Exam" : "Study"}</span>
                  <span className="timer-course-dot" style={{ background: timerCourse?.color ?? "var(--accent)" }} />
                  <span className="timer-context-title">{timerTask?.title ?? timerCourse?.name ?? "General focus"}</span>
                </div>
                {appStyle === "modern" ? (
                  <strong><TimerClockDigits timer={state.timer} formatClock={formatClock} /></strong>
                ) : timerFaceEditing ? (
                  <input
                    className="timer-face-input"
                    value={timerFaceDraft}
                    onChange={(event) => setTimerFaceDraft(event.target.value)}
                    onBlur={applyTimerFaceDraft}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") {
                        event.preventDefault();
                        applyTimerFaceDraft();
                      } else if (event.key === "Escape") {
                        setTimerFaceEditing(false);
                        setTimerFaceDraft(formatClock(state.timer.remainingSeconds));
                      }
                    }}
                    inputMode="numeric"
                    aria-label="Timer duration"
                    autoFocus
                  />
                ) : (
                  <button type="button" className="timer-face-time" onClick={startTimerFaceEditing} disabled={state.timer.running || state.timer.phase !== "idle" || (state.timer.mode !== "exam" && !isCustomTimerPreset)}>
                    <TimerClockDigits timer={state.timer} formatClock={formatClock} />
                  </button>
                )}
                <p>{state.timer.running ? "In session" : state.timer.phase === "idle" ? "Ready" : "Paused"} · {formatMinutes(getTimerMinutes(state.timer))} logged</p>
                {timerTask ? <span className="timer-context-unit">{getNextUnitLabel(timerTask).replace("Next: ", "")}</span> : null}
              </div>

              <div className="timer-action-row roomy-top" data-tour="timer-actions">
                <button
                  type="button"
                  className="timer-primary-action"
                  data-tour="timer-start"
                  onClick={state.timer.phase === "idle" ? startTimer : pauseTimer}
                >
                  <span>{"\u25B7"}</span>
                  {state.timer.phase === "idle" ? (state.timer.mode === "endless" ? "Start tracking" : "Start") : state.timer.running ? "Pause" : "Resume"}
                </button>
                <button type="button" className="timer-save-action" data-tour="timer-save" onClick={completeSessionManually} disabled={state.timer.phase === "idle" || state.timer.phase === "break"}>
                  ▣ Save
                </button>
                {state.timer.running && state.timer.phase !== "idle" ? (
                  <button type="button" className="timer-maximise-action" onClick={() => setFullscreen(true)} title="Maximise">
                    ⛶
                  </button>
                ) : null}
                <button type="button" className="timer-reset-action" data-tour="timer-reset" onClick={resetTimer} title="Reset">
                  ↻
                </button>
              </div>

              {state.timer.mode !== "exam" && state.timer.mode !== "endless" ? (
                <button
                  type="button"
                  className="timer-break-switch"
                  onClick={() => {
                    if (state.timer.phase === "study" && getTimerMinutes(state.timer) > 0) {
                      setMessage("Save or reset the current study session before switching to break.");
                      return;
                    }
                    setState((current) => ({
                      ...current,
                      timer: {
                        ...current.timer,
                        running: false,
                        phase: current.timer.phase === "break" ? "study" : "break",
                        remainingSeconds: (current.timer.phase === "break" ? current.timer.studyMinutes : current.timer.breakMinutes) * 60,
                        startedAt: null,
                        endsAt: null,
                        loggedSplitSeconds: 0,
                        activeSegments: [],
                      },
                    }));
                  }}
                >
                  Switch to {state.timer.phase === "break" ? "study" : "break"} →
                </button>
              ) : null}
            </div>

            {renderTimerAdvancedPanel()}
          </article>

          <article className="panel-card session-log-card" data-tour="timer-session-log">
            <div className="section-head">
              <div>
                <p className="eyebrow">Recent Sessions</p>
                <h2>Your focus log</h2>
              </div>
              <span className="design-chip">{state.sessions.length}</span>
            </div>

            <div className="stack-list compact">
              {state.sessions.length ? (
                state.sessions.slice(0, 10).map((session) => (
                  <div key={session.id} className="session-row">
                    <div>
                      <strong>{session.goal || session.presetLabel}</strong>
                      <p>
                        {semesterLookup.get(session.semesterId ?? "")?.name ?? "No semester"} • {courseLookup.get(session.courseId ?? "")?.name ?? "General"} • {formatMinutes(session.minutes)}
                      </p>
                      {session.learned ? <small>Learned: {session.learned}</small> : null}
                    </div>
                    <div className="session-side">
                      <span>{session.confidence}/5</span>
                      <small>{new Intl.DateTimeFormat("en", { month: "short", day: "numeric" }).format(new Date(session.endedAt))} · {new Intl.DateTimeFormat("en", { hour: "numeric", minute: "2-digit" }).format(new Date(session.endedAt))}</small>
                      <button type="button" className="mini-danger" aria-label={`Delete session ${session.goal || session.presetLabel}`} onClick={() => removeSession(session.id)}>
                        {'\u2715'}
                      </button>
                    </div>
                  </div>
                ))
              ) : (
                <p className="empty-copy">Completed study blocks land here and feed the daily note export.</p>
              )}
            </div>
          </article>
        </section>
      ) : null}

      {state.activeTab === "vault" ? (
        <section className={`vault-shell ${appStyle === "field-notebook" ? "fn-vault" : appStyle === "wabi-sabi" ? "wabi-vault" : ""}`}>
          {appStyle === "field-notebook" ? (
            <aside className="fn-vault-rail" aria-label="Vault drawers">
              <div className="fn-vault-status">
                <span className={state.settings.vaultPath ? "linked" : ""} />
                <strong>{state.settings.vaultName || "StudyTrackerVault"}</strong>
                <em>{state.settings.vaultPath ? "SYNCED" : "NOT LINKED"}</em>
              </div>
              <div className="fn-vault-tabs">
                {vaultSpaces.map((space) => (
                  <button key={space.id} type="button" className={vaultSpace === space.id ? "active" : ""} onClick={() => setVaultSpace(space.id)}>{space.label}</button>
                ))}
              </div>
              <div className="fn-vault-tools">
                <button type="button" onClick={() => setMarkdownCheatsheetOpen(true)}>Markdown</button>
                <button type="button" onClick={() => setVaultSetupOpen((current) => !current)}>{vaultSetupOpen ? "Close setup" : "Vault setup"}</button>
              </div>
              {vaultSpace === "daily" ? (
                <>
                  <div className="fn-rail-section">
                    <div className="fn-rail-label">Recent daily notes</div>
                    <button type="button" className="fn-rail-row active" onClick={() => setVaultNoteDate(calendarToday)}>
                      <strong>{calendarToday}</strong>
                      <span>{state.sessions.filter((session) => isoDate(new Date(session.endedAt)) === calendarToday).length} sessions · today</span>
                    </button>
                    {state.exports.slice(0, 5).map((item) => (
                      <button key={item.id} type="button" className="fn-rail-row" onClick={() => setVaultNoteDate(item.noteDate)}>
                        <strong>{item.noteDate}</strong>
                        <span>{item.notePath}</span>
                      </button>
                    ))}
                  </div>
                </>
              ) : (
                <div className="fn-rail-section fn-vault-tree">
                  <div className="fn-rail-label">Course drawers</div>
                  {state.semesters.map((semester) => {
                    const courses = getSemesterCourses(state, semester.id);
                    const activeSemester = expandedSemesterIds.includes(semester.id);
                    return (
                      <div key={semester.id} className="fn-semester-tree">
                        <button type="button" className={`fn-rail-row fn-semester-folder ${activeSemester ? "active" : ""}`} onClick={() => toggleSemester(semester.id)}>
                          <strong>{semester.name}</strong>
                          <span>{activeSemester ? "close" : "open"} · {courses.length} courses</span>
                        </button>
                        {activeSemester ? (
                          <div className="fn-course-tree">
                            {courses.map((course) => {
                              const activeCourse = vaultSpace === "references" ? course.id === referenceCourseId : course.id === summaryCourseId;
                              return (
                                <div key={course.id} className="fn-course-tree-item">
                                  <button type="button" className={`fn-course-folder ${activeCourse ? "active" : ""}`} style={{ "--fn-course": course.color } as CSSProperties} onClick={() => {
                                    if (vaultSpace === "references") {
                                      if (referenceDirty) {
                                        setMessage("Save the current reference note before switching courses.");
                                        return;
                                      }
                                      setReferenceSemesterId(semester.id);
                                      setReferenceCourseId(course.id);
                                    } else {
                                      setSummarySemesterId(semester.id);
                                      setSummaryCourseId(course.id);
                                    }
                                  }}>
                                    <strong>{course.name}</strong>
                                    <span>{vaultSpace === "references" ? "note" : vaultSpace === "summaries" && activeCourse ? summaryFiles.length : "files"}</span>
                                  </button>
                                  {vaultSpace === "summaries" && activeCourse ? (
                                    <div className="fn-task-tree fn-summary-files-tree">
                                      <div className="fn-sidebar-actions fn-summary-file-actions">
                                        <button type="button" onClick={() => loadSummaryFileList()} disabled={summaryLoading}>{summaryLoading ? "loading" : "refresh"}</button>
                                        <button type="button" onClick={handleAddSummaryFiles} disabled={summaryLoading}>add files</button>
                                      </div>
                                      {summaryFiles.length ? summaryFiles.map((file) => (
                                        <button key={file.path} type="button" className={`fn-sidebar-task ${file.path === selectedSummaryFile?.path ? "active" : ""}`} onClick={() => setSelectedSummaryPath(file.path)}>
                                          <span>{file.name}</span>
                                          <em>{file.kind.toUpperCase()}</em>
                                        </button>
                                      )) : <p className="fn-sidebar-empty">No files yet.</p>}
                                    </div>
                                  ) : null}
                                </div>
                              );
                            })}
                          </div>
                        ) : null}
                      </div>
                    );
                  })}
                </div>
              )}
            </aside>
          ) : null}
          {appStyle !== "field-notebook" ? <div className="vault-hero">
            <div>
              <h1>Vault</h1>
              <p>Your Obsidian-compatible markdown knowledge base.</p>
            </div>
            <div className="vault-hero-actions">
              {state.settings.vaultPath ? (
                <span className="vault-status-pill"><span />{state.settings.vaultName || "Linked vault"}</span>
              ) : null}
              <button type="button" className="ghost-button" data-tour="vault-markdown" onClick={() => setMarkdownCheatsheetOpen(true)}>
                Markdown
              </button>
              <button type="button" className="ghost-button vault-settings-button" data-tour="vault-setup" onClick={() => setVaultSetupOpen((current) => !current)}>
                {vaultSetupOpen ? "Close setup" : "Vault setup"}
              </button>
            </div>
          </div> : null}

          {markdownCheatsheetOpen ? (
            <div className="markdown-cheatsheet-backdrop" onMouseDown={() => setMarkdownCheatsheetOpen(false)}>
              <section className="markdown-cheatsheet-panel" onMouseDown={(event) => event.stopPropagation()} aria-label="Markdown cheatsheet">
                <div className="markdown-cheatsheet-head">
                  <div>
                    <p className="eyebrow">Vault markdown</p>
                    <h2>Cheatsheet</h2>
                    <p>These are the markdown features Study Tracker currently previews.</p>
                  </div>
                  <button type="button" className="ghost-button small-button" onClick={() => setMarkdownCheatsheetOpen(false)} aria-label="Close markdown cheatsheet">
                    X
                  </button>
                </div>

                <div className="markdown-cheatsheet-grid">
                  <div className="markdown-cheatsheet-example">
                    <h3>Headings</h3>
                    <pre><code>{"# Title\n## Section\n### Subsection"}</code></pre>
                  </div>
                  <div className="markdown-cheatsheet-example">
                    <h3>Emphasis</h3>
                    <pre><code>{"**bold**\n*italic*\n`inline code`"}</code></pre>
                  </div>
                  <div className="markdown-cheatsheet-example">
                    <h3>Lists</h3>
                    <pre><code>{"- bullet item\n* another bullet\n\n1. first\n2. second"}</code></pre>
                  </div>
                  <div className="markdown-cheatsheet-example">
                    <h3>Quotes</h3>
                    <pre><code>{"> Important note or reminder"}</code></pre>
                  </div>
                  <div className="markdown-cheatsheet-example">
                    <h3>Links</h3>
                    <pre><code>{"[ETH Video](https://video.ethz.ch/...)\nhttps://video.ethz.ch/..."}</code></pre>
                  </div>
                  <div className="markdown-cheatsheet-example">
                    <h3>Code Blocks</h3>
                    <pre><code>{"```\nconst topic = \"series\";\n```"}</code></pre>
                  </div>
                </div>

                <div className="markdown-cheatsheet-note">
                  <strong>Not supported in preview yet:</strong> tables, images, LaTeX/math rendering, nested lists, and interactive task checkboxes.
                </div>
              </section>
            </div>
          ) : null}

          {!state.settings.vaultPath ? (
            <article className="panel-card vault-empty-card">
              <p className="eyebrow">No vault linked</p>
              <h2>Connect your markdown vault</h2>
              <p className="section-note">Notes are saved as standard `.md` files you can open in Obsidian or any editor.</p>
              <div className="control-row roomy-top">
                <button type="button" data-tour="vault-link" onClick={handleLinkVault}>Link existing vault</button>
                <button type="button" className="ghost-button" data-tour="vault-create" onClick={handleCreateVault}>Create new vault</button>
              </div>
            </article>
          ) : null}

          {vaultSetupOpen ? (
            <article className="panel-card vault-settings-panel">
              <div className="section-head">
                <div>
                  <p className="eyebrow">Obsidian vault</p>
                  <h2>Vault configuration</h2>
                </div>
              </div>
              <div className="form-grid compact-grid">
                <label className="field">
                  <span>Vault name</span>
                  <input
                    data-tour="vault-name"
                    value={state.settings.vaultName}
                    onChange={(event) => setState((current) => ({ ...current, settings: { ...current.settings, vaultName: event.target.value } }))}
                    placeholder="StudyTrackerVault"
                  />
                </label>
                <label className="field wide">
                  <span>Current vault path</span>
                  <input data-tour="vault-path" value={state.settings.vaultPath ?? "Not created yet"} readOnly />
                </label>
              </div>
              <div className="vault-folder-strip" aria-label="Vault folders">
                {["Daily", "References", "Summaries"].map((folder) => <span key={folder}>{folder}</span>)}
              </div>
              <div className="control-row left roomy-top">
                <button type="button" data-tour="vault-create" onClick={handleCreateVault}>Create new vault</button>
                <button type="button" className="ghost-button" data-tour="vault-link" onClick={handleLinkVault}>Link existing vault</button>
              </div>
            </article>
          ) : null}

          {state.settings.vaultPath ? (
            <>
              {appStyle !== "wabi-sabi" ? (
                <nav className="vault-nav" aria-label="Vault spaces" data-tour="vault-spaces">
                  {vaultSpaces.map((space) => {
                    const active = space.id === vaultSpace;
                    return (
                      <button
                        key={space.id}
                        type="button"
                        className={`vault-nav-item ${active ? "active" : ""}`}
                        onClick={() => setVaultSpace(space.id)}
                      >
                        {space.label}
                      </button>
                    );
                  })}
                </nav>
              ) : null}

              {appStyle === "wabi-sabi" ? (
                <nav className="wabi-notes-nav wabi-vault-space-nav" aria-label="Vault spaces" data-tour="vault-spaces">
                  {vaultSpaces.map((space) => (
                    <button key={space.id} type="button" className={vaultSpace === space.id ? "active" : ""} onClick={() => setVaultSpace(space.id)}>
                      {space.id === "daily" ? "Notes" : space.label}
                    </button>
                  ))}
                </nav>
              ) : null}

              {vaultSpace === "daily" ? (
                appStyle === "wabi-sabi" ? (
                  <div className="wabi-notes-layout" data-tour="vault-editor">
                    <aside className="wabi-notes-shelf">
                      <div className="wabi-notes-shelf-head">
                        <p className="wabi-notes-count">{vaultNotes.length} in this shelf</p>
                        <button type="button" onClick={startNewVaultNote}>New note</button>
                      </div>
                      {vaultNotes.map((note) => (
                        <button key={note.path} type="button" className={`wabi-note-shelf-row ${vaultNotePath === note.path ? "active" : ""}`} onClick={() => void openVaultNote(note)}>
                          <strong>{note.title}</strong>
                          <span>{new Intl.DateTimeFormat("en", { year: "numeric", month: "short", day: "numeric" }).format(new Date(note.savedAt))}</span>
                        </button>
                      ))}
                      {!vaultNotes.length && !vaultNotesLoading ? <p className="wabi-notes-empty">Create your first note.</p> : null}
                      <div className="wabi-notes-actions">
                        <button type="button" onClick={() => void loadVaultNotes()} disabled={vaultNotesLoading}>{vaultNotesLoading ? "Loading..." : "Refresh"}</button>
                        {vaultDailyEditing ? <button type="button" onClick={() => void handleSaveWabiVaultNote()} disabled={vaultNotesLoading}>Save</button> : <button type="button" onClick={() => setVaultDailyEditing(true)} disabled={!vaultNoteTitle}>Edit</button>}
                        <button type="button" className="wabi-note-delete" onClick={() => void handleDeleteWabiVaultNote()} disabled={!vaultNotePath || vaultNotesLoading}>Delete</button>
                      </div>
                    </aside>
                    <article className="wabi-note-reader">
                      <header>
                        {vaultDailyEditing ? (
                          <input
                            className="wabi-note-title-input"
                            value={vaultNoteTitle}
                            onChange={(event) => {
                              setVaultNoteTitle(event.target.value);
                              setVaultNoteDirty(true);
                            }}
                            placeholder="Note title"
                          />
                        ) : <h2>{vaultNoteTitle || "Untitled note"}</h2>}
                        <p>{vaultNoteDirty ? "unsaved changes" : vaultNoteSavedAt ? new Intl.DateTimeFormat("en", { year: "numeric", month: "short", day: "numeric" }).format(new Date(vaultNoteSavedAt)) : "not saved"}</p>
                      </header>
                      {vaultDailyEditing ? (
                        <textarea
                          className="wabi-note-editor"
                          value={vaultNoteContent}
                          onChange={(event) => {
                            setVaultNoteContent(event.target.value);
                            setVaultNoteDirty(true);
                          }}
                          placeholder="Write your note."
                        />
                      ) : (
                        <div className="markdown-preview wabi-note-prose">{renderMarkdownPreview(vaultNoteContent)}</div>
                      )}
                    </article>
                  </div>
                ) : <div className="vault-space-panel" data-tour="vault-editor">
                  <div className="vault-toolbar">
                    <div className="vault-toolbar-main">
                      <label className="vault-compact-field">
                        <span>Daily</span>
                        <input
                          type="date"
                          data-tour="vault-daily-date"
                          value={vaultNoteDate}
                          onChange={(event) => {
                            setVaultNoteDate(event.target.value || localIsoDate());
                            setVaultNotePath(null);
                            setVaultNoteContent("");
                            setVaultNoteDirty(false);
                            setVaultDailyEditing(false);
                          }}
                        />
                      </label>
                      <span className="vault-path-chip">{vaultNotePath ?? `Daily/${vaultNoteDate}.md`}</span>
                    </div>
                    <div className="vault-toolbar-actions">
                      <button type="button" className="ghost-button" data-tour="vault-load" onClick={() => loadVaultNote()} disabled={vaultNoteLoading}>{vaultNoteLoading ? "Working..." : "Load"}</button>
                      <button type="button" className="ghost-button" data-tour="vault-session-draft" onClick={handleUseGeneratedNote}>Use session draft</button>
                      {vaultDailyEditing ? (
                        <button type="button" className="ghost-button" data-tour="vault-save" onClick={handleSaveVaultNote} disabled={vaultNoteLoading}>Save</button>
                      ) : (
                        <button type="button" className="ghost-button" data-tour="vault-edit" onClick={() => setVaultDailyEditing(true)}>Edit</button>
                      )}
                      <span className="design-chip">{vaultNoteDirty ? "Unsaved" : "Saved"}</span>
                    </div>
                  </div>

                  {vaultDailyEditing ? (
                    <div className="vault-split">
                      <div className="vault-editor-pane">
                        <div className="vault-pane-head"><span className="eyebrow">Editor</span></div>
                        <textarea
                          className="vault-markdown-editor"
                          value={vaultNoteContent || notePreview}
                          onChange={(event) => {
                            setVaultNoteContent(event.target.value);
                            setVaultNoteDirty(true);
                          }}
                          placeholder="Write today's markdown note."
                        />
                      </div>
                      <div className="vault-preview-pane">
                        <div className="vault-pane-head"><span className="eyebrow">Preview</span></div>
                        <div className="markdown-preview vault-prose compact">{renderMarkdownPreview(dailyPreviewContent)}</div>
                      </div>
                    </div>
                  ) : (
                      <div className="vault-document-wrap">
                        <article className="panel-card vault-document">
                          <div className="vault-document-meta">
                            <span>Daily</span>
                            <code>{vaultNotePath ?? `Daily/${vaultNoteDate}.md`}</code>
                          </div>
                          <div className="markdown-preview vault-prose">{renderMarkdownPreview(dailyPreviewContent)}</div>
                        </article>

                        {appStyle !== "field-notebook" ? <div className="vault-recent-list" data-tour="vault-recent">
                          <p className="eyebrow">Recent exports</p>
                          {state.exports.length ? state.exports.slice(0, 4).map((item) => (
                            <div key={item.id} className="vault-recent-row">
                            <span>{item.noteDate}</span>
                            <code>{item.notePath}</code>
                            </div>
                          )) : <p className="empty-copy">Exports will appear here once you save a daily note.</p>}
                        </div> : null}
                      </div>
                  )}
                </div>
              ) : vaultSpace === "references" ? (
                <div className="vault-space-panel">
                  {appStyle === "wabi-sabi" ? renderWabiVaultCourseShelf("references") : null}
                  <div className="vault-toolbar">
                    {appStyle !== "field-notebook" ? <div className="vault-toolbar-main references-toolbar-main">
                      <label className="vault-compact-field">
                        <span>Semester</span>
                        <select
                          value={referenceSemesterId}
                          onChange={(event) => {
                            if (referenceDirty) {
                              setMessage("Save the current reference note before switching courses.");
                              return;
                            }
                            setReferenceSemesterId(event.target.value);
                            setReferenceCourseId("");
                          }}
                        >
                          {state.semesters.map((semester) => <option key={semester.id} value={semester.id}>{semester.name}</option>)}
                        </select>
                      </label>
                      <div className="vault-course-chips">
                        {referenceCourses.map((course) => (
                          <button
                            key={course.id}
                            type="button"
                            className={`vault-course-chip ${course.id === referenceCourseId ? "active" : ""}`}
                            onClick={() => {
                              if (referenceDirty) {
                                setMessage("Save the current reference note before switching courses.");
                                return;
                              }
                              setReferenceCourseId(course.id);
                            }}
                          >
                            <span style={{ background: course.color }} />{course.name}
                          </button>
                        ))}
                      </div>
                    </div> : <div className="vault-toolbar-main references-toolbar-main fn-vault-selection-title">
                      <p className="eyebrow">References</p>
                      <h2>{selectedReferenceCourse ? selectedReferenceCourse.name : "Choose a course"}</h2>
                    </div>}
                    <div className="vault-toolbar-actions">
                      {referenceEditing ? (
                        <button type="button" className="ghost-button" onClick={handleSaveReferenceNote} disabled={referenceLoading || !selectedReferenceCourse}>{referenceLoading ? "Saving..." : "Save"}</button>
                      ) : (
                        <button type="button" className="ghost-button" onClick={handleEditReferenceNote} disabled={!selectedReferenceCourse}>Edit</button>
                      )}
                      <span className="design-chip">{referenceDirty ? "Unsaved" : "Saved"}</span>
                    </div>
                  </div>

                  {selectedReferenceSemester && selectedReferenceCourse ? (
                    referenceEditing ? (
                      <div className="vault-split">
                        <div className="vault-editor-pane">
                          <div className="vault-pane-head"><span className="eyebrow">Markdown</span></div>
                          <textarea
                            className="vault-markdown-editor"
                            value={referenceContent}
                            onChange={(event) => {
                              setReferenceContent(event.target.value);
                              setReferenceDirty(true);
                            }}
                            placeholder="Add useful markdown links for this course."
                          />
                        </div>
                        <div className="vault-preview-pane">
                          <div className="vault-pane-head"><span className="eyebrow">Preview</span></div>
                          <div className="markdown-preview vault-prose compact">{renderMarkdownPreview(referencePreview)}</div>
                        </div>
                      </div>
                    ) : (
                      <div className="vault-document-wrap">
                        <article className="panel-card vault-document">
                          <div className="vault-document-meta">
                            <button type="button" className="vault-document-meta-button" onClick={() => setReferencePathVisible((current) => !current)}>
                              References
                            </button>
                            {referencePathVisible ? <code>{referencePath ?? `References/${selectedReferenceSemester.name}/${selectedReferenceCourse.name}.md`}</code> : null}
                          </div>
                          <div className="markdown-preview vault-prose">{renderMarkdownPreview(referencePreview)}</div>
                        </article>
                      </div>
                    )
                  ) : (
                    <article className="panel-card vault-empty-card">
                      <p className="eyebrow">No courses yet</p>
                      <h2>References need courses</h2>
                      <p className="section-note">Create at least one semester and course in Planner before adding course reference notes.</p>
                    </article>
                  )}
                </div>
              ) : (
                <div className="vault-space-panel">
                  {appStyle === "wabi-sabi" ? renderWabiVaultCourseShelf("summaries") : null}
                  <div className="vault-toolbar">
                    {appStyle !== "field-notebook" ? <div className="vault-toolbar-main references-toolbar-main">
                      <label className="vault-compact-field">
                        <span>Semester</span>
                        <select
                          value={summarySemesterId}
                          onChange={(event) => {
                            setSummarySemesterId(event.target.value);
                            setSummaryCourseId("");
                          }}
                        >
                          {state.semesters.map((semester) => <option key={semester.id} value={semester.id}>{semester.name}</option>)}
                        </select>
                      </label>
                      <div className="vault-course-chips">
                        {summaryCourses.map((course) => (
                          <button
                            key={course.id}
                            type="button"
                            className={`vault-course-chip ${course.id === summaryCourseId ? "active" : ""}`}
                            onClick={() => setSummaryCourseId(course.id)}
                          >
                            <span style={{ background: course.color }} />{course.name}
                          </button>
                        ))}
                      </div>
                    </div> : <div className="vault-toolbar-main references-toolbar-main fn-vault-selection-title">
                      <p className="eyebrow">Summaries</p>
                      <h2>{selectedSummaryCourse ? selectedSummaryCourse.name : "Choose a course"}</h2>
                    </div>}
                    {appStyle !== "field-notebook" ? <div className="vault-toolbar-actions">
                      <button type="button" className="ghost-button" onClick={() => loadSummaryFileList()} disabled={summaryLoading || !selectedSummaryCourse}>{summaryLoading ? "Loading..." : "Refresh"}</button>
                      <button type="button" onClick={handleAddSummaryFiles} disabled={summaryLoading || !selectedSummaryCourse}>Add files</button>
                    </div> : null}
                  </div>

                  {selectedSummarySemester && selectedSummaryCourse ? (
                    <div className="summaries-layout">
                      {appStyle !== "field-notebook" && (appStyle !== "wabi-sabi" || summaryFiles.length > 1) ? <aside className="panel-card summary-file-list">
                        <div className="summary-file-list-head">
                          <div>
                            <p className="eyebrow">Summaries</p>
                            <h2>{selectedSummaryCourse.name}</h2>
                          </div>
                          <span>{summaryFiles.length}</span>
                        </div>
                        {summaryFiles.length ? (
                          <div className="summary-file-links">
                            {summaryFiles.map((file) => (
                              <button
                                key={file.path}
                                type="button"
                                className={`summary-file-link ${file.path === selectedSummaryFile?.path ? "active" : ""}`}
                                onClick={() => setSelectedSummaryPath(file.path)}
                              >
                                <span>{file.name}</span>
                                <small>{file.kind.toUpperCase()} • {formatFileSize(file.sizeBytes)}</small>
                              </button>
                            ))}
                          </div>
                        ) : (
                          <p className="empty-copy">No summaries yet. Add PDFs, formula sheets, or cheatsheet images for this course.</p>
                        )}
                      </aside> : null}

                      <section className="panel-card summary-viewer-card">
                        {selectedSummaryFile && selectedSummaryUrl ? (
                          <>
                            <div className="summary-viewer-head">
                              <div>
                                <p className="eyebrow">Viewer</p>
                                <h2>{selectedSummaryFile.name}</h2>
                              </div>
                              <button type="button" className="ghost-button small-button" onClick={() => revealItemInDir(selectedSummaryFile.path)}>Show file</button>
                            </div>
                            {selectedSummaryFile.kind === "pdf" ? (
                              <SummaryPdfViewer vaultPath={state.settings.vaultPath} path={selectedSummaryFile.path} title={selectedSummaryFile.name} />
                            ) : (
                              <div className="summary-image-viewer">
                                <img src={selectedSummaryUrl} alt={selectedSummaryFile.name} />
                              </div>
                            )}
                          </>
                        ) : (
                          <div className="summary-viewer-empty">
                            <p className="eyebrow">Viewer</p>
                            <h2>Select a summary</h2>
                            <p className="section-note">Choose a file from the list, or add PDFs/images to this course.</p>
                          </div>
                        )}
                      </section>
                    </div>
                  ) : (
                    <article className="panel-card vault-empty-card">
                      <p className="eyebrow">No courses yet</p>
                      <h2>Summaries need courses</h2>
                      <p className="section-note">Create at least one semester and course in Planner before adding summary files.</p>
                    </article>
                  )}
                </div>
              )}
            </>
          ) : null}
        </section>
      ) : null}

      {state.activeTab === "friends" ? (
        <section className={`arena-root fade-up ${appStyle === "field-notebook" ? "study-circle-root" : appStyle === "wabi-sabi" ? "wabi-circle-root" : ""}`}>
          {appStyle === "modern" ? <ArenaBg /> : null}
          <div className="arena-hero-header">
            <span className="arena-hero-title">{appStyle === "field-notebook" ? "Study Circle" : appStyle === "wabi-sabi" ? "Circle" : "Study Arena"}</span>
            <span className="arena-hero-sub">{appStyle === "field-notebook" ? "Attendance sheet · friends · squads · quiet accountability" : appStyle === "wabi-sabi" ? "Who is sitting down today. Attendance, not a leaderboard." : "Compete. Focus. Rise."}</span>
            {appStyle === "wabi-sabi" ? (
              <button
                type="button"
                className={`wabi-competitive-toggle ${wabiCircleCompetitive ? "active" : ""}`}
                onClick={() => setWabiCircleCompetitive((current) => {
                  const next = !current;
                  if (!next && socialSubtab === "leaderboard") setSocialSubtab("feed");
                  return next;
                })}
              >
                {wabiCircleCompetitive ? "COMPETITIVE ON · SHOWING LEADERBOARD" : "TURN ON COMPETITIVE →"}
              </button>
            ) : null}
          </div>

          <nav className="social-nav" aria-label="Social spaces" data-tour="social-nav">
            {socialSubtabs.filter((space) => appStyle !== "wabi-sabi" || space.id !== "leaderboard" || wabiCircleCompetitive).map((space) => {
              const active = space.id === socialSubtab;
              return (
                <button key={space.id} type="button" data-tour={`social-${space.id}-tab`} className={`social-nav-item ${active ? "active" : ""}`} onClick={() => setSocialSubtab(space.id)}>
                  {appStyle === "wabi-sabi" && space.id === "leaderboard" ? "Standings" : space.label}
                  {space.id === "friends" && incomingFriendRequestCount > 0 ? (
                    <span className="social-nav-request-badge" aria-label={`${incomingFriendRequestCount} incoming friend request${incomingFriendRequestCount === 1 ? "" : "s"}`}>
                      {incomingFriendRequestCount > 9 ? "9+" : incomingFriendRequestCount}
                    </span>
                  ) : null}
                  {space.badge ? <span className="social-nav-item-badge">{space.badge}</span> : null}
                </button>
              );
            })}
          </nav>

          {appStyle === "wabi-sabi" && socialSubtab === "feed" ? (
            <div className="wabi-attendance" data-tour="social-live">
              {wabiAttendanceFriends.length ? (
                wabiAttendanceFriends.map((friend) => {
                  const sitting = isRecentlyActive(friend.lastSeenAt);
                  return (
                    <div key={friend.userId} className="wabi-attendance-row" onClick={() => void openFriendProfile(friend)} role="button" tabIndex={0} onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); openFriendProfile(friend); } }}>
                      <span className="wabi-attendance-name">{friend.displayName}</span>
                      <span className={`wabi-attendance-state ${sitting ? "sitting" : "resting"}`}>{sitting ? "sitting" : "resting"}</span>
                    </div>
                  );
                })
              ) : (
                <p className="wabi-empty">No friends seen in the last 48 hours. Add friends from the Friends tab below.</p>
              )}
            </div>
          ) : null}

          {socialSubtab === "feed" ? (
            <div className={`social-feed-shell ${appStyle === "wabi-sabi" ? "wabi-feed-shell" : ""}`}>
              <div className="social-feed-sidebar">
                <div className="arena-scope-toggle social-feed-scope" aria-label="Feed scope" data-tour="social-feed-scope">
                  {(["friends", "global"] as SocialFeedScope[]).map((scope) => (
                    <button key={scope} type="button" className={feedScope === scope ? "arena-scope-btn arena-scope-btn--active" : "arena-scope-btn"} onClick={() => setFeedScope(scope)}>
                      {appStyle === "field-notebook" ? (scope === "global" ? "Global log" : "Friends log") : (scope === "global" ? "Global Feed" : "Friends Feed")}
                    </button>
                  ))}
                  <button type="button" className="arena-btn arena-btn--decline social-refresh-btn" data-tour="social-refresh" onClick={() => void runSocialSync()} disabled={socialSyncing || !socialConfigured}>
                    {socialSyncing ? "Syncing..." : "Refresh"}
                  </button>
                </div>

                <div className="stories" aria-label="Study circle stories" data-tour="social-stories">
                  <div className="story">
                    <div className="story__ring story__ring--self"><ArenaAvatar name={state.social.displayName} avatar={state.social.avatar} self size="md" /></div>
                    <span>You</span>
                  </div>
                  {state.social.friends.slice(0, 10).map((friend) => (
                    <div key={friend.userId} className="story" onClick={() => void openFriendProfile(friend)} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); openFriendProfile(friend); } }}>
                      <div className={`story__ring ${isRecentlyActive(friend.lastSeenAt) ? "" : "story__ring--idle"}`}><ArenaAvatar name={friend.displayName} avatar={friend.avatar} size="md" /></div>
                      <span>{friend.displayName}</span>
                    </div>
                  ))}
                </div>

                <div className="live-bar" data-tour="social-live">
                  <span className="live-dot" />
                  <strong>{liveFriends.length ? `${liveFriends.length} friends` : "No friends"}</strong>
                  <span>recently active</span>
                  <small>{liveFriends.slice(0, 3).map((friend) => friend.displayName).join(" · ") || "Sync to update live status"}</small>
                </div>

                {canViewR2Usage && (r2UsageStatus?.warning || r2UsageStatus?.paused) ? (
                  <div className={`feed-r2-safety ${r2UsageStatus.paused ? "feed-r2-safety--paused" : ""}`}>
                    <strong>{r2UsageStatus.paused ? "Image uploads paused" : "Image usage close to safety limit"}</strong>
                    <span>
                      Storage {formatBytes(r2UsageStatus.storageBytes)} / {formatBytes(r2UsageStatus.limits.storageHardBytes)} · Writes {formatCompactNumber(r2UsageStatus.classAOps)} / {formatCompactNumber(r2UsageStatus.limits.classAHardMonthly)} · Reads {formatCompactNumber(r2UsageStatus.classBOps)} / {formatCompactNumber(r2UsageStatus.limits.classBHardMonthly)}
                    </span>
                    <small>These strict app limits are far below Cloudflare R2's free tier to avoid overage risk.</small>
                  </div>
                ) : null}

                <form className={`feed-composer ${appStyle === "wabi-sabi" ? "wabi-feed-composer" : ""}`} data-tour="social-composer" onSubmit={postLatestSessionToFeed}>
                  <div>
                    <span className="arena-kicker">{appStyle === "field-notebook" ? "Post your last block" : appStyle === "wabi-sabi" ? "Circle note" : "Share latest session"}</span>
                    <h3>{latestFeedSession ? appStyle === "wabi-sabi" ? "Say something to the circle" : `${formatMinutes(latestFeedSession.minutes)} ${latestFeedSession.kind} block` : "No session ready"}</h3>
                    <p>{latestFeedSession ? (appStyle === "field-notebook" ? "One line for the circle, or leave it blank." : appStyle === "wabi-sabi" ? "A note will accompany your latest study block." : "Write one sentence, or leave it blank for a chaotic default.") : "Finish a study or exam block, then publish it here."}</p>
                  </div>
                  <input className="arena-input" data-tour="social-note" value={feedNoteDraft} onChange={(event) => setFeedNoteDraft(event.target.value)} placeholder={appStyle === "field-notebook" ? "one line, or leave it blank..." : appStyle === "wabi-sabi" ? "say something to the circle, then Enter" : "one sentence for the feed..."} disabled={!latestFeedSession || latestFeedSessionPosted} />
                  <div className="feed-composer-actions">
                    <label className="feed-action-icon" data-tour="social-image" title={feedImageDraft ? "Change image" : "Add image"} aria-label={feedImageDraft ? "Change image" : "Add image"}>
                      <input type="file" accept="image/png,image/jpeg,image/webp,image/gif" onChange={(event) => void handleFeedImageDraftChange(event)} disabled={!latestFeedSession || latestFeedSessionPosted || (canViewR2Usage && r2UsageStatus?.paused)} />
                      <span aria-hidden="true">▧</span>
                    </label>
                    <button type="button" className={`feed-action-icon ${feedPollHasDraft ? "feed-action-icon--active" : ""}`} data-tour="social-poll" onClick={() => setFeedPollPanelOpen((open) => !open)} disabled={!latestFeedSession || latestFeedSessionPosted} title="Create poll" aria-label="Create poll">◉</button>
                  </div>
                  <button type="submit" className="arena-btn arena-btn--send" data-tour="social-post" disabled={!latestFeedSession || latestFeedSessionPosted}>{latestFeedSessionPosted ? "Posted" : "Post"}</button>
                  {feedPollPanelOpen ? (
                    <div className="feed-poll-popover">
                      <div className="feed-poll-popover__head">
                        <strong>Create poll</strong>
                        <label className="feed-poll-switch">
                          <span>Multiple answers</span>
                          <input type="checkbox" checked={feedPollDraft.multiple} onChange={(event) => setFeedPollDraft((current) => ({ ...current, multiple: event.target.checked }))} />
                          <i className="ios-switch" aria-hidden="true" />
                        </label>
                      </div>
                      <input className="arena-input" value={feedPollDraft.question} onChange={(event) => setFeedPollDraft((current) => ({ ...current, question: event.target.value }))} placeholder="Question" maxLength={180} />
                      <div className="feed-poll-options-editor">
                        {feedPollDraft.options.map((option, index) => (
                          <div key={index} className="feed-poll-option-editor">
                            <input className="arena-input" value={option} onChange={(event) => updateFeedPollOption(index, event.target.value)} placeholder={`Option ${index + 1}`} maxLength={100} />
                            {feedPollDraft.options.length > 2 ? <button type="button" className="feed-poll-option-remove" onClick={() => removeFeedPollOption(index)} aria-label={`Remove option ${index + 1}`}>×</button> : null}
                          </div>
                        ))}
                      </div>
                      <button type="button" className="feed-poll-add-option" onClick={addFeedPollOption} disabled={feedPollDraft.options.length >= MAX_FEED_POLL_OPTIONS}>+ Add option</button>
                      <div className="feed-poll-popover__actions">
                        <button type="button" className="arena-btn arena-btn--send" onClick={() => setFeedPollPanelOpen(false)}>Done</button>
                        <button type="button" className="ghost-button small-button" onClick={clearFeedPollDraft}>Clear</button>
                      </div>
                    </div>
                  ) : null}
                  {feedImageDraft ? (
                    <div className="feed-image-draft">
                      <img src={feedImageDraft.previewUrl} alt="Selected feed post preview" />
                      <button type="button" className="ghost-button small-button" onClick={() => setFeedImageDraft((current) => { if (current) URL.revokeObjectURL(current.previewUrl); return null; })}>Remove image</button>
                    </div>
                  ) : null}
                </form>
              </div>

              <div className={`social-feed-main ${appStyle === "wabi-sabi" ? "wabi-feed-main" : ""}`}>
              <div className="section-label" data-tour="social-feed">{appStyle === "field-notebook" ? "Circle log" : appStyle === "wabi-sabi" ? "Feed" : "Activity"} {appStyle === "wabi-sabi" ? "· oldest first · updates when synced" : ""} {feedLoading ? "· Refreshing" : ""}</div>
              {displayedSocialFeed.length ? displayedSocialFeed.map((item) => {
                const isOwnPost = item.userId === state.social.userId || item.isSelf;
                const profileTarget = { userId: item.userId, displayName: item.displayName, friendCode: item.friendCode, avatar: isOwnPost ? state.social.avatar : item.avatar };
                const comments = item.comments ?? [];
                const commentsOpen = expandedFeedComments.has(item.id);
                return item.type === "milestone" ? (
                  <article key={item.id} className="milestone">
                    <div className="milestone__icon">{item.icon || "🏆"}</div>
                    <div>
                      <h3><button type="button" className="social-name-button" onClick={() => void openFriendProfile(profileTarget)}>{item.displayName}</button> hit a milestone</h3>
                      <p>{item.note || item.detail}</p>
                    </div>
                  </article>
                ) : (
                  <article key={item.id} className={`feed-card ${appStyle === "wabi-sabi" ? "wabi-feed-card" : ""}`}>
                    <div className="feed-card__head">
                      <ArenaAvatar name={item.displayName} avatar={isOwnPost ? state.social.avatar : item.avatar} self={isOwnPost} />
                      <div>
                        <button type="button" className="social-name-button social-name-button--strong" onClick={() => void openFriendProfile(profileTarget)}>{item.displayName}{isOwnPost ? " (You)" : ""}</button>
                        <span>{formatFeedPostedAt(item.createdAt)}</span>
                      </div>
                      {isOwnPost ? <button type="button" className="arena-icon-button feed-edit-button" onClick={() => startEditingFeedPost(item)} title="Edit post">✎</button> : null}
                    </div>
                    <div className="feed-card__body">
                      <div className="feed-card__session">
                        <span>{item.icon || "✦"}</span>
                        <div>
                          <strong>{item.subject || "Study session"}</strong>
                          <small>{item.detail || `${formatMinutes(item.minutes)} · ${item.presetLabel || "Focus"}`}</small>
                        </div>
                      </div>
                      {editingFeedPostId === item.id ? (
                        <div className="feed-edit-panel">
                          <textarea className="arena-input feed-edit-textarea" value={editingFeedPostNote} onChange={(event) => setEditingFeedPostNote(event.target.value)} maxLength={220} autoFocus />
                          <div className="feed-image-edit-row">
                            <label className="feed-image-picker">
                              <input type="file" accept="image/png,image/jpeg,image/webp,image/gif" onChange={(event) => void handleEditingFeedPostImageChange(event)} disabled={feedPostSaving || (canViewR2Usage && Boolean(r2UsageStatus?.paused)) || state.social.pendingFeedPosts.some((post) => post.id === item.id)} />
                              <span>{item.imageUrl || item.imageExpiredAt || editingFeedPostImage ? "Replace image" : "Add image"}</span>
                            </label>
                            {item.imageUrl && !editingFeedPostImage ? <button type="button" className="ghost-button small-button" onClick={() => setEditingFeedPostRemoveImage(true)} disabled={feedPostSaving}>Remove image</button> : null}
                          </div>
                          {state.social.pendingFeedPosts.some((post) => post.id === item.id) ? <p className="feed-image-hint">Sync this post before adding an image.</p> : null}
                          {editingFeedPostImage ? (
                            <div className="feed-image-draft feed-image-draft--edit">
                              <img src={editingFeedPostImage.previewUrl} alt="Replacement feed post preview" />
                              <button type="button" className="ghost-button small-button" onClick={() => setEditingFeedPostImage((current) => { if (current) URL.revokeObjectURL(current.previewUrl); return null; })}>Clear replacement</button>
                            </div>
                          ) : editingFeedPostRemoveImage ? <p className="feed-image-hint">Image will be removed when you save.</p> : null}
                          <div className="feed-edit-actions">
                            <button type="button" className="arena-btn arena-btn--send" onClick={() => void saveFeedPostEdit(item.id)} disabled={feedPostSaving}>Save</button>
                            <button type="button" className="ghost-button small-button" onClick={cancelEditingFeedPost} disabled={feedPostSaving}>Cancel</button>
                            <button type="button" className="arena-btn arena-btn--decline" onClick={() => void deleteOwnFeedPost(item.id)} disabled={feedPostSaving}>Delete</button>
                          </div>
                        </div>
                      ) : item.note ? <p>"{item.note}"</p> : null}
                      {feedPollsVisible && item.poll ? (
                        <div className="feed-poll-card">
                          <div className="feed-poll-card__head">
                            <strong>{item.poll.question}</strong>
                            <span>{item.poll.multiple ? "Multiple answers" : "One answer"}</span>
                          </div>
                          <div className="feed-poll-card__options">
                            {item.poll.options.map((option) => {
                              const percent = item.poll?.totalVotes ? Math.round((option.votes / item.poll.totalVotes) * 100) : 0;
                              return (
                                <button key={option.id} type="button" className={`feed-poll-vote ${option.selected ? "feed-poll-vote--selected" : ""}`} onClick={() => void voteFeedPoll(item, option.id)}>
                                  <span className="feed-poll-vote__fill" style={{ width: `${percent}%` }} />
                                  <span className="feed-poll-vote__check">{option.selected ? "✓" : item.poll?.multiple ? "□" : "○"}</span>
                                  <span className="feed-poll-vote__text">{option.text}</span>
                                  <span className="feed-poll-vote__count">{option.votes} · {percent}%</span>
                                </button>
                              );
                            })}
                          </div>
                          <small>{item.poll.totalVotes} vote{item.poll.totalVotes === 1 ? "" : "s"}</small>
                        </div>
                      ) : null}
                      {feedImagesVisible && item.imageUrl && !failedFeedImages.has(item.id) ? (
                        <button type="button" className="feed-card__image" onClick={() => setExpandedFeedImageId(item.id)} aria-label="Open feed image fullscreen">
                          <img src={`${item.imageUrl}${item.imageUrl.includes("?") ? "&" : "?"}v=${encodeURIComponent(item.imageExpiresAt ?? item.createdAt)}`} alt={`${item.displayName}'s feed post image`} loading="lazy" onLoad={() => setFailedFeedImages((current) => { if (!current.has(item.id)) return current; const next = new Set(current); next.delete(item.id); return next; })} onError={() => setFailedFeedImages((current) => new Set(current).add(item.id))} />
                        </button>
                      ) : feedImagesVisible && item.imageUrl && failedFeedImages.has(item.id) ? <p className="feed-card__image-expired">Image could not load</p> : feedImagesVisible && item.imageExpiredAt ? <p className="feed-card__image-expired">Image expired</p> : null}
                    </div>
                    <div className="feed-card__reactions">
                      {(() => {
                        const emojiKeys = appStyle === "wabi-sabi"
                          ? ["fire"]
                          : ["fire", "brain", "clap", ...Object.keys(item.reactions ?? {}).filter((k) => k !== "fire" && k !== "brain" && k !== "clap" && (item.reactions?.[k] ?? 0) > 0)];
                        const seen = new Set<string>();
                        return emojiKeys.filter((k) => { if (seen.has(k)) return false; seen.add(k); return true; }).map((emojiKey) => {
                          const count = item.reactions?.[emojiKey] ?? 0;
                          if (count === 0 && emojiKey !== "fire" && emojiKey !== "brain" && emojiKey !== "clap") return null;
                          const display = appStyle === "wabi-sabi" ? "Nod" : emojiKey === "fire" ? "🔥" : emojiKey === "brain" ? "🧠" : emojiKey === "clap" ? "👏" : emojiKey;
                          const names = item.reactedBy?.[emojiKey];
                          const label = names?.length ? (names.length <= 3 ? names.join(", ") : `${names.slice(0, 3).join(", ")} +${names.length - 3} more`) : "";
                          return (
                            <button key={emojiKey} type="button" className={`reaction-btn ${item.reacted?.[emojiKey] ? "reaction-btn--active" : ""}`} onClick={() => void toggleLocalFeedReaction(item.id, emojiKey)}>
                              {display} {count}
                              {label ? <span className="reaction-tooltip">{label}</span> : null}
                            </button>
                          );
                        });
                      })()}
                      <button type="button" className="reaction-btn reaction-btn--add" onClick={() => setEmojiPickerPostId(emojiPickerPostId === item.id ? null : item.id)} title="Add reaction">+</button>
                      {emojiPickerPostId === item.id ? (
                        <div className="emoji-picker">
                          {["🔥", "🧠", "👏", "⭐", "🎯", "💪", "📚", "⚡", "🎉", "🏆", "✨", "💡", "🎓", "🚀", "💎", "🌟", "📖", "🕐", "💯", "🙌", "🤯", "😤", "👑", "🌊", "😂", "🤣", "😭", "🥲", "😅", "🥹", "❤️", "🙏", "👍", "👎", "😍", "😎"].map((emj) => (
                            <button key={emj} type="button" className={`emoji-picker__item ${item.reacted?.[emj] ? "emoji-picker__item--active" : ""}`} onClick={() => { void toggleLocalFeedReaction(item.id, emj); setEmojiPickerPostId(null); }}>{emj}</button>
                          ))}
                        </div>
                      ) : null}
                    </div>
                    <div className="feed-card__comments-shell">
                      <button type="button" className="feed-comments-toggle" onClick={() => toggleFeedComments(item.id)} aria-expanded={commentsOpen}>
                        {appStyle === "wabi-sabi" ? `Reply${comments.length ? ` · ${comments.length}` : ""}` : `Comments (${comments.length}) ${commentsOpen ? "↑" : "↓"}`}
                      </button>
                      {commentsOpen ? (
                        <div className="feed-card__comments">
                          {comments.length ? comments.map((comment: SocialFeedComment) => {
                            const commentTarget = { userId: comment.userId, displayName: comment.displayName, friendCode: comment.friendCode, avatar: comment.isSelf ? state.social.avatar : comment.avatar };
                            return (
                              <div key={comment.id} className="feed-comment">
                                <ArenaAvatar name={comment.displayName} avatar={comment.isSelf ? state.social.avatar : comment.avatar} self={comment.isSelf} />
                                <div>
                                  <div className="feed-comment__meta">
                                    <button type="button" className="social-name-button social-name-button--strong" onClick={() => void openFriendProfile(commentTarget)}>{comment.displayName}{comment.isSelf ? " (You)" : ""}</button>
                                    <span>{formatFeedPostedAt(comment.createdAt)}</span>
                                  </div>
                                  <p>{comment.body}</p>
                                </div>
                              </div>
                            );
                          }) : <p className="feed-comments-empty">No comments yet. Start the reply chain.</p>}
                          <form className="feed-comment-form" onSubmit={(event) => void submitFeedComment(event, item)}>
                            <input
                              className="arena-input"
                              value={feedCommentDrafts[item.id] ?? ""}
                              onChange={(event) => setFeedCommentDrafts((current) => ({ ...current, [item.id]: event.target.value }))}
                              maxLength={220}
                              placeholder="Reply or comment under this post..."
                              disabled={feedCommentSavingId === item.id}
                            />
                            <button type="submit" className="arena-btn arena-btn--send" disabled={feedCommentSavingId === item.id || !(feedCommentDrafts[item.id] ?? "").trim()}>{feedCommentSavingId === item.id ? "Posting" : "Reply"}</button>
                          </form>
                        </div>
                      ) : null}
                    </div>
                  </article>
                );
              }) : (
                <div className="arena-empty"><strong>No feed posts yet</strong><span>Post a session or sync to pull the latest arena activity.</span></div>
              )}

              {appStyle !== "wabi-sabi" ? <><div className="section-label">This Week</div>
              <article className="week-compare">
                {weekCompareEntries.length ? weekCompareEntries.map((entry) => {
                  const max = Math.max(1, ...weekCompareEntries.map((item) => item.minutes));
                  return (
                    <div key={entry.userId} className="week-compare__row">
                      <ArenaAvatar name={entry.displayName} avatar={entry.isSelf ? state.social.avatar : entry.avatar} self={entry.isSelf} />
                      <strong>{entry.displayName}{entry.isSelf ? " (You)" : ""}</strong>
                      <div className="week-compare__bar-wrap"><span style={{ width: `${(entry.minutes / max) * 100}%` }} /></div>
                      <small>{formatMinutes(entry.minutes)}</small>
                    </div>
                  );
                }) : <p className="empty-copy">Weekly comparison appears after you sync with friends.</p>}
              </article></> : <p className="wabi-feed-end">That is the whole feed. No counts, no ranks, nothing new until someone shares again.</p>}
              </div>
            </div>
          ) : null}

          {socialSubtab === "profile" ? (
            <div className={`social-single-panel ${appStyle === "wabi-sabi" ? "wabi-profile-panel" : ""}`}>
              <article className="arena-player-card">
                <div className="arena-player-card__inner">
                  <div className="arena-player-main">
                    <button type="button" className="profile-avatar-button" onClick={openProfileAvatarEditor} title="Change profile avatar">
                      <ArenaAvatar name={state.social.displayName} avatar={state.social.avatar} self size="lg" />
                      <span>Edit</span>
                    </button>
                    <div className="arena-player-copy">
                      {socialNameEditing ? (
                        <form className="arena-name-edit" onSubmit={saveSocialName}>
                          <input className="arena-name-input" value={socialNameDraft} onChange={(event) => setSocialNameDraft(event.target.value)} maxLength={48} autoFocus />
                          <button type="submit" className="arena-icon-button arena-icon-button--save" title="Save player name">✓</button>
                          <button type="button" className="arena-icon-button" onClick={cancelEditingSocialName} title="Cancel">×</button>
                        </form>
                      ) : (
                        <div className="arena-name-row">
                          <h2>{state.social.displayName}</h2>
                          <button type="button" className="arena-name-edit-button" onClick={startEditingSocialName}>Edit</button>
                        </div>
                      )}
                      <div className="arena-player-tags">
                        <button type="button" className="arena-code-plate" onClick={copyFriendCode} title="Copy player tag">
                          <span className="arena-code-key">#</span>
                          <span>{state.social.friendCode}</span>
                          <span className="arena-code-copy">⧉</span>
                        </button>
                        <button type="button" className="arena-code-plate" onClick={copyFriendInviteLink} title="Copy invite link">
                          <span className="arena-code-key">↗</span>
                          <span>Invite link</span>
                          <span className="arena-code-copy">⧉</span>
                        </button>
                        <button type="button" className="arena-code-plate profile-badges-button" onClick={() => setBadgesOpen(true)} title="View badges">
                          <span className="arena-code-key">★</span>
                          <span>Badges</span>
                          <span className="arena-code-copy">↗</span>
                        </button>
                        <span className={`arena-sync-pill ${state.social.lastSyncError ? "arena-sync-pill--error" : socialConfigured ? "arena-sync-pill--ready" : "arena-sync-pill--local"}`}>
                          <span />{state.social.lastSyncError ? "Sync Issue" : socialConfigured ? "Arena Synced" : "Local Only"}
                        </span>
                      </div>
                    </div>
                  </div>

                  <div className="arena-player-stats">
                    <div className="arena-mini-stat arena-mini-stat--daily"><span className="arena-mini-stat__icon">↯</span><div><strong>{formatMinutes(localSocialDaily.minutes)}</strong><span>Today · {localSocialDaily.sessions} ses.</span></div></div>
                    <div className="arena-mini-stat arena-mini-stat--weekly"><span className="arena-mini-stat__icon">◆</span><div><strong>{formatMinutes(localSocialWeekly.minutes)}</strong><span>This Week · {localSocialWeekly.sessions} ses.</span></div></div>
                    <div className="arena-mini-stat arena-mini-stat--overall"><span className="arena-mini-stat__icon">★</span><div><strong>{formatMinutes(localSocialOverall.minutes)}</strong><span>All Time · {localSocialOverall.sessions} ses.</span></div></div>
                    <div className="arena-mini-stat"><span className="arena-mini-stat__icon">📅</span><div><strong>{formatMinutes(localSocialMonthly.minutes)}</strong><span>This Month · {localSocialMonthly.sessions} ses.</span></div></div>
                    <div className="arena-mini-stat"><span className="arena-mini-stat__icon">⚔</span><div><strong>{state.social.isPrivate ? "Hidden" : myGlobalRank ? `#${myGlobalRank}` : "—"}</strong><span>Global Rank</span></div></div>
                    <div className="arena-mini-stat"><span className="arena-mini-stat__icon">👥</span><div><strong>{myFriendRank ? `#${myFriendRank}` : "—"}</strong><span>Friends Rank</span></div></div>
                  </div>
                </div>

                <div className="arena-sync-console">
                  <div>
                    <span className="arena-kicker">Cloud sync</span>
                    <p>Last synced: {lastSocialSyncLabel}</p>
                    {state.social.lastSyncError ? <p className="arena-error">{state.social.lastSyncError}</p> : null}
                    {!socialConfigured ? <p className="arena-error">Cloudflare Worker URL has not been configured yet.</p> : null}
                  </div>
                  <button type="button" className="arena-btn arena-btn--send" onClick={() => void runSocialSync()} disabled={socialSyncing || !socialConfigured}>
                    {socialSyncing ? "Syncing..." : "Sync Arena"}
                  </button>
                </div>

                <div className="profile-options">
                  <button type="button" className={`profile-toggle ${state.social.isPrivate ? "" : "active"}`} onClick={toggleProfilePrivacy}>
                    <strong>{state.social.isPrivate ? "Private profile" : "Public profile"}</strong>
                    <span>{state.social.isPrivate ? "Hidden from global feed and leaderboard." : "Shown on global feed and leaderboard."}</span>
                  </button>
                    <button type="button" className={`profile-toggle ${state.social.autoPostSessions ? "active" : ""}`} onClick={toggleAutoPostSessions}>
                      <strong>{state.social.autoPostSessions ? "Auto-post on" : "Auto-post off"}</strong>
                      <span>{state.social.autoPostSessions ? "Completed sessions queue feed posts automatically." : "You choose which sessions to post."}</span>
                    </button>
                    <button type="button" className={`profile-toggle ${state.social.showHoursToFriends ? "active" : ""}`} onClick={toggleShowHoursToFriends}>
                      <strong>Show hours to friends</strong>
                      <span>Your totals in the friends standings.</span>
                    </button>
                </div>
              </article>
            </div>
          ) : null}

          {socialSubtab === "squad" ? (
            <div className={`social-single-panel squad-panel ${appStyle === "wabi-sabi" ? "wabi-squad-panel" : ""}`}>
              {!currentSquad ? (
                <>
                  <article className="arena-panel squad-card">
                    <div className="arena-panel-head">
                      <span className="arena-panel-icon">S</span>
                      <div>
                        <span className="arena-kicker">Create squad</span>
                        <h3>Start a squad</h3>
                      </div>
                    </div>
                    <p className="squad-copy">Squads hold up to 4 players. Once you join one, you cannot create or join another until you leave.</p>
                    <form className="squad-form" onSubmit={submitSquadCreate}>
                      <input className="arena-input" value={squadNameDraft} onChange={(event) => setSquadNameDraft(event.target.value)} placeholder="Squad name" maxLength={48} disabled={!socialConfigured || socialSyncing} />
                      <button type="button" className={`profile-toggle squad-privacy-toggle ${squadPrivateDraft ? "" : "active"}`} onClick={() => setSquadPrivateDraft((value) => !value)} disabled={!socialConfigured || socialSyncing}>
                        <strong>{squadPrivateDraft ? "Private squad" : "Public squad"}</strong>
                        <span>{squadPrivateDraft ? "Players must request to join." : "Players can join instantly."}</span>
                      </button>
                      <button type="submit" className="arena-btn arena-btn--send" disabled={!socialConfigured || socialSyncing}>{socialSyncing ? "Working..." : "Create"}</button>
                    </form>
                  </article>

                  <article className="arena-panel squad-card">
                    <div className="arena-panel-head">
                      <span className="arena-panel-icon">⌕</span>
                      <div>
                        <span className="arena-kicker">Search all squads</span>
                        <h3>Find a squad</h3>
                      </div>
                    </div>
                    <form className="squad-form squad-search-form" onSubmit={submitSquadSearch}>
                      <input className="arena-input" value={squadSearchDraft} onChange={(event) => setSquadSearchDraft(event.target.value)} placeholder="Search by squad name" disabled={!socialConfigured || squadSearching} />
                      <button type="submit" className="arena-btn arena-btn--send" disabled={!socialConfigured || squadSearching}>{squadSearching ? "Searching..." : "Search"}</button>
                    </form>
                    <div className="squad-suggestion-head">
                      <div>
                        <strong>Suggested squads</strong>
                        <span>Don't know a name? Join or request one of these.</span>
                      </div>
                      {squadSuggestionPool.length > 4 ? (
                        <button type="button" className="arena-btn arena-btn--decline" onClick={() => void loadSquadSuggestions()} disabled={squadSuggestionsLoading}>{squadSuggestionsLoading ? "Loading..." : "Reload"}</button>
                      ) : null}
                    </div>
                    <div className="squad-search-results squad-suggestions">
                      {squadSuggestions.map((squad) => (
                        <div key={squad.id} className="squad-search-card">
                          <div>
                            <strong>{squad.name}</strong>
                            <span>{squad.isPrivate ? "Private" : "Public"} · {squad.memberCount}/{squad.maxMembers} members · {formatMinutes(squad.totalMinutes)}</span>
                          </div>
                          {squad.action === "join" || squad.action === "request" ? (
                            <button type="button" className="arena-btn arena-btn--accept" onClick={() => void joinOrRequestSquad(squad)} disabled={socialSyncing}>{squad.action === "join" ? "Join" : "Request"}</button>
                          ) : (
                            <span className="arena-pending-badge">{squad.action === "pending" ? "Pending" : squad.action === "full" ? "Full" : "Unavailable"}</span>
                          )}
                        </div>
                      ))}
                      {squadSuggestionsLoading ? <div className="arena-empty small">Loading suggestions...</div> : null}
                      {!squadSuggestionsLoading && !squadSuggestions.length ? <div className="arena-empty small">No squads exist yet. Create the first one.</div> : null}
                    </div>
                    {state.social.outgoingSquadRequests.length ? (
                      <div className="squad-request-note">Pending request: {state.social.outgoingSquadRequests.map((request) => request.squadName ?? "Squad").join(", ")}</div>
                    ) : null}
                    <div className="squad-result-head">Search results</div>
                    <div className="squad-search-results">
                      {squadSearchResults.map((squad) => (
                        <div key={squad.id} className="squad-search-card">
                          <div>
                            <strong>{squad.name}</strong>
                            <span>{squad.isPrivate ? "Private" : "Public"} · {squad.memberCount}/{squad.maxMembers} members · {formatMinutes(squad.totalMinutes)}</span>
                          </div>
                          {squad.action === "join" || squad.action === "request" ? (
                            <button type="button" className="arena-btn arena-btn--accept" onClick={() => void joinOrRequestSquad(squad)} disabled={socialSyncing}>{squad.action === "join" ? "Join" : "Request"}</button>
                          ) : (
                            <span className="arena-pending-badge">{squad.action === "pending" ? "Pending" : squad.action === "full" ? "Full" : "Unavailable"}</span>
                          )}
                        </div>
                      ))}
                      {!squadSearchResults.length ? <div className="arena-empty small">Search by name to discover public and private squads.</div> : null}
                    </div>
                  </article>
                </>
              ) : (
                <>
                  <article className="arena-panel squad-card squad-hq-card">
                    <div className="squad-hq-head">
                      <div className="arena-title-cluster">
                        <span className="arena-title-icon">S</span>
                        <div>
                          <span className="arena-kicker">{currentSquad.isPrivate ? "Private squad" : "Public squad"}</span>
                          <h2>{currentSquad.name}</h2>
                          <p>{currentSquad.memberCount}/4 members · Your rank: {squadRoleLabels[currentSquad.myRole]}</p>
                        </div>
                      </div>
                      <div className="squad-actions">
                        {currentSquad.myRole === "leader" ? <button type="button" className="arena-btn arena-btn--decline" onClick={startSquadSettingsEdit}>Edit</button> : null}
                        <button type="button" className="arena-btn arena-btn--decline" onClick={() => void leaveCurrentSquad()} disabled={socialSyncing}>Leave</button>
                      </div>
                    </div>
                    {squadSettingsEditing ? (
                      <form className="squad-form squad-settings-form" onSubmit={submitSquadSettings}>
                        <input className="arena-input" value={squadSettingsNameDraft} onChange={(event) => setSquadSettingsNameDraft(event.target.value)} maxLength={48} />
                        <button type="button" className={`profile-toggle squad-privacy-toggle ${squadSettingsPrivateDraft ? "" : "active"}`} onClick={() => setSquadSettingsPrivateDraft((value) => !value)} disabled={socialSyncing}>
                          <strong>{squadSettingsPrivateDraft ? "Private squad" : "Public squad"}</strong>
                          <span>{squadSettingsPrivateDraft ? "Players must request to join." : "Players can join instantly."}</span>
                        </button>
                        <button type="submit" className="arena-btn arena-btn--send" disabled={socialSyncing}>Save</button>
                        <button type="button" className="arena-btn arena-btn--decline" onClick={() => setSquadSettingsEditing(false)}>Cancel</button>
                      </form>
                    ) : null}
                    <div className="squad-stats-grid">
                      <div><strong>{formatMinutes(currentSquad.totalMinutes)}</strong><span>Total squad focus</span></div>
                      <div><strong>{currentSquad.totalSessions}</strong><span>Sessions</span></div>
                      <div><strong>{state.social.incomingSquadRequests.length}</strong><span>Pending requests</span></div>
                    </div>
                  </article>

                  <div className="squad-main-grid">
                    <article className="arena-panel squad-card squad-roster-panel">
                      <div className="arena-panel-head"><span className="arena-panel-icon">R</span><div><span className="arena-kicker">Squad roster</span><h3>Members</h3></div></div>
                      <div className="squad-member-list">
                        {currentSquad.members.map((member) => {
                          const expanded = expandedSquadMemberId === member.userId;
                          const assignableRoles = currentSquadRole && !member.isSelf ? getAssignableSquadRoles(currentSquadRole, member.role) : [];
                          const canKickMember = Boolean(currentSquadRole && !member.isSelf && canKickSquadMember(currentSquadRole, member.role));
                          const canManageMember = assignableRoles.length > 0 || canKickMember;
                          return (
                            <div key={member.userId} className={`squad-member-card ${expanded ? "squad-member-card--expanded" : ""}`}>
                              <button type="button" className="squad-member-summary" onClick={() => setExpandedSquadMemberId((current) => current === member.userId ? null : member.userId)}>
                                <ArenaAvatar name={member.displayName} avatar={member.isSelf ? state.social.avatar : member.avatar} self={member.isSelf} size="sm" />
                                <div className="squad-member-main"><strong>{member.displayName}{member.isSelf ? " (You)" : ""}</strong><span>{member.friendCode} · {formatMinutes(member.minutes)} · {member.sessions} sessions</span></div>
                                <span className={`squad-role-badge squad-role-badge--${member.role}`}>{squadRoleLabels[member.role]}</span>
                              </button>
                              {expanded ? (
                                <div className="squad-member-expanded">
                                  <div>
                                    <strong>{canManageMember ? `Manage ${member.displayName}` : member.isSelf ? "This is you" : "No actions available"}</strong>
                                    <span>{squadRoleLabels[member.role]} · joined {formatProfileSeenAt(member.joinedAt)}</span>
                                  </div>
                                  {assignableRoles.length ? (
                                    <div className="squad-member-role-actions">
                                      {assignableRoles.map((role) => (
                                        <button key={role} type="button" className={`arena-btn ${member.role === role ? "arena-btn--send" : "arena-btn--decline"}`} onClick={() => void changeSquadMemberRole(member.userId, role)} disabled={socialSyncing || member.role === role}>
                                          Make {squadRoleLabels[role]}
                                        </button>
                                      ))}
                                    </div>
                                  ) : null}
                                  {canKickMember ? <button type="button" className="arena-btn arena-btn--decline squad-kick-expanded" onClick={() => void kickFromSquad(member.userId, member.displayName)} disabled={socialSyncing}>Kick from squad</button> : null}
                                </div>
                              ) : null}
                            </div>
                          );
                        })}
                      </div>
                    </article>

                    <article className="arena-panel squad-card squad-internal-leaderboard-panel">
                      <div className="arena-panel-head"><span className="arena-panel-icon">⚔</span><div><span className="arena-kicker">Internal leaderboard</span><h3>Squad members</h3></div></div>
                      <div className="arena-period-chips squad-periods" aria-label="Internal squad leaderboard period">
                        {(["daily", "weekly", "overall"] as SocialLeaderboardPeriod[]).map((period) => <button key={period} type="button" className={socialPeriod === period ? "arena-period-chip arena-period-chip--active" : "arena-period-chip"} onClick={() => setSocialPeriod(period)}>{period === "daily" ? "Daily" : period === "weekly" ? "Weekly" : "Overall"}</button>)}
                      </div>
                      <div className="arena-lb-rows squad-lb-rows">
                        {squadMemberLeaderboard.map((entry) => {
                          const profileTarget = { userId: entry.userId, displayName: entry.displayName, friendCode: entry.friendCode, avatar: entry.isSelf ? state.social.avatar : entry.avatar };
                          return <ArenaLeaderboardRow key={entry.userId} entry={entry} selfAvatar={state.social.avatar} onProfile={() => void openFriendProfile(profileTarget)} />;
                        })}
                        {!squadMemberLeaderboard.length ? <div className="arena-empty small">Sync your squad to see member rankings.</div> : null}
                      </div>
                    </article>
                  </div>

                  <article className="arena-panel squad-card squad-chat-card">
                    <div className="arena-panel-head"><span className="arena-panel-icon">#</span><div><span className="arena-kicker">Squad chat</span><h3>Chat</h3></div></div>
                    <div className="squad-chat-list">
                      {state.social.squadMessages.map((message) => (
                        <div key={message.id} className={`squad-chat-message ${message.isSelf ? "squad-chat-message--self" : ""}`}>
                          <ArenaAvatar name={message.displayName} avatar={message.isSelf ? state.social.avatar : message.avatar} self={message.isSelf} size="sm" />
                          <div><strong>{message.displayName} <span>{squadRoleLabels[message.role]}</span></strong><p>{message.body}</p></div>
                          {message.isSelf ? <button type="button" className="squad-chat-delete" onClick={() => void deleteOwnSquadMessage(message.id)} title="Delete message">Delete</button> : null}
                        </div>
                      ))}
                      {!state.social.squadMessages.length ? <div className="arena-empty small">No messages yet. Start the squad chat.</div> : null}
                    </div>
                    <form className="squad-chat-form" onSubmit={submitSquadChat}>
                      <input className="arena-input" value={squadChatDraft} onChange={(event) => setSquadChatDraft(event.target.value)} placeholder="Message your squad" maxLength={500} />
                      <button type="submit" className="arena-btn arena-btn--send" disabled={!squadChatDraft.trim()}>Send</button>
                    </form>
                  </article>

                  {canManageCurrentSquadRequests ? (
                    <article className="arena-panel squad-card squad-requests-card">
                      <div className="arena-panel-head"><span className="arena-panel-icon">?</span><div><span className="arena-kicker">Private requests</span><h3>Join requests</h3></div></div>
                      {state.social.incomingSquadRequests.length ? state.social.incomingSquadRequests.map((request) => (
                        <div key={request.id} className="arena-request-card">
                          <ArenaAvatar name={request.displayName ?? "Student"} avatar={request.avatar} size="sm" />
                          <div className="arena-request-copy"><strong>{request.displayName}</strong><span>{request.friendCode}</span></div>
                          <div className="arena-request-actions">
                            <button type="button" className="arena-btn arena-btn--accept" onClick={() => void answerSquadRequest(request.id, "accepted")} disabled={socialSyncing}>Accept</button>
                            <button type="button" className="arena-btn arena-btn--decline" onClick={() => void answerSquadRequest(request.id, "declined")} disabled={socialSyncing}>Decline</button>
                          </div>
                        </div>
                      )) : <div className="arena-empty small">No pending squad requests.</div>}
                    </article>
                  ) : null}
                </>
              )}
            </div>
          ) : null}

          {socialSubtab === "friends" ? (
            <div className={`social-single-panel ${appStyle === "wabi-sabi" ? "wabi-friends-panel" : ""}`}>
              <article className="arena-panel arena-friends-panel">
                <div className="arena-panel-head">
                  <span className="arena-panel-icon">+</span>
                  <div>
                    <span className="arena-kicker">Player tags</span>
                    <h3>Friends</h3>
                  </div>
                  </div>

                  <div className="arena-invite-card">
                    <div>
                      <span className="arena-kicker">Invite link</span>
                      <p>Share this link in WhatsApp or anywhere else. It opens the download page with your player tag ready to copy.</p>
                    </div>
                    <input className="arena-input" value={friendInviteLink} readOnly onFocus={(event) => event.currentTarget.select()} />
                    <button type="button" className="arena-btn arena-btn--send" onClick={copyFriendInviteLink}>Copy invite link</button>
                  </div>

                  <form className="arena-add-friend" onSubmit={submitFriendRequest}>
                    <input className="arena-input" value={friendCodeDraft} onChange={(event) => setFriendCodeDraft(event.target.value.toUpperCase())} placeholder="Enter player tag, e.g. ABCD-1234" disabled={!socialConfigured} />
                    <button type="submit" className="arena-btn arena-btn--send" disabled={socialSyncing || !socialConfigured}>Send</button>
                  </form>

                <div className="arena-social-section">
                  <h4>Incoming <span>{state.social.incomingFriendRequests.length}</span></h4>
                  {state.social.incomingFriendRequests.length ? state.social.incomingFriendRequests.map((request) => (
                    <div key={request.id} className="arena-request-card">
                      <ArenaAvatar name={request.fromDisplayName} avatar={request.fromAvatar} size="sm" />
                      <div className="arena-request-copy"><strong>{request.fromDisplayName}</strong><span>{request.fromFriendCode}</span></div>
                      <div className="arena-request-actions">
                        <button type="button" className="arena-btn arena-btn--accept" onClick={() => void answerFriendRequest(request.id, "accepted")} disabled={socialSyncing}>Accept</button>
                        <button type="button" className="arena-btn arena-btn--decline" onClick={() => void answerFriendRequest(request.id, "declined")} disabled={socialSyncing}>Decline</button>
                      </div>
                    </div>
                  )) : <div className="arena-empty small">No incoming requests.</div>}
                </div>

                <div className="arena-social-section">
                  <h4>Pending <span>{state.social.outgoingFriendRequests.length}</span></h4>
                  {state.social.outgoingFriendRequests.length ? state.social.outgoingFriendRequests.map((request) => (
                    <div key={request.id} className="arena-request-card">
                      <ArenaAvatar name={request.toDisplayName} avatar={request.toAvatar} size="sm" />
                      <div className="arena-request-copy"><strong>{request.toDisplayName}</strong><span>{request.toFriendCode}</span></div>
                      <span className="arena-pending-badge">Pending</span>
                    </div>
                  )) : <div className="arena-empty small">No pending sent requests.</div>}
                </div>

                <div className="arena-social-section">
                  <h4>Your Friends <span>{state.social.friends.length}</span></h4>
                  {state.social.friends.length ? (
                    <div className="arena-friend-grid">
                      {state.social.friends.map((friend) => (
                        <div key={friend.userId} className="arena-friend-card" onClick={() => void openFriendProfile(friend)} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); openFriendProfile(friend); } }}>
                          <ArenaAvatar name={friend.displayName} avatar={friend.avatar} size="sm" />
                          <div><strong>{friend.displayName}</strong><span>{friend.friendCode}{friend.lastSeenAt ? ` · seen ${formatProfileSeenAt(friend.lastSeenAt)}` : ""}</span></div>
                        </div>
                      ))}
                    </div>
                  ) : <div className="arena-empty">Share your player tag to build your friends leaderboard.</div>}
                </div>
              </article>
            </div>
          ) : null}

          {socialSubtab === "leaderboard" ? (
            <article className={`arena-leaderboard ${appStyle === "wabi-sabi" ? "wabi-standings" : ""}`}>
              <div className="arena-leaderboard-head">
                <div className="arena-title-cluster">
                  <span className="arena-title-icon">⚔</span>
                  <div>
                    <span className="arena-kicker">{appStyle === "wabi-sabi" ? "Standings" : "Arena standings"}</span>
                    <h2>{appStyle === "wabi-sabi" ? socialScope === "global" ? "World" : socialScope === "squad" ? "Squad" : "Friends" : socialArenaTitle}</h2>
                    <p>{socialArenaSubtitle}</p>
                  </div>
                </div>
                <div className="arena-sync-status">
                  <span>Last synced: {lastSocialSyncLabel}</span>
                  <button type="button" className="arena-btn arena-btn--decline social-refresh-btn" onClick={() => void runSocialSync()} disabled={socialSyncing || !socialConfigured}>
                    {socialSyncing ? "Syncing..." : "Refresh"}
                  </button>
                </div>
              </div>

              <div className="arena-scope-toggle" aria-label="Leaderboard scope">
                {(["friends", "squad", "global"] as SocialLeaderboardScope[]).map((scope) => (
                  <button key={scope} type="button" className={socialScope === scope ? "arena-scope-btn arena-scope-btn--active" : "arena-scope-btn"} onClick={() => setSocialScope(scope)}>
                    {scope === "global" ? "World Arena" : scope === "squad" ? "Squad Arena" : "Friends Arena"}
                  </button>
                ))}
              </div>

              {socialScope === "squad" ? (
                <div className="arena-period-chips" aria-label="Squad leaderboard period">
                  {(["daily", "season", "overall"] as SocialSquadScorePeriod[]).map((period) => (
                    <button key={period} type="button" className={squadScorePeriod === period ? "arena-period-chip arena-period-chip--active" : "arena-period-chip"} onClick={() => setSquadScorePeriod(period)}>
                      {period === "daily" ? "Daily" : period === "season" ? "Seasonal Points" : "Overall Points"}
                    </button>
                  ))}
                </div>
              ) : (
                <div className="arena-period-chips" aria-label="Leaderboard period">
                  {(["daily", "weekly", "overall"] as SocialLeaderboardPeriod[]).map((period) => (
                    <button key={period} type="button" className={socialPeriod === period ? "arena-period-chip arena-period-chip--active" : "arena-period-chip"} onClick={() => setSocialPeriod(period)}>
                      {period === "daily" ? "Daily Sprint" : period === "weekly" ? "Weekly League" : "Hall of Focus"}
                    </button>
                  ))}
                </div>
              )}

              {socialScope === "global" && state.social.isPrivate ? (
                <div className="arena-empty small arena-private-notice">
                  <strong>Private profile enabled</strong>
                  <span>You are hidden from the global leaderboard. Switch to Friends Arena to compare with friends.</span>
                </div>
              ) : null}

              {socialScope === "squad" ? (
                <div className="arena-empty small arena-private-notice">
                  <strong>{SQUAD_SEASON_NAME}</strong>
                  <span>Season: {SQUAD_SEASON_RANGE_LABEL} · Overall tracking started: {SQUAD_TRACKING_START_LABEL}</span>
                </div>
              ) : null}

              {socialScope !== "squad" && appStyle === "field-notebook" ? (
                <div className="fn-lb-table">
                  <div className="fn-lb-head">
                    <span />
                    <span>Name</span>
                    <span>Hours</span>
                    <span>{socialPeriod === "daily" ? "Today" : socialPeriod === "overall" ? "All time" : "This week"}</span>
                    <span>Sessions</span>
                  </div>
                  <div className="arena-lb-rows">
                    {socialLeaderboard.map((entry) => {
                      const profileTarget = { userId: entry.userId, displayName: entry.displayName, friendCode: entry.friendCode, avatar: entry.isSelf ? state.social.avatar : entry.avatar };
                      const barPct = entry.minutes > 0 ? Math.max(4, Math.round((entry.minutes / socialLeaderboardTopMinutes) * 100)) : 0;
                      return (
                        <div key={entry.userId} className={`fn-lb-row ${entry.isSelf ? "fn-lb-row--self" : ""}`} onClick={() => void openFriendProfile(profileTarget)} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); openFriendProfile(profileTarget); } }}>
                          <span className="fn-lb-rank">{entry.rank}</span>
                          <span className="fn-lb-name">{entry.displayName}{entry.isSelf ? <em>{entry.friendCode}</em> : null}</span>
                          <span className="fn-lb-hours">{formatMinutes(entry.minutes)}</span>
                          <span className="fn-lb-bar"><i style={{ width: `${barPct}%` }} /></span>
                          <span className="fn-lb-sessions">{entry.sessions}</span>
                        </div>
                      );
                    })}
                    {!socialLeaderboard.length ? (
                      <div className="arena-empty">
                        <strong>No contenders yet</strong>
                        <span>Start studying and sync to claim your rank.</span>
                      </div>
                    ) : null}
                  </div>
                  {socialLeaderboard.length ? <p className="fn-lb-note">Bars are relative to the top of the board.</p> : null}
                </div>
              ) : null}

              {socialScope !== "squad" && appStyle !== "field-notebook" && socialLeaderboard.length >= 3 ? (
                <div className="arena-podium">
                  {[socialLeaderboard[1], socialLeaderboard[0], socialLeaderboard[2]].map((entry, index) => {
                    const profileTarget = { userId: entry.userId, displayName: entry.displayName, friendCode: entry.friendCode, avatar: entry.isSelf ? state.social.avatar : entry.avatar };
                    return (
                      <div key={entry.userId} className={`arena-podium-col ${entry.isSelf ? "arena-podium-col--self" : ""}`} onClick={() => void openFriendProfile(profileTarget)} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); openFriendProfile(profileTarget); } }}>
                        <ArenaAvatar name={entry.displayName} avatar={entry.isSelf ? state.social.avatar : entry.avatar} self={entry.isSelf} size={index === 1 ? "lg" : "md"} />
                        <strong>{entry.displayName}{entry.isSelf ? " (You)" : ""}</strong>
                        <span>{formatMinutes(entry.minutes)}</span>
                        <div className={`arena-podium-block arena-podium-block--${entry.rank}`}>
                          <ArenaRankBadge rank={entry.rank} large={index === 1} />
                          <small>{entry.sessions} sessions</small>
                        </div>
                      </div>
                    );
                  })}
                </div>
              ) : null}

              {socialScope === "squad" || appStyle !== "field-notebook" ? (
                <div className="arena-lb-rows">
                  {socialScope === "squad" ? squadScoreLeaderboard.map((entry) => (
                    <SquadArenaRow key={entry.squadId} entry={entry} period={squadScorePeriod} isSelf={entry.squadId === state.social.squad?.id} onOpen={() => void openSquadDetails(entry)} />
                  )) : (socialLeaderboard.length >= 3 ? socialLeaderboard.slice(3) : socialLeaderboard).map((entry) => {
                    const profileTarget = { userId: entry.userId, displayName: entry.displayName, friendCode: entry.friendCode, avatar: entry.isSelf ? state.social.avatar : entry.avatar };
                    return <ArenaLeaderboardRow key={entry.userId} entry={entry} selfAvatar={state.social.avatar} onProfile={() => void openFriendProfile(profileTarget)} />;
                  })}
                  {socialScope === "squad" && !squadScoreLeaderboard.length ? (
                    <div className="arena-empty">
                      <strong>No eligible squads yet</strong>
                      <span>Squads need at least 2 members to enter the Squad Arena.</span>
                    </div>
                  ) : null}
                  {socialScope !== "squad" && !socialLeaderboard.length ? (
                    <div className="arena-empty">
                      <strong>No contenders yet</strong>
                      <span>Start studying and sync to claim your rank.</span>
                    </div>
                  ) : null}
                </div>
              ) : null}
            </article>
          ) : null}
        </section>
      ) : null}

      {expandedFeedImage?.imageUrl ? (
        <div className="feed-image-lightbox" onClick={() => setExpandedFeedImageId(null)} role="presentation">
          <button type="button" className="feed-image-lightbox__frame" onClick={() => setExpandedFeedImageId(null)} aria-label="Close fullscreen feed image">
            <img src={`${expandedFeedImage.imageUrl}${expandedFeedImage.imageUrl.includes("?") ? "&" : "?"}v=${encodeURIComponent(expandedFeedImage.imageExpiresAt ?? expandedFeedImage.createdAt)}`} alt={`${expandedFeedImage.displayName}'s feed post image`} />
          </button>
        </div>
      ) : null}

      {viewingFriend ? (
        <div className="calendar-drawer-backdrop" style={{ justifyContent: "center", alignItems: "center" }} onClick={() => setViewingFriend(null)} role="presentation">
          <article className="arena-player-card" style={{ width: "min(440px, 100%)", padding: 0, alignSelf: "center" }} onClick={(e) => e.stopPropagation()} role="dialog" aria-label={`${viewingFriend.displayName}'s profile`}>
            <div className="arena-player-card__inner">
              <div className="arena-title-cluster" style={{ marginBottom: 16 }}>
                <ArenaAvatar name={viewingFriend.displayName} avatar={viewingFriend.avatar} size="lg" />
                <div style={{ flex: 1 }}>
                  <span className="arena-kicker">{viewingFriend.friendCode}</span>
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <h3 style={{ margin: 0 }}>{viewingFriend.displayName}</h3>
                    <button type="button" className="arena-icon-button" onClick={() => setViewingFriend(null)} title="Close" style={{ marginLeft: "auto" }}>×</button>
                  </div>
                  {viewingFriend.lastSeenAt ? <span style={{ color: "var(--muted)", fontSize: "0.82rem" }}>Seen {formatProfileSeenAt(viewingFriend.lastSeenAt)}</span> : null}
                  <div className="profile-action-row">
                    <span className="arena-pending-badge">{viewingIsSelf ? "Your profile" : viewingIsFriend ? "Friend" : viewingRequestPending ? "Request pending" : "Not friends"}</span>
                    {!viewingIsSelf && !viewingIsFriend && !viewingRequestPending ? (
                      <button type="button" className="arena-btn arena-btn--send" onClick={() => void sendFriendRequestToCode(viewingFriend.friendCode)} disabled={socialSyncing || !socialConfigured}>Send friend request</button>
                    ) : null}
                  </div>
                </div>
              </div>
              {viewingFriendLoading ? (
                <div className="arena-empty">Loading stats...</div>
              ) : viewingFriendStats?.hoursVisible && viewingFriendStats.daily && viewingFriendStats.weekly && viewingFriendStats.overall ? (
                <div className="arena-player-stats" style={{ marginTop: 0 }}>
                  <div className="arena-mini-stat"><span className="arena-mini-stat__icon">↯</span><div><strong>{formatMinutes(viewingFriendStats.daily.minutes)}</strong><span>Today · {viewingFriendStats.daily.sessions} ses.</span></div></div>
                  <div className="arena-mini-stat"><span className="arena-mini-stat__icon">◆</span><div><strong>{formatMinutes(viewingFriendStats.weekly.minutes)}</strong><span>This Week · {viewingFriendStats.weekly.sessions} ses.</span></div></div>
                  <div className="arena-mini-stat"><span className="arena-mini-stat__icon">★</span><div><strong>{formatMinutes(viewingFriendStats.overall.minutes)}</strong><span>All Time · {viewingFriendStats.overall.sessions} ses.</span></div></div>
                  <div className="arena-mini-stat"><span className="arena-mini-stat__icon">📅</span><div><strong>{viewingFriendStats.daily.lastActiveDate || "—"}</strong><span>Last Active</span></div></div>
                </div>
              ) : viewingFriendStats ? (
                <div className="arena-empty">This friend has chosen not to share their study hours.</div>
              ) : (
                <div className="arena-error">Could not load stats.</div>
              )}
            </div>
          </article>
        </div>
      ) : null}

      {viewingSquadEntry ? (
        <div className="calendar-drawer-backdrop" style={{ justifyContent: "center", alignItems: "center" }} onClick={() => { setViewingSquadEntry(null); setViewingSquadDetails(null); }} role="presentation">
          <article className="arena-player-card squad-details-modal" onClick={(e) => e.stopPropagation()} role="dialog" aria-label={`${viewingSquadEntry.squadName} squad details`}>
            <div className="arena-player-card__inner">
              <div className="arena-title-cluster squad-details-head">
                <ArenaRankBadge rank={viewingSquadEntry.rank} large />
                <div style={{ flex: 1 }}>
                  <span className="arena-kicker">{viewingSquadDetails?.isPrivate ?? viewingSquadEntry.isPrivate ? "Private squad" : "Public squad"}</span>
                  <div className="squad-details-title-row">
                    <h3>{viewingSquadDetails?.name ?? viewingSquadEntry.squadName}</h3>
                    <button type="button" className="arena-icon-button" onClick={() => { setViewingSquadEntry(null); setViewingSquadDetails(null); }} title="Close">×</button>
                  </div>
                  <div className="profile-action-row">
                    <span className="arena-pending-badge">
                      {viewingSquadAction === "current" ? "Your squad" : viewingSquadAction === "pending" ? "Request pending" : viewingSquadAction === "full" ? "Full" : viewingSquadAction === "unavailable" ? "Unavailable" : viewingSquadAction === "request" ? "Request to join" : "Open to join"}
                    </span>
                    {viewingSquadAction === "join" || viewingSquadAction === "request" ? (
                      <button type="button" className="arena-btn arena-btn--send" onClick={() => void joinOrRequestViewedSquad()} disabled={socialSyncing || !socialConfigured}>
                        {viewingSquadAction === "join" ? "Join squad" : "Request to join"}
                      </button>
                    ) : null}
                  </div>
                </div>
              </div>

              {viewingSquadLoading ? (
                <div className="arena-empty">Loading squad...</div>
              ) : viewingSquadDetails ? (
                <>
                  <div className="arena-player-stats squad-details-stats" style={{ marginTop: 0 }}>
                    <div className="arena-mini-stat"><span className="arena-mini-stat__icon">#</span><div><strong>{viewingSquadDetails.memberCount}/{viewingSquadDetails.maxMembers}</strong><span>Members</span></div></div>
                    <div className="arena-mini-stat"><span className="arena-mini-stat__icon">◆</span><div><strong>{formatMinutes(viewingSquadDetails.totalMinutes)}</strong><span>Today</span></div></div>
                    <div className="arena-mini-stat"><span className="arena-mini-stat__icon">★</span><div><strong>{viewingSquadEntry.points} pts</strong><span>{squadScorePeriod === "season" ? "Season" : squadScorePeriod === "overall" ? "Overall" : "Today if held"}</span></div></div>
                    <div className="arena-mini-stat"><span className="arena-mini-stat__icon">↯</span><div><strong>{formatMinutes(Math.round(viewingSquadDetails.previousDayAverageMinutes ?? 0))}</strong><span>Yesterday avg</span></div></div>
                  </div>

                  <div className="squad-details-roster">
                    <div className="section-label">Members</div>
                    {viewingSquadDetails.members.map((member) => (
                      <div key={member.userId} className="squad-details-member">
                        <ArenaAvatar name={member.displayName} avatar={member.isSelf ? state.social.avatar : member.avatar} self={member.isSelf} size="sm" />
                        <div>
                          <strong>{member.displayName}{member.isSelf ? " (You)" : ""}</strong>
                          <span>{squadRoleLabels[member.role]} · today {formatMinutes(member.minutes)} · {member.sessions} sessions</span>
                        </div>
                      </div>
                    ))}
                  </div>
                </>
              ) : (
                <div className="arena-error">Could not load squad details.</div>
              )}
            </div>
          </article>
        </div>
      ) : null}

      {badgesOpen ? (
        <div className="calendar-drawer-backdrop profile-badges-backdrop" onClick={() => setBadgesOpen(false)} role="presentation">
          <article className="profile-badges-modal" onClick={(event) => event.stopPropagation()} role="dialog" aria-label="Profile badges">
            <div className="profile-badges-head">
              <div>
                <span className="arena-kicker">Profile collection</span>
                <h3>Badges</h3>
                <p>Hover or focus a badge to see how to unlock it.</p>
              </div>
              <button type="button" className="arena-icon-button" onClick={() => setBadgesOpen(false)} title="Close">×</button>
            </div>

            <div className="profile-badges-groups">
              {profileBadgeGroups.map((group) => (
                <section key={group.category} className="profile-badge-group">
                  <div className="profile-badge-group-head">
                    <h4>{group.category}</h4>
                    <span>{group.source}</span>
                  </div>
                  <div className="profile-badge-grid">
                    {group.badges.map(renderProfileBadgeCard)}
                  </div>
                  {group.subgroups?.map((subgroup) => (
                    <div key={subgroup.category} className="profile-badge-subgroup">
                      <div className="profile-badge-subgroup-head">
                        <h5>{subgroup.category}</h5>
                        <span>{subgroup.source}</span>
                      </div>
                      <div className="profile-badge-grid">
                        {subgroup.badges.map(renderProfileBadgeCard)}
                      </div>
                    </div>
                  ))}
                </section>
              ))}
            </div>
          </article>
        </div>
      ) : null}

      {profileAvatarEditorOpen ? (
        <div className="calendar-drawer-backdrop" style={{ justifyContent: "center", alignItems: "center" }} onClick={closeProfileAvatarEditor} role="presentation">
          <article className="profile-avatar-editor" onClick={(event) => event.stopPropagation()} role="dialog" aria-label="Edit profile avatar">
            <button type="button" className="arena-icon-button profile-avatar-close" onClick={closeProfileAvatarEditor} title="Close">×</button>
            <div className="profile-avatar-editor-head">
              <ArenaAvatar name={state.social.displayName} avatar={profileAvatarDraft} self size="lg" />
              <div>
                <span className="arena-kicker">Profile picture</span>
                <h3>Choose your arena mark</h3>
                <p>Friends will see this after your next arena sync.</p>
              </div>
            </div>

            <div className="profile-avatar-mode-toggle" aria-label="Avatar type">
              <button type="button" className={profileAvatarDraft.kind === "letter" ? "active" : ""} onClick={() => setProfileAvatarDraft({ kind: "letter", letter: getFirstAvatarLetter(state.social.displayName), style: "classic" })}>Letter</button>
              <button type="button" className={profileAvatarDraft.kind === "icon" ? "active" : ""} onClick={() => setProfileAvatarDraft({ kind: "icon", icon: avatarIcons[0] })}>Icon</button>
              <button type="button" className={profileAvatarDraft.kind === "photo" ? "active" : ""} onClick={() => setProfileAvatarDraft(profileAvatarDraft.kind === "photo" ? profileAvatarDraft : { kind: "photo", name: "", url: "", mimeType: "image/webp" })}>Photo</button>
            </div>

            {profileAvatarDraft.kind === "letter" ? (
              <div className="profile-avatar-panel">
                <div className="profile-avatar-grid profile-avatar-grid--styles">
                  {avatarStyles.map((style) => {
                    const avatar: SocialAvatar = { kind: "letter", letter: profileAvatarDraft.letter, style: style.id };
                    const selected = profileAvatarDraft.kind === "letter" && profileAvatarDraft.style === style.id;
                    return (
                      <button key={style.id} type="button" className={`profile-avatar-choice ${selected ? "selected" : ""}`} onClick={() => setProfileAvatarDraft(avatar)}>
                        <ArenaAvatar name={state.social.displayName} avatar={avatar} self size="md" />
                        <span>{style.label}</span>
                      </button>
                    );
                  })}
                </div>
                <button type="button" className="ghost-button small-button profile-avatar-change-letter" onClick={() => setProfileAvatarLetterPickerOpen((open) => !open)}>
                  Change letter
                </button>
                {profileAvatarLetterPickerOpen ? (
                  <div className="profile-avatar-letter-picker" aria-label="Choose avatar letter">
                    {alphabetLetters.map((letter) => (
                      <button key={letter} type="button" className={profileAvatarDraft.letter === letter ? "selected" : ""} onClick={() => setProfileAvatarDraft({ ...profileAvatarDraft, letter })}>
                        {letter}
                      </button>
                    ))}
                  </div>
                ) : null}
              </div>
            ) : profileAvatarDraft.kind === "icon" ? (
              <div className="profile-avatar-grid profile-avatar-grid--icons">
                {avatarIcons.map((icon) => {
                  const avatar: SocialAvatar = { kind: "icon", icon };
                  const selected = profileAvatarDraft.kind === "icon" && profileAvatarDraft.icon === icon;
                  return (
                    <button key={icon} type="button" className={`profile-avatar-choice ${selected ? "selected" : ""}`} onClick={() => setProfileAvatarDraft(avatar)}>
                      <ArenaAvatar name={state.social.displayName} avatar={avatar} self size="md" />
                    </button>
                  );
                })}
              </div>
            ) : (
              <div className="profile-avatar-panel">
                <button type="button" className="profile-avatar-photo-upload" onClick={() => profileAvatarFileInputRef.current?.click()}>
                  {profileAvatarDraft.url ? "Change photo" : "Choose a photo"}
                </button>
                <input ref={profileAvatarFileInputRef} type="file" accept="image/png,image/jpeg,image/webp" hidden onChange={(event) => void handleProfileAvatarPhotoChange(event)} />
                {profileAvatarDraft.url ? (
                  <button type="button" className="ghost-button small-button profile-avatar-photo-remove" onClick={() => setProfileAvatarDraft({ kind: "photo", name: "", url: "", mimeType: "image/webp" })}>Remove photo</button>
                ) : null}
                <p className="profile-avatar-photo-hint">Square photos look best. Up to 512px, under 1 MB. PNG, JPEG, or WebP (no GIFs).</p>
              </div>
            )}

            <div className="profile-avatar-actions">
              <button type="button" className="ghost-button small-button" onClick={closeProfileAvatarEditor}>Cancel</button>
              <button type="button" className="arena-btn arena-btn--send" onClick={() => void saveProfileAvatar()} disabled={socialSyncing}>Save</button>
            </div>
          </article>
        </div>
      ) : null}

      {avatarCropOpen && avatarCropSource ? (
        <div className="calendar-drawer-backdrop" style={{ justifyContent: "center", alignItems: "center" }} onClick={cancelAvatarCrop} role="presentation">
          <article className="profile-crop-editor" onClick={(event) => event.stopPropagation()} role="dialog" aria-label="Crop profile picture">
            <div className="profile-crop-head">
              <div>
                <span className="arena-kicker">Profile picture</span>
                <h3>Crop your photo</h3>
                <p>Drag to reposition, scroll or use the slider to zoom. The result is a square picture.</p>
              </div>
            </div>
            <div
              className={`profile-crop-stage ${avatarCropDragging ? "profile-crop-stage--dragging" : ""}`}
              style={{ width: AVATAR_CROP_VIEWPORT_PX, height: AVATAR_CROP_VIEWPORT_PX }}
              onPointerDown={handleAvatarCropPointerDown}
              onPointerMove={handleAvatarCropPointerMove}
              onPointerUp={handleAvatarCropPointerEnd}
              onPointerCancel={handleAvatarCropPointerEnd}
              onWheel={handleAvatarCropWheel}
            >
              <img
                src={avatarCropSource.url}
                alt="Profile picture to crop"
                draggable={false}
                style={{
                  width: avatarCropSource.width * Math.max(AVATAR_CROP_VIEWPORT_PX / avatarCropSource.width, AVATAR_CROP_VIEWPORT_PX / avatarCropSource.height) * avatarCrop.zoom,
                  height: avatarCropSource.height * Math.max(AVATAR_CROP_VIEWPORT_PX / avatarCropSource.width, AVATAR_CROP_VIEWPORT_PX / avatarCropSource.height) * avatarCrop.zoom,
                  transform: `translate(${AVATAR_CROP_VIEWPORT_PX / 2 - avatarCrop.x * avatarCropSource.width * Math.max(AVATAR_CROP_VIEWPORT_PX / avatarCropSource.width, AVATAR_CROP_VIEWPORT_PX / avatarCropSource.height) * avatarCrop.zoom}px, ${AVATAR_CROP_VIEWPORT_PX / 2 - avatarCrop.y * avatarCropSource.height * Math.max(AVATAR_CROP_VIEWPORT_PX / avatarCropSource.width, AVATAR_CROP_VIEWPORT_PX / avatarCropSource.height) * avatarCrop.zoom}px)`,
                }}
              />
              <div className="profile-crop-grid" aria-hidden="true" />
            </div>
            <div className="profile-crop-zoom-row">
              <span aria-hidden="true">−</span>
              <input type="range" min={1} max={AVATAR_CROP_MAX_ZOOM} step={0.01} value={avatarCrop.zoom} onChange={(event) => handleAvatarCropZoomChange(Number(event.target.value))} aria-label="Zoom" />
              <span aria-hidden="true">+</span>
            </div>
            <div className="profile-avatar-actions">
              <button type="button" className="ghost-button small-button" onClick={cancelAvatarCrop}>Cancel</button>
              <button type="button" className="arena-btn arena-btn--send" onClick={() => void confirmAvatarCrop()}>Apply crop</button>
            </div>
          </article>
        </div>
      ) : null}

      {state.activeTab === "break" && appStyle === "wabi-sabi" ? renderWabiBreakRoom() : null}

      {state.activeTab === "break" && appStyle !== "wabi-sabi" ? (
        <section className={`break-grid ${appStyle === "field-notebook" ? "fn-break" : ""}`}>
          <article className="panel-card break-main-card">
            <div className="section-head">
              <div>
                <p className="eyebrow">Recharge</p>
                <h2>Break Room</h2>
              </div>
              <div className="page-head-actions">
                <span className="section-note">{effectiveUnlocked.length} of {STUDY_BREAK_GAME_COUNT} breaks available</span>
              </div>
            </div>

            <div className="break-xp-bar-wrap" data-tour="break-xp">
              <div className="break-xp-bar">
                <div className="break-xp-fill" style={{ width: `${xpPercent}%` }} />
                <div className="break-xp-tick" style={{ left: "25%" }} />
                <div className="break-xp-tick" style={{ left: "50%" }} />
                <div className="break-xp-tick" style={{ left: "75%" }} />
              </div>
              <span className="break-xp-label">
                XP: {xpProgress} / 45 — ~{minsUntilNext} min
              </span>
              {streakEmoji ? <span className="break-streak-badge">{streakEmoji} {state.unlockStreak}-day streak</span> : null}
            </div>

            <p className="break-quote">{"\u201C"}{quote.text}{"\u201D"} — {quote.author}</p>

            <div className="break-card-grid" data-tour="break-games">
              {studyBreakGames.map((game) => {
                const unlocked = effectiveUnlocked.includes(game.name);
                return (
                  <div
                    key={game.name}
                    className={`break-game-card ${unlocked ? "unlocked" : "locked"} ${celebrating === game.name ? "celebrating" : ""}`}
                    onAnimationEnd={() => setCelebrating(null)}
                  >
                    <div className="break-game-main">
                      <strong>{game.name}</strong>
                      <span>{game.desc}</span>
                    </div>
                    <div className="break-game-side">
                      {unlocked ? (
                        game.name === "Daily Durak" ? (
                          <div className="break-game-durak">
                            <span className="break-durak-progress">{(state.durakPuzzle.solvedCount || 0)}/3</span>
                            <button type="button" className="design-chip" data-tour="break-game-action" onClick={() => { logPlayedBreak(game.name); setShowDurakPuzzle(true); }}>
                              Play
                            </button>
                          </div>
                        ) : game.name === "Wordle" ? (
                          <div className="break-game-durak">
                            {wordlePuzzleIsToday && state.wordlePuzzle.completed ? <span className="break-durak-progress">{state.wordlePuzzle.won ? "Solved" : "Failed"}</span> : <span />}
                            <button type="button" className="design-chip" data-tour="break-game-action" onClick={() => { logPlayedBreak(game.name); setShowWordlePuzzle(true); }}>
                              Play
                            </button>
                          </div>
                        ) : game.name === "Travle" ? (
                          <div className="break-game-durak">
                            {travlePuzzleIsToday && state.travlePuzzle.completed ? <span className="break-durak-progress">{state.travlePuzzle.won ? "Solved" : "Failed"}</span> : <span />}
                            <button type="button" className="design-chip" data-tour="break-game-action" onClick={() => { logPlayedBreak(game.name); setShowTravlePuzzle(true); }}>
                              Play
                            </button>
                          </div>
                        ) : game.name === "Geodle" ? (
                          <div className="break-game-durak">
                            {geodlePuzzleIsToday && state.geodlePuzzle.completed ? <span className="break-durak-progress">{state.geodlePuzzle.won ? "Solved" : "Failed"}</span> : <span />}
                            <button type="button" className="design-chip" data-tour="break-game-action" onClick={() => { logPlayedBreak(game.name); setShowGeodlePuzzle(true); }}>
                              Play
                            </button>
                          </div>
                        ) : game.name === "Flaggle" ? (
                          <div className="break-game-durak">
                            {flagglePuzzleIsToday && state.flagglePuzzle.completed ? <span className="break-durak-progress">{state.flagglePuzzle.won ? "Solved" : "Failed"}</span> : <span />}
                            <button type="button" className="design-chip" data-tour="break-game-action" onClick={() => { logPlayedBreak(game.name); setShowFlagglePuzzle(true); }}>
                              Play
                            </button>
                          </div>
                        ) : (
                          <a href={game.url} target="_blank" rel="noreferrer" className="design-chip" data-tour="break-game-action" onClick={() => logPlayedBreak(game.name)}>
                            Play
                          </a>
                        )
                      ) : canUnlockMore ? (
                        <button type="button" className="break-unlock-btn" data-tour="break-game-action" onClick={() => { unlockGame(game.name); setCelebrating(game.name); }}>
                          Unlock
                        </button>
                      ) : (
                        <span className="break-lock">~{minsUntilNext} min</span>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>

            <div className="break-stats-row" data-tour="break-stats">
              <span className="break-stats-chip">{'\u{1F513}'} Unlocked {effectiveUnlocked.length}/{STUDY_BREAK_GAME_COUNT}</span>
              <span className="break-stats-chip">{'\u25B6'} Played {todayPlayedNames.length} today</span>
              {badgeFullHouse ? <span className="break-stats-chip collection">{'\u{1F3C6}'} Full house!</span> : null}
            </div>

            <div className="break-water-row" data-tour="break-water">
              <button type="button" className="break-water-btn" onClick={addWater}>{'\u{1F4A7}'}</button>
              <span className="break-water-count">{waterCount} glass{waterCount !== 1 ? "es" : ""} today</span>
              <button type="button" className="break-water-btn plus" onClick={addWater}>+</button>
            </div>

            <div className="break-badge-heading">Achievements</div>

            <div className="break-badge-row" data-tour="break-achievements">
              {badges.map(b => (
                <div key={b.id} className={`break-badge ${b.earned ? "earned" : ""}`}>
                  <span>{b.icon}</span>
                  <span>{b.name}</span>
                </div>
              ))}
            </div>

            <div className="break-stretch-card" data-tour="break-stretch">
              <span className="stretch-icon">{'\u{1F9D8}'}</span>
              <span className="stretch-text">{stretch}</span>
              <button type="button" className="stretch-refresh" onClick={() => setStretchIndex(i => (i + 1) % stretchIdeas.length)}>
                {'\u21BB'}
              </button>
            </div>

            <div className="break-pet-rock" data-tour="break-rock" onClick={patRock} role="button" tabIndex={0} onKeyDown={(e) => { if (e.repeat) return; if (e.key === "Enter" || e.key === " ") { e.preventDefault(); patRock(); } }}>
              <div className="rock-area">
                <span className={`rock ${rockBounce ? "bounce" : ""} ${rockCelebrating ? "celebrate" : ""}`} onAnimationEnd={() => setRockCelebrating(false)}>{'\u{1FAA8}'}</span>
                {rockStage.plant ? <span className="rock-plant">{rockStage.plant}</span> : null}
              </div>
              <span className="rock-pats">{rockStage.label} · {state.petRockPats} pat{state.petRockPats !== 1 ? "s" : ""}</span>
            </div>
          </article>
        </section>
      ) : null}

      {(showTravlePuzzle || showFlagglePuzzle || showGeodlePuzzle || showWordlePuzzle || showDurakPuzzle) ? createPortal(<>
      {showTravlePuzzle ? (
        <div className="travle-overlay" onClick={() => setShowTravlePuzzle(false)} role="presentation">
          <div className="travle-modal" onClick={(event) => event.stopPropagation()} role="dialog" aria-label="Daily Travle puzzle">
            <div className="travle-head">
              <div>
                <p className="travle-kicker">Daily Travle</p>
                <h2>Build the border route</h2>
                <span>{TRAVLE_COUNTRY_COUNT} connected countries · {travleGuessesLeft} step{travleGuessesLeft === 1 ? "" : "s"} left</span>
              </div>
              <button type="button" className="travle-close-btn" onClick={() => setShowTravlePuzzle(false)} aria-label="Close Travle puzzle">✕</button>
            </div>

            <div className="travle-target-card">
              <div>
                <span>Start</span>
                <strong>{state.travlePuzzle.start}</strong>
              </div>
              <div>
                <span>Destination</span>
                <strong>{state.travlePuzzle.target}</strong>
              </div>
            </div>

            <div className="travle-map-card">
              <svg className="travle-map" viewBox={travleMapViewBox} role="img" aria-label={`Map route from ${state.travlePuzzle.start} to ${state.travlePuzzle.target}`}>
                <rect className="travle-map-water" x="0" y="0" width="1000" height="500" />
                <g className="travle-map-graticule" aria-hidden="true">
                  {[125, 250, 375].map((y) => <line key={`lat-${y}`} x1="0" y1={y} x2="1000" y2={y} />)}
                  {[250, 500, 750].map((x) => <line key={`lon-${x}`} x1={x} y1="0" x2={x} y2="500" />)}
                </g>
                <g>
                  {TRAVLE_MAP_COUNTRIES.map((country) => {
                    const isStart = country.code === travleRouteCodes[0];
                    const isTarget = country.code === travleTargetCode;
                    const isRoute = travleRouteCodeSet.has(country.code);
                    const countryName = travleCountryNameByCode.get(country.code);
                    const guessState = countryName ? travleGuessStates[countryName] : undefined;
                    const isCurrent = country.code === travleCurrentCode;
                    const isSolvedPath = travleSolvedCodeSet.has(country.code);
                    const className = [
                      "travle-map-country",
                      isRoute && guessState === "possible" ? "possible" : "",
                      isRoute && guessState === "miss" ? "miss" : "",
                      (isRoute && guessState === "route") || isSolvedPath ? "route" : "",
                      isStart ? "start" : "",
                      isTarget ? "target" : "",
                      isCurrent ? "current" : "",
                    ].filter(Boolean).join(" ");
                    if (country.d) return <path key={country.code} className={className} d={country.d} />;
                    if (country.point) return <circle key={country.code} className={`${className} marker`} cx={country.point[0]} cy={country.point[1]} r="3.6" />;
                    return null;
                  })}
                </g>
                {travleMapRoutePoints ? <polyline className="travle-map-route-line" points={travleMapRoutePoints} /> : null}
              </svg>
              <div className="travle-map-controls" aria-label="Map zoom controls">
                <button type="button" onClick={() => setTravleMapZoom((zoom) => Math.min(2.5, Math.round((zoom + 0.35) * 100) / 100))} aria-label="Zoom in">+</button>
                <button type="button" onClick={() => setTravleMapZoom((zoom) => Math.max(1, Math.round((zoom - 0.35) * 100) / 100))} aria-label="Zoom out">−</button>
              </div>
              <span className="travle-map-watermark">travle</span>
            </div>

            <div className="travle-route" aria-label="Current Travle route">
              {travleRoute.map((country, index) => (
                <span key={`${country}-${index}`} className="travle-route-node">{country}</span>
              ))}
              {!state.travlePuzzle.won ? <span className="travle-route-target">{state.travlePuzzle.target}</span> : null}
            </div>

            <div className="travle-current-card">
              <span>Latest guess</span>
              <strong>{travleCurrent}</strong>
              <small>{state.travlePuzzle.guesses.length}/{TRAVLE_MAX_GUESSES} countries guessed</small>
            </div>

            {state.travlePuzzle.completed ? (
              <div className={`travle-result-card ${state.travlePuzzle.won ? "won" : "lost"}`}>
                <p className="travle-result-kicker">{state.travlePuzzle.won ? "Arrived" : "Route missed"}</p>
                <h3>{state.travlePuzzle.won ? "Route complete" : "Better route found"}</h3>
                <div className="travle-result-route">
                  <span>{state.travlePuzzle.won ? "Your route" : "Shortest route"}</span>
                  <strong>{(state.travlePuzzle.won ? travleSolvedPath : travleShortestPath).join(" -> ")}</strong>
                </div>
                <div className="travle-result-stats">
                  <div>
                    <span>Guesses used</span>
                    <strong>{state.travlePuzzle.guesses.length}/{TRAVLE_MAX_GUESSES}</strong>
                  </div>
                  <div>
                    <span>Shortest</span>
                    <strong>{Math.max(0, travleShortestPath.length - 1)} steps</strong>
                  </div>
                  <div>
                    <span>{state.travlePuzzle.won ? "Solved route" : "Your route"}</span>
                    <strong>{state.travlePuzzle.won ? `${Math.max(0, travleSolvedPath.length - 1)} steps` : `${state.travlePuzzle.guesses.length} guesses`}</strong>
                  </div>
                </div>
                <div className="travle-result-legend">
                  <span><i className="route" /> Route</span>
                  <span><i className="possible" /> Useful</span>
                  <span><i className="miss" /> Off route</span>
                </div>
                <div className="travle-result-actions">
                  <button type="button" className="travle-result-close" onClick={() => setShowTravlePuzzle(false)}>Close</button>
                </div>
              </div>
            ) : (
              <>
                <div className="travle-input-row">
                  <div className="travle-combo">
                    <input
                      value={travleDraft}
                      onChange={(event) => { setTravleDraft(event.target.value); setTravleDropdownOpen(true); setTravleMessage(""); }}
                      onFocus={() => setTravleDropdownOpen(true)}
                      onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); submitTravleGuess(); } }}
                      placeholder="Enter a country"
                    />
                    {travleDraft ? <button type="button" className="travle-input-icon" onClick={() => { setTravleDraft(""); setTravleDropdownOpen(true); }} aria-label="Clear country">×</button> : null}
                    <button type="button" className="travle-input-icon travle-input-icon--dropdown" onClick={() => setTravleDropdownOpen((open) => !open)} aria-label="Show countries">▾</button>
                    {travleDropdownOpen ? (
                      <div className="travle-dropdown">
                        {travleOptions.length ? travleOptions.map((country) => (
                          <button key={country.code} type="button" onClick={() => selectTravleCountry(country.name)}>
                            <strong>{country.name}</strong>
                            <span>{country.region}</span>
                          </button>
                        )) : <p>No connected countries found.</p>}
                      </div>
                    ) : null}
                  </div>
                  <button type="button" className="travle-submit" onClick={submitTravleGuess}>Step</button>
                </div>

                <div className="travle-status">
                  {travleMessage || `Enter countries to connect ${state.travlePuzzle.start} to ${state.travlePuzzle.target}.`}
                </div>

                <div className="travle-shortest">
                  <span>Shortest route</span>
                  <strong>{travleShortestPath.length ? `${travleShortestPath.length - 1} steps` : "Hidden"}</strong>
                </div>
              </>
            )}
          </div>
        </div>
      ) : null}

      {showFlagglePuzzle ? (
        <div className="flaggle-overlay" onClick={() => setShowFlagglePuzzle(false)} role="presentation">
          <div className="flaggle-modal" onClick={(event) => event.stopPropagation()} role="dialog" aria-label="Daily Flaggle puzzle">
            <div className="flaggle-head">
              <div>
                <p className="flaggle-kicker">Daily Flaggle</p>
                <h2>Guess the flag</h2>
                <span>{FLAGGLE_COUNTRY_COUNT} countries · {flaggleGuessesLeft} guess{flaggleGuessesLeft === 1 ? "" : "es"} left</span>
              </div>
              <button type="button" className="flaggle-close-btn" onClick={() => setShowFlagglePuzzle(false)} aria-label="Close Flaggle puzzle">✕</button>
            </div>

            <div className="flaggle-preview-card">
              {flagglePreviewDataUrl ? (
                <img src={flagglePreviewDataUrl} alt="Revealed target flag clue" />
              ) : (
                <div className="flaggle-preview-empty">Guess a country to reveal matching flag colors.</div>
              )}
            </div>

            <div className="flaggle-input-row">
              <div className="flaggle-combo">
                <input
                  value={flaggleDraft}
                  onChange={(event) => { setFlaggleDraft(event.target.value); setFlaggleDropdownOpen(true); setFlaggleMessage(""); }}
                  onFocus={() => setFlaggleDropdownOpen(true)}
                  onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); void submitFlaggleGuess(); } }}
                  placeholder="Choose a country"
                  disabled={state.flagglePuzzle.completed || flaggleProcessing}
                />
                {flaggleDraft ? <button type="button" className="flaggle-input-icon" onClick={() => { setFlaggleDraft(""); setFlaggleDropdownOpen(true); }} aria-label="Clear country">×</button> : null}
                <button type="button" className="flaggle-input-icon flaggle-input-icon--dropdown" onClick={() => setFlaggleDropdownOpen((open) => !open)} aria-label="Show countries">▾</button>
                {flaggleDropdownOpen && !state.flagglePuzzle.completed ? (
                  <div className="flaggle-dropdown">
                    {flaggleOptions.length ? flaggleOptions.map((country) => (
                      <button key={country.code} type="button" onClick={() => selectFlaggleCountry(country.name)}>
                        <img src={getFlagImageSrc(country.name)} alt={`${country.name} flag`} />
                        <strong>{country.name}</strong>
                      </button>
                    )) : <p>No countries found.</p>}
                  </div>
                ) : null}
              </div>
              <button type="button" className="flaggle-submit" onClick={() => void submitFlaggleGuess()} disabled={state.flagglePuzzle.completed || flaggleProcessing}>{flaggleProcessing ? "Working..." : "Guess"}</button>
            </div>

            <div className="flaggle-status">
              {state.flagglePuzzle.completed ? (
                state.flagglePuzzle.won ? `Solved in ${state.flagglePuzzle.guesses.length}.` : `The flag was ${state.flagglePuzzle.answer}.`
              ) : flaggleMessage || "Only colors shared with the target flag stay visible."}
            </div>

            <div className="flaggle-history">
              {flaggleRows.length ? flaggleRows.map((guess) => (
                <div key={guess.country} className="flaggle-guess-row">
                  <strong>{guess.country}</strong>
                  <span>{guess.similarity.toFixed(1)}%</span>
                  <img src={getFlagImageSrc(guess.country)} alt={`${guess.country} flag`} />
                </div>
              )) : <div className="flaggle-empty-history">No guesses yet.</div>}
            </div>
          </div>
        </div>
      ) : null}

      {showGeodlePuzzle ? (
        <div className="geodle-overlay" onClick={() => setShowGeodlePuzzle(false)} role="presentation">
          <div className="geodle-modal" onClick={(event) => event.stopPropagation()} role="dialog" aria-label="Daily Geodle puzzle">
            <div className="geodle-head">
              <div>
                <p className="geodle-kicker">Daily Geodle</p>
                <h2>Guess the country</h2>
                <span>{GEODLE_COUNTRY_COUNT} countries · {geodleGuessesLeft} guess{geodleGuessesLeft === 1 ? "" : "es"} left</span>
              </div>
              <button type="button" className="geodle-close-btn" onClick={() => setShowGeodlePuzzle(false)} aria-label="Close Geodle puzzle">✕</button>
            </div>

            <div className="geodle-input-row">
              <div className="geodle-combo">
                <input
                  value={geodleDraft}
                  onChange={(event) => { setGeodleDraft(event.target.value); setGeodleDropdownOpen(true); setGeodleMessage(""); }}
                  onFocus={() => setGeodleDropdownOpen(true)}
                  onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); submitGeodleGuess(); } }}
                  placeholder="Country"
                  disabled={state.geodlePuzzle.completed}
                />
                {geodleDraft ? <button type="button" className="geodle-input-icon" onClick={() => { setGeodleDraft(""); setGeodleDropdownOpen(true); }} aria-label="Clear country">×</button> : null}
                <button type="button" className="geodle-input-icon geodle-input-icon--dropdown" onClick={() => setGeodleDropdownOpen((open) => !open)} aria-label="Show countries">▾</button>
                {geodleDropdownOpen && !state.geodlePuzzle.completed ? (
                  <div className="geodle-dropdown">
                    {geodleOptions.length ? geodleOptions.map((country) => (
                      <button key={country.code} type="button" onClick={() => selectGeodleCountry(country.name)}>
                        <strong>{country.name}</strong>
                        <span>{country.continent}</span>
                      </button>
                    )) : <p>No countries found.</p>}
                  </div>
                ) : null}
              </div>
              <button type="button" className="geodle-submit" onClick={submitGeodleGuess} disabled={state.geodlePuzzle.completed}>Submit</button>
            </div>

            <div className="geodle-status">
              {state.geodlePuzzle.completed ? (
                state.geodlePuzzle.won ? `Solved in ${state.geodlePuzzle.guesses.length}.` : `The country was ${state.geodlePuzzle.answer}.`
              ) : geodleMessage || "Use the clues after each guess to narrow it down."}
            </div>

            <div className="geodle-table-wrap">
              <table className="geodle-table">
                <thead>
                  <tr>
                    <th>Country</th>
                    {scoreGeodleGuess(state.geodlePuzzle.answer, state.geodlePuzzle.answer).map((clue) => <th key={clue.label}>{clue.label}</th>)}
                  </tr>
                </thead>
                <tbody>
                  {geodleRows.map((row) => (
                    <tr key={row.guess}>
                      <td className="geodle-country-cell">{row.guess}</td>
                      {row.clues.map((clue) => (
                        <td key={clue.label}>
                          <span className={`geodle-clue geodle-clue--${clue.state}`} data-hint={clue.hint} tabIndex={0} aria-label={clue.hint}>
                            {clue.state === "higher" ? "▲" : clue.state === "lower" ? "▼" : clue.state === "match" ? "✓" : clue.state === "close" ? "≈" : "×"}
                            <small>{clue.value}</small>
                          </span>
                        </td>
                      ))}
                    </tr>
                  ))}
                  {!geodleRows.length ? (
                    <tr>
                      <td className="geodle-empty-row" colSpan={7}>No guesses yet.</td>
                    </tr>
                  ) : null}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      ) : null}

      {showWordlePuzzle ? (
        <div className="wordle-overlay" onClick={() => setShowWordlePuzzle(false)} role="presentation">
          <div ref={wordleModalRef} className="wordle-modal" onClick={(event) => event.stopPropagation()} onKeyDown={handleWordleKeyDown} role="dialog" aria-label="Daily Wordle puzzle" tabIndex={-1}>
            <div className="wordle-head">
              <div>
                <p className="wordle-kicker">Daily Wordle</p>
                <h2>Find the 5-letter word</h2>
                <span>{WORDLE_ANSWER_COUNT.toLocaleString()} answers · {WORDLE_ACCEPTED_GUESS_COUNT.toLocaleString()} accepted guesses</span>
              </div>
              <div className="wordle-head-actions">
                <button type="button" className={`wordle-hard-toggle ${state.wordlePuzzle.hardMode ? "active" : ""}`} onClick={toggleWordleHardMode} disabled={wordleHardModeLocked} aria-pressed={state.wordlePuzzle.hardMode}>
                  <span>Hard mode</span>
                  <strong>{state.wordlePuzzle.hardMode ? "On" : "Off"}</strong>
                </button>
                {wordleHardModeLocked ? <small className="wordle-hard-lock">Locked during puzzle</small> : null}
                <button type="button" className="wordle-close-btn" onClick={() => setShowWordlePuzzle(false)} aria-label="Close Wordle puzzle">✕</button>
              </div>
            </div>

            <div className="wordle-board" aria-label="Wordle guesses">
              {wordleRows.map((row, rowIndex) => (
                <div key={rowIndex} className="wordle-row">
                  {row.map((cell, colIndex) => (
                    <span key={`${rowIndex}-${colIndex}`} className={`wordle-tile ${cell.letter ? "filled" : ""} ${cell.state ? `wordle-tile--${cell.state}` : ""}`}>
                      {cell.letter.toUpperCase()}
                    </span>
                  ))}
                </div>
              ))}
            </div>

            <div className="wordle-status">
              {state.wordlePuzzle.completed ? (
                state.wordlePuzzle.won ? `Solved in ${state.wordlePuzzle.guesses.length}.` : `The word was ${state.wordlePuzzle.answer.toUpperCase()}.`
              ) : wordleMessage || "Type a guess, then press Enter."}
            </div>

            <div className="wordle-keyboard" aria-label="Wordle keyboard">
              {["qwertyuiop", "asdfghjkl", "zxcvbnm"].map((row, rowIndex) => (
                <div key={row} className="wordle-keyboard-row">
                  {rowIndex === 2 ? <button type="button" className="wordle-key wide" onClick={submitWordleGuess}>Enter</button> : null}
                  {row.split("").map((letter) => (
                    <button key={letter} type="button" className={`wordle-key ${wordleKeyboardState[letter] ? `wordle-key--${wordleKeyboardState[letter]}` : ""}`} onClick={() => addWordleLetter(letter)} disabled={state.wordlePuzzle.completed}>
                      {letter.toUpperCase()}
                    </button>
                  ))}
                  {rowIndex === 2 ? <button type="button" className="wordle-key wide" onClick={handleWordleBackspace} disabled={state.wordlePuzzle.completed}>⌫</button> : null}
                </div>
              ))}
            </div>
          </div>
        </div>
      ) : null}

      {showDurakPuzzle ? (
        <div className="durak-overlay" onClick={() => setShowDurakPuzzle(false)} role="presentation">
          <div className="durak-modal" onClick={(e) => e.stopPropagation()} role="dialog" aria-label="Daily Durak puzzle">
            {state.durakPuzzle.solvedCount >= 3 && !durakGameState ? (
              <div className="durak-finished">
                <div className="durak-result durak-result--win">
                  <span className="durak-result-icon">🎉</span>
                  <span className="durak-result-text">All 3 puzzles solved for today!</span>
                </div>
                <button type="button" className="durak-btn durak-btn--close" onClick={() => setShowDurakPuzzle(false)}>Close</button>
              </div>
            ) : durakGameState ? (<div className="durak-game">
            <div className="durak-head">
              <div className="durak-hint-area">
                <span className="durak-hint-label">Hint</span>
                <span className="durak-hint">{state.durakPuzzle.hint || "CPU has some strong cards..."}</span>
              </div>
              <div className="durak-trump-area">
                <span className="durak-trump-label">Trump</span>
                <span className="durak-trump-card">{SUIT_SYMBOL[durakGameState.trumpSuit]}</span>
              </div>
              <button type="button" className="durak-close-btn" onClick={() => setShowDurakPuzzle(false)} aria-label="Close puzzle">✕</button>
            </div>

            <div className="durak-cpu-area">
              <div className="durak-cpu-label">CPU ({durakGameState.cpuHand.length})</div>
              <div className="durak-cpu-cards">
                {durakGameState.cpuHand.map((_, i) => (
                  <div key={i} className="durak-card durak-card--back">
                    <span className="durak-card-back-inner">?</span>
                  </div>
                ))}
              </div>
            </div>

            <div className="durak-message">{durakGameState.message}</div>

            <div className="durak-table-area">
              {durakGameState.table.length === 0 ? (
                <div className="durak-table-empty">Play your cards to attack</div>
              ) : (
                <div className="durak-table-grid">
                  {durakGameState.table.map((entry, i) => {
                    const isActive = !entry.defense;
                    const attacksOnTop = durakGameState.phase === "player_defense" ? true : durakGameState.phase === "cpu_defense" ? false : entry.attackBy === "cpu";
                    const attackCard = (
                      <div className="durak-card durak-card--table" style={{ color: SUIT_COLOR[entry.attack.suit] }}>
                        <span className="durak-rank">{entry.attack.rank}</span>
                        <span className="durak-suit">{SUIT_SYMBOL[entry.attack.suit]}</span>
                      </div>
                    );
                    const defenseSlot = entry.defense ? (
                      <div className="durak-card durak-card--table" style={{ color: SUIT_COLOR[entry.defense.suit] }}>
                        <span className="durak-rank">{entry.defense.rank}</span>
                        <span className="durak-suit">{SUIT_SYMBOL[entry.defense.suit]}</span>
                      </div>
                    ) : (
                      <div className="durak-card durak-card--undefended">
                        <span className="durak-card-empty">?</span>
                      </div>
                    );
                    return (
                      <div key={i} className={`durak-col ${isActive ? "durak-col--active" : ""}`}>
                        {attacksOnTop ? attackCard : defenseSlot}
                        {attacksOnTop ? defenseSlot : attackCard}
                      </div>
                    );
                  })}
                </div>
              )}
            </div>

            <div className="durak-hand-area">
              <div className="durak-hand-label">Your hand ({durakGameState.playerHand.length})</div>
              <div className="durak-hand-cards">
                {durakGameState.playerHand.map((card, i) => {
                  const isSelected = durakSelected.includes(i);
                  const canPlay = durakGameState.phase === "player_attack" || durakGameState.phase === "player_throw"
                    ? true
                    : durakGameState.phase === "player_defense"
                      ? durakGameState.table.some((e) => !e.defense) && canBeat(card, durakGameState.table.find((e) => !e.defense)!.attack, durakGameState.trumpSuit)
                      : false;
                  return (
                    <div
                      key={i}
                      className={`durak-card durak-card--hand ${isSelected ? "durak-card--selected" : ""} ${canPlay ? "durak-card--playable" : ""}`}
                      style={{ color: SUIT_COLOR[card.suit] }}
                      onClick={() => handleDurakCardClick(i)}
                    >
                      <span className="durak-rank">{card.rank}</span>
                      <span className="durak-suit">{SUIT_SYMBOL[card.suit]}</span>
                    </div>
                  );
                })}
              </div>
            </div>

            {durakGameState.phase === "finished" ? (
              <div className="durak-finished">
                {durakGameState.winner === "player" ? (
                  <>
                    <div className="durak-result durak-result--win">
                      <span className="durak-result-icon">🎉</span>
                      <span className="durak-result-text">You solved the puzzle!</span>
                    </div>
                    <div className="durak-failures-display">{'\u{274C}'} Failures: {state.durakPuzzle.failures}</div>
                    <button type="button" className="durak-btn durak-btn--close" onClick={() => setShowDurakPuzzle(false)}>Close</button>
                  </>
                ) : (
                  <>
                    <div className="durak-result durak-result--lose">
                      <span className="durak-result-icon">💀</span>
                      <span className="durak-result-text">CPU wins! Try a different approach.</span>
                    </div>
                    <button type="button" className="durak-btn durak-btn--primary" onClick={resetDurakAfterFail}>Try Again</button>
                  </>
                )}
              </div>
            ) : (
              <>
                <div className="durak-actions">
                  {(durakGameState.phase === "player_attack") && (
                    <button type="button" className="durak-btn durak-btn--primary" onClick={handleDurakAttack} disabled={durakSelected.length === 0}>
                      Attack
                    </button>
                  )}
                  {durakGameState.phase === "player_throw" && (
                    <>
                      <button type="button" className="durak-btn durak-btn--primary" onClick={() => handleDurakThrow()} disabled={durakSelected.length === 0}>
                        Throw
                      </button>
                      <button type="button" className="durak-btn durak-btn--secondary" onClick={() => handleDurakThrow(true)}>
                        Pass
                      </button>
                    </>
                  )}
                  {durakGameState.phase === "player_defense" && (() => {
                    const selectedCard = durakSelected.length > 0 ? durakGameState!.playerHand[durakSelected[0]] : null;
                    const target = durakGameState!.table.find((e) => !e.defense);
                    const canDefend = selectedCard !== null && target != null && canBeat(selectedCard, target.attack, durakGameState!.trumpSuit);
                    const canSlide = selectedCard !== null && getLegalSlideCards(durakGameState!, "player").some((slideCard) => slideCard.suit === selectedCard.suit && slideCard.rank === selectedCard.rank);
                    return (
                      <>
                        <button type="button" className="durak-btn durak-btn--primary" onClick={handleDurakDefend} disabled={!canDefend}>
                          Defend
                        </button>
                        <button type="button" className="durak-btn durak-btn--accent" onClick={() => { if (selectedCard) handleDurakSlide(selectedCard); }} disabled={!canSlide}>
                          Slide
                        </button>
                        <button type="button" className="durak-btn durak-btn--danger" onClick={() => { setDurakSelected([]); handleDurakPickUp(); }}>
                          Pick up
                        </button>
                      </>
                    );
                  })()}
                </div>
                <div className="durak-footer">
                  <span className="durak-failures">{'\u{274C}'} Failures: {state.durakPuzzle.failures}</span>
                </div>
              </>
            )}
            </div>
            ) : null}
          </div>
        </div>
      ) : null}
      </>, document.body) : null}

      {fullscreen && state.timer.phase !== "idle" ? (
        <div className="exam-fullscreen-overlay">
          <button
            type="button"
            className="exam-fullscreen-exit"
            onClick={() => setFullscreen(false)}
          >
            Minimize
          </button>

          <div ref={examFaceRingRef} className={`exam-fullscreen-face ${state.timer.running ? "running" : "paused"}`} style={{ "--aura-color": timerCourse?.color ?? "var(--accent)" } as CSSProperties}>
            <div className="timer-phase-row">
              <span className="timer-phase-pill">{state.timer.phase === "exam" ? "Exam" : state.timer.phase === "stopwatch" ? "Stopwatch" : state.timer.phase === "break" ? "Break" : "Study"}</span>
              <span className="timer-course-dot" style={{ background: timerCourse?.color ?? "var(--accent)" }} />
              <span>{timerTask?.title ?? timerCourse?.name ?? "General focus"}</span>
            </div>
            <strong><TimerClockDigits timer={state.timer} formatClock={formatClock} /></strong>
            <p>{state.timer.running ? "In session" : "Paused"} · {formatMinutes(getTimerMinutes(state.timer))} logged</p>
          </div>

          <div className="exam-fullscreen-actions">
            <button
              type="button"
              className="timer-primary-action"
              onClick={pauseTimer}
            >
              <span>{state.timer.running ? "▷" : "▶"}</span>
              {state.timer.running ? "Pause" : "Resume"}
            </button>
            <button
              type="button"
              className="timer-save-action"
              onClick={completeSessionManually}
            >
              ▣ Save
            </button>
            <button type="button" className="timer-reset-action" onClick={resetTimer} title="Reset">
              ↻
            </button>
          </div>
        </div>
      ) : null}
    </div>
    </>
  );
}

export default App;
