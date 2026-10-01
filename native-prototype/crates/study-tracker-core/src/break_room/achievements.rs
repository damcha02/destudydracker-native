//! Every achievement production v0.1.67 defines (`badges`, `petRockBadges`, `profileBadgeGroups`
//! in `desktop/src/App.tsx`), evaluated as pure functions of the domain state.
//!
//! Production does **not** store "unlocked" flags: every achievement is recomputed from state on
//! each render, so it can never be awarded twice and survives restarts/imports by construction.
//! Production stores only three things around them, all reproduced here:
//! - `badgeCounts`/`badgeCountDates`: the five *daily* badges count once per local day
//!   ([`BreakRoomState::count_daily_badges`](super::state::BreakRoomState::count_daily_badges));
//!   their "earned" is `earned today || count > 0`.
//! - `achievementEarnedOnDates`: the local day an achievement was *watched* turning earned while
//!   the app ran ([`EarnedDateTracker`]); nothing is back-dated.
//! - nothing else: no XP, no reward.
//!
//! Garden achievements are functions of sessions, tasks and the rolling week, exactly the inputs
//! the (Stage 23) Garden itself derives from, so they are evaluated here without any Garden state.

use super::catalog::GAME_COUNT;
use super::state::BreakRoomState;
use crate::dashboard::civil::{CivilDate, LocalClock};
use crate::timer::WallTimestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// "Break Room": "Unlock and play break games from the Break Room."
    BreakRoom,
    /// The "Pet Rock" subgroup of Break Room.
    PetRock,
    /// "Focus Fossil": "Earn these by building all-time study hours."
    FocusFossil,
    /// "Garden of Knowledge": "Earn these by keeping your study garden alive."
    Garden,
}

impl Category {
    pub fn title(self) -> &'static str {
        match self {
            Category::BreakRoom => "Break Room",
            Category::PetRock => "Pet Rock",
            Category::FocusFossil => "Focus Fossil",
            Category::Garden => "Garden of Knowledge",
        }
    }
    pub fn source(self) -> &'static str {
        match self {
            Category::BreakRoom => "Unlock and play break games from the Break Room.",
            Category::PetRock => "Earn these by patting the Break Room pet rock.",
            Category::FocusFossil => "Earn these by building all-time study hours.",
            Category::Garden => "Earn these by keeping your study garden alive.",
        }
    }
}

/// One evaluated achievement, in production's display order.
#[derive(Debug, Clone, PartialEq)]
pub struct Achievement {
    pub id: String,
    pub category: Category,
    /// The profile/Field Notebook glyph (emoji, or ◆ ❀ ... for fossils/garden).
    pub icon: &'static str,
    pub name: &'static str,
    pub how: String,
    pub earned: bool,
    /// The five daily badges carry a per-day count (`×N` on the profile, copies on the album).
    pub daily: bool,
    pub count: u64,
}

/// `petRockMilestones` (also the rock's stages, below `rockStage`).
pub const PET_ROCK_MILESTONES: [(&str, &str, &str, u64, &str); 19] = [
    ("rock-sprouting", "\u{1F331}", "Sprouting Rock", 10, "10"),
    ("rock-growing", "\u{1F33F}", "Growing Rock", 50, "50"),
    (
        "rock-flourished",
        "\u{1F333}",
        "Flourished Rock",
        100,
        "100",
    ),
    ("rock-blooming", "\u{1F98B}", "Blooming Rock", 250, "250"),
    ("rock-royal", "\u{1F451}", "Royal Rock", 500, "500"),
    ("rock-hellish", "\u{1F525}", "Hellish Rock", 666, "666"),
    ("rock-heavenly", "\u{1F607}", "Heavenly Rock", 888, "888"),
    ("rock-cosmic", "\u{1F31F}", "Cosmic Rock", 1000, "1k"),
    ("rock-galactic", "\u{1F30C}", "Galactic Rock", 5000, "5k"),
    ("rock-eternal", "\u{1FAA8}", "Eternal Rock", 10000, "10k"),
    (
        "rock-meteoric",
        "\u{2604}\u{FE0F}",
        "Meteoric Rock",
        20000,
        "20k",
    ),
    (
        "rock-planetary",
        "\u{1FA90}",
        "Planetary Rock",
        50000,
        "50k",
    ),
    (
        "rock-celestial",
        "\u{1F320}",
        "Celestial Rock",
        100000,
        "100k",
    ),
    (
        "rock-starstone",
        "\u{1F48E}",
        "Ancient Starstone",
        500000,
        "500k",
    ),
    (
        "rock-hells-diplomat",
        "\u{1F608}",
        "Hell's Diplomat",
        666666,
        "666,666",
    ),
    (
        "rock-saint",
        "\u{1F54A}\u{FE0F}",
        "Saint",
        888888,
        "888,888",
    ),
    ("rock-god", "\u{1F5FF}", "Rock God", 1000000, "1M"),
    ("rock-demon", "\u{1F47F}", "Demon", 6666666, "6,666,666"),
    (
        "rock-guardian-angel",
        "\u{1F47C}",
        "Guardian Angel",
        8888888,
        "8,888,888",
    ),
];

