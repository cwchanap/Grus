// Database service factory - now uses SQLite for both development and production
import {
  type DatabaseResult,
  DatabaseService as _DatabaseService,
  getDatabaseService as getService,
} from "./db/index.ts";
import type { Player, Room, Score } from "../types/game.ts";

// Database service interface
export interface IDatabaseService {
  createRoom(name: string, hostId: string, maxPlayers?: number): DatabaseResult<string>;
  getRoomById(id: string): DatabaseResult<Room | null>;
  getActiveRooms(limit?: number): DatabaseResult<Room[]>;
  updateRoom(id: string, updates: Partial<Room>): DatabaseResult<boolean>;
  deleteRoom(id: string): DatabaseResult<boolean>;

  createPlayer(name: string, roomId: string, isHost?: boolean): DatabaseResult<string>;
  getPlayerById(id: string): DatabaseResult<Player | null>;
  getPlayersByRoom(roomId: string): DatabaseResult<Player[]>;
  updatePlayer(
    id: string,
    updates: Partial<{ name: string; isHost: boolean }>,
  ): DatabaseResult<boolean>;
  removePlayer(id: string): DatabaseResult<boolean>;

  createGameSession(roomId: string, totalRounds?: number): DatabaseResult<string>;
  endGameSession(id: string, winnerId?: string): DatabaseResult<boolean>;

  createScore(sessionId: string, playerId: string, points?: number): DatabaseResult<string>;
  updateScore(id: string, points: number, correctGuesses: number): DatabaseResult<boolean>;
  getScoresBySession(sessionId: string): DatabaseResult<Score[]>;

  healthCheck(): DatabaseResult<boolean>;
  close(): void;
}

// Re-export the database service
export function getDatabaseService(): IDatabaseService {
  return getService();
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
