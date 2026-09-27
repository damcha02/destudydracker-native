import { inkArt } from "./inkArt";

/** Achievement ids that have real hand-illustrated art (watercolour, transparent background) in
 * `public/achievements/<id>.png`, supplied directly rather than generated. Kept as an explicit
 * set - not "does the file exist" - so a typo'd id fails obviously instead of silently 404ing an
 * <img>. Anything not listed here still gets the generated ink illustration. */
const REAL_ART_IDS = new Set([
  "full-house", "first-break", "on-fire", "early-bird", "night-owl", "speedrunner", "explorer", "perfectionist", "veteran",
  "rock-sprouting", "rock-growing", "rock-flourished", "rock-blooming", "rock-royal", "rock-hellish", "rock-heavenly",
  "rock-cosmic", "rock-galactic", "rock-eternal", "rock-meteoric", "rock-planetary", "rock-celestial", "rock-starstone",
  "rock-hells-diplomat", "rock-saint", "rock-god", "rock-demon", "rock-guardian-angel",
  "fossil-10", "fossil-25", "fossil-50", "fossil-100", "fossil-250", "fossil-500", "fossil-1000",
  "garden-first-sprout", "garden-streak-bloom", "garden-mushroom-ring", "garden-cross-pollinator",
  "garden-full-bloom", "garden-harvest-season", "garden-wise-tree",
]);

/** The picture for one achievement: real supplied art when there is a piece for this id, else the
 * same generated ink-illustration fallback as before (from the achievement's own icon glyph). */
export function achievementArt(id: string, icon: string): string {
  return REAL_ART_IDS.has(id) ? `/achievements/${id}.png` : inkArt(icon);
}

/** Whether an id resolves to real full-colour art rather than the generated ink illustration -
 * real pieces keep their own colour (no multiply blend), the generated ones still get it. */
export function isRealAchievementArt(id: string): boolean {
  return REAL_ART_IDS.has(id);
}
