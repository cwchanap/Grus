import { assertEquals, assertExists, assert } from "https://deno.land/std@0.208.0/assert/mod.ts";
import { beforeEach, describe, it } from "https://deno.land/std@0.208.0/testing/bdd.ts";
import { RoomManager } from "../room-manager.ts";
import { RoomDAO } from "../dao/room-dao.ts";
import { PlayerDAO } from "../dao/player-dao.ts";
import type { DatabaseResult } from "../database.ts";

// Simple mock implementations for integration testing
class MockRoomDAO {
  private rooms = new Map<string, any>();
  private nextId = 1;

  async create(room: any): Promise<DatabaseResult<string>> {
    const id = `room-${this.nextId++}`;
    this.rooms.set(id, {
      id,
      name: room.name,
      hostId: room.hostId,
      maxPlayers: room.maxPlayers ?? 8,
      isActive: room.isActive ?? true,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString()
    });
    return { success: true, data: id };
  }

  async findById(id: string): Promise<DatabaseResult<any | null>> {
    const room = this.rooms.get(id);
    return { success: true, data: room || null };
  }

  async findActive(): Promise<DatabaseResult<any[]>> {
    const activeRooms = Array.from(this.rooms.values()).filter(r => r.isActive);
    return { success: true, data: activeRooms };
  }

  async findWithPlayerCount(id: string): Promise<DatabaseResult<any | null>> {
    const room = this.rooms.get(id);
    if (!room) return { success: true, data: null };
    
    return { 
      success: true, 
      data: { ...room, playerCount: 0 } // Simplified for testing
    };
  }

  async getRoomSummary(id: string): Promise<DatabaseResult<any | null>> {
    const room = this.rooms.get(id);
    if (!room) return { success: false, error: 'Room not found' };
    
    return {
      success: true,
      data: {
        room,
        playerCount: 0,
        canJoin: room.isActive && 0 < room.maxPlayers
      }
    };
  }

  async setInactive(id: string): Promise<DatabaseResult<void>> {
    const room = this.rooms.get(id);
    if (room) {
      room.isActive = false;
      room.updatedAt = new Date().toISOString();
    }
    return { success: true };
  }

  async delete(id: string): Promise<DatabaseResult<void>> {
    this.rooms.delete(id);
    return { success: true };
  }

  reset() {
    this.rooms.clear();
    this.nextId = 1;
  }
}

class MockPlayerDAO {
  private players = new Map<string, any>();
  private nextId = 1;

  async create(player: any): Promise<DatabaseResult<string>> {
    const id = `player-${this.nextId++}`;
    this.players.set(id, {
      id,
      name: player.name,
      roomId: player.roomId || null,
      isHost: player.isHost ?? false,
      joinedAt: new Date().toISOString()
    });
    return { success: true, data: id };
  }

  async findById(id: string): Promise<DatabaseResult<any | null>> {
    const player = this.players.get(id);
    return { success: true, data: player || null };
  }

  async findByRoom(roomId: string): Promise<DatabaseResult<any[]>> {
    const roomPlayers = Array.from(this.players.values()).filter(p => p.roomId === roomId);
    return { success: true, data: roomPlayers };
  }

  async addToRoom(playerId: string, roomId: string): Promise<DatabaseResult<void>> {
    const player = this.players.get(playerId);
    if (player) {
      player.roomId = roomId;
    }
    return { success: true };
  }

  async removeFromRoom(playerId: string): Promise<DatabaseResult<void>> {
    const player = this.players.get(playerId);
    if (player) {
      player.roomId = null;
    }
    return { success: true };
  }

  async update(id: string, updates: any): Promise<DatabaseResult<void>> {
    const player = this.players.get(id);
    if (player) {
      Object.assign(player, updates);
    }
    return { success: true };
  }

  async delete(id: string): Promise<DatabaseResult<void>> {
    this.players.delete(id);
    return { success: true };
  }

  async validatePlayerInRoom(playerId: string, roomId: string): Promise<DatabaseResult<boolean>> {
    const player = this.players.get(playerId);
    if (!player) return { success: false, error: 'Player not found' };
    return { success: true, data: player.roomId === roomId };
  }

  async transferHost(currentHostId: string, newHostId: string, roomId: string): Promise<DatabaseResult<void>> {
    const currentHost = this.players.get(currentHostId);
    const newHost = this.players.get(newHostId);
    
    if (currentHost) currentHost.isHost = false;
    if (newHost) newHost.isHost = true;
    
    return { success: true };
  }

  reset() {
    this.players.clear();
    this.nextId = 1;
  }
}

