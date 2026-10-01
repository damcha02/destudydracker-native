// Stage 20: golden fixtures for the Break Room domain, computed by PRODUCTION's own TypeScript
// (desktop/src/lib/*.ts, copied read-only into a temp dir and run with Node's built-in type
// stripping - nothing in desktop/ is modified or executed in place).
//
//   node scripts/stage20-goldens.mjs [--desktop ../desktop]
//
// Writes crates/study-tracker-core/tests/fixtures/break_room/*.json. Every choice below that needs
// randomness uses a fixed mulberry32 so the fixtures are reproducible; Durak's hint uses
// Math.random(), which is stubbed per call to make the chosen hint known.
import { cpSync, mkdtempSync, readdirSync, readFileSync, writeFileSync, mkdirSync, rmSync } from "node:fs";
import { join, dirname } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const args = Object.fromEntries(process.argv.slice(2).reduce((acc, a, i, all) => (a.startsWith("--") ? [...acc, [a.slice(2), all[i + 1]]] : acc), []));
const desktop = args.desktop ?? join(here, "..", "..", "desktop");
const out = join(here, "..", "crates", "study-tracker-core", "tests", "fixtures", "break_room");
mkdirSync(out, { recursive: true });

const lib = mkdtempSync(join(tmpdir(), "st20-prodlib-"));
for (const f of ["wordle.ts", "wordleWords.ts", "geodle.ts", "countries.ts", "durak.ts"]) {
  cpSync(join(desktop, "src/lib", f), join(lib, f));
  const p = join(lib, f);
  writeFileSync(p, readFileSync(p, "utf8").replace(/from "\.\/([A-Za-z]+)"/g, 'from "./$1.ts"'));
}
const imp = (f) => import(pathToFileURL(join(lib, f)).href);
const wordle = await imp("wordle.ts");
const words = await imp("wordleWords.ts");
const geodle = await imp("geodle.ts");
const { COUNTRIES } = await imp("countries.ts");
const durak = await imp("durak.ts");

