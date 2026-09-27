import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties, PointerEvent } from "react";
import type { AppState } from "../types";
import {
  getSkribblGallery,
  getSkribblLeaderboard,
  getSkribblTheme,
  submitSkribblDrawing,
  voteSkribblDrawing,
  SKRIBBL_DRAW_SECONDS,
} from "../lib/skribbl";
import type { SkribblDrawing, SkribblWinner } from "../lib/skribbl";

const CANVAS_W = 900;
const CANVAS_H = 600;

const PALETTE = [
  "#000000", "#37474f", "#795548", "#6d4c41", "#d84315", "#e53935", "#d81b60", "#8e24aa",
  "#5e35b1", "#3949ab", "#1e88e5", "#039be5", "#00acc1", "#00897b", "#43a047", "#7cb342",
  "#c0ca33", "#fdd835", "#ffb300", "#fb8c00", "#ffffff",
];

const BRUSH_SIZES = [3, 6, 10, 16, 26];

type Phase = "loading" | "intro" | "drawing" | "submitting" | "submitted";
type Tool = "brush" | "fill";

interface SkribblRoomProps {
  state: AppState;
  onClose: () => void;
}

function floodFill(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  fill: [number, number, number, number],
  tolerance = 40,
) {
  const { width, height } = ctx.canvas;
  const imageData = ctx.getImageData(0, 0, width, height);
  const data = imageData.data;
  const startIdx = (y * width + x) * 4;
  const target = [data[startIdx], data[startIdx + 1], data[startIdx + 2], data[startIdx + 3]];
  if (matchRgba(data[startIdx], data[startIdx + 1], data[startIdx + 2], data[startIdx + 3], fill, tolerance)) return;

  const stack: Array<[number, number]> = [[x, y]];
  while (stack.length) {
    const [px, py] = stack.pop()!;
    const base = (py * width + px) * 4;
    if (!matchRgba(data[base], data[base + 1], data[base + 2], data[base + 3], target, tolerance)) continue;

    let left = px;
    while (left > 0) {
      const i = (py * width + left - 1) * 4;
      if (!matchRgba(data[i], data[i + 1], data[i + 2], data[i + 3], target, tolerance)) break;
      left -= 1;
    }
    let right = px;
    while (right < width - 1) {
      const i = (py * width + right + 1) * 4;
      if (!matchRgba(data[i], data[i + 1], data[i + 2], data[i + 3], target, tolerance)) break;
      right += 1;
    }
    for (let cx = left; cx <= right; cx += 1) {
      const i = (py * width + cx) * 4;
      data[i] = fill[0];
      data[i + 1] = fill[1];
      data[i + 2] = fill[2];
      data[i + 3] = fill[3];
    }

    for (const dy of [-1, 1]) {
      const rowY = py + dy;
      if (rowY < 0 || rowY >= height) continue;
      let inSpan = false;
      for (let cx = left; cx <= right; cx += 1) {
        const i = (rowY * width + cx) * 4;
        const matches = matchRgba(data[i], data[i + 1], data[i + 2], data[i + 3], target, tolerance);
        if (matches && !inSpan) {
          stack.push([cx, rowY]);
          inSpan = true;
        } else if (!matches && inSpan) {
          inSpan = false;
        }
      }
    }
  }
  ctx.putImageData(imageData, 0, 0);
}

function matchRgba(r: number, g: number, b: number, a: number, target: number[], tolerance: number) {
  return Math.abs(r - target[0]) <= tolerance
    && Math.abs(g - target[1]) <= tolerance
    && Math.abs(b - target[2]) <= tolerance
    && Math.abs(a - target[3]) <= tolerance;
}

function canvasToBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (blob) return resolve(blob);
      canvas.toBlob((fallback) => {
        if (fallback) return resolve(fallback);
        reject(new Error("Could not export the drawing."));
      }, "image/png");
    }, "image/webp", 0.85);
  });
}