/// `rockStage`: the plant drawn on the rock and the rock's title. Production's ladder is the
/// milestone list read top-down (so 666 666 pats is "Hell's Diplomat", 888 888 "Saint" ...); it
/// coincides with "the last milestone reached" for every value, which a test pins.
pub fn rock_stage(pats: u64) -> (&'static str, &'static str) {
    match PET_ROCK_MILESTONES.iter().rev().find(|m| pats >= m.3) {
        Some(m) => (m.1, m.2),
        None => ("", "Pet Rock"),
    }
}

/// `focusMilestones` hours, ids `fossil-<hours>`.
pub const FOSSIL_MILESTONES: [(u64, &str, &str); 7] = [
    (10, "Seed Fossil", "10 hours of study unearthed"),
    (25, "Shell Fragment", "25 hours - patterns forming"),
    (50, "Ammonite", "50 hours - taking shape"),
    (100, "Crystal Cluster", "100 hours crystallized"),
    (250, "Complete Specimen", "250 hours - a rare find"),
    (500, "Ancient Artifact", "500 hours - legendary"),
    (1000, "Golden Record", "1000 hours - transcendent"),
];

/// The derived inputs the Garden badges read, computed by the caller from the academic domain
/// with production's own metric functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GardenInputs {
    /// `getStudySessionCount`: study + exam sessions in the (pruned) history.
    pub study_session_count: usize,
    /// `getStreakDays` (any session kind).
    pub streak_days: u32,
    /// `getWeeklyCourseCount` (with production's UTC-date quirk).
    pub weekly_course_count: usize,
    /// `getWeeklyActivity` total over the rolling seven days (all session kinds).
    pub weekly_total_minutes: u64,
    /// Tasks with `totalUnits > 0 && completedUnits >= totalUnits`.
    pub completed_task_count: usize,
}

/// `gardenStage`: thresholds 0/30/90/210/420/720 minutes in the rolling week -> 0..5.
pub fn garden_stage(weekly_total_minutes: u64) -> u32 {
    let reached = [0u64, 30, 90, 210, 420, 720]
        .iter()
        .filter(|t| weekly_total_minutes >= **t)
        .count() as u32;
    reached.saturating_sub(1)
}

/// Every input the evaluation needs, for one local day.
pub struct AchievementInputs<'a> {
    pub state: &'a BreakRoomState,
    pub today: CivilDate,
    pub clock: &'a dyn LocalClock,
    /// `lifetimeStudyMinutes` (the running counter, not the pruned history).
    pub lifetime_minutes: u64,
    pub garden: GardenInputs,
}

/// The five daily badges' "seen earned today" flags, in production's effect order.
pub fn daily_hits(input: &AchievementInputs<'_>) -> [(&'static str, bool); 5] {
    let b = BreakFlags::compute(input);
    [
        ("full-house", b.full_house),
        ("early-bird", b.early_bird),
        ("night-owl", b.night_owl),
        ("speedrunner", b.speedrunner_today),
        ("perfectionist", b.perfectionist),
    ]
}

struct BreakFlags {
    full_house: bool,
    first_break: bool,
    on_fire: bool,
    early_bird: bool,
    night_owl: bool,
    speedrunner_today: bool,
    explorer: bool,
    perfectionist: bool,
    veteran: bool,
}

