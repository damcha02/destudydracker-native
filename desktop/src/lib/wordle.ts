import { WORDLE_ACCEPTED_GUESSES, WORDLE_ANSWERS } from "./wordleWords";

export type WordleLetterState = "correct" | "present" | "absent";

export interface WordleScoredLetter {
  letter: string;
  state: WordleLetterState;
}

const WORD_LENGTH = 5;
const MAX_GUESSES = 6;
const FIRST_PUZZLE_DATE = "2026-01-01";
const acceptedGuesses = new Set<string>([...WORDLE_ACCEPTED_GUESSES, ...WORDLE_ANSWERS]);

function hashString(value: string): number {
  let hash = 2166136261;
  for (let i = 0; i < value.length; i++) {
    hash ^= value.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function daysSinceFirstPuzzle(dateStr: string): number {
  const start = new Date(`${FIRST_PUZZLE_DATE}T00:00:00Z`).getTime();
  const target = new Date(`${dateStr}T00:00:00Z`).getTime();
  if (!Number.isFinite(target)) return 0;
  return Math.max(0, Math.floor((target - start) / 86400000));
}

export function makeWordleSeedSalt() {
  if (globalThis.crypto?.randomUUID) return globalThis.crypto.randomUUID();
  return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export function normalizeWordleGuess(value: string) {
  return value.trim().toLowerCase().replace(/[^a-z]/g, "").slice(0, WORD_LENGTH);
}

export function isAcceptedWordleGuess(value: string) {
  return acceptedGuesses.has(value.toLowerCase());
}

export function getWordleAnswerForDate(dateStr: string, seedSalt: string) {
  const order = WORDLE_ANSWERS.map((answer, index) => ({ answer, sort: hashString(`${seedSalt}:${answer}:${index}`) }))
    .sort((a, b) => a.sort - b.sort)
    .map((item) => item.answer);
  return order[daysSinceFirstPuzzle(dateStr) % order.length];
}

export function getWordlePuzzleId(dateStr: string, seedSalt: string) {
  return `${dateStr}:${hashString(seedSalt).toString(36)}`;
}

export function scoreWordleGuess(guess: string, answer: string): WordleScoredLetter[] {
  const normalizedGuess = normalizeWordleGuess(guess);
  const normalizedAnswer = normalizeWordleGuess(answer);
  const result: WordleScoredLetter[] = normalizedGuess.split("").map((letter) => ({ letter, state: "absent" }));
  const remaining = new Map<string, number>();

  for (let i = 0; i < WORD_LENGTH; i++) {
    if (normalizedGuess[i] === normalizedAnswer[i]) {
      result[i].state = "correct";
    } else {
      remaining.set(normalizedAnswer[i], (remaining.get(normalizedAnswer[i]) ?? 0) + 1);
    }
  }

  for (let i = 0; i < WORD_LENGTH; i++) {
    if (result[i].state === "correct") continue;
    const count = remaining.get(normalizedGuess[i]) ?? 0;
    if (count > 0) {
      result[i].state = "present";
      remaining.set(normalizedGuess[i], count - 1);
    }
  }

  return result;
}

export function getWordleKeyboardState(guesses: string[], answer: string) {
  const strength: Record<WordleLetterState, number> = { absent: 1, present: 2, correct: 3 };
  const states: Record<string, WordleLetterState> = {};
  guesses.forEach((guess) => {
    scoreWordleGuess(guess, answer).forEach(({ letter, state }) => {
      if (!states[letter] || strength[state] > strength[states[letter]]) states[letter] = state;
    });
  });
  return states;
}

export function getWordleHardModeViolation(guess: string, previousGuesses: string[], answer: string): string | null {
  const normalizedGuess = normalizeWordleGuess(guess);
  const requiredPositions = new Map<number, string>();
  const requiredCounts = new Map<string, number>();
  const eliminatedLetters = new Set<string>();
  const revealedLetters = new Set<string>();

  previousGuesses.forEach((previousGuess) => {
    const scored = scoreWordleGuess(previousGuess, answer);
    const revealedCounts = new Map<string, number>();

    scored.forEach(({ letter, state }, index) => {
      if (state === "correct") requiredPositions.set(index, letter);
      if (state === "correct" || state === "present") {
        revealedLetters.add(letter);
        revealedCounts.set(letter, (revealedCounts.get(letter) ?? 0) + 1);
      } else {
        eliminatedLetters.add(letter);
      }
    });

    revealedCounts.forEach((count, letter) => {
      requiredCounts.set(letter, Math.max(requiredCounts.get(letter) ?? 0, count));
    });
  });

  for (const [index, letter] of requiredPositions) {
    if (normalizedGuess[index] !== letter) return `${letter.toUpperCase()} must be in position ${index + 1}.`;
  }

  for (const [letter, count] of requiredCounts) {
    const actual = normalizedGuess.split("").filter((item) => item === letter).length;
    if (actual < count) return count === 1 ? `Guess must contain ${letter.toUpperCase()}.` : `Guess must contain ${count} ${letter.toUpperCase()}s.`;
  }

  for (const letter of eliminatedLetters) {
    if (!revealedLetters.has(letter) && normalizedGuess.includes(letter)) return `${letter.toUpperCase()} has been eliminated.`;
  }

  return null;
}

export const WORDLE_WORD_LENGTH = WORD_LENGTH;
export const WORDLE_MAX_GUESSES = MAX_GUESSES;
export const WORDLE_ANSWER_COUNT = WORDLE_ANSWERS.length;
export const WORDLE_ACCEPTED_GUESS_COUNT = acceptedGuesses.size;
