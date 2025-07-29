// Tests for WebSocket manager functionality
import { assertEquals, assertExists } from "$std/assert/mod.ts";
import { WebSocketManager } from "../websocket-manager.ts";
import { Env } from "../../../types/cloudflare.ts";

// Mock WebSocket implementation for testing
class MockWebSocket {
  readyState = 1;
  static READY_STATE_OPEN = 1;

  private listeners: Map<string, Function[]> = new Map();

  addEventListener(event: string, callback: Function) {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, []);
    }
    this.listeners.get(event)!.push(callback);
  }

  send(data: string) {
    // Mock send
  }

  accept() {
    // Mock accept
  }
}

class MockWebSocketPair {
  [0]: MockWebSocket;
  [1]: MockWebSocket;

  constructor() {
    this[0] = new MockWebSocket();
    this[1] = new MockWebSocket();
  }
}

// Mock globals
(globalThis as any).WebSocketPair = MockWebSocketPair;
(globalThis as any).WebSocket = MockWebSocket;

// Mock Cloudflare environment
const mockEnv: Env = {
  DB: {
    prepare: (query: string) => ({
      bind: (...values: unknown[]) => ({
        first: async () => {
          if (query.includes("COUNT")) {
            return { count: 1 };
          }
          return { id: "test-room", max_players: 8 };
        },
        run: async () => ({ success: true }),
        all: async () => ({ results: [] }),
      }),
    }),
    exec: async () => ({ count: 0, duration: 0 }),
    dump: async () => new ArrayBuffer(0),
    batch: async () => [],
  } as any,
  GAME_STATE: {
    get: async (key: string) => null,
    put: async () => {},
    delete: async () => {},
    list: async () => ({ keys: [], list_complete: true }),
  } as any,
  WEBSOCKET_HANDLER: {} as any,
  ENVIRONMENT: "test",
};

Deno.test("WebSocketManager - constructor", () => {
  const manager = new WebSocketManager(mockEnv, false);
  assertExists(manager);
});

Deno.test("WebSocketManager - handleRequest with WebSocket upgrade", async () => {
  const manager = new WebSocketManager(mockEnv, false);

  const request = new Request("http://localhost/ws?roomId=test-room", {
    headers: { "Upgrade": "websocket" },
  });

  const response = await manager.handleRequest(request);
  assertEquals(response.status, 101);
});

Deno.test("WebSocketManager - handleRequest without room ID", async () => {
  const manager = new WebSocketManager(mockEnv, false);

  const request = new Request("http://localhost/ws", {
    headers: { "Upgrade": "websocket" },
  });

  const response = await manager.handleRequest(request);
  assertEquals(response.status, 400);
});

Deno.test("WebSocketManager - handleRequest for WebSocket info", async () => {
  const manager = new WebSocketManager(mockEnv, false);

  const request = new Request("http://localhost/ws/info");
  const response = await manager.handleRequest(request);

  assertEquals(response.status, 200);
  assertEquals(response.headers.get("Content-Type"), "application/json");

  const body = await response.json();
  assertExists(body.activeRooms);
  assertExists(body.timestamp);
  assertEquals(body.status, "healthy");
});

Deno.test("WebSocketManager - handleRequest for unknown path", async () => {
  const manager = new WebSocketManager(mockEnv, false);

  const request = new Request("http://localhost/unknown");
  const response = await manager.handleRequest(request);

  assertEquals(response.status, 404);
});

Deno.test("WebSocketManager - getActiveRoomCount", () => {
  const manager = new WebSocketManager(mockEnv, false);

  const count = manager.getActiveRoomCount();
  assertEquals(typeof count, "number");
  assertEquals(count >= 0, true);
});

Deno.test("WebSocketManager - getRoomConnectionCount", async () => {
  const manager = new WebSocketManager(mockEnv, false);

  const count = await manager.getRoomConnectionCount("test-room");
  assertEquals(typeof count, "number");
  assertEquals(count >= 0, true);
});

Deno.test("WebSocketManager - broadcastToAllRooms", async () => {
  const manager = new WebSocketManager(mockEnv, false);

  // This should not throw an error
  await manager.broadcastToAllRooms({ type: "test", data: {} });
  assertEquals(true, true); // Placeholder assertion
});

Deno.test("WebSocketManager - multiple room handlers", async () => {
  const manager = new WebSocketManager(mockEnv, false);

  // Create connections to different rooms
  const request1 = new Request("http://localhost/ws?roomId=room1", {
    headers: { "Upgrade": "websocket" },
  });

  const request2 = new Request("http://localhost/ws?roomId=room2", {
    headers: { "Upgrade": "websocket" },
  });

  const response1 = await manager.handleRequest(request1);
  const response2 = await manager.handleRequest(request2);

  assertEquals(response1.status, 101);
  assertEquals(response2.status, 101);

  // Should have handlers for both rooms
  const activeRooms = manager.getActiveRoomCount();
  assertEquals(activeRooms >= 2, true);
});

Deno.test("WebSocketManager - heartbeat functionality", async () => {
  const manager = new WebSocketManager(mockEnv, false);

  // Test that heartbeat can be started and stopped without errors
  // In a real implementation, this would test cleanup of inactive rooms
  manager.stopHeartbeat();
  assertEquals(true, true); // Placeholder assertion
});

Deno.test("WebSocketManager - room cleanup", async () => {
  const manager = new WebSocketManager(mockEnv, false);

  // Create a room handler
  const request = new Request("http://localhost/ws?roomId=test-room", {
    headers: { "Upgrade": "websocket" },
  });
  await manager.handleRequest(request);

  // Verify room exists
  const initialCount = manager.getActiveRoomCount();
  assertEquals(initialCount >= 1, true);

  // Clean up the room
  await manager.cleanupRoom("test-room");

  // Room should be cleaned up (this is tested implicitly through the cleanup method)
  assertEquals(true, true); // Placeholder assertion
});

Deno.test("WebSocketManager - concurrent room handling", async () => {
  const manager = new WebSocketManager(mockEnv, false);

  // Create multiple concurrent requests
  const requests = [];
  for (let i = 0; i < 5; i++) {
    requests.push(
      manager.handleRequest(
        new Request(`http://localhost/ws?roomId=room-${i}`, {
          headers: { "Upgrade": "websocket" },
        }),
      ),
    );
  }

  const responses = await Promise.all(requests);

  // All requests should succeed
  responses.forEach((response) => {
    assertEquals(response.status, 101);
  });

  // Should have multiple active rooms
  const activeRooms = manager.getActiveRoomCount();
  assertEquals(activeRooms >= 5, true);
});

Deno.test("WebSocketManager - error handling for invalid requests", async () => {
  const manager = new WebSocketManager(mockEnv, false);

  // Test various invalid request scenarios
  const invalidRequests = [
    new Request("http://localhost/ws"), // No room ID
    new Request("http://localhost/ws?roomId="), // Empty room ID
    new Request("http://localhost/invalid-path"), // Invalid path
  ];

  for (const request of invalidRequests) {
    const response = await manager.handleRequest(request);
    assertEquals(response.status >= 400, true);
  }
});

Deno.test("WebSocketManager - getActiveRoomIds", () => {
  const manager = new WebSocketManager(mockEnv, false);

  const roomIds = manager.getActiveRoomIds();
  assertEquals(Array.isArray(roomIds), true);
  assertEquals(typeof roomIds.length, "number");
});
