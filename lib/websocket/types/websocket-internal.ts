// Internal WebSocket types and interfaces
import type { Env } from "../../../types/cloudflare.ts";
import type { ClientMessage, ServerMessage as _ServerMessage, GameState, PlayerState } from "../../../types/game.ts";

export interface WebSocketConnection {
  ws: WebSocket;
  playerId: string;
  roomId: string;
  lastActivity: number;
}

export interface RoomConnections {
  connections: Map<string, WebSocketConnection>; // playerId -> connection
  playerRooms: Map<string, string>; // playerId -> roomId
  roomConnections: Map<string, Set<string>>; // roomId -> Set<playerId>
}

export interface RateLimitInfo {
  messages: number;
  drawing: number;
  lastReset: number;
}

export interface MessageHandler {
  handle(connection: WebSocketConnection, message: ClientMessage): Promise<void>;
}

export interface WebSocketService {
  env: Env;
  connections: RoomConnections;
}

export interface TimerManager {
  startRoundTimer(roomId: string, callback: (roomId: string) => Promise<void>): void;
  clearRoundTimer(roomId: string): void;
  cleanup(): void;
}

export interface GameStateStorage {
  getGameState(roomId: string): Promise<GameState | null>;
  updateGameState(roomId: string, gameState: GameState): Promise<void>;
  getPlayerState(playerId: string): Promise<PlayerState | null>;
  updatePlayerState(playerId: string, playerState: PlayerState): Promise<void>;
}