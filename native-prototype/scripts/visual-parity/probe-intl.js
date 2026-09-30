(() => {
  const out = [];
  for (let m = 0; m < 12; m++) {
    const d = new Date(2026, m, 7 + m % 3);
    out.push([
      new Intl.DateTimeFormat("en-GB", { weekday: "short", day: "numeric", month: "short" }).format(d),
      new Intl.DateTimeFormat("en", { month: "short", day: "numeric" }).format(d),
      new Intl.DateTimeFormat("en", { weekday: "long", day: "2-digit", month: "short", year: "numeric" }).format(d),
      new Intl.DateTimeFormat("en", { weekday: "short" }).format(d),
    ].join(" | "));
  }
  return out.join("\n");
})()
