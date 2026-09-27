import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties, KeyboardEvent, RefObject } from "react";
import type { Achievement } from "./AchievementBoard";
import { achievementArt, isRealAchievementArt } from "./achievementArt";

/** The quiet ways to display achievements: ema (wooden plaques on a rack), a tokonoma shelf, an ink
 * scroll, or a photo album ("book") kept closed on the table until it's opened. */
export type InkVariant = "ema" | "shelf" | "scroll" | "book";

/** Every piece is the same size, whatever the achievement. */
const SIZE = 76;
const ART_WIDTH: Record<Exclude<InkVariant, "book">, number> = { ema: SIZE * 1.28, shelf: SIZE * 1.5, scroll: SIZE * 1.5 };
/** The eave (roof) of the ema rack. */
const EAVE = 62;

type Item = { achievement: Achievement; x: number; y: number; delay: number };
type Layout = { width: number; height: number; items: Item[]; shelves: number[] };

/** Where each earned achievement goes. They simply appear, in order, and fill the display. */
function layout(variant: Exclude<InkVariant, "book">, achievements: Achievement[], box: { w: number; h: number }): Layout {
  const items: Item[] = [];
  const shelves: number[] = [];
  const count = achievements.length;
  const width = Math.max(box.w, 320);

  if (variant === "ema") {
    const cell = SIZE + 34;
    const columns = Math.max(1, Math.floor((width - 120) / cell));
    const margin = (width - columns * cell) / 2;
    achievements.forEach((achievement, index) => {
      const column = index % columns;
      const row = Math.floor(index / columns);
      // each plaque hangs from its own length of string, so the rack is not a rigid grid
      const drop = ((index * 37) % 4) * 9;
      items.push({ achievement, x: margin + column * cell + cell / 2, y: EAVE + 62 + row * (SIZE + 64) + drop, delay: (index % 7) * -0.9 });
    });
    const rows = Math.max(1, Math.ceil(count / columns));
    return { width, height: Math.max(box.h, EAVE + 62 + rows * (SIZE + 64) + 40), items, shelves };
  }

  if (variant === "shelf") {
    const cell = SIZE + 26;
    const perShelf = Math.max(1, Math.floor((width - 120) / cell));
    const margin = (width - perShelf * cell) / 2;
    const boards = Math.max(3, Math.ceil(count / perShelf));
    const spacing = SIZE + 84;
    for (let board = 0; board < boards; board += 1) shelves.push(90 + spacing + board * spacing - 40);
    achievements.forEach((achievement, index) => {
      const shelf = Math.floor(index / perShelf);
      const column = index % perShelf;
      items.push({ achievement, x: margin + column * cell + cell / 2, y: shelves[shelf] - SIZE / 2, delay: 0 });
    });
    return { width, height: Math.max(box.h, shelves[shelves.length - 1] + 60), items, shelves };
  }

  // scroll: a long sheet unrolled sideways, the marks running along a gentle wave
  const step = SIZE + 34;
  const height = Math.max(box.h, 340);
  achievements.forEach((achievement, index) => {
    items.push({ achievement, x: 110 + index * step, y: height / 2 + Math.sin(index * 0.85) * height * 0.09, delay: 0 });
  });
  return { width: Math.max(width, 110 * 2 + count * step), height, items, shelves };
}

/** One page holds three achievements, one per row. */
const PER_PAGE = 3;

function formatEarnedDay(iso: string | undefined) {
  if (!iso) return "";
  const date = new Date(`${iso}T00:00:00`);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleDateString(undefined, { month: "long", day: "numeric", year: "numeric" });
}

type BookRow = { id: string; name: string; how: string; icon: string; earnedAt?: string };
type BookPageData = { rows: BookRow[]; num: number; left: boolean; empty?: boolean };

/** Dated pieces read like a diary, oldest first; anything from before the album could track
 * dates has no known day, so it settles at the back rather than being given one it didn't earn. */
function buildBookPages(achievements: Achievement[]): BookPageData[] {
  const sorted = [...achievements].sort((a, b) => {
    if (a.earnedAt && b.earnedAt) return a.earnedAt.localeCompare(b.earnedAt) || a.id.localeCompare(b.id);
    if (a.earnedAt) return -1;
    if (b.earnedAt) return 1;
    return a.name.localeCompare(b.name);
  });
  const pages: BookPageData[] = [];
  for (let index = 0; index < sorted.length; index += PER_PAGE) {
    pages.push({ rows: sorted.slice(index, index + PER_PAGE), num: 0, left: true });
  }
  if (!pages.length) pages.push({ rows: [], num: 0, left: true, empty: true });
  if (pages.length % 2) pages.push({ rows: [], num: 0, left: false });
  return pages.map((page, index) => ({ ...page, num: index + 1, left: index % 2 === 0 }));
}

/** A short Japanese equivalent for every achievement, in the same spirit as the design's own
 * examples (深水 for "Deep water", 老松 for "Old pine") - flavour text, not a literal gloss,
 * matched by achievement id. Unmapped ids (a badge added later and not yet given one) just fall
 * back to showing the English name alone, never a blank or a guessed character. */
