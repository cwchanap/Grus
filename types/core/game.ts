// Core game types - game agnostic base interfaces

import { ChatMessage, PlayerState } from "./room.ts";

export interface BaseGameSettings {
  maxRounds: number;
  roundTimeSeconds: number;
}

export interface BaseGameState<
  TSettings extends BaseGameSettings = BaseGameSettings,
  TGameData = any,
> {
  roomId: string;
  gameType: string;
  currentPlayer?: string; // Generic current player (could be drawer, current turn, etc.)
  roundNumber: number;
  timeRemaining: number;
  phase: "waiting" | "playing" | "results" | "finished";
  players: PlayerState[];
  scores: Record<string, number>;
  gameData: TGameData; // Game-specific data
  chatMessages: ChatMessage[];
  settings: TSettings;
}

export interface GameSession {
  id: string;
  roomId: string;
  gameType: string;
  winnerId?: string;
  totalRounds: number;
  startedAt?: string;
  endedAt?: string;
}

export interface Score {
  id: string;
  sessionId: string;
  playerId: string;
  points: number;
  // Game-specific score data can be stored as JSON
  gameSpecificData?: Record<string, any>;
}
