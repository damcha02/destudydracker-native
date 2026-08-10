/* ============================================================
   Study Tracker — ambient background effects
   Ports the Odysseus per-theme "background pattern" layer: a single
   full-viewport canvas behind the UI that runs a generative effect
   keyed on the active [data-palette]. Respects prefers-reduced-motion.
   ============================================================ */
(function () {
  'use strict';

  // palette -> { type, color: 'fg' | hex, intensity }
  var CONFIG = {
    original:  { type: 'none' },
    default:   { type: 'none' },
    midnight:  { type: 'rain',           color: '#ffffff', intensity: 0.5 },
    paper:     { type: 'dots',           color: 'fg',      intensity: 1 },
    cyberpunk: { type: 'synapse',        color: 'fg',      intensity: 1 },
    retrowave: { type: 'embers',         color: 'fg',      intensity: 1 },
    forest:    { type: 'petals',         color: 'fg',      intensity: 1 },
    ocean:     { type: 'constellations', color: 'fg',      intensity: 1 },
    organs:    { type: 'rain',           color: '#451616', intensity: 0.65 },
    ume:       { type: 'petals',         color: '#f5a0c0', intensity: 1 },
    cute:      { type: 'sparkles',       color: '#ff8cb8', intensity: 1 },
    copper:    { type: 'none' },
    lavender:  { type: 'dots',           color: 'fg',      intensity: 0.7 },
    gpt:       { type: 'none' },
    claude:    { type: 'none' }
  };

  var canvas, ctx, W = 0, H = 0, DPR = 1, raf = 0, t = 0;
  var current = null;          // active config
  var parts = [];              // particle store
  var reduce = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  function ready(fn) {
    if (document.body) fn();
    else document.addEventListener('DOMContentLoaded', fn, { once: true });
  }

  function init() {
    canvas = document.createElement('canvas');
    canvas.id = 'fx-canvas';
    document.body.insertBefore(canvas, document.body.firstChild);
    ctx = canvas.getContext('2d');
    resize();
    window.addEventListener('resize', resize);
    apply();
    new MutationObserver(apply).observe(document.documentElement, {
      attributes: true, attributeFilter: ['data-palette', 'data-theme']
    });
  }

  function resize() {
    DPR = Math.min(window.devicePixelRatio || 1, 1.5);
    W = window.innerWidth; H = window.innerHeight;
    canvas.width = W * DPR; canvas.height = H * DPR;
    canvas.style.width = W + 'px'; canvas.style.height = H + 'px';
    ctx.setTransform(DPR, 0, 0, DPR, 0, 0);
    if (current) spawn(current.type);   // re-seed for new size
  }

  function cssVar(name) {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }
  function resolveColor(c) { return (!c || c === 'fg') ? (cssVar('--ink') || '#888') : c; }

  function apply() {
    var pal = document.documentElement.getAttribute('data-palette') || 'default';
    var cfg = CONFIG[pal] || CONFIG.default;
    current = { type: cfg.type, color: resolveColor(cfg.color), intensity: cfg.intensity || 1 };
    cancelAnimationFrame(raf);
    ctx.clearRect(0, 0, W, H);
    if (current.type === 'none') { canvas.style.opacity = '0'; return; }
    canvas.style.opacity = '1';
    spawn(current.type);
    if (current.type === 'dots') { drawDots(); return; }   // static
    if (reduce) { staticFrame(); return; }                  // one calm frame
    loop();
  }

  function rnd(a, b) { return a + Math.random() * (b - a); }
  function area() { return W * H; }
  function count(base) { return Math.max(8, Math.round(base * current.intensity * Math.min(1.6, area() / 900000))); }

  function spawn(type) {
    parts = [];
    var i, n;
    if (type === 'synapse') {
      n = count(90);
      for (i = 0; i < n; i++) parts.push({ x: rnd(0, W), y: rnd(0, H), vx: rnd(-0.25, 0.25), vy: rnd(-0.25, 0.25) });
    } else if (type === 'rain') {
      n = count(160);
      for (i = 0; i < n; i++) parts.push({ x: rnd(0, W), y: rnd(0, H), len: rnd(8, 22), sp: rnd(4, 11), a: rnd(0.15, 0.5) });
    } else if (type === 'embers') {
      n = count(70);
      for (i = 0; i < n; i++) parts.push({ x: rnd(0, W), y: rnd(0, H), r: rnd(0.6, 2.2), vy: rnd(0.3, 1.1), sway: rnd(0, 6.28), a: rnd(0.2, 0.7) });
    } else if (type === 'petals') {
      n = count(46);
      for (i = 0; i < n; i++) parts.push({ x: rnd(0, W), y: rnd(0, H), r: rnd(2.5, 6), vy: rnd(0.3, 0.9), sway: rnd(0, 6.28), sp: rnd(0.4, 1.1), rot: rnd(0, 6.28), a: rnd(0.18, 0.5) });
    } else if (type === 'constellations') {
      n = count(80);
      for (i = 0; i < n; i++) parts.push({ x: rnd(0, W), y: rnd(0, H), r: rnd(0.5, 1.7), tw: rnd(0, 6.28), ts: rnd(0.6, 1.8) });
    } else if (type === 'perlinflow') {
      n = count(140);
      for (i = 0; i < n; i++) parts.push({ x: rnd(0, W), y: rnd(0, H), a: rnd(0.15, 0.5) });
    } else if (type === 'sparkles') {
      n = count(70);
      for (i = 0; i < n; i++) parts.push({ x: rnd(0, W), y: rnd(0, H), r: rnd(0.8, 2.4), ph: rnd(0, 6.28), sp: rnd(0.8, 2.2) });
    }
  }

  function hexToRgb(h) {
    h = (h || '#888888').replace('#', '');
    if (h.length === 3) h = h.split('').map(function (x) { return x + x; }).join('');
    return [parseInt(h.slice(0, 2), 16) || 136, parseInt(h.slice(2, 4), 16) || 136, parseInt(h.slice(4, 6), 16) || 136];
  }
  function rgba(h, a) { var c = hexToRgb(h); return 'rgba(' + c[0] + ',' + c[1] + ',' + c[2] + ',' + a + ')'; }

  // ---- static renderers ----
  function drawDots() {
    ctx.clearRect(0, 0, W, H);
    var gap = 26, r = 1.1, col = rgba(current.color, 0.10 * current.intensity);
    ctx.fillStyle = col;
    for (var y = gap; y < H; y += gap) for (var x = gap; x < W; x += gap) {
      ctx.beginPath(); ctx.arc(x, y, r, 0, 6.2832); ctx.fill();
    }
  }
  function staticFrame() { t = 40; frame(); cancelAnimationFrame(raf); }

  // ---- animation ----
  function loop() { frame(); raf = requestAnimationFrame(loop); }

  function frame() {
    t += 1;
    var type = current.type, col = current.color, I = current.intensity, i, p, q, d, dx, dy;

    if (type === 'perlinflow') {
      // fade trails instead of hard clear
      ctx.fillStyle = rgba(cssVar('--bg') || '#000', 0.12);
      ctx.fillRect(0, 0, W, H);
    } else {
      ctx.clearRect(0, 0, W, H);
    }

    if (type === 'synapse') {
      ctx.shadowBlur = 6; ctx.shadowColor = rgba(col, 0.8);
      for (i = 0; i < parts.length; i++) {
        p = parts[i]; p.x += p.vx; p.y += p.vy;
        if (p.x < 0 || p.x > W) p.vx *= -1;
        if (p.y < 0 || p.y > H) p.vy *= -1;
      }
      var R = 150;
      ctx.lineWidth = 1;
      for (i = 0; i < parts.length; i++) {
        p = parts[i];
        for (var j = i + 1; j < parts.length; j++) {
          q = parts[j]; dx = p.x - q.x; dy = p.y - q.y; d = dx * dx + dy * dy;
          if (d < R * R) {
            ctx.strokeStyle = rgba(col, (1 - Math.sqrt(d) / R) * 0.32 * I);
            ctx.beginPath(); ctx.moveTo(p.x, p.y); ctx.lineTo(q.x, q.y); ctx.stroke();
          }
        }
      }
      ctx.fillStyle = rgba(col, 0.9 * I);
      for (i = 0; i < parts.length; i++) { p = parts[i]; ctx.beginPath(); ctx.arc(p.x, p.y, 1.5, 0, 6.2832); ctx.fill(); }
      ctx.shadowBlur = 0;

    } else if (type === 'rain') {
      ctx.lineWidth = 1.1;
      for (i = 0; i < parts.length; i++) {
        p = parts[i];
        ctx.strokeStyle = rgba(col, p.a * I);
        ctx.beginPath(); ctx.moveTo(p.x, p.y); ctx.lineTo(p.x + 0.6, p.y + p.len); ctx.stroke();
        p.y += p.sp; if (p.y > H) { p.y = -p.len; p.x = rnd(0, W); }
      }

    } else if (type === 'embers') {
      for (i = 0; i < parts.length; i++) {
        p = parts[i];
        p.y -= p.vy; p.x += Math.sin((t * 0.02) + p.sway) * 0.3;
        if (p.y < -4) { p.y = H + 4; p.x = rnd(0, W); }
        ctx.fillStyle = rgba(col, p.a * I * (0.6 + 0.4 * Math.sin(t * 0.05 + p.sway)));
        ctx.beginPath(); ctx.arc(p.x, p.y, p.r, 0, 6.2832); ctx.fill();
      }

    } else if (type === 'petals') {
      for (i = 0; i < parts.length; i++) {
        p = parts[i];
        p.y += p.vy; p.x += Math.sin((t * 0.01 * p.sp) + p.sway) * 0.6; p.rot += 0.01;
        if (p.y > H + 8) { p.y = -8; p.x = rnd(0, W); }
        ctx.save(); ctx.translate(p.x, p.y); ctx.rotate(p.rot);
        ctx.fillStyle = rgba(col, p.a * I);
        ctx.beginPath(); ctx.ellipse(0, 0, p.r, p.r * 0.55, 0, 0, 6.2832); ctx.fill();
        ctx.restore();
      }

    } else if (type === 'constellations') {
      var CR = 120;
      for (i = 0; i < parts.length; i++) {
        p = parts[i];
        for (var k = i + 1; k < parts.length; k++) {
          q = parts[k]; dx = p.x - q.x; dy = p.y - q.y; d = dx * dx + dy * dy;
          if (d < CR * CR) {
            ctx.strokeStyle = rgba(col, (1 - Math.sqrt(d) / CR) * 0.14 * I);
            ctx.lineWidth = 1; ctx.beginPath(); ctx.moveTo(p.x, p.y); ctx.lineTo(q.x, q.y); ctx.stroke();
          }
        }
      }
      for (i = 0; i < parts.length; i++) {
        p = parts[i]; var tw = 0.4 + 0.6 * Math.abs(Math.sin(t * 0.02 * p.ts + p.tw));
        ctx.fillStyle = rgba(col, tw * 0.8 * I);
        ctx.beginPath(); ctx.arc(p.x, p.y, p.r, 0, 6.2832); ctx.fill();
      }

    } else if (type === 'perlinflow') {
      ctx.lineWidth = 1.1;
      for (i = 0; i < parts.length; i++) {
        p = parts[i];
        var ang = (Math.sin(p.x * 0.008) + Math.cos(p.y * 0.008) + t * 0.004) * Math.PI;
        var nx = p.x + Math.cos(ang) * 1.4, ny = p.y + Math.sin(ang) * 1.4;
        ctx.strokeStyle = rgba(col, p.a * I);
        ctx.beginPath(); ctx.moveTo(p.x, p.y); ctx.lineTo(nx, ny); ctx.stroke();
        p.x = nx; p.y = ny;
        if (p.x < 0 || p.x > W || p.y < 0 || p.y > H) { p.x = rnd(0, W); p.y = rnd(0, H); }
      }

    } else if (type === 'sparkles') {
      for (i = 0; i < parts.length; i++) {
        p = parts[i];
        var a = Math.max(0, Math.sin(t * 0.04 * p.sp + p.ph));
        if (a <= 0.02) { p.x = rnd(0, W); p.y = rnd(0, H); }
        ctx.fillStyle = rgba(col, a * 0.9 * I);
        var s = p.r * (0.5 + a);
        ctx.beginPath();
        ctx.moveTo(p.x, p.y - s * 2); ctx.lineTo(p.x + s * 0.6, p.y - s * 0.6);
        ctx.lineTo(p.x + s * 2, p.y); ctx.lineTo(p.x + s * 0.6, p.y + s * 0.6);
        ctx.lineTo(p.x, p.y + s * 2); ctx.lineTo(p.x - s * 0.6, p.y + s * 0.6);
        ctx.lineTo(p.x - s * 2, p.y); ctx.lineTo(p.x - s * 0.6, p.y - s * 0.6);
        ctx.closePath(); ctx.fill();
      }
    }
  }

  ready(init);
})();
