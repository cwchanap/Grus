import { assertEquals, assertExists } from "$std/testing/asserts.ts";
import { TestRoomManager } from "./test-room-manager.ts";

let roomManager: TestRoomManager;

function setupTest() {
  roomManager = new TestRoomManager();
}

function teardownTest() {
  // Clean up any test data
  roomManager.close();
}

/**
 * Unit tests for RoomManager private room functionality
 */

Deno.test("RoomManager - createRoom with private flag", async () => {
  setupTest();

  const roomName = "Test Private Room";
  const hostName = "Test Host";

  // Create a private room
  const createResult = await roomManager.createRoom({
    name: roomName,
    hostName,
    gameType: "drawing",
    maxPlayers: 6,
    isPrivate: true,
  });

  assertEquals(createResult.success, true);
  assertExists(createResult.data);

  const { roomId, playerId, room } = createResult.data!;

  // Verify room properties
  assertExists(roomId);
  assertExists(playerId);
  assertEquals(room.room.name, roomName);
  assertEquals(room.room.isPrivate, true);
  assertEquals(room.room.gameType, "drawing");
  assertEquals(room.room.maxPlayers, 6);

  // Verify host player was created
  assertEquals(room.players.length, 1);
  assertEquals(room.players[0].name, hostName);
  assertEquals(room.players[0].isHost, true);

  await teardownTest();
});

Deno.test("RoomManager - createRoom with public flag (default)", async () => {
  setupTest();

  const roomName = "Test Public Room";
  const hostName = "Public Host";

  // Create a public room (default)
  const createResult = await roomManager.createRoom({
    name: roomName,
    hostName,
    gameType: "drawing",
    maxPlayers: 8,
    isPrivate: false,
  });

  assertEquals(createResult.success, true);
  assertExists(createResult.data);

  const { room } = createResult.data!;

  // Verify room is public
  assertEquals(room.room.isPrivate, false);

  await teardownTest();
});

Deno.test("RoomManager - createRoom defaults to public when isPrivate not specified", async () => {
  setupTest();

  const roomName = "Default Public Room";
  const hostName = "Default Host";

  // Create a room without specifying isPrivate
  const createResult = await roomManager.createRoom({
    name: roomName,
    hostName,
    gameType: "drawing",
    maxPlayers: 8,
  });

  assertEquals(createResult.success, true);
  assertExists(createResult.data);

  const { room } = createResult.data!;

  // Verify room defaults to public
  assertEquals(room.room.isPrivate, false);

  await teardownTest();
});

Deno.test("RoomManager - getActiveRoomsWithCleanup filters out private rooms", async () => {
  setupTest();

  // Create a public room
  const publicRoomResult = await roomManager.createRoom({
    name: "Public Room",
    hostName: "Public Host",
    gameType: "drawing",
    isPrivate: false,
  });
  assertEquals(publicRoomResult.success, true);

  // Create a private room
  const privateRoomResult = await roomManager.createRoom({
    name: "Private Room",
    hostName: "Private Host",
    gameType: "drawing",
    isPrivate: true,
  });
  assertEquals(privateRoomResult.success, true);

  // Get active rooms with cleanup
  const activeRoomsResult = await roomManager.getActiveRoomsWithCleanup(10);
  assertEquals(activeRoomsResult.success, true);
  assertExists(activeRoomsResult.data);

  const activeRooms = activeRoomsResult.data!;

  // Should only contain the public room
  assertEquals(activeRooms.length, 1);
  assertEquals(activeRooms[0].room.name, "Public Room");
  assertEquals(activeRooms[0].room.isPrivate, false);

  await teardownTest();
});

Deno.test("RoomManager - joinRoom works for private rooms via direct access", async () => {
  setupTest();

  // Create a private room
  const createResult = await roomManager.createRoom({
    name: "Private Join Test",
    hostName: "Host",
    gameType: "drawing",
    isPrivate: true,
  });
  assertEquals(createResult.success, true);

  const roomId = createResult.data!.roomId;

  // Join the private room
  const joinResult = await roomManager.joinRoom({
    roomId,
    playerName: "Joiner",
  });

  assertEquals(joinResult.success, true);
  assertExists(joinResult.data);

  const { playerId, room } = joinResult.data!;

  // Verify join was successful
  assertExists(playerId);
  assertEquals(room.room.name, "Private Join Test");
  assertEquals(room.players.length, 2); // Host + new player

  await teardownTest();
});

Deno.test("RoomManager - getRoomSummary works for private rooms", async () => {
  setupTest();

  // Create a private room
  const createResult = await roomManager.createRoom({
    name: "Private Summary Test",
    hostName: "Host",
    gameType: "drawing",
    isPrivate: true,
  });
  assertEquals(createResult.success, true);

  const roomId = createResult.data!.roomId;

  // Get room summary
  const summaryResult = await roomManager.getRoomSummary(roomId);
  assertEquals(summaryResult.success, true);
  assertExists(summaryResult.data);

  const summary = summaryResult.data!;

  // Verify summary contains correct data
  assertEquals(summary.room.name, "Private Summary Test");
  assertEquals(summary.room.isPrivate, true);
  assertEquals(summary.players.length, 1);
  assertEquals(summary.canJoin, true); // Room has space
  assertExists(summary.host);

  await teardownTest();
});

Deno.test("RoomManager - leaveRoom works for private rooms", async () => {
  setupTest();

  // Create a private room
  const createResult = await roomManager.createRoom({
    name: "Private Leave Test",
    hostName: "Host",
    gameType: "drawing",
    isPrivate: true,
  });
  assertEquals(createResult.success, true);

  const roomId = createResult.data!.roomId;
  const _hostPlayerId = createResult.data!.playerId;

  // Join with another player
  const joinResult = await roomManager.joinRoom({
    roomId,
    playerName: "Joiner",
  });
  assertEquals(joinResult.success, true);
  const joinerPlayerId = joinResult.data!.playerId;

  // Leave with the joiner
  const leaveResult = await roomManager.leaveRoom(roomId, joinerPlayerId);
  assertEquals(leaveResult.success, true);
  assertExists(leaveResult.data);

  const leaveData = leaveResult.data!;
  assertEquals(leaveData.wasHost, false);
  assertExists(leaveData.remainingPlayers);
  assertEquals(leaveData.remainingPlayers!.length, 1); // Only host left

  await teardownTest();
});

Deno.test("RoomManager - private room deletion when last player leaves", async () => {
  setupTest();

  // Create a private room
  const createResult = await roomManager.createRoom({
    name: "Private Delete Test",
    hostName: "Host",
    gameType: "drawing",
    isPrivate: true,
  });
  assertEquals(createResult.success, true);

  const roomId = createResult.data!.roomId;
  const hostPlayerId = createResult.data!.playerId;

  // Leave with the host (last player)
  const leaveResult = await roomManager.leaveRoom(roomId, hostPlayerId);
  assertEquals(leaveResult.success, true);
  assertExists(leaveResult.data);

  const leaveData = leaveResult.data!;
  assertEquals(leaveData.wasHost, true);
  assertEquals(leaveData.roomDeleted, true);

  // Verify room is gone
  const roomSummary = await roomManager.getRoomSummary(roomId);
  assertEquals(roomSummary.success, false); // Room should not exist

  await teardownTest();
});
