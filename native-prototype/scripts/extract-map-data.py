#!/usr/bin/env python3
"""One-time (re-runnable) extraction of the Stage 11 map dataset.

Reads (READ-ONLY) the production Study Tracker files
  desktop/src/lib/travleMapData.ts   (pre-projected SVG country paths, viewBox 0 0 1000 500)
  desktop/src/lib/countries.ts       (names / continent / region / population / area)
and writes a compact tab-separated copy to native-prototype/assets/map/world-countries.tsv.

Columns: code, name, continent, region, population, area_km2, point_x, point_y, path
  - path is the original absolute `M x y L x y ... Z` data (sub-paths separated by " M"); empty for point-only countries
  - point_x/point_y are set only for point-only microstates ("-" otherwise)
Nothing under desktop/ is modified.
"""
import json, re, pathlib, sys

root = pathlib.Path(__file__).resolve().parents[2]
ts = (root / "desktop/src/lib/travleMapData.ts").read_text()
countries_ts = (root / "desktop/src/lib/countries.ts").read_text()

start = countries_ts.index("export const COUNTRIES = [") + len("export const COUNTRIES = ")
end = countries_ts.index("\n] ", start) + 2
facts = {c["code"]: c for c in json.loads(countries_ts[start:end])}

entries = re.findall(r'\{ code: "([A-Z]{3})"(?:, d: "([^"]*)")?(?:, point: \[([^\]]*)\])?, bounds: \[([^\]]*)\] \}', ts)
out = []
for code, d, point, _bounds in entries:
    f = facts.get(code, {})
    px, py = ("-", "-") if not point else [v.strip() for v in point.split(",")]
    out.append("\t".join([
        code, f.get("name", code), f.get("continent", "Unknown"), f.get("region", ""),
        str(f.get("population", 0)), str(f.get("areaKm2", 0)), px, py, d or "",
    ]))
dest = root / "native-prototype/assets/map/world-countries.tsv"
dest.write_text("\n".join(out) + "\n")
print(f"wrote {len(out)} regions to {dest} ({dest.stat().st_size} bytes)", file=sys.stderr)
