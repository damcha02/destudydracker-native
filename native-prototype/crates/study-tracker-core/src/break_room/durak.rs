//! Daily Durak, ported line by line from `desktop/src/lib/durak.ts` and the `App.tsx` handlers
//! (`initDurakPuzzle`, `handleDurak*`, `resetDurakAfterFail`, `saveDurakState`).
//!
//! It is an *endgame puzzle*, not a full game: a 36-card deck (6..A), no drawing from a stock, the
//! player against a deterministic CPU. Production's own simplifications are kept on purpose:
//! - the CPU only ever takes **one** step after each player action (`processCpuTurn` once);
//! - "throw-in" limits are `min(6, cards the defender can still cover)`;
//! - "slide" (perevodnoy) is allowed only before any defence is on the table, never with the
//!   slider's last card, and only if the receiver could cover every undefended card;
//! - winning: the player wins when their hand empties (even if both hands empty), the CPU wins
//!   when its hand empties while the player still holds cards, or when it has nothing to attack.
//! - Each day has three puzzles (`<date>_0`, `_1`, `_2`), each found by a seeded search that keeps
//!   only deals the depth-limited solver proves winnable.
//!
//! The only nondeterminism in production is the hint (`Math.random()` over up to four candidate
//! hints). Native takes the random number from the caller ([`find_daily_puzzle`]'s `pick`), so
//! tests are reproducible and the candidate list is exactly production's.

use std::collections::HashSet;
use std::fmt;

use crate::appearance::sakura::Mulberry32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

pub const SUITS: [Suit; 4] = [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades];

impl Suit {
    pub fn symbol(self) -> char {
        match self {
            Suit::Hearts => '♥',
            Suit::Diamonds => '♦',
            Suit::Clubs => '♣',
            Suit::Spades => '♠',
        }
    }
    /// Production's `trumpSuit` string.
    pub fn id(self) -> &'static str {
        match self {
            Suit::Hearts => "hearts",
            Suit::Diamonds => "diamonds",
            Suit::Clubs => "clubs",
            Suit::Spades => "spades",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        SUITS.into_iter().find(|s| s.id() == id)
    }
    pub fn from_symbol(c: char) -> Option<Self> {
        SUITS.into_iter().find(|s| s.symbol() == c)
    }
    /// `SUIT_COLOR`: hearts/diamonds red, clubs/spades dark.
    pub fn is_red(self) -> bool {
        matches!(self, Suit::Hearts | Suit::Diamonds)
    }
}

/// Ordered 6 < 7 < ... < A (`RANK_ORDER`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rank {
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
    Ace,
}

pub const RANKS: [Rank; 9] = [
    Rank::Six,
    Rank::Seven,
    Rank::Eight,
    Rank::Nine,
    Rank::Ten,
    Rank::Jack,
    Rank::Queen,
    Rank::King,
    Rank::Ace,
];

impl Rank {
    pub fn label(self) -> &'static str {
        match self {
            Rank::Six => "6",
            Rank::Seven => "7",
            Rank::Eight => "8",
            Rank::Nine => "9",
            Rank::Ten => "10",
            Rank::Jack => "J",
            Rank::Queen => "Q",
            Rank::King => "K",
            Rank::Ace => "A",
        }
    }
    pub fn from_label(label: &str) -> Option<Self> {
        RANKS.into_iter().find(|r| r.label() == label)
    }
    fn order(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}

impl Card {
    pub const fn new(rank: Rank, suit: Suit) -> Self {
        Self { suit, rank }
    }
    /// `cardKey` / `cardToString`: `"10♥"`.
    pub fn key(self) -> String {
        format!("{}{}", self.rank.label(), self.suit.symbol())
    }
    /// `parseCardKey`.
    pub fn parse(key: &str) -> Option<Self> {
        let suit_char = key.chars().last()?;
        let suit = Suit::from_symbol(suit_char)?;
        let rank = Rank::from_label(&key[..key.len() - suit_char.len_utf8()])?;
        Some(Card { suit, rank })
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.rank.label(), self.suit.symbol())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Player,
    Cpu,
}

