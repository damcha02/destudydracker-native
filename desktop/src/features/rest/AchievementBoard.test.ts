import { describe, expect, it } from "vitest";
import { isFree, SIZE } from "./AchievementBoard";

// A 1000x600 board (as a DOMRect-like) with one icon centred at (500, 300).
const board = { width: 1000, height: 600 } as DOMRect;
const placed = [{ id: "a", x: 0.5, y: 0.5 }];
const big = [{ id: "a", x: 0.5, y: 0.5, size: 80 }];

describe("achievement board overlap rule", () => {
  it("refuses to sit on top of another icon, or over its middle", () => {
    expect(isFree(board, placed, 500, 300)).toBe(false);
    expect(isFree(board, placed, 500 + SIZE * 0.3, 300)).toBe(false);
    expect(isFree(board, placed, 500 + SIZE * 0.5, 300 + SIZE * 0.5)).toBe(false);
  });

  it("allows overlapping only a little at the edges", () => {
    expect(isFree(board, placed, 500 + SIZE * 0.8, 300)).toBe(true); // 20% overlap
    expect(isFree(board, placed, 500 + SIZE * 0.8, 300 + SIZE * 0.8)).toBe(true); // corner touch
  });

  it("never lets an icon reach the other's centre", () => {
    // at the smallest allowed gap the new icon's edge is still a quarter of an icon short of the centre
    const gap = SIZE * 0.75;
    expect(isFree(board, placed, 500 + gap, 300)).toBe(true);
    expect(gap - SIZE / 2).toBeGreaterThan(0);
  });

  it("scales the gap with the larger piece when sizes differ", () => {
    expect(isFree(board, big, 500 + 80 * 0.7, 300, 50)).toBe(false); // too close to the big one
    expect(isFree(board, big, 500 + 80 * 0.8, 300, 50)).toBe(true);
    // a big piece dropped near a small one: still measured by the big piece
    expect(isFree(board, placed, 500 + 60 * 0.8, 300, 80)).toBe(false);
    expect(isFree(board, placed, 500 + 80 * 0.8, 300, 80)).toBe(true);
  });

  it("is free on an empty board", () => {
    expect(isFree(board, [], 100, 100)).toBe(true);
  });
});
