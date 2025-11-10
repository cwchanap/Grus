// Room-specific types for route handlers and components
import { PlayerState } from "../core/room.ts";
import { DrawingCommand } from "../games/drawing.ts";
import { PokerCard, PokerPlayer, BettingRound } from "../games/poker.ts";

/**
 * Player representation used in room routes - extends base PlayerState
 * with additional fields needed for room initialization
 */
export interface RoomPlayer extends PlayerState {
  // RoomPlayer extends PlayerState which already has: id, name, isHost, isConnected, lastActivity
  // Additional fields can be added here if needed in the future
}

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
