// Stage 20: probe-dom.js without the y>1000 cutoff, rooted at window.__probeRoot (a CSS selector) when set.
(() => {
  const root = document.querySelector(window.__probeRoot || '#root') || document.querySelector('#root');
  const rows = [];
  const r1 = (n) => Math.round(n * 10) / 10;
  const vis = (cs) => cs.display !== 'none' && cs.visibility !== 'hidden';
  const transparent = (c) => c === 'rgba(0, 0, 0, 0)' || c === 'transparent';
  const walk = (el, depth) => {
    const cs = getComputedStyle(el);
    if (!vis(cs)) return;
    const rect = el.getBoundingClientRect();
    if (rect.width === 0 && rect.height === 0) return;

    const own = [...el.childNodes].filter(n => n.nodeType === 3).map(n => n.textContent.trim()).filter(Boolean).join(' ');
    const b = (s) => `${cs.getPropertyValue('border-' + s + '-width')}/${cs.getPropertyValue('border-' + s + '-style')}/${cs.getPropertyValue('border-' + s + '-color')}`;
    const borders = ['top', 'right', 'bottom', 'left'].map(b).filter(x => !x.startsWith('0px/'));
    const bg = cs.backgroundColor;
    const interesting = own || borders.length || !transparent(bg) || cs.backgroundImage !== 'none' || el.tagName === 'svg' || el.tagName === 'SVG' || el.tagName === 'INPUT';
    if (interesting) {
      rows.push({
        d: depth, t: el.tagName.toLowerCase() + (el.className && typeof el.className === 'string' ? '.' + el.className.trim().split(/\s+/).join('.') : ''),
        box: [r1(rect.x), r1(rect.y), r1(rect.width), r1(rect.height)],
        text: own.slice(0, 48),
        font: own ? `${cs.fontFamily.split(',')[0]} ${cs.fontSize} w${cs.fontWeight} ${cs.fontStyle !== 'normal' ? cs.fontStyle : ''} lh=${cs.lineHeight} ls=${cs.letterSpacing} ${cs.textTransform !== 'none' ? cs.textTransform : ''}`.trim() : undefined,
        color: own ? cs.color : undefined,
        bg: !transparent(bg) ? bg : undefined, bgi: cs.backgroundImage !== 'none' ? cs.backgroundImage.slice(0, 90) : undefined,
        borders: borders.length ? borders : undefined, radius: cs.borderRadius !== '0px' ? cs.borderRadius : undefined,
        pad: cs.padding !== '0px' ? cs.padding : undefined, shadow: cs.boxShadow !== 'none' ? cs.boxShadow : undefined, opacity: cs.opacity !== '1' ? cs.opacity : undefined,
      });
    }
    for (const c of el.children) walk(c, depth + 1);
  };
  walk(root, 0);
  return rows.map(r => JSON.stringify(r)).join('\n');
})()
