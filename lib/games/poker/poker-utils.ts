import {
  HandRank,
  PokerCard,
  PokerHand,
  Rank,
  Suit,
} from "../../../types/games/poker.ts";

/**
 * Converts a card rank to its numeric value.
 * Aces can be high (14) or low (1).
 * @param rank The rank to convert.
 * @returns The numeric value of the rank.
 */
export function rankToValue(rank: Rank): number {
  switch (rank) {
    case Rank.Ace:
      return 14;
    case Rank.King:
      return 13;
    case Rank.Queen:
      return 12;
    case Rank.Jack:
      return 11;
    case Rank.Ten:
      return 10;
    default:
      return parseInt(rank, 10);
  }
}

/**
 * Creates a standard 52-card poker deck.
 * @returns An array of PokerCard objects.
 */
export function createDeck(): PokerCard[] {
  const suits = Object.values(Suit);
  const ranks = Object.values(Rank);
  const deck: PokerCard[] = [];
  for (const suit of suits) {
    for (const rank of ranks) {
      deck.push({ suit, rank });
    }
  }
  return deck;
}

/**
 * Shuffles a deck of cards using the Fisher-Yates algorithm.
 * @param deck The deck to shuffle.
 * @returns A new array with the shuffled deck.
 */
export function shuffleDeck(deck: PokerCard[]): PokerCard[] {
  const shuffled = [...deck];
  for (let i = shuffled.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
  }
  return shuffled;
}

/**
 * Deals a specified number of cards from the top of a deck.
 * @param deck The deck to deal from.
 * @param count The number of cards to deal.
 * @returns An array of dealt cards.
 */
export function dealCards(deck: PokerCard[], count: number): PokerCard[] {
  return deck.slice(0, count);
}

/**
 * Checks if a hand of cards is a flush.
 * @param cards An array of 5 to 7 cards.
 * @returns True if the hand is a flush, false otherwise.
 */
export function isFlush(cards: PokerCard[]): boolean {
  if (cards.length < 5) return false;
  const suitCounts = cards.reduce((acc, card) => {
    acc[card.suit] = (acc[card.suit] || 0) + 1;
    return acc;
  }, {} as Record<Suit, number>);
  return Object.values(suitCounts).some((count) => count >= 5);
}

/**
 * Checks if a hand of cards contains a straight.
 * @param cards An array of 5 to 7 cards.
 * @returns True if the hand contains a straight, false otherwise.
 */
export function isStraight(cards: PokerCard[]): boolean {
  if (cards.length < 5) return false;
  const uniqueValues = [...new Set(cards.map((c) => rankToValue(c.rank)))].sort(
    (a, b) => a - b,
  );
  if (uniqueValues.length < 5) return false;

  // Ace-low straight (A-2-3-4-5)
  const hasAce = uniqueValues.includes(14);
  if (hasAce && [2, 3, 4, 5].every((v) => uniqueValues.includes(v))) {
    return true;
  }

  for (let i = 0; i <= uniqueValues.length - 5; i++) {
    let isStraight = true;
    for (let j = 1; j < 5; j++) {
      if (uniqueValues[i + j] !== uniqueValues[i] + j) {
        isStraight = false;
        break;
      }
    }
    if (isStraight) return true;
  }

  return false;
}

/**
 * Finds pairs, three-of-a-kind, and four-of-a-kind in a hand.
 * @param cards An array of cards.
 * @returns An array of counts of matching ranks (e.g., [2, 2] for two pair).
 */
export function findPairs(cards: PokerCard[]): number[] {
  const counts = cards.reduce((acc, card) => {
    const value = rankToValue(card.rank);
    acc[value] = (acc[value] || 0) + 1;
    return acc;
  }, {} as Record<number, number>);
  return Object.values(counts).filter((count) => count >= 2).sort((a, b) =>
    b - a
  );
}

/**
 * Evaluates the best 5-card hand from a set of 7 cards.
 * @param allCards The 7 cards to evaluate.
 * @returns The best PokerHand.
 */