/// The local hour (0..23) of an instant, `new Date(x).getHours()`.
pub fn local_hour(clock: &dyn LocalClock, instant: WallTimestamp) -> i64 {
    let midnight = clock.local_midnight(clock.local_date(instant));
    (instant.unix_millis - midnight.unix_millis).div_euclid(3_600_000)
}

impl BreakFlags {
    fn compute(input: &AchievementInputs<'_>) -> Self {
        let s = input.state;
        let today = input.today.to_iso();
        let unlocked_today = if s.unlocked_games_date == today {
            s.unlocked_games.len()
        } else {
            0
        };
        let played = s.played_today(&today);
        let hours: Vec<i64> = played
            .iter()
            .filter_map(|p| p.played_at.map(|t| local_hour(input.clock, t)))
            .collect();
        let mut distinct: Vec<&str> = Vec::new();
        for p in played {
            if !distinct.contains(&p.name.as_str()) {
                distinct.push(&p.name);
            }
        }
        let full_house = unlocked_today == GAME_COUNT;
        Self {
            full_house,
            first_break: s.total_unlocks >= 1,
            on_fire: s.unlock_streak >= 3,
            early_bird: hours.iter().any(|h| *h < 9),
            night_owl: hours.iter().any(|h| *h >= 22),
            speedrunner_today: s.speedrunner_today && s.last_unlock_date == today,
            explorer: s.played_games_all_time.len() >= GAME_COUNT,
            perfectionist: full_house && distinct.len() == GAME_COUNT,
            veteran: s.total_unlocks >= 10,
        }
    }
}

