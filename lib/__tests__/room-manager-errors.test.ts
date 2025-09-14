import { assertEquals, assertExists } from "$std/testing/asserts.ts";
import { RoomManager } from "../core/room-manager.ts";
import { KVRoomService } from "../db/kv-room-service.ts";

class MockDBService extends KVRoomService {
    shouldFailPlayerCreation = false;
    shouldFailRoomUpdate = false;

    override createPlayer(name: string, roomId: string, isHost?: boolean | undefined): Promise<{ success: boolean; data?: string | undefined; error?: string | undefined; }> {
        if (this.shouldFailPlayerCreation) {
            return Promise.resolve({ success: false, error: "Failed to create host player" });
        }
        return super.createPlayer(name, roomId, isHost);
    }

    override updateRoom(id: string, updates: Partial<{ id: string; name: string; hostId: string | null; maxPlayers: number; isActive: boolean; gameType: string; isPrivate?: boolean | undefined; createdAt: string; updatedAt: string; }>): Promise<{ success: boolean; data?: boolean | undefined; error?: string | undefined; }> {
        if (this.shouldFailRoomUpdate) {
            return Promise.resolve({ success: false, error: "Failed to update room" });
        }
        return super.updateRoom(id, updates);
    }
}

async function withTestRoomManager(
  name: string,
  testFn: (roomManager: RoomManager, db: MockDBService) => Promise<void>,
) {
  Deno.test(name, async () => {
    const db = new MockDBService();
    const roomManager = new RoomManager(db);
    try {
      await testFn(roomManager, db);
    } finally {
      await db.deleteAllRooms();
      db.close();
    }
  });
}

withTestRoomManager("RoomManager - createRoom handles player creation failure", async (roomManager, db) => {
    db.shouldFailPlayerCreation = true;

    const result = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Test Host",
        gameType: "drawing",
    });

    assertEquals(result.success, false);
    assertEquals(result.error, "Failed to create host player");
});

withTestRoomManager("RoomManager - createRoom handles room update failure", async (roomManager, db) => {
    db.shouldFailRoomUpdate = true;

    const result = await roomManager.createRoom({
        name: "Test Room",
        hostName: "Test Host",
        gameType: "drawing",
    });

    assertEquals(result.success, false);
    assertEquals(result.error, "Failed to update room with host");
});

withTestRoomManager("RoomManager - joinRoom handles room not found", async (roomManager) => {
    const result = await roomManager.joinRoom({
        roomId: "non-existent-room",
        playerName: "Test Player",
    });

    assertEquals(result.success, false);
    assertEquals(result.error, "Room not found");
});

withTestRoomManager("RoomManager - joinRoom handles full room", async (roomManager) => {
    const createResult = await roomManager.createRoom({
        name: "Full Room",
        hostName: "Host",
        maxPlayers: 1,
        gameType: "drawing",
    });

    const roomId = createResult.data!.roomId;

    const result = await roomManager.joinRoom({
        roomId: roomId,
        playerName: "Late Player",
    });

    assertEquals(result.success, false);
    assertEquals(result.error, "Room is full");
});

withTestRoomManager("RoomManager - joinRoom handles player name taken", async (roomManager) => {
    const createResult = await roomManager.createRoom({
        name: "Name Clash Room",
        hostName: "Host",
        gameType: "drawing",
    });

    const roomId = createResult.data!.roomId;

    const result = await roomManager.joinRoom({
        roomId: roomId,
        playerName: "Host",
    });

    assertEquals(result.success, false);
    assertEquals(result.error, "Player name is already taken");
});
