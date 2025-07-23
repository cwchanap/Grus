import type { D1Database } from "../types/cloudflare.ts";
import type { Room, Player } from "../types/game.ts";
import { RoomDAO } from "./dao/room-dao.ts";
import { PlayerDAO } from "./dao/player-dao.ts";
import type { DatabaseResult } from "./database.ts";

export interface RoomCreationOptions {
  name: string;
  hostName: string;
  maxPlayers?: number;
}

export interface RoomJoinOptions {
  roomId: string;
  playerName: string;
}

export interface RoomListOptions {
  limit?: number;
  offset?: number;
  filterByCapacity?: boolean;
}

export interface RoomSummary {
  room: Room;
  playerCount: number;
  canJoin: boolean;
  players: Player[];
  host: Player | null;
}

export interface RoomValidationResult {
  isValid: boolean;
  reason?: string;
}

export class RoomManager {
  private roomDAO: RoomDAO;
  private playerDAO: PlayerDAO;

  constructor(db: D1Database) {
    this.roomDAO = new RoomDAO(db);
    this.playerDAO = new PlayerDAO(db);
  }

  /**
   * Create a new room with the specified host
   */
  async createRoom(options: RoomCreationOptions): Promise<DatabaseResult<{ roomId: string; playerId: string }>> {
    try {
      // Validate room name
      if (!options.name || options.name.trim().length === 0) {
        return { success: false, error: 'Room name is required' };
      }

      if (options.name.length > 50) {
        return { success: false, error: 'Room name must be 50 characters or less' };
      }

      // Validate host name
      if (!options.hostName || options.hostName.trim().length === 0) {
        return { success: false, error: 'Host name is required' };
      }

      if (options.hostName.length > 30) {
        return { success: false, error: 'Host name must be 30 characters or less' };
      }

      // Validate max players
      const maxPlayers = options.maxPlayers ?? 8;
      if (maxPlayers < 2 || maxPlayers > 16) {
        return { success: false, error: 'Max players must be between 2 and 16' };
      }

      // Create host player first
      const hostResult = await this.playerDAO.create({
        name: options.hostName.trim(),
        isHost: true
      });

      if (!hostResult.success || !hostResult.data) {
        return { success: false, error: hostResult.error || 'Failed to create host player' };
      }

      const hostId = hostResult.data;

      // Create room
      const roomResult = await this.roomDAO.create({
        name: options.name.trim(),
        hostId,
        maxPlayers,
        isActive: true
      });

      if (!roomResult.success || !roomResult.data) {
        // Cleanup host player if room creation failed
        await this.playerDAO.delete(hostId);
        return { success: false, error: roomResult.error || 'Failed to create room' };
      }

      const roomId = roomResult.data;

      // Add host to room
      const addToRoomResult = await this.playerDAO.addToRoom(hostId, roomId);
      if (!addToRoomResult.success) {
        // Cleanup on failure
        await this.roomDAO.delete(roomId);
        await this.playerDAO.delete(hostId);
        return { success: false, error: addToRoomResult.error || 'Failed to add host to room' };
      }

      return {
        success: true,
        data: { roomId, playerId: hostId }
      };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error occurred'
      };
    }
  }

  /**
   * Join an existing room
   */
  async joinRoom(options: RoomJoinOptions): Promise<DatabaseResult<{ playerId: string }>> {
    try {
      // Validate player name
      if (!options.playerName || options.playerName.trim().length === 0) {
        return { success: false, error: 'Player name is required' };
      }

      if (options.playerName.length > 30) {
        return { success: false, error: 'Player name must be 30 characters or less' };
      }

      // Check if room exists and can be joined
      const canJoinResult = await this.canJoinRoom(options.roomId);
      if (!canJoinResult.success) {
        return { success: false, error: canJoinResult.error };
      }

      if (!canJoinResult.data?.canJoin) {
        return { success: false, error: canJoinResult.data?.reason || 'Cannot join room' };
      }

      // Check for duplicate player names in the room
      const playersResult = await this.playerDAO.findByRoom(options.roomId);
      if (playersResult.success && playersResult.data) {
        const existingPlayer = playersResult.data.find(
          player => player.name.toLowerCase() === options.playerName.trim().toLowerCase()
        );
        if (existingPlayer) {
          return { success: false, error: 'A player with this name already exists in the room' };
        }
      }

      // Create player
      const playerResult = await this.playerDAO.create({
        name: options.playerName.trim(),
        isHost: false
      });

      if (!playerResult.success || !playerResult.data) {
        return { success: false, error: playerResult.error || 'Failed to create player' };
      }

      const playerId = playerResult.data;

      // Add player to room
      const addToRoomResult = await this.playerDAO.addToRoom(playerId, options.roomId);
      if (!addToRoomResult.success) {
        // Cleanup player if adding to room failed
        await this.playerDAO.delete(playerId);
        return { success: false, error: addToRoomResult.error || 'Failed to join room' };
      }

      return {
        success: true,
        data: { playerId }
      };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error occurred'
      };
    }
  }

  /**
   * Remove a player from a room
   */
  async leaveRoom(playerId: string): Promise<DatabaseResult<{ wasHost: boolean; newHostId?: string }>> {
    try {
      // Get player info
      const playerResult = await this.playerDAO.findById(playerId);
      if (!playerResult.success || !playerResult.data) {
        return { success: false, error: 'Player not found' };
      }

      const player = playerResult.data;
      const roomId = player.roomId;
      const wasHost = player.isHost;

      if (!roomId) {
        return { success: false, error: 'Player is not in a room' };
      }

      // Remove player from room
      const removeResult = await this.playerDAO.removeFromRoom(playerId);
      if (!removeResult.success) {
        return { success: false, error: removeResult.error || 'Failed to remove player from room' };
      }

      // Delete player
      await this.playerDAO.delete(playerId);

      let newHostId: string | undefined;

      // If the leaving player was the host, transfer host privileges
      if (wasHost) {
        const remainingPlayersResult = await this.playerDAO.findByRoom(roomId);
        if (remainingPlayersResult.success && remainingPlayersResult.data && remainingPlayersResult.data.length > 0) {
          // Transfer host to the first remaining player
          const newHost = remainingPlayersResult.data[0];
          const transferResult = await this.playerDAO.update(newHost.id, { isHost: true });
          if (transferResult.success) {
            newHostId = newHost.id;
          }
        } else {
          // No players left, deactivate the room
          await this.roomDAO.setInactive(roomId);
        }
      }

      return {
        success: true,
        data: { wasHost, newHostId }
      };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error occurred'
      };
    }
  }

  /**
   * Get list of active rooms with filtering options
   */
  async listRooms(options: RoomListOptions = {}): Promise<DatabaseResult<RoomSummary[]>> {
    try {
      const { limit = 20, offset = 0, filterByCapacity = true } = options;

      // Get active rooms
      const roomsResult = await this.roomDAO.findActive({ limit, offset });
      if (!roomsResult.success || !roomsResult.data) {
        return { success: false, error: roomsResult.error || 'Failed to get rooms' };
      }

      const rooms = roomsResult.data;
      const roomSummaries: RoomSummary[] = [];

      // Get detailed info for each room
      for (const room of rooms) {
        const summaryResult = await this.getRoomSummary(room.id);
        if (summaryResult.success && summaryResult.data) {
          // Apply capacity filter if requested
          if (!filterByCapacity || summaryResult.data.canJoin) {
            roomSummaries.push(summaryResult.data);
          }
        }
      }

      return { success: true, data: roomSummaries };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error occurred'
      };
    }
  }

  /**
   * Get detailed information about a specific room
   */
  async getRoomSummary(roomId: string): Promise<DatabaseResult<RoomSummary | null>> {
    try {
      // Get room with player count
      const roomResult = await this.roomDAO.getRoomSummary(roomId);
      if (!roomResult.success || !roomResult.data) {
        return { success: false, error: roomResult.error || 'Room not found' };
      }

      const { room, playerCount, canJoin } = roomResult.data;

      // Get players in room
      const playersResult = await this.playerDAO.findByRoom(roomId);
      if (!playersResult.success) {
        return { success: false, error: playersResult.error || 'Failed to get players' };
      }

      const players = playersResult.data || [];
      const host = players.find(player => player.isHost) || null;

      return {
        success: true,
        data: {
          room,
          playerCount,
          canJoin,
          players,
          host
        }
      };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error occurred'
      };
    }
  }

  /**
   * Check if a room can be joined
   */
  async canJoinRoom(roomId: string): Promise<DatabaseResult<{ canJoin: boolean; reason?: string }>> {
    try {
      // Get room info
      const roomResult = await this.roomDAO.findWithPlayerCount(roomId);
      if (!roomResult.success || !roomResult.data) {
        return {
          success: true,
          data: { canJoin: false, reason: 'Room not found' }
        };
      }

      const room = roomResult.data;

      // Check if room is active
      if (!room.isActive) {
        return {
          success: true,
          data: { canJoin: false, reason: 'Room is not active' }
        };
      }

      // Check capacity
      if (room.playerCount >= room.maxPlayers) {
        return {
          success: true,
          data: { canJoin: false, reason: 'Room is full' }
        };
      }

      return {
        success: true,
        data: { canJoin: true }
      };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error occurred'
      };
    }
  }

  /**
   * Validate room state and capacity
   */
  async validateRoom(roomId: string): Promise<DatabaseResult<RoomValidationResult>> {
    try {
      const summaryResult = await this.getRoomSummary(roomId);
      if (!summaryResult.success || !summaryResult.data) {
        return {
          success: true,
          data: { isValid: false, reason: 'Room not found' }
        };
      }

      const { room, players, host } = summaryResult.data;

      // Check if room is active
      if (!room.isActive) {
        return {
          success: true,
          data: { isValid: false, reason: 'Room is inactive' }
        };
      }

      // Check if there's a host
      if (!host) {
        return {
          success: true,
          data: { isValid: false, reason: 'Room has no host' }
        };
      }

      // Check player count consistency
      if (players.length > room.maxPlayers) {
        return {
          success: true,
          data: { isValid: false, reason: 'Room exceeds maximum capacity' }
        };
      }

      // Check for duplicate player names
      const playerNames = players.map(p => p.name.toLowerCase());
      const uniqueNames = new Set(playerNames);
      if (playerNames.length !== uniqueNames.size) {
        return {
          success: true,
          data: { isValid: false, reason: 'Room has duplicate player names' }
        };
      }

      return {
        success: true,
        data: { isValid: true }
      };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error occurred'
      };
    }
  }

  /**
   * Transfer host privileges to another player
   */
  async transferHost(roomId: string, currentHostId: string, newHostId: string): Promise<DatabaseResult<void>> {
    try {
      // Validate that both players are in the room
      const currentHostValid = await this.playerDAO.validatePlayerInRoom(currentHostId, roomId);
      const newHostValid = await this.playerDAO.validatePlayerInRoom(newHostId, roomId);

      if (!currentHostValid.success || !currentHostValid.data) {
        return { success: false, error: 'Current host is not in the room' };
      }

      if (!newHostValid.success || !newHostValid.data) {
        return { success: false, error: 'New host is not in the room' };
      }

      // Verify current host is actually the host
      const currentHostResult = await this.playerDAO.findById(currentHostId);
      if (!currentHostResult.success || !currentHostResult.data || !currentHostResult.data.isHost) {
        return { success: false, error: 'Current player is not the host' };
      }

      // Transfer host privileges
      return await this.playerDAO.transferHost(currentHostId, newHostId, roomId);
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error occurred'
      };
    }
  }

  /**
   * Cleanup inactive rooms and orphaned players
   */
  async cleanup(): Promise<DatabaseResult<{ roomsCleaned: number; playersCleaned: number }>> {
    try {
      let roomsCleaned = 0;
      let playersCleaned = 0;

      // Get all active rooms
      const roomsResult = await this.roomDAO.findActive({ limit: 1000 });
      if (!roomsResult.success || !roomsResult.data) {
        return { success: false, error: 'Failed to get rooms for cleanup' };
      }

      // Check each room for validity
      for (const room of roomsResult.data) {
        const validationResult = await this.validateRoom(room.id);
        if (validationResult.success && validationResult.data && !validationResult.data.isValid) {
          // Deactivate invalid rooms
          await this.roomDAO.setInactive(room.id);
          roomsCleaned++;
        }
      }

      // TODO: Add logic to clean up orphaned players (players not in any room)
      // This would require additional database queries

      return {
        success: true,
        data: { roomsCleaned, playersCleaned }
      };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error occurred'
      };
    }
  }
}