/// All achievements, grouped and ordered as production's `profileBadgeGroups` (Break Room's nine
/// plus `rock-current`, then the 19 pet-rock milestones, 7 Focus Fossil, 7 Garden).
pub fn evaluate(input: &AchievementInputs<'_>) -> Vec<Achievement> {
    let s = input.state;
    let b = BreakFlags::compute(input);
    let count = |id: &str| s.badge_counts.get(id).copied().unwrap_or(0);
    let n = GAME_COUNT;
    let mut out = Vec::with_capacity(43);
    let mut push = |id: &str, category, icon, name, how: String, earned: bool, daily: bool| {
        out.push(Achievement {
            id: id.to_string(),
            category,
            icon,
            name,
            how,
            earned,
            daily,
            count: if daily { count(id) } else { 0 },
        })
    };
    use Category::*;
    push(
        "full-house",
        BreakRoom,
        "\u{1F3C6}",
        "Full House",
        format!("Unlock all {n} break games in one day."),
        b.full_house || count("full-house") > 0,
        true,
    );
    push(
        "first-break",
        BreakRoom,
        "\u{2B50}",
        "First Break",
        "Unlock any Break Room game once.".into(),
        b.first_break,
        false,
    );
    push(
        "on-fire",
        BreakRoom,
        "\u{1F525}",
        "On Fire",
        "Unlock at least one break game on 3 consecutive days.".into(),
        b.on_fire,
        false,
    );
    push(
        "early-bird",
        BreakRoom,
        "\u{1F305}",
        "Early Bird",
        "Play an unlocked break game before 9:00 AM.".into(),
        b.early_bird || count("early-bird") > 0,
        true,
    );
    push(
        "night-owl",
        BreakRoom,
        "\u{1F989}",
        "Night Owl",
        "Play an unlocked break game at or after 10:00 PM.".into(),
        b.night_owl || count("night-owl") > 0,
        true,
    );
    push("speedrunner", BreakRoom, "\u{26A1}", "Speedrunner", "Unlock your first break after earning a 45-minute break token from one single study or exam session.".into(), b.speedrunner_today || count("speedrunner") > 0, true);
    push(
        "explorer",
        BreakRoom,
        "\u{1F5FA}\u{FE0F}",
        "Explorer",
        format!("Play all {n} different break games at least once."),
        b.explorer,
        false,
    );
    push(
        "perfectionist",
        BreakRoom,
        "\u{1F3AF}",
        "Perfectionist",
        format!("Unlock all {n} games and play all {n} games on the same day."),
        b.perfectionist || count("perfectionist") > 0,
        true,
    );
    push(
        "veteran",
        BreakRoom,
        "\u{1F48E}",
        "Veteran",
        "Unlock break games 10 total times.".into(),
        b.veteran,
        false,
    );
    let pats = s.pet_rock_pats;
    let current = PET_ROCK_MILESTONES.iter().rev().find(|m| pats >= m.3);
    let next = PET_ROCK_MILESTONES.iter().find(|m| pats < m.3);
    push(
        "rock-current",
        BreakRoom,
        current.map_or("\u{1FAA8}", |m| m.1),
        current.map_or("Pet Rock", |m| m.2),
        match next {
            Some(m) => format!("Pat the pet rock {} times.", m.4),
            None => "Reach the final pet rock form.".into(),
        },
        current.is_some(),
        false,
    );
    for (id, icon, name, threshold, label) in PET_ROCK_MILESTONES {
        push(
            id,
            PetRock,
            icon,
            name,
            format!("Pat the pet rock {label} times."),
            pats >= threshold,
            false,
        );
    }
    for (hours, name, desc) in FOSSIL_MILESTONES {
        // The profile shows `◆`; ids are `fossil-<hours>`.
        out.push(Achievement {
            id: format!("fossil-{hours}"),
            category: FocusFossil,
            icon: "\u{25C6}",
            name,
            how: desc.to_string(),
            earned: input.lifetime_minutes >= hours * 60,
            daily: false,
            count: 0,
        });
    }
    let g = input.garden;
    let stage = garden_stage(g.weekly_total_minutes);
    let garden: [(&str, &str, &str, &str, bool); 7] = [
        (
            "garden-first-sprout",
            "\u{2740}",
            "First Sprout",
            "Log your first study session.",
            g.study_session_count >= 1,
        ),
        (
            "garden-streak-bloom",
            "\u{273A}",
            "Streak Bloom",
            "Maintain a 5-day study streak.",
            g.streak_days >= 5,
        ),
        (
            "garden-mushroom-ring",
            "\u{2741}",
            "Mushroom Ring",
            "Log 20 study sessions.",
            g.study_session_count >= 20,
        ),
        (
            "garden-cross-pollinator",
            "\u{2724}",
            "Cross-Pollinator",
            "Study 3 or more different courses in the last 7 days.",
            g.weekly_course_count >= 3,
        ),
        (
            "garden-full-bloom",
            "\u{25C7}",
            "Full Bloom",
            "Reach the Blooming garden stage (420+ minutes in 7 days).",
            stage >= 4,
        ),
        (
            "garden-harvest-season",
            "\u{274B}",
            "Harvest Season",
            "Fully complete 10 tasks.",
            g.completed_task_count >= 10,
        ),
        (
            "garden-wise-tree",
            "\u{2663}",
            "The Wise Tree",
            "Reach 720 minutes in the last 7 days, or maintain a 10-day study streak.",
            stage >= 5 || g.streak_days >= 10,
        ),
    ];
    for (id, icon, name, how, earned) in garden {
        out.push(Achievement {
            id: id.to_string(),
            category: Garden,
            icon,
            name,
            how: how.to_string(),
            earned,
            daily: false,
            count: 0,
        });
    }
    out
}

/// `BREAK_ROOM_WALL_ICONS`: the picture glyphs used instead of the fossil/garden text symbols.
pub fn wall_icon(id: &str) -> Option<&'static str> {
    Some(match id {
        "fossil-10" => "\u{1F330}",
        "fossil-25" => "\u{1F9AA}",
        "fossil-50" => "\u{1F41A}",
        "fossil-100" => "\u{1F52E}",
        "fossil-250" => "\u{1F9B4}",
        "fossil-500" => "\u{1F3FA}",
        "fossil-1000" => "\u{1F4C0}",
        "garden-first-sprout" => "\u{1F33C}",
        "garden-streak-bloom" => "\u{1F338}",
        "garden-mushroom-ring" => "\u{1F344}",
        "garden-cross-pollinator" => "\u{1F41D}",
        "garden-full-bloom" => "\u{1F33A}",
        "garden-harvest-season" => "\u{1F33E}",
        "garden-wise-tree" => "\u{1F332}",
        _ => return None,
    })
}

