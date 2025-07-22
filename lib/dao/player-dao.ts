import type { D1Database } from "../../types/cloudflare.ts";
import type { Player } from "../../types/game.ts";
import { DatabaseService, type DatabaseResult } from "../database.ts";

export class PlayerDAO {
  private dbService: DatabaseService;

  constructor(db: D1Database) {
    this.dbService = new DatabaseService(db);
  }

  async create(player: Omit<Player, 'id' | 'joinedAt'> & { roomId?: string }): Promise<DatabaseResult<string>> {
    return this.dbService.createPlayer(player);
  }

  async findById(id: string): Promise<DatabaseResult<Player | null>> {
    return this.dbService.getPlayerById(id);
  }

  async findByRoom(roomId: string): Promise<DatabaseResult<Player[]>> {
    return this.dbService.getPlayersByRoom(roomId);
  }

  async getCount(roomId: string): Promise<DatabaseResult<number>> {
    return this.dbService.getPlayerCount(roomId);
  }

  async update(id: string, updates: Partial<Omit<Player, 'id' | 'joinedAt'>>): Promise<DatabaseResult<void>> {
    return this.dbService.updatePlayer(id, updates);
  }

  async addToRoom(playerId: string, roomId: string): Promise<DatabaseResult<void>> {
    return this.dbService.addPlayerToRoom(playerId, roomId);
  }

  async removeFromRoom(playerId: string): Promise<DatabaseResult<void>> {
    return this.dbService.removePlayerFromRoom(playerId);
  }

  async delete(id: string): Promise<DatabaseResult<void>> {
    return this.dbService.deletePlayer(id);
  }

  async findHost(roomId: string): Promise<DatabaseResult<Player | null>> {
    const playersResult = await this.findByRoom(roomId);
    if (!playersResult.success || !playersResult.data) {
      return { success: false, error: playersResult.error || 'Failed to find players' };
    }

    const host = playersResult.data.find(player => player.isHost);
    return { success: true, data: host || null };
  }

  async transferHost(currentHostId: string, newHostId: string, roomId: string): Promise<DatabaseResult<void>> {
    // This should be done in a transaction, but D1 doesn't support transactions yet
    // So we'll do it sequentially and hope for the best
    const removeHostResult = await this.update(currentHostId, { isHost: false });
    if (!removeHostResult.success) {
      return removeHostResult;
    }

    const addHostResult = await this.update(newHostId, { isHost: true });
    if (!addHostResult.success) {
      // Try to rollback
      await this.update(currentHostId, { isHost: true });
      return addHostResult;
    }

    return { success: true };
  }

  async validatePlayerInRoom(playerId: string, roomId: string): Promise<DatabaseResult<boolean>> {
    const playerResult = await this.findById(playerId);
    if (!playerResult.success || !playerResult.data) {
      return { success: false, error: 'Player not found' };
    }

    const isInRoom = playerResult.data.roomId === roomId;
    return { success: true, data: isInRoom };
  }
}