impl Side {
    pub fn id(self) -> &'static str {
        match self {
            Side::Player => "player",
            Side::Cpu => "cpu",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "player" => Some(Side::Player),
            "cpu" => Some(Side::Cpu),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableEntry {
    pub attack: Card,
    pub defense: Option<Card>,
    pub attack_by: Side,
    pub defense_by: Option<Side>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    PlayerAttack,
    PlayerDefense,
    PlayerThrow,
    CpuDefense,
    CpuAttack,
    CpuThrow,
    Finished,
}

impl Phase {
    pub fn id(self) -> &'static str {
        match self {
            Phase::PlayerAttack => "player_attack",
            Phase::PlayerDefense => "player_defense",
            Phase::PlayerThrow => "player_throw",
            Phase::CpuDefense => "cpu_defense",
            Phase::CpuAttack => "cpu_attack",
            Phase::CpuThrow => "cpu_throw",
            Phase::Finished => "finished",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        [
            Phase::PlayerAttack,
            Phase::PlayerDefense,
            Phase::PlayerThrow,
            Phase::CpuDefense,
            Phase::CpuAttack,
            Phase::CpuThrow,
            Phase::Finished,
        ]
        .into_iter()
        .find(|p| p.id() == id)
    }
}

/// `DurakGameState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameState {
    pub player_hand: Vec<Card>,
    pub cpu_hand: Vec<Card>,
    pub table: Vec<TableEntry>,
    pub trump: Suit,
    pub discard: Vec<Card>,
    pub phase: Phase,
    pub winner: Option<Side>,
    pub message: String,
}

fn join(cards: &[Card]) -> String {
    cards.iter().map(|c| c.key()).collect::<Vec<_>>().join(", ")
}

/// `canBeat`.
pub fn can_beat(card: Card, target: Card, trump: Suit) -> bool {
    if card.suit == target.suit {
        return card.rank.order() > target.rank.order();
    }
    card.suit == trump && target.suit != trump
}

fn remove_card(hand: &mut Vec<Card>, card: Card) {
    if let Some(i) = hand.iter().position(|c| *c == card) {
        hand.remove(i);
    }
}

fn check_win(s: &mut GameState) {
    if s.player_hand.is_empty() && !s.cpu_hand.is_empty() {
        s.phase = Phase::Finished;
        s.winner = Some(Side::Player);
        s.message = "You win! The CPU still has cards.".into();
    } else if s.cpu_hand.is_empty() && !s.player_hand.is_empty() {
        s.phase = Phase::Finished;
        s.winner = Some(Side::Cpu);
        s.message = "CPU wins! CPU has no cards left.".into();
    } else if s.player_hand.is_empty() && s.cpu_hand.is_empty() {
        s.phase = Phase::Finished;
        s.winner = Some(Side::Player);
        s.message = "You win! Both hands are empty.".into();
    }
}

fn pick_up_cards(state: &GameState, target: Side) -> GameState {
    let mut next = state.clone();
    let mut table_cards = Vec::new();
    for entry in &next.table {
        table_cards.push(entry.attack);
        if let Some(d) = entry.defense {
            table_cards.push(d);
        }
    }
    match target {
        Side::Player => next.player_hand.extend(table_cards),
        Side::Cpu => next.cpu_hand.extend(table_cards),
    }
    next.table.clear();
    if target == Side::Cpu {
        next.phase = Phase::PlayerAttack;
        next.message = "CPU picked up cards. Your turn to attack.".into();
    } else {
        next.phase = Phase::CpuAttack;
        next.message = "You picked up cards. CPU's turn to attack.".into();
    }
    check_win(&mut next);
    next
}

/// `getAttackLimitAgainstCpu`.
pub fn attack_limit_against_cpu(state: &GameState) -> usize {
    let cpu_defended = state
        .table
        .iter()
        .filter(|e| e.defense_by == Some(Side::Cpu))
        .count();
    6.min(state.cpu_hand.len() + cpu_defended)
}

fn cpu_is_picking_up(state: &GameState) -> bool {
    state.phase == Phase::PlayerThrow && state.table.iter().any(|e| e.defense.is_none())
}

fn finish_cpu_pickup(state: &GameState) -> GameState {
    let mut next = pick_up_cards(state, Side::Cpu);
    if next.phase != Phase::Finished {
        next.phase = Phase::PlayerAttack;
        next.message = "CPU picked up cards. Your turn to attack.".into();
    }
    next
}

fn start_cpu_pickup(state: &GameState) -> GameState {
    let mut next = state.clone();
    next.phase = Phase::PlayerThrow;
    next.message = "CPU picks up. You may throw in matching ranks, or pass.".into();
    next
}

fn clear_table(state: &GameState) -> GameState {
    let mut next = state.clone();
    for entry in std::mem::take(&mut next.table) {
        next.discard.push(entry.attack);
        if let Some(d) = entry.defense {
            next.discard.push(d);
        }
    }
    next
}

/// Groups of equal rank, in the order each rank first appears in the hand (a JS `Map`).
fn rank_groups(hand: &[Card]) -> Vec<Vec<Card>> {
    let mut groups: Vec<Vec<Card>> = Vec::new();
    for &card in hand {
        match groups.iter_mut().find(|g| g[0].rank == card.rank) {
            Some(group) => group.push(card),
            None => groups.push(vec![card]),
        }
    }
    groups
}

/// `getValidAttacks`: one option per rank, all its cards (capped at `max`).
pub fn valid_attacks(hand: &[Card], max: Option<usize>) -> Vec<Vec<Card>> {
    if max == Some(0) {
        return Vec::new();
    }
    rank_groups(hand)
        .into_iter()
        .map(|mut g| {
            if let Some(m) = max {
                g.truncate(m);
            }
            g
        })
        .collect()
}

/// `getValidThrows`: for every rank on the table (attack ranks then defence ranks, in table
/// order), the hand's cards of that rank, capped at the room left under `max_total`.
pub fn valid_throws(hand: &[Card], table: &[TableEntry], max_total: usize) -> Vec<Vec<Card>> {
    let mut ranks: Vec<Rank> = Vec::new();
    for entry in table {
        for rank in std::iter::once(entry.attack.rank).chain(entry.defense.map(|d| d.rank)) {
            if !ranks.contains(&rank) {
                ranks.push(rank);
            }
        }
    }
    let max_add = max_total.saturating_sub(table.len());
    if max_add == 0 {
        return Vec::new();
    }
    ranks
        .into_iter()
        .filter_map(|rank| {
            let mut cards: Vec<Card> = hand.iter().copied().filter(|c| c.rank == rank).collect();
            if cards.is_empty() {
                None
            } else {
                cards.truncate(max_add);
                Some(cards)
            }
        })
        .collect()
}

/// `getDefenseOptions`.
pub fn defense_options(hand: &[Card], attack: Card, trump: Suit) -> Vec<Card> {
    hand.iter()
        .copied()
        .filter(|c| can_beat(*c, attack, trump))
        .collect()
}

/// `getBestDefense`: the lowest non-trump that beats it, else the lowest trump. Ties keep hand
/// order (`Array.prototype.sort` is stable).
pub fn best_defense(hand: &[Card], attack: Card, trump: Suit) -> Option<Card> {
    let options = defense_options(hand, attack, trump);
    if options.is_empty() {
        return None;
    }
    let non_trump: Vec<Card> = options
        .iter()
        .copied()
        .filter(|c| c.suit != trump)
        .collect();
    let pool = if non_trump.is_empty() {
        options
    } else {
        non_trump
    };
    pool.into_iter().min_by_key(|c| c.rank.order()) // min_by_key keeps the first minimum
}

/// `getLegalSlideCards`.
pub fn legal_slide_cards(state: &GameState, slide_by: Side) -> Vec<Card> {
    let (hand, receiver) = match slide_by {
        Side::Player => (&state.player_hand, &state.cpu_hand),
        Side::Cpu => (&state.cpu_hand, &state.player_hand),
    };
    let undefended_after = state.table.iter().filter(|e| e.defense.is_none()).count() + 1;
    if hand.len() <= 1 || receiver.len() < undefended_after {
        return Vec::new();
    }
    if state.table.iter().any(|e| e.defense.is_some()) {
        return Vec::new();
    }
    let ranks: Vec<Rank> = state.table.iter().map(|e| e.attack.rank).collect();
    hand.iter()
        .copied()
        .filter(|c| ranks.contains(&c.rank))
        .collect()
}

/// `executeSlide`.
pub fn execute_slide(state: &GameState, card: Card) -> GameState {
    let mut next = state.clone();
    if next.phase != Phase::CpuDefense && next.phase != Phase::PlayerDefense {
        return next;
    }
    let slide_by = if next.phase == Phase::CpuDefense {
        Side::Cpu
    } else {
        Side::Player
    };
    if !legal_slide_cards(&next, slide_by).contains(&card) {
        return next;
    }
    next.table.push(TableEntry {
        attack: card,
        defense: None,
        attack_by: slide_by,
        defense_by: None,
    });
    if slide_by == Side::Cpu {
        remove_card(&mut next.cpu_hand, card);
        next.phase = Phase::PlayerDefense;
        next.message = format!("CPU slides with {card}! You must defend.");
    } else {
        remove_card(&mut next.player_hand, card);
        next.phase = Phase::CpuDefense;
        next.message = format!("You slide with {card}! CPU must defend.");
    }
    check_win(&mut next);
    next
}

fn cpu_defend(state: &GameState) -> GameState {
    let mut next = state.clone();
    for i in 0..next.table.len() {
        if next.table[i].defense.is_some() {
            continue;
        }
        match best_defense(&next.cpu_hand, next.table[i].attack, next.trump) {
            Some(best) => {
                next.table[i].defense = Some(best);
                next.table[i].defense_by = Some(Side::Cpu);
                remove_card(&mut next.cpu_hand, best);
            }
            None => return start_cpu_pickup(&next),
        }
    }
    next.message = "CPU defended all cards.".into();
    next.phase = Phase::PlayerThrow;
    check_win(&mut next);
    next
}

fn cpu_attack(state: &GameState) -> GameState {
    let mut next = state.clone();
    let mut attacks = valid_attacks(&next.cpu_hand, Some(6.min(next.player_hand.len())));
    if attacks.is_empty() {
        next.phase = Phase::Finished;
        next.winner = Some(Side::Player);
        next.message = "CPU has no cards to attack. You win!".into();
        return next;
    }
    // most cards first, then the lowest rank; stable for ties
    attacks.sort_by(|a, b| {
        b.len()
            .cmp(&a.len())
            .then(a[0].rank.order().cmp(&b[0].rank.order()))
    });
    let chosen = attacks.swap_remove(0);
    next.table = chosen
        .iter()
        .map(|&c| TableEntry {
            attack: c,
            defense: None,
            attack_by: Side::Cpu,
            defense_by: None,
        })
        .collect();
    for &c in &chosen {
        remove_card(&mut next.cpu_hand, c);
    }
    next.phase = Phase::PlayerDefense;
    next.message = format!("CPU attacks with {}. Defend or pick up.", join(&chosen));
    check_win(&mut next);
    next
}

fn cpu_throw_cards(state: &GameState) -> GameState {
    let mut next = state.clone();
    let max_total = 6.min(next.table.len() + next.player_hand.len());
    let throws = valid_throws(&next.cpu_hand, &next.table, max_total);
    if let Some(chosen) = throws.into_iter().next() {
        for &c in &chosen {
            next.table.push(TableEntry {
                attack: c,
                defense: None,
                attack_by: Side::Cpu,
                defense_by: None,
            });
            remove_card(&mut next.cpu_hand, c);
        }
        next.phase = Phase::PlayerDefense;
        next.message = format!("CPU throws in {}. Defend!", join(&chosen));
    } else {
        next = clear_table(&next);
        next.phase = Phase::PlayerAttack;
        next.message = "CPU passes. Your turn to attack.".into();
    }
    check_win(&mut next);
    next
}

/// `executePlayerAttack`.
pub fn player_attack(state: &GameState, cards: &[Card]) -> GameState {
    let mut next = state.clone();
    if cards.is_empty() {
        return next;
    }
    let max = 6.min(next.cpu_hand.len());
    let used: Vec<Card> = cards.iter().copied().take(max).collect();
    for &c in &used {
        next.table.push(TableEntry {
            attack: c,
            defense: None,
            attack_by: Side::Player,
            defense_by: None,
        });
        remove_card(&mut next.player_hand, c);
    }
    next.phase = Phase::CpuDefense;
    next.message = format!("You attack with {}.", join(&used));
    check_win(&mut next);
    next
}

/// `executePlayerThrow`.
pub fn player_throw(state: &GameState, cards: &[Card]) -> GameState {
    let mut next = state.clone();
    let was_picking_up = cpu_is_picking_up(&next);
    let max_add = attack_limit_against_cpu(&next).saturating_sub(next.table.len());
    let used: Vec<Card> = cards.iter().copied().take(max_add).collect();
    if used.is_empty() {
        return next;
    }
    for &c in &used {
        next.table.push(TableEntry {
            attack: c,
            defense: None,
            attack_by: Side::Player,
            defense_by: None,
        });
        remove_card(&mut next.player_hand, c);
    }
    if was_picking_up {
        let limit = attack_limit_against_cpu(&next);
        let throws = valid_throws(&next.player_hand, &next.table, limit);
        if throws.is_empty() || next.table.len() >= limit {
            return finish_cpu_pickup(&next);
        }
        next.phase = Phase::PlayerThrow;
        next.message = format!("You throw in {}. CPU will pick them up too.", join(&used));
    } else {
        next.phase = Phase::CpuDefense;
        next.message = format!("You throw in {}.", join(&used));
    }
    check_win(&mut next);
    next
}

/// `defendOneCard`: covers the first undefended attack if `card` beats it.
pub fn defend_one_card(state: &GameState, card: Card) -> GameState {
    let mut next = state.clone();
    let Some(target) = next.table.iter().position(|e| e.defense.is_none()) else {
        return next;
    };
    if !can_beat(card, next.table[target].attack, next.trump) {
        return next;
    }
    next.table[target].defense = Some(card);
    next.table[target].defense_by = Some(Side::Player);
    remove_card(&mut next.player_hand, card);
    match next.table.iter().find(|e| e.defense.is_none()) {
        None => {
            next.phase = Phase::CpuThrow;
            next.message = "You defended all cards!".into();
        }
        Some(remaining) => {
            next.message = format!("Defend {} — select a card to beat it.", remaining.attack);
        }
    }
    check_win(&mut next);
    next
}

/// `playerPassThrow`.
pub fn player_pass_throw(state: &GameState) -> GameState {
    if cpu_is_picking_up(state) {
        return finish_cpu_pickup(state);
    }
    let mut next = clear_table(state);
    next.phase = Phase::CpuAttack;
    next.message = "CPU's turn to attack.".into();
    check_win(&mut next);
    next
}

/// `playerPickUp`.
pub fn player_pick_up(state: &GameState) -> GameState {
    pick_up_cards(state, Side::Player)
}

/// `processCpuTurn`: exactly one CPU step for the current phase.
pub fn process_cpu_turn(state: &GameState) -> GameState {
    match state.phase {
        Phase::CpuDefense => {
            let slides = legal_slide_cards(state, Side::Cpu);
            if let Some(&first) = slides.first() {
                return execute_slide(state, first);
            }
            cpu_defend(state)
        }
        Phase::CpuAttack => cpu_attack(state),
        Phase::CpuThrow => cpu_throw_cards(state),
        _ => state.clone(),
    }
}

/// `generateHint`'s candidate list, in production order; production then picks
/// `hints[floor(Math.random() * len)]`.
pub fn hint_candidates(cpu_hand: &[Card], trump: Suit) -> Vec<String> {
    let mut hints = Vec::new();
    let trump_count = cpu_hand.iter().filter(|c| c.suit == trump).count();
    if trump_count > 0 {
        hints.push(format!(
            "CPU has {trump_count} trump card{}",
            if trump_count > 1 { "s" } else { "" }
        ));
    }
    if let Some(ace) = cpu_hand.iter().find(|c| c.rank == Rank::Ace) {
        hints.push(format!("CPU holds {ace}"));
    }
    let present: Vec<Suit> = cpu_hand.iter().map(|c| c.suit).collect();
    let missing: String = SUITS
        .iter()
        .filter(|s| !present.contains(s) && **s != trump)
        .map(|s| s.symbol())
        .collect();
    if !missing.is_empty() {
        hints.push(format!("CPU has no {missing}"));
    }
    if let Some(lowest) = cpu_hand.iter().min_by_key(|c| c.rank.order()) {
        hints.push(format!(
            "CPU's lowest card is {}{}",
            lowest.rank.label(),
            lowest.suit.symbol()
        ));
    }
    if hints.is_empty() {
        hints.push(format!("CPU has {} cards", cpu_hand.len()));
    }
    hints
}

/// `solver`: production's depth-limited DFS (depth 20, throw branching cut after depth 3, the CPU
/// side deterministic) - "found" means this search proves a win, which is what decides whether a
/// generated deal is used.
pub fn solve(state: &GameState) -> bool {
    let mut visited = HashSet::new();
    dfs(state, 0, &mut visited)
}

fn state_key(s: &GameState) -> String {
    let sorted = |hand: &[Card]| {
        let mut h = hand.to_vec();
        // RANK_ORDER, then suit name (localeCompare on ascii suit names = byte order)
        h.sort_by(|a, b| {
            a.rank
                .order()
                .cmp(&b.rank.order())
                .then(a.suit.id().cmp(b.suit.id()))
        });
        h.iter().map(|c| c.key()).collect::<Vec<_>>().join(",")
    };
    let table: Vec<String> = s
        .table
        .iter()
        .map(|e| {
            let by = &e.attack_by.id()[..1];
            match e.defense {
                Some(d) => format!(
                    "{by}{}>{}{}",
                    e.attack,
                    e.defense_by.map_or("?", |s| &s.id()[..1]),
                    d
                ),
                None => format!("{by}{}?", e.attack),
            }
        })
        .collect();
    format!(
        "{}|{}|{}|{}",
        s.phase.id(),
        sorted(&s.player_hand),
        sorted(&s.cpu_hand),
        table.join("|")
    )
}

fn dfs(s: &GameState, depth: u32, visited: &mut HashSet<String>) -> bool {
    if depth > 20 {
        return false;
    }
    if !visited.insert(state_key(s)) {
        return false;
    }
    match s.winner {
        Some(Side::Player) => return true,
        Some(Side::Cpu) => return false,
        None => {}
    }
    match s.phase {
        Phase::PlayerAttack => {
            for attack in valid_attacks(&s.player_hand, Some(6.min(s.cpu_hand.len()))) {
                if dfs(&player_attack(s, &attack), depth + 1, visited) {
                    return true;
                }
            }
            false
        }
        Phase::CpuDefense => {
            let slides = legal_slide_cards(s, Side::Cpu);
            if let Some(&first) = slides.first() {
                return dfs(&execute_slide(s, first), depth + 1, visited);
            }
            dfs(&cpu_defend(s), depth + 1, visited)
        }
        Phase::PlayerThrow => {
            let max_total = attack_limit_against_cpu(s);
            let throws = valid_throws(&s.player_hand, &s.table, max_total);
            if throws.is_empty() || depth > 3 {
                let picking_up = cpu_is_picking_up(s);
                let mut n = if picking_up {
                    finish_cpu_pickup(s)
                } else {
                    clear_table(s)
                };
                if !picking_up && n.phase != Phase::Finished {
                    n.phase = Phase::CpuAttack;
                }
                if dfs(&n, depth + 1, visited) {
                    return true;
                }
            }
            for throw in throws {
                if dfs(&player_throw(s, &throw), depth + 1, visited) {
                    return true;
                }
            }
            false
        }
        Phase::CpuAttack => dfs(&cpu_attack(s), depth + 1, visited),
        Phase::PlayerDefense => {
            let Some(target) = s.table.iter().position(|e| e.defense.is_none()) else {
                let mut n = s.clone();
                n.phase = Phase::CpuThrow;
                return dfs(&n, depth + 1, visited);
            };
            for option in defense_options(&s.player_hand, s.table[target].attack, s.trump) {
                let mut n = s.clone();
                n.table[target].defense = Some(option);
                n.table[target].defense_by = Some(Side::Player);
                remove_card(&mut n.player_hand, option);
                if dfs(&n, depth + 1, visited) {
                    return true;
                }
            }
            if dfs(&pick_up_cards(s, Side::Player), depth + 1, visited) {
                return true;
            }
            for card in legal_slide_cards(s, Side::Player) {
                if dfs(&execute_slide(s, card), depth + 1, visited) {
                    return true;
                }
            }
            false
        }
        Phase::CpuThrow => dfs(&cpu_throw_cards(s), depth + 1, visited),
        Phase::Finished => s.winner == Some(Side::Player),
    }
}

/// `hashDate`: Java-style `hash * 31 + ch` over UTF-16 units with int32 wrap, then `Math.abs`
/// (as a double, so `-2^31` becomes `2^31`).
fn hash_date(date: &str) -> i64 {
    let mut hash: i32 = 0;
    for unit in date.encode_utf16() {
        hash = hash
            .wrapping_shl(5)
            .wrapping_sub(hash)
            .wrapping_add(i32::from(unit));
    }
    i64::from(hash).abs()
}

fn shuffle(deck: &[Card], rng: &mut Mulberry32) -> Vec<Card> {
    let mut result = deck.to_vec();
    for i in (1..result.len()).rev() {
        let j = (rng.next_f64() * (i + 1) as f64).floor() as usize;
        result.swap(i, j);
    }
    result
}

/// A generated daily puzzle (`DailyPuzzle`). `hint_candidates` is what production chose its hint
/// from; `hint` is the one picked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyPuzzle {
    pub seed: String,
    pub initial: GameState,
    pub hint_candidates: Vec<String>,
    pub hint: String,
}

