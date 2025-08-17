// Database service factory - now uses SQLite for both development and production
import {
  type DatabaseResult,
  DatabaseService as _DatabaseService,
  getDatabaseService as getService,
  getKVRoomService,
  KVRoomService,
} from "./db/index.ts";
import type { Player, Room } from "../types/core/room.ts";
import type { Score } from "../types/core/game.ts";

// Database service interface
export interface IDatabaseService {
  createRoom(
    name: string,
    hostId: string | null,
    maxPlayers?: number,
    gameType?: string,
  ): DatabaseResult<string>;
  getRoomById(id: string): DatabaseResult<Room | null>;
  getAllRooms(limit?: number): DatabaseResult<Room[]>;
  getActiveRooms(limit?: number): DatabaseResult<Room[]>;
  updateRoom(id: string, updates: Partial<Room>): DatabaseResult<boolean>;
  deleteRoom(id: string): DatabaseResult<boolean>;
  deleteAllRooms(): DatabaseResult<number>;

  createPlayer(name: string, roomId: string, isHost?: boolean): DatabaseResult<string>;
  getPlayerById(id: string): DatabaseResult<Player | null>;
  getPlayersByRoom(roomId: string): DatabaseResult<Player[]>;
  updatePlayer(
    id: string,
    updates: Partial<{ name: string; isHost: boolean }>,
  ): DatabaseResult<boolean>;
  removePlayer(id: string): DatabaseResult<boolean>;

  createGameSession(roomId: string, gameType: string, totalRounds?: number): DatabaseResult<string>;
  endGameSession(id: string, winnerId?: string): DatabaseResult<boolean>;

  createScore(sessionId: string, playerId: string, points?: number): DatabaseResult<string>;
  updateScore(id: string, points: number, correctGuesses: number): DatabaseResult<boolean>;
  getScoresBySession(sessionId: string): DatabaseResult<Score[]>;

  healthCheck(): DatabaseResult<boolean>;
  close(): void;
}

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

// Re-export the database service
export function getDatabaseService(): IDatabaseService {
  return getService();
}

// Async getter that returns the KV-backed implementation
export function getAsyncDatabaseService(): IAsyncDatabaseService {
  // KVRoomService structurally implements IAsyncDatabaseService
  const svc: KVRoomService = getKVRoomService();
  return svc;
}

// For development: clear all data (SQLite version)
export function clearDevelopmentData(): void {
  const isDevelopment = Deno.env.get("DENO_ENV") !== "production";
  if (isDevelopment) {
    // For SQLite, we could truncate tables or delete the database file
    // For now, just log that this would clear data
    console.log("Development database clear requested (SQLite)");
  }
}

// For development: get stats (SQLite version)
export function getDevelopmentStats(): any {
  const isDevelopment = Deno.env.get("DENO_ENV") !== "production";
  if (isDevelopment) {
    // Could implement table row counts for SQLite
    console.log("Development stats requested (SQLite)");
    return { message: "SQLite stats not implemented yet" };
  }
  return null;
}
