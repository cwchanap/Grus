import type { D1Database } from "../types/cloudflare.ts";
import type { Room, Player, GameSession, Score } from "../types/game.ts";

export interface DatabaseResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

export interface PaginationOptions {
  limit?: number;
  offset?: number;
}

export class DatabaseError extends Error {
  constructor(message: string, public override cause?: Error) {
    super(message);
    this.name = 'DatabaseError';
  }
}

export class DatabaseService {
  constructor(private db: D1Database) {}

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
  async createRoom(room: Omit<Room, 'id' | 'createdAt' | 'updatedAt'>): Promise<DatabaseResult<string>> {
    return this.executeQuery(async () => {
      const id = this.generateId();
      const stmt = this.db.prepare(`
        INSERT INTO rooms (id, name, host_id, max_players, is_active)
        VALUES (?, ?, ?, ?, ?)
      `);
      await stmt.bind(id, room.name, room.hostId, room.maxPlayers ?? 8, room.isActive ?? true).run();
      return id;
    }, 'Failed to create room');
  }

  async getRoomById(id: string): Promise<DatabaseResult<Room | null>> {
    return this.executeQuery(async () => {
      const stmt = this.db.prepare("SELECT * FROM rooms WHERE id = ?");
      const result = await stmt.bind(id).first<Room>();
      return result || null;
    }, `Failed to get room with id: ${id}`);
  }

  async getActiveRooms(options: PaginationOptions = {}): Promise<DatabaseResult<Room[]>> {
    return this.executeQuery(async () => {
      const { limit = 50, offset = 0 } = options;
      const stmt = this.db.prepare(`
        SELECT * FROM rooms 
        WHERE is_active = true 
        ORDER BY created_at DESC 
        LIMIT ? OFFSET ?
      `);
      const result = await stmt.bind(limit, offset).all<Room>();
      return result.results || [];
    }, 'Failed to get active rooms');
  }

  async getRoomWithPlayerCount(id: string): Promise<DatabaseResult<Room & { playerCount: number } | null>> {
    return this.executeQuery(async () => {
      const stmt = this.db.prepare(`
        SELECT r.*, COUNT(p.id) as player_count
        FROM rooms r
        LEFT JOIN players p ON r.id = p.room_id
        WHERE r.id = ?
        GROUP BY r.id
      `);
      const result = await stmt.bind(id).first<Room & { player_count: number }>();
      if (!result) return null;
      
      return {
        ...result,
        playerCount: result.player_count || 0
      };
    }, `Failed to get room with player count for id: ${id}`);
  }

  async updateRoom(id: string, updates: Partial<Omit<Room, 'id' | 'createdAt' | 'updatedAt'>>): Promise<DatabaseResult<void>> {
    return this.executeQuery(async () => {
      if (Object.keys(updates).length === 0) {
        throw new DatabaseError('No fields to update');
      }
      
      const fields = Object.keys(updates).map(key => `${key} = ?`).join(', ');
      const values = Object.values(updates);
      const stmt = this.db.prepare(`UPDATE rooms SET ${fields}, updated_at = CURRENT_TIMESTAMP WHERE id = ?`);
      await stmt.bind(...values, id).run();
    }, `Failed to update room with id: ${id}`);
  }

  async deleteRoom(id: string): Promise<DatabaseResult<void>> {
    return this.executeQuery(async () => {
      const stmt = this.db.prepare("DELETE FROM rooms WHERE id = ?");
      await stmt.bind(id).run();
    }, `Failed to delete room with id: ${id}`);
  }

  async setRoomInactive(id: string): Promise<DatabaseResult<void>> {
    return this.updateRoom(id, { isActive: false });
  }

  // Player operations
  async createPlayer(player: Omit<Player, 'id' | 'joinedAt'> & { roomId?: string }): Promise<DatabaseResult<string>> {
    return this.executeQuery(async () => {
      const id = this.generateId();
      const stmt = this.db.prepare(`
        INSERT INTO players (id, name, room_id, is_host)
        VALUES (?, ?, ?, ?)
      `);
      await stmt.bind(id, player.name, player.roomId || null, player.isHost ?? false).run();
      return id;
    }, 'Failed to create player');
  }