pub const INITIAL_MESSAGE: &str =
    "Your turn to attack. Select cards of the same rank and play them.";

/// `findDailyPuzzle(seed)`: up to 200 seeded deals; the first the solver proves winnable.
/// `pick` stands in for production's `Math.random()` (a value in `[0, 1)`) for the hint choice.
pub fn find_daily_puzzle(seed: &str, pick: f64) -> Option<DailyPuzzle> {
    let base = hash_date(seed);
    let mut deck = Vec::with_capacity(36);
    for suit in SUITS {
        for rank in RANKS {
            deck.push(Card::new(rank, suit));
        }
    }
    for attempt in 0..200i64 {
        // `seed |= 0` inside mulberry32 truncates the double to 32 bits; wrapping is equivalent.
        let mut rng = Mulberry32::new((base + attempt * 7919) as u32);
        let shuffled = shuffle(&deck, &mut rng);
        let trump = shuffled[0].suit;
        let dealt = shuffle(&shuffled, &mut rng);
        let player_count = 5 + (rng.next_f64() * 3.0).floor() as usize;
        let cpu_count = 3 + (rng.next_f64() * 2.0).floor() as usize;
        let mut player: Vec<Card> = dealt[..player_count].to_vec();
        let mut cpu: Vec<Card> = dealt[player_count..player_count + cpu_count].to_vec();
        // player: highest rank first; cpu: lowest first (stable)
        player.sort_by(|a, b| b.rank.order().cmp(&a.rank.order()));
        cpu.sort_by(|a, b| a.rank.order().cmp(&b.rank.order()));
        let (highest, lowest) = (player[0], cpu[0]);
        if highest.rank.order() > lowest.rank.order() {
            // swap the player's best card for the CPU's worst
            player[0] = lowest;
            cpu[0] = highest;
        }
        let state = GameState {
            player_hand: player,
            cpu_hand: cpu,
            table: Vec::new(),
            trump,
            discard: Vec::new(),
            phase: Phase::PlayerAttack,
            winner: None,
            message: INITIAL_MESSAGE.into(),
        };
        if solve(&state) {
            let candidates = hint_candidates(&state.cpu_hand, trump);
            let index =
                ((pick.clamp(0.0, 1.0 - f64::EPSILON)) * candidates.len() as f64).floor() as usize;
            let hint = candidates[index.min(candidates.len() - 1)].clone();
            return Some(DailyPuzzle {
                seed: seed.to_string(),
                initial: state,
                hint_candidates: candidates,
                hint,
            });
        }
    }
    None
}

