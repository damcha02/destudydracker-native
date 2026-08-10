export type Suit = "hearts" | "diamonds" | "clubs" | "spades";
export type Rank = "6" | "7" | "8" | "9" | "10" | "J" | "Q" | "K" | "A";

export interface Card {
  suit: Suit;
  rank: Rank;
}

export interface TableEntry {
  attack: Card;
  defense?: Card;
  attackBy: "player" | "cpu";
  defenseBy?: "player" | "cpu";
}

export type DurakPhase =
  | "player_attack"
  | "player_defense"
  | "player_throw"
  | "cpu_defense"
  | "cpu_attack"
  | "cpu_throw"
  | "finished";

export interface DurakGameState {
  playerHand: Card[];
  cpuHand: Card[];
  table: TableEntry[];
  trumpSuit: Suit;
  discardPile: Card[];
  phase: DurakPhase;
  winner?: "player" | "cpu";
  message: string;
}

export interface DailyPuzzle {
  seed: string;
  initialState: DurakGameState;
  hint: string;
}

export const SUITS: Suit[] = ["hearts", "diamonds", "clubs", "spades"];
export const RANKS: Rank[] = ["6", "7", "8", "9", "10", "J", "Q", "K", "A"];

export const RANK_ORDER: Record<Rank, number> = {
  "6": 0, "7": 1, "8": 2, "9": 3, "10": 4,
  "J": 5, "Q": 6, "K": 7, "A": 8,
};

export const SUIT_SYMBOL: Record<Suit, string> = {
  hearts: "♥",
  diamonds: "♦",
  clubs: "♣",
  spades: "♠",
};

export const SUIT_COLOR: Record<Suit, string> = {
  hearts: "#e74c3c",
  diamonds: "#e74c3c",
  clubs: "#2c3e50",
  spades: "#2c3e50",
};

export function cardToString(card: Card): string {
  return `${card.rank}${SUIT_SYMBOL[card.suit]}`;
}

export function canBeat(card: Card, target: Card, trumpSuit: Suit): boolean {
  if (card.suit === target.suit) {
    return RANK_ORDER[card.rank] > RANK_ORDER[target.rank];
  }
  return card.suit === trumpSuit && target.suit !== trumpSuit;
}

export function sameRank(a: Card, b: Card): boolean {
  return a.rank === b.rank;
}