function mulberry32(seed) {
  return function () {
    seed |= 0; seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const rng = mulberry32(20);
const pick = (list) => list[Math.floor(rng() * list.length)];
const write = (name, value) => writeFileSync(join(out, name), JSON.stringify(value) + "\n");

// ---------- daily seeding ----------
const salts = ["salt-a", "3f2b8c1e-7d4a-4c55-9a0e-1b2c3d4e5f60", "x", ""];
const dates = ["2025-12-31", "2026-01-01", "2026-01-02", "2026-01-31", "2026-02-01", "2026-02-28", "2026-02-30", "2026-03-01",
  "2026-09-30", "2026-10-01", "2026-12-31", "2027-01-01", "2028-02-28", "2028-02-29", "2028-03-01", "2031-07-15", "", "garbage", "2026-13-01", "2026-02-32"];
for (let i = 0; i < 40; i++) { const d = new Date(Date.UTC(2026, 9, 1 + i)); dates.push(d.toISOString().slice(0, 10)); }
write("daily.json", {
  wordle: salts.flatMap((salt) => dates.map((date) => ({ salt, date, answer: wordle.getWordleAnswerForDate(date, salt), id: wordle.getWordlePuzzleId(date, salt) }))),
  geodle: salts.flatMap((salt) => dates.map((date) => ({ salt, date, answer: geodle.getGeodleAnswerForDate(date, salt), id: geodle.getGeodlePuzzleId(date, salt) }))),
});

// ---------- wordle ----------
const canonical = [["speed", "abide"], ["eerie", "there"], ["abbey", "babes"], ["allee", "apple"], ["sassy", "glass"], ["mamma", "maxim"], ["lolly", "allow"], ["geese", "eerie"], ["crane", "crane"], ["aaaaa", "abase"], ["array", "rarer"]];
const answers = words.WORDLE_ANSWERS;
const accepted = [...words.WORDLE_ACCEPTED_GUESSES, ...answers];
const scorePairs = [...canonical];
for (let i = 0; i < 1500; i++) scorePairs.push([pick(accepted), pick(answers)]);
const hardCases = [];
for (let i = 0; i < 600; i++) {
  const answer = pick(answers);
  const previous = Array.from({ length: 1 + Math.floor(rng() * 4) }, () => pick(accepted));
  const guess = rng() < 0.3 ? answer : pick(accepted);
  hardCases.push({ answer, previous, guess, violation: wordle.getWordleHardModeViolation(guess, previous, answer) });
}
write("wordle.json", {
  answerCount: wordle.WORDLE_ANSWER_COUNT,
  acceptedCount: wordle.WORDLE_ACCEPTED_GUESS_COUNT,
  scores: scorePairs.map(([guess, answer]) => ({ guess, answer, score: wordle.scoreWordleGuess(guess, answer).map((s) => s.state[0]).join("") })),
  keyboards: hardCases.slice(0, 100).map((c) => ({ answer: c.answer, guesses: c.previous, state: Object.entries(wordle.getWordleKeyboardState(c.previous, c.answer)).sort().map(([k, v]) => `${k}${v[0]}`).join("") })),
  hard: hardCases,
  normalize: ["  CrAnE ", "ab-cd!e", "crane2", "ÉCLAT", "a b c d e f"].map((v) => ({ input: v, output: wordle.normalizeWordleGuess(v) })),
  accept: ["crane", "CRANE", "zzzzz", "aahed", "abide"].map((v) => ({ input: v, accepted: wordle.isAcceptedWordleGuess(v) })),
});

// ---------- geodle ----------
const geodlePairs = [];
for (const answer of [COUNTRIES[57]]) for (const guess of COUNTRIES) geodlePairs.push([guess.name, answer.name]);
for (let i = 0; i < 200; i++) geodlePairs.push([pick(COUNTRIES).name, pick(COUNTRIES).name]);
const fmt = (v) => new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: v >= 1000000 ? 1 : 0 }).format(v);
const compactValues = [0, 1, 999, 1000, 1499, 1500, 2500, 999499, 999500, 999999, 1000000, 1049999, 1050000, 9950000, 99950000, 999950000, 1000000000, 9999999999, 12345678901234,
  ...COUNTRIES.flatMap((c) => [c.population, c.areaKm2])];
write("geodle.json", {
  countryCount: geodle.GEODLE_COUNTRY_COUNT,
  clues: geodlePairs.map(([guess, answer]) => ({ guess, answer, clues: geodle.scoreGeodleGuess(guess, answer) })),
  compact: compactValues.map((v) => ({ value: v, text: fmt(v) })),
  find: ["  türkiye ", "Sao Tome and Principe", "são tomé and príncipe", "bosnia", "Côte", "united states", "United-States", "", "zzz"].map((q) => ({ query: q, found: geodle.findCountryByName(q)?.name ?? null, filtered: geodle.filterCountries(q).map((c) => c.code) })),
});