const JP_NAMES: Record<string, string> = {
  "full-house": "満室",
  "first-break": "初休",
  "on-fire": "熱中",
  "early-bird": "早起き",
  "night-owl": "夜更かし",
  speedrunner: "疾走",
  explorer: "探検",
  perfectionist: "完璧",
  veteran: "熟練",
  "rock-sprouting": "発芽",
  "rock-growing": "生長",
  "rock-flourished": "繁茂",
  "rock-blooming": "開花",
  "rock-royal": "王者",
  "rock-hellish": "地獄",
  "rock-heavenly": "天国",
  "rock-cosmic": "宇宙",
  "rock-galactic": "銀河",
  "rock-eternal": "永遠",
  "rock-meteoric": "流星",
  "rock-planetary": "惑星",
  "rock-celestial": "天体",
  "rock-starstone": "星石",
  "rock-hells-diplomat": "使者",
  "rock-saint": "聖人",
  "rock-god": "石神",
  "rock-demon": "悪魔",
  "rock-guardian-angel": "守護天使",
  "rock-current": "石",
  "fossil-10": "発掘",
  "fossil-25": "貝殻",
  "fossil-50": "化石",
  "fossil-100": "結晶",
  "fossil-250": "標本",
  "fossil-500": "遺物",
  "fossil-1000": "金字塔",
  "garden-first-sprout": "初芽",
  "garden-streak-bloom": "継続",
  "garden-mushroom-ring": "菌輪",
  "garden-cross-pollinator": "受粉",
  "garden-full-bloom": "満開",
  "garden-harvest-season": "収穫",
  "garden-wise-tree": "老木",
};

/** A leaf of the album: up to three rows, each headed by the day it was earned - only when that
 * day is actually known - plus a small photo-corner mount around every icon. */
