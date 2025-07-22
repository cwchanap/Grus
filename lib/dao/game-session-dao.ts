import type { D1Database } from "../../types/cloudflare.ts";
import type { GameSession, Score } from "../../types/game.ts";
import { DatabaseService, type DatabaseResult } from "../database.ts";

export class GameSessionDAO {
  private dbService: DatabaseService;

  constructor(db: D1Database) {
    this.dbService = new DatabaseService(db);
  }

  async create(session: Omit<GameSession, 'id' | 'startedAt' | 'endedAt'>): Promise<DatabaseResult<string>> {
    return this.dbService.createGameSession(session);
  }

  async findById(id: string): Promise<DatabaseResult<GameSession | null>> {
    return this.dbService.getGameSession(id);
  }

  async findByRoom(roomId: string): Promise<DatabaseResult<GameSession[]>> {
    return this.dbService.getGameSessionsByRoom(roomId);
  }

  async update(id: string, updates: Partial<Omit<GameSession, 'id' | 'startedAt'>>): Promise<DatabaseResult<void>> {
    return this.dbService.updateGameSession(id, updates);
  }

  async end(id: string, winnerId?: string): Promise<DatabaseResult<void>> {
    return this.dbService.endGameSession(id, winnerId);
  }

  async getCurrentSession(roomId: string): Promise<DatabaseResult<GameSession | null>> {
    const sessionsResult = await this.findByRoom(roomId);
    if (!sessionsResult.success || !sessionsResult.data) {
      return { success: false, error: sessionsResult.error || 'Failed to get sessions' };
    }

    // Find the most recent session that hasn't ended
    const currentSession = sessionsResult.data.find(session => !session.endedAt);
    return { success: true, data: currentSession || null };
  }

  async getSessionStats(id: string): Promise<DatabaseResult<{
    session: GameSession;
    totalPlayers: number;
    completedRounds: number;
    leaderboard: Array<Score & { playerName: string }>;
  } | null>> {
    const sessionResult = await this.findById(id);
    if (!sessionResult.success || !sessionResult.data) {
      return { success: false, error: sessionResult.error || 'Session not found' };
    }

    const session = sessionResult.data;
    const leaderboardResult = await this.dbService.getLeaderboard(id);
    
    if (!leaderboardResult.success) {
      return { success: false, error: leaderboardResult.error };
    }

    return {
      success: true,
      data: {
        session,
        totalPlayers: leaderboardResult.data?.length || 0,
        completedRounds: session.totalRounds,
        leaderboard: leaderboardResult.data || []
      }
    };
  }
}