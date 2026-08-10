import { COUNTRIES } from "./countries";
import type { CountryFact } from "./countries";

const flagUrls = import.meta.glob("../assets/flags/*.svg", { query: "?url", import: "default", eager: true }) as Record<string, string>;
const MAX_GUESSES = 7;
const FIRST_PUZZLE_DATE = "2026-01-01";
const FLAG_WIDTH = 240;
const FLAG_HEIGHT = 180;
const COLOR_TOLERANCE = 52;

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

function flagPath(country: CountryFact) {
  return `../assets/flags/${country.iso2.toLowerCase()}.svg`;
}

function colorDistance(a: [number, number, number], b: [number, number, number]) {
  const dr = a[0] - b[0];
  const dg = a[1] - b[1];
  const db = a[2] - b[2];
  return Math.sqrt(dr * dr + dg * dg + db * db);
}

async function imageFromSrc(src: string): Promise<HTMLImageElement> {
  const image = new Image();
  image.decoding = "async";
  image.src = src;
  await image.decode();
  return image;
}

async function flagUrlToImageData(src: string) {
  const canvas = document.createElement("canvas");
  canvas.width = FLAG_WIDTH;
  canvas.height = FLAG_HEIGHT;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Could not render flag.");
  const image = await imageFromSrc(src);
  context.clearRect(0, 0, FLAG_WIDTH, FLAG_HEIGHT);
  context.drawImage(image, 0, 0, FLAG_WIDTH, FLAG_HEIGHT);
  return { canvas, context, data: context.getImageData(0, 0, FLAG_WIDTH, FLAG_HEIGHT) };
}

function pixelsMatchAt(target: ImageData, guess: ImageData, index: number) {
  if (target.data[index + 3] < 128 || guess.data[index + 3] < 128) return false;
  const targetColor: [number, number, number] = [target.data[index], target.data[index + 1], target.data[index + 2]];
  const guessColor: [number, number, number] = [guess.data[index], guess.data[index + 1], guess.data[index + 2]];
  return colorDistance(targetColor, guessColor) <= COLOR_TOLERANCE;
}

export function makeFlaggleSeedSalt() {
  if (globalThis.crypto?.randomUUID) return globalThis.crypto.randomUUID();
  return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export function getFlaggleAnswerForDate(dateStr: string, seedSalt: string): string {
  const order = COUNTRIES.map((country, index) => ({ country, sort: hashString(`${seedSalt}:${country.code}:${index}`) }))
    .sort((a, b) => a.sort - b.sort)
    .map((item) => item.country.name);
  return order[daysSinceFirstPuzzle(dateStr) % order.length];
}

export function getFlagglePuzzleId(dateStr: string, seedSalt: string) {
  return `${dateStr}:${hashString(seedSalt).toString(36)}`;
}

export function findFlaggleCountry(value: string): CountryFact | null {
  const normalized = normalizeCountryName(value);
  return COUNTRIES.find((country) => normalizeCountryName(country.name) === normalized) ?? null;
}

export function filterFlaggleCountries(query: string): readonly CountryFact[] {
  const normalized = normalizeCountryName(query);
  if (!normalized) return COUNTRIES;
  return COUNTRIES.filter((country) => normalizeCountryName(country.name).includes(normalized));
}

export function getFlagImageSrc(countryName: string) {
  const country = findFlaggleCountry(countryName);
  if (!country) return "";
  return flagUrls[flagPath(country)] ?? "";
}

export async function maskFlagByTargetColors(guessName: string, answerName: string) {
  const guess = findFlaggleCountry(guessName);
  const answer = findFlaggleCountry(answerName);
  if (!guess || !answer) throw new Error("Country not found.");
  const guessUrl = flagUrls[flagPath(guess)];
  const answerUrl = flagUrls[flagPath(answer)];
  if (!guessUrl || !answerUrl) throw new Error("Flag asset not found.");

  const [guessRender, answerRender] = await Promise.all([flagUrlToImageData(guessUrl), flagUrlToImageData(answerUrl)]);
  const output = new ImageData(new Uint8ClampedArray(answerRender.data.data), FLAG_WIDTH, FLAG_HEIGHT);
  let visible = 0;
  let matched = 0;

  for (let i = 0; i < output.data.length; i += 4) {
    if (answerRender.data.data[i + 3] < 128) continue;
    visible++;
    if (pixelsMatchAt(answerRender.data, guessRender.data, i)) {
      matched++;
      continue;
    }
    output.data[i] = 25;
    output.data[i + 1] = 25;
    output.data[i + 2] = 25;
  }

  answerRender.context.putImageData(output, 0, 0);
  return {
    maskedFlagDataUrl: answerRender.canvas.toDataURL("image/png"),
    similarity: visible > 0 ? Math.round((matched / visible) * 1000) / 10 : 0,
  };
}

export async function revealTargetFlagByGuesses(answerName: string, guessNames: string[], revealFull = false) {
  const answer = findFlaggleCountry(answerName);
  if (!answer) throw new Error("Country not found.");
  const answerUrl = flagUrls[flagPath(answer)];
  if (!answerUrl) throw new Error("Flag asset not found.");
  const answerRender = await flagUrlToImageData(answerUrl);
  if (revealFull || guessNames.includes(answerName)) return answerRender.canvas.toDataURL("image/png");

  const guessUrls = guessNames.flatMap((guessName) => {
    const country = findFlaggleCountry(guessName);
    if (!country) return [];
    const url = flagUrls[flagPath(country)];
    return url ? [url] : [];
  });
  if (!guessUrls.length) return "";
  const guessImages = await Promise.all(guessUrls.map(flagUrlToImageData));
  const output = new ImageData(new Uint8ClampedArray(answerRender.data.data), FLAG_WIDTH, FLAG_HEIGHT);

  for (let i = 0; i < output.data.length; i += 4) {
    if (output.data[i + 3] < 128) continue;
    if (guessImages.some((guessImage) => pixelsMatchAt(answerRender.data, guessImage.data, i))) continue;
    output.data[i] = 25;
    output.data[i + 1] = 25;
    output.data[i + 2] = 25;
  }

  answerRender.context.putImageData(output, 0, 0);
  return answerRender.canvas.toDataURL("image/png");
}

export const FLAGGLE_MAX_GUESSES = MAX_GUESSES;
export const FLAGGLE_COUNTRY_COUNT = COUNTRIES.length;
