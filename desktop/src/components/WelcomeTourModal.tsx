import { useEffect, useState } from "react";
import { WELCOME_TOUR_STOPS } from "../lib/welcomeTour";

type Props = {
  /** Called on finish, skip or Escape - the tour is recorded as seen either way. */
  onClose: () => void;
};

/** Step 0 is the greeting and overview; every stop after that gets a step of its own. */
const STEP_COUNT = WELCOME_TOUR_STOPS.length + 1;

export function WelcomeTourModal({ onClose }: Props) {
  const [step, setStep] = useState(0);
  const stop = step === 0 ? null : WELCOME_TOUR_STOPS[step - 1];
  const last = step === STEP_COUNT - 1;

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
      if (event.key === "ArrowRight") setStep((current) => Math.min(current + 1, STEP_COUNT - 1));
      if (event.key === "ArrowLeft") setStep((current) => Math.max(current - 1, 0));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="whatsnew-backdrop">
      <div className="whatsnew-card tour-card" role="dialog" aria-modal="true" aria-labelledby="tour-title">
        {step === 0 ? (
          <>
            <div className="whatsnew-hero">
              <span className="whatsnew-badge">Welcome</span>
              <h2 id="tour-title">Good to have you here</h2>
              <p>Study Tracker is six pages and a menu. Here is the whole thing in one screen - then we will go through them one at a time.</p>
            </div>
            <ul className="whatsnew-list tour-overview">
              {WELCOME_TOUR_STOPS.map((item) => (
                <li key={item.id}>
                  <span className="whatsnew-icon" aria-hidden="true">{item.icon}</span>
                  <div>
                    <strong>{item.label}</strong>
                    <span>{item.tagline}</span>
                  </div>
                </li>
              ))}
            </ul>
          </>
        ) : stop ? (
          <div className="tour-stop">
            <span className="tour-stop-icon" aria-hidden="true">{stop.icon}</span>
            <span className="whatsnew-badge">{stop.id === "menu" ? "The menu" : `Tab ${step} of ${WELCOME_TOUR_STOPS.length - 1}`}</span>
            <h2 id="tour-title">{stop.label}</h2>
            <p className="tour-stop-tagline">{stop.tagline}</p>
            <p className="tour-stop-body">{stop.body}</p>
          </div>
        ) : null}

        <div className="tour-footer">
          <div className="tour-dots" role="presentation">
            {Array.from({ length: STEP_COUNT }, (_, index) => (
              <span key={index} className={index === step ? "active" : ""} />
            ))}
          </div>
          <div className="tour-actions">
            {last ? null : (
              <button type="button" className="ghost-button small-button" onClick={onClose}>Skip</button>
            )}
            {step > 0 ? (
              <button type="button" className="ghost-button small-button" onClick={() => setStep(step - 1)}>Back</button>
            ) : null}
            <button
              type="button"
              className="whatsnew-cta tour-next"
              autoFocus
              onClick={() => (last ? onClose() : setStep(step + 1))}
            >
              {last ? "Start studying" : step === 0 ? "Show me around" : "Next"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
