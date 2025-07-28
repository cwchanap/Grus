import { assertEquals, assertExists } from "$std/testing/asserts.ts";
import { createHandler } from "$fresh/server.ts";
import { handler as healthHandler } from "../api/health.ts";
import { handler as roomsHandler } from "../api/rooms.ts";

// Mock environment for testing
const mockEnv = {
  CLOUDFLARE_ACCOUNT_ID: "test-account",
  CLOUDFLARE_API_TOKEN: "test-token",
  DATABASE_ID: "test-db-id",
  KV_NAMESPACE_ID: "test-kv-id",
  ENVIRONMENT: "test",
};

// Set up mock environment
function setupMockEnv() {
  Object.entries(mockEnv).forEach(([key, value]) => {
    Deno.env.set(key, value);
  });
}

function teardownMockEnv() {
  Object.keys(mockEnv).forEach((key) => {
    Deno.env.delete(key);
  });
}

// Mock fetch for Cloudflare API calls
const originalFetch = globalThis.fetch;

function mockCloudflareAPI() {
  globalThis.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
    const urlString = input.toString();
    const options = init;

    // Mock D1 health check
    if (urlString.includes("/d1/database/") && urlString.includes("/query")) {
      const body = JSON.parse(options?.body as string || "{}");

      if (body.sql?.includes("SELECT 1 as test")) {
        return new Response(
          JSON.stringify({
            result: [{ results: [{ test: 1 }], success: true }],
          }),
          { status: 200 },
        );
      }

      if (body.sql?.includes("SELECT r.*, COUNT(p.id) as player_count")) {
        return new Response(
          JSON.stringify({
            result: [{
              results: [
                {
                  id: "room-1",
                  name: "Test Room",
                  host_id: "host-1",
                  max_players: 8,
                  is_active: true,
                  player_count: 2,
                  created_at: "2024-01-01T00:00:00Z",
                },
              ],
              success: true,
            }],
          }),
          { status: 200 },
        );
      }

      if (body.sql?.includes("INSERT INTO rooms")) {
        return new Response(
          JSON.stringify({
            result: [{ results: [], success: true }],
          }),
          { status: 200 },
        );
      }

      if (body.sql?.includes("INSERT INTO players")) {
        return new Response(
          JSON.stringify({
            result: [{ results: [], success: true }],
          }),
          { status: 200 },
        );
      }
    }

    // Mock KV health check
    if (urlString.includes("/storage/kv/namespaces/")) {
      if (options?.method === "PUT") {
        return new Response("", { status: 200 });
      }
      if (options?.method === "GET") {
        return new Response('{"timestamp":1234567890}', { status: 200 });
      }
      if (options?.method === "DELETE") {
        return new Response("", { status: 200 });
      }
    }

    return new Response("Not Found", { status: 404 });
  };
}

function restoreFetch() {
  globalThis.fetch = originalFetch;
}

Deno.test("API Integration - Health Check", async () => {
  setupMockEnv();
  mockCloudflareAPI();

  const request = new Request("http://localhost:8000/api/health");
  const response = await healthHandler.GET!(request, {} as any);

  assertEquals(response.status, 200);

  const data = await response.json();
  assertEquals(data.status, "ok");
  assertExists(data.checks);
  assertExists(data.checks.database);
  assertExists(data.checks.kv_storage);
  assertExists(data.checks.websocket);

  restoreFetch();
  teardownMockEnv();
});

Deno.test("API Integration - Get Rooms", async () => {
  setupMockEnv();
  mockCloudflareAPI();

  const request = new Request("http://localhost:8000/api/rooms");
  const response = await roomsHandler.GET!(request, {} as any);

  assertEquals(response.status, 200);

  const data = await response.json();
  assertExists(data.rooms);
  assertEquals(Array.isArray(data.rooms), true);

  if (data.rooms.length > 0) {
    const room = data.rooms[0];
    assertExists(room.id);
    assertExists(room.name);
    assertExists(room.max_players);
  }

  restoreFetch();
  teardownMockEnv();
});

Deno.test("API Integration - Create Room", async () => {
  setupMockEnv();
  mockCloudflareAPI();

  const requestBody = {
    name: "Test Room",
    hostName: "Test Host",
    maxPlayers: 6,
  };

  const request = new Request("http://localhost:8000/api/rooms", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(requestBody),
  });

  const response = await roomsHandler.POST!(request, {} as any);

  assertEquals(response.status, 201);

  const data = await response.json();
  assertExists(data.roomId);
  assertExists(data.playerId);
  assertExists(data.code);
  assertEquals(typeof data.roomId, "string");
  assertEquals(typeof data.playerId, "string");
  assertEquals(typeof data.code, "string");

  restoreFetch();
  teardownMockEnv();
});

Deno.test("API Integration - Create Room Validation Error", async () => {
  setupMockEnv();
  mockCloudflareAPI();

  const requestBody = {
    name: "", // Invalid: empty name
    hostName: "Test Host",
  };

  const request = new Request("http://localhost:8000/api/rooms", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(requestBody),
  });

  const response = await roomsHandler.POST!(request, {} as any);

  assertEquals(response.status, 400);

  const data = await response.json();
  assertExists(data.error);
  assertEquals(data.error, "Room name and host name are required");

  restoreFetch();
  teardownMockEnv();
});

Deno.test("API Integration - Health Check Database Error", async () => {
  setupMockEnv();

  // Mock API to return error
  globalThis.fetch = async (input: RequestInfo | URL) => {
    if (input.toString().includes("/d1/database/")) {
      return new Response("Database Error", { status: 500 });
    }
    return new Response("", { status: 200 });
  };

  const request = new Request("http://localhost:8000/api/health");
  const response = await healthHandler.GET!(request, {} as any);

  assertEquals(response.status, 503);

  const data = await response.json();
  assertEquals(data.status, "error");
  assertEquals(data.checks.database.status, "error");

  restoreFetch();
  teardownMockEnv();
});