function mulberry32(seed: number): () => number {
  return function () {
    seed |= 0;
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function hashDate(dateStr: string): number {
  let hash = 0;
  for (let i = 0; i < dateStr.length; i++) {
    const ch = dateStr.charCodeAt(i);
    hash = ((hash << 5) - hash + ch) | 0;
  }
  return Math.abs(hash);
}

function createDeck(): Card[] {
  const deck: Card[] = [];
  for (const suit of SUITS) {
    for (const rank of RANKS) {
      deck.push({ suit, rank });
    }
  }
  return deck;
}

function shuffleDeck(deck: Card[], rng: () => number): Card[] {
  const result = [...deck];
  for (let i = result.length - 1; i > 0; i--) {
    const j = Math.floor(rng() * (i + 1));
    [result[i], result[j]] = [result[j], result[i]];
  }
  return result;
}

function copyState(state: DurakGameState): DurakGameState {
  return {
    playerHand: [...state.playerHand],
    cpuHand: [...state.cpuHand],
    table: state.table.map((e) => ({ ...e, attack: { ...e.attack }, attackBy: e.attackBy, defense: e.defense ? { ...e.defense } : undefined, defenseBy: e.defenseBy })),
    trumpSuit: state.trumpSuit,
    discardPile: [...state.discardPile],
    phase: state.phase,
    winner: state.winner,
    message: state.message,
  };
}

function removeCard(hand: Card[], card: Card): Card[] {
  const idx = hand.findIndex((c) => c.suit === card.suit && c.rank === card.rank);
  if (idx === -1) return hand;
  const copy = [...hand];
  copy.splice(idx, 1);
  return copy;
}

function pickUpCards(state: DurakGameState, target: "player" | "cpu"): DurakGameState {
  const next = copyState(state);
  const tableCards: Card[] = [];
  for (const entry of next.table) {
    tableCards.push(entry.attack);
    if (entry.defense) tableCards.push(entry.defense);
  }
  if (target === "player") {
    next.playerHand = [...next.playerHand, ...tableCards];
  } else {
    next.cpuHand = [...next.cpuHand, ...tableCards];
  }
  next.table = [];
  if (target === "cpu") {
    next.phase = "player_attack";
    next.message = "CPU picked up cards. Your turn to attack.";
  } else {
    next.phase = "cpu_attack";
    next.message = "You picked up cards. CPU's turn to attack.";
  }
  checkWin(next);
  return next;
}

export function getAttackLimitAgainstCpu(state: DurakGameState): number {
  const cpuDefenseCardsOnTable = state.table.filter((entry) => entry.defenseBy === "cpu").length;
  return Math.min(6, state.cpuHand.length + cpuDefenseCardsOnTable);
}

function cpuIsPickingUp(state: DurakGameState): boolean {
  return state.phase === "player_throw" && state.table.some((entry) => !entry.defense);
}

function finishCpuPickup(state: DurakGameState): DurakGameState {
  const next = pickUpCards(state, "cpu");
  if (next.phase !== "finished") {
    next.phase = "player_attack";
    next.message = "CPU picked up cards. Your turn to attack.";
  }
  return next;
}

function startCpuPickup(state: DurakGameState): DurakGameState {
  const next = copyState(state);
  next.phase = "player_throw";
  next.message = "CPU picks up. You may throw in matching ranks, or pass.";
  return next;
}

function clearTable(state: DurakGameState): DurakGameState {
  const next = copyState(state);
  const discards: Card[] = [];
  for (const entry of next.table) {
    discards.push(entry.attack);
    if (entry.defense) discards.push(entry.defense);
  }
  next.discardPile = [...next.discardPile, ...discards];
  next.table = [];
  return next;
}

function checkWin(state: DurakGameState): void {
  if (state.playerHand.length === 0 && state.cpuHand.length > 0) {
    state.phase = "finished";
    state.winner = "player";
    state.message = "You win! The CPU still has cards.";
  } else if (state.cpuHand.length === 0 && state.playerHand.length > 0) {
    state.phase = "finished";
    state.winner = "cpu";
    state.message = "CPU wins! CPU has no cards left.";
  } else if (state.playerHand.length === 0 && state.cpuHand.length === 0) {
    state.phase = "finished";
    state.winner = "player";
    state.message = "You win! Both hands are empty.";
  }
}

export function getValidAttacks(hand: Card[], maxCards?: number): Card[][] {
  if (maxCards != null && maxCards <= 0) return [];
  const groups = new Map<Rank, Card[]>();
  for (const card of hand) {
    const list = groups.get(card.rank) ?? [];
    list.push(card);
    groups.set(card.rank, list);
  }
  const result: Card[][] = [];
  for (const cards of groups.values()) {
    result.push(maxCards != null ? cards.slice(0, maxCards) : cards);
  }
  return result;
}

export function getValidThrows(hand: Card[], table: TableEntry[], maxTotal: number): Card[][] {
  const tableRanks = new Set<Rank>();
  for (const entry of table) {
    tableRanks.add(entry.attack.rank);
    if (entry.defense) tableRanks.add(entry.defense.rank);
  }
  const currentAttackCount = table.length;
  const maxAdd = Math.max(0, maxTotal - currentAttackCount);
  if (maxAdd <= 0) return [];
  const result: Card[][] = [];
  for (const rank of tableRanks) {
    const cards = hand.filter((c) => c.rank === rank);
    if (cards.length > 0) {
      result.push(cards.slice(0, maxAdd));
    }
  }
  return result;
}

export function getDefenseOptions(hand: Card[], attackCard: Card, trumpSuit: Suit): Card[] {
  return hand.filter((c) => canBeat(c, attackCard, trumpSuit));
}

export function getBestDefense(hand: Card[], attackCard: Card, trumpSuit: Suit): Card | null {
  const options = getDefenseOptions(hand, attackCard, trumpSuit);
  if (options.length === 0) return null;
  const nonTrump = options.filter((c) => c.suit !== trumpSuit);
  if (nonTrump.length > 0) {
    nonTrump.sort((a, b) => RANK_ORDER[a.rank] - RANK_ORDER[b.rank]);
    return nonTrump[0];
  }
  options.sort((a, b) => RANK_ORDER[a.rank] - RANK_ORDER[b.rank]);
  return options[0];
}

export function getSlideRanks(table: TableEntry[]): Set<Rank> {
  const ranks = new Set<Rank>();
  for (const entry of table) {
    ranks.add(entry.attack.rank);
  }
  return ranks;
}

export function getSlideCards(hand: Card[], table: TableEntry[]): Card[] {
  const ranks = getSlideRanks(table);
  return hand.filter((c) => ranks.has(c.rank));
}

export function getLegalSlideCards(state: DurakGameState, slideBy: "player" | "cpu"): Card[] {
  const hand = slideBy === "player" ? state.playerHand : state.cpuHand;
  const receiverHand = slideBy === "player" ? state.cpuHand : state.playerHand;
  const undefendedAfterSlide = state.table.filter((entry) => !entry.defense).length + 1;
  if (hand.length <= 1 || receiverHand.length < undefendedAfterSlide) return [];
  if (state.table.some((entry) => entry.defense)) return [];
  return getSlideCards(hand, state.table);
}

export function executeSlide(state: DurakGameState, slideCard: Card): DurakGameState {
  const next = copyState(state);
  if (next.phase !== "cpu_defense" && next.phase !== "player_defense") return next;
  const slideBy = next.phase === "cpu_defense" ? "cpu" : "player";
  const legalSlide = getLegalSlideCards(next, slideBy).some((card) => card.suit === slideCard.suit && card.rank === slideCard.rank);
  if (!legalSlide) return next;
  next.table.push({ attack: { ...slideCard }, attackBy: slideBy });
  const hand = next.phase === "cpu_defense" ? next.cpuHand : next.playerHand;
  const idx = hand.findIndex((c) => c.suit === slideCard.suit && c.rank === slideCard.rank);
  if (idx !== -1) hand.splice(idx, 1);
  if (next.phase === "cpu_defense") {
    next.phase = "player_defense";
    next.message = `CPU slides with ${cardToString(slideCard)}! You must defend.`;
  } else if (next.phase === "player_defense") {
    next.phase = "cpu_defense";
    next.message = `You slide with ${cardToString(slideCard)}! CPU must defend.`;
  }
  checkWin(next);
  return next;
}

function cpuDefend(state: DurakGameState): DurakGameState {
  const next = copyState(state);
  const undefended = next.table.filter((e) => !e.defense);
  for (const entry of undefended) {
    const best = getBestDefense(next.cpuHand, entry.attack, next.trumpSuit);
    if (best) {
      entry.defense = { ...best };
      entry.defenseBy = "cpu";
      next.cpuHand = removeCard(next.cpuHand, best);
    } else {
      return startCpuPickup(next);
    }
  }
  next.message = "CPU defended all cards.";
  next.phase = "player_throw";
  checkWin(next);
  return next;
}

function cpuAttack(state: DurakGameState): DurakGameState {
  const next = copyState(state);
  const attacks = getValidAttacks(next.cpuHand, Math.min(6, next.playerHand.length));
  if (attacks.length === 0) {
    next.phase = "finished";
    next.winner = "player";
    next.message = "CPU has no cards to attack. You win!";
    return next;
  }
  attacks.sort((a, b) => {
    if (a.length !== b.length) return b.length - a.length;
    return RANK_ORDER[a[0].rank] - RANK_ORDER[b[0].rank];
  });
  const chosen = attacks[0];
  next.table = chosen.map((c) => ({ attack: { ...c }, attackBy: "cpu" }));
  next.cpuHand = chosen.reduce((h, c) => removeCard(h, c), next.cpuHand);
  next.phase = "player_defense";
  next.message = `CPU attacks with ${chosen.map(cardToString).join(", ")}. Defend or pick up.`;
  checkWin(next);
  return next;
}

function cpuThrowCards(state: DurakGameState): DurakGameState {
  let next = copyState(state);
  const maxTotal = Math.min(6, next.table.length + next.playerHand.length);
  const throws = getValidThrows(next.cpuHand, next.table, maxTotal);
  if (throws.length > 0) {
    const chosen = throws[0];
    for (const card of chosen) {
      next.table.push({ attack: { ...card }, attackBy: "cpu" });
      next.cpuHand = removeCard(next.cpuHand, card);
    }
    next.phase = "player_defense";
    next.message = `CPU throws in ${chosen.map(cardToString).join(", ")}. Defend!`;
  } else {
    next = clearTable(next);
    next.phase = "player_attack";
    next.message = "CPU passes. Your turn to attack.";
  }
  checkWin(next);
  return next;
}

export function executePlayerAttack(state: DurakGameState, cards: Card[]): DurakGameState {
  const next = copyState(state);
  if (cards.length === 0) return next;
  const maxAttack = Math.min(6, next.cpuHand.length);
  const used = cards.slice(0, maxAttack);
  for (const card of used) {
    next.table.push({ attack: { ...card }, attackBy: "player" });
    next.playerHand = removeCard(next.playerHand, card);
  }
  next.phase = "cpu_defense";
  next.message = `You attack with ${used.map(cardToString).join(", ")}.`;
  checkWin(next);
  return next;
}

export function executePlayerThrow(state: DurakGameState, cards: Card[]): DurakGameState {
  const next = copyState(state);
  const wasCpuPickingUp = cpuIsPickingUp(next);
  const maxAdd = Math.max(0, getAttackLimitAgainstCpu(next) - next.table.length);
  const used = cards.slice(0, maxAdd);
  if (used.length === 0) return next;
  for (const card of used) {
    next.table.push({ attack: { ...card }, attackBy: "player" });
    next.playerHand = removeCard(next.playerHand, card);
  }
  if (wasCpuPickingUp) {
    const throws = getValidThrows(next.playerHand, next.table, getAttackLimitAgainstCpu(next));
    if (throws.length === 0 || next.table.length >= getAttackLimitAgainstCpu(next)) {
      return finishCpuPickup(next);
    }
    next.phase = "player_throw";
    next.message = `You throw in ${used.map(cardToString).join(", ")}. CPU will pick them up too.`;
  } else {
    next.phase = "cpu_defense";
    next.message = `You throw in ${used.map(cardToString).join(", ")}.`;
  }
  checkWin(next);
  return next;
}

export function executePlayerDefend(state: DurakGameState, defenseMap: Map<number, Card>): DurakGameState {
  const next = copyState(state);
  const undefended = next.table.map((e, i) => ({ entry: e, idx: i })).filter((e) => !e.entry.defense);
  for (const { idx } of undefended) {
    const defCard = defenseMap.get(idx);
    if (defCard) {
      next.table[idx].defense = { ...defCard };
      next.playerHand = removeCard(next.playerHand, defCard);
    } else {
      return pickUpCards(next, "player");
    }
  }
  next.phase = "cpu_throw";
  next.message = "You defended all cards. CPU may throw more.";
  checkWin(next);
  return next;
}

export function defendOneCard(state: DurakGameState, card: Card): DurakGameState {
  const next = copyState(state);
  const targetIdx = next.table.findIndex((e) => !e.defense);
  if (targetIdx === -1 || !canBeat(card, next.table[targetIdx].attack, next.trumpSuit)) return next;
  next.table[targetIdx].defense = { ...card };
  next.table[targetIdx].defenseBy = "player";
  next.playerHand = removeCard(next.playerHand, card);
  const remaining = next.table.filter((e) => !e.defense);
  if (remaining.length === 0) {
    next.phase = "cpu_throw";
    next.message = "You defended all cards!";
  } else {
    next.message = `Defend ${cardToString(remaining[0].attack)} — select a card to beat it.`;
  }
  checkWin(next);
  return next;
}

export function playerPassThrow(state: DurakGameState): DurakGameState {
  if (cpuIsPickingUp(state)) return finishCpuPickup(state);
  const next = clearTable(state);
  next.phase = "cpu_attack";
  next.message = "CPU's turn to attack.";
  checkWin(next);
  return next;
}

export function playerPickUp(state: DurakGameState): DurakGameState {
  return pickUpCards(state, "player");
}

export function processCpuTurn(state: DurakGameState): DurakGameState {
  switch (state.phase) {
    case "cpu_defense": {
      const slideCards = getLegalSlideCards(state, "cpu");
      if (slideCards.length > 0) return executeSlide(state, slideCards[0]);
      return cpuDefend(state);
    }
    case "cpu_attack":
      return cpuAttack(state);
    case "cpu_throw":
      return cpuThrowCards(state);
    default:
      return state;
  }
}

function generateHint(cpuHand: Card[], trumpSuit: Suit): string {
  const trumpCount = cpuHand.filter((c) => c.suit === trumpSuit).length;
  const hints: string[] = [];
  if (trumpCount > 0) {
    hints.push(`CPU has ${trumpCount} trump card${trumpCount > 1 ? "s" : ""}`);
  }
  const ace = cpuHand.find((c) => c.rank === "A");
  if (ace) {
    hints.push(`CPU holds ${cardToString(ace)}`);
  }
  const suitsPresent = new Set(cpuHand.map((c) => c.suit));
  const missingSuits = SUITS.filter((s) => !suitsPresent.has(s) && s !== trumpSuit);
  if (missingSuits.length > 0) {
    hints.push(`CPU has no ${missingSuits.map((s) => SUIT_SYMBOL[s]).join("")}`);
  }
  const lowest = [...cpuHand].sort((a, b) => RANK_ORDER[a.rank] - RANK_ORDER[b.rank])[0];
  if (lowest) {
    hints.push(`CPU's lowest card is ${lowest.rank}${SUIT_SYMBOL[lowest.suit]}`);
  }
  if (hints.length === 0) {
    hints.push(`CPU has ${cpuHand.length} cards`);
  }
  return hints[Math.floor(Math.random() * hints.length)];
}

export interface WinResult {
  found: boolean;
  sequence: string[];
}

export function solver(state: DurakGameState): WinResult {
  const visited = new Set<string>();
  const seq: string[] = [];

  function stateKey(s: DurakGameState): string {
    const p = [...s.playerHand].sort((a, b) => RANK_ORDER[a.rank] - RANK_ORDER[b.rank] || a.suit.localeCompare(b.suit)).map(cardToString).join(",");
    const c = [...s.cpuHand].sort((a, b) => RANK_ORDER[a.rank] - RANK_ORDER[b.rank] || a.suit.localeCompare(b.suit)).map(cardToString).join(",");
    const t = s.table.map((e) => e.attackBy[0] + cardToString(e.attack) + (e.defense ? ">" + (e.defenseBy ?? "?")[0] + cardToString(e.defense) : "?")).join("|");
    return `${s.phase}|${p}|${c}|${t}`;
  }

  function dfs(s: DurakGameState, depth: number): boolean {
    if (depth > 20) return false;
    const key = stateKey(s);
    if (visited.has(key)) return false;
    visited.add(key);

    if (s.winner === "player") return true;
    if (s.winner === "cpu") return false;

    const next = copyState(s);

    switch (next.phase) {
      case "player_attack": {
        const attacks = getValidAttacks(next.playerHand, Math.min(6, next.cpuHand.length));
        for (const atk of attacks) {
          const n = executePlayerAttack(next, atk);
          seq.push(`play ${atk.map(cardToString).join(",")}`);
          if (dfs(n, depth + 1)) return true;
          seq.pop();
        }
        return false;
      }
      case "cpu_defense": {
        const slideOpts = getLegalSlideCards(next, "cpu");
        if (slideOpts.length > 0) {
          const ns = executeSlide(next, slideOpts[0]);
          seq.push(`cpu slide ${cardToString(slideOpts[0])}`);
          if (dfs(ns, depth + 1)) return true;
          return false;
        }
        const n = cpuDefend(next);
        if (dfs(n, depth + 1)) return true;
        return false;
      }
      case "player_throw": {
        const maxTotal = getAttackLimitAgainstCpu(next);
        const throws = getValidThrows(next.playerHand, next.table, maxTotal);
        if (throws.length === 0 || depth > 3) {
          const n = cpuIsPickingUp(next) ? finishCpuPickup(next) : clearTable(next);
          if (!cpuIsPickingUp(next) && n.phase !== "finished") n.phase = "cpu_attack";
          seq.push(`pass throw`);
          if (dfs(n, depth + 1)) return true;
          seq.pop();
        }
        for (const thr of throws) {
          const n = executePlayerThrow(next, thr);
          seq.push(`throw ${thr.map(cardToString).join(",")}`);
          if (dfs(n, depth + 1)) return true;
          seq.pop();
        }
        return false;
      }
      case "cpu_attack": {
        const n = cpuAttack(next);
        if (dfs(n, depth + 1)) return true;
        return false;
      }
      case "player_defense": {
        const undefended = next.table.filter((e) => !e.defense);
        const undefendedIndices: number[] = [];
        for (let ti = 0; ti < next.table.length; ti++) { if (!next.table[ti].defense) undefendedIndices.push(ti); }
        if (undefended.length === 0) {
          const n = copyState(next);
          n.phase = "cpu_throw";
          if (dfs(n, depth + 1)) return true;
          return false;
        }
        const targetEntry = undefended[0];
        const targetIdx = undefendedIndices[0];
        const options = getDefenseOptions(next.playerHand, targetEntry.attack, next.trumpSuit);
        for (const opt of options) {
          const n = copyState(next);
          n.table[targetIdx].defense = { ...opt };
          n.table[targetIdx].defenseBy = "player";
          n.playerHand = removeCard(n.playerHand, opt);
          seq.push(`defend ${cardToString(targetEntry.attack)} with ${cardToString(opt)}`);
          if (dfs(n, depth + 1)) return true;
          seq.pop();
        }
        const n = pickUpCards(next, "player");
        seq.push(`pick up`);
        if (dfs(n, depth + 1)) return true;
        seq.pop();
        const slideOpts = getLegalSlideCards(next, "player");
        for (const sc of slideOpts) {
          const ns = executeSlide(next, sc);
          seq.push(`slide ${cardToString(sc)}`);
          if (dfs(ns, depth + 1)) return true;
          seq.pop();
        }
        return false;
      }
      case "cpu_throw": {
        const beforeLen = next.table.length;
        const n = cpuThrowCards(next);
        if (n.table.length > beforeLen) {
          const thrown = n.table.slice(beforeLen);
          seq.push(`cpu throws ${thrown.map(t => cardToString(t.attack)).join(",")}`);
        } else {
          seq.push(`cpu passes throw`);
        }
        if (dfs(n, depth + 1)) return true;
        return false;
      }
      case "finished":
        return next.winner === "player";
    }
    return false;
  }

  const found = dfs(state, 0);
  return { found, sequence: seq };
}

export function findDailyPuzzle(dateStr: string): DailyPuzzle | null {
  const baseSeed = hashDate(dateStr);
  for (let attempt = 0; attempt < 200; attempt++) {
    const rng = mulberry32(baseSeed + attempt * 7919);
    const deck = shuffleDeck(createDeck(), rng);
    const trumpSuit = deck[0].suit;
    const deck2 = shuffleDeck(deck, rng);
    const playerCount = 5 + Math.floor(rng() * 3);
    const cpuCount = 3 + Math.floor(rng() * 2);
    let ptr = 0;
    const playerHand = deck2.slice(ptr, ptr + playerCount);
    ptr += playerCount;
    const cpuHand = deck2.slice(ptr, ptr + cpuCount);

    playerHand.sort((a, b) => RANK_ORDER[b.rank] - RANK_ORDER[a.rank]);
    cpuHand.sort((a, b) => RANK_ORDER[a.rank] - RANK_ORDER[b.rank]);
    const playerHighest = playerHand[0];
    const cpuLowest = cpuHand[0];
    if (RANK_ORDER[playerHighest.rank] > RANK_ORDER[cpuLowest.rank]) {
      const pi = playerHand.findIndex((c) => c.suit === playerHighest.suit && c.rank === playerHighest.rank);
      const ci = cpuHand.findIndex((c) => c.suit === cpuLowest.suit && c.rank === cpuLowest.rank);
      if (pi !== -1 && ci !== -1) {
        playerHand[pi] = cpuLowest;
        cpuHand[ci] = playerHighest;
      }
    }

    const state: DurakGameState = {
      playerHand: [...playerHand],
      cpuHand: [...cpuHand],
      table: [],
      trumpSuit,
      discardPile: [],
      phase: "player_attack",
      message: "Your turn to attack. Select cards of the same rank and play them.",
    };

    const result = solver(state);
    if (result.found) {
      const hint = generateHint(cpuHand, trumpSuit);
      return { seed: dateStr, initialState: state, hint };
    }
  }
  return null;
}

export function cardKey(card: Card): string {
  return `${card.rank}${SUIT_SYMBOL[card.suit]}`;
}

export function parseCardKey(key: string): Card | null {
  const suitChar = key.slice(-1);
  const rankStr = key.slice(0, -1) as Rank;
  const suit = (Object.entries(SUIT_SYMBOL).find(([, v]) => v === suitChar)?.[0] as Suit) ?? null;
  if (!suit || !RANKS.includes(rankStr)) return null;
  return { rank: rankStr, suit };
}

export function gameStateToPuzzle(state: DurakGameState, failures: number, completed: boolean, hint: string, seed: string) {
  return {
    seed,
    hint,
    playerHand: state.playerHand.map(cardKey),
    cpuHand: state.cpuHand.map(cardKey),
    trumpSuit: state.trumpSuit,
    table: state.table.map((e) => ({
      attack: cardKey(e.attack),
      defense: e.defense ? cardKey(e.defense) : undefined,
      attackBy: e.attackBy,
      defenseBy: e.defenseBy,
    })),
    discardPile: state.discardPile.map(cardKey),
    phase: state.phase,
    winner: state.winner,
    message: state.message,
    failures,
    completed,
  };
}

export function puzzleToGameState(puzzle: {
  playerHand: string[];
  cpuHand: string[];
  table: Array<{ attack: string; defense?: string; attackBy?: "player" | "cpu"; defenseBy?: "player" | "cpu" }>;
  trumpSuit: string;
  discardPile: string[];
  phase: string;
  winner?: string;
  message: string;
}): DurakGameState | null {
  if (!Array.isArray(puzzle.playerHand) || !Array.isArray(puzzle.cpuHand) || !Array.isArray(puzzle.table) || !Array.isArray(puzzle.discardPile)) return null;
  if (typeof puzzle.trumpSuit !== "string" || typeof puzzle.phase !== "string" || typeof puzzle.message !== "string") return null;
  if (!puzzle.playerHand.every((key) => typeof key === "string") || !puzzle.cpuHand.every((key) => typeof key === "string") || !puzzle.discardPile.every((key) => typeof key === "string")) return null;
  const parseList = (keys: string[]) => keys.map(parseCardKey).filter((c): c is Card => c !== null);
  const playerHand = parseList(puzzle.playerHand);
  const cpuHand = parseList(puzzle.cpuHand);
  const table = puzzle.table.map((e, i): TableEntry | null => {
    if (!e || typeof e !== "object" || typeof e.attack !== "string") return null;
    if (e.defense !== undefined && typeof e.defense !== "string") return null;
    const attack = parseCardKey(e.attack);
    const defense = e.defense ? parseCardKey(e.defense) : undefined;
    const attackBy = e.attackBy ?? (i === 0 ? "cpu" : "player");
    if (!attack || (e.defense && !defense)) return null;
    if (attackBy !== "player" && attackBy !== "cpu") return null;
    if (e.defenseBy && e.defenseBy !== "player" && e.defenseBy !== "cpu") return null;
    return { attack, defense: defense ?? undefined, attackBy, defenseBy: e.defenseBy };
  });
  const discardPile = parseList(puzzle.discardPile);
  if (playerHand.length !== puzzle.playerHand.length || cpuHand.length !== puzzle.cpuHand.length) return null;
  if (table.some((entry) => entry === null)) return null;
  if (discardPile.length !== puzzle.discardPile.length) return null;
  if (!SUITS.includes(puzzle.trumpSuit as Suit)) return null;
  if (!["player_attack", "player_defense", "player_throw", "cpu_defense", "cpu_attack", "cpu_throw", "finished"].includes(puzzle.phase)) return null;
  if (puzzle.winner && puzzle.winner !== "player" && puzzle.winner !== "cpu") return null;

  return {
    playerHand,
    cpuHand,
    table: table as TableEntry[],
    trumpSuit: puzzle.trumpSuit as Suit,
    discardPile,
    phase: puzzle.phase as DurakPhase,
    winner: puzzle.winner as "player" | "cpu" | undefined,
    message: puzzle.message,
  };
}