  async getPlayerById(id: string): Promise<DatabaseResult<Player | null>> {
    return this.executeQuery(async () => {
      const stmt = this.db.prepare("SELECT * FROM players WHERE id = ?");
      const result = await stmt.bind(id).first<Player>();
      return result || null;
    }, `Failed to get player with id: ${id}`);
  }

  async getPlayersByRoom(roomId: string): Promise<DatabaseResult<Player[]>> {
    return this.executeQuery(async () => {
      const stmt = this.db.prepare("SELECT * FROM players WHERE room_id = ? ORDER BY joined_at");
      const result = await stmt.bind(roomId).all<Player>();
      return result.results || [];
    }, `Failed to get players for room: ${roomId}`);
  }

  async getPlayerCount(roomId: string): Promise<DatabaseResult<number>> {
    return this.executeQuery(async () => {
      const stmt = this.db.prepare("SELECT COUNT(*) as count FROM players WHERE room_id = ?");
      const result = await stmt.bind(roomId).first<{ count: number }>();
      return result?.count || 0;
    }, `Failed to get player count for room: ${roomId}`);
  }

  async updatePlayer(id: string, updates: Partial<Omit<Player, 'id' | 'joinedAt'>>): Promise<DatabaseResult<void>> {
    return this.executeQuery(async () => {
      if (Object.keys(updates).length === 0) {
        throw new DatabaseError('No fields to update');
      }
      
      const fields = Object.keys(updates).map(key => `${key} = ?`).join(', ');
      const values = Object.values(updates);
      const stmt = this.db.prepare(`UPDATE players SET ${fields} WHERE id = ?`);
      await stmt.bind(...values, id).run();
    }, `Failed to update player with id: ${id}`);
  }

  async addPlayerToRoom(playerId: string, roomId: string): Promise<DatabaseResult<void>> {
    return this.updatePlayer(playerId, { roomId });
  }

  async removePlayerFromRoom(playerId: string): Promise<DatabaseResult<void>> {
    return this.executeQuery(async () => {
      const stmt = this.db.prepare("UPDATE players SET room_id = NULL WHERE id = ?");
      await stmt.bind(playerId).run();
    }, `Failed to remove player from room: ${playerId}`);
  }

  async deletePlayer(id: string): Promise<DatabaseResult<void>> {
    return this.executeQuery(async () => {
      const stmt = this.db.prepare("DELETE FROM players WHERE id = ?");
      await stmt.bind(id).run();
    }, `Failed to delete player with id: ${id}`);
  }

  // Game session operations
  async createGameSession(session: Omit<GameSession, 'id' | 'startedAt' | 'endedAt'>): Promise<DatabaseResult<string>> {
    return this.executeQuery(async () => {
      const id = this.generateId();
      const stmt = this.db.prepare(`
        INSERT INTO game_sessions (id, room_id, winner_id, total_rounds, started_at)
        VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
      `);
      await stmt.bind(id, session.roomId, session.winnerId || null, session.totalRounds || 0).run();
      return id;
    }, 'Failed to create game session');
  }

  async getGameSession(id: string): Promise<DatabaseResult<GameSession | null>> {
    return this.executeQuery(async () => {
      const stmt = this.db.prepare("SELECT * FROM game_sessions WHERE id = ?");
      const result = await stmt.bind(id).first<GameSession>();
      return result || null;
    }, `Failed to get game session with id: ${id}`);
  }

  async getGameSessionsByRoom(roomId: string): Promise<DatabaseResult<GameSession[]>> {
    return this.executeQuery(async () => {
      const stmt = this.db.prepare(`
        SELECT * FROM game_sessions 
        WHERE room_id = ? 
        ORDER BY started_at DESC
      `);
      const result = await stmt.all<GameSession>();
      return result.results || [];
    }, `Failed to get game sessions for room: ${roomId}`);
  }

  async endGameSession(id: string, winnerId?: string): Promise<DatabaseResult<void>> {
    return this.executeQuery(async () => {
      const stmt = this.db.prepare(`
        UPDATE game_sessions 
        SET ended_at = CURRENT_TIMESTAMP, winner_id = ?
        WHERE id = ?
      `);
      await stmt.bind(winnerId || null, id).run();
    }, `Failed to end game session with id: ${id}`);
  }

