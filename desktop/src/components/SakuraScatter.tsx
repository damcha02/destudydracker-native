import { useMemo } from "react";

const BLOSSOMS = [
  "/196-1963264_cherry-blossom-free-icon-flowey-i-am-not-cute.png",
  "/flower-flowers-sakura-cherryblossom-tumblr-kawaii-ftest-sakura-flower-png-kawaii-115628518804pkqn9p63w.png",
];

type ScatterDot = {
  src: string;
  left: number;
  size: number;
  opacity: number;
  drift: number;
  spin: number;
  fallDuration: number;
  delay: number;
  swayDuration: number;
};

function mulberry32(seed: number) {
  return function () {
    let t = (seed += 0x6d2b79f5);
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export function SakuraScatter() {
  const dots = useMemo<ScatterDot[]>(() => {
    const rnd = mulberry32(0x534b5552);
    const out: ScatterDot[] = [];
    const count = 22;
    for (let i = 0; i < count; i += 1) {
      out.push({
        src: BLOSSOMS[i % BLOSSOMS.length],
        left: rnd() * 100,
        size: 16 + rnd() * 20,
        opacity: 0.22 + rnd() * 0.2,
        drift: (rnd() - 0.5) * 160,
        spin: (rnd() > 0.5 ? 1 : -1) * (180 + rnd() * 360),
        fallDuration: 14 + rnd() * 14,
        delay: -rnd() * 28,
        swayDuration: 3 + rnd() * 3,
      });
    }
    return out;
  }, []);

  return (
    <div
      aria-hidden="true"
      className="sakura-scatter"
      style={{
        position: "fixed",
        inset: 0,
        pointerEvents: "none",
        zIndex: 40,
        overflow: "hidden",
      }}
    >
      {dots.map((d, idx) => (
        <div
          key={idx}
          className="sakura-petal"
          style={{
            left: `${d.left}%`,
            animationDuration: `${d.fallDuration}s`,
            animationDelay: `${d.delay}s`,
            ["--petal-drift" as string]: `${d.drift}px`,
            ["--petal-spin" as string]: `${d.spin}deg`,
            ["--petal-opacity" as string]: d.opacity,
          }}
        >
          <img
            src={d.src}
            alt=""
            draggable={false}
            style={{
              width: `${d.size}px`,
              height: `${d.size}px`,
              animationDuration: `${d.swayDuration}s`,
            }}
          />
        </div>
      ))}
    </div>
  );
}
