(() => {
  const re = /fn-dashboard|fn-quiet|fn-full|fn-next|fn-main|fn-side|fn-margin|fn-weekly|fn-pace|fn-focus-ledger|fn-ledger|fn-section-rule|fn-footer|fn-stamp|fn-view-switch|fn-course-code|fn-pressure|design-task|design-priority|design-course|design-exam|fossil|health-track|health-fill|priority-chip|dashboard-task-check|fn-selected|\.tab-row|\.tab-button|\.topbar|health-pill|theme-toggle|hamburger|\.shell\b|tab-help|page-help|\.brand|\.ghost-button|\.small-button|\.eyebrow|\.panel-card|\.fade-up|window-titlebar|\.design-card|empty-copy|section-head|section-note/;
  const out = [];
  const visit = (rules, media) => {
    for (const r of rules) {
      if (r.cssRules && r.media) { visit(r.cssRules, (media ? media + ' && ' : '') + r.conditionText); continue; }
      if (r.cssRules && !r.selectorText) { if (r.conditionText !== undefined) visit(r.cssRules, (media ? media + ' && ' : '') + '@' + r.conditionText); continue; }
      if (!r.selectorText || !r.style) continue;
      if (!re.test(r.selectorText)) continue;
      const scoped = /field-notebook|^\.(fn-|fossil|design|dashboard|health|priority|tab-|topbar|shell|panel|eyebrow|ghost|small|section|empty|page-help|theme-toggle|hamburger|brand)|^:root|^\.(tab|top)/.test(r.selectorText) || /fn-|fossil/.test(r.selectorText);
      if (!scoped) continue;
      out.push((media ? `@media ${media} ` : '') + r.cssText);
    }
  };
  for (const sheet of document.styleSheets) { try { visit(sheet.cssRules, ''); } catch {} }
  return out.join('\n');
})()
