// Test file for database service
import { assertEquals, assertExists } from "$std/testing/asserts.ts";
import { getDatabaseService } from "../database-service.ts";

Deno.test("Database Service - Basic Operations", async (t) => {
  const db = getDatabaseService();

  // Test health check
  await t.step("health check", async () => {
    const result = db.healthCheck();
    assertEquals(result.success, true);
    assertEquals(result.data, true);
  });

  let roomId: string;
  let playerId: string;
  let sessionId: string;
  let scoreId: string;

  // Test room operations
  await t.step("create room", async () => {
    const result = db.createRoom("Test Room", "host-123", 8);
    assertEquals(result.success, true);
    assertExists(result.data);
    roomId = result.data!;
  });

  await t.step("get room by id", async () => {
    const result = db.getRoomById(roomId);
    assertEquals(result.success, true);
    assertExists(result.data);
    assertEquals(result.data?.name, "Test Room");
    assertEquals(result.data?.hostId, "host-123");
  });

  await t.step("get active rooms", async () => {
    const result = db.getActiveRooms(10);
    assertEquals(result.success, true);
    assertExists(result.data);
    assertEquals(Array.isArray(result.data), true);
  });

  // Test player operations
  await t.step("create player", async () => {
    const result = db.createPlayer("Test Player", roomId, false);
    assertEquals(result.success, true);
    assertExists(result.data);
    playerId = result.data!;
  });

  await t.step("get player by id", async () => {
    const result = db.getPlayerById(playerId);
    assertEquals(result.success, true);
    assertExists(result.data);
    assertEquals(result.data?.name, "Test Player");
    assertEquals(result.data?.roomId, roomId);
    assertEquals(result.data?.isHost, false);
  });

  await t.step("get players by room", async () => {
    const result = db.getPlayersByRoom(roomId);
    assertEquals(result.success, true);
    assertExists(result.data);
    assertEquals(Array.isArray(result.data), true);
    assertEquals(result.data?.length, 1);
  });

  // Test game session operations
  await t.step("create game session", async () => {
    const result = db.createGameSession(roomId, 5);
    assertEquals(result.success, true);
    assertExists(result.data);
    sessionId = result.data!;
  });

  await t.step("end game session", async () => {
    const result = db.endGameSession(sessionId, playerId);
    assertEquals(result.success, true);
    assertEquals(result.data, true);
  });

  // Test score operations
  await t.step("create score", async () => {
    const result = db.createScore(sessionId, playerId, 100);
    assertEquals(result.success, true);
    assertExists(result.data);
    scoreId = result.data!;
  });

  await t.step("update score", async () => {
    const result = db.updateScore(scoreId, 150, 3);
    assertEquals(result.success, true);
    assertEquals(result.data, true);
  });

  await t.step("get scores by session", async () => {
    const result = db.getScoresBySession(sessionId);
    assertEquals(result.success, true);
    assertExists(result.data);
    assertEquals(Array.isArray(result.data), true);
    assertEquals(result.data?.length, 1);
  });

  // Cleanup
  await t.step("cleanup", async () => {
    db.removePlayer(playerId);
    db.deleteRoom(roomId);
    db.close();
    // Clean up test database file
    try {
      Deno.removeSync("db/game.db");
    } catch {
      // Ignore if file doesn't exist
    }
  });
});
