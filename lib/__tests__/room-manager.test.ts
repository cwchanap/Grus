import { assertEquals, assertExists, assert } from "https://deno.land/std@0.208.0/assert/mod.ts";
import { beforeEach, describe, it } from "https://deno.land/std@0.208.0/testing/bdd.ts";
import { RoomManager } from "../room-manager.ts";
import type { D1Database } from "../../types/cloudflare.ts";

// Mock D1Database for testing - simplified approach using integration test pattern
class MockD1Database {
  // We'll use the same mock approach as the integration tests since those are working
  prepare(query: string) {
    return {
      bind: (...params: any[]) => ({
        run: async () => ({ success: true }),
        first: async () => null,
        all: async () => ({ results: [] })
      }),
      first: async (...params: any[]) => null,
      run: async (...params: any[]) => ({ success: true }),
      all: async (...params: any[]) => ({ results: [] }),
      raw: async (...params: any[]) => ({ results: [] })
    };
  }
}

describe("RoomManager", () => {
  let mockDb: MockD1Database;
  let roomManager: RoomManager;

  beforeEach(() => {
    mockDb = new MockD1Database();
    roomManager = new RoomManager(mockDb as any);
  });

  describe("createRoom", () => {
    it("should create a room with valid parameters", async () => {
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

    it("should reject empty room name", async () => {
      const result = await roomManager.createRoom({
        name: "",
        hostName: "Host Player"
      });

      assert(!result.success);
      assertEquals(result.error, "Room name is required");
    });

    it("should reject empty host name", async () => {
      const result = await roomManager.createRoom({
        name: "Test Room",
        hostName: ""
      });

      assert(!result.success);
      assertEquals(result.error, "Host name is required");
    });

    it("should reject room name that's too long", async () => {
      const result = await roomManager.createRoom({
        name: "a".repeat(51),
        hostName: "Host Player"
      });

      assert(!result.success);
      assertEquals(result.error, "Room name must be 50 characters or less");
    });

    it("should reject host name that's too long", async () => {
      const result = await roomManager.createRoom({
        name: "Test Room",
        hostName: "a".repeat(31)
      });

      assert(!result.success);
      assertEquals(result.error, "Host name must be 30 characters or less");
    });

    it("should reject invalid max players", async () => {
      const result = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Host Player",
        maxPlayers: 1
      });

      assert(!result.success);
      assertEquals(result.error, "Max players must be between 2 and 16");
    });

    it("should use default max players when not specified", async () => {
      const result = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Host Player"
      });

      assert(result.success);
      
      // Verify room was created with default max players
      if (result.data) {
        const summaryResult = await roomManager.getRoomSummary(result.data.roomId);
        assert(summaryResult.success);
        assertEquals(summaryResult.data?.room.maxPlayers, 8);
      }
    });
  });

  describe("joinRoom", () => {
    it("should allow player to join existing room", async () => {
      // Create a room first
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

    it("should reject empty player name", async () => {
      const result = await roomManager.joinRoom({
        roomId: "test-room-id",
        playerName: ""
      });

      assert(!result.success);
      assertEquals(result.error, "Player name is required");
    });

    it("should reject player name that's too long", async () => {
      const result = await roomManager.joinRoom({
        roomId: "test-room-id",
        playerName: "a".repeat(31)
      });

      assert(!result.success);
      assertEquals(result.error, "Player name must be 30 characters or less");
    });

    it("should reject joining non-existent room", async () => {
      const result = await roomManager.joinRoom({
        roomId: "non-existent-room",
        playerName: "Player"
      });

      assert(!result.success);
      assertEquals(result.error, "Room not found");
    });

    it("should prevent duplicate player names in same room", async () => {
      // Create a room
      const createResult = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Host Player"
      });
      assert(createResult.success);
      assertExists(createResult.data);

      // Try to join with same name as host
      const joinResult = await roomManager.joinRoom({
        roomId: createResult.data.roomId,
        playerName: "Host Player"
      });

      assert(!joinResult.success);
      assertEquals(joinResult.error, "A player with this name already exists in the room");
    });
  });

  describe("leaveRoom", () => {
    it("should allow player to leave room", async () => {
      // Create room and join
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

      // Leave room
      const leaveResult = await roomManager.leaveRoom(joinResult.data.playerId);
      assert(leaveResult.success);
      assertExists(leaveResult.data);
      assertEquals(leaveResult.data.wasHost, false);
    });

    it("should transfer host when host leaves", async () => {
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
      assertEquals(leaveResult.data.newHostId, joinResult.data.playerId);
    });

    it("should deactivate room when last player leaves", async () => {
      // Create room
      const createResult = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Host Player"
      });
      assert(createResult.success);
      assertExists(createResult.data);

      // Host leaves (only player)
      const leaveResult = await roomManager.leaveRoom(createResult.data.playerId);
      assert(leaveResult.success);

      // Verify room is deactivated
      const summaryResult = await roomManager.getRoomSummary(createResult.data.roomId);
      assert(summaryResult.success);
      assertEquals(summaryResult.data?.room.isActive, false);
    });
  });

  describe("listRooms", () => {
    it("should return empty list when no rooms exist", async () => {
      const result = await roomManager.listRooms();
      assert(result.success);
      assertEquals(result.data?.length, 0);
    });

    it("should return list of active rooms", async () => {
      // Create multiple rooms
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

    it("should respect limit parameter", async () => {
      // Create multiple rooms
      await roomManager.createRoom({ name: "Room 1", hostName: "Host 1" });
      await roomManager.createRoom({ name: "Room 2", hostName: "Host 2" });
      await roomManager.createRoom({ name: "Room 3", hostName: "Host 3" });

      const result = await roomManager.listRooms({ limit: 2 });
      assert(result.success);
      assertEquals(result.data?.length, 2);
    });
  });

  describe("getRoomSummary", () => {
    it("should return detailed room information", async () => {
      // Create room
      const createResult = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Host Player"
      });
      assert(createResult.success);
      assertExists(createResult.data);

      // Get summary
      const summaryResult = await roomManager.getRoomSummary(createResult.data.roomId);
      assert(summaryResult.success);
      assertExists(summaryResult.data);
      
      const summary = summaryResult.data;
      assertEquals(summary.room.name, "Test Room");
      assertEquals(summary.playerCount, 1);
      assertEquals(summary.canJoin, true);
      assertEquals(summary.players.length, 1);
      assertExists(summary.host);
      assertEquals(summary.host.name, "Host Player");
    });

    it("should return null for non-existent room", async () => {
      const result = await roomManager.getRoomSummary("non-existent");
      assert(!result.success);
      assertEquals(result.error, "Room not found");
    });
  });

  describe("canJoinRoom", () => {
    it("should return true for joinable room", async () => {
      // Create room
      const createResult = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Host Player",
        maxPlayers: 4
      });
      assert(createResult.success);
      assertExists(createResult.data);

      const result = await roomManager.canJoinRoom(createResult.data.roomId);
      assert(result.success);
      assertEquals(result.data?.canJoin, true);
    });

    it("should return false for non-existent room", async () => {
      const result = await roomManager.canJoinRoom("non-existent");
      assert(result.success);
      assertEquals(result.data?.canJoin, false);
      assertEquals(result.data?.reason, "Room not found");
    });
  });

  describe("validateRoom", () => {
    it("should validate a properly configured room", async () => {
      // Create room
      const createResult = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Host Player"
      });
      assert(createResult.success);
      assertExists(createResult.data);

      const result = await roomManager.validateRoom(createResult.data.roomId);
      assert(result.success);
      assertEquals(result.data?.isValid, true);
    });

    it("should invalidate non-existent room", async () => {
      const result = await roomManager.validateRoom("non-existent");
      assert(result.success);
      assertEquals(result.data?.isValid, false);
      assertEquals(result.data?.reason, "Room not found");
    });
  });

  describe("transferHost", () => {
    it("should transfer host privileges successfully", async () => {
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

      // Verify transfer
      const summaryResult = await roomManager.getRoomSummary(createResult.data.roomId);
      assert(summaryResult.success);
      assertEquals(summaryResult.data?.host?.id, joinResult.data.playerId);
    });

    it("should reject transfer to player not in room", async () => {
      // Create room
      const createResult = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Host Player"
      });
      assert(createResult.success);
      assertExists(createResult.data);

      const result = await roomManager.transferHost(
        createResult.data.roomId,
        createResult.data.playerId,
        "non-existent-player"
      );
      assert(!result.success);
      assertEquals(result.error, "New host is not in the room");
    });
  });

  describe("cleanup", () => {
    it("should clean up invalid rooms", async () => {
      const result = await roomManager.cleanup();
      assert(result.success);
      assertExists(result.data);
      assertEquals(typeof result.data.roomsCleaned, "number");
      assertEquals(typeof result.data.playersCleaned, "number");
    });
  });
});