import { DatabaseService } from "./database-service.ts";
import { GameState, Player, Room } from "../types/game.ts";

export interface RoomSummary {
  room: Room;
  players: Player[];
  playerCount: number;
  canJoin: boolean;
  host: Player | null;
}

export interface JoinRoomParams {
  roomId: string;
  playerName: string;
}

export interface JoinRoomResult {
  success: boolean;
  error?: string;
  data?: {
    playerId: string;
    room: RoomSummary;
  };
}

export interface CreateRoomParams {
  hostName: string;
  maxPlayers?: number;
}

export interface CreateRoomResult {
  success: boolean;
  error?: string;
  data?: {
    roomId: string;
    playerId: string;
    room: RoomSummary;
  };
}

export class RoomManager {
  private db: DatabaseService;

  constructor() {
    this.db = new DatabaseService();
  }

  async createRoom(params: CreateRoomParams): Promise<CreateRoomResult> {
    try {
      const { hostName, maxPlayers = 8 } = params;

      if (!hostName?.trim()) {
        return { success: false, error: "Host name is required" };
      }

      // Generate room ID
      const roomId = this.generateRoomId();

      // Create room
      const roomResult = await this.db.createRoom(roomId, "temp-host-id", maxPlayers);

      if (!roomResult.success || !roomResult.data) {
        return { success: false, error: roomResult.error || "Failed to create room" };
      }

      // Create host player
      const playerResult = await this.db.createPlayer(
        hostName.trim(),
        roomId,
        true,
      );

      if (!playerResult.success || !playerResult.data) {
        return { success: false, error: "Failed to create host player" };
      }

      const hostPlayerId = playerResult.data;

      // Get the created room with players
      const roomSummary = await this.getRoomSummary(roomId);
      if (!roomSummary.success || !roomSummary.data) {
        return { success: false, error: "Failed to get room summary" };
      }

      return {
        success: true,
        data: {
          roomId,
          playerId: hostPlayerId,
          room: roomSummary.data,
        },
      };
    } catch (error) {
      console.error("Error creating room:", error);
      return { success: false, error: "Internal server error" };
    }
  }

  async joinRoom(params: JoinRoomParams): Promise<JoinRoomResult> {
    try {
      const { roomId, playerName } = params;

      if (!roomId?.trim()) {
        return { success: false, error: "Room ID is required" };
      }

      if (!playerName?.trim()) {
        return { success: false, error: "Player name is required" };
      }

      // Get room summary
      const roomSummary = await this.getRoomSummary(roomId);
      if (!roomSummary.success || !roomSummary.data) {
        return { success: false, error: "Room not found" };
      }

      const { room, players } = roomSummary.data;

      if (!room.isActive) {
        return { success: false, error: "Room is not active" };
      }

      // Check if room is full
      if (players.length >= room.maxPlayers) {
        return { success: false, error: "Room is full" };
      }

      // Check if player name is already taken
      const existingPlayer = players.find(
        (p: any) => p.name.toLowerCase() === playerName.trim().toLowerCase(),
      );

      if (existingPlayer) {
        return { success: false, error: "Player name is already taken" };
      }

      // Add player to room
      const playerResult = await this.db.createPlayer(
        playerName.trim(),
        roomId,
        false,
      );

      if (!playerResult.success || !playerResult.data) {
        return { success: false, error: "Failed to join room" };
      }

      const playerId = playerResult.data;

      // Get updated room summary
      const updatedRoomSummary = await this.getRoomSummary(roomId);
      if (!updatedRoomSummary.success || !updatedRoomSummary.data) {
        return { success: false, error: "Failed to get updated room" };
      }

      return {
        success: true,
        data: {
          playerId,
          room: updatedRoomSummary.data,
        },
      };
    } catch (error) {
      console.error("Error joining room:", error);
      return { success: false, error: "Internal server error" };
    }
  }

  async leaveRoom(
    roomId: string,
    playerId: string,
  ): Promise<
    { success: boolean; data?: { wasHost: boolean; newHostId?: string }; error?: string }
  > {
    try {
      const roomSummary = await this.getRoomSummary(roomId);
      if (!roomSummary.success || !roomSummary.data) {
        return { success: false, error: "Room not found" };
      }

      const { players } = roomSummary.data;
      const player = players.find((p: any) => p.id === playerId);
      if (!player) {
        return { success: false, error: "Player not found in room" };
      }

      const wasHost = player.isHost;
      let newHostId: string | undefined;

      // If host is leaving and there are other players, transfer host
      if (wasHost && players.length > 1) {
        const newHost = players.find((p: any) => p.id !== playerId);
        if (newHost) {
          newHostId = newHost.id;
          // Note: We need to add updatePlayer method to DatabaseService
          // For now, we'll skip this functionality
        }
      }

      // Remove player from room
      await this.db.removePlayer(playerId);

      // If no players left, deactivate room
      if (players.length === 1) { // Only the leaving player
        await this.db.updateRoom(roomId, { isActive: false });
      }

      return {
        success: true,
        data: {
          wasHost,
          newHostId,
        },
      };
    } catch (error) {
      console.error("Error leaving room:", error);
      return { success: false, error: "Internal server error" };
    }
  }

  async getRoom(roomId: string): Promise<Room | null> {
    try {
      const result = await this.db.getRoomById(roomId);
      return result.success ? result.data || null : null;
    } catch (error) {
      console.error("Error getting room:", error);
      return null;
    }
  }

  async getRoomSummary(
    roomId: string,
  ): Promise<{ success: boolean; data?: RoomSummary; error?: string }> {
    try {
      const roomResult = await this.db.getRoomById(roomId);
      if (!roomResult.success || !roomResult.data) {
        return { success: false, error: "Room not found" };
      }

      const playersResult = await this.db.getPlayersByRoom(roomId);
      if (!playersResult.success) {
        return { success: false, error: "Failed to get players" };
      }

      const room = roomResult.data;
      const players = playersResult.data || [];
      const host = players.find((p) => p.isHost) || null;

      const roomSummary: RoomSummary = {
        room,
        players,
        playerCount: players.length,
        canJoin: players.length < room.maxPlayers && room.isActive,
        host,
      };

      return { success: true, data: roomSummary };
    } catch (error) {
      console.error("Error getting room summary:", error);
      return { success: false, error: "Internal server error" };
    }
  }

  async updatePlayerActivity(roomId: string, playerId: string): Promise<boolean> {
    try {
      // Note: updatePlayer method needs to be added to DatabaseService
      // For now, we'll just return true
      return true;
    } catch (error) {
      console.error("Error updating player activity:", error);
      return false;
    }
  }

  async setPlayerDisconnected(roomId: string, playerId: string): Promise<boolean> {
    try {
      // Note: updatePlayer method needs to be added to DatabaseService
      // For now, we'll just return true
      return true;
    } catch (error) {
      console.error("Error setting player disconnected:", error);
      return false;
    }
  }

  private generateRoomId(): string {
    // Generate a 6-character room code
    const chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let result = "";
    for (let i = 0; i < 6; i++) {
      result += chars.charAt(Math.floor(Math.random() * chars.length));
    }
    return result;
  }

  private generatePlayerId(): string {
    return `player_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
  }
}
