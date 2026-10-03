// Stage 20 visual/behaviour-parity fixtures: a synthetic production backup (see gen-fixture.mjs)
// with deterministic Break Room state on top. Entirely synthetic; nothing read from any profile.
//
//   node gen-break-fixture.mjs <variant> <base-backup.json> <out.json> --today YYYY-MM-DD [--tz +02:00]
//
// variant:
//   early     30 minutes studied today, nothing unlocked, 3 pats (the first-day Break Room)
//   unlocked  210 minutes today (4 tokens), Daily Durak/Wordle/Geodle unlocked and played,
//             counters and badges filled in, 1234 pats, today's Wordle/Geodle/Flaggle half played
//   full      300 minutes today, all six unlocked and played (Full House + Perfectionist)
//   empty     no sessions at all, nothing earned (empty album)
//
// Stage 21: [--travle fresh|mid|won|lost] stores today's Travle puzzle (salt "fixture-travle-salt",
// start/target from production's travle.ts) with no guesses, a three-guess route in progress
// (route, route, off route), the shortest route guessed (won) or seven off-route guesses (lost).
// Without it the stored puzzle is stale and production draws today's on open, as before.
//
// The daily puzzles use fixed seed salts and production's own answer functions (desktop/src/lib,
// copied read-only to a temp dir and run with Node's type stripping), so both apps load the same
// "today" puzzle and keep the stored guesses.
import { cpSync, mkdtempSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { join, dirname } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const [variant, basePath, outPath, ...rest] = process.argv.slice(2);
const opt = (k, d) => (rest.indexOf(k) >= 0 ? rest[rest.indexOf(k) + 1] : d);
const today = opt("--today");
const tz = opt("--tz", "+02:00");
if (!variant || !basePath || !outPath || !today) throw new Error("usage: gen-break-fixture.mjs <variant> <base> <out> --today YYYY-MM-DD");
const desktop = join(here, "..", "..", "..", "desktop");

const lib = mkdtempSync(join(tmpdir(), "st20-fixlib-"));
for (const f of ["wordle.ts", "wordleWords.ts", "geodle.ts", "countries.ts", "travle.ts", "countryBorders.ts"]) {
  cpSync(join(desktop, "src/lib", f), join(lib, f));
  const p = join(lib, f);
  writeFileSync(p, readFileSync(p, "utf8").replace(/from "\.\/([A-Za-z]+)"/g, 'from "./$1.ts"'));
}
const wordle = await import(pathToFileURL(join(lib, "wordle.ts")).href);
const geodle = await import(pathToFileURL(join(lib, "geodle.ts")).href);
const travle = await import(pathToFileURL(join(lib, "travle.ts")).href);
rmSync(lib, { recursive: true, force: true });

const backup = JSON.parse(readFileSync(basePath, "utf8"));
const s = backup.state;
const at = (h, m) => new Date(`${today}T${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:00${tz}`).toISOString();
const localDay = (isoString) => {
  const shifted = new Date(new Date(isoString).getTime() + (tz[0] === "-" ? -1 : 1) * (Number(tz.slice(1, 3)) * 60 + Number(tz.slice(4, 6))) * 60000);
  return shifted.toISOString().slice(0, 10);
};
// Replace whatever the base had today with exactly the minutes this variant needs.
const todaySessions = { early: [[9, 0, 30]], unlocked: [[8, 0, 50], [10, 0, 80], [13, 0, 80]], full: [[7, 0, 100], [9, 0, 100], [13, 0, 100]], empty: [] }[variant];
if (!todaySessions) throw new Error(`unknown variant ${variant}`);
s.sessions = variant === "empty" ? [] : s.sessions.filter((x) => localDay(x.endedAt) !== today);
todaySessions.forEach(([h, m, minutes], i) => {
  const start = at(h, m);
  const end = new Date(new Date(start).getTime() + minutes * 60000).toISOString();
  s.sessions.push({ id: `break-session-${i}`, semesterId: "sem-active", courseId: "course-0", taskId: null, kind: "study", goal: "", learned: "", blocker: "", nextStep: "", confidence: 3, startedAt: start, endedAt: end, minutes, presetLabel: "Deep Work 52/17" });
});
s.lifetimeStudyMinutes = s.sessions.filter((x) => x.kind !== "break").reduce((a, x) => a + x.minutes, 0);
s.lifetimeStudySessions = s.sessions.filter((x) => x.kind !== "break").length;
if (variant === "empty") { s.lifetimeStudyMinutes = 0; s.lifetimeStudySessions = 0; s.tasks = []; }

const ALL = ["Daily Durak", "Wordle", "Travle", "Flaggle", "Daily Skribbl", "Geodle"];
Object.assign(s, {
  unlockedGames: [], unlockedGamesDate: "", playedBreaks: [], playedBreaksDate: "", totalUnlocks: 0, unlockStreak: 0, lastUnlockDate: "",
  speedrunnerToday: false, playedGamesAllTime: [], badgeCounts: {}, badgeCountDates: {}, waterGlasses: 0, waterDate: "", petRockPats: 0,
  achievementBoard: [], achievementEarnedOnDates: {},
});
const wordleSalt = "fixture-wordle-salt";
const geodleSalt = "fixture-geodle-salt";
const flaggleSalt = "fixture-flaggle-salt";
const travleSalt = "fixture-travle-salt";
const puzzle = (salt, extra) => ({ seedSalt: salt, activeDate: today, puzzleId: wordle.getWordlePuzzleId(today, salt), guesses: [], completed: false, won: false, ...extra });
s.wordlePuzzle = puzzle(wordleSalt, { answer: wordle.getWordleAnswerForDate(today, wordleSalt), hardMode: false });
s.geodlePuzzle = puzzle(geodleSalt, { answer: geodle.getGeodleAnswerForDate(today, geodleSalt) });
s.flagglePuzzle = puzzle(flaggleSalt, { answer: geodle.getGeodleAnswerForDate(today, flaggleSalt) });
// Travle's start/target need the Stage 21 border graph; a stale day makes production regenerate it.
s.travlePuzzle = { seedSalt: travleSalt, activeDate: "", puzzleId: "", start: "", target: "", guesses: [], completed: false, won: false };
const travleState = opt("--travle");
if (travleState) {
  const p = travle.getTravlePuzzleForDate(today, travleSalt);
  const inner = p.shortestPath.slice(1, -1);
  const states = (guesses) => travle.getTravleGuessStates(p.start, p.target, guesses);
  const offRoute = travle.filterTravleCountries("").map((c) => c.name).filter((n) => n !== p.start && n !== p.target && states([n])[n] === "miss");
  const guesses = { fresh: [], mid: [inner[0], inner[1], offRoute[0]], won: inner, lost: offRoute.slice(0, 7) }[travleState];
  if (!guesses) throw new Error(`unknown --travle ${travleState}`);
  const won = travle.isTravleRouteSolved(p.start, p.target, guesses);
  s.travlePuzzle = { seedSalt: travleSalt, activeDate: today, puzzleId: travle.getTravlePuzzleId(today, travleSalt), start: p.start, target: p.target, guesses, completed: won || guesses.length >= travle.TRAVLE_MAX_GUESSES, won };
}

if (variant === "early") s.petRockPats = 3;
if (variant === "unlocked" || variant === "full") {
  const unlocked = variant === "full" ? ALL : ["Daily Durak", "Wordle", "Geodle"];
  const plays = variant === "full" ? ALL.map((n, i) => [n, 8, 10 + i]) : [["Wordle", 8, 30], ["Geodle", 11, 5], ["Daily Durak", 11, 40]];
  Object.assign(s, {
    unlockedGames: unlocked, unlockedGamesDate: today,
    playedBreaks: plays.map(([name, h, m]) => ({ name, playedAt: at(h, m) })), playedBreaksDate: today,
    totalUnlocks: variant === "full" ? 14 : 12, unlockStreak: 1, lastUnlockDate: today, speedrunnerToday: true,
    playedGamesAllTime: variant === "full" ? ALL : ["Wordle", "Geodle", "Daily Durak", "Flaggle"],
    badgeCounts: { "early-bird": 3, speedrunner: 2, "full-house": 1 }, badgeCountDates: { "early-bird": today, speedrunner: today, "full-house": "2026-09-12" },
    waterGlasses: 2, waterDate: today, petRockPats: 1234,
    achievementEarnedOnDates: { "first-break": "2026-08-30", "rock-sprouting": "2026-09-01", veteran: "2026-09-20", "rock-cosmic": "2026-09-28" },
  });
  // Today's puzzles, part-way through, with production-valid stored guesses.
  const w = s.wordlePuzzle;
  w.guesses = ["slate", "crony"].filter((g) => g !== w.answer);
  const countries = ["Switzerland", "Brazil", "Japan"].filter((c) => c !== s.geodlePuzzle.answer);
  s.geodlePuzzle.guesses = countries;
  // Production keeps a Flaggle guess only with a string maskedFlagDataUrl (never displayed).
  s.flagglePuzzle.guesses = ["Italy", "Germany"].filter((c) => c !== s.flagglePuzzle.answer).map((country, i) => ({ country, similarity: [38.4, 12.9][i], maskedFlagDataUrl: "data:image/png;base64," }));
}
backup.preferences = backup.preferences ?? {};
writeFileSync(outPath, JSON.stringify(backup, null, 2));
console.log(`${variant}${travleState ? `+travle-${travleState}` : ""}: travle=${JSON.stringify(s.travlePuzzle.guesses.length ? s.travlePuzzle : s.travlePuzzle.start || "stale")} today ${today}, ${todaySessions.reduce((a, x) => a + x[2], 0)} min today, wordle=${s.wordlePuzzle.answer} geodle=${s.geodlePuzzle.answer} flaggle=${s.flagglePuzzle.answer} -> ${outPath}`);
