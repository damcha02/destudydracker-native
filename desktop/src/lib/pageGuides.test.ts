import { describe, expect, it } from "vitest";
import { PAGE_GUIDES } from "./pageGuides";
import { WELCOME_TOUR_STOPS } from "./welcomeTour";

describe("PAGE_GUIDES", () => {
  it("covers every tab except the Dashboard, whose question mark reopens the introduction", () => {
    expect(Object.keys(PAGE_GUIDES).sort()).toEqual(["break", "friends", "planner", "timer", "vault"]);
    expect(PAGE_GUIDES).not.toHaveProperty("dashboard");
  });

  it("uses the same icon and title for a tab as the introduction does", () => {
    for (const [tab, guide] of Object.entries(PAGE_GUIDES)) {
      const stop = WELCOME_TOUR_STOPS.find((item) => item.id === tab);
      expect(stop, `${tab} should also be a tour stop`).toBeDefined();
      expect(guide.icon, `${tab} icon differs from the introduction`).toBe(stop?.icon);
      expect(guide.title, `${tab} title differs from the introduction`).toBe(stop?.label);
    }
  });

  it("gives every guide an intro and real steps", () => {
    for (const [tab, guide] of Object.entries(PAGE_GUIDES)) {
      expect(guide.tagline.length, `${tab} tagline too long`).toBeLessThanOrEqual(48);
      expect(guide.intro.length, `${tab} needs an intro`).toBeGreaterThan(40);
      expect(guide.stepsHeading, `${tab} needs a steps heading`).not.toBe("");
      expect(guide.steps.length, `${tab} needs at least three steps`).toBeGreaterThanOrEqual(3);
      const leads = guide.steps.map((step) => step.lead);
      // The list is keyed by lead, so duplicates would collide in React.
      expect(new Set(leads).size, `${tab} has duplicate step leads`).toBe(leads.length);
      for (const step of guide.steps) {
        expect(step.body.length, `${tab}: "${step.lead}" needs a body`).toBeGreaterThan(30);
      }
    }
  });
});
