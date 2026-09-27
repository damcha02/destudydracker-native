import { useEffect } from "react";
import type { PageGuide } from "../lib/pageGuides";

type Props = {
  guide: PageGuide;
  onClose: () => void;
  /** Launches the old spotlight tutorial for this tab, when one exists. */
  onStartTutorial?: () => void;
};

export function PageGuideModal({ guide, onClose, onStartTutorial }: Props) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="whatsnew-backdrop" onMouseDown={onClose}>
      <div
        className="whatsnew-card guide-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby="page-guide-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="whatsnew-hero guide-hero">
          <span className="guide-hero-icon" aria-hidden="true">{guide.icon}</span>
          <div>
            <span className="whatsnew-badge">Page guide</span>
            <h2 id="page-guide-title">{guide.title}</h2>
            <p className="guide-tagline">{guide.tagline}</p>
          </div>
        </div>

        <p className="guide-intro">{guide.intro}</p>

        <h3 className="guide-steps-heading">{guide.stepsHeading}</h3>
        <ol className="guide-steps">
          {guide.steps.map((step) => (
            <li key={step.lead}>
              <div>
                <strong>{step.lead}</strong>
                <span>{step.body}</span>
              </div>
            </li>
          ))}
        </ol>

        {guide.tips?.length ? (
          <ul className="guide-tips">
            {guide.tips.map((tip) => (
              <li key={tip}>
                <span aria-hidden="true">💡</span>
                <span>{tip}</span>
              </li>
            ))}
          </ul>
        ) : null}

        <div className="guide-footer">
          {onStartTutorial ? (
            <button type="button" className="ghost-button small-button" onClick={onStartTutorial}>
              Walk me through it <span aria-hidden="true">-&gt;</span>
            </button>
          ) : null}
          <button type="button" className="whatsnew-cta guide-done" autoFocus onClick={onClose}>Got it</button>
        </div>
      </div>
    </div>
  );
}