// ---------- durak ----------
const realRandom = Math.random;
function findWithPick(seed, value) {
  Math.random = () => value;
  try { return durak.findDailyPuzzle(seed); } finally { Math.random = realRandom; }
}
const ser = (gs) => {
  const p = durak.gameStateToPuzzle(gs, 0, false, "", "");
  return { playerHand: p.playerHand, cpuHand: p.cpuHand, trumpSuit: p.trumpSuit, table: p.table, discardPile: p.discardPile, phase: p.phase, winner: p.winner ?? null, message: p.message };
};
const seeds = [];
for (let i = 0; i < 45; i++) { const d = new Date(Date.UTC(2026, 8, 1 + i * 9)).toISOString().slice(0, 10); for (const k of [0, 1, 2]) seeds.push(`${d}_${k}`); }
seeds.push("2026-10-01", "2028-02-29_0", "x");
const puzzles = seeds.map((seed) => {
  const first = findWithPick(seed, 0);
  const last = findWithPick(seed, 0.999999);
  return first ? { seed, state: ser(first.initialState), hintFirst: first.hint, hintLast: last.hint } : { seed, state: null };
});
// Random play trajectories through production's own handlers (processCpuTurn once per action).
function legalActions(gs) {
  const actions = [];
  const cardsByRank = (hand) => {
    const m = new Map();
    hand.forEach((c) => { const l = m.get(c.rank) ?? []; l.push(c); m.set(c.rank, l); });
    return [...m.values()];
  };
  if (gs.phase === "player_attack") {
    const max = Math.min(6, gs.cpuHand.length);
    for (const group of cardsByRank(gs.playerHand)) for (let k = 1; k <= Math.min(max, group.length); k++) actions.push({ type: "attack", cards: group.slice(0, k) });
  } else if (gs.phase === "player_throw") {
    actions.push({ type: "pass", cards: [] });
    const ranks = new Set(gs.table.flatMap((e) => [e.attack.rank, e.defense?.rank].filter(Boolean)));
    const max = Math.max(0, durak.getAttackLimitAgainstCpu(gs) - gs.table.length);
    for (const group of cardsByRank(gs.playerHand.filter((c) => ranks.has(c.rank)))) for (let k = 1; k <= Math.min(max, group.length); k++) actions.push({ type: "throw", cards: group.slice(0, k) });
  } else if (gs.phase === "player_defense") {
    actions.push({ type: "pickup", cards: [] });
    const target = gs.table.find((e) => !e.defense);
    if (target) for (const c of gs.playerHand) if (durak.canBeat(c, target.attack, gs.trumpSuit)) actions.push({ type: "defend", cards: [c] });
    for (const c of durak.getLegalSlideCards(gs, "player")) actions.push({ type: "slide", cards: [c] });
  }
  return actions;
}
function apply(gs, a) {
  switch (a.type) {
    case "attack": return durak.processCpuTurn(durak.executePlayerAttack(gs, a.cards));
    case "throw": return durak.processCpuTurn(durak.executePlayerThrow(gs, a.cards));
    case "pass": return durak.processCpuTurn(durak.playerPassThrow(gs));
    case "defend": return durak.processCpuTurn(durak.defendOneCard(gs, a.cards[0]));
    case "pickup": return durak.processCpuTurn(durak.playerPickUp(gs));
    case "slide": return durak.processCpuTurn(durak.executeSlide(gs, a.cards[0]));
  }
}
const trajectories = [];
for (const p of puzzles.filter((p) => p.state).slice(0, 60)) {
  for (let run = 0; run < 4; run++) {
    let gs = durak.puzzleToGameState({ ...p.state, winner: p.state.winner ?? undefined });
    const steps = [];
    for (let i = 0; i < 40 && gs.phase !== "finished"; i++) {
      const actions = legalActions(gs);
      if (!actions.length) break;
      const a = pick(actions);
      gs = apply(gs, a);
      steps.push({ type: a.type, cards: a.cards.map(durak.cardKey), after: ser(gs) });
    }
    trajectories.push({ seed: p.seed, steps });
  }
}
write("durak.json", { puzzles, trajectories });

// ---------- album name order (localeCompare) ----------
const albumNames = ["Full House", "First Break", "On Fire", "Early Bird", "Night Owl", "Speedrunner", "Explorer", "Perfectionist", "Veteran",
  "Sprouting Rock", "Growing Rock", "Flourished Rock", "Blooming Rock", "Royal Rock", "Hellish Rock", "Heavenly Rock", "Cosmic Rock", "Galactic Rock", "Eternal Rock",
  "Meteoric Rock", "Planetary Rock", "Celestial Rock", "Ancient Starstone", "Hell's Diplomat", "Saint", "Rock God", "Demon", "Guardian Angel",
  "Seed Fossil", "Shell Fragment", "Ammonite", "Crystal Cluster", "Complete Specimen", "Ancient Artifact", "Golden Record",
  "First Sprout", "Streak Bloom", "Mushroom Ring", "Cross-Pollinator", "Full Bloom", "Harvest Season", "The Wise Tree"];
write("album.json", { sorted: [...albumNames].sort((a, b) => a.localeCompare(b)) });

rmSync(lib, { recursive: true, force: true });
for (const f of readdirSync(out)) console.log(f, readFileSync(join(out, f)).length);