const AlbumPage = memo(function AlbumPage({ page }: { page: BookPageData | null }) {
  const left = page ? page.left : true;
  return (
    <div className={`wabi-album-page ${left ? "wabi-album-page--left" : "wabi-album-page--right"}`}>
      {page?.empty ? (
        <div className="wabi-album-empty">
          <strong>The album is empty.</strong>
          <span>Achievements are mounted here as you earn them.</span>
        </div>
      ) : page && page.rows.length ? (
        <div className="wabi-album-rows">
          {page.rows.map((row) => {
            const jp = JP_NAMES[row.id];
            return (
              <div key={row.id} className="wabi-album-row">
                {row.earnedAt ? <time className="wabi-album-day">{formatEarnedDay(row.earnedAt)}</time> : null}
                <div className="wabi-album-row-main">
                  <span className="wabi-album-icon">
                    <img src={achievementArt(row.id, row.icon)} alt="" draggable={false} className={isRealAchievementArt(row.id) ? "wabi-real-art" : undefined} />
                    <i aria-hidden="true" /><i aria-hidden="true" /><i aria-hidden="true" /><i aria-hidden="true" />
                  </span>
                  <div className="wabi-album-copy">
                    <div className="wabi-album-names">
                      {jp ? <strong className="wabi-album-jp">{jp}</strong> : null}
                      <em className={jp ? "wabi-album-en" : "wabi-album-en wabi-album-en--solo"}>{row.name}</em>
                    </div>
                    <span>{row.how}</span>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      ) : null}
      {page ? <span className="wabi-album-pageno">{page.num}</span> : null}
    </div>
  );
});

const px = (value: number) => `${value.toFixed(2)}px`;
const poly = (points: [number, number][]) => `polygon(${points.map(([x, y]) => `${px(x)} ${px(y)}`).join(",")})`;

type BookGeom = {
  stageLeft: number; stageTop: number; BW: number; BH: number; margin: number; PW: number; PH: number; fontSize: number;
  cupLeft: number; cupTop: number; cupD: number;
  penLeft: number; penTop: number; penW: number; penH: number; penTip: number;
  closedX: number; closedW: number; closedH: number;
  closedShadowLeft: number; closedShadowTop: number; closedShadowW: number; openShadowW: number;
  groundShadow: string;
  pBoard: string; pSpine: string; pFore: string; pTail: string; pFront: string;
  slipLeft: number; slipTop: number; slipW: number; slipH: number;
  zoneW: number;
};

/** The book's whole geometry, derived once from the widget's measured box. Ported directly from
 * the approved design's own math (its `g` object) - the closed book's "3D" edges are five flat
 * clip-path slices of one box (cheaper than real depth, reads convincingly at this size), and the
 * binding studs/thread on the cover are sized off the resulting font-size in plain CSS `em` units,
 * exactly as the source does, so only the handful of numbers that are genuinely structural live here. */
function computeBookGeometry(w: number, h: number): BookGeom {
  const padX = Math.max(14, w * 0.05);
  const padY = Math.max(12, h * 0.08);
  const ratio = 0.78;
  const cupD = Math.min(h * 0.3, w * 0.15);
  const stripW = cupD + padX * 0.7;
  const availW = w - stripW;
  const BH = Math.max(110, Math.min(h - 2 * padY, (availW - 2 * padX) / (2 * ratio)));
  const BW = BH * ratio;
  const stageLeft = padX + Math.max(0, (availW - 2 * padX - 2 * BW) / 2);
  const stageTop = (h - BH) / 2 - BH * ratio * 0.023;
  const potX = w - padX * 0.7 - cupD / 2;
  const potY = padY + cupD * 0.55;
  const penW = Math.max(7, cupD * 0.11);
  const penTopStart = potY + cupD * 0.62;
  const penBottom = h - padY * 0.7;
  const margin = BW * 0.04;
  // Margin on *both* sides of the page, not just the side facing the spine - the right page's
  // own outer edge needs the same breathing room as the left page's, or its content (and the
  // paper's own edge) sits flush against - and visually spills past - the book's outer border.
  const PW = BW - 2 * margin;
  const PH = BH - 2 * margin;
  const fontSize = PH / 19;
  const e = BW * 0.065;
  const bevel = BW * 0.016;
  const a = BW * 0.022;
  const p0 = BW * 0.03;
  const closedX = -(BW + e + bevel) / 2;
  return {
    stageLeft, stageTop, BW, BH, margin, PW, PH, fontSize,
    cupLeft: potX - cupD / 2, cupTop: potY - cupD / 2, cupD,
    penLeft: potX - penW / 2, penTop: penTopStart, penW, penH: penBottom - penTopStart, penTip: penW * 6,
    closedX, closedW: BW + e + bevel + 1, closedH: BH + e + bevel + 1,
    closedShadowLeft: BW + e * 0.5, closedShadowTop: e * 0.6, closedShadowW: BW + bevel,
    openShadowW: 2 * BW + 4,
    groundShadow: `0 ${px(BH * 0.03)} ${px(BH * 0.09)} rgba(20,10,0,.5), 0 1px 3px rgba(0,0,0,.45)`,
    pBoard: poly([[e, e], [BW + e, e], [BW + e + bevel, e + bevel], [BW + e + bevel, BH + e + bevel], [e + bevel, BH + e + bevel], [e, BH + e]]),
    pSpine: poly([[0, BH - 1], [p0, BH], [p0 + e + bevel, BH + e + bevel], [e + bevel, BH + e + bevel]]),
    pFore: poly([[BW - a, a], [BW - a + e, a + e], [BW - a + e, BH - a + e], [BW - a, BH - a]]),
    pTail: poly([[p0, BH - a], [BW - a, BH - a], [BW - a + e, BH - a + e], [p0 + e, BH - a + e]]),
    pFront: poly([[0, 0], [BW, 0], [BW + bevel, bevel], [BW + bevel, BH + bevel], [bevel, BH + bevel], [0, BH]]),
    slipLeft: BW * 0.2, slipTop: BH * 0.08, slipW: BW * 0.15, slipH: BH * 0.42,
    zoneW: Math.max(18, BW * 0.13),
  };
}

/** A stack of paper-edge slivers behind the visible page - taller the more pages sit on that
 * side, so the book visibly thins out as you near either cover. */
function pageStackShadow(remaining: number, totalPages: number, dir: 1 | -1, fontSize: number, maxOffset: number) {
  const layers = Math.max(1, Math.min(6, Math.round((remaining / Math.max(2, totalPages)) * 6)));
  const k = fontSize / 10;
  // Clamped to the page's own margin: this is meant to peek out just past the paper's own edge,
  // like a stack of sheets underneath - not spill past the board itself, which is what made it
  // look like the paper was bigger than the book.
  const clampAxis = (raw: number) => Math.max(-maxOffset, Math.min(maxOffset, raw));
  const parts: string[] = [];
  for (let i = 1; i <= layers; i += 1) {
    parts.push(`${px(clampAxis(dir * i * 0.7 * k))} ${px(clampAxis(i * 0.8 * k))} 0 var(${i % 2 ? "--book-paper-edge-deep" : "--book-paper-edge"})`);
  }
  parts.push(`${px(clampAxis(dir * (layers + 1) * 0.7 * k))} ${px(clampAxis((layers + 1.5) * 0.8 * k))} ${px(1.5 * k)} rgba(35,20,5,.35)`);
  return parts.join(", ");
}

type BookAnim = "open" | "close" | "next" | "prev" | null;

/** How long the finished leaf/cover is held on screen after a turn has been applied, so the board
 *  it covers can repaint to its new page out of sight. A few frames is all it needs; long enough to
 *  survive a slow one, short enough that it cannot be felt as input lag on the next turn. */
const SETTLE_MS = 90;

function prefersReducedMotion() {
  return typeof window !== "undefined" && !!window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
}

/** A closed hardcover album resting on the desk. Its cover swings open on a real hinge (a scaleX
 * squish down to the spine and back out, cross-fading its two faces at the midpoint - not a
 * fade, and not a 3D rotateY, which foreshortens under perspective and reads as the page
 * shifting), and each page turn does the same to a single leaf over the spine, while the pages
 * underneath are already the next spread, so nothing "cuts" - it just turns. */
function BookGallery({ achievements }: { achievements: Achievement[] }) {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const slideRef = useRef<HTMLDivElement | null>(null);
  const coverRef = useRef<HTMLDivElement | null>(null);
  const coverFrontRef = useRef<HTMLDivElement | null>(null);
  const coverBackRef = useRef<HTMLDivElement | null>(null);
  const leafRef = useRef<HTMLDivElement | null>(null);
  const leafFrontRef = useRef<HTMLDivElement | null>(null);
  const leafBackRef = useRef<HTMLDivElement | null>(null);
  const closedEdgeRef = useRef<HTMLDivElement | null>(null);
  const timeoutRef = useRef<number | null>(null);
  const runningRef = useRef<BookAnim>(null);
  const activeAnimsRef = useRef<Animation[]>([]);

  const [size, setSize] = useState({ w: 0, h: 0 });
  const [open, setOpen] = useState(false);
  const [spread, setSpread] = useState(0);
  const [anim, setAnim] = useState<BookAnim>(null);
  // The turn has played out and its result has been applied to `open`/`spread`, but the animated
  // element is deliberately left mounted (holding its final frame) for a moment longer. That gives
  // the board underneath a chance to repaint with its new page while it is still completely hidden,
  // so removing the leaf afterwards reveals something already correct. Doing both in one commit is
  // what used to show a frame of the previous spread.
  const [settling, setSettling] = useState(false);
  const settleTimeoutRef = useRef<number | null>(null);
  /** The spread a turn started from. The three pages that are not being repainted stay pinned to
   *  this for the whole turn, so advancing `spread` part-way through cannot disturb them. State
   *  rather than a ref because the render reads it: it is set in the same batch as `anim`, so the
   *  two always commit together. */
  const [turnFrom, setTurnFrom] = useState(0);

  useLayoutEffect(() => {
    const element = rootRef.current;
    if (!element || typeof ResizeObserver === "undefined") return undefined;
    const observer = new ResizeObserver(() => {
      const w = Math.round(element.clientWidth);
      const h = Math.round(element.clientHeight);
      setSize((prev) => (prev.w === w && prev.h === h ? prev : { w, h }));
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  // On unmount, stop the clock AND cancel anything still running: a cancel fires `oncancel`, not
  // `onfinish`, so the in-flight turn can no longer come back and setState on a dead component.
  useEffect(() => () => {
    if (timeoutRef.current) window.clearTimeout(timeoutRef.current);
    if (settleTimeoutRef.current) window.clearTimeout(settleTimeoutRef.current);
    activeAnimsRef.current.forEach((animation) => { try { animation.cancel(); } catch { /* already done */ } });
    activeAnimsRef.current = [];
  }, []);

  const pages = useMemo(() => buildBookPages(achievements), [achievements]);
  const spreadCount = Math.max(1, pages.length / 2);
  const spreadIndex = Math.max(0, Math.min(spread, spreadCount - 1));
  // Turning past either end shuts the album rather than doing nothing, so you can leaf out of the
  // front or the back the way you would close a real book you had read to the end of.
  const atFirstSpread = spreadIndex <= 0;
  const atLastSpread = spreadIndex >= spreadCount - 1;
  const geometry = useMemo(() => computeBookGeometry(size.w, size.h), [size.w, size.h]);

  // Every page's pictures, fetched and decoded the moment the album is available - not the
  // moment a page is first turned to. Without this, the very first time a given page comes into
  // view (opening the book, or turning past it) the browser is still fetching/decoding its
  // <img>s while the flip animation is already running, so you catch it rendering mid-turn
  // instead of it just being there, the way a real printed page already has its picture on it.
  // Keyed on the picture URLs themselves, not on the `achievements` array: the break room re-renders
  // once a second while a timer runs, and keying this on the array identity meant re-fetching and
  // re-decoding every picture on every one of those ticks - the single biggest source of the stutter
  // during a page turn, since the decode work landed on the main thread mid-animation.
  const artKey = achievements.map((row) => achievementArt(row.id, row.icon)).join("\n");
  useEffect(() => {
    if (!artKey) return;
    for (const src of artKey.split("\n")) {
      const image = new Image();
      image.src = src;
      // decode() forces the (often costlier than the fetch) bitmap decode to happen now too,
      // off-screen - fetching alone can leave the first on-screen paint still doing that work.
      image.decode?.().catch(() => { /* decoded on demand instead if this fails */ });
    }
  }, [artKey]);

  function runAnimation(kind: Exclude<BookAnim, null>) {
    if (runningRef.current === kind) return;
    runningRef.current = kind;
    const duration = kind === "open" || kind === "close" ? 780 : 520;
    const easing = "cubic-bezier(.36,.02,.24,1)";
    const target = kind === "open" || kind === "close" ? coverRef.current : leafRef.current;
    const anims: Animation[] = [];
    let finished = false;

    const finish = () => {
      if (finished) return;
      finished = true;
      runningRef.current = null;
      if (timeoutRef.current) window.clearTimeout(timeoutRef.current);
      // Apply the result, but leave `anim` alone: the leaf (or cover) stays mounted on its last
      // frame, still covering the board it landed on, while that board quietly repaints to its new
      // page underneath. Only then does the element come away - see SETTLE_MS.
      if (kind === "open") setOpen(true);
      else if (kind === "close") { setOpen(false); setSpread(0); }
      else setSpread(turnFrom + (kind === "next" ? 1 : -1));
      setSettling(true);
      settleTimeoutRef.current = window.setTimeout(() => { setSettling(false); setAnim(null); }, SETTLE_MS);
      // The animations are NOT cancelled here. Cancelling drops their `fill: forwards` and snaps
      // every element back to its inline style, so it is only safe once React has committed the
      // render that clears `anim` - which unmounts the leaf and repaints the spread at its new
      // page. This used to cancel from a requestAnimationFrame, betting one frame was enough for
      // that commit; when it wasn't, the leaf was still on screen and its two faces reverted to
      // their starting opacities, showing one frame of the OLD spread a few hundred ms after the
      // turn had visibly finished. The layout effect on `anim` does the cancelling instead.
    };

    if (!target?.animate) { finish(); return; }

    // A real turn: the leaf swings a half circle about the spine under perspective, so the page
    // foreshortens the way paper does instead of being squashed flat. Because the leaf is exactly
    // a half-board pinned at the spine, half a turn lands it on the opposite board on its own -
    // there is no sideways jump and no pivot flip to keep in step with one any more.
    //
    // The earlier attempt at this was reverted because a `backface-visibility: hidden` face past
    // 90deg painted as an opaque rectangle under this compositor rather than vanishing. Nothing
    // here relies on backface culling: the two faces cross-fade on opacity at the midpoint (see
    // `swap` below), where the leaf is edge-on and a zero-width sliver, so whichever face is
    // wrong-way-round is already fully transparent.
    //
    // Sign: CSS rotateY sends the +X side away from the viewer, so a right-hand page pivoting on
    // its left edge has to go negative for its free edge to lift towards you before sweeping left.
    // "prev" is the mirror image and goes positive.
    //
    // The cover is hinged at the same spine and swings through the same half circle: opening lays
    // it over onto the left board (0 -> -180), closing brings it back onto the right (-180 -> 0).
    // It used to keep the flat scaleX squish, which both read as a squash rather than a swing and
    // made the browser re-rasterise the cloth's eight stacked gradients at a new width every
    // frame - the reason opening and closing felt rougher than turning a page.
    const spin = kind === "next" ? -180 : kind === "prev" ? 180 : kind === "open" ? -180 : 0;
    const from = kind === "close" ? -180 : 0;
    anims.push(target.animate(
      [{ transform: `rotateY(${from}deg)` }, { transform: `rotateY(${spin}deg)` }],
      { duration, easing, fill: "forwards" },
    ));

    // Deliberately just the rotation plus the one cross-fade (and, for open/close, the cover's
    // own reveal/slide) - the cast-shadow and shade overlays this used to also animate were pure
    // polish, and each extra concurrent WAAPI animation is more for the compositor to keep up
    // with. Fewer moving parts reads as smoother than more "realistic" ones that stutter.
    const swap = (outRef: RefObject<HTMLDivElement | null>, inRef: RefObject<HTMLDivElement | null>) => {
      const options = { duration, easing, fill: "forwards" as const };
      if (outRef.current) anims.push(outRef.current.animate([{ offset: 0, opacity: 1 }, { offset: 0.5, opacity: 1 }, { offset: 0.501, opacity: 0 }, { offset: 1, opacity: 0 }], options));
      if (inRef.current) anims.push(inRef.current.animate([{ offset: 0, opacity: 0 }, { offset: 0.5, opacity: 0 }, { offset: 0.501, opacity: 1 }, { offset: 1, opacity: 1 }], options));
    };

    if (kind === "open") {
      swap(coverFrontRef, coverBackRef);
      if (closedEdgeRef.current) anims.push(closedEdgeRef.current.animate([{ opacity: 1 }, { opacity: 0, offset: 0.3 }, { opacity: 0 }], { duration, fill: "forwards" }));
      if (slideRef.current) anims.push(slideRef.current.animate([{ transform: `translateX(${px(geometry.closedX)})` }, { transform: "translateX(0px)" }], { duration, easing: "cubic-bezier(.5,0,.25,1)", fill: "forwards" }));
    } else if (kind === "close") {
      swap(coverBackRef, coverFrontRef);
      if (closedEdgeRef.current) anims.push(closedEdgeRef.current.animate([{ opacity: 0 }, { opacity: 0, offset: 0.72 }, { opacity: 1 }], { duration, fill: "forwards" }));
      if (slideRef.current) anims.push(slideRef.current.animate([{ transform: "translateX(0px)" }, { transform: `translateX(${px(geometry.closedX)})` }], { duration, easing: "cubic-bezier(.5,0,.25,1)", fill: "forwards" }));
    } else {
      // Same for both turn directions now: the lifting page (front face) fades out at the midpoint
      // and the landing page (back face) fades in, whichever way the leaf is travelling.
      swap(leafFrontRef, leafBackRef);
    }

    activeAnimsRef.current = anims;
    anims[0].onfinish = finish;
    timeoutRef.current = window.setTimeout(finish, duration + 250);
  }

  // Setting `anim` is what puts the leaf (or the cover) in the DOM; the animation can only be
  // started once it is there. A layout effect is exactly that moment - it runs in the same commit,
  // with the refs already attached and before the browser paints. This used to be a 20ms timer,
  // which on a busy main thread landed well past 20ms and read as a stall before the page moved.
  useLayoutEffect(() => {
    if (anim) { runAnimation(anim); return; }
    // `anim` has just cleared, and this runs after the commit that acted on it - the leaf is gone
    // and the spread is painted at its new page - so the finished animations can now be cancelled
    // without anything snapping back to an inline style that is still on screen.
    activeAnimsRef.current.forEach((animation) => { try { animation.cancel(); } catch { /* already done */ } });
    activeAnimsRef.current = [];
    // runAnimation closes over the current geometry, which is why this re-reads it per `anim`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [anim]);

  function openBook() {
    if (open || anim) return;
    if (prefersReducedMotion()) { setOpen(true); setSpread(0); return; }
    setAnim("open");
  }
  function turn(direction: "next" | "prev") {
    const target = spreadIndex + (direction === "next" ? 1 : -1);
    if (!open || anim || target < 0 || target >= spreadCount) return;
    if (prefersReducedMotion()) { setSpread(target); return; }
    setTurnFrom(spreadIndex);
    setAnim(direction);
  }
  function closeBook() {
    if (!open || anim) return;
    if (prefersReducedMotion()) { setOpen(false); setSpread(0); return; }
    setSpread(0);
    setAnim("close");
  }
  /** One step forward: open a shut album, turn a page, or close it once past the last spread. */
  function pageForward() {
    if (!open) { openBook(); return; }
    if (atLastSpread) closeBook(); else turn("next");
  }
  /** One step back: turn a page, or close the album when already on the first spread. */
  function pageBack() {
    if (!open) return;
    if (atFirstSpread) closeBook(); else turn("prev");
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if ((event.key === "Enter" || event.key === " ") && !open) { openBook(); event.preventDefault(); }
    else if (event.key === "Escape") closeBook();
  }

  // Left/Right turn the album from anywhere on the page, not only when the scene itself happens to
  // hold focus: nobody thinks to click a book before turning its page, and until now the arrow keys
  // silently did nothing unless you had. Enter/Space/Escape stay on the focused element above -
  // they collide with buttons and with the app's own window-level Escape handlers otherwise.
  useEffect(() => {
    const onArrowKey = (event: globalThis.KeyboardEvent) => {
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
      if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) return;
      const target = event.target as HTMLElement | null;
      // Never steal the caret keys from somewhere the user is actually typing or picking.
      if (target && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName))) return;
      if (event.key === "ArrowRight") pageForward();
      else if (open) pageBack();
      else return;
      event.preventDefault();
    };
    window.addEventListener("keydown", onArrowKey);
    return () => window.removeEventListener("keydown", onArrowKey);
    // turn/openBook close over open, anim and the current spread, so this re-binds when they move.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, anim, spreadIndex, spreadCount]);

  const at = (index: number) => pages[index] ?? null;
  // Laid out from the spread the turn STARTED on, so that advancing `spread` at the end of the turn
  // moves only the one board we mean to move (below) and leaves the other three pages alone.
  const base = anim === "next" || anim === "prev" ? turnFrom : spreadIndex;
  const L = base * 2;
  const R = L + 1;
  let leftPage = at(L);
  let rightPage = at(R);
  let leafFront: BookPageData | null = null;
  let leafBack: BookPageData | null = null;
  // Both directions put the page that LIFTS on the front face and the page that LANDS on the back
  // face, so the front is always the one on show at rotation 0 and the back always the one on show
  // at the half turn. That has to match the faces' own pre-rotation: the back face is permanently
  // rotateY(180deg) so it reads square once the leaf has turned over, which also means it reads
  // mirrored before the leaf has moved. Giving "prev" the opposite assignment - as it had, from
  // back when the turn was a flat scaleX squish and mirroring could not arise - showed the whole
  // reverse turn back-to-front.
  //
  // The board the leaf lands on repaints to its destination page the moment `spread` advances,
  // which finish() does while the landed leaf still covers it completely. It must NOT happen any
  // earlier: halfway through the turn the leaf stands edge-on and is exactly zero pixels wide, so
  // it hides nothing at all, and swapping the page there put the next page's pictures on the
  // previous page in full view.
  if (anim === "next") { rightPage = at(R + 2); leafFront = at(R); leafBack = at(R + 1); if (spread > base) leftPage = at(L + 2); }
  if (anim === "prev") { leftPage = at(L - 2); leafFront = at(L); leafBack = at(L - 1); if (spread < base) rightPage = at(R - 2); }
  if (anim === "open") rightPage = at(1);

  const opening = anim === "open";
  const showLeft = open && anim !== "close";
  const showRight = open || opening;
  const showClosed = !open || anim === "close";
  // Held past the end of an opening too: the cover is what hides the left board while that board
  // paints its first page, exactly as the leaf does for a page turn.
  const showCover = !open || anim === "close" || (anim === "open" && settling);
  const showZones = open;
  const turning = anim === "next" || anim === "prev";
  const g = geometry;
  const ready = size.w > 0 && size.h > 0;

  return (
    <div
      ref={rootRef}
      className="wabi-book-scene"
      tabIndex={0}
      onKeyDown={onKeyDown}
      aria-label="Achievement album"
    >
      <div className="wabi-book-shoji" aria-hidden="true" />
      {ready ? (
        <>
          <div className="wabi-book-cup" style={{ left: px(g.cupLeft), top: px(g.cupTop), width: px(g.cupD), height: px(g.cupD) }} aria-hidden="true">
            <i className="wabi-book-cup-rim" />
            <i className="wabi-book-cup-inner" />
            <i className="wabi-book-cup-well" />
            <i className="wabi-book-cup-pool" />
          </div>
          <div className="wabi-book-pen" style={{ left: px(g.penLeft), top: px(g.penTop), width: px(g.penW), height: px(g.penH), "--pen-tip": px(g.penTip) } as CSSProperties} aria-hidden="true">
            <i className="wabi-book-pen-tip" />
            <i className="wabi-book-pen-shaft" />
            <i className="wabi-book-pen-cap" />
          </div>

          <div className="wabi-book-stage" style={{ left: px(g.stageLeft), top: px(g.stageTop), width: px(2 * g.BW), height: px(g.BH) }}>
            <div
              ref={slideRef}
              className="wabi-book-slide"
              onClick={!open && !anim ? openBook : undefined}
              style={{
                transform: open ? "translateX(0px)" : `translateX(${px(g.closedX)})`,
                cursor: !open && !anim ? "pointer" : undefined,
                // The viewer's distance from the page, scaled to the book so the turn foreshortens
                // the same at any window size. Shallower than this and the sweeping page bulges;
                // deeper and the turn flattens back out into the old squash. The default
                // perspective-origin (50% 50%) is the spine, which is what the leaf pivots on.
                perspective: px(g.BW * 4.2),
              }}
            >
              <div className="wabi-book-groundshadow" style={{ left: px(g.closedShadowLeft), top: px(g.closedShadowTop), width: px(g.closedShadowW), height: px(g.BH), boxShadow: g.groundShadow, opacity: open || opening ? (anim === "close" ? 1 : 0) : 1 }} />
              <div className="wabi-book-groundshadow" style={{ left: "-2px", top: "1px", width: px(g.openShadowW), height: px(g.BH), boxShadow: g.groundShadow, opacity: open || opening ? (anim === "close" ? 0 : 1) : 0 }} />

              {showLeft ? (
                <>
                  <div className="wabi-book-board wabi-book-board--left" style={{ width: px(g.BW), height: px(g.BH) }} />
                  <div className="wabi-book-pagebox" style={{ left: px(g.margin), top: px(g.margin), width: px(g.PW), height: px(g.PH), boxShadow: pageStackShadow(L + 1, pages.length, -1, g.fontSize, g.margin - 1), fontSize: px(g.fontSize) }}>
                    <AlbumPage page={leftPage} />
                  </div>
                </>
              ) : null}

              {showRight ? (
                <>
                  <div className="wabi-book-board wabi-book-board--right" style={{ left: px(g.BW), width: px(g.BW), height: px(g.BH) }} />
                  <div className="wabi-book-pagebox" style={{ left: px(g.BW + g.margin), top: px(g.margin), width: px(g.PW), height: px(g.PH), boxShadow: pageStackShadow(pages.length - R, pages.length, 1, g.fontSize, g.margin - 1), fontSize: px(g.fontSize) }}>
                    <AlbumPage page={rightPage} />
                  </div>
                </>
              ) : null}

              {showClosed ? (
                <div ref={closedEdgeRef} className="wabi-book-closededge" onClick={openBook} style={{ left: px(g.BW), width: px(g.closedW), height: px(g.closedH) }}>
                  <div className="wabi-book-closedface wabi-book-closedface--board" style={{ clipPath: g.pBoard }} />
                  <div className="wabi-book-closedface wabi-book-closedface--spine" style={{ clipPath: g.pSpine }} />
                  <div className="wabi-book-closedface wabi-book-closedface--fore" style={{ clipPath: g.pFore }} />
                  <div className="wabi-book-closedface wabi-book-closedface--tail" style={{ clipPath: g.pTail }} />
                  <div className="wabi-book-closedface wabi-book-closedface--front" style={{ clipPath: g.pFront }} />
                </div>
              ) : null}

            {/* The leaf is a whole half-board, not just the paper: it carries its page inset by the
                same `margin` as the static pageboxes, so at either end of the turn the paper lands
                exactly on the pagebox it replaces, and the leaf's own edge at the spine is the
                pivot. Sizing it to the paper instead left it a full margin off on both sides of the
                spine - which is what made a strip of the next page show at the gutter the instant a
                turn began, and the turning page's text sit shifted towards the fore-edge. */}
            {turning ? (
              <div
                ref={leafRef}
                className="wabi-book-leaf"
                style={{
                  left: px(anim === "prev" ? 0 : g.BW),
                  top: 0,
                  width: px(g.BW),
                  height: px(g.BH),
                  fontSize: px(g.fontSize),
                  transformOrigin: anim === "prev" ? "100% 50%" : "0% 50%",
                  // Only ever set while the leaf exists, which is only during a turn - so this is
                  // not the permanent layer the stylesheet used to pin. It holds the compositor
                  // layer steady across the moment the animation ends: without it the layer was
                  // promoted by the animation and dropped again the instant it finished, and the
                  // leaf went missing for a frame right as it landed.
                  willChange: "transform",
                }}
              >
                <div ref={leafFrontRef} className="wabi-book-leaf-face" style={{ opacity: 1, willChange: "opacity" }}>
                  <div className="wabi-book-pagebox" style={{ left: px(g.margin), top: px(g.margin), width: px(g.PW), height: px(g.PH) }}>
                    <AlbumPage page={leafFront} />
                  </div>
                </div>
                <div ref={leafBackRef} className="wabi-book-leaf-face wabi-book-leaf-face--back" style={{ opacity: 0, willChange: "opacity" }}>
                  <div className="wabi-book-pagebox" style={{ left: px(g.margin), top: px(g.margin), width: px(g.PW), height: px(g.PH) }}>
                    <AlbumPage page={leafBack} />
                  </div>
                </div>
              </div>
            ) : null}

            {showCover ? (
              <div
                ref={coverRef}
                className="wabi-book-cover-flip"
                style={{
                  left: px(g.BW), width: px(g.BW), height: px(g.BH), fontSize: px(g.fontSize),
                  // A close begins with the cover already laid open over the left board, so it
                  // starts its swing from there rather than popping upright for a frame first.
                  transform: anim === "close" ? "rotateY(-180deg)" : "rotateY(0deg)",
                  ...(anim === "open" || anim === "close" ? { willChange: "transform" } : {}),
                }}
              >
                <div
                  ref={coverFrontRef}
                  className="wabi-book-cover-face"
                  aria-label="Open the album"
                  onClick={openBook}
                  style={{ opacity: anim === "close" ? 0 : 1 }}
                >
                  <span className="wabi-book-cover-bevel" />
                  <span className="wabi-book-cover-crease" />
                  <span className="wabi-book-cover-thread" />
                  <span className="wabi-book-stud" style={{ top: "9%" }} />
                  <span className="wabi-book-stud" style={{ top: "36.3%" }} />
                  <span className="wabi-book-stud" style={{ top: "63.7%" }} />
                  <span className="wabi-book-stud" style={{ top: "91%" }} />
                  <span className="wabi-book-tick" style={{ top: "9%" }} />
                  <span className="wabi-book-tick" style={{ top: "36.3%" }} />
                  <span className="wabi-book-tick" style={{ top: "63.7%" }} />
                  <span className="wabi-book-tick" style={{ top: "91%" }} />
                  <div className="wabi-book-slip" style={{ left: px(g.slipLeft), top: px(g.slipTop), width: px(g.slipW), height: px(g.slipH) }}>
                    <span className="wabi-book-slip-rule" />
                    <span className="wabi-book-slip-text">記念帖</span>
                    <span className="wabi-book-slip-seal">記</span>
                  </div>
                </div>
                <div ref={coverBackRef} className="wabi-book-cover-back" style={{ opacity: anim === "close" ? 1 : 0 }}>
                  <div className="wabi-book-pagebox" style={{ left: px(g.margin), top: px(g.margin), width: px(g.PW), height: px(g.PH) }}>
                    <AlbumPage page={at(0)} />
                  </div>
                </div>
              </div>
            ) : null}

            {/* Not aria-hidden: these are the only real controls for turning the page, and hiding
                their container took the two buttons (and their labels) away from screen readers. */}
            {showZones ? (
              <div className="wabi-book-zones">
                <button
                  type="button"
                  className="wabi-book-zone wabi-book-zone--left"
                  style={{ width: px(g.zoneW) }}
                  aria-label={atFirstSpread ? "Close the album" : "Previous pages"}
                  disabled={Boolean(anim)}
                  onClick={pageBack}
                />
                <button
                  type="button"
                  className="wabi-book-zone wabi-book-zone--right"
                  style={{ width: px(g.zoneW) }}
                  aria-label={atLastSpread ? "Close the album" : "Next pages"}
                  disabled={Boolean(anim)}
                  onClick={pageForward}
                />
              </div>
            ) : null}
            </div>
          </div>
        </>
      ) : null}
    </div>
  );
}

export function InkGallery({ variant, achievements }: { variant: InkVariant; achievements: Achievement[] }) {
  const [board, setBoard] = useState<HTMLDivElement | null>(null);
  const [box, setBox] = useState({ w: 0, h: 0 });
  const [tip, setTip] = useState<{ id: string; x: number; y: number } | null>(null);

  useEffect(() => {
    if (!board) return undefined;
    const measure = () => setBox({ w: board.clientWidth, h: board.clientHeight });
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(board);
    return () => observer.disconnect();
  }, [board]);

  if (variant === "book") {
    return (
      <div className="wabi-ink wabi-ink--book">
        <BookGallery achievements={achievements} />
      </div>
    );
  }

  const laid = layout(variant, achievements, box);
  const tipped = tip ? achievements.find((achievement) => achievement.id === tip.id) ?? null : null;

  return (
    <div className={`wabi-ink wabi-ink--${variant}`}>
      <div className="wabi-ink-board" ref={setBoard}>
        <div className="wabi-ink-canvas" style={{ width: laid.width, height: laid.height }}>
          {variant === "ema" ? (
            <>
              <div className="wabi-ema-eave" aria-hidden="true" />
              <div className="wabi-ema-post wabi-ema-post--left" aria-hidden="true" />
              <div className="wabi-ema-post wabi-ema-post--right" aria-hidden="true" />
            </>
          ) : null}
          {variant === "shelf" ? laid.shelves.map((top) => <div key={top} className="wabi-shelf-board" style={{ top }} aria-hidden="true" />) : null}
          {variant === "scroll" ? (
            <>
              <div className="wabi-scroll-roller wabi-scroll-roller--left" aria-hidden="true" />
              <div className="wabi-scroll-roller wabi-scroll-roller--right" aria-hidden="true" />
              <svg className="wabi-scroll-brush" viewBox="0 0 1000 60" preserveAspectRatio="none" aria-hidden="true"><path d="M0 34 C 120 6, 210 58, 340 30 S 560 8, 680 34 S 880 54, 1000 26" /></svg>
            </>
          ) : null}

          {achievements.length === 0 ? (
            <p className="wabi-ink-empty">{variant === "ema" ? "An empty rack. Plaques appear here as you earn them." : variant === "shelf" ? "An empty alcove. Things appear here as you earn them." : "A blank scroll. Marks appear here as you earn them."}</p>
          ) : null}

          {laid.items.map(({ achievement, x, y, delay }) => (
            <span
              key={achievement.id}
              className={`wabi-ink-piece wabi-ink-piece--${variant}`}
              style={{ left: x, top: y, width: SIZE, height: SIZE, "--delay": `${delay}s` } as CSSProperties}
              onPointerEnter={(event) => setTip({ id: achievement.id, x: event.clientX, y: event.clientY })}
              onPointerMove={(event) => setTip((current) => (current ? { ...current, x: event.clientX, y: event.clientY } : current))}
              onPointerLeave={() => setTip(null)}
            >
              <img
                src={achievementArt(achievement.id, achievement.icon)}
                alt=""
                draggable={false}
                style={{ width: ART_WIDTH[variant] }}
                className={isRealAchievementArt(achievement.id) ? "wabi-real-art" : undefined}
              />
            </span>
          ))}
        </div>
      </div>

      {tipped && tip ? (
        <div className="wabi-ach-tip" style={{ left: tip.x + 14, top: tip.y + 16 }} role="tooltip">
          <strong>{tipped.name}</strong>
          <span>{tipped.how}</span>
        </div>
      ) : null}
    </div>
  );
}