/// One table entry as production persists it (`DurakPuzzleState.table[]`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StoredEntry {
    pub attack: String,
    pub defense: Option<String>,
    pub attack_by: Option<String>,
    pub defense_by: Option<String>,
}

/// `DurakPuzzleState`: the game serialized with card keys, plus the day's bookkeeping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurakPuzzle {
    /// `"<date>_<0|1|2>"`, or `None` before the first puzzle / after a day reset.
    pub seed: Option<String>,
    pub hint: String,
    pub player_hand: Vec<String>,
    pub cpu_hand: Vec<String>,
    pub trump_suit: String,
    pub table: Vec<StoredEntry>,
    pub discard_pile: Vec<String>,
    pub phase: String,
    pub winner: Option<String>,
    pub message: String,
    pub failures: u32,
    pub completed: bool,
    pub solved_count: u32,
}

impl Default for DurakPuzzle {
    /// `defaultState.durakPuzzle`.
    fn default() -> Self {
        Self {
            seed: None,
            hint: String::new(),
            player_hand: Vec::new(),
            cpu_hand: Vec::new(),
            trump_suit: "hearts".into(),
            table: Vec::new(),
            discard_pile: Vec::new(),
            phase: "player_attack".into(),
            winner: None,
            message: String::new(),
            failures: 0,
            completed: false,
            solved_count: 0,
        }
    }
}

