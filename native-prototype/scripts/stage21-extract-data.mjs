// Stage 21: extracts production's Travle data, verbatim and in production order, from
// desktop/src/lib/{countryBorders,travleMapData}.ts (copied read-only into a temp dir and imported
// with Node's built-in type stripping - nothing in desktop/ is modified or executed in place).
//
//   node scripts/stage21-extract-data.mjs [--desktop ../desktop]
//
// Writes:
//   crates/study-tracker-core/data/break_room/borders.tsv   COUNTRY_BORDERS: "<code>\t<n1> <n2> ..."
//       (object key order and neighbour order kept: production's BFS visits neighbours in this
//       order, so it decides which of several equally short routes is returned)
//   assets/map/travle-map.tsv   TRAVLE_MAP_COUNTRIES: "<code>\t<px>\t<py>\t<minx>\t<miny>\t<maxx>\t<maxy>\t<d>"
//       ("-" for an absent point / path; numbers printed with JS String(), which round-trips)
//
// The map file replaces Stage 11's world-countries.tsv (same geometry, now with production's own
// bounds and points), so the app embeds exactly one copy of the map.
import { cpSync, mkdtempSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { join, dirname } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const args = Object.fromEntries(process.argv.slice(2).reduce((acc, a, i, all) => (a.startsWith("--") ? [...acc, [a.slice(2), all[i + 1]]] : acc), []));
const desktop = args.desktop ?? join(here, "..", "..", "desktop");
const root = join(here, "..");

const lib = mkdtempSync(join(tmpdir(), "st21-extract-"));
for (const f of ["countryBorders.ts", "travleMapData.ts"]) cpSync(join(desktop, "src/lib", f), join(lib, f));
const { COUNTRY_BORDERS } = await import(pathToFileURL(join(lib, "countryBorders.ts")).href);
const { TRAVLE_MAP_COUNTRIES, TRAVLE_MAP_VIEWBOX } = await import(pathToFileURL(join(lib, "travleMapData.ts")).href);
rmSync(lib, { recursive: true, force: true });

if (TRAVLE_MAP_VIEWBOX !== "0 0 1000 500") throw new Error(`unexpected viewBox ${TRAVLE_MAP_VIEWBOX}`);

const borders = Object.entries(COUNTRY_BORDERS).map(([code, list]) => `${code}\t${list.join(" ")}`);
writeFileSync(join(root, "crates/study-tracker-core/data/break_room/borders.tsv"),
  "# code\tneighbours (production countryBorders.ts, key and neighbour order kept)\n" + borders.join("\n") + "\n");

const num = (v) => (v === undefined ? "-" : String(v));
const rows = TRAVLE_MAP_COUNTRIES.map((c) => {
  if (/[\t\n]/.test(c.d ?? "")) throw new Error(`tab/newline in path of ${c.code}`);
  return [c.code, num(c.point?.[0]), num(c.point?.[1]), ...c.bounds.map(String), c.d ?? "-"].join("\t");
});
writeFileSync(join(root, "assets/map/travle-map.tsv"),
  "# code\tpoint_x\tpoint_y\tmin_x\tmin_y\tmax_x\tmax_y\tpath (production travleMapData.ts, viewBox 0 0 1000 500)\n" + rows.join("\n") + "\n");
console.log(`borders: ${borders.length} countries; map: ${rows.length} countries`);
