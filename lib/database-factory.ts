// Database service factory - KV-backed (Deno KV) for all environments
import {
  type DatabaseResult,
  getKVRoomService,
  KVRoomService,
} from "./db/index.ts";
import type { Player, Room } from "../types/core/room.ts";
import type { Score } from "../types/core/game.ts";

// Synchronous SQLite interface removed during KV migration

// Async database service interface (KV-backed)
export interface IAsyncDatabaseService {
  createRoom(
    name: string,
    hostId: string | null,
    maxPlayers?: number,
    gameType?: string,
  ): Promise<DatabaseResult<string>>;
  getRoomById(id: string): Promise<DatabaseResult<Room | null>>;
  getAllRooms(limit?: number): Promise<DatabaseResult<Room[]>>;
  getActiveRooms(limit?: number): Promise<DatabaseResult<Room[]>>;
  updateRoom(id: string, updates: Partial<Room>): Promise<DatabaseResult<boolean>>;
  deleteRoom(id: string): Promise<DatabaseResult<boolean>>;
  deleteAllRooms(): Promise<DatabaseResult<number>>;

  createPlayer(name: string, roomId: string, isHost?: boolean): Promise<DatabaseResult<string>>;
  getPlayerById(id: string): Promise<DatabaseResult<Player | null>>;
  getPlayersByRoom(roomId: string): Promise<DatabaseResult<Player[]>>;
  updatePlayer(
    id: string,
    updates: Partial<{ name: string; isHost: boolean }>,
  ): Promise<DatabaseResult<boolean>>;
  removePlayer(id: string): Promise<DatabaseResult<boolean>>;

  createGameSession(
    roomId: string,
    gameType: string,
    totalRounds?: number,
  ): Promise<DatabaseResult<string>>;
  endGameSession(id: string, winnerId?: string): Promise<DatabaseResult<boolean>>;

  createScore(
    sessionId: string,
    playerId: string,
    points?: number,
  ): Promise<DatabaseResult<string>>;
  updateScore(id: string, points: number, correctGuesses: number): Promise<DatabaseResult<boolean>>;
  getScoresBySession(sessionId: string): Promise<DatabaseResult<Score[]>>;

  healthCheck(): Promise<DatabaseResult<boolean>>;
  close(): void;
}

// getDatabaseService removed; use getAsyncDatabaseService() instead

// Async getter that returns the KV-backed implementation
export function getAsyncDatabaseService(): IAsyncDatabaseService {
  // KVRoomService structurally implements IAsyncDatabaseService
  const svc: KVRoomService = getKVRoomService();
  return svc;
}

// For development: clear all data (KV)
export function clearDevelopmentData(): void {
  const isDevelopment = Deno.env.get("DENO_ENV") !== "production";
  if (isDevelopment) {
    console.log("Development KV cleanup: run 'deno task db:cleanup'");
  }
}

// For development: get stats (KV)
export function getDevelopmentStats(): any {
  const isDevelopment = Deno.env.get("DENO_ENV") !== "production";
  if (isDevelopment) {
    console.log("Development stats requested (KV)");
    return { message: "KV stats not implemented. Use scripts/inspect-database.ts" };
  }
  return null;
}