/// Puzzles per day.
pub const DAILY_PUZZLES: u32 = 3;

impl DurakPuzzle {
    /// `gameStateToPuzzle` merged over the existing record (`{...s.durakPuzzle, ...serialized}`),
    /// which keeps `solvedCount`.
    pub fn store(
        &mut self,
        game: &GameState,
        failures: u32,
        completed: bool,
        hint: &str,
        seed: &str,
    ) {
        self.seed = Some(seed.to_string());
        self.hint = hint.to_string();
        self.player_hand = game.player_hand.iter().map(|c| c.key()).collect();
        self.cpu_hand = game.cpu_hand.iter().map(|c| c.key()).collect();
        self.trump_suit = game.trump.id().to_string();
        self.table = game
            .table
            .iter()
            .map(|e| StoredEntry {
                attack: e.attack.key(),
                defense: e.defense.map(|d| d.key()),
                attack_by: Some(e.attack_by.id().to_string()),
                defense_by: e.defense_by.map(|s| s.id().to_string()),
            })
            .collect();
        self.discard_pile = game.discard.iter().map(|c| c.key()).collect();
        self.phase = game.phase.id().to_string();
        self.winner = game.winner.map(|w| w.id().to_string());
        self.message = game.message.clone();
        self.failures = failures;
        self.completed = completed;
    }

