import type { D1Database } from "../types/cloudflare.ts";
import type { Room, Player, GameSession, Score } from "../types/game.ts";

export class DatabaseService {
  constructor(private db: D1Database) {}

  // Room operations
  async createRoom(room: Omit<Room, 'createdAt' | 'updatedAt'>): Promise<void> {
    const stmt = this.db.prepare(`
      INSERT INTO rooms (id, name, host_id, max_players, is_active)
      VALUES (?, ?, ?, ?, ?)
    `);
    await stmt.bind(room.id, room.name, room.hostId, room.maxPlayers, room.isActive).run();
  }

  async getRoomById(id: string): Promise<Room | null> {
    const stmt = this.db.prepare("SELECT * FROM rooms WHERE id = ?");
    return await stmt.bind(id).first<Room>();
  }

  async getActiveRooms(): Promise<Room[]> {
    const stmt = this.db.prepare("SELECT * FROM rooms WHERE is_active = true ORDER BY created_at DESC");
    const result = await stmt.all<Room>();
    return result.results || [];
  }

  async updateRoom(id: string, updates: Partial<Room>): Promise<void> {
    const fields = Object.keys(updates).map(key => `${key} = ?`).join(', ');
    const values = Object.values(updates);
    const stmt = this.db.prepare(`UPDATE rooms SET ${fields}, updated_at = CURRENT_TIMESTAMP WHERE id = ?`);
    await stmt.bind(...values, id).run();
  }

  async deleteRoom(id: string): Promise<void> {
    const stmt = this.db.prepare("DELETE FROM rooms WHERE id = ?");
    await stmt.bind(id).run();
  }

  // Player operations
  async createPlayer(player: Omit<Player, 'joinedAt'>): Promise<void> {
    const stmt = this.db.prepare(`
      INSERT INTO players (id, name, room_id, is_host)
      VALUES (?, ?, ?, ?)
    `);
    await stmt.bind(player.id, player.name, player.roomId || null, player.isHost).run();
  }

  async getPlayerById(id: string): Promise<Player | null> {
    const stmt = this.db.prepare("SELECT * FROM players WHERE id = ?");
    return await stmt.bind(id).first<Player>();
  }

  async getPlayersByRoom(roomId: string): Promise<Player[]> {
    const stmt = this.db.prepare("SELECT * FROM players WHERE room_id = ? ORDER BY joined_at");
    const result = await stmt.all<Player>();
    return result.results || [];
  }

  async updatePlayer(id: string, updates: Partial<Player>): Promise<void> {
    const fields = Object.keys(updates).map(key => `${key} = ?`).join(', ');
    const values = Object.values(updates);
    const stmt = this.db.prepare(`UPDATE players SET ${fields} WHERE id = ?`);
    await stmt.bind(...values, id).run();
  }

  async removePlayerFromRoom(playerId: string): Promise<void> {
    const stmt = this.db.prepare("UPDATE players SET room_id = NULL WHERE id = ?");
    await stmt.bind(playerId).run();
  }

  // Game session operations
  async createGameSession(session: Omit<GameSession, 'startedAt' | 'endedAt'>): Promise<void> {
    const stmt = this.db.prepare(`
      INSERT INTO game_sessions (id, room_id, winner_id, total_rounds, started_at)
      VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
    `);
    await stmt.bind(session.id, session.roomId, session.winnerId || null, session.totalRounds).run();
  }

  async getGameSession(id: string): Promise<GameSession | null> {
    const stmt = this.db.prepare("SELECT * FROM game_sessions WHERE id = ?");
    return await stmt.bind(id).first<GameSession>();
  }

  async endGameSession(id: string, winnerId?: string): Promise<void> {
    const stmt = this.db.prepare(`
      UPDATE game_sessions 
      SET ended_at = CURRENT_TIMESTAMP, winner_id = ?
      WHERE id = ?
    `);
    await stmt.bind(winnerId || null, id).run();
  }

  // Score operations
  async createScore(score: Score): Promise<void> {
    const stmt = this.db.prepare(`
      INSERT INTO scores (id, session_id, player_id, points, correct_guesses)
      VALUES (?, ?, ?, ?, ?)
    `);
    await stmt.bind(score.id, score.sessionId, score.playerId, score.points, score.correctGuesses).run();
  }

  async updateScore(id: string, points: number, correctGuesses: number): Promise<void> {
    const stmt = this.db.prepare(`
      UPDATE scores 
      SET points = ?, correct_guesses = ?
      WHERE id = ?
    `);
    await stmt.bind(points, correctGuesses, id).run();
  }

  async getScoresBySession(sessionId: string): Promise<Score[]> {
    const stmt = this.db.prepare("SELECT * FROM scores WHERE session_id = ? ORDER BY points DESC");
    const result = await stmt.all<Score>();
    return result.results || [];
  }
}