/// `JP_NAMES` of the album.
pub fn japanese_name(id: &str) -> Option<&'static str> {
    Some(match id {
        "full-house" => "満室",
        "first-break" => "初休",
        "on-fire" => "熱中",
        "early-bird" => "早起き",
        "night-owl" => "夜更かし",
        "speedrunner" => "疾走",
        "explorer" => "探検",
        "perfectionist" => "完璧",
        "veteran" => "熟練",
        "rock-sprouting" => "発芽",
        "rock-growing" => "生長",
        "rock-flourished" => "繁茂",
        "rock-blooming" => "開花",
        "rock-royal" => "王者",
        "rock-hellish" => "地獄",
        "rock-heavenly" => "天国",
        "rock-cosmic" => "宇宙",
        "rock-galactic" => "銀河",
        "rock-eternal" => "永遠",
        "rock-meteoric" => "流星",
        "rock-planetary" => "惑星",
        "rock-celestial" => "天体",
        "rock-starstone" => "星石",
        "rock-hells-diplomat" => "使者",
        "rock-saint" => "聖人",
        "rock-god" => "石神",
        "rock-demon" => "悪魔",
        "rock-guardian-angel" => "守護天使",
        "rock-current" => "石",
        "fossil-10" => "発掘",
        "fossil-25" => "貝殻",
        "fossil-50" => "化石",
        "fossil-100" => "結晶",
        "fossil-250" => "標本",
        "fossil-500" => "遺物",
        "fossil-1000" => "金字塔",
        "garden-first-sprout" => "初芽",
        "garden-streak-bloom" => "継続",
        "garden-mushroom-ring" => "菌輪",
        "garden-cross-pollinator" => "受粉",
        "garden-full-bloom" => "満開",
        "garden-harvest-season" => "収穫",
        "garden-wise-tree" => "老木",
        _ => return None,
    })
}

/// `achievementEarnedOnDates` bookkeeping. The first evaluation after launch only records a
/// baseline (an achievement already earned then has no known day and is never given one); after
/// that, an id that turns earned is dated today, once - a later dip and re-earn keeps its first
/// date. This is exactly production's `seenEarnedIdsRef` effect, including that the baseline is
/// per app run.
#[derive(Debug, Default)]
pub struct EarnedDateTracker {
    baseline: Option<Vec<String>>,
}

impl EarnedDateTracker {
    /// Feed the currently earned ids; returns the ids newly dated (already written into `dates`).
    pub fn observe(
        &mut self,
        earned: &[String],
        dates: &mut std::collections::BTreeMap<String, String>,
        today: &str,
    ) -> Vec<String> {
        let previous = self.baseline.replace(earned.to_vec());
        let Some(previous) = previous else {
            return Vec::new();
        };
        let mut dated = Vec::new();
        for id in earned {
            if previous.contains(id) || dates.contains_key(id) {
                continue;
            }
            dates.insert(id.clone(), today.to_string());
            dated.push(id.clone());
        }
        dated
    }
}

/// One album entry (`earnedAchievements`): every earned achievement once, `rock-current` excluded,
/// with the wall icon substituted and the recorded day when there is one.
#[derive(Debug, Clone, PartialEq)]
pub struct AlbumEntry {
    pub id: String,
    pub icon: &'static str,
    pub name: &'static str,
    pub how: String,
    pub earned_on: Option<String>,
}

pub fn album_entries(
    achievements: &[Achievement],
    dates: &std::collections::BTreeMap<String, String>,
) -> Vec<AlbumEntry> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for a in achievements {
        if !a.earned || a.id == "rock-current" || seen.contains(&a.id.as_str()) {
            continue;
        }
        seen.push(&a.id);
        out.push(AlbumEntry {
            id: a.id.clone(),
            icon: wall_icon(&a.id).unwrap_or(a.icon),
            name: a.name,
            how: a.how.clone(),
            earned_on: dates.get(&a.id).filter(|d| !d.is_empty()).cloned(),
        });
    }
    out
}

/// `buildBookPages`: dated entries first (oldest first, then id), undated after (by name), three
/// per page; an empty album is one "empty" page; an odd page count gets a blank right page.
#[derive(Debug, Clone, PartialEq)]
pub struct AlbumPage {
    pub rows: Vec<AlbumEntry>,
    /// 1-based.
    pub number: usize,
    pub left: bool,
    pub empty: bool,
}

pub const ALBUM_PER_PAGE: usize = 3;