    /// `puzzleToGameState`: `None` for anything production would reject (an unparsable card, an
    /// unknown suit/phase/side). A legacy entry without `attackBy` is the CPU's when it is the
    /// first one, the player's otherwise, like production.
    pub fn to_game_state(&self) -> Option<GameState> {
        let parse_all = |keys: &[String]| {
            keys.iter()
                .map(|k| Card::parse(k))
                .collect::<Option<Vec<_>>>()
        };
        let player_hand = parse_all(&self.player_hand)?;
        let cpu_hand = parse_all(&self.cpu_hand)?;
        let discard = parse_all(&self.discard_pile)?;
        let mut table = Vec::with_capacity(self.table.len());
        for (i, e) in self.table.iter().enumerate() {
            let attack = Card::parse(&e.attack)?;
            let defense = match &e.defense {
                Some(d) if !d.is_empty() => Some(Card::parse(d)?),
                _ => None,
            };
            let attack_by = match &e.attack_by {
                Some(id) => Side::from_id(id)?,
                None if i == 0 => Side::Cpu,
                None => Side::Player,
            };
            let defense_by = match &e.defense_by {
                Some(id) if !id.is_empty() => Some(Side::from_id(id)?),
                _ => None,
            };
            table.push(TableEntry {
                attack,
                defense,
                attack_by,
                defense_by,
            });
        }
        let winner = match &self.winner {
            Some(id) if !id.is_empty() => Some(Side::from_id(id)?),
            _ => None,
        };
        Some(GameState {
            player_hand,
            cpu_hand,
            table,
            trump: Suit::from_id(&self.trump_suit)?,
            discard,
            phase: Phase::from_id(&self.phase)?,
            winner,
            message: self.message.clone(),
        })
    }
}

