import { COUNTRY_BORDERS } from "./countryBorders";
import { COUNTRIES } from "./countries";
import type { CountryFact } from "./countries";

const FIRST_PUZZLE_DATE = "2026-01-01";
const MAX_GUESSES = 7;

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

function countryByCode(code: string) {
  return COUNTRIES.find((country) => country.code === code) ?? null;
}

function codeToCountryName(code: string) {
  return countryByCode(code)?.name ?? code;
}

function countryCode(name: string) {
  return findTravleCountry(name)?.code ?? "";
}

function playableCountryCodes() {
  return COUNTRIES.map((country) => country.code).filter((code) => (COUNTRY_BORDERS[code] ?? []).length > 0).sort();
}

export interface TravleDailyPuzzle {
  start: string;
  target: string;
  shortestPath: string[];
}

export type TravleGuessState = "route" | "possible" | "miss";

export function makeTravleSeedSalt() {
  if (globalThis.crypto?.randomUUID) return globalThis.crypto.randomUUID();
  return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export function getTravlePuzzleId(dateStr: string, seedSalt: string) {
  return `${dateStr}:${hashString(seedSalt).toString(36)}`;
}

export function findTravleCountry(value: string): CountryFact | null {
  const normalized = normalizeCountryName(value);
  return COUNTRIES.find((country) => normalizeCountryName(country.name) === normalized) ?? null;
}

export function filterTravleCountries(query: string): readonly CountryFact[] {
  const normalized = normalizeCountryName(query);
  const countries = COUNTRIES.filter((country) => (COUNTRY_BORDERS[country.code] ?? []).length > 0);
  if (!normalized) return countries;
  return countries.filter((country) => normalizeCountryName(country.name).includes(normalized));
}

export function getTravleNeighbors(countryName: string) {
  const code = countryCode(countryName);
  if (!code) return [];
  return (COUNTRY_BORDERS[code] ?? []).map(codeToCountryName).sort();
}

export function areTravleNeighbors(countryA: string, countryB: string) {
  const codeA = countryCode(countryA);
  const codeB = countryCode(countryB);
  if (!codeA || !codeB) return false;
  return (COUNTRY_BORDERS[codeA] ?? []).includes(codeB);
}

export function getTravleShortestPath(startName: string, targetName: string) {
  const start = countryCode(startName);
  const target = countryCode(targetName);
  if (!start || !target) return [];
  const queue: string[][] = [[start]];
  const seen = new Set([start]);

  while (queue.length) {
    const path = queue.shift()!;
    const current = path[path.length - 1];
    if (current === target) return path.map(codeToCountryName);
    for (const neighbor of COUNTRY_BORDERS[current] ?? []) {
      if (seen.has(neighbor)) continue;
      seen.add(neighbor);
      queue.push([...path, neighbor]);
    }
  }

  return [];
}

function shortestPathWithAllowedCodes(startCode: string, targetCode: string, allowedCodes: Set<string>) {
  const queue: string[][] = [[startCode]];
  const seen = new Set([startCode]);

  while (queue.length) {
    const path = queue.shift()!;
    const current = path[path.length - 1];
    if (current === targetCode) return path;
    for (const neighbor of COUNTRY_BORDERS[current] ?? []) {
      if (!allowedCodes.has(neighbor) || seen.has(neighbor)) continue;
      seen.add(neighbor);
      queue.push([...path, neighbor]);
    }
  }

  return [];
}

function distanceMapFrom(startCode: string) {
  const distances = new Map<string, number>([[startCode, 0]]);
  const queue = [startCode];

  while (queue.length) {
    const current = queue.shift()!;
    const currentDistance = distances.get(current) ?? 0;
    for (const neighbor of COUNTRY_BORDERS[current] ?? []) {
      if (distances.has(neighbor)) continue;
      distances.set(neighbor, currentDistance + 1);
      queue.push(neighbor);
    }
  }

  return distances;
}

export function getTravleSolvedPath(startName: string, targetName: string, guesses: readonly string[]) {
  const start = countryCode(startName);
  const target = countryCode(targetName);
  if (!start || !target) return [];
  const allowedCodes = new Set([start, target, ...guesses.flatMap((guess) => {
    const code = countryCode(guess);
    return code ? [code] : [];
  })]);
  return shortestPathWithAllowedCodes(start, target, allowedCodes).map(codeToCountryName);
}

export function isTravleRouteSolved(startName: string, targetName: string, guesses: readonly string[]) {
  return getTravleSolvedPath(startName, targetName, guesses).length > 0;
}

export function getTravleDisplayPath(startName: string, targetName: string, guesses: readonly string[]) {
  const solvedPath = getTravleSolvedPath(startName, targetName, guesses);
  if (solvedPath.length) return solvedPath;

  const start = countryCode(startName);
  const target = countryCode(targetName);
  if (!start || !target) return [];
  const guessedCodes = new Set(guesses.flatMap((guess) => {
    const code = countryCode(guess);
    return code ? [code] : [];
  }));
  const allowedCodes = new Set([start, ...guessedCodes]);
  const targetDistances = distanceMapFrom(target);
  const queue: string[][] = [[start]];
  let bestPath: string[] = [start];

  while (queue.length) {
    const path = queue.shift()!;
    const current = path[path.length - 1];
    const bestDistance = targetDistances.get(bestPath[bestPath.length - 1]) ?? Infinity;
    const currentDistance = targetDistances.get(current) ?? Infinity;
    if (path.length > bestPath.length || (path.length === bestPath.length && currentDistance < bestDistance)) {
      bestPath = path;
    }
    for (const neighbor of COUNTRY_BORDERS[current] ?? []) {
      if (!allowedCodes.has(neighbor) || path.includes(neighbor)) continue;
      queue.push([...path, neighbor]);
    }
  }

  return bestPath.map(codeToCountryName);
}

export function getTravleGuessStates(startName: string, targetName: string, guesses: readonly string[]): Record<string, TravleGuessState> {
  const start = countryCode(startName);
  const target = countryCode(targetName);
  if (!start || !target) return {};

  const guessCodes = guesses.flatMap((guess) => {
    const code = countryCode(guess);
    return code ? [code] : [];
  });
  const solvedPath = getTravleSolvedPath(startName, targetName, guesses);
  const solvedCodes = new Set(solvedPath.flatMap((name) => {
    const code = countryCode(name);
    return code ? [code] : [];
  }));
  const displayPathCodes = new Set(getTravleDisplayPath(startName, targetName, guesses).flatMap((name) => {
    const code = countryCode(name);
    return code ? [code] : [];
  }));
  const answerPathCodes = new Set(getTravleShortestPath(startName, targetName).flatMap((name) => {
    const code = countryCode(name);
    return code ? [code] : [];
  }));
  const shortestLength = answerPathCodes.size;
  const startDistances = distanceMapFrom(start);
  const targetDistances = distanceMapFrom(target);
  const states: Record<string, TravleGuessState> = {};

  for (const code of guessCodes) {
    if (solvedCodes.has(code) || displayPathCodes.has(code) || answerPathCodes.has(code)) {
      states[codeToCountryName(code)] = "route";
      continue;
    }

    const startDistance = startDistances.get(code) ?? Infinity;
    const targetDistance = targetDistances.get(code) ?? Infinity;
    const canFitReasonableRoute = Number.isFinite(startDistance) && Number.isFinite(targetDistance) && startDistance + targetDistance <= shortestLength + 2;
    states[codeToCountryName(code)] = canFitReasonableRoute ? "possible" : "miss";
  }

  return states;
}

function buildDailyPairs() {
  const codes = playableCountryCodes();
  const pairs: TravleDailyPuzzle[] = [];
  for (const start of codes) {
    for (const target of codes) {
      if (start === target) continue;
      const path = getTravleShortestPath(codeToCountryName(start), codeToCountryName(target));
      const guessesNeeded = path.length - 1;
      if (guessesNeeded >= 3 && guessesNeeded <= 5) {
        pairs.push({ start: codeToCountryName(start), target: codeToCountryName(target), shortestPath: path });
      }
    }
  }
  return pairs;
}

let dailyPairsCache: TravleDailyPuzzle[] | null = null;

function dailyPairs() {
  dailyPairsCache ??= buildDailyPairs();
  return dailyPairsCache;
}

export function getTravlePuzzleForDate(dateStr: string, seedSalt: string): TravleDailyPuzzle {
  const pairs = dailyPairs();
  const order = pairs.map((pair, index) => ({ pair, sort: hashString(`${seedSalt}:${pair.start}:${pair.target}:${index}`) }))
    .sort((a, b) => a.sort - b.sort)
    .map((item) => item.pair);
  return order[daysSinceFirstPuzzle(dateStr) % order.length];
}

export const TRAVLE_MAX_GUESSES = MAX_GUESSES;
export const TRAVLE_COUNTRY_COUNT = playableCountryCodes().length;
