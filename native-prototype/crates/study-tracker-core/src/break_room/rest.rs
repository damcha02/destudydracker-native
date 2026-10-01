//! The Wabi-Sabi Rest room's two clocks (`renderWabiBreakRoom` in `desktop/src/App.tsx`): the
//! rest timer ("Rest, N minutes") and the 4-7-8 breathing exercise. Neither is persisted in
//! production (plain React state); both tick once a second there. Native derives them from
//! elapsed time instead of counting ticks, so a late or skipped tick cannot drift them.

/// One breathing phase: label, seconds, instruction.
pub const BREATH_PHASES: [(&str, u32, &str); 3] = [
    ("Breathe in", 4, "Through the nose, quietly."),
    ("Hold", 7, "Still, shoulders down."),
    ("Breathe out", 8, "Through the mouth, slow and audible."),
];
/// One round is 4 + 7 + 8 seconds.
pub const BREATH_CYCLE: u32 = 19;
/// Production stops the exercise on the tick after 75 elapsed seconds (four rounds = 76 s).
pub const BREATH_LAST_SECOND: u32 = 75;

/// The breathing ring at `elapsed` whole seconds (production's `breathElapsedSeconds`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreathFrame {
    pub phase: usize,
    /// Seconds left in this phase (the number in the ring).
    pub remaining: u32,
    /// Ring diameter in px: 60 -> 170 while breathing in, 170 held, 170 -> 60 breathing out.
    pub size: f64,
    /// 1-based round number ("Round N of 4").
    pub round: u32,
}

pub fn breath_frame(elapsed: u32) -> BreathFrame {
    let step = elapsed % BREATH_CYCLE;
    let (phase, phase_elapsed) = if step < 4 {
        (0, step)
    } else if step < 11 {
        (1, step - 4)
    } else {
        (2, step - 11)
    };
    let size = match phase {
        0 => 60.0 + f64::from(step) / 4.0 * 110.0,
        1 => 170.0,
        _ => 170.0 - f64::from(step - 11) / 8.0 * 110.0,
    };
    BreathFrame {
        phase,
        remaining: BREATH_PHASES[phase].1 - phase_elapsed,
        size,
        round: elapsed / BREATH_CYCLE + 1,
    }
}

/// The breathing exercise. `elapsed` is whole seconds since BEGIN; production's interval adds one
/// per second and switches the exercise off (resetting to 0) on the tick after 75.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Breathing {
    pub on: bool,
    pub elapsed: u32,
}

impl Breathing {
    /// BEGIN / STOP: both flip the state and reset the count.
    pub fn toggle(&mut self) {
        self.on = !self.on;
        self.elapsed = 0;
    }

    /// Bring the count to `seconds` elapsed since BEGIN (from a monotonic clock). Returns whether
    /// anything visible changed.
    pub fn advance_to(&mut self, seconds: u64) -> bool {
        if !self.on {
            return false;
        }
        if seconds > u64::from(BREATH_LAST_SECOND) {
            *self = Self::default();
            return true;
        }
        let seconds = seconds as u32;
        if seconds == self.elapsed {
            return false;
        }
        self.elapsed = seconds;
        true
    }
}

/// The rest timer. Production defaults to 5 minutes; editing the face accepts a clock value and
/// rounds the minute label (`Math.max(1, Math.round(seconds / 60))`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestTimer {
    pub minutes: u32,
    pub remaining: u32,
    pub running: bool,
}

impl Default for RestTimer {
    fn default() -> Self {
        Self {
            minutes: 5,
            remaining: 5 * 60,
            running: false,
        }
    }
}

impl RestTimer {
    /// START / PAUSE. Starting a finished timer refills it first.
    pub fn toggle(&mut self) {
        if !self.running && self.remaining == 0 {
            self.remaining = self.minutes * 60;
        }
        self.running = !self.running;
    }

    /// RESET.
    pub fn reset(&mut self) {
        self.running = false;
        self.remaining = self.minutes * 60;
    }

    /// Applies an edited face value in seconds (`applyBreakTimerDraft` with a parsed value).
    pub fn set_seconds(&mut self, seconds: u32) {
        self.minutes = ((f64::from(seconds) / 60.0 + 0.5).floor() as u32).max(1);
        self.remaining = seconds;
    }

    /// Counts down to `left` seconds (computed by the caller from a monotonic start), stopping
    /// at zero like production's interval. Returns whether anything visible changed.
    pub fn advance_to(&mut self, left: u32) -> bool {
        if !self.running {
            return false;
        }
        if left == 0 {
            self.remaining = 0;
            self.running = false;
            return true;
        }
        if left == self.remaining {
            return false;
        }
        self.remaining = left;
        true
    }

    /// `breakProgress`: elapsed fraction 0..1.
    pub fn progress(&self) -> f64 {
        let total = f64::from((self.minutes * 60).max(1));
        (1.0 - f64::from(self.remaining) / total).clamp(0.0, 1.0)
    }

    /// `MM:SS` of the face.
    pub fn face(&self) -> String {
        format!("{:02}:{:02}", self.remaining / 60, self.remaining % 60)
    }
}

/// `parseTimerFaceInput`: whole minutes (`"7"` -> 420 s) or `M:SS`/`MMMM:SS` with seconds <= 59;
/// at least one second; `None` for anything else (the edit is then discarded).
pub fn parse_timer_face(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if digits(trimmed) {
        let minutes: u64 = trimmed.parse().unwrap_or(u64::MAX / 60);
        return Some(minutes.saturating_mul(60).clamp(1, u64::from(u32::MAX)) as u32);
    }
    let (m, s) = trimmed.split_once(':')?;
    if !digits(m) || m.len() > 4 || !digits(s) || s.len() > 2 {
        return None;
    }
    let (minutes, seconds): (u32, u32) = (m.parse().ok()?, s.parse().ok()?);
    if seconds > 59 {
        return None;
    }
    Some((minutes * 60 + seconds).max(1))
}
