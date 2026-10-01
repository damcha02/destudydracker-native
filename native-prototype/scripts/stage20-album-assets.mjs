// Stage 20: derives the achievements album's static decor from PRODUCTION's own CSS.
//
// The album (`InkGallery` variant "book", desktop/src/features/rest/InkGallery.tsx + App.css
// `.wabi-book-*`) is painted entirely with CSS: ~10 stacked gradients for the desk (repeating,
// elliptical and conic gradients, a blurred masked shoji light), layered conic/radial gradients
// for the ink dish and the brush, cloth gradients and inset shadows for the boards and cover,
// paper gradients for the pages. Slint has no repeating/conic/elliptical gradients, masks or blur,
// so this script renders each of those static layers in headless Chromium from a scratch build of
// desktop/ (never the installed app, never a real profile) and saves it as an image:
//
//   assets/break/album/<theme>/desk.jpg      the desk + shoji light (opaque)
//   assets/break/album/<theme>/cup.png       ink dish incl. its drop shadow (pad PAD px)
//   assets/break/album/<theme>/pen.png       brush incl. its drop shadow (pad PAD px)
//   assets/break/album/<theme>/cover.jpg     cover front: cloth, bevel, crease, thread, studs
//                                            (the title slip is drawn natively)
//   assets/break/album/<theme>/edge.png      the closed book's paper/board edges (pad PAD px)
//   assets/break/album/<theme>/board-left.jpg / board-right.jpg   open boards (JPEG, 1.5x) (cloth, glints)
//   assets/break/album/<theme>/page-left.jpg  / page-right.jpg    page paper (JPEG, 1.5x) incl. gutter shade
//
// Everything is captured with the book stage's -0.8deg rotation removed (native rotates the whole
// stage itself) at device scale 2, from a 1520x980 window (scene 1220x926); native scales them to
// the book geometry it computes with production's own `computeBookGeometry`.
//
//   node scripts/stage20-album-assets.mjs --dist <prod-dist> --fixture <backup.json> [--browser chromium]
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { readFileSync, writeFileSync, existsSync, mkdtempSync, rmSync, statSync, mkdirSync } from "node:fs";
import { join, extname, dirname, relative } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const args = Object.fromEntries(process.argv.slice(2).reduce((acc, a, i, all) => (a.startsWith("--") ? [...acc, [a.slice(2), all[i + 1]]] : acc), []));
const browser = args.browser ?? ["/usr/bin/chromium", "C:/Program Files/Google/Chrome/Application/chrome.exe"].find(existsSync);
const W = 1520, H = 980, DPR = 2, PAD = 10;
const outRoot = join(here, "..", "assets", "break", "album");
const backup = JSON.parse(readFileSync(args.fixture, "utf8"));
const { social, timer, ...core } = backup.state;

