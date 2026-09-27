import { useLayoutEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import type { Course, StudySession } from "../../types";

/** Flat, minimal takes on the knowledge garden. Cycle through them to pick a favourite. */
export type ModernGardenVariant = "terraces" | "stems";

export const MODERN_GARDEN_VARIANTS: { id: ModernGardenVariant; name: string }[] = [
  { id: "terraces", name: "Terraces" },
  { id: "stems", name: "Stems" },
];

const STAGE_NAMES = ["Dormant", "Sprouting", "Growing", "Leafy", "Blooming", "Flourishing"];
const STAGE_THRESHOLDS = [0, 30, 90, 210, 420, 720];

type CourseLoad = { id: string; name: string; color: string; minutes: number };

const tint = (color: string, pct: number, base = "var(--garden)") => `color-mix(in oklab, ${color} ${pct}%, ${base})`;

function fmt(minutes: number) {
  const h = Math.floor(minutes / 60);
  const m = Math.round(minutes % 60);
  return h ? `${h}h ${m}m` : `${m}m`;
}

/** Smooth rolling hill, closed down to the bottom edge. */
function hillY(x: number, width: number, base: number, amp: number, freq: number, phase: number) {
  return base + amp * Math.sin((x / width) * Math.PI * 2 * freq + phase);
}

function hillPath(width: number, height: number, base: number, amp: number, freq: number, phase: number) {
  let d = `M0 ${height} L0 ${hillY(0, width, base, amp, freq, phase).toFixed(1)}`;
  for (let x = 8; x <= width + 8; x += 8) d += ` L${x} ${hillY(x, width, base, amp, freq, phase).toFixed(1)}`;
  return `${d} L${width + 8} ${height} Z`;
}

function Sway({ delay, children }: { delay: number; children: ReactNode }) {
  return <g className="mg-sway" style={{ animationDelay: `${-delay}s` } as CSSProperties}>{children}</g>;
}

function Bloom({ cx, cy, r, color }: { cx: number; cy: number; r: number; color: string }) {
  return (
    <g>
      {Array.from({ length: 6 }).map((_, index) => (
        <ellipse key={index} cx={cx} cy={cy - r * 0.62} rx={r * 0.38} ry={r * 0.66} fill={color} opacity="0.95" transform={`rotate(${index * 60} ${cx} ${cy})`} />
      ))}
      <circle cx={cx} cy={cy} r={r * 0.3} fill="var(--warn)" />
    </g>
  );
}

function Terraces({ w, h, loads }: { w: number; h: number; loads: CourseLoad[] }) {
  const ground = h * 0.64;
  const maxMinutes = Math.max(1, ...loads.map((load) => load.minutes));
  const maxRadius = Math.max(12, Math.min(26, (ground - 34) / 2.3));
  const step = w / (loads.length + 1);
  return (
    <>
      <circle cx={w - 44} cy={38} r={15} fill="var(--warn)" opacity="0.5" />
      <path d={hillPath(w, h, ground - 26, 9, 1.1, 0.4)} fill={tint("var(--garden)", 100, "var(--garden-sky)")} opacity="0.16" />
      <path d={hillPath(w, h, ground - 8, 8, 1.4, 2.2)} fill={tint("var(--garden)", 100, "var(--garden-sky)")} opacity="0.3" />
      {loads.map((load, index) => {
        const x = step * (index + 1);
        const radius = 7 + (maxRadius - 7) * Math.sqrt(load.minutes / maxMinutes);
        const y = hillY(x, w, ground - 8, 8, 1.4, 2.2) + 6;
        const trunk = radius * 0.9 + 6;
        const crown = tint(load.color, 42);
        return (
          <Sway key={load.id} delay={index * 0.7}>
            <title>{`${load.name} · ${fmt(load.minutes)}`}</title>
            <ellipse cx={x} cy={y + 1} rx={radius * 0.8} ry={3} fill="var(--garden-deep)" opacity="0.25" />
            <rect x={x - 1.5} y={y - trunk} width="3" height={trunk} rx="1.5" fill="var(--ink-4)" opacity="0.8" />
            <circle cx={x} cy={y - trunk - radius * 0.6} r={radius} fill={crown} />
            <circle cx={x - radius * 0.32} cy={y - trunk - radius * 0.92} r={radius * 0.5} fill="white" opacity="0.14" />
          </Sway>
        );
      })}
      <path d={hillPath(w, h, ground + 14, 7, 0.9, 4)} fill={tint("var(--garden-soil)", 55, "var(--garden-deep)")} opacity="0.55" />
    </>
  );
}

function Stems({ w, h, loads }: { w: number; h: number; loads: CourseLoad[] }) {
  const base = h - 62;
  const top = 34;
  const maxMinutes = Math.max(1, ...loads.map((load) => load.minutes));
  const step = w / (loads.length + 1);
  return (
    <>
      {[0.33, 0.66, 1].map((level) => (
        <line key={level} x1="18" x2={w - 18} y1={base - (base - top) * level} y2={base - (base - top) * level} stroke="var(--line-soft)" strokeDasharray="2 5" />
      ))}
      <line x1="18" x2={w - 18} y1={base} y2={base} stroke="var(--line-strong)" strokeWidth="1.5" strokeLinecap="round" />
      {loads.map((load, index) => {
        const x = step * (index + 1);
        const height = 22 + (base - top - 22) * (load.minutes / maxMinutes);
        const y = base - height;
        const color = tint(load.color, 70, "var(--garden)");
        const bloomR = 8 + 5 * (load.minutes / maxMinutes);
        return (
          <g key={load.id}>
            <title>{`${load.name} · ${fmt(load.minutes)}`}</title>
            <Sway delay={index * 0.9}>
              <path d={`M${x} ${base} Q ${x + 4} ${base - height / 2} ${x} ${y}`} stroke="var(--garden-deep)" strokeWidth="2" fill="none" strokeLinecap="round" />
              <ellipse cx={x - 7} cy={base - height * 0.36} rx="6.5" ry="2.8" fill="var(--garden)" transform={`rotate(-28 ${x - 7} ${base - height * 0.36})`} />
              <ellipse cx={x + 8} cy={base - height * 0.55} rx="6.5" ry="2.8" fill="var(--garden)" transform={`rotate(28 ${x + 8} ${base - height * 0.55})`} />
              <Bloom cx={x} cy={y} r={bloomR} color={color} />
            </Sway>
            <text x={x} y={base + 15} textAnchor="middle" className="mg-label">{load.name.length > 10 ? `${load.name.slice(0, 9)}…` : load.name}</text>
          </g>
        );
      })}
    </>
  );
}

export function ModernGarden({ variant, courses, sessions, weeklyMinutes, streak }: {
  variant: ModernGardenVariant;
  courses: Course[];
  sessions: StudySession[];
  weeklyMinutes: number;
  streak: number;
}) {
  const wrapRef = useRef<HTMLDivElement | null>(null);
  const [size, setSize] = useState({ w: 380, h: 220 });
  const stage = Math.max(0, STAGE_THRESHOLDS.filter((threshold) => weeklyMinutes >= threshold).length - 1);

  useLayoutEffect(() => {
    const element = wrapRef.current;
    if (!element || typeof ResizeObserver === "undefined") return undefined;
    const observer = new ResizeObserver(() => {
      const w = Math.round(element.clientWidth);
      const h = Math.round(element.clientHeight);
      setSize((prev) => (prev.w === w && prev.h === h ? prev : { w, h }));
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const loads = useMemo(() => {
    const cutoff = new Date();
    cutoff.setHours(0, 0, 0, 0);
    cutoff.setDate(cutoff.getDate() - 6);
    const perCourse = new Map<string, number>();
    for (const session of sessions) {
      if (!session.courseId || new Date(session.startedAt) < cutoff) continue;
      perCourse.set(session.courseId, (perCourse.get(session.courseId) ?? 0) + session.minutes);
    }
    const courseLoads: CourseLoad[] = courses
      .map((course) => ({ id: course.id, name: course.name, color: course.color, minutes: perCourse.get(course.id) ?? 0 }))
      .filter((load) => load.minutes > 0)
      .sort((a, b) => b.minutes - a.minutes)
      .slice(0, 8);
    return courseLoads;
  }, [courses, sessions]);

  const fit = Math.max(2, Math.floor(size.w / 64));
  const shown = loads.slice(0, fit);
  const empty = shown.length === 0;

  return (
    <div ref={wrapRef} className={`garden-stage mg-stage mg-${variant}`} aria-label={`Knowledge Garden, ${MODERN_GARDEN_VARIANTS.find((v) => v.id === variant)?.name}: ${STAGE_NAMES[stage]}, ${fmt(weeklyMinutes)} last 7 days`}>
      <svg className="mg-svg" width={size.w} height={size.h} viewBox={`0 0 ${size.w} ${size.h}`} aria-hidden="true">
        {variant === "terraces" ? <Terraces w={size.w} h={size.h} loads={shown} /> : null}
        {variant === "stems" ? <Stems w={size.w} h={size.h} loads={shown} /> : null}
      </svg>
      <div className="gk-stage-dots" aria-hidden="true">{Array.from({ length: 5 }).map((_, index) => <span key={index} className={index < stage ? "active" : ""} />)}</div>
      <div className="gk-stats"><span className="serif">{STAGE_NAMES[stage]}</span><span className="mono">{fmt(weeklyMinutes)} last 7 days{streak >= 3 ? ` · ${streak}-day streak` : ""}</span></div>
      {empty ? <p className="gk-empty mono">log a session to plant your first sprout</p> : null}
    </div>
  );
}
