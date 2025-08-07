// Internal WebSocket types and interfaces
import type { Env } from "../../../types/cloudflare.ts";
import type { BaseClientMessage, BaseServerMessage } from "../../../types/core/websocket.ts";
import type { BaseGameState } from "../../../types/core/game.ts";
import type { PlayerState } from "../../../types/core/room.ts";

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
  handle(connection: WebSocketConnection, message: BaseClientMessage): Promise<void>;
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
  getGameState(roomId: string): Promise<BaseGameState | null>;
  updateGameState(roomId: string, gameState: BaseGameState): Promise<void>;
  getPlayerState(playerId: string): Promise<PlayerState | null>;
  updatePlayerState(playerId: string, playerState: PlayerState): Promise<void>;
}
