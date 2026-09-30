(() => {
  const names = new Set();
  for (const sheet of document.styleSheets) {
    let rules; try { rules = sheet.cssRules; } catch { continue; }
    for (const r of rules) {
      if (!r.style) continue;
      for (const p of r.style) if (p.startsWith('--')) names.add(p);
    }
  }
  const cs = getComputedStyle(document.documentElement);
  const out = {};
  for (const n of [...names].sort()) {
    if (/^--(fn-|surface|ink|line|bg|panel|accent|ok|warn|danger|steady|watch|critical|font|radius|shadow|text|muted|border|card|paper)/.test(n)) out[n] = cs.getPropertyValue(n).trim();
  }
  return { attrs: Object.fromEntries([...document.documentElement.attributes].map(a => [a.name, a.value])), bodyFont: getComputedStyle(document.body).fontFamily, vars: out };
})()
