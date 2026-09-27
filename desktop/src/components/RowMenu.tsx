import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";

/** A small "more actions" (three dots) menu; closes on outside click, Escape, or choosing an item. */
export function RowMenu({ label, children }: { label: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return undefined;
    const close = (event: MouseEvent) => { if (!ref.current?.contains(event.target as Node)) setOpen(false); };
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") setOpen(false); };
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", escape);
    return () => { document.removeEventListener("mousedown", close); document.removeEventListener("keydown", escape); };
  }, [open]);
  return (
    <div className="msm-menu" ref={ref}>
      <button type="button" className="msm-icon-button" aria-label={label} aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((current) => !current)}>&#8943;</button>
      {open ? <div className="msm-menu-list" role="menu" onClick={() => setOpen(false)}>{children}</div> : null}
    </div>
  );
}