/// What the Durak modal shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurakView<'a> {
    /// `solvedCount >= 3` with no game in memory: "All 3 puzzles solved for today!".
    AllSolved,
    Game(&'a GameState),
    /// No puzzle could be generated (production renders an empty modal).
    Empty,
}

/// The modal's live game plus production's handlers. The stored [`DurakPuzzle`] is the persisted
/// half; `game`/`selected` live only while the app runs, exactly like production's React state.
#[derive(Debug, Clone, Default)]
pub struct DurakSession {
    pub game: Option<GameState>,
    pub selected: Vec<usize>,
}

impl DurakSession {
    /// `initDurakPuzzle`, run each time the modal opens. `pick` replaces `Math.random()` for a
    /// newly generated hint. Returns whether the stored puzzle changed (needs a save).
    pub fn open(&mut self, puzzle: &mut DurakPuzzle, today: &str, pick: f64) -> bool {
        let mut changed = false;
        if puzzle
            .seed
            .as_deref()
            .is_some_and(|s| !s.starts_with(today))
        {
            puzzle.solved_count = 0;
            puzzle.seed = None;
            puzzle.failures = 0;
            changed = true;
        }
        if puzzle.solved_count >= DAILY_PUZZLES {
            return changed;
        }
        let seed = format!("{today}_{}", puzzle.solved_count);
        if self.game.is_some() && puzzle.seed.as_deref() == Some(seed.as_str()) {
            return changed;
        }
        if puzzle.seed.as_deref() == Some(seed.as_str()) {
            if let Some(saved) = puzzle.to_game_state() {
                self.game = Some(saved);
                self.selected.clear();
                return changed;
            }
        }
        let Some(daily) = find_daily_puzzle(&seed, pick) else {
            return changed;
        };
        puzzle.store(&daily.initial, 0, false, &daily.hint, &seed);
        self.game = Some(daily.initial);
        true
    }