  async updateGameSession(id: string, updates: Partial<Omit<GameSession, 'id' | 'startedAt'>>): Promise<DatabaseResult<void>> {
    return this.executeQuery(async () => {
      if (Object.keys(updates).length === 0) {
        throw new DatabaseError('No fields to update');
      }
      
      const fields = Object.keys(updates).map(key => `${key} = ?`).join(', ');
      const values = Object.values(updates);
      const stmt = this.db.prepare(`UPDATE game_sessions SET ${fields} WHERE id = ?`);
      await stmt.bind(...values, id).run();
    }, `Failed to update game session with id: ${id}`);
  }

  // Score operations
  async createScore(score: Omit<Score, 'id'>): Promise<DatabaseResult<string>> {
    return this.executeQuery(async () => {
      const id = this.generateId();
      const stmt = this.db.prepare(`
        INSERT INTO scores (id, session_id, player_id, points, correct_guesses)
        VALUES (?, ?, ?, ?, ?)
      `);
      await stmt.bind(id, score.sessionId, score.playerId, score.points || 0, score.correctGuesses || 0).run();
      return id;
    }, 'Failed to create score');
  }

  async updateScore(id: string, points: number, correctGuesses: number): Promise<DatabaseResult<void>> {
    return this.executeQuery(async () => {
      const stmt = this.db.prepare(`
        UPDATE scores 
        SET points = ?, correct_guesses = ?
        WHERE id = ?
      `);
      await stmt.bind(points, correctGuesses, id).run();
    }, `Failed to update score with id: ${id}`);
  }

  async getScoresBySession(sessionId: string): Promise<DatabaseResult<Score[]>> {
    return this.executeQuery(async () => {
      const stmt = this.db.prepare("SELECT * FROM scores WHERE session_id = ? ORDER BY points DESC");
      const result = await stmt.all<Score>();
      return result.results || [];
    }, `Failed to get scores for session: ${sessionId}`);
  }

  async getScoresByPlayer(playerId: string): Promise<DatabaseResult<Score[]>> {
    return this.executeQuery(async () => {
      const stmt = this.db.prepare("SELECT * FROM scores WHERE player_id = ? ORDER BY points DESC");
      const result = await stmt.all<Score>();
      return result.results || [];
    }, `Failed to get scores for player: ${playerId}`);
  }

  async getLeaderboard(sessionId: string): Promise<DatabaseResult<Array<Score & { playerName: string }>>> {
    return this.executeQuery(async () => {
      const stmt = this.db.prepare(`
        SELECT s.*, p.name as player_name
        FROM scores s
        JOIN players p ON s.player_id = p.id
        WHERE s.session_id = ?
        ORDER BY s.points DESC, s.correct_guesses DESC
      `);
      const result = await stmt.all<Score & { player_name: string }>();
      return (result.results || []).map(row => ({
        ...row,
        playerName: row.player_name
      }));
    }, `Failed to get leaderboard for session: ${sessionId}`);
  }

  // Utility methods
  async healthCheck(): Promise<DatabaseResult<boolean>> {
    return this.executeQuery(async () => {
      const stmt = this.db.prepare("SELECT 1 as test");
      const result = await stmt.first<{ test: number }>();
      return result?.test === 1;
    }, 'Database health check failed');
  }

  async getStats(): Promise<DatabaseResult<{
    totalRooms: number;
    activeRooms: number;
    totalPlayers: number;
    totalSessions: number;
  }>> {
    return this.executeQuery(async () => {
      const roomsStmt = this.db.prepare("SELECT COUNT(*) as count FROM rooms");
      const activeRoomsStmt = this.db.prepare("SELECT COUNT(*) as count FROM rooms WHERE is_active = true");
      const playersStmt = this.db.prepare("SELECT COUNT(*) as count FROM players");
      const sessionsStmt = this.db.prepare("SELECT COUNT(*) as count FROM game_sessions");

      const [totalRooms, activeRooms, totalPlayers, totalSessions] = await Promise.all([
        roomsStmt.first<{ count: number }>(),
        activeRoomsStmt.first<{ count: number }>(),
        playersStmt.first<{ count: number }>(),
        sessionsStmt.first<{ count: number }>()
      ]);

      return {
        totalRooms: totalRooms?.count || 0,
        activeRooms: activeRooms?.count || 0,
        totalPlayers: totalPlayers?.count || 0,
        totalSessions: totalSessions?.count || 0
      };
    }, 'Failed to get database stats');
  }
}