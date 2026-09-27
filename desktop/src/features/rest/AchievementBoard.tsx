import { useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import { createPortal } from "react-dom";
import { graffitiArt } from "./graffiti";

/** rarity 0..1: how hard it is to earn, which sets how big it is sprayed (harder = bigger). */
export type Achievement = {
  id: string; icon: string; name: string; how: string; rarity?: number;
  /** Daily achievements can be earned again every day: one small piece per time earned. */
  daily?: boolean;
  /** How many pieces of it may hang on the wall (1 unless it is a daily one). */
  copies?: number;
  /** The day (YYYY-MM-DD) this was first seen earned. Only the "book" wall uses it. */
  earnedAt?: string;
};
/** x/y: icon centre as a fraction of the board; size in px; colour and tilt are the spray-can look. */
export type Placement = {
  id: string; x: number; y: number; size?: number; color?: string; rot?: number;
  /** Unique per piece, since a daily achievement can hang several times. */
  uid?: string;
  /** A copy of the achievement as it was when hung, so the wall never loses a piece if the live badge state changes. */
  icon?: string; name?: string; how?: string;
};

/** Base icon box in px, and the random range a sprayed piece can take (not too big, not too small). */
export const SIZE = 60;
const MIN_SIZE = 40;
const MAX_SIZE = 134;
/** Daily achievements are small keepsakes, however many of them there are. */
const DAILY_MIN = 32;
const DAILY_MAX = 46;
const SPRAY_COLOURS = ["#ff3d81", "#19c3e6", "#ffd23f", "#7ed957", "#ff8a1f", "#9b5cff", "#ff4d4d"];
/**
 * Two pieces may only touch at their edges: the centres must be at least this fraction of the larger piece
 * apart along the wider axis. At 0.75 a piece can overlap another by at most a quarter of its size, so it can
 * never cover the other's middle (or anywhere near it).
 */
const MIN_CENTRE_GAP = 0.75;
/** The artwork canvas is larger than the glyph inside it (room for outline, shadow and drips). */
const ART_SCALE = 1.62;
/** One brick, in px. Rows alternate by half a brick, and only whole bricks are drawn. */
const BRICK_W = 60;
const BRICK_H = 24;

type Drag = { id: string; pointerX: number; pointerY: number; size: number; color: string; rot: number };

function hash(text: string) {
  let value = 2166136261;
  for (let index = 0; index < text.length; index += 1) {
    value ^= text.charCodeAt(index);
    value = Math.imul(value, 16777619);
  }
  return value >>> 0;
}

function boardPosition(board: DOMRect, pointerX: number, pointerY: number, size: number) {
  const half = size / 2;
  const px = Math.min(Math.max(pointerX - board.left, half), board.width - half);
  const py = Math.min(Math.max(pointerY - board.top, half), board.height - half);
  return { px, py, x: px / board.width, y: py / board.height };
}

export function isFree(board: DOMRect, placements: Placement[], px: number, py: number, size = SIZE) {
  return placements.every((placed) => {
    const dx = Math.abs(placed.x * board.width - px);
    const dy = Math.abs(placed.y * board.height - py);
    return Math.max(dx, dy) >= Math.max(size, placed.size ?? SIZE) * MIN_CENTRE_GAP;
  });
}

export function AchievementBoard({ achievements, placements, onPlace, onReset }: {
  achievements: Achievement[];
  placements: Placement[];
  onPlace: (placement: Required<Placement>) => void;
  onReset: () => void;
}) {
  const [confirmReset, setConfirmReset] = useState(false);
  const boardRef = useRef<HTMLDivElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  // The wall is built from whole bricks only, so its size snaps to the brick grid.
  const [wall, setWall] = useState({ w: 0, h: 0 });
  const [drag, setDrag] = useState<Drag | null>(null);
  const [tip, setTip] = useState<{ id: string; x: number; y: number } | null>(null);
  const [fresh, setFresh] = useState<string | null>(null);

  const byId = new Map(achievements.map((achievement) => [achievement.id, achievement]));
  const placedCount = new Map<string, number>();
  placements.forEach((placement) => placedCount.set(placement.id, (placedCount.get(placement.id) ?? 0) + 1));
  const remainingOf = (achievement: Achievement) => Math.max(0, (achievement.copies ?? 1) - (placedCount.get(achievement.id) ?? 0));
  const tray = achievements.filter((achievement) => remainingOf(achievement) > 0);

  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) return undefined;
    const measure = () => {
      const box = stage.getBoundingClientRect();
      setWall({ w: Math.max(0, Math.floor(box.width / BRICK_W) * BRICK_W), h: Math.max(0, Math.floor(box.height / BRICK_H) * BRICK_H) });
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(stage);
    return () => observer.disconnect();
  }, []);

  // While dragging, follow the pointer anywhere on the page and spray on release.
  useEffect(() => {
    if (!drag) return undefined;
    const move = (event: PointerEvent) => setDrag((current) => (current ? { ...current, pointerX: event.clientX, pointerY: event.clientY } : current));
    const up = (event: PointerEvent) => {
      const board = boardRef.current?.getBoundingClientRect();
      const inside = board && event.clientX >= board.left && event.clientX <= board.right && event.clientY >= board.top && event.clientY <= board.bottom;
      if (board && inside) {
        const spot = boardPosition(board, event.clientX, event.clientY, drag.size);
        if (isFree(board, placements, spot.px, spot.py, drag.size)) {
          const achievement = byId.get(drag.id);
          const uid = `${drag.id}-${Date.now().toString(36)}${Math.floor(Math.random() * 1296).toString(36)}`;
          onPlace({ id: drag.id, uid, x: spot.x, y: spot.y, size: drag.size, color: drag.color, rot: drag.rot, icon: achievement?.icon ?? "", name: achievement?.name ?? "", how: achievement?.how ?? "" });
          setFresh(uid);
          window.setTimeout(() => setFresh((current) => (current === uid ? null : current)), 1600);
        }
      }
      setDrag(null);
    };
    const cancel = () => setDrag(null);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", cancel);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", cancel);
    };
  }, [drag?.id, drag?.size, drag?.color, drag?.rot, placements, onPlace]);

  function startDrag(event: ReactPointerEvent, id: string) {
    event.preventDefault();
    setTip(null);
    // Each piece gets its own random size, colour and tilt at the moment you pick it up.
    const achievement = byId.get(id);
    const rarity = Math.min(1, Math.max(0, achievement?.rarity ?? 0.3));
    const size = achievement?.daily
      ? Math.round(DAILY_MIN + Math.random() * (DAILY_MAX - DAILY_MIN))
      // Harder achievements are sprayed bigger; the wide jitter keeps every piece clearly different.
      : Math.round(Math.min(MAX_SIZE, Math.max(MIN_SIZE, MIN_SIZE + rarity * (MAX_SIZE - MIN_SIZE) + (Math.random() - 0.5) * 26)));
    setDrag({
      id,
      pointerX: event.clientX,
      pointerY: event.clientY,
      size,
      color: SPRAY_COLOURS[Math.floor(Math.random() * SPRAY_COLOURS.length)],
      rot: Math.round((Math.random() - 0.5) * 28),
    });
  }

  let ghostValid = false;
  let ghostInBoard = false;
  if (drag && boardRef.current) {
    const board = boardRef.current.getBoundingClientRect();
    ghostInBoard = drag.pointerX >= board.left && drag.pointerX <= board.right && drag.pointerY >= board.top && drag.pointerY <= board.bottom;
    if (ghostInBoard) {
      const spot = boardPosition(board, drag.pointerX, drag.pointerY, drag.size);
      ghostValid = isFree(board, placements, spot.px, spot.py, drag.size);
    }
  }
  const dragged = drag ? byId.get(drag.id) : null;
  // A hung piece uses its own snapshot; the live achievement is only the fallback for older placements.
  const describe = (id: string): Achievement | null => {
    const placed = placements.find((item) => item.id === id);
    const live = byId.get(id);
    if (live) return live; // up to date (e.g. a symbol that has since become a picture)
    if (placed?.icon) return { id, icon: placed.icon, name: placed.name || "", how: placed.how || "" };
    return null;
  };
  const tipped = tip ? describe(tip.id) : null;

  return (
    <div className="wabi-ach">
      <svg width="0" height="0" aria-hidden="true" style={{ position: "absolute" }}>
        <filter id="ach-spray" x="-25%" y="-25%" width="150%" height="150%">
          <feTurbulence type="fractalNoise" baseFrequency="0.85" numOctaves="2" seed="4" result="noise" />
          <feDisplacementMap in="SourceGraphic" in2="noise" scale="4.5" />
        </filter>
      </svg>

      <div className="wabi-ach-tray" aria-label="Achievements to spray on the wall">
        {tray.length ? tray.map((achievement) => (
          <button
            key={achievement.id}
            type="button"
            className={`wabi-ach-tray-icon ${drag?.id === achievement.id ? "is-dragging" : ""}`}
            title={`${achievement.name} - drag it onto the wall`}
            onPointerDown={(event) => startDrag(event, achievement.id)}
          >
            <span>{achievement.icon}</span>
            {remainingOf(achievement) > 1 ? <sup className="wabi-ach-count">×{remainingOf(achievement)}</sup> : null}
          </button>
        )) : (
          <p className="wabi-ach-hint">{achievements.length ? "Everything you have earned is on the wall." : "Earn achievements to spray them here."}</p>
        )}
      </div>

      <div className="wabi-ach-stage" ref={stageRef}>
      <div className="wabi-ach-board" ref={boardRef} style={{ width: wall.w, height: wall.h }}>
        <BrickWall width={wall.w} height={wall.h} />
        {/* The right-most brick of the second row from the top is secretly the reset switch: nothing marks it.
            (Odd rows start half a brick in, so that row's last brick begins at half + (columns - 2) bricks.) */}
        {wall.w >= BRICK_W * 3 && wall.h >= BRICK_H * 2 ? (
          <button
            type="button"
            className="wabi-ach-secret"
            aria-label="Reset the wall"
            style={{ left: BRICK_W / 2 + (Math.floor(wall.w / BRICK_W) - 2) * BRICK_W, top: BRICK_H, width: BRICK_W, height: BRICK_H }}
            onClick={() => setConfirmReset(true)}
          />
        ) : null}
        {placements.length === 0 && !drag ? <p className="wabi-ach-empty">A clean white wall. Drag an achievement onto it.</p> : null}
        {placements.map((placement) => {
          const achievement = describe(placement.id);
          if (!achievement) return null;
          return <SprayedPiece
            key={placement.uid ?? placement.id}
            placement={placement}
            achievement={achievement}
            spraying={fresh === (placement.uid ?? placement.id)}
            onEnter={(event) => setTip({ id: placement.id, x: event.clientX, y: event.clientY })}
            onMove={(event) => setTip((current) => (current ? { ...current, x: event.clientX, y: event.clientY } : current))}
            onLeave={() => setTip(null)}
          />;
        })}
      </div>
      </div>

      {dragged && drag ? (
        <span
          className={`wabi-ach-ghost ${ghostInBoard ? (ghostValid ? "is-valid" : "is-blocked") : ""}`}
          style={{ left: drag.pointerX, top: drag.pointerY, width: drag.size, height: drag.size, "--spray": drag.color, transform: `translate(-50%, -50%) rotate(${drag.rot}deg)` } as CSSProperties}
        >
          <img className="wabi-ach-art" src={graffitiArt(dragged.icon, drag.color)} alt="" draggable={false} style={{ width: drag.size * ART_SCALE }} />
        </span>
      ) : null}

      {confirmReset ? createPortal(
        <div className="wabi-logs-backdrop" onMouseDown={() => setConfirmReset(false)}>
          <section className="wabi-ach-confirm" role="alertdialog" aria-modal="true" aria-label="Reset the wall" onMouseDown={(event) => event.stopPropagation()}>
            <h3>Reset the wall?</h3>
            <p>Every achievement is taken down, and you can hang them again.</p>
            <div className="wabi-ach-confirm-actions">
              <button type="button" className="wabi-btn-solid" onClick={() => { onReset(); setConfirmReset(false); }}>Reset</button>
              <button type="button" className="ghost-button" onClick={() => setConfirmReset(false)}>Cancel</button>
            </div>
          </section>
        </div>,
        document.body,
      ) : null}

      {tipped && tip ? (
        <div className="wabi-ach-tip" style={{ left: tip.x + 14, top: tip.y + 16 }} role="tooltip">
          <strong>{tipped.name}</strong>
          <span>{tipped.how}</span>
        </div>
      ) : null}
    </div>
  );
}