    pub fn view<'a>(&'a self, puzzle: &DurakPuzzle) -> DurakView<'a> {
        match &self.game {
            None if puzzle.solved_count >= DAILY_PUZZLES => DurakView::AllSolved,
            None => DurakView::Empty,
            Some(game) => DurakView::Game(game),
        }
    }

    /// `handleDurakCardClick`.
    pub fn click_card(&mut self, index: usize) {
        let Some(game) = &self.game else { return };
        let Some(&card) = game.player_hand.get(index) else {
            return;
        };
        match game.phase {
            Phase::PlayerDefense => {
                let target = game.table.iter().find(|e| e.defense.is_none());
                let beats = target.is_some_and(|t| can_beat(card, t.attack, game.trump));
                let slides = legal_slide_cards(game, Side::Player).contains(&card);
                if !beats && !slides {
                    return;
                }
                self.selected = if self.selected.contains(&index) {
                    Vec::new()
                } else {
                    vec![index]
                };
            }
            Phase::PlayerAttack | Phase::PlayerThrow => {
                if game.phase == Phase::PlayerThrow {
                    let on_table = game.table.iter().any(|e| {
                        e.attack.rank == card.rank || e.defense.is_some_and(|d| d.rank == card.rank)
                    });
                    if !on_table {
                        return;
                    }
                }
                if let Some(pos) = self.selected.iter().position(|&i| i == index) {
                    self.selected.remove(pos);
                    return;
                }
                if self.selected.is_empty() {
                    self.selected = vec![index];
                    return;
                }
                let first = game.player_hand[self.selected[0]];
                if first.rank != card.rank {
                    self.selected = vec![index];
                    return;
                }
                let max = if game.phase == Phase::PlayerAttack {
                    6.min(game.cpu_hand.len())
                } else {
                    attack_limit_against_cpu(game).saturating_sub(game.table.len())
                };
                if self.selected.len() >= max {
                    return;
                }
                self.selected.push(index);
            }
            _ => {}
        }
    }

    fn selected_cards(&self, game: &GameState) -> Vec<Card> {
        self.selected
            .iter()
            .filter_map(|&i| game.player_hand.get(i).copied())
            .collect()
    }

    /// `handleDurakFinished` after a player action and the CPU's single reply.
    fn finish(&mut self, puzzle: &mut DurakPuzzle, next: GameState, today: &str) {
        let seed = puzzle.seed.clone().unwrap_or_else(|| today.to_string());
        let hint = puzzle.hint.clone();
        let failures = puzzle.failures;
        if next.phase != Phase::Finished {
            let completed = puzzle.completed;
            puzzle.store(&next, failures, completed, &hint, &seed);
        } else if next.winner == Some(Side::Player) {
            puzzle.store(&next, failures, true, &hint, &seed);
            puzzle.solved_count += 1;
        } else {
            puzzle.store(&next, failures, false, &hint, &seed);
        }
        self.game = Some(next);
        self.selected.clear();
    }

    /// The "Attack" button. Returns whether anything happened.
    pub fn attack(&mut self, puzzle: &mut DurakPuzzle, today: &str) -> bool {
        let Some(game) = self.game.clone() else {
            return false;
        };
        if self.selected.is_empty() || game.phase != Phase::PlayerAttack {
            return false;
        }
        let cards: Vec<Card> = self
            .selected_cards(&game)
            .into_iter()
            .take(6.min(game.cpu_hand.len()))
            .collect();
        let next = process_cpu_turn(&player_attack(&game, &cards));
        self.finish(puzzle, next, today);
        true
    }

    /// The "Throw" (`pass == false`, with a selection) and "Pass" buttons.
    pub fn throw_or_pass(&mut self, puzzle: &mut DurakPuzzle, today: &str, pass: bool) -> bool {
        let Some(game) = self.game.clone() else {
            return false;
        };
        if game.phase != Phase::PlayerThrow {
            return false;
        }
        let next = if !pass && !self.selected.is_empty() {
            let max = attack_limit_against_cpu(&game).saturating_sub(game.table.len());
            let cards: Vec<Card> = self.selected_cards(&game).into_iter().take(max).collect();
            process_cpu_turn(&player_throw(&game, &cards))
        } else {
            process_cpu_turn(&player_pass_throw(&game))
        };
        self.finish(puzzle, next, today);
        true
    }

    /// The "Defend" button: the selected card on the first undefended attack.
    pub fn defend(&mut self, puzzle: &mut DurakPuzzle, today: &str) -> bool {
        let Some(game) = self.game.clone() else {
            return false;
        };
        if self.selected.is_empty() || game.phase != Phase::PlayerDefense {
            return false;
        }
        let card = game.player_hand[self.selected[0]];
        let next = process_cpu_turn(&defend_one_card(&game, card));
        self.finish(puzzle, next, today);
        true
    }

    /// The "Pick up" button.
    pub fn pick_up(&mut self, puzzle: &mut DurakPuzzle, today: &str) -> bool {
        let Some(game) = self.game.clone() else {
            return false;
        };
        if game.phase != Phase::PlayerDefense {
            return false;
        }
        self.selected.clear();
        let next = process_cpu_turn(&player_pick_up(&game));
        self.finish(puzzle, next, today);
        true
    }

    /// The "Slide" button with the selected card.
    pub fn slide(&mut self, puzzle: &mut DurakPuzzle, today: &str) -> bool {
        let Some(game) = self.game.clone() else {
            return false;
        };
        if game.phase != Phase::PlayerDefense {
            return false;
        }
        let Some(&index) = self.selected.first() else {
            return false;
        };
        let card = game.player_hand[index];
        if !legal_slide_cards(&game, Side::Player).contains(&card) {
            return false;
        }
        let next = process_cpu_turn(&execute_slide(&game, card));
        self.finish(puzzle, next, today);
        true
    }

    /// "Try Again" after the CPU won: a fresh deal of the same seed, one more failure.
    pub fn retry(&mut self, puzzle: &mut DurakPuzzle, today: &str, pick: f64) -> bool {
        let failures = puzzle.failures + 1;
        let seed = puzzle.seed.clone().unwrap_or_else(|| today.to_string());
        let mut changed = false;
        if let Some(daily) = find_daily_puzzle(&seed, pick) {
            puzzle.store(&daily.initial, failures, false, &daily.hint, &seed);
            self.game = Some(daily.initial);
            changed = true;
        }
        self.selected.clear();
        changed
    }

    /// Whether the "Defend" / "Slide" buttons are enabled for the current selection.
    pub fn can_defend_and_slide(&self) -> (bool, bool) {
        let Some(game) = &self.game else {
            return (false, false);
        };
        let Some(card) = self
            .selected
            .first()
            .and_then(|&i| game.player_hand.get(i).copied())
        else {
            return (false, false);
        };
        let target = game.table.iter().find(|e| e.defense.is_none());
        (
            target.is_some_and(|t| can_beat(card, t.attack, game.trump)),
            legal_slide_cards(game, Side::Player).contains(&card),
        )
    }

    /// `canPlay` for the hand card at `index` (the "playable" highlight).
    pub fn playable(&self, index: usize) -> bool {
        let Some(game) = &self.game else { return false };
        let Some(&card) = game.player_hand.get(index) else {
            return false;
        };
        match game.phase {
            Phase::PlayerAttack | Phase::PlayerThrow => true,
            Phase::PlayerDefense => game
                .table
                .iter()
                .find(|e| e.defense.is_none())
                .is_some_and(|t| can_beat(card, t.attack, game.trump)),
            _ => false,
        }
    }
}

/// `attacksOnTop` for one table column: during the player's defence attacks are drawn on top,
/// during the CPU's below, otherwise the CPU's attacks on top.
pub fn attack_on_top(phase: Phase, entry: &TableEntry) -> bool {
    match phase {
        Phase::PlayerDefense => true,
        Phase::CpuDefense => false,
        _ => entry.attack_by == Side::Cpu,
    }
}
