// Stage 21: golden fixtures for Travle, computed by PRODUCTION's own code:
//   - desktop/src/lib/{travle,countries,countryBorders,storage,...}.ts, copied read-only into a temp
//     dir and imported with Node's built-in type stripping;
//   - the Travle handlers and render-time derivations that live inside desktop/src/App.tsx
//     (`initTravlePuzzle`, `submitTravleGuess`, the `travle*` constants and map viewBox, the map
//     `className` expression), cut out of the source TEXT by marker and run verbatim (types
//     stripped by `node:module`'s `stripTypeScriptTypes`) against small React-state shims.
// Nothing in desktop/ is modified or executed in place.
//
//   node scripts/stage21-goldens.mjs [--desktop ../desktop]
//
// Writes crates/study-tracker-core/tests/fixtures/break_room/travle.json and
// tests/fixtures/travle-view.json (the app-crate map/view goldens). Choices that need randomness use
// a fixed mulberry32, so the fixtures are reproducible.
import { cpSync, mkdtempSync, readFileSync, writeFileSync, rmSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";
import { stripTypeScriptTypes } from "node:module";
import { execFileSync } from "node:child_process";

const here = dirname(fileURLToPath(import.meta.url));
const args = Object.fromEntries(process.argv.slice(2).reduce((acc, a, i, all) => (a.startsWith("--") ? [...acc, [a.slice(2), all[i + 1]]] : acc), []));
const desktop = args.desktop ?? join(here, "..", "..", "desktop");

// ---------------------------------------------------------------- production modules
const lib = mkdtempSync(join(tmpdir(), "st21-prodlib-"));
for (const f of readdirSync(join(desktop, "src/lib")).filter((f) => f.endsWith(".ts") && !f.endsWith(".test.ts"))) {
  cpSync(join(desktop, "src/lib", f), join(lib, f));
  const p = join(lib, f);
  writeFileSync(p, readFileSync(p, "utf8").replace(/from "\.\/([A-Za-z]+)"/g, 'from "./$1.ts"').replace(/from "\.\.\/types"/g, 'from "./__types.ts"')
    // Vite-only module features, unused by anything Travle/storage touch: flag image URLs and the
    // social Worker's env URL.
    .replace(/import\.meta\.glob\([^)]*\)/g, "{}").replace(/import\.meta\.env\.VITE_SOCIAL_API_URL/g, "undefined"));
}
writeFileSync(join(lib, "__types.ts"), "export {};\n");

// `--tz-probe <instant>`: a child process run under TZ=<zone> (Node reads TZ at start) reporting
// production's local "today" and the Travle puzzle a fresh profile gets for it.
if (args["tz-probe"]) {
  const fixed = Date.parse(args["tz-probe"]);
  const R = Date;
  globalThis.Date = class extends R { constructor(...a) { if (a.length === 0) super(fixed); else super(...a); } static now() { return fixed; } };
  const storage = await import(pathToFileURL(join(lib, "storage.ts")).href);
  const travle = await import(pathToFileURL(join(lib, "travle.ts")).href);
  const today = storage.todayIso();
  const p = travle.getTravlePuzzleForDate(today, "fixture-travle-salt");
  process.stdout.write(JSON.stringify({ today, start: p.start, target: p.target }));
  rmSync(lib, { recursive: true, force: true });
  process.exit(0);
}

const imp = (f) => import(pathToFileURL(join(lib, f)).href);
const travle = await imp("travle.ts");
const { COUNTRIES } = await imp("countries.ts");
const { COUNTRY_BORDERS } = await imp("countryBorders.ts");
const { TRAVLE_MAP_COUNTRIES } = await imp("travleMapData.ts");