function SprayedPiece({ placement, achievement, spraying, onEnter, onMove, onLeave }: {
  placement: Placement;
  achievement: Achievement;
  spraying: boolean;
  onEnter: (event: ReactPointerEvent) => void;
  onMove: (event: ReactPointerEvent) => void;
  onLeave: () => void;
}) {
  const size = placement.size ?? SIZE;
  const color = placement.color ?? SPRAY_COLOURS[hash(placement.id) % SPRAY_COLOURS.length];
  const art = useMemo(() => graffitiArt(achievement.icon, color), [achievement.icon, color]);
  return (
    <span
      className={`wabi-ach-piece ${spraying ? "is-spraying" : ""}`}
      style={{ left: `${placement.x * 100}%`, top: `${placement.y * 100}%`, width: size, height: size, "--spray": color, "--tilt": `${placement.rot ?? 0}deg` } as CSSProperties}
      onPointerEnter={onEnter}
      onPointerMove={onMove}
      onPointerLeave={onLeave}
    >
      <img className="wabi-ach-art" src={art} alt="" draggable={false} style={{ width: size * ART_SCALE }} />
    </span>
  );
}

/**
 * A white brick wall made of whole bricks. Even rows start flush with the left edge and odd rows half a brick
 * in, so each side ends in a stagger of brick ends instead of a straight cut through them.
 */
