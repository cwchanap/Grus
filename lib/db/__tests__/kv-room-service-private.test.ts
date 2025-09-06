import { assertEquals, assertExists } from "$std/testing/asserts.ts";
import { KVRoomService } from "../kv-room-service.ts";

let kvRoomService: KVRoomService;

function setupTest() {
  kvRoomService = new KVRoomService();
}

function teardownTest() {
  // Clean up any test data and close the KV connection
  kvRoomService.close();
}

/**
 * Unit tests for KVRoomService private room functionality
 */

Deno.test("KVRoomService - createRoom with private flag", async () => {
  setupTest();

  const roomName = "Test Private Room";
  const hostId = "host-123";
  const maxPlayers = 6;
  const gameType = "drawing";
  const isPrivate = true;

  // Create a private room
  const createResult = await kvRoomService.createRoom(
    roomName,
    hostId,
    maxPlayers,
    gameType,
    isPrivate
  );

  assertEquals(createResult.success, true);
  assertExists(createResult.data);

  const roomId = createResult.data!;

  // Verify the room was created with private flag
  const getResult = await kvRoomService.getRoomById(roomId);
  assertEquals(getResult.success, true);
  assertExists(getResult.data);
  assertEquals(getResult.data!.name, roomName);
  assertEquals(getResult.data!.hostId, hostId);
  assertEquals(getResult.data!.maxPlayers, maxPlayers);
  assertEquals(getResult.data!.gameType, gameType);
  assertEquals(getResult.data!.isPrivate, isPrivate);
  assertEquals(getResult.data!.isActive, true);

  await teardownTest();
});

Deno.test("KVRoomService - createRoom with public flag (default)", async () => {
  setupTest();

  const roomName = "Test Public Room";
  const hostId = "host-456";

  // Create a public room (default)
  const createResult = await kvRoomService.createRoom(roomName, hostId);

  assertEquals(createResult.success, true);
  assertExists(createResult.data);

  const roomId = createResult.data!;

  // Verify the room was created as public
  const getResult = await kvRoomService.getRoomById(roomId);
  assertEquals(getResult.success, true);
  assertExists(getResult.data);
  assertEquals(getResult.data!.isPrivate, false);

  await teardownTest();
});

Deno.test("KVRoomService - getActiveRooms filters out private rooms", async () => {
  setupTest();

  // Create a public room
  const publicRoomResult = await kvRoomService.createRoom(
    "Public Room",
    "host-public",
    8,
    "drawing",
    false
  );
  assertEquals(publicRoomResult.success, true);

  // Create a private room
  const privateRoomResult = await kvRoomService.createRoom(
    "Private Room",
    "host-private",
    8,
    "drawing",
    true
  );
  assertEquals(privateRoomResult.success, true);

  // Get active rooms - should only return public room
  const activeRoomsResult = await kvRoomService.getActiveRooms(10);
  assertEquals(activeRoomsResult.success, true);
  assertExists(activeRoomsResult.data);

  const activeRooms = activeRoomsResult.data!;
  assertEquals(activeRooms.length, 1);
  assertEquals(activeRooms[0].room.name, "Public Room");
  assertEquals(activeRooms[0].room.isPrivate, false);

  await teardownTest();
});

Deno.test("KVRoomService - getAllRooms includes both public and private", async () => {
  setupTest();

  // Create a public room
  const publicRoomResult = await kvRoomService.createRoom(
    "Public Room 2",
    "host-public-2",
    8,
    "drawing",
    false
  );
  assertEquals(publicRoomResult.success, true);

  // Create a private room
  const privateRoomResult = await kvRoomService.createRoom(
    "Private Room 2",
    "host-private-2",
    8,
    "drawing",
    true
  );
  assertEquals(privateRoomResult.success, true);

  // Get all rooms - should include both
  const allRoomsResult = await kvRoomService.getAllRooms(10);
  assertEquals(allRoomsResult.success, true);
  assertExists(allRoomsResult.data);

  const allRooms = allRoomsResult.data!;
  assertEquals(allRooms.length, 2);

  const publicRoom = allRooms.find(r => r.name === "Public Room 2");
  const privateRoom = allRooms.find(r => r.name === "Private Room 2");

  assertExists(publicRoom);
  assertExists(privateRoom);
  assertEquals(publicRoom!.isPrivate, false);
  assertEquals(privateRoom!.isPrivate, true);

  await teardownTest();
});

Deno.test("KVRoomService - updateRoom preserves isPrivate flag", async () => {
  setupTest();

  // Create a private room
  const createResult = await kvRoomService.createRoom(
    "Update Test Room",
    "host-update",
    8,
    "drawing",
    true
  );
  assertEquals(createResult.success, true);

  const roomId = createResult.data!;

  // Update the room (change max players)
  const updateResult = await kvRoomService.updateRoom(roomId, { maxPlayers: 12 });
  assertEquals(updateResult.success, true);

  // Verify the room still has isPrivate = true
  const getResult = await kvRoomService.getRoomById(roomId);
  assertEquals(getResult.success, true);
  assertExists(getResult.data);
  assertEquals(getResult.data!.maxPlayers, 12);
  assertEquals(getResult.data!.isPrivate, true);

  await teardownTest();
});

Deno.test("KVRoomService - mixed private/public rooms in getAllRooms", async () => {
  setupTest();

  // Create multiple rooms with different privacy settings
  const rooms = [
    { name: "Public 1", isPrivate: false },
    { name: "Private 1", isPrivate: true },
    { name: "Public 2", isPrivate: false },
    { name: "Private 2", isPrivate: true },
    { name: "Public 3", isPrivate: false },
  ];

  for (const room of rooms) {
    const result = await kvRoomService.createRoom(
      room.name,
      `host-${room.name.replace(" ", "-")}`,
      8,
      "drawing",
      room.isPrivate
    );
    assertEquals(result.success, true);
  }

  // Get all rooms
  const allRoomsResult = await kvRoomService.getAllRooms();
  assertEquals(allRoomsResult.success, true);
  assertExists(allRoomsResult.data);

  const allRooms = allRoomsResult.data!;
  assertEquals(allRooms.length, 5);

  // Check that we have the right mix of private/public
  const publicRooms = allRooms.filter(r => !r.isPrivate);
  const privateRooms = allRooms.filter(r => r.isPrivate);

  assertEquals(publicRooms.length, 3);
  assertEquals(privateRooms.length, 2);

  // Verify active rooms only shows public ones
  const activeRoomsResult = await kvRoomService.getActiveRooms();
  assertEquals(activeRoomsResult.success, true);
  assertExists(activeRoomsResult.data);

  const activeRooms = activeRoomsResult.data!;
  assertEquals(activeRooms.length, 3); // Only public rooms

  // All active rooms should be public
  for (const room of activeRooms) {
    assertEquals(room.room.isPrivate, false);
  }

  await teardownTest();
});