export function evaluateHand(allCards: PokerCard[]): PokerHand {
  // This is a simplified implementation. A full implementation would be more complex.
  // It should check for all hand ranks and handle tie-breaking correctly.

  const cardValues = allCards.map((c) => rankToValue(c.rank)).sort((a, b) =>
    b - a
  );

  const flushSuit = (Object.entries(allCards.reduce((acc, card) => {
    acc[card.suit] = (acc[card.suit] || 0) + 1;
    return acc;
  }, {} as Record<Suit, number>)).find(([, count]) => count >= 5)?.[0] as
    | Suit
    | undefined);

  const flushCards = flushSuit
    ? allCards.filter((c) => c.suit === flushSuit)
    : [];

  const hasFlush = !!flushSuit;
  const hasStraight = isStraight(allCards);
  const pairs = findPairs(allCards);

  if (hasFlush && isStraight(flushCards)) {
    const flushValues = flushCards.map((c) => rankToValue(c.rank)).sort((a, b) =>
      b - a
    );
    // Simplified Royal Flush check
    if (flushValues.includes(14) && flushValues.includes(13)) {
      return { rank: HandRank.ROYAL_FLUSH, values: [] };
    }
    return { rank: HandRank.STRAIGHT_FLUSH, values: [flushValues[0]] };
  }

  if (pairs[0] === 4) {
    const quadValue = parseInt(
      Object.entries(allCards.reduce((acc, card) => {
        const value = rankToValue(card.rank);
        acc[value] = (acc[value] || 0) + 1;
        return acc;
      }, {} as Record<number, number>)).find(([, count]) => count === 4)!,
    );
    const kicker = Math.max(...cardValues.filter((v) => v !== quadValue));
    return { rank: HandRank.FOUR_OF_A_KIND, values: [quadValue, kicker] };
  }

  if (pairs[0] === 3 && pairs[1] >= 2) {
    const tripleValue = parseInt(
      Object.entries(allCards.reduce((acc, card) => {
        const value = rankToValue(card.rank);
        acc[value] = (acc[value] || 0) + 1;
        return acc;
      }, {} as Record<number, number>)).find(([, count]) => count === 3)!,
    );
    const pairValue = parseInt(
      Object.entries(allCards.reduce((acc, card) => {
        const value = rankToValue(card.rank);
        acc[value] = (acc[value] || 0) + 1;
        return acc;
      }, {} as Record<number, number>)).filter(([, count]) => count >= 2).find((
        [val],
      ) => parseInt(val) !== tripleValue)?.[0] || "0",
    );
    return { rank: HandRank.FULL_HOUSE, values: [tripleValue, pairValue] };
  }

  if (hasFlush) {
    const flushValues = flushCards.map((c) => rankToValue(c.rank)).sort((a, b) =>
      b - a
    );
    return { rank: HandRank.FLUSH, values: flushValues.slice(0, 5) };
  }

  if (hasStraight) {
    const uniqueValues = [...new Set(allCards.map((c) => rankToValue(c.rank)))]
      .sort((a, b) => b - a);
    if (
      uniqueValues.includes(14) && uniqueValues.includes(5) &&
      uniqueValues.includes(4) &&
      uniqueValues.includes(3) && uniqueValues.includes(2)
    ) {
      return { rank: HandRank.STRAIGHT, values: [5, 4, 3, 2, 1] };
    }
    for (let i = 0; i <= uniqueValues.length - 5; i++) {
      if (uniqueValues[i] - uniqueValues[i + 4] === 4) {
        return {
          rank: HandRank.STRAIGHT,
          values: uniqueValues.slice(i, i + 5),
        };
      }
    }
  }

  if (pairs[0] === 3) {
    const tripleValue = parseInt(
      Object.entries(allCards.reduce((acc, card) => {
        const value = rankToValue(card.rank);
        acc[value] = (acc[value] || 0) + 1;
        return acc;
      }, {} as Record<number, number>)).find(([, count]) => count === 3)!,
    );
    const kickers = cardValues.filter((v) => v !== tripleValue).slice(0, 2);
    return { rank: HandRank.THREE_OF_A_KIND, values: [tripleValue, ...kickers] };
  }

  if (pairs[0] === 2 && pairs[1] === 2) {
    const pairValues = Object.entries(allCards.reduce((acc, card) => {
      const value = rankToValue(card.rank);
      acc[value] = (acc[value] || 0) + 1;
      return acc;
    }, {} as Record<number, number>)).filter(([, count]) => count === 2).map((
      [val],
    ) => parseInt(val)).sort((a, b) => b - a);
    const kicker = Math.max(
      ...cardValues.filter((v) => v !== pairValues[0] && v !== pairValues[1]),
    );
    return {
      rank: HandRank.TWO_PAIR,
      values: [pairValues[0], pairValues[1], kicker],
    };
  }

  if (pairs[0] === 2) {
    const pairValue = parseInt(
      Object.entries(allCards.reduce((acc, card) => {
        const value = rankToValue(card.rank);
        acc[value] = (acc[value] || 0) + 1;
        return acc;
      }, {} as Record<number, number>)).find(([, count]) => count === 2)!,
    );
    const kickers = cardValues.filter((v) => v !== pairValue).slice(0, 3);
    return { rank: HandRank.ONE_PAIR, values: [pairValue, ...kickers] };
  }

  return { rank: HandRank.HIGH_CARD, values: cardValues.slice(0, 5) };
}

/**
 * Compares two poker hands to determine the winner.
 * @param hand1 The first hand.
 * @param hand2 The second hand.
 * @returns 1 if hand1 wins, -1 if hand2 wins, 0 for a tie.
 */
export function compareHands(hand1: PokerHand, hand2: PokerHand): number {
  if (hand1.rank !== hand2.rank) {
    return hand1.rank > hand2.rank ? 1 : -1;
  }
  for (let i = 0; i < hand1.values.length; i++) {
    if (hand1.values[i] !== hand2.values[i]) {
      return hand1.values[i] > hand2.values[i] ? 1 : -1;
    }
  }
  return 0;
}
