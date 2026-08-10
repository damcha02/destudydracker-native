import { COUNTRIES } from "./countries";
import type { CountryFact } from "./countries";

export type GeodleClueState = "match" | "miss" | "higher" | "lower" | "close";

export interface GeodleClue {
  label: string;
  value: string;
  state: GeodleClueState;
  hint: string;
}

const MAX_GUESSES = 7;
const FIRST_PUZZLE_DATE = "2026-01-01";

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

function normalizeCountryName(value: string) {
  return value.trim().toLowerCase().replace(/[^a-z0-9]+/g, " ").replace(/\s+/g, " ").trim();
}

function numericClue(guess: number, answer: number): GeodleClueState {
  if (guess === answer) return "match";
  const ratio = Math.abs(guess - answer) / Math.max(answer, 1);
  if (ratio <= 0.2) return "close";
  return guess < answer ? "higher" : "lower";
}

function formatCompact(value: number) {
  return new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: value >= 1000000 ? 1 : 0 }).format(value);
}

function numericHint(label: "population" | "area", state: GeodleClueState, guess: CountryFact) {
  const noun = label === "population" ? "people" : "area";
  if (state === "match") return `The answer has the same ${noun} as ${guess.name}.`;
  if (state === "close") return `The answer is close to ${guess.name}'s ${noun}.`;
  if (state === "higher") return label === "population" ? `The answer has more people than ${guess.name}.` : `The answer is larger than ${guess.name}.`;
  if (state === "lower") return label === "population" ? `The answer has fewer people than ${guess.name}.` : `The answer is smaller than ${guess.name}.`;
  return "";
}

export function makeGeodleSeedSalt() {
  if (globalThis.crypto?.randomUUID) return globalThis.crypto.randomUUID();
  return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export function getGeodleAnswerForDate(dateStr: string, seedSalt: string): string {
  const order = COUNTRIES.map((country, index) => ({ country, sort: hashString(`${seedSalt}:${country.code}:${index}`) }))
    .sort((a, b) => a.sort - b.sort)
    .map((item) => item.country.name);
  return order[daysSinceFirstPuzzle(dateStr) % order.length];
}

export function getGeodlePuzzleId(dateStr: string, seedSalt: string) {
  return `${dateStr}:${hashString(seedSalt).toString(36)}`;
}

export function findCountryByName(value: string): CountryFact | null {
  const normalized = normalizeCountryName(value);
  return COUNTRIES.find((country) => normalizeCountryName(country.name) === normalized) ?? null;
}

export function filterCountries(query: string): readonly CountryFact[] {
  const normalized = normalizeCountryName(query);
  if (!normalized) return COUNTRIES;
  return COUNTRIES.filter((country) => normalizeCountryName(country.name).includes(normalized));
}

export function scoreGeodleGuess(guessName: string, answerName: string): GeodleClue[] {
  const guess = findCountryByName(guessName);
  const answer = findCountryByName(answerName);
  if (!guess || !answer) return [];
  const populationState = numericClue(guess.population, answer.population);
  const areaState = numericClue(guess.areaKm2, answer.areaKm2);
  const guessCoast = guess.landlocked ? "is landlocked" : "touches water";
  const answerCoast = answer.landlocked ? "is landlocked" : "touches water";

  return [
    {
      label: "Continent",
      value: guess.continent,
      state: guess.continent === answer.continent ? "match" : "miss",
      hint: guess.continent === answer.continent ? `Both countries are in ${guess.continent}.` : `The answer is not in ${guess.continent}.`,
    },
    { label: "Population", value: formatCompact(guess.population), state: populationState, hint: numericHint("population", populationState, guess) },
    {
      label: "Landlocked",
      value: guess.landlocked ? "Yes" : "No",
      state: guess.landlocked === answer.landlocked ? "match" : "miss",
      hint: guess.landlocked === answer.landlocked ? `Both countries ${guessCoast}.` : `${guess.name} ${guessCoast}, but the answer ${answerCoast}.`,
    },
    {
      label: "Religion",
      value: guess.religion,
      state: guess.religion === answer.religion ? "match" : "miss",
      hint: guess.religion === answer.religion ? `Both countries have ${guess.religion} as the dominant religion.` : `The answer does not have ${guess.religion} as the dominant religion.`,
    },
    { label: "Area", value: `${formatCompact(guess.areaKm2)} km²`, state: areaState, hint: numericHint("area", areaState, guess) },
    {
      label: "Gov.",
      value: guess.government,
      state: guess.government === answer.government ? "match" : "miss",
      hint: guess.government === answer.government ? `Both countries are ${guess.government}.` : `The answer is not ${guess.government}.`,
    },
  ];
}

export const GEODLE_MAX_GUESSES = MAX_GUESSES;
export const GEODLE_COUNTRY_COUNT = COUNTRIES.length;
