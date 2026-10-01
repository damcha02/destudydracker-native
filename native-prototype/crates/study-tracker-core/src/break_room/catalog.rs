//! The Break Room game catalog (`studyBreakGames` in `desktop/src/App.tsx`, v0.1.67): six games,
//! in production's order, identified everywhere in production state by their display **name**
//! (`unlockedGames`, `playedBreaks[].name`, `playedGamesAllTime` all hold names).

/// Whether the native build can run the game itself yet (Stage 20 scope). The catalog, unlocking,
/// "played" logging and achievements treat all six alike, exactly like production.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// Fully local; implemented natively.
    Local,
    /// Travle: the border-route map game, migrated in Stage 21.
    MapStage21,
    /// Daily Skribbl: theme, upload, gallery and votes all live on the social Worker (Stage 22).
    NetworkStage22,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GameId {
    DailyDurak,
    Wordle,
    Travle,
    Flaggle,
    DailySkribbl,
    Geodle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameInfo {
    pub id: GameId,
    /// Production's display name, which is also the persisted identifier.
    pub name: &'static str,
    pub desc: &'static str,
    pub availability: Availability,
}

/// `studyBreakGames`, in production order.
pub const GAMES: [GameInfo; 6] = [
    GameInfo {
        id: GameId::DailyDurak,
        name: "Daily Durak",
        desc: "Solve today's Durak endgame puzzle",
        availability: Availability::Local,
    },
    GameInfo {
        id: GameId::Wordle,
        name: "Wordle",
        desc: "Guess the 5-letter word in 6 tries",
        availability: Availability::Local,
    },
    GameInfo {
        id: GameId::Travle,
        name: "Travle",
        desc: "Build a border route between countries",
        availability: Availability::MapStage21,
    },
    GameInfo {
        id: GameId::Flaggle,
        name: "Flaggle",
        desc: "Guess the flag from shared colors",
        availability: Availability::Local,
    },
    GameInfo {
        id: GameId::DailySkribbl,
        name: "Daily Skribbl",
        desc: "Draw today's theme, then vote on the gallery",
        availability: Availability::NetworkStage22,
    },
    GameInfo {
        id: GameId::Geodle,
        name: "Geodle",
        desc: "Guess the country from geography clues",
        availability: Availability::Local,
    },
];

/// `STUDY_BREAK_GAME_COUNT`.
pub const GAME_COUNT: usize = GAMES.len();

impl GameId {
    pub fn info(self) -> &'static GameInfo {
        GAMES
            .iter()
            .find(|g| g.id == self)
            .expect("every id is in the catalog")
    }

    pub fn name(self) -> &'static str {
        self.info().name
    }

    pub fn from_name(name: &str) -> Option<Self> {
        GAMES.iter().find(|g| g.name == name).map(|g| g.id)
    }

    /// Position in production's card order.
    pub fn index(self) -> usize {
        GAMES
            .iter()
            .position(|g| g.id == self)
            .expect("every id is in the catalog")
    }
}

const QUOTES: &str = include_str!("../../data/break_room/quotes.tsv");
const STRETCHES: &str = include_str!("../../data/break_room/stretches.txt");

/// `breakQuotes` (text, author), production order; one is picked at random per launch.
pub fn quotes() -> &'static [(&'static str, &'static str)] {
    static LIST: std::sync::OnceLock<Vec<(&'static str, &'static str)>> =
        std::sync::OnceLock::new();
    LIST.get_or_init(|| {
        QUOTES
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .collect()
    })
}

/// `stretchIdeas`, production order.
pub fn stretch_ideas() -> &'static [&'static str] {
    static LIST: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    LIST.get_or_init(|| STRETCHES.lines().filter(|l| !l.is_empty()).collect())
}
