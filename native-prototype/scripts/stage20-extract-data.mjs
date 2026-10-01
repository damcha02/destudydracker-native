// Stage 20: extracts production's Break Room game data from desktop/ (read-only) into compact,
// diff-friendly text files that study-tracker-core embeds with include_str!.
//
//   node scripts/stage20-extract-data.mjs [--desktop ../desktop]
//
// Outputs (crates/study-tracker-core/data/break_room/):
//   wordle-answers.txt   WORDLE_ANSWERS, one word per line, production order (the daily order is a
//                        hash sort over this list, so the order itself is part of the semantics)
//   wordle-guesses.txt   WORDLE_ACCEPTED_GUESSES, one per line, production order
//   countries.tsv        COUNTRIES (code, iso2, name, continent, region, population, landlocked,
//                        areaKm2, religion, government), production order
//   quotes.tsv           App.tsx `breakQuotes` (text, author), production order
//   stretches.txt        App.tsx `stretchIdeas`, production order
//
// Nothing is transformed beyond the container format; the script fails if any record does not
// round-trip.
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const args = Object.fromEntries(process.argv.slice(2).reduce((acc, a, i, all) => (a.startsWith("--") ? [...acc, [a.slice(2), all[i + 1]]] : acc), []));
const desktop = args.desktop ?? join(here, "..", "..", "desktop");
const out = join(here, "..", "crates", "study-tracker-core", "data", "break_room");
mkdirSync(out, { recursive: true });

function arrayLiteral(source, name) {
  const start = source.indexOf(`export const ${name}`);
  if (start < 0) throw new Error(`${name} not found`);
  const open = source.indexOf("[", start);
  let depth = 0;
  for (let i = open; i < source.length; i++) {
    if (source[i] === "[") depth++;
    else if (source[i] === "]" && --depth === 0) return JSON.parse(source.slice(open, i + 1));
  }
  throw new Error(`${name} is unterminated`);
}

const words = readFileSync(join(desktop, "src/lib/wordleWords.ts"), "utf8");
const answers = arrayLiteral(words, "WORDLE_ANSWERS");
const guesses = arrayLiteral(words, "WORDLE_ACCEPTED_GUESSES");
for (const w of [...answers, ...guesses]) if (!/^[a-z]{5}$/.test(w)) throw new Error(`unexpected word ${w}`);
writeFileSync(join(out, "wordle-answers.txt"), answers.join("\n") + "\n");
writeFileSync(join(out, "wordle-guesses.txt"), guesses.join("\n") + "\n");

const countries = arrayLiteral(readFileSync(join(desktop, "src/lib/countries.ts"), "utf8"), "COUNTRIES");
const fields = ["code", "iso2", "name", "continent", "region", "population", "landlocked", "areaKm2", "religion", "government"];
const rows = countries.map((c) => {
  for (const f of fields) if (!(f in c)) throw new Error(`${c.name} lacks ${f}`);
  if (Object.keys(c).length !== fields.length) throw new Error(`${c.name} has extra fields`);
  const cells = fields.map((f) => String(c[f]));
  if (cells.some((cell) => cell.includes("\t") || cell.includes("\n"))) throw new Error(`${c.name} has a tab/newline`);
  return cells.join("\t");
});
writeFileSync(join(out, "countries.tsv"), `# ${fields.join("\t")}\n` + rows.join("\n") + "\n");

// App.tsx literals: evaluated as plain JS array literals (static data from this repository).
const app = readFileSync(join(desktop, "src/App.tsx"), "utf8");
function appLiteral(name) {
  const start = app.indexOf(`const ${name} = [`);
  if (start < 0) throw new Error(`${name} not found`);
  const open = app.indexOf("[", start);
  const close = app.indexOf("\n];", open);
  return new Function(`return ${app.slice(open, close + 2)}`)();
}
const quotes = appLiteral("breakQuotes");
for (const q of quotes) if (typeof q.text !== "string" || typeof q.author !== "string" || /[\t\n]/.test(q.text + q.author)) throw new Error("bad quote");
writeFileSync(join(out, "quotes.tsv"), quotes.map((q) => `${q.text}\t${q.author}`).join("\n") + "\n");
const stretches = appLiteral("stretchIdeas");
writeFileSync(join(out, "stretches.txt"), stretches.join("\n") + "\n");

console.log(`quotes ${quotes.length}, stretches ${stretches.length}`);
console.log(`answers ${answers.length}, accepted guesses ${guesses.length}, countries ${countries.length} -> ${out}`);