pub fn album_pages(entries: &[AlbumEntry]) -> Vec<AlbumPage> {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| match (&a.earned_on, &b.earned_on) {
        (Some(x), Some(y)) => x.cmp(y).then_with(|| a.id.cmp(&b.id)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.name.cmp(b.name),
    });
    let mut pages: Vec<AlbumPage> = sorted
        .chunks(ALBUM_PER_PAGE)
        .map(|rows| AlbumPage {
            rows: rows.to_vec(),
            number: 0,
            left: true,
            empty: false,
        })
        .collect();
    if pages.is_empty() {
        pages.push(AlbumPage {
            rows: Vec::new(),
            number: 0,
            left: true,
            empty: true,
        });
    }
    if pages.len() % 2 == 1 {
        pages.push(AlbumPage {
            rows: Vec::new(),
            number: 0,
            left: false,
            empty: false,
        });
    }
    for (i, page) in pages.iter_mut().enumerate() {
        page.number = i + 1;
        page.left = i % 2 == 0;
    }
    pages
}

/// Album ids that have production's hand-painted art (`REAL_ART_IDS`); every earned achievement
/// production can produce is in this set.
pub fn has_real_art(id: &str) -> bool {
    id != "rock-current"
        && (PET_ROCK_MILESTONES.iter().any(|m| m.0 == id)
            || FOSSIL_MILESTONES
                .iter()
                .any(|m| format!("fossil-{}", m.0) == id)
            || id.starts_with("garden-")
            || matches!(
                id,
                "full-house"
                    | "first-break"
                    | "on-fire"
                    | "early-bird"
                    | "night-owl"
                    | "speedrunner"
                    | "explorer"
                    | "perfectionist"
                    | "veteran"
            ))
}

/// Everything the Break Room reads from the academic domain for one local day, computed with
/// production's own metric functions (`metrics.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AcademicInputs {
    /// `getTodayMinutes`: every session kind that ended today (tokens and "XP").
    pub today_minutes: u64,
    /// Speedrunner's condition: a study/exam session that ended today and lasted >= 45 minutes.
    pub earned_token_in_one_session: bool,
    /// `lifetimeStudyMinutes` (Focus Fossil).
    pub lifetime_minutes: u64,
    pub garden: GardenInputs,
}

pub fn academic_inputs(
    academic: &crate::academic::AcademicState,
    today: CivilDate,
    clock: &dyn LocalClock,
) -> AcademicInputs {
    use crate::academic::SessionKind;
    use crate::dashboard::metrics::SessionDays;
    let days = SessionDays::new(&academic.sessions, clock);
    let study = |kind: SessionKind| matches!(kind, SessionKind::Study | SessionKind::Exam);
    // `getWeeklyCourseCount`: production takes the *UTC* date prefix of `endedAt`'s ISO string,
    // reads it as local midnight, and keeps sessions on or after local midnight of today minus
    // exactly 6 x 24 h.
    let cutoff = clock.local_midnight(today).unix_millis - 6 * 86_400_000;
    let mut courses: Vec<&str> = Vec::new();
    for session in &academic.sessions {
        let Some(course) = &session.course_id else {
            continue;
        };
        if !study(session.kind) {
            continue;
        }
        let utc_day = CivilDate::from_days(session.ended_at.unix_millis.div_euclid(86_400_000));
        if clock.local_midnight(utc_day).unix_millis >= cutoff
            && !courses.contains(&course.as_str())
        {
            courses.push(course.as_str());
        }
    }
    AcademicInputs {
        today_minutes: days.minutes_on(today),
        earned_token_in_one_session: academic
            .sessions
            .iter()
            .zip(&days.ended_day)
            .any(|(s, d)| study(s.kind) && *d == today && s.minutes >= 45),
        lifetime_minutes: academic.lifetime_study_minutes,
        garden: GardenInputs {
            study_session_count: academic.sessions.iter().filter(|s| study(s.kind)).count(),
            streak_days: days.streak_days(today),
            weekly_course_count: courses.len(),
            weekly_total_minutes: days.weekly_activity(today).iter().map(|b| b.1).sum(),
            completed_task_count: academic
                .tasks
                .iter()
                .filter(|t| t.total_units > 0 && t.completed_units >= t.total_units)
                .count(),
        },
    }
}
