/* ============================================================
   Study Tracker — Landing page interactions
   Vanilla JS, no dependencies.
   ============================================================ */
(function () {
  'use strict';

  var root = document.documentElement;
  var reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  /* ---------- Theme (persisted) ---------- */
  var THEME_KEY = 'studytracker.landing.theme';
  function applyTheme(t) { root.setAttribute('data-theme', t); }
  try {
    var saved = localStorage.getItem(THEME_KEY);
    if (saved) applyTheme(saved);
    else if (window.matchMedia('(prefers-color-scheme: light)').matches) applyTheme('light');
  } catch (e) {}

  var toggle = document.getElementById('themeToggle');
  if (toggle) {
    toggle.addEventListener('click', function () {
      var next = root.getAttribute('data-theme') === 'light' ? 'dark' : 'light';
      applyTheme(next);
      try { localStorage.setItem(THEME_KEY, next); } catch (e) {}
    });
  }

  /* ---------- Current year ---------- */
  var yr = document.getElementById('year');
  if (yr) yr.textContent = new Date().getFullYear();

  /* ---------- Friend invite links ---------- */
  var inviteCard = document.getElementById('inviteCard');
  var inviteCode = document.getElementById('inviteCode');
  var copyInviteCode = document.getElementById('copyInviteCode');
  var invite = new URLSearchParams(window.location.search).get('invite');
  invite = invite ? invite.trim().toUpperCase().replace(/[^A-Z0-9-]/g, '').slice(0, 24) : '';
  if (invite && inviteCard && inviteCode) {
    inviteCode.textContent = invite;
    inviteCard.hidden = false;
  }
  if (copyInviteCode && invite) {
    copyInviteCode.addEventListener('click', function () {
      function copied() {
        copyInviteCode.textContent = 'Copied';
        setTimeout(function () { copyInviteCode.textContent = 'Copy player tag'; }, 1800);
      }
      function fallbackCopy() {
        var input = document.createElement('input');
        input.value = invite;
        input.setAttribute('readonly', '');
        input.style.position = 'fixed';
        input.style.opacity = '0';
        document.body.appendChild(input);
        input.select();
        try { document.execCommand('copy'); copied(); } catch (e) {}
        document.body.removeChild(input);
      }
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(invite).then(copied).catch(fallbackCopy);
      } else {
        fallbackCopy();
      }
    });
  }

  /* ---------- Nav: shadow on scroll + mobile menu ---------- */
  var nav = document.getElementById('nav');
  var navLinks = document.getElementById('navLinks');
  var navToggle = document.getElementById('navToggle');

  function onScroll() {
    if (nav) nav.classList.toggle('scrolled', window.scrollY > 12);
  }
  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();

  if (navToggle && navLinks) {
    navToggle.addEventListener('click', function () { navLinks.classList.toggle('open'); });
    navLinks.addEventListener('click', function (e) {
      if (e.target.tagName === 'A') navLinks.classList.remove('open');
    });
  }

  /* ---------- Smooth scroll for in-page anchors ---------- */
  document.querySelectorAll('a[href^="#"]').forEach(function (a) {
    a.addEventListener('click', function (e) {
      var id = a.getAttribute('href');
      if (id.length < 2) return;
      var target = document.querySelector(id);
      if (!target) return;
      e.preventDefault();
      target.scrollIntoView({ behavior: reduceMotion ? 'auto' : 'smooth', block: 'start' });
      history.replaceState(null, '', id);
    });
  });

  /* ---------- Scroll reveal (manual, robust across embeds) ---------- */
  var reveals = Array.prototype.slice.call(document.querySelectorAll('.reveal'));
  if (reduceMotion) {
    reveals.forEach(function (el) { el.classList.add('in'); });
  } else {
    // stagger items that share a parent
    var groups = {};
    reveals.forEach(function (el) {
      var p = el.parentElement;
      groups[p] = groups[p] || 0;
      el.style.transitionDelay = Math.min(groups[p] * 70, 350) + 'ms';
      groups[p]++;
    });

    var pending = reveals.slice();
    function revealCheck() {
      if (!pending.length) return;
      var vh = window.innerHeight || document.documentElement.clientHeight;
      var still = [];
      pending.forEach(function (el) {
        var r = el.getBoundingClientRect();
        if (r.top < vh * 0.92 && r.bottom > 0) el.classList.add('in');
        else still.push(el);
      });
      pending = still;
    }
    window.addEventListener('scroll', revealCheck, { passive: true });
    window.addEventListener('resize', revealCheck, { passive: true });
    revealCheck();
    // re-run a few times to catch font/layout shifts
    [120, 350, 700, 1200].forEach(function (t) { setTimeout(revealCheck, t); });
    // failsafe: never leave content hidden if scroll events don't fire
    setTimeout(function () { pending.forEach(function (el) { el.classList.add('in'); }); pending = []; }, 2600);
  }

  /* ---------- Install OS tabs ---------- */
  var osTabs = document.querySelectorAll('.os-tab');
  var osPanels = document.querySelectorAll('.os-panel');
  osTabs.forEach(function (tab) {
    tab.addEventListener('click', function () {
      var os = tab.getAttribute('data-os');
      osTabs.forEach(function (t) {
        var on = t === tab;
        t.classList.toggle('on', on);
        t.setAttribute('aria-selected', on ? 'true' : 'false');
      });
      osPanels.forEach(function (p) {
        p.classList.toggle('on', p.getAttribute('data-os') === os);
      });
    });
  });

  /* ---------- Linux distro chooser ---------- */
  var distroOptions = document.querySelectorAll('.distro-option');
  var distroPanels = document.querySelectorAll('.distro-panel');
  distroOptions.forEach(function (option) {
    option.addEventListener('click', function () {
      var distro = option.getAttribute('data-distro');
      distroOptions.forEach(function (o) {
        var on = o === option;
        o.classList.toggle('on', on);
        o.setAttribute('aria-pressed', on ? 'true' : 'false');
      });
      distroPanels.forEach(function (p) {
        p.classList.toggle('on', p.getAttribute('data-distro') === distro);
      });
    });
  });

  /* Auto-select the visitor's OS on first load */
  (function detectOS() {
    var ua = (navigator.userAgent || '').toLowerCase();
    var os = null;
    if (ua.indexOf('win') > -1) os = 'win';
    else if (ua.indexOf('mac') > -1) os = 'mac';
    else if (ua.indexOf('linux') > -1 || ua.indexOf('x11') > -1) os = 'linux';
    if (!os) return;
    var tab = document.querySelector('.os-tab[data-os="' + os + '"]');
    if (tab) tab.click();
  })();

  /* ---------- Ambient orb + mockup parallax ---------- */
  if (!reduceMotion) {
    var orbs = Array.prototype.slice.call(document.querySelectorAll('.orb'));
    var win = document.getElementById('window');
    var mx = 0, my = 0, cx = 0, cy = 0, sy = 0, csy = 0, raf = null;

    window.addEventListener('pointermove', function (e) {
      mx = (e.clientX / window.innerWidth - 0.5);
      my = (e.clientY / window.innerHeight - 0.5);
      schedule();
    }, { passive: true });

    window.addEventListener('scroll', function () { sy = window.scrollY; schedule(); }, { passive: true });

    function schedule() { if (!raf) raf = requestAnimationFrame(frame); }
    function frame() {
      raf = null;
      cx += (mx - cx) * 0.06;
      cy += (my - cy) * 0.06;
      csy += (sy - csy) * 0.1;
      orbs.forEach(function (o) {
        var s = parseFloat(o.getAttribute('data-speed')) || 0.05;
        o.style.transform = 'translate(' + (cx * 60 * s * 6) + 'px,' + (cy * 60 * s * 6 + csy * s) + 'px)';
      });
      if (win && window.innerWidth > 760) {
        var lift = Math.max(0, 1 - csy / 620);
        win.style.transform = 'rotateX(' + (8 * lift) + 'deg) translateY(' + (cy * 8) + 'px)';
      }
      if (Math.abs(mx - cx) > 0.001 || Math.abs(my - cy) > 0.001 || Math.abs(sy - csy) > 0.5) schedule();
    }
    frame();
  }
})();
