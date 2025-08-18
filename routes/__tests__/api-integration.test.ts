import { assertEquals, assertExists } from "$std/testing/asserts.ts";
import { handler as healthHandler } from "../api/health.ts";
import { handler as roomsHandler } from "../api/rooms.ts";
import { RoomManager } from "../../lib/core/room-manager.ts";
import { getKVRoomService, getKVService } from "../../lib/db/index.ts";

// Minimal test environment setup
const mockEnv = {
  ENVIRONMENT: "test",
};

function setupMockEnv() {
  Object.entries(mockEnv).forEach(([key, value]) => Deno.env.set(key, value));
}

function teardownMockEnv() {
  Object.keys(mockEnv).forEach((key) => Deno.env.delete(key));
}

// Ensure Deno.Kv connections are closed to avoid resource leaks between tests
function teardownResources() {
  try {
    getKVService().close();
  } catch (_) {
    // ignore
  }
  try {
    getKVRoomService().close();
  } catch (_) {
    // ignore
  }
}

Deno.test("API Integration - Health Check", async () => {
  setupMockEnv();

  const request = new Request("http://localhost:3000/api/health");
  const response = await healthHandler.GET!(request, {} as any);

  assertEquals(response.status, 200);

  const data = await response.json();
  assertEquals(data.status, "ok");
  assertExists(data.checks);
  assertExists(data.checks.database);
  assertExists(data.checks.kv_storage);
  assertExists(data.checks.websocket);

  teardownMockEnv();
  teardownResources();
});

Deno.test("API Integration - Get Rooms", async () => {
  setupMockEnv();

  const request = new Request("http://localhost:3000/api/rooms");
  const response = await roomsHandler.GET!(request, {} as any);

  assertEquals(response.status, 200);

  const data = await response.json();
  assertExists(data.rooms);
  assertEquals(Array.isArray(data.rooms), true);

  if (data.rooms.length > 0) {
    const summary = data.rooms[0];
    assertExists(summary.room);
    assertExists(summary.players);
    assertExists(summary.playerCount);
    assertExists(summary.canJoin);

    // Validate nested room fields
    assertExists(summary.room.id);
    assertExists(summary.room.name);
    assertExists(summary.room.maxPlayers);
  }

  teardownMockEnv();
  teardownResources();
});

Deno.test("API Integration - Create Room", async () => {
  setupMockEnv();

  const requestBody = {
    name: "Test Room",
    hostName: "Test Host",
    maxPlayers: 6,
  };

  const request = new Request("http://localhost:3000/api/rooms", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(requestBody),
  });

  const response = await roomsHandler.POST!(request, {} as any);

  assertEquals(response.status, 201);

  const data = await response.json();
  assertExists(data.roomId);
  assertExists(data.playerId);
  assertEquals(typeof data.roomId, "string");
  assertEquals(typeof data.playerId, "string");
  assertExists(data.room);
  assertExists(data.room.room.id);
  assertExists(data.room.room.name);
  assertExists(data.room.room.maxPlayers);

  teardownMockEnv();
  teardownResources();
});

Deno.test("API Integration - Create Room Validation Error", async () => {
  setupMockEnv();

  const requestBody = {
    name: "", // Invalid: empty name
    hostName: "Test Host",
  };

  const request = new Request("http://localhost:3000/api/rooms", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(requestBody),
  });

  const response = await roomsHandler.POST!(request, {} as any);

  assertEquals(response.status, 400);

  const data = await response.json();
  assertExists(data.error);
  assertEquals(data.error, "Room name and host name are required");

  teardownMockEnv();
  teardownResources();
});

Deno.test("API Integration - Health Check Database Error", async () => {
  setupMockEnv();

  // Stub RoomManager to simulate a database error in the health check
  const original = RoomManager.prototype.getActiveRoomsWithCleanup;
  RoomManager.prototype.getActiveRoomsWithCleanup = () => ({ success: false, error: "Simulated DB error" } as any);

  const request = new Request("http://localhost:3000/api/health");
  const response = await healthHandler.GET!(request, {} as any);

  assertEquals(response.status, 503);

  const data = await response.json();
  assertEquals(data.status, "error");
  assertEquals(data.checks.database.status, "error");

  // Restore
  RoomManager.prototype.getActiveRoomsWithCleanup = original;
  teardownMockEnv();
  teardownResources();
});
