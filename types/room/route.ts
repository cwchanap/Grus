// Room-specific types for route handlers and components
import { PlayerState } from "../core/room.ts";
import { DrawingCommand } from "../games/drawing.ts";
import { BettingRound, PokerCard, PokerPlayer } from "../games/poker.ts";

/**
 * Type alias for player representation in room routes.
 * Currently equivalent to PlayerState - can be extended to an intersection type
 * if additional room-specific fields are needed in the future.
 */
export type RoomPlayer = PlayerState;

/**
 * Discriminated union for game-specific data in room initialization
 */
export type GameData =
  | {
    type: "drawing";
    currentDrawer: string | null;
    currentWord: string;
    drawingData: DrawingCommand[];
    correctGuesses: Array<{ playerId: string; timestamp: number }>;
  }
  | {
    type: "poker";
    deck: PokerCard[];
    communityCards: PokerCard[];
    pot: number;
    currentBet: number;
    bettingRound: BettingRound;
    currentPlayerIndex: number;
    smallBlindIndex: number;
    bigBlindIndex: number;
    players: PokerPlayer[];
  };

/**
 * Type alias for game scores - maps player IDs to their scores
 */
export type GameScores = Record<string, number>;