describe("RoomManager Integration Tests", () => {
  let roomManager: RoomManager;
  let mockRoomDAO: MockRoomDAO;
  let mockPlayerDAO: MockPlayerDAO;

  beforeEach(() => {
    mockRoomDAO = new MockRoomDAO();
    mockPlayerDAO = new MockPlayerDAO();
    
    // Create a room manager with mocked DAOs
    roomManager = new RoomManager({} as any);
    (roomManager as any).roomDAO = mockRoomDAO;
    (roomManager as any).playerDAO = mockPlayerDAO;
  });

  describe("Room Creation", () => {
    it("should create a room successfully", async () => {
      const result = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Host Player",
        maxPlayers: 6
      });

      assert(result.success);
      assertExists(result.data);
      assertExists(result.data.roomId);
      assertExists(result.data.playerId);
    });

    it("should validate room name requirements", async () => {
      const emptyNameResult = await roomManager.createRoom({
        name: "",
        hostName: "Host Player"
      });
      assert(!emptyNameResult.success);
      assertEquals(emptyNameResult.error, "Room name is required");

      const longNameResult = await roomManager.createRoom({
        name: "a".repeat(51),
        hostName: "Host Player"
      });
      assert(!longNameResult.success);
      assertEquals(longNameResult.error, "Room name must be 50 characters or less");
    });

    it("should validate host name requirements", async () => {
      const emptyHostResult = await roomManager.createRoom({
        name: "Test Room",
        hostName: ""
      });
      assert(!emptyHostResult.success);
      assertEquals(emptyHostResult.error, "Host name is required");

      const longHostResult = await roomManager.createRoom({
        name: "Test Room",
        hostName: "a".repeat(31)
      });
      assert(!longHostResult.success);
      assertEquals(longHostResult.error, "Host name must be 30 characters or less");
    });

    it("should validate max players range", async () => {
      const tooFewResult = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Host Player",
        maxPlayers: 1
      });
      assert(!tooFewResult.success);
      assertEquals(tooFewResult.error, "Max players must be between 2 and 16");

      const tooManyResult = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Host Player",
        maxPlayers: 17
      });
      assert(!tooManyResult.success);
      assertEquals(tooManyResult.error, "Max players must be between 2 and 16");
    });
  });

  describe("Room Joining", () => {
    it("should allow joining an existing room", async () => {
      // Create room first
      const createResult = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Host Player"
      });
      assert(createResult.success);
      assertExists(createResult.data);

      // Join the room
      const joinResult = await roomManager.joinRoom({
        roomId: createResult.data.roomId,
        playerName: "Player 2"
      });

      assert(joinResult.success);
      assertExists(joinResult.data);
      assertExists(joinResult.data.playerId);
    });

    it("should validate player name requirements", async () => {
      const emptyNameResult = await roomManager.joinRoom({
        roomId: "test-room",
        playerName: ""
      });
      assert(!emptyNameResult.success);
      assertEquals(emptyNameResult.error, "Player name is required");

      const longNameResult = await roomManager.joinRoom({
        roomId: "test-room",
        playerName: "a".repeat(31)
      });
      assert(!longNameResult.success);
      assertEquals(longNameResult.error, "Player name must be 30 characters or less");
    });
  });

  describe("Room Leaving", () => {
    it("should handle player leaving room", async () => {
      // Create room and add player
      const createResult = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Host Player"
      });
      assert(createResult.success);
      assertExists(createResult.data);

      const joinResult = await roomManager.joinRoom({
        roomId: createResult.data.roomId,
        playerName: "Player 2"
      });
      assert(joinResult.success);
      assertExists(joinResult.data);

      // Player leaves
      const leaveResult = await roomManager.leaveRoom(joinResult.data.playerId);
      assert(leaveResult.success);
      assertExists(leaveResult.data);
      assertEquals(leaveResult.data.wasHost, false);
    });

    it("should handle host leaving and transfer privileges", async () => {
      // Create room
      const createResult = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Host Player"
      });
      assert(createResult.success);
      assertExists(createResult.data);

      // Add another player
      const joinResult = await roomManager.joinRoom({
        roomId: createResult.data.roomId,
        playerName: "Player 2"
      });
      assert(joinResult.success);
      assertExists(joinResult.data);

      // Host leaves
      const leaveResult = await roomManager.leaveRoom(createResult.data.playerId);
      assert(leaveResult.success);
      assertExists(leaveResult.data);
      assertEquals(leaveResult.data.wasHost, true);
      assertExists(leaveResult.data.newHostId);
    });
  });

  describe("Room Listing", () => {
    it("should return empty list when no rooms exist", async () => {
      const result = await roomManager.listRooms();
      assert(result.success);
      assertEquals(result.data?.length, 0);
    });

    it("should list active rooms", async () => {
      // Create rooms
      await roomManager.createRoom({
        name: "Room 1",
        hostName: "Host 1"
      });
      await roomManager.createRoom({
        name: "Room 2",
        hostName: "Host 2"
      });

      const result = await roomManager.listRooms();
      assert(result.success);
      assertEquals(result.data?.length, 2);
    });
  });

  describe("Room Validation", () => {
    it("should validate room capacity constraints", async () => {
      const result = await roomManager.canJoinRoom("non-existent");
      assert(result.success);
      assertEquals(result.data?.canJoin, false);
      assertEquals(result.data?.reason, "Room not found");
    });

    it("should validate room state", async () => {
      const result = await roomManager.validateRoom("non-existent");
      assert(result.success);
      assertEquals(result.data?.isValid, false);
      assertEquals(result.data?.reason, "Room not found");
    });
  });

  describe("Host Transfer", () => {
    it("should handle host privilege transfer", async () => {
      // Create room
      const createResult = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Host Player"
      });
      assert(createResult.success);
      assertExists(createResult.data);

      // Add another player
      const joinResult = await roomManager.joinRoom({
        roomId: createResult.data.roomId,
        playerName: "Player 2"
      });
      assert(joinResult.success);
      assertExists(joinResult.data);

      // Transfer host
      const transferResult = await roomManager.transferHost(
        createResult.data.roomId,
        createResult.data.playerId,
        joinResult.data.playerId
      );
      assert(transferResult.success);
    });
  });

  describe("Cleanup Operations", () => {
    it("should perform cleanup operations", async () => {
      const result = await roomManager.cleanup();
      assert(result.success);
      assertExists(result.data);
      assertEquals(typeof result.data.roomsCleaned, "number");
      assertEquals(typeof result.data.playersCleaned, "number");
    });
  });
});