const mime = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".png": "image/png", ".woff2": "font/woff2", ".json": "application/json", ".mp3": "audio/mpeg", ".gif": "image/gif" };
const server = createServer((req, res) => {
  let p = decodeURIComponent(req.url.split("?")[0]);
  if (p === "/") p = "/index.html";
  const f = join(args.dist, p);
  if (!existsSync(f) || !statSync(f).isFile()) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { "content-type": mime[extname(f)] ?? "application/octet-stream" });
  res.end(readFileSync(f));
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function session(theme) {
  const profile = mkdtempSync(join(tmpdir(), "st-album-"));
  const port = 9800 + Math.floor(Math.random() * 150);
  const chrome = spawn(browser, ["--headless=new", `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, `--window-size=${W},${H}`, "--hide-scrollbars", "--no-first-run", "--disable-extensions", "about:blank"], { stdio: "ignore" });
  let targets;
  for (let i = 0; i < 60; i++) {
    try { targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json(); if (targets.some((t) => t.type === "page")) break; } catch { /* not up */ }
    await sleep(250);
  }
  const ws = new WebSocket(targets.find((t) => t.type === "page").webSocketDebuggerUrl);
  await new Promise((r) => (ws.onopen = r));
  let id = 1;
  const pending = new Map();
  ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && pending.has(d.id)) { pending.get(d.id)(d); pending.delete(d.id); } };
  const send = (method, params = {}) => new Promise((res) => { const i = id++; pending.set(i, res); ws.send(JSON.stringify({ id: i, method, params })); });
  const evaluate = async (expression) => (await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true })).result?.result?.value;
  const seed = `(() => { if (localStorage.getItem('__seeded')) return;
    localStorage.setItem('study-tracker-desktop-v3-core', ${JSON.stringify(JSON.stringify(core))});
    localStorage.setItem('study-tracker-desktop-v3-social', ${JSON.stringify(JSON.stringify({ social }))});
    localStorage.setItem('study-tracker-style', 'wabi-sabi');
    localStorage.setItem('study-tracker-theme', ${JSON.stringify(theme)});
    localStorage.setItem('study-tracker-welcome-seen', '1');
    localStorage.setItem('__seeded', '1'); })();`;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Network.enable");
  await send("Network.setBlockedURLs", { urls: ["*fonts.googleapis.com*", "*fonts.gstatic.com*", "*github.com*", "*workers.dev*"] });
  await send("Page.addScriptToEvaluateOnNewDocument", { source: seed });
  await send("Page.addScriptToEvaluateOnNewDocument", { source: "Math.random = () => 0.25;" });
  await send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: DPR, mobile: false });
  await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-reduced-motion", value: "reduce" }] });
  await send("Page.navigate", { url: `http://127.0.0.1:${server.address().port}/` });
  for (let i = 0; i < 60 && !(await evaluate("!!document.querySelector('.wabi-nav-item')")); i++) await sleep(250);
  await sleep(600);
  await send("Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
  await evaluate(`[...document.querySelectorAll('.wabi-nav-item')].find(b => b.querySelector('.wabi-nav-sub')?.textContent.trim() === 'break')?.click()`);
  await sleep(400);
  await evaluate(`[...document.querySelectorAll('button')].find(b => b.textContent.trim() === 'Achievements')?.click()`);
  await sleep(1200);
  await evaluate(`(() => { const s = document.createElement('style'); s.id = 'st-capture'; s.textContent = '.wabi-book-stage { transform: none !important; } html, body { background: transparent !important; }'; document.head.appendChild(s); })()`);
  await sleep(200);
  return { send, evaluate, close: () => { ws.close(); chrome.kill(); rmSync(profile, { recursive: true, force: true }); } };
}

// Shows only `selector` (and its subtree), optionally hiding descendants matching `hide`;
// returns its viewport rect.
const isolate = (selector, hide = "") => `(() => {
  document.querySelectorAll('body *').forEach((e) => { e.style.visibility = 'hidden'; });
  const t = document.querySelector(${JSON.stringify(selector)});
  if (!t) return null;
  t.style.visibility = 'visible';
  t.querySelectorAll('*').forEach((e) => { e.style.visibility = 'visible'; });
  ${hide ? `t.querySelectorAll(${JSON.stringify(hide)}).forEach((e) => { e.style.visibility = 'hidden'; e.querySelectorAll('*').forEach((c) => { c.style.visibility = 'hidden'; }); });` : ""}
  const r = t.getBoundingClientRect();
  return { x: r.x, y: r.y, w: r.width, h: r.height };
})()`;

async function capture(s, file, rect, pad, format, scale = 1) {
  await s.send("Emulation.setDefaultBackgroundColorOverride", { color: { r: 0, g: 0, b: 0, a: format === "jpeg" ? 1 : 0 } });
  const clip = { x: rect.x - pad, y: rect.y - pad, width: rect.w + 2 * pad, height: rect.h + 2 * pad, scale };
  const shot = await s.send("Page.captureScreenshot", { format, ...(format === "jpeg" ? { quality: 88 } : {}), clip, captureBeyondViewport: false });
  writeFileSync(file, Buffer.from(shot.result.data, "base64"));
  return { file, css: [rect.x, rect.y, rect.w, rect.h].map((n) => Math.round(n * 100) / 100), pad };
}

const report = {};
for (const theme of ["light", "dark"]) {
  const dir = join(outRoot, theme);
  mkdirSync(dir, { recursive: true });
  const s = await session(theme);
  const rows = [];
  const grab = async (name, selector, hide, pad, format = "png", scale = 1) => {
    const rect = await s.evaluate(isolate(selector, hide));
    if (!rect) throw new Error(`${theme}: ${selector} not found`);
    await sleep(400);
    rows.push({ name, ...(await capture(s, join(dir, name), rect, pad, format, scale)) });
  };
  // closed book
  const scene = await s.evaluate(isolate(".wabi-book-scene", ".wabi-book-cup, .wabi-book-pen, .wabi-book-stage"));
  await sleep(150);
  rows.push({ name: "desk.jpg", ...(await capture(s, join(dir, "desk.jpg"), scene, 0, "jpeg")) });
  await grab("cup.png", ".wabi-book-cup", "", PAD);
  await grab("pen.png", ".wabi-book-pen", "", PAD);
  await grab("cover.jpg", ".wabi-book-cover-face", ".wabi-book-slip", 0, "jpeg", 0.75);
  await grab("edge.png", ".wabi-book-closededge", "", PAD);
  // open book: everything visible again, open it, wait for the turn to settle
  await s.evaluate(`document.querySelectorAll('body *').forEach((e) => { e.style.visibility = ''; }); document.querySelector('.wabi-book-closededge')?.click();`);
  await sleep(1500);
  await grab("board-left.jpg", ".wabi-book-board--left", "", 0, "jpeg", 0.75);
  await grab("board-right.jpg", ".wabi-book-board--right", "", 0, "jpeg", 0.75);
  await grab("page-left.jpg", ".wabi-album-page--left", ".wabi-album-rows, .wabi-album-empty, .wabi-album-pageno", 0, "jpeg", 0.75);
  await grab("page-right.jpg", ".wabi-album-page--right", ".wabi-album-rows, .wabi-album-empty, .wabi-album-pageno", 0, "jpeg", 0.75);
  report[theme] = { scene, rows };
  s.close();
}
server.close();
for (const [theme, r] of Object.entries(report)) for (const row of r.rows) console.log(theme, row.name, JSON.stringify(row.css), statSync(row.file).size);
// the committed report keeps paths relative to the crate root (no machine-specific prefix)
const rel = (f) => relative(join(here, ".."), f);
for (const r of Object.values(report)) for (const row of r.rows) row.file = rel(row.file);
writeFileSync(join(outRoot, "capture-report.json"), JSON.stringify(report, null, 2) + "\n");
process.exit(0);
