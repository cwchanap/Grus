// Player management service
import type { Env } from "../../../types/cloudflare.ts";

export interface PlayerInfo {
  id: string;
  name: string;
  isHost: boolean;
}

export class PlayerService {
  private env: Env;

  constructor(env: Env) {
    this.env = env;
  }

  async checkRoomExists(roomId: string): Promise<boolean> {
    try {
      // In development, we don't have access to Cloudflare DB
      // Use the database service instead
      if (!this.env?.DB) {
        const { getDatabaseService } = await import("../../database-factory.ts");
        const db = getDatabaseService();
        const result = await db.getRoomById(roomId);
        return result.success && result.data !== null;
      }

      const stmt = this.env.DB.prepare(
        "SELECT id FROM rooms WHERE id = ? AND is_active = 1",
      );
      const result = await stmt.bind(roomId).first();
      return result !== null;
    } catch (error) {
      console.error("Error checking room existence:", error);
      return false;
    }
  }

  async checkRoomCapacity(roomId: string): Promise<boolean> {
    try {
      // In development, we don't have access to Cloudflare DB
      // Use the database service instead
      if (!this.env?.DB) {
        const { getDatabaseService } = await import("../../database-factory.ts");
        const db = getDatabaseService();
        const roomResult = await db.getRoomById(roomId);
        const playersResult = await db.getPlayersByRoom(roomId);

        if (!roomResult.success || !roomResult.data) return false;
        if (!playersResult.success) return false;

        const playerCount = playersResult.data?.length || 0;
        return playerCount < roomResult.data.maxPlayers;
      }

      const roomStmt = this.env.DB.prepare(
        "SELECT max_players FROM rooms WHERE id = ?",
      );
      const room = await roomStmt.bind(roomId).first<{ max_players: number }>();

      if (!room) return false;

      const playerStmt = this.env.DB.prepare(
        "SELECT COUNT(*) as count FROM players WHERE room_id = ?",
      );
      const playerCount = await playerStmt
        .bind(roomId)
        .first<{ count: number }>();

      return (playerCount?.count || 0) < room.max_players;
    } catch (error) {
      console.error("Error checking room capacity:", error);
      return false;
    }
  }

  async verifyPlayerIsHost(playerId: string, roomId: string): Promise<boolean> {
    try {
      // In development, we don't have access to Cloudflare DB
      // Use the database service instead
      if (!this.env?.DB) {
        const { getDatabaseService } = await import("../../database-factory.ts");
        const db = getDatabaseService();
        const result = await db.getPlayerById(playerId);
        return result.success && result.data?.isHost === true;
      }

      const stmt = this.env.DB.prepare(
        "SELECT is_host FROM players WHERE id = ? AND room_id = ?",
      );
      const result = await stmt
        .bind(playerId, roomId)
        .first<{ is_host: number }>();
      return result?.is_host === 1;
    } catch (error) {
      console.error("Error verifying host status:", error);
      return false;
    }
  }

  async getPlayersFromDatabase(roomId: string): Promise<PlayerInfo[] | null> {
    try {
      // In development, we don't have access to Cloudflare DB
      // Use the database service instead
      if (!this.env?.DB) {
        const { getDatabaseService } = await import("../../database-factory.ts");
        const db = getDatabaseService();
        const result = await db.getPlayersByRoom(roomId);
        if (result.success && result.data) {
          return result.data.map((player: any) => ({
            id: player.id,
            name: player.name,
            isHost: player.isHost,
          }));
        }
        return null;
      }

      const stmt = this.env.DB.prepare(
        "SELECT id, name, is_host FROM players WHERE room_id = ? ORDER BY joined_at ASC",
      );
      const results = await stmt.bind(roomId).all<{
        id: string;
        name: string;
        is_host: number;
      }>();

      if (!results.success) {
        console.error("Failed to get players from database:", results.error);
        return null;
      }

      return (results.results || []).map((row) => ({
        id: row.id,
        name: row.name,
        isHost: row.is_host === 1,
      }));
    } catch (error) {
      console.error("Error getting players from database:", error);
      return null;
    }
  }
}
