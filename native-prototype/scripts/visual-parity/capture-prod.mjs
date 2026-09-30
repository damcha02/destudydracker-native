// Stage 17: renders the PRODUCTION Dashboard (a scratch build of desktop/, never the installed
// app, never a real profile) in headless Chrome/Edge against a synthetic backup fixture, and
// writes a PNG (+ optionally the result of an in-page probe script).
//
//   node capture-prod.mjs --dist <prod-dist> --fixture <backup.json> --out <shot.png>
//        [--w 1920 --h 1080] [--layout quiet|full] [--theme dark|light] [--tab dashboard|timer]
//        [--eval probe.js --eval-out probe.json] [--browser <exe>]
//        [--now 2026-09-30T12:00:00+02:00] [--tz Europe/Zurich] [--dpr 1.25]
//
// Isolation: a throw-away --user-data-dir (fresh, deleted afterwards) means production's own
// localStorage/app-data are never read or written; the fixture is injected into that temp
// profile's localStorage via Page.addScriptToEvaluateOnNewDocument. desktop/ is only read (the
// dist was built with `vite build --outDir <scratch>`).
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { readFileSync, writeFileSync, existsSync, mkdtempSync, rmSync, statSync } from "node:fs";
import { join, extname } from "node:path";
import { tmpdir } from "node:os";

const args = Object.fromEntries(process.argv.slice(2).reduce((acc, a, i, all) => (a.startsWith("--") ? [...acc, [a.slice(2), all[i + 1]]] : acc), []));
const W = Number(args.w ?? 1920), H = Number(args.h ?? 1080);
const layout = args.layout ?? "quiet", theme = args.theme ?? "dark";
const browser = args.browser ?? ["C:/Program Files/Google/Chrome/Application/chrome.exe", "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"].find(existsSync);
const dist = args.dist;
const backup = JSON.parse(readFileSync(args.fixture, "utf8"));
const { social, timer, ...core } = backup.state;

const mime = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".png": "image/png", ".mp3": "audio/mpeg", ".json": "application/json", ".woff2": "font/woff2" };
const server = createServer((req, res) => {
  let p = decodeURIComponent(req.url.split("?")[0]);
  if (p === "/") p = "/index.html";
  const f = join(dist, p);
  if (!existsSync(f) || !statSync(f).isFile()) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { "content-type": mime[extname(f)] ?? "application/octet-stream" });
  res.end(readFileSync(f));
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const appUrl = `http://127.0.0.1:${server.address().port}/`;

const profile = mkdtempSync(join(tmpdir(), "st-parity-"));
const port = 9300 + Math.floor(Math.random() * 500);
const chrome = spawn(browser, [
  "--headless=new", `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`,
  `--window-size=${W},${H}`, "--force-device-scale-factor=1", "--hide-scrollbars",
  "--no-first-run", "--no-default-browser-check", "--disable-extensions", "--disable-background-networking",
  "about:blank",
], { stdio: "ignore" });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let targets;
for (let i = 0; i < 60; i++) {
  try { targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json(); if (targets.some((t) => t.type === "page")) break; } catch { /* not up yet */ }
  await sleep(250);
}
const page = targets.find((t) => t.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
let nextId = 1;
const pending = new Map();
ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && pending.has(d.id)) { pending.get(d.id)(d); pending.delete(d.id); } };
const send = (method, params = {}) => new Promise((res) => { const id = nextId++; pending.set(id, res); ws.send(JSON.stringify({ id, method, params })); });

const seed = `(() => { try {
  if (localStorage.getItem('__seeded')) return;
  localStorage.setItem('study-tracker-desktop-v3-core', ${JSON.stringify(JSON.stringify(core))});
  localStorage.setItem('study-tracker-desktop-v3-social', ${JSON.stringify(JSON.stringify({ social }))});
  localStorage.setItem('study-tracker-style', 'field-notebook');
  localStorage.setItem('study-tracker-field-dashboard-layout', ${JSON.stringify(layout)});
  localStorage.setItem('study-tracker-theme', ${JSON.stringify(theme)});
  localStorage.setItem('study-tracker-welcome-seen', '1');
  localStorage.setItem('__seeded', '1');
} catch (e) {} })();`;
await send("Page.enable");
await send("Runtime.enable");
// The packaged Tauri app's CSP (desktop/src-tauri/tauri.conf.json: font-src 'self' data:,
// connect-src limited) blocks Google Fonts and never reaches the update/announcement hosts in a
// browser preview. Reproduce that: without this, headless Chrome would fetch Newsreader/Courier
// Prime from the network and render fonts the shipped app never shows, plus an "update available"
// toast that is a preview artifact. --fonts online opts back in.
await send("Network.enable");
await send("Network.setBlockedURLs", { urls: [...(args.fonts === "online" ? [] : ["*fonts.googleapis.com*", "*fonts.gstatic.com*"]), "*github.com*", "*workers.dev*"] });
await send("Page.addScriptToEvaluateOnNewDocument", { source: seed });
await send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: Number(args.dpr ?? 1), mobile: false });
if (args.tz) await send("Emulation.setTimezoneOverride", { timezoneId: args.tz });
if (args.now) {
  // Freeze "now" so the synthetic fixture and the native app agree on what today is.
  const fixed = Date.parse(args.now);
  await send("Page.addScriptToEvaluateOnNewDocument", { source: `(() => { const R = Date; const t0 = ${fixed}; class F extends R { constructor(...a) { if (a.length === 0) super(t0); else super(...a); } static now() { return t0; } } window.Date = F; })();` });
}
await send("Page.navigate", { url: appUrl });
for (let i = 0; i < 60; i++) {
  const r = await send("Runtime.evaluate", { expression: "!!document.querySelector('.fn-dashboard, .dashboard-design, .timer-grid, .fn-timer')", returnByValue: true });
  if (r.result?.result?.value) break;
  await sleep(250);
}
await sleep(800);
// Close any first-run modal that would cover the Dashboard (Escape is what production binds).
await send("Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
await sleep(400);
if (args.tab && args.tab !== "dashboard") {
  await send("Runtime.evaluate", { expression: `[...document.querySelectorAll('.tab-button')].find(b => b.textContent.trim().toLowerCase().startsWith(${JSON.stringify(args.tab)}))?.click()` });
  await sleep(600);
}
if (args.eval) {
  const r = await send("Runtime.evaluate", { expression: readFileSync(args.eval, "utf8"), returnByValue: true, awaitPromise: true });
  writeFileSync(args["eval-out"] ?? "probe.json", JSON.stringify(r.result?.result?.value ?? r, null, 2));
}
if (args.out) {
  const shot = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  writeFileSync(args.out, Buffer.from(shot.result.data, "base64"));
}
ws.close();
chrome.kill();
server.close();
await sleep(500);
try { rmSync(profile, { recursive: true, force: true }); } catch { /* chrome may still hold a handle; temp dir */ }
console.log(`captured ${args.out ?? "(no png)"} ${W}x${H} layout=${layout} theme=${theme}`);
process.exit(0);
