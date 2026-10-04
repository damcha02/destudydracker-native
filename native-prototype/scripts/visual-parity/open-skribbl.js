// Stage 22a: clicks the Daily Skribbl card's Play button (Field Notebook page or Wabi Rest games list).
[...document.querySelectorAll('[data-tour="break-game-action"]')].find((b) => { let e = b; for (let i = 0; i < 5 && e; i++) { e = e.parentElement; const t = e?.textContent ?? ""; if (t.includes("Daily Skribbl")) return !t.includes("Travle") && !t.includes("Wordle"); } return false; })?.click()