function BrickWall({ width, height }: { width: number; height: number }) {
  const bricks = useMemo(() => {
    const list: { key: string; x: number; y: number; fill: string }[] = [];
    const rows = Math.floor(height / BRICK_H);
    const columns = Math.floor(width / BRICK_W);
    for (let row = 0; row < rows; row += 1) {
      const start = row % 2 === 0 ? 0 : BRICK_W / 2;
      const count = row % 2 === 0 ? columns : columns - 1;
      for (let column = 0; column < count; column += 1) {
        const shade = ((row * 31 + column * 17 + ((row * column) % 7)) % 6) - 3; // small, uneven tone per brick
        list.push({ key: `${row}-${column}`, x: start + column * BRICK_W, y: row * BRICK_H, fill: `rgb(${248 + shade},${246 + shade},${240 + shade})` });
      }
    }
    return list;
  }, [width, height]);
  if (!width || !height) return null;
  return (
    <svg className="wabi-ach-wall" width={width} height={height} viewBox={`0 0 ${width} ${height}`} aria-hidden="true">
      {bricks.map((brick) => (
        <rect key={brick.key} x={brick.x + 0.75} y={brick.y + 0.75} width={BRICK_W - 1.5} height={BRICK_H - 1.5} rx="1.5" fill={brick.fill} stroke="#d9d4c9" strokeWidth="1.5" />
      ))}
    </svg>
  );
}
