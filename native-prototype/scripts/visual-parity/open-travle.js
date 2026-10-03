// Stage 21: clicks the Travle card's Play button (Field Notebook page or Wabi Rest games list).
[...document.querySelectorAll('[data-tour="break-game-action"]')].find((b) => { let e = b; for (let i = 0; i < 5 && e; i++) { e = e.parentElement; const t = e?.textContent ?? ""; if (t.includes("Travle")) return !t.includes("Flaggle") && !t.includes("Wordle"); } return false; })?.click()
