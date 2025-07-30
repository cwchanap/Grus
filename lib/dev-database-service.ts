// Development database service using in-memory storage
import type { GameSession, Player, Room, Score } from "../types/game.ts";

export interface DatabaseResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

// In-memory storage
const rooms = new Map<string, Room>();
const players = new Map<string, Player>();
const gameSessions = new Map<string, GameSession>();
const scores = new Map<string, Score>();

export class DevDatabaseService {
  private generateId(): string {
    return crypto.randomUUID();
  }

  // Room operations
  async createRoom(name: string, hostId: string, maxPlayers = 8): Promise<DatabaseResult<string>> {
    try {
      const id = this.generateId();
      const room: Room = {
        id,
        name,
        hostId,
        maxPlayers,
        isActive: true,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };

      rooms.set(id, room);
      return { success: true, data: id };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : "Failed to create room",
      };
    }
  }

  async getRoomById(id: string): Promise<DatabaseResult<Room | null>> {
    try {
      const room = rooms.get(id) || null;
      return { success: true, data: room };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : `Failed to get room with id: ${id}`,
      };
    }
  }

  async getActiveRooms(limit = 20): Promise<DatabaseResult<Room[]>> {
    try {
      const activeRooms = Array.from(rooms.values())
        .filter((room) => room.isActive)
        .sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime())
        .slice(0, limit);

      return { success: true, data: activeRooms };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : "Failed to get active rooms",
      };
    }
  }

  async updateRoom(id: string, updates: Partial<Room>): Promise<DatabaseResult<boolean>> {
    try {
      const room = rooms.get(id);
      if (!room) {
        return { success: false, error: "Room not found" };
      }

      const updatedRoom = {
        ...room,
        ...updates,
        updatedAt: new Date().toISOString(),
      };

      rooms.set(id, updatedRoom);
      return { success: true, data: true };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : `Failed to update room with id: ${id}`,
      };
    }
  }

  async deleteRoom(id: string): Promise<DatabaseResult<boolean>> {
    try {
      // Also remove all players from this room
      for (const [playerId, player] of players.entries()) {
        if (player.roomId === id) {
          players.delete(playerId);
        }
      }

      const deleted = rooms.delete(id);
      return { success: true, data: deleted };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : `Failed to delete room with id: ${id}`,
      };
    }
  }

  // Player operations
  async createPlayer(
    name: string,
    roomId: string,
    isHost = false,
  ): Promise<DatabaseResult<string>> {
    try {
      const id = this.generateId();
      const player: Player = {
        id,
        name,
        roomId,
        isHost,
        isConnected: true,
        lastActivity: Date.now(),
        joinedAt: new Date().toISOString(),
      };

      players.set(id, player);
      return { success: true, data: id };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : "Failed to create player",
      };
    }
  }

  async getPlayerById(id: string): Promise<DatabaseResult<Player | null>> {
    try {
      const player = players.get(id) || null;
      return { success: true, data: player };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : `Failed to get player with id: ${id}`,
      };
    }
  }

  async getPlayersByRoom(roomId: string): Promise<DatabaseResult<Player[]>> {
    try {
      const roomPlayers = Array.from(players.values())
        .filter((player) => player.roomId === roomId)
        .sort((a, b) => new Date(a.joinedAt).getTime() - new Date(b.joinedAt).getTime());

      return { success: true, data: roomPlayers };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : `Failed to get players for room: ${roomId}`,
      };
    }
  }

  async removePlayer(id: string): Promise<DatabaseResult<boolean>> {
    try {
      const deleted = players.delete(id);
      return { success: true, data: deleted };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : `Failed to remove player with id: ${id}`,
      };
    }
  }

  // Game session operations
  async createGameSession(roomId: string, totalRounds = 5): Promise<DatabaseResult<string>> {
    try {
      const id = this.generateId();
      const session: GameSession = {
        id,
        roomId,
        totalRounds,
        currentRound: 0,
        startedAt: new Date().toISOString(),
        endedAt: null,
        winnerId: null,
      };

      gameSessions.set(id, session);
      return { success: true, data: id };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : "Failed to create game session",
      };
    }
  }

  async endGameSession(id: string, winnerId?: string): Promise<DatabaseResult<boolean>> {
    try {
      const session = gameSessions.get(id);
      if (!session) {
        return { success: false, error: "Game session not found" };
      }

      const updatedSession = {
        ...session,
        endedAt: new Date().toISOString(),
        winnerId: winnerId || null,
      };

      gameSessions.set(id, updatedSession);
      return { success: true, data: true };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : `Failed to end game session with id: ${id}`,
      };
    }
  }

  // Score operations
  async createScore(
    sessionId: string,
    playerId: string,
    points = 0,
  ): Promise<DatabaseResult<string>> {
    try {
      const id = this.generateId();
      const score: Score = {
        id,
        sessionId,
        playerId,
        points,
        correctGuesses: 0,
      };

      scores.set(id, score);
      return { success: true, data: id };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : "Failed to create score",
      };
    }
  }

  async updateScore(
    id: string,
    points: number,
    correctGuesses: number,
  ): Promise<DatabaseResult<boolean>> {
    try {
      const score = scores.get(id);
      if (!score) {
        return { success: false, error: "Score not found" };
      }

      const updatedScore = {
        ...score,
        points,
        correctGuesses,
      };

      scores.set(id, updatedScore);
      return { success: true, data: true };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : `Failed to update score with id: ${id}`,
      };
    }
  }

  async getScoresBySession(sessionId: string): Promise<DatabaseResult<Score[]>> {
    try {
      const sessionScores = Array.from(scores.values())
        .filter((score) => score.sessionId === sessionId)
        .sort((a, b) => b.points - a.points);

      return { success: true, data: sessionScores };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error
          ? error.message
          : `Failed to get scores for session: ${sessionId}`,
      };
    }
  }

  // Health check
  async healthCheck(): Promise<DatabaseResult<boolean>> {
    return { success: true, data: true };
  }

  // Development utilities
  clearAll(): void {
    rooms.clear();
    players.clear();
    gameSessions.clear();
    scores.clear();
  }

  getStats(): { rooms: number; players: number; sessions: number; scores: number } {
    return {
      rooms: rooms.size,
      players: players.size,
      sessions: gameSessions.size,
      scores: scores.size,
    };
  }
}

// Singleton instance
let devDatabaseService: DevDatabaseService | null = null;

export function getDevDatabaseService(): DevDatabaseService {
  if (!devDatabaseService) {
    devDatabaseService = new DevDatabaseService();
  }
  return devDatabaseService;
}