export default function SkribblRoom({ state, onClose }: SkribblRoomProps) {
  // eslint-disable-next-line react-hooks/exhaustive-deps -- only userId/deviceSecret identity matter here.
  const social = useMemo(() => state.social, [state.social.userId, state.social.deviceSecret]);
  const socialConfigured = Boolean(social.userId && social.deviceSecret);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const drawingRef = useRef(false);
  const lastPointRef = useRef<{ x: number; y: number } | null>(null);
  const undoStackRef = useRef<string[]>([]);
  const submittingRef = useRef(false);

  const [phase, setPhase] = useState<Phase>("loading");
  const [submitting, setSubmitting] = useState(false);
  const [undoDepth, setUndoDepth] = useState(0);
  const [theme, setTheme] = useState<string>("");
  const [themeDate, setThemeDate] = useState<string>("");
  const [submitted, setSubmitted] = useState(false);
  const [myImageUrl, setMyImageUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [color, setColor] = useState(PALETTE[0]);
  const [brushSize, setBrushSize] = useState(6);
  const [tool, setTool] = useState<Tool>("brush");
  const [timeLeft, setTimeLeft] = useState(SKRIBBL_DRAW_SECONDS);

  const [drawings, setDrawings] = useState<SkribblDrawing[]>([]);
  const [galleryHasMore, setGalleryHasMore] = useState(false);
  const [galleryOffset, setGalleryOffset] = useState(0);
  const [galleryLoading, setGalleryLoading] = useState(false);
  const [winner, setWinner] = useState<SkribblWinner | null>(null);
  const [expandedDrawing, setExpandedDrawing] = useState<SkribblDrawing | null>(null);

  const loadGallery = useCallback(async (offset: number) => {
    if (!socialConfigured) return;
    try {
      setGalleryLoading(true);
      const page = await getSkribblGallery(social, themeDate, offset);
      setDrawings((prev) => (offset === 0 ? page.drawings : [...prev, ...page.drawings]));
      setGalleryHasMore(page.hasMore);
      setGalleryOffset(page.nextOffset ?? offset);
    } catch {
      // Gallery is secondary; ignore load failures silently.
    } finally {
      setGalleryLoading(false);
    }
  }, [socialConfigured, social, themeDate]);

  const loadTheme = useCallback(async () => {
    if (!socialConfigured) return;
    setError(null);
    try {
      const data = await getSkribblTheme(social);
      setTheme(data.theme);
      setThemeDate(data.date);
      setSubmitted(data.submitted);
      setMyImageUrl(data.imageUrl);
      setPhase("intro");
      if (data.submitted) await loadGallery(0);
    } catch (requestError) {
      setTheme("");
      setSubmitted(false);
      const message = requestError instanceof Error ? requestError.message : "";
      setError(/Not found/i.test(message)
        ? "Daily Skribbl isn't live on the server yet. If this keeps happening, re-deploy the worker."
        : message || "Could not reach the Daily Skribbl server.");
      setPhase("intro");
    }
  }, [socialConfigured, social, loadGallery]);

  useEffect(() => {
    if (!socialConfigured) {
      setPhase("intro");
      return;
    }
    void loadTheme();
  }, [socialConfigured, social, loadTheme]);

  useEffect(() => {
    if (socialConfigured && themeDate) {
      void getSkribblLeaderboard(social).then((data) => setWinner(data.winner)).catch(() => undefined);
    }
  }, [socialConfigured, social, themeDate]);

  useEffect(() => {
    if (!expandedDrawing) return;
    function closeOnEscape(event: globalThis.KeyboardEvent) {
      if (event.key === "Escape") setExpandedDrawing(null);
    }
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [expandedDrawing]);

  const snapshotForUndo = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    try {
      undoStackRef.current.push(canvas.toDataURL("image/webp", 0.9));
      if (undoStackRef.current.length > 14) undoStackRef.current.shift();
      setUndoDepth(undoStackRef.current.length);
    } catch {
      // ignore snapshot failures
    }
  }, []);

  const startDrawing = useCallback(() => {
    setError(null);
    setTimeLeft(SKRIBBL_DRAW_SECONDS);
    setPhase("drawing");
    const canvas = canvasRef.current;
    if (canvas) {
      const ctx = canvas.getContext("2d");
      if (ctx) {
        ctx.fillStyle = "#ffffff";
        ctx.fillRect(0, 0, CANVAS_W, CANVAS_H);
      }
      undoStackRef.current = [];
      setUndoDepth(0);
    }
  }, []);

  const getCanvasPoint = useCallback((event: PointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return { x: 0, y: 0 };
    const rect = canvas.getBoundingClientRect();
    return {
      x: Math.min(CANVAS_W - 1, Math.max(0, Math.round((event.clientX - rect.left) * (CANVAS_W / rect.width)))),
      y: Math.min(CANVAS_H - 1, Math.max(0, Math.round((event.clientY - rect.top) * (CANVAS_H / rect.height)))),
    };
  }, []);

  const handlePointerDown = useCallback((event: PointerEvent<HTMLCanvasElement>) => {
    if (phase !== "drawing") return;
    event.preventDefault();
    (event.target as HTMLCanvasElement).setPointerCapture(event.pointerId);
    const point = getCanvasPoint(event);
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    snapshotForUndo();

    if (tool === "fill") {
      const rgba: [number, number, number, number] = [parseInt(color.slice(1, 3), 16), parseInt(color.slice(3, 5), 16), parseInt(color.slice(5, 7), 16), 255];
      floodFill(ctx, point.x, point.y, rgba);
      return;
    }

    drawingRef.current = true;
    lastPointRef.current = point;
    ctx.beginPath();
    ctx.arc(point.x, point.y, brushSize / 2, 0, Math.PI * 2);
    ctx.fillStyle = color;
    ctx.fill();
  }, [phase, getCanvasPoint, snapshotForUndo, tool, color, brushSize]);

  const handlePointerMove = useCallback((event: PointerEvent<HTMLCanvasElement>) => {
    if (!drawingRef.current || tool !== "brush" || phase !== "drawing") return;
    event.preventDefault();
    const point = getCanvasPoint(event);
    const last = lastPointRef.current;
    const canvas = canvasRef.current;
    if (!canvas || !last) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.strokeStyle = color;
    ctx.lineWidth = brushSize;
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    ctx.beginPath();
    ctx.moveTo(last.x, last.y);
    ctx.lineTo(point.x, point.y);
    ctx.stroke();
    lastPointRef.current = point;
  }, [tool, phase, getCanvasPoint, color, brushSize]);

  const handlePointerUp = useCallback(() => {
    drawingRef.current = false;
    lastPointRef.current = null;
  }, []);

  const undo = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const snapshot = undoStackRef.current.pop();
    if (!snapshot) return;
    setUndoDepth(undoStackRef.current.length);
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const image = new Image();
    image.onload = () => {
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(0, 0, CANVAS_W, CANVAS_H);
      ctx.drawImage(image, 0, 0, CANVAS_W, CANVAS_H);
    };
    image.src = snapshot;
  }, []);

  const clearCanvas = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    snapshotForUndo();
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, CANVAS_W, CANVAS_H);
  }, [snapshotForUndo]);

  const submitNow = useCallback(async () => {
    if (submittingRef.current) return;
    submittingRef.current = true;
    setSubmitting(true);
    setError(null);
    setPhase("submitting");
    try {
      const canvas = canvasRef.current;
      if (!canvas) throw new Error("Canvas is not available.");
      const blob = await canvasToBlob(canvas);
      const result = await submitSkribblDrawing(social, blob, themeDate);
      setSubmitted(true);
      setMyImageUrl(result.imageUrl);
      setPhase("submitted");
      await loadGallery(0);
    } catch (submitError) {
      setError(submitError instanceof Error ? submitError.message : "Submission failed. Try again.");
      setPhase("drawing");
    } finally {
      submittingRef.current = false;
      setSubmitting(false);
    }
  }, [social, themeDate, loadGallery]);

  useEffect(() => {
    if (phase !== "drawing") return;
    const interval = setInterval(() => {
      setTimeLeft((t) => Math.max(0, t - 1));
    }, 1000);
    return () => clearInterval(interval);
  }, [phase]);

  useEffect(() => {
    if (phase === "drawing" && timeLeft <= 0) {
      void submitNow();
    }
  }, [phase, timeLeft, submitNow]);

  const castVote = useCallback(async (drawing: SkribblDrawing, vote: -1 | 1) => {
    if (drawing.isSelf) return;
    const previous = drawing;
    const next: -1 | 0 | 1 = previous.myVote === vote ? 0 : vote;
    setDrawings((prev) => prev.map((d) => d.id === drawing.id ? {
      ...d,
      myVote: next,
      voteScore: d.voteScore - (previous.myVote ?? 0) + next,
      voteCount: Math.max(0, d.voteCount - (previous.myVote ? 1 : 0) + (next ? 1 : 0)),
    } : d));
    try {
      const result = await voteSkribblDrawing(social, drawing.id, next);
      setDrawings((prev) => prev.map((d) => d.id === drawing.id ? { ...d, voteScore: result.score, myVote: next } : d));
    } catch {
      setDrawings((prev) => prev.map((d) => d.id === drawing.id ? previous : d));
      setError("Vote could not be saved.");
    }
  }, [social]);

  const minutes = Math.floor(timeLeft / 60);
  const seconds = String(timeLeft % 60).padStart(2, "0");
  const timeDisplay = `${minutes}:${seconds}`;

  return (
    <div className="skribbl-overlay" onClick={onClose} role="presentation">
      <div className="skribbl-modal" onClick={(event) => event.stopPropagation()} role="dialog" aria-label="Daily Skribbl">
        <div className="skribbl-head">
          <div>
            <p className="skribbl-kicker">Daily Skribbl</p>
            <h2>Draw today&apos;s theme</h2>
          </div>
          <button type="button" className="skribbl-close-btn" onClick={onClose} aria-label="Close Daily Skribbl">✕</button>
        </div>

        {error ? (
          <div className="skribbl-error" role="alert">
            <p>{error}</p>
            <button type="button" className="skribbl-retry-btn" onClick={() => void loadTheme()}>Try again</button>
          </div>
        ) : null}

        {!socialConfigured ? (
          <div className="skribbl-empty">
            <p>Connect the Social tab once to enable Daily Skribbl, then come back and draw.</p>
          </div>
        ) : phase === "loading" ? (
          <div className="skribbl-empty">Loading today&apos;s theme…</div>
        ) : (
          <div className="skribbl-body">
            <div className="skribbl-theme-card">
              <span className="skribbl-theme-label">Today&apos;s theme</span>
              <strong>{theme}</strong>
            </div>

            {phase === "intro" ? (
              <div className="skribbl-intro">
                {submitted ? (
                  <>
                    <p className="skribbl-intro-note">You already submitted a drawing today. Vote on the gallery below.</p>
                    {myImageUrl ? <img className="skribbl-thumb" src={myImageUrl} alt="Your submitted drawing" /> : null}
                  </>
                ) : (
                  <>
                    <p className="skribbl-intro-note">You have {SKRIBBL_DRAW_SECONDS / 60} minutes. Draw, then submit — everyone with the same theme will vote.</p>
                    <button type="button" className="skribbl-start-btn" onClick={startDrawing}>Start Drawing</button>
                  </>
                )}
              </div>
            ) : phase === "drawing" ? (
              <div className="skribbl-draw-area">
                <div className="skribbl-toolbar">
                  <div className="skribbl-tool-group" aria-label="Tool">
                    <button type="button" className={`skribbl-tool-btn ${tool === "brush" ? "active" : ""}`} onClick={() => setTool("brush")} aria-pressed={tool === "brush"}>✏️ Brush</button>
                    <button type="button" className={`skribbl-tool-btn ${tool === "fill" ? "active" : ""}`} onClick={() => setTool("fill")} aria-pressed={tool === "fill"}>🪣 Fill</button>
                  </div>
                  <div className="skribbl-tool-group" aria-label="Brush size">
                    {BRUSH_SIZES.map((size) => (
                      <button key={size} type="button" className={`skribbl-size-btn ${brushSize === size ? "active" : ""}`} onClick={() => setBrushSize(size)} aria-label={`Brush size ${size}`} style={{ "--dot": `${size}px` } as CSSProperties} />
                    ))}
                  </div>
                  <div className="skribbl-tool-group" aria-label="Actions">
                    <button type="button" className="skribbl-tool-btn" onClick={undo} disabled={!undoDepth}>Undo</button>
                    <button type="button" className="skribbl-tool-btn" onClick={clearCanvas}>Clear</button>
                  </div>
                </div>

                <div className="skribbl-color-row" aria-label="Colors">
                  {PALETTE.map((paletteColor) => (
                    <button key={paletteColor} type="button" className={`skribbl-color-btn ${color === paletteColor ? "active" : ""}`} style={{ background: paletteColor }} onClick={() => setColor(paletteColor)} aria-label={`Color ${paletteColor}`} aria-pressed={color === paletteColor} />
                  ))}
                  <label className="skribbl-custom-color" title="Custom color">
                    <input type="color" value={color} onChange={(event) => setColor(event.target.value)} aria-label="Custom color" />
                    <span style={{ background: color }} />
                  </label>
                </div>

                <div className="skribbl-canvas-wrap" data-low={timeLeft <= 30}>
                  <canvas
                    ref={canvasRef}
                    width={CANVAS_W}
                    height={CANVAS_H}
                    className="skribbl-canvas"
                    onPointerDown={handlePointerDown}
                    onPointerMove={handlePointerMove}
                    onPointerUp={handlePointerUp}
                    onPointerLeave={handlePointerUp}
                  />
                  <span className="skribbl-timer" aria-live="polite">{timeDisplay}</span>
                </div>

                <button type="button" className="skribbl-submit-btn" onClick={() => void submitNow()} disabled={submitting}>
                  {submitting ? "Submitting…" : "Submit drawing"}
                </button>
              </div>
            ) : phase === "submitting" ? (
              <div className="skribbl-empty">Submitting your drawing…</div>
            ) : (
              <div className="skribbl-intro">
                <p className="skribbl-intro-note">Submitted! Your drawing is live below. Vote on the gallery.</p>
                {myImageUrl ? <img className="skribbl-thumb" src={myImageUrl} alt="Your submitted drawing" /> : null}
              </div>
            )}

            {submitted ? (
              <div className="skribbl-gallery">
                <div className="skribbl-gallery-head">
                  <h3>Gallery</h3>
                  <span className="skribbl-gallery-count">{drawings.length} drawing{drawings.length === 1 ? "" : "s"}</span>
                  <button type="button" className="skribbl-refresh-btn" onClick={() => void loadGallery(0)} disabled={galleryLoading} aria-label="Refresh gallery" title="Refresh gallery">
                    <span className={galleryLoading ? "skribbl-refresh-spin" : ""}>{'↻'}</span>
                  </button>
                </div>
              {drawings.length ? (
                <div className="skribbl-gallery-grid">
                  {drawings.map((drawing) => (
                    <div key={drawing.id} className="skribbl-drawing-card">
                      <button type="button" className="skribbl-drawing-thumb-btn" onClick={() => setExpandedDrawing(drawing)} aria-label={`Expand drawing by ${drawing.isSelf ? "you" : drawing.displayName}`}>
                        <img src={drawing.imageUrl} alt={`Drawing by ${drawing.displayName}`} loading="lazy" />
                      </button>
                      <div className="skribbl-drawing-meta">
                        <span className="skribbl-drawing-name">{drawing.isSelf ? "You" : drawing.displayName}</span>
                        <div className="skribbl-vote-row">
                          <button type="button" className={`skribbl-vote-btn ${drawing.myVote === 1 ? "up active" : "up"}`} onClick={() => void castVote(drawing, 1)} disabled={drawing.isSelf} aria-label="Upvote">👍</button>
                          <span className="skribbl-vote-score">{drawing.voteScore}</span>
                          <button type="button" className={`skribbl-vote-btn ${drawing.myVote === -1 ? "down active" : "down"}`} onClick={() => void castVote(drawing, -1)} disabled={drawing.isSelf} aria-label="Downvote">👎</button>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="skribbl-gallery-empty">No drawings yet today — be the first.</div>
              )}
              {galleryHasMore ? (
                <button type="button" className="skribbl-more-btn" onClick={() => void loadGallery(galleryOffset)} disabled={galleryLoading}>
                  {galleryLoading ? "Loading…" : "Load more"}
                </button>
              ) : null}
              </div>
            ) : (
              <p className="skribbl-intro-note">The gallery unlocks after you submit your drawing today.</p>
            )}

            <div className="skribbl-leaderboard">
              <div className="skribbl-gallery-head">
                <h3>Yesterday&apos;s Winner</h3>
              </div>
              {winner ? (
                <div className="skribbl-winner-card">
                  <span className="skribbl-winner-trophy">{'\u{1F3C6}'}</span>
                  <div>
                    <strong>{winner.displayName}</strong>
                    <span className="skribbl-winner-score">{winner.score} point{winner.score === 1 ? "" : "s"}</span>
                  </div>
                </div>
              ) : (
                <div className="skribbl-gallery-empty">No winner yet — the first theme&apos;s winner appears tomorrow.</div>
              )}
            </div>
          </div>
        )}

        {expandedDrawing ? (
          <div className="skribbl-lightbox" onClick={() => setExpandedDrawing(null)} role="presentation">
            <div className="skribbl-lightbox-frame">
              <img src={expandedDrawing.imageUrl} alt={`Drawing by ${expandedDrawing.isSelf ? "you" : expandedDrawing.displayName}`} />
            </div>
          </div>
        ) : null}
      </div>
    </div>
  );
}
