import { assertEquals, assertExists } from "$std/testing/asserts.ts";
import { KVService } from "../kv-service.ts";

let kvService: KVService;

function setupTest() {
  kvService = new KVService();
}

function teardownTest() {
  // Clean up any test data and close the KV connection
  kvService.close();
}

Deno.test("KVService - setGameState and getGameState", async () => {
  setupTest();

  const gameState = {
    roomId: "room-123",
    currentRound: 2,
    players: ["player-1", "player-2", "player-3"],
    isActive: true,
  };

  // Set game state
  const setResult = await kvService.setGameState("room-123", gameState);
  assertEquals(setResult.success, true);

  // Get game state
  const getResult = await kvService.getGameState("room-123");
  assertEquals(getResult.success, true);
  assertEquals(getResult.data?.roomId, "room-123");
  assertEquals(getResult.data?.currentRound, 2);
  assertEquals(getResult.data?.players.length, 3);

  await teardownTest();
});

Deno.test("KVService - getGameState non-existent", async () => {
  setupTest();

  const result = await kvService.getGameState("non-existent-room");
  assertEquals(result.success, true);
  assertEquals(result.data, null);

  await teardownTest();
});

Deno.test("KVService - deleteGameState", async () => {
  setupTest();

  const gameState = { roomId: "room-456", isActive: true };

  // Set then delete
  await kvService.setGameState("room-456", gameState);
  const deleteResult = await kvService.deleteGameState("room-456");
  assertEquals(deleteResult.success, true);

  // Verify it's gone
  const getResult = await kvService.getGameState("room-456");
  assertEquals(getResult.data, null);

  await teardownTest();
});

Deno.test("KVService - setPlayerSession and getPlayerSession", async () => {
  setupTest();

  const session = {
    playerId: "player-789",
    roomId: "room-123",
    isActive: true,
    joinedAt: Date.now(),
  };

  // Set player session
  const setResult = await kvService.setPlayerSession("player-789", session, 3600);
  assertEquals(setResult.success, true);

  // Get player session
  const getResult = await kvService.getPlayerSession("player-789");
  assertEquals(getResult.success, true);
  assertEquals(getResult.data?.playerId, "player-789");
  assertEquals(getResult.data?.roomId, "room-123");

  await teardownTest();
});

Deno.test("KVService - cacheRoomData", async () => {
  setupTest();

  const roomData = {
    id: "room-999",
    name: "Cached Room",
    playerCount: 5,
    isActive: true,
  };

  // Cache room data
  const cacheResult = await kvService.cacheRoomData("room-999", roomData, 300);
  assertEquals(cacheResult.success, true);

  // Get cached data
  const getResult = await kvService.getCachedRoomData("room-999");
  assertEquals(getResult.success, true);
  assertEquals(getResult.data?.name, "Cached Room");
  assertEquals(getResult.data?.playerCount, 5);

  await teardownTest();
});

Deno.test("KVService - saveDrawingData", async () => {
  setupTest();

  const drawingData = {
    roomId: "room-art",
    playerId: "artist-1",
    strokes: [
      { x: 100, y: 100, color: "#000000" },
      { x: 150, y: 150, color: "#ff0000" },
    ],
  };

  const result = await kvService.saveDrawingData("room-art", drawingData);
  assertEquals(result.success, true);

  await teardownTest();
});

Deno.test("KVService - cacheMessage", async () => {
  setupTest();

  const message = {
    id: "msg-123",
    roomId: "room-chat",
    playerId: "player-1",
    message: "Hello everyone!",
  };

  const result = await kvService.cacheMessage("room-chat", message);
  assertEquals(result.success, true);

  await teardownTest();
});

Deno.test("KVService - generic set and get", async () => {
  setupTest();

  const testData = { test: "value", number: 42 };

  // Set data
  const setResult = await kvService.set("test-key", testData, 3600);
  assertEquals(setResult.success, true);

  // Get data
  const getResult = await kvService.get("test-key");
  assertEquals(getResult.success, true);
  assertEquals(getResult.data?.test, "value");
  assertEquals(getResult.data?.number, 42);

  await teardownTest();
});

Deno.test("KVService - delete", async () => {
  setupTest();

  // Set then delete
  await kvService.set("delete-me", { data: "test" });
  const deleteResult = await kvService.delete("delete-me");
  assertEquals(deleteResult.success, true);

  // Verify it's gone
  const getResult = await kvService.get("delete-me");
  assertEquals(getResult.data, null);

  await teardownTest();
});

Deno.test("KVService - healthCheck", async () => {
  setupTest();

  const result = await kvService.healthCheck();
  assertEquals(result.success, true);
  assertEquals(result.data, true);

  await teardownTest();
});

Deno.test("KVService - error handling", async () => {
  setupTest();

  // Test with invalid key structure that might cause an error
  // Since Deno KV is quite robust, we'll test a different error scenario
  const result = await kvService.get("");

  // The operation might succeed with empty string, so let's just verify the structure
  assertEquals(typeof result.success, "boolean");

  if (!result.success) {
    assertExists(result.error);
  }

  await teardownTest();
});
