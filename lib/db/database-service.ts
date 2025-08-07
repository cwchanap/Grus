// Database service using local SQLite
import { Database } from "https://deno.land/x/sqlite3@0.12.0/mod.ts";
import type { Player, Room } from "../../types/core/room.ts";
import type { Score } from "../../types/core/game.ts";

export interface DatabaseResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

export class DatabaseService {
  private db: Database;

  constructor() {
    this.db = new Database("db/game.db");
    this.initializeDatabase();
  }

  private initializeDatabase(): void {
    // Enable foreign key constraints
    this.db.exec("PRAGMA foreign_keys = ON;");

    // Read and execute schema
    const schemaSQL = Deno.readTextFileSync("db/schema.sql");
    this.db.exec(schemaSQL);
  }

  private executeQuery<T>(
    operation: () => T,
    errorMessage: string,
  ): DatabaseResult<T> {
    try {
      const data = operation();
      return { success: true, data };
    } catch (error) {
      console.error(`Database error: ${errorMessage}`, error);
      return {
        success: false,
        error: error instanceof Error ? error.message : errorMessage,
      };
    }
  }

  private generateId(): string {
    return crypto.randomUUID();
  }

  // Room operations
  createRoom(
    name: string,
    hostId: string,
    maxPlayers = 8,
    gameType = "drawing",
  ): DatabaseResult<string> {
    return this.executeQuery(() => {
      const id = this.generateId();
      const stmt = this.db.prepare(
        `INSERT INTO rooms (id, name, host_id, max_players, game_type, is_active, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))`,
      );
      stmt.run(id, name, hostId, maxPlayers, gameType, true);
      stmt.finalize();
      return id;
    }, "Failed to create room");
  }

  getRoomById(id: string): DatabaseResult<Room | null> {
    return this.executeQuery(() => {
      const stmt = this.db.prepare("SELECT * FROM rooms WHERE id = ?");
      const result = stmt.get(id) as any;
      stmt.finalize();
      if (!result) return null;

      // Map snake_case to camelCase and convert boolean
      return {
        id: result.id,
        name: result.name,
        hostId: result.host_id,
        maxPlayers: result.max_players,
        gameType: result.game_type || "drawing",
        isActive: Boolean(result.is_active),
        createdAt: result.created_at,
        updatedAt: result.updated_at,
      } as Room;
    }, `Failed to get room with id: ${id}`);
  }

  getActiveRooms(limit = 20): DatabaseResult<Room[]> {
    return this.executeQuery(() => {
      const stmt = this.db.prepare(`
        SELECT r.*, COUNT(p.id) as player_count
        FROM rooms r
        LEFT JOIN players p ON r.id = p.room_id
        WHERE r.is_active = true 
        GROUP BY r.id
        ORDER BY r.created_at DESC 
        LIMIT ?
      `);
      const results = stmt.all(limit) as any[];
      stmt.finalize();

      // Map snake_case to camelCase and convert booleans
      return results.map((result) => ({
        id: result.id,
        name: result.name,
        hostId: result.host_id,
        maxPlayers: result.max_players,
        gameType: result.game_type || "drawing",
        isActive: Boolean(result.is_active),
        createdAt: result.created_at,
        updatedAt: result.updated_at,
        player_count: result.player_count,
      })) as Room[];
    }, "Failed to get active rooms");
  }

  updateRoom(id: string, updates: Partial<Room>): DatabaseResult<boolean> {
    return this.executeQuery(() => {
      // Map camelCase to snake_case for database fields
      const fieldMapping: Record<string, string> = {
        hostId: "host_id",
        maxPlayers: "max_players",
        gameType: "game_type",
        isActive: "is_active",
        createdAt: "created_at",
        updatedAt: "updated_at",
      };

      const fields: string[] = [];
      const values: any[] = [];

      Object.entries(updates).forEach(([key, value]) => {
        const dbField = fieldMapping[key] || key;
        fields.push(`${dbField} = ?`);
        values.push(value);
      });

      if (fields.length === 0) {
        return true; // No updates needed
      }

      const stmt = this.db.prepare(
        `UPDATE rooms SET ${fields.join(", ")}, updated_at = datetime('now') WHERE id = ?`,
      );
      stmt.run(...values, id);
      stmt.finalize();
      return true;
    }, `Failed to update room with id: ${id}`);
  }

  deleteRoom(id: string): DatabaseResult<boolean> {
    return this.executeQuery(() => {
      const stmt = this.db.prepare("DELETE FROM rooms WHERE id = ?");
      stmt.run(id);
      stmt.finalize();
      return true;
    }, `Failed to delete room with id: ${id}`);
  }

  // Player operations
  createPlayer(
    name: string,
    roomId: string,
    isHost = false,
  ): DatabaseResult<string> {
    return this.executeQuery(() => {
      const id = this.generateId();
      const stmt = this.db.prepare(
        `INSERT INTO players (id, name, room_id, is_host, joined_at)
         VALUES (?, ?, ?, ?, datetime('now'))`,
      );
      stmt.run(id, name, roomId, isHost);
      stmt.finalize();
      return id;
    }, "Failed to create player");
  }

  getPlayerById(id: string): DatabaseResult<Player | null> {
    return this.executeQuery(() => {
      const stmt = this.db.prepare("SELECT * FROM players WHERE id = ?");
      const result = stmt.get(id) as any;
      stmt.finalize();
      if (!result) return null;

      // Map snake_case to camelCase and convert boolean
      return {
        id: result.id,
        name: result.name,
        roomId: result.room_id,
        isHost: Boolean(result.is_host),
        joinedAt: result.joined_at,
      } as Player;
    }, `Failed to get player with id: ${id}`);
  }

  getPlayersByRoom(roomId: string): DatabaseResult<Player[]> {
    return this.executeQuery(() => {
      const stmt = this.db.prepare(
        "SELECT * FROM players WHERE room_id = ? ORDER BY joined_at ASC",
      );
      const results = stmt.all(roomId) as any[];
      stmt.finalize();

      // Map snake_case to camelCase and convert booleans
      return results.map((result) => ({
        id: result.id,
        name: result.name,
        roomId: result.room_id,
        isHost: Boolean(result.is_host),
        joinedAt: result.joined_at,
      })) as Player[];
    }, `Failed to get players for room: ${roomId}`);
  }

  updatePlayer(
    id: string,
    updates: Partial<{ name: string; isHost: boolean }>,
  ): DatabaseResult<boolean> {
    return this.executeQuery(() => {
      const fields: string[] = [];
      const values: any[] = [];

      if (updates.name !== undefined) {
        fields.push("name = ?");
        values.push(updates.name);
      }

      if (updates.isHost !== undefined) {
        fields.push("is_host = ?");
        values.push(updates.isHost);
      }

      if (fields.length === 0) {
        return true; // No updates needed
      }

      const stmt = this.db.prepare(
        `UPDATE players SET ${fields.join(", ")} WHERE id = ?`,
      );
      stmt.run(...values, id);
      stmt.finalize();
      return true;
    }, `Failed to update player with id: ${id}`);
  }

  removePlayer(id: string): DatabaseResult<boolean> {
    return this.executeQuery(() => {
      const stmt = this.db.prepare("DELETE FROM players WHERE id = ?");
      stmt.run(id);
      stmt.finalize();
      return true;
    }, `Failed to remove player with id: ${id}`);
  }

  // Game session operations
  createGameSession(roomId: string, gameType: string, totalRounds = 5): DatabaseResult<string> {
    return this.executeQuery(() => {
      const id = this.generateId();
      const stmt = this.db.prepare(
        `INSERT INTO game_sessions (id, room_id, game_type, total_rounds, started_at)
         VALUES (?, ?, ?, ?, datetime('now'))`,
      );
      stmt.run(id, roomId, gameType, totalRounds);
      stmt.finalize();
      return id;
    }, "Failed to create game session");
  }

  endGameSession(id: string, winnerId?: string): DatabaseResult<boolean> {
    return this.executeQuery(() => {
      const stmt = this.db.prepare(
        `UPDATE game_sessions 
         SET ended_at = datetime('now'), winner_id = ?
         WHERE id = ?`,
      );
      stmt.run(winnerId || null, id);
      stmt.finalize();
      return true;
    }, `Failed to end game session with id: ${id}`);
  }

  // Score operations
  createScore(
    sessionId: string,
    playerId: string,
    points = 0,
  ): DatabaseResult<string> {
    return this.executeQuery(() => {
      const id = this.generateId();
      const stmt = this.db.prepare(
        `INSERT INTO scores (id, session_id, player_id, points, correct_guesses)
         VALUES (?, ?, ?, ?, 0)`,
      );
      stmt.run(id, sessionId, playerId, points);
      stmt.finalize();
      return id;
    }, "Failed to create score");
  }

  updateScore(
    id: string,
    points: number,
    correctGuesses: number,
  ): DatabaseResult<boolean> {
    return this.executeQuery(() => {
      const stmt = this.db.prepare(
        `UPDATE scores 
         SET points = ?, correct_guesses = ?
         WHERE id = ?`,
      );
      stmt.run(points, correctGuesses, id);
      stmt.finalize();
      return true;
    }, `Failed to update score with id: ${id}`);
  }

  getScoresBySession(sessionId: string): DatabaseResult<Score[]> {
    return this.executeQuery(() => {
      const stmt = this.db.prepare(`
        SELECT s.*, p.name as player_name
        FROM scores s
        JOIN players p ON s.player_id = p.id
        WHERE s.session_id = ?
        ORDER BY s.points DESC
      `);
      const results = stmt.all(sessionId) as any[];
      stmt.finalize();

      // Map snake_case to camelCase
      return results.map((result) => ({
        id: result.id,
        sessionId: result.session_id,
        playerId: result.player_id,
        points: result.points,
        correctGuesses: result.correct_guesses,
        player_name: result.player_name,
      })) as Score[];
    }, `Failed to get scores for session: ${sessionId}`);
  }

  // Health check
  healthCheck(): DatabaseResult<boolean> {
    return this.executeQuery(() => {
      const stmt = this.db.prepare("SELECT 1 as test");
      const result = stmt.get();
      stmt.finalize();
      return (result as any)?.test === 1;
    }, "Database health check failed");
  }

  // Close database connection
  close(): void {
    this.db.close();
  }
}

// Singleton instance
let databaseService: DatabaseService | null = null;

export function getDatabaseService(): DatabaseService {
  if (!databaseService) {
    databaseService = new DatabaseService();
  }
  return databaseService;
}
