// Simplified database service using Cloudflare REST API
import { getCloudflareAPI } from "./cloudflare-api.ts";
import type { Room, Player, GameSession, Score } from "../types/game.ts";

export interface DatabaseResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

export class DatabaseService {
  private api = getCloudflareAPI();

  private async executeQuery<T>(
    operation: () => Promise<T>,
    errorMessage: string
  ): Promise<DatabaseResult<T>> {
    try {
      const data = await operation();
      return { success: true, data };
    } catch (error) {
      console.error(`Database error: ${errorMessage}`, error);
      return { 
        success: false, 
        error: error instanceof Error ? error.message : errorMessage 
      };
    }
  }

  private generateId(): string {
    return crypto.randomUUID();
  }

  // Room operations
  async createRoom(name: string, hostId: string, maxPlayers = 8): Promise<DatabaseResult<string>> {
    return this.executeQuery(async () => {
      const id = this.generateId();
      const sql = `
        INSERT INTO rooms (id, name, host_id, max_players, is_active, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, datetime('now'), datetime('now'))
      `;
      await this.api.executeD1Query(sql, [id, name, hostId, maxPlayers, true]);
      return id;
    }, 'Failed to create room');
  }

  async getRoomById(id: string): Promise<DatabaseResult<Room | null>> {
    return this.executeQuery(async () => {
      const sql = "SELECT * FROM rooms WHERE id = ?";
      const result = await this.api.executeD1Query(sql, [id]);
      return result[0]?.results?.[0] || null;
    }, `Failed to get room with id: ${id}`);
  }

  async getActiveRooms(limit = 20): Promise<DatabaseResult<Room[]>> {
    return this.executeQuery(async () => {
      const sql = `
        SELECT r.*, COUNT(p.id) as player_count
        FROM rooms r
        LEFT JOIN players p ON r.id = p.room_id
        WHERE r.is_active = true 
        GROUP BY r.id
        ORDER BY r.created_at DESC 
        LIMIT ?
      `;
      const result = await this.api.executeD1Query(sql, [limit]);
      return result[0]?.results || [];
    }, 'Failed to get active rooms');
  }

  async updateRoom(id: string, updates: Partial<Room>): Promise<DatabaseResult<boolean>> {
    return this.executeQuery(async () => {
      const fields = Object.keys(updates).map(key => `${key} = ?`).join(', ');
      const values = Object.values(updates);
      const sql = `UPDATE rooms SET ${fields}, updated_at = datetime('now') WHERE id = ?`;
      await this.api.executeD1Query(sql, [...values, id]);
      return true;
    }, `Failed to update room with id: ${id}`);
  }

  async deleteRoom(id: string): Promise<DatabaseResult<boolean>> {
    return this.executeQuery(async () => {
      const sql = "DELETE FROM rooms WHERE id = ?";
      await this.api.executeD1Query(sql, [id]);
      return true;
    }, `Failed to delete room with id: ${id}`);
  }

  // Player operations
  async createPlayer(name: string, roomId: string, isHost = false): Promise<DatabaseResult<string>> {
    return this.executeQuery(async () => {
      const id = this.generateId();
      const sql = `
        INSERT INTO players (id, name, room_id, is_host, joined_at)
        VALUES (?, ?, ?, ?, datetime('now'))
      `;
      await this.api.executeD1Query(sql, [id, name, roomId, isHost]);
      return id;
    }, 'Failed to create player');
  }

  async getPlayerById(id: string): Promise<DatabaseResult<Player | null>> {
    return this.executeQuery(async () => {
      const sql = "SELECT * FROM players WHERE id = ?";
      const result = await this.api.executeD1Query(sql, [id]);
      return result[0]?.results?.[0] || null;
    }, `Failed to get player with id: ${id}`);
  }

  async getPlayersByRoom(roomId: string): Promise<DatabaseResult<Player[]>> {
    return this.executeQuery(async () => {
      const sql = "SELECT * FROM players WHERE room_id = ? ORDER BY joined_at ASC";
      const result = await this.api.executeD1Query(sql, [roomId]);
      return result[0]?.results || [];
    }, `Failed to get players for room: ${roomId}`);
  }

  async removePlayer(id: string): Promise<DatabaseResult<boolean>> {
    return this.executeQuery(async () => {
      const sql = "DELETE FROM players WHERE id = ?";
      await this.api.executeD1Query(sql, [id]);
      return true;
    }, `Failed to remove player with id: ${id}`);
  }

  // Game session operations
  async createGameSession(roomId: string, totalRounds = 5): Promise<DatabaseResult<string>> {
    return this.executeQuery(async () => {
      const id = this.generateId();
      const sql = `
        INSERT INTO game_sessions (id, room_id, total_rounds, started_at)
        VALUES (?, ?, ?, datetime('now'))
      `;
      await this.api.executeD1Query(sql, [id, roomId, totalRounds]);
      return id;
    }, 'Failed to create game session');
  }

  async endGameSession(id: string, winnerId?: string): Promise<DatabaseResult<boolean>> {
    return this.executeQuery(async () => {
      const sql = `
        UPDATE game_sessions 
        SET ended_at = datetime('now'), winner_id = ?
        WHERE id = ?
      `;
      await this.api.executeD1Query(sql, [winnerId || null, id]);
      return true;
    }, `Failed to end game session with id: ${id}`);
  }

  // Score operations
  async createScore(sessionId: string, playerId: string, points = 0): Promise<DatabaseResult<string>> {
    return this.executeQuery(async () => {
      const id = this.generateId();
      const sql = `
        INSERT INTO scores (id, session_id, player_id, points, correct_guesses)
        VALUES (?, ?, ?, ?, 0)
      `;
      await this.api.executeD1Query(sql, [id, sessionId, playerId, points]);
      return id;
    }, 'Failed to create score');
  }

  async updateScore(id: string, points: number, correctGuesses: number): Promise<DatabaseResult<boolean>> {
    return this.executeQuery(async () => {
      const sql = `
        UPDATE scores 
        SET points = ?, correct_guesses = ?
        WHERE id = ?
      `;
      await this.api.executeD1Query(sql, [points, correctGuesses, id]);
      return true;
    }, `Failed to update score with id: ${id}`);
  }

  async getScoresBySession(sessionId: string): Promise<DatabaseResult<Score[]>> {
    return this.executeQuery(async () => {
      const sql = `
        SELECT s.*, p.name as player_name
        FROM scores s
        JOIN players p ON s.player_id = p.id
        WHERE s.session_id = ?
        ORDER BY s.points DESC
      `;
      const result = await this.api.executeD1Query(sql, [sessionId]);
      return result[0]?.results || [];
    }, `Failed to get scores for session: ${sessionId}`);
  }

  // Health check
  async healthCheck(): Promise<DatabaseResult<boolean>> {
    return this.executeQuery(async () => {
      const sql = "SELECT 1 as test";
      const result = await this.api.executeD1Query(sql);
      return result[0]?.results?.[0]?.test === 1;
    }, 'Database health check failed');
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