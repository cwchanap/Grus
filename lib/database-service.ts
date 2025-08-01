// Database service using local SQLite
import { Database } from "https://deno.land/x/sqlite3@0.12.0/mod.ts";
import type { GameSession, Player, Room, Score } from "../types/game.ts";

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
  async createRoom(name: string, hostId: string, maxPlayers = 8): Promise<DatabaseResult<string>> {
    return this.executeQuery(() => {
      const id = this.generateId();
      const stmt = this.db.prepare(
        `INSERT INTO rooms (id, name, host_id, max_players, is_active, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, datetime('now'), datetime('now'))`,
      );
      stmt.run(id, name, hostId, maxPlayers, true);
      stmt.finalize();
      return id;
    }, "Failed to create room");
  }

  async getRoomById(id: string): Promise<DatabaseResult<Room | null>> {
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
        isActive: Boolean(result.is_active),
        createdAt: result.created_at,
        updatedAt: result.updated_at,
      } as Room;
    }, `Failed to get room with id: ${id}`);
  }

  async getActiveRooms(limit = 20): Promise<DatabaseResult<Room[]>> {
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
        isActive: Boolean(result.is_active),
        createdAt: result.created_at,
        updatedAt: result.updated_at,
        player_count: result.player_count,
      })) as Room[];
    }, "Failed to get active rooms");
  }

  async updateRoom(id: string, updates: Partial<Room>): Promise<DatabaseResult<boolean>> {
    return this.executeQuery(() => {
      const fields = Object.keys(updates).map((key) => `${key} = ?`).join(", ");
      const values = Object.values(updates);
      const stmt = this.db.prepare(
        `UPDATE rooms SET ${fields}, updated_at = datetime('now') WHERE id = ?`,
      );
      stmt.run(...values, id);
      stmt.finalize();
      return true;
    }, `Failed to update room with id: ${id}`);
  }

  async deleteRoom(id: string): Promise<DatabaseResult<boolean>> {
    return this.executeQuery(() => {
      const stmt = this.db.prepare("DELETE FROM rooms WHERE id = ?");
      stmt.run(id);
      stmt.finalize();
      return true;
    }, `Failed to delete room with id: ${id}`);
  }

  // Player operations
  async createPlayer(
    name: string,
    roomId: string,
    isHost = false,
  ): Promise<DatabaseResult<string>> {
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

  async getPlayerById(id: string): Promise<DatabaseResult<Player | null>> {
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

  async getPlayersByRoom(roomId: string): Promise<DatabaseResult<Player[]>> {
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

  async removePlayer(id: string): Promise<DatabaseResult<boolean>> {
    return this.executeQuery(() => {
      const stmt = this.db.prepare("DELETE FROM players WHERE id = ?");
      stmt.run(id);
      stmt.finalize();
      return true;
    }, `Failed to remove player with id: ${id}`);
  }

  // Game session operations
  async createGameSession(roomId: string, totalRounds = 5): Promise<DatabaseResult<string>> {
    return this.executeQuery(() => {
      const id = this.generateId();
      const stmt = this.db.prepare(
        `INSERT INTO game_sessions (id, room_id, total_rounds, started_at)
         VALUES (?, ?, ?, datetime('now'))`,
      );
      stmt.run(id, roomId, totalRounds);
      stmt.finalize();
      return id;
    }, "Failed to create game session");
  }

  async endGameSession(id: string, winnerId?: string): Promise<DatabaseResult<boolean>> {
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
  async createScore(
    sessionId: string,
    playerId: string,
    points = 0,
  ): Promise<DatabaseResult<string>> {
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

  async updateScore(
    id: string,
    points: number,
    correctGuesses: number,
  ): Promise<DatabaseResult<boolean>> {
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

  async getScoresBySession(sessionId: string): Promise<DatabaseResult<Score[]>> {
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
  async healthCheck(): Promise<DatabaseResult<boolean>> {
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
