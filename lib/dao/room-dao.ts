import type { D1Database } from "../../types/cloudflare.ts";
import type { Room } from "../../types/game.ts";
import { DatabaseService, type DatabaseResult, type PaginationOptions } from "../database.ts";

export class RoomDAO {
  private dbService: DatabaseService;

  constructor(db: D1Database) {
    this.dbService = new DatabaseService(db);
  }

  async create(room: Omit<Room, 'id' | 'createdAt' | 'updatedAt'>): Promise<DatabaseResult<string>> {
    return this.dbService.createRoom(room);
  }

  async findById(id: string): Promise<DatabaseResult<Room | null>> {
    return this.dbService.getRoomById(id);
  }

  async findActive(options?: PaginationOptions): Promise<DatabaseResult<Room[]>> {
    return this.dbService.getActiveRooms(options);
  }

  async findWithPlayerCount(id: string): Promise<DatabaseResult<Room & { playerCount: number } | null>> {
    return this.dbService.getRoomWithPlayerCount(id);
  }

  async update(id: string, updates: Partial<Omit<Room, 'id' | 'createdAt' | 'updatedAt'>>): Promise<DatabaseResult<void>> {
    return this.dbService.updateRoom(id, updates);
  }

  async delete(id: string): Promise<DatabaseResult<void>> {
    return this.dbService.deleteRoom(id);
  }

  async setInactive(id: string): Promise<DatabaseResult<void>> {
    return this.dbService.setRoomInactive(id);
  }

  async canJoin(roomId: string): Promise<DatabaseResult<boolean>> {
    const roomResult = await this.findWithPlayerCount(roomId);
    if (!roomResult.success || !roomResult.data) {
      return { success: false, error: 'Room not found' };
    }

    const room = roomResult.data;
    const canJoin = room.isActive && room.playerCount < room.maxPlayers;
    
    return { success: true, data: canJoin };
  }

  async getRoomSummary(id: string): Promise<DatabaseResult<{
    room: Room;
    playerCount: number;
    canJoin: boolean;
  } | null>> {
    const roomResult = await this.findWithPlayerCount(id);
    if (!roomResult.success || !roomResult.data) {
      return { success: false, error: roomResult.error || 'Room not found' };
    }

    const room = roomResult.data;
    const canJoin = room.isActive && room.playerCount < room.maxPlayers;

    return {
      success: true,
      data: {
        room: {
          id: room.id,
          name: room.name,
          hostId: room.hostId,
          maxPlayers: room.maxPlayers,
          isActive: room.isActive,
          createdAt: room.createdAt,
          updatedAt: room.updatedAt
        },
        playerCount: room.playerCount,
        canJoin
      }
    };
  }
}