import { useEffect } from "react";
import type { ReleaseNoteEntry } from "../lib/releaseNotes";

/** `releases` is newest first and never empty - the caller renders nothing when there is nothing. */
type Props = { releases: ReleaseNoteEntry[]; onClose: () => void };

export function WhatsNewModal({ releases, onClose }: Props) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" || event.key === "Enter") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const newest = releases[0];
  const stacked = releases.length > 1;

  return (
    <div className="whatsnew-backdrop" onClick={onClose}>
      <div className="whatsnew-card" role="dialog" aria-modal="true" aria-labelledby="whatsnew-title" onClick={(event) => event.stopPropagation()}>
        <div className="whatsnew-hero">
          <span className="whatsnew-badge">Updated to v{newest.version}</span>
          {/* With several releases stacked the individual titles become section headings, so the hero
              needs a heading of its own rather than borrowing the newest one. */}
          <h2 id="whatsnew-title">{stacked ? "You have been away a while" : newest.title}</h2>
          <p>
            {stacked
              ? `Here is everything that changed across ${releases.length} updates, newest first.`
              : "Here's what's new since you last stopped by."}
          </p>
        </div>

        {releases.map((release) => (
          <section key={release.version} className="whatsnew-release">
            {stacked ? (
              <h3 className="whatsnew-release-head">
                <span>{release.title}</span>
                <span className="whatsnew-release-version">v{release.version}</span>
              </h3>
            ) : null}
            <ul className="whatsnew-list">
              {release.items.map((item) => (
                <li key={item.heading}>
                  <span className="whatsnew-icon" aria-hidden="true">{item.icon}</span>
                  <div>
                    <strong>{item.heading}</strong>
                    <span>{item.body}</span>
                  </div>
                </li>
              ))}
            </ul>
          </section>
        ))}

        <button type="button" className="whatsnew-cta" autoFocus onClick={onClose}>Let's go</button>
      </div>
    </div>
  );
}
