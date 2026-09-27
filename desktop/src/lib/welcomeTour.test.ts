import { describe, expect, it } from "vitest";
import { WELCOME_TOUR_STOPS } from "./welcomeTour";

describe("WELCOME_TOUR_STOPS", () => {
  it("covers every tab in tab-bar order, with the menu last", () => {
    expect(WELCOME_TOUR_STOPS.map((stop) => stop.id)).toEqual([
      "dashboard",
      "planner",
      "timer",
      "vault",
      "break",
      "friends",
      "menu",
    ]);
  });

  it("gives every stop an icon, a one-line tagline and a body", () => {
    for (const stop of WELCOME_TOUR_STOPS) {
      expect(stop.icon, `${stop.id} needs an icon`).not.toBe("");
      expect(stop.label, `${stop.id} needs a label`).not.toBe("");
      // The tagline sits in a list on the overview step, so it has to stay one short line.
      expect(stop.tagline.length, `${stop.id} tagline too long`).toBeLessThanOrEqual(48);
      expect(stop.body.length, `${stop.id} needs a real body`).toBeGreaterThan(40);
    }
  });
});