function mulberry32(seed) {
  return function () {
    seed |= 0; seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const rng = mulberry32(21);
const pick = (list) => list[Math.floor(rng() * list.length)];
const shuffle = (list) => { const a = [...list]; for (let i = a.length - 1; i > 0; i--) { const j = Math.floor(rng() * (i + 1)); [a[i], a[j]] = [a[j], a[i]]; } return a; };
const codeOf = new Map(COUNTRIES.map((c) => [c.name, c.code]));
const codes = (names) => names.map((n) => codeOf.get(n) ?? `?${n}`).join(">");
const names = COUNTRIES.map((c) => c.name);
const playable = travle.filterTravleCountries("").map((c) => c.name);
const isolated = names.filter((n) => !playable.includes(n));

// ---------------------------------------------------------------- App.tsx excerpts
const app = readFileSync(join(desktop, "src/App.tsx"), "utf8");
function cut(start, end) {
  const i = app.indexOf(start);
  if (i < 0) throw new Error(`marker not found: ${start}`);
  const j = app.indexOf(end, i);
  if (j < 0) throw new Error(`end marker not found: ${end}`);
  return stripTypeScriptTypes(app.slice(i, j + end.length));
}
const submitSrc = cut("function submitTravleGuess() {", "\n    setTravleDropdownOpen(false);\n  }");
const initSrc = cut("const initTravlePuzzle = useEffectEvent(() => {", "\n    setTravleMapZoom(1);\n  });");
const deriveSrc = cut("  const travleOptions = filterTravleCountries(travleDraft).slice(0, 80);", "}, [travleMapCountryByCode, travleMapZoom, travleRouteCodes, travleTargetCode]);");
const classSrc = cut("const isStart = country.code === travleRouteCodes[0];", '].filter(Boolean).join(" ");');
const travleScope = { ...travle, TRAVLE_MAP_COUNTRIES };

/** Production's Travle handlers against a plain state object (`setState` applies at once). */
function makeSession(puzzle, today, newSalt = "fresh-salt") {
  const ui = { state: { travlePuzzle: puzzle }, draft: "", message: "", dropdown: false, zoom: 1 };
  const shims = {
    setState: (f) => { ui.state = typeof f === "function" ? f(ui.state) : f; },
    setTravleMessage: (m) => { ui.message = m; },
    setTravleDraft: (d) => { ui.draft = d; },
    setTravleDropdownOpen: (o) => { ui.dropdown = typeof o === "function" ? o(ui.dropdown) : o; },
    setTravleMapZoom: (z) => { ui.zoom = typeof z === "function" ? z(ui.zoom) : z; },
    useEffectEvent: (f) => f,
    localIsoDate: () => today,
    makeTravleSeedSalt: () => newSalt,
  };
  const run = (src, call) => {
    const scope = { ...travleScope, ...shims, state: ui.state, travleDraft: ui.draft };
    // eslint-disable-next-line no-new-func
    new Function(...Object.keys(scope), `${src}\n${call}`)(...Object.values(scope));
  };
  return {
    ui,
    init: () => run(initSrc, "initTravlePuzzle();"),
    submit: (draft) => { ui.draft = draft; run(submitSrc, "submitTravleGuess();"); },
  };
}

/** The render-time derivations (`travle*` constants, viewBox, map classes) for a state. */
function derive(puzzle, zoom = 1, draft = "") {
  const scope = { ...travleScope, state: { travlePuzzle: puzzle }, travleDraft: draft, travleMapZoom: zoom, useMemo: (f) => f() };
  const body = `${deriveSrc}
    const classes = {};
    for (const country of TRAVLE_MAP_COUNTRIES) {
      ${classSrc}
      if (className !== "travle-map-country") classes[country.code] = className.replace("travle-map-country ", "");
    }
    return { route: travleRoute, current: travleCurrent, left: travleGuessesLeft, shortest: travleShortestPath, solved: travleSolvedPath,
      display: travleDisplayPath, states: travleGuessStates, viewBox: travleMapViewBox, points: travleMapRoutePoints, classes,
      options: travleOptions.map((c) => c.name) };`;
  // eslint-disable-next-line no-new-func
  return new Function(...Object.keys(scope), body)(...Object.values(scope));
}

// ---------------------------------------------------------------- daily puzzles
const salts = ["salt-a", "3f2b8c1e-7d4a-4c55-9a0e-1b2c3d4e5f60", "x", "fixture-travle-salt", ""];
const dates = [];
for (let i = 0; i < 120; i++) dates.push(new Date(Date.UTC(2026, 8, 1 + i)).toISOString().slice(0, 10)); // 120 consecutive days
for (let i = 0; i < 10; i++) dates.push(new Date(Date.UTC(2026, 11, 26 + i)).toISOString().slice(0, 10)); // year boundary
dates.push("2028-02-27", "2028-02-28", "2028-02-29", "2028-03-01"); // leap day
dates.push("2026-03-08", "2026-03-29", "2026-11-01", "2027-03-28", "2027-10-31"); // US/EU DST transition days
dates.push("2025-12-31", "2026-01-01", "", "garbage", "2026-02-30", "2026-13-01", "2031-07-15");
const daily = salts.flatMap((salt) => [...new Set(dates)].map((date) => {
  const p = travle.getTravlePuzzleForDate(date, salt);
  return { salt, date, id: travle.getTravlePuzzleId(date, salt), start: p.start, target: p.target, path: codes(p.shortestPath) };
}));

// timezone: the same instant is a different local day around the world; production feeds the local day
const tzProbes = [];
for (const [tz, instant] of [["Europe/Zurich", "2026-10-02T23:30:00Z"], ["America/Los_Angeles", "2026-10-02T23:30:00Z"], ["Asia/Tokyo", "2026-10-02T15:30:00Z"], ["Pacific/Auckland", "2026-12-31T11:30:00Z"], ["America/Sao_Paulo", "2027-01-01T01:30:00Z"]]) {
  const out = execFileSync(process.execPath, ["--no-warnings", fileURLToPath(import.meta.url), "--desktop", desktop, "--tz-probe", instant], { env: { ...process.env, TZ: tz } }).toString();
  tzProbes.push({ tz, instant, ...JSON.parse(out) });
}

// ---------------------------------------------------------------- names, normalization, filter
const findInputs = [...names, ...names.map((n) => `  ${n.toUpperCase()}  `), ...names.map((n) => n.replace(/ /g, "-")),
  "turkey", "Turkiye", "Türkiye", "TÜRKIYE", "t rkiye", "Cote d'Ivoire", "Côte d'Ivoire", "Ivory Coast", "USA", "United States", "united   states!",
  "UK", "Czechia", "Czech Republic", "DR Congo", "Congo", "Republic of the Congo", "Democratic Republic of the Congo", "Bosnia", "Bosnia & Herzegovina",
  "bosnia and herzegovina", "N. Korea", "North Korea", "korea", "South Sudan", "Sudan", "Guinea", "Guinea-Bissau", "Equatorial Guinea", "Papua New Guinea",
  "São Tomé and Príncipe", "Sao Tome and Principe", "s o tom and pr ncipe", "Timor-Leste", "East Timor", "Eswatini", "Swaziland", "Myanmar", "Burma",
  "Vatican City", "Holy See", "Palestine", "Kosovo", "Taiwan", "Western Sahara", "Greenland", "Hong Kong", "", "   ", "-", "123", "France1", "Fr ance",
  "franc", "\u{feff}France", "\tFrance\n", "FRANCE", "france.", "'France'", "Saint Kitts and Nevis", "St Kitts", "Trinidad & Tobago"];
const find = [...new Set(findInputs)].map((input) => ({ input, name: travle.findTravleCountry(input)?.name ?? null }));
const filterQueries = ["", "a", "an", "united", "guinea", "GUI", "ü", "t r", "rep", "island", "x", "zz", "  sw  ", "-", "côte", "congo", "s", "korea", "Türk"];
const filter = filterQueries.map((query) => ({ query, names: travle.filterTravleCountries(query).map((c) => c.code).join(" ") }));

// ---------------------------------------------------------------- adjacency and shortest paths
const neighbours = names.map((name) => ({ name, neighbours: travle.getTravleNeighbors(name) }));
const neighbourPairs = [];
for (let i = 0; i < 600; i++) { const a = pick(names), b = rng() < 0.5 ? pick(names) : pick(travle.getTravleNeighbors(a).length ? travle.getTravleNeighbors(a) : names); neighbourPairs.push([a, b, travle.areTravleNeighbors(a, b)]); }
neighbourPairs.push(["Sri Lanka", "India", travle.areTravleNeighbors("Sri Lanka", "India")], ["India", "Sri Lanka", travle.areTravleNeighbors("India", "Sri Lanka")], ["Nowhere", "France", false]);
const paths = [];
for (let i = 0; i < 4000; i++) { const a = rng() < 0.9 ? pick(playable) : pick(names), b = rng() < 0.9 ? pick(playable) : pick(names); paths.push([codeOf.get(a), codeOf.get(b), codes(travle.getTravleShortestPath(a, b))]); }
for (const [a, b] of [["Sri Lanka", "India"], ["India", "Sri Lanka"], ["Sri Lanka", "China"], ["China", "Sri Lanka"], ["France", "France"], ["Australia", "France"], ["France", "Australia"], ["Portugal", "Viet Nam"], ["Portugal", "Vietnam"], ["Spain", "Mongolia"], ["Morocco", "South Africa"], ["Canada", "Argentina"], ["Panama", "Colombia"], ["Russia", "North Korea"], ["Haiti", "Dominican Republic"], ["United Kingdom", "Ireland"], ["Denmark", "Germany"], ["Egypt", "Israel"]]) {
  paths.push([codeOf.get(a) ?? a, codeOf.get(b) ?? b, codes(travle.getTravleShortestPath(a, b))]);
}

// ---------------------------------------------------------------- games (production handlers)
function guessPlan(start, target) {
  const shortest = travle.getTravleShortestPath(start, target);
  const inner = shortest.slice(1);
  const near = [...new Set(shortest.flatMap((n) => travle.getTravleNeighbors(n)))];
  const kind = Math.floor(rng() * 7);
  const noise = () => (rng() < 0.15 ? pick(["", "atlantis", "turkey", "  ", start, target, pick(isolated), pick(names).toLowerCase(), `${pick(names)}!`]) : pick(rng() < 0.5 ? near : playable));
  switch (kind) {
    case 0: return inner;                                         // perfect route
    case 1: return shuffle(inner);                                // right countries, any order
    case 2: return [...shuffle(near).slice(0, 2), ...inner];      // detours, then the route
    case 3: return Array.from({ length: 9 }, noise);              // mostly wrong: runs out
    case 4: return [inner[0], inner[0], start, ...inner.slice(1)]; // duplicates and the start
    case 5: return [target, ...shuffle(near).slice(0, 8)];        // destination first
    default: return Array.from({ length: 6 }, noise).concat(inner);
  }
}
const games = [];
const gameSeeds = [...daily.filter((d) => d.salt !== "" && d.date.startsWith("2026-1")).slice(0, 120).map((d) => [d.start, d.target]),
  ...Array.from({ length: 80 }, () => [pick(playable), pick(playable)]),
  ["Sri Lanka", "China"], ["China", "Sri Lanka"], ["France", "Australia"], ["Australia", "France"], ["France", "France"]];
for (const [start, target] of gameSeeds) {
  const puzzle = { seedSalt: "game", activeDate: "2026-10-02", puzzleId: travle.getTravlePuzzleId("2026-10-02", "game"), start, target, guesses: [], completed: false, won: false };
  const s = makeSession(puzzle, "2026-10-02");
  s.init(); // today's puzzle with a start and a destination: no reset
  const steps = guessPlan(start, target).filter((d) => typeof d === "string").map((draft) => {
    s.submit(draft);
    const p = s.ui.state.travlePuzzle;
    return { draft, message: s.ui.message, draftAfter: s.ui.draft, dropdown: s.ui.dropdown, guesses: [...p.guesses], completed: p.completed, won: p.won };
  });
  const d = derive(s.ui.state.travlePuzzle);
  games.push({ start, target, steps, final: { shortest: codes(d.shortest), solved: codes(d.solved), display: codes(d.display), states: d.states } });
}

// ---------------------------------------------------------------- route queries on raw guess lists
// (imported/corrupt guesses: unknown names, isolated countries, the start, duplicates, > 7 entries)
const routeQueries = [];
for (let i = 0; i < 300; i++) {
  const [start, target] = rng() < 0.97 ? [pick(playable), pick(playable)] : [pick(["Nowhere", "", "Australia"]), pick(playable)];
  const shortest = travle.getTravleShortestPath(start, target);
  const pool = [...new Set([...shortest, ...shortest.flatMap((n) => travle.getTravleNeighbors(n))])];
  const guesses = Array.from({ length: Math.floor(rng() * 9) }, () => (rng() < 0.8 ? pick(pool.length ? pool : playable) : pick([...isolated, "Nowhere", start, target])));
  routeQueries.push({ start, target, guesses, solved: codes(travle.getTravleSolvedPath(start, target, guesses)), display: codes(travle.getTravleDisplayPath(start, target, guesses)), states: travle.getTravleGuessStates(start, target, guesses), isSolved: travle.isTravleRouteSolved(start, target, guesses) });
}

// ---------------------------------------------------------------- load normalization (storage.ts)
const storage = await imp("storage.ts");
const store = new Map();
globalThis.localStorage = { getItem: (k) => (store.has(k) ? store.get(k) : null), setItem: (k, v) => store.set(k, String(v)), removeItem: (k) => store.delete(k), key: (i) => [...store.keys()][i] ?? null, get length() { return store.size; } };
const RealDate = Date;
const TODAY = "2026-10-02";
const fixedNow = RealDate.parse(`${TODAY}T12:00:00`);
globalThis.Date = class extends RealDate { constructor(...a) { if (a.length === 0) super(fixedNow); else super(...a); } static now() { return fixedNow; } };
const realUuid = globalThis.crypto.randomUUID.bind(globalThis.crypto);
globalThis.crypto.randomUUID = () => "11111111-2222-4333-8444-555555555555";
const todayId = travle.getTravlePuzzleId(TODAY, "keep-salt");
const todays = travle.getTravlePuzzleForDate(TODAY, "keep-salt");
const storedCases = {
  current_midgame: { seedSalt: "keep-salt", activeDate: TODAY, puzzleId: todayId, start: todays.start, target: todays.target, guesses: [todays.shortestPath[1]], completed: false, won: false },
  current_won: { seedSalt: "keep-salt", activeDate: TODAY, puzzleId: todayId, start: todays.start, target: todays.target, guesses: todays.shortestPath.slice(1, -1), completed: true, won: true },
  current_lost: { seedSalt: "keep-salt", activeDate: TODAY, puzzleId: todayId, start: todays.start, target: todays.target, guesses: ["Australia", "Japan", "Fiji", "Cuba", "Iceland", "Malta", "Cyprus"], completed: true, won: false },
  old_day: { seedSalt: "keep-salt", activeDate: "2026-10-01", puzzleId: travle.getTravlePuzzleId("2026-10-01", "keep-salt"), start: "France", target: "Poland", guesses: ["Germany"], completed: true, won: true },
  wrong_id: { seedSalt: "keep-salt", activeDate: TODAY, puzzleId: `${TODAY}:zzz`, start: "France", target: "Poland", guesses: ["Germany"], completed: true, won: true },
  stage20_placeholder: { seedSalt: "keep-salt", activeDate: "", puzzleId: "", start: "", target: "", guesses: [], completed: false, won: false },
  missing_salt: { activeDate: TODAY, puzzleId: todayId, start: "France", target: "Poland", guesses: [], completed: false, won: false },
  empty_salt: { seedSalt: "", activeDate: TODAY, puzzleId: todayId, start: "France", target: "Poland" },
  numeric_salt: { seedSalt: 42, activeDate: TODAY, start: "France" },
  impossible_names: { seedSalt: "keep-salt", activeDate: TODAY, puzzleId: todayId, start: "Atlantis", target: "", guesses: ["Mordor", 7, null, "France", "France", "Gondor", "Narnia", "Oz", "Lilliput", "Utopia"], completed: "yes", won: 0 },
  too_many_guesses: { seedSalt: "keep-salt", activeDate: TODAY, puzzleId: todayId, start: todays.start, target: todays.target, guesses: ["A", "B", "C", "D", "E", "F", "G", "H", "I"], completed: false, won: false },
  guesses_not_array: { seedSalt: "keep-salt", activeDate: TODAY, puzzleId: todayId, start: todays.start, target: todays.target, guesses: "France", completed: 1, won: 1 },
  array: [1, 2, 3],
  string: "travle",
  null: null,
  extra_keys: { seedSalt: "keep-salt", activeDate: TODAY, puzzleId: todayId, start: todays.start, target: todays.target, guesses: [], completed: false, won: false, zoom: 2, future: { x: 1 } },
};
const normalize = [];
for (const [name, stored] of Object.entries(storedCases)) {
  store.clear();
  store.set("study-tracker-desktop-v3-core", JSON.stringify({ semesters: [], travlePuzzle: stored }));
  normalize.push({ name, stored, loaded: storage.loadAppState().travlePuzzle });
}
store.clear();
store.set("study-tracker-desktop-v3-core", JSON.stringify({ semesters: [] }));
normalize.push({ name: "absent", stored: "__absent__", loaded: storage.loadAppState().travlePuzzle });
globalThis.Date = RealDate;
globalThis.crypto.randomUUID = realUuid;

// open-time init (initTravlePuzzle) and rollover across midnight
const init = [];
for (const [name, puzzle, today] of [
  ["today_kept", { ...storedCases.current_midgame }, TODAY],
  ["next_day_resets", { ...storedCases.current_midgame }, "2026-10-03"],
  ["missing_start_resets", { ...storedCases.current_midgame, start: "" }, TODAY],
  ["missing_target_resets", { ...storedCases.current_midgame, target: "" }, TODAY],
  ["empty_salt_draws", { ...storedCases.current_midgame, seedSalt: "" }, TODAY],
  ["completed_today_kept", { ...storedCases.current_won }, TODAY],
  ["completed_yesterday_resets", { ...storedCases.current_won }, "2026-10-03"],
]) {
  const s = makeSession(puzzle, today, "drawn-salt");
  s.ui.draft = "Fra"; s.ui.message = "old"; s.ui.dropdown = true; s.ui.zoom = 2.05;
  s.init();
  init.push({ name, today, before: puzzle, after: s.ui.state.travlePuzzle, draft: s.ui.draft, message: s.ui.message, dropdown: s.ui.dropdown, zoom: s.ui.zoom });
}

// ---------------------------------------------------------------- map/view derivations (app crate)
const views = [];
const viewStates = [
  ...games.slice(0, 60).map((g) => ({ start: g.start, target: g.target, guesses: g.steps.at(-1)?.guesses ?? [] })),
  ...games.slice(0, 40).flatMap((g) => g.steps.slice(0, 3).map((st) => ({ start: g.start, target: g.target, guesses: st.guesses }))),
  { start: "Liechtenstein", target: "Vatican City", guesses: ["Switzerland", "Italy"] },
  { start: "Andorra", target: "Monaco", guesses: ["France"] },
  { start: "San Marino", target: "Liechtenstein", guesses: ["Italy", "Austria"] },
  { start: "Russia", target: "Chile", guesses: [] },
  { start: "Canada", target: "Russia", guesses: ["United States of America"] },
  { start: "France", target: "Poland", guesses: ["Australia"] },
  { start: "Atlantis", target: "Poland", guesses: ["Germany"] },
  { start: "France", target: "Poland", guesses: ["Germany", "Atlantis"] },
];
for (const [i, v] of viewStates.entries()) {
  const puzzle = { seedSalt: "v", activeDate: TODAY, puzzleId: "", completed: false, won: false, ...v };
  for (const zoom of i % 7 === 0 ? [1, 1.35, 1.7, 2.05, 2.4, 2.5] : [1]) {
    const d = derive(puzzle, zoom);
    views.push({ ...v, zoom, viewBox: d.viewBox, points: d.points, classes: d.classes, route: d.route, current: d.current, left: d.left });
  }
}
// zoom buttons: production's rounding on each click
const zoomSteps = [];
let zin = 1; for (let i = 0; i < 6; i++) { zin = Math.min(2.5, Math.round((zin + 0.35) * 100) / 100); zoomSteps.push(["in", zin]); }
let zout = zin; for (let i = 0; i < 6; i++) { zout = Math.max(1, Math.round((zout - 0.35) * 100) / 100); zoomSteps.push(["out", zout]); }
const options = ["", "a", "united", "Türk", "zz"].map((q) => ({ query: q, names: derive({ ...storedCases.current_midgame }, 1, q).options }));

rmSync(lib, { recursive: true, force: true });

const out = join(here, "..", "crates", "study-tracker-core", "tests", "fixtures", "break_room", "travle.json");
writeFileSync(out, JSON.stringify({
  meta: { countryCount: travle.TRAVLE_COUNTRY_COUNT, maxGuesses: travle.TRAVLE_MAX_GUESSES, playable: playable.length, isolated },
  daily, tzProbes, find, filter, neighbours, neighbourPairs, paths, games, routeQueries, normalize, init,
}) + "\n");
const outView = join(here, "..", "tests", "fixtures", "travle-view.json");
writeFileSync(outView, JSON.stringify({ views, zoomSteps, options }) + "\n");
console.log(`daily ${daily.length}, tz ${tzProbes.length}, find ${find.length}, filter ${filter.length}, neighbours ${neighbours.length}, pairs ${neighbourPairs.length}, paths ${paths.length}, games ${games.length} (${games.reduce((a, g) => a + g.steps.length, 0)} submits), routeQueries ${routeQueries.length}, normalize ${normalize.length}, init ${init.length}, views ${views.length}`);
