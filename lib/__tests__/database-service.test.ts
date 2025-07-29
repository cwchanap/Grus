import { assertEquals, assertExists } from "$std/testing/asserts.ts";
import { DatabaseService } from "../database-service.ts";

// Mock the CloudflareAPI
class MockCloudflareAPI {
  private mockResponses: Map<string, any> = new Map();

  setMockResponse(sql: string, response: any) {
    this.mockResponses.set(sql.trim(), response);
  }

  async executeD1Query(sql: string, params: any[] = []) {
    const normalizedSql = sql.trim().replace(/\s+/g, " ");

    // Handle different SQL patterns
    if (normalizedSql.includes("INSERT INTO rooms")) {
      return [{ results: [], success: true, meta: { duration: 10 } }];
    }

    if (normalizedSql.includes("SELECT * FROM rooms WHERE id")) {
      const roomId = params[0];
      if (roomId === "existing-room") {
        return [{
          results: [{
            id: "existing-room",
            name: "Test Room",
            host_id: "host-1",
            max_players: 8,
            is_active: true,
            created_at: "2024-01-01T00:00:00Z",
          }],
          success: true,
        }];
      }
      return [{ results: [], success: true }];
    }

    if (normalizedSql.includes("SELECT r.*, COUNT(p.id) as player_count")) {
      return [{
        results: [
          {
            id: "room-1",
            name: "Active Room 1",
            host_id: "host-1",
            max_players: 8,
            is_active: true,
            player_count: 2,
            created_at: "2024-01-01T00:00:00Z",
          },
          {
            id: "room-2",
            name: "Active Room 2",
            host_id: "host-2",
            max_players: 6,
            is_active: true,
            player_count: 1,
            created_at: "2024-01-01T01:00:00Z",
          },
        ],
        success: true,
      }];
    }

    if (normalizedSql.includes("INSERT INTO players")) {
      return [{ results: [], success: true, meta: { duration: 5 } }];
    }

    if (normalizedSql.includes("SELECT * FROM players WHERE id")) {
      const playerId = params[0];
      if (playerId === "existing-player") {
        return [{
          results: [{
            id: "existing-player",
            name: "Test Player",
            room_id: "room-1",
            is_host: false,
            joined_at: "2024-01-01T00:00:00Z",
          }],
          success: true,
        }];
      }
      return [{ results: [], success: true }];
    }

    if (normalizedSql.includes("SELECT 1 as test")) {
      return [{ results: [{ test: 1 }], success: true }];
    }

    // Default response
    return [{ results: [], success: true }];
  }
}

// Override the getCloudflareAPI function for testing
const originalGetCloudflareAPI = (await import("../cloudflare-api.ts")).getCloudflareAPI;
let mockAPI: MockCloudflareAPI;

function setupMockAPI() {
  mockAPI = new MockCloudflareAPI();

  // Set up mock environment variables
  Deno.env.set("CLOUDFLARE_ACCOUNT_ID", "test-account");
  Deno.env.set("CLOUDFLARE_API_TOKEN", "test-token");
  Deno.env.set("DATABASE_ID", "test-db-id");
  Deno.env.set("KV_NAMESPACE_ID", "test-kv-id");
}

function restoreMockAPI() {
  // Clean up environment variables
  Deno.env.delete("CLOUDFLARE_ACCOUNT_ID");
  Deno.env.delete("CLOUDFLARE_API_TOKEN");
  Deno.env.delete("DATABASE_ID");
  Deno.env.delete("KV_NAMESPACE_ID");
}

// Create a test database service that uses our mock
class TestDatabaseService extends DatabaseService {
  constructor() {
    super();
    // @ts-ignore - Override the api property for testing
    this.api = mockAPI;
  }
}

Deno.test("DatabaseService - createRoom success", async () => {
  setupMockAPI();

  const dbService = new TestDatabaseService();
  const result = await dbService.createRoom("Test Room", "host-123", 8);

  assertEquals(result.success, true);
  assertExists(result.data);
  assertEquals(typeof result.data, "string"); // Should return room ID

  restoreMockAPI();
});

Deno.test("DatabaseService - getRoomById existing room", async () => {
  setupMockAPI();

  const dbService = new TestDatabaseService();
  const result = await dbService.getRoomById("existing-room");

  assertEquals(result.success, true);
  assertExists(result.data);
  assertEquals(result.data?.id, "existing-room");
  assertEquals(result.data?.name, "Test Room");

  restoreMockAPI();
});

Deno.test("DatabaseService - getRoomById non-existent room", async () => {
  setupMockAPI();

  const dbService = new TestDatabaseService();
  const result = await dbService.getRoomById("non-existent");

  assertEquals(result.success, true);
  assertEquals(result.data, null);

  restoreMockAPI();
});

Deno.test("DatabaseService - getActiveRooms", async () => {
  setupMockAPI();

  const dbService = new TestDatabaseService();
  const result = await dbService.getActiveRooms(10);

  assertEquals(result.success, true);
  assertExists(result.data);
  assertEquals(result.data?.length, 2);
  assertEquals(result.data?.[0].name, "Active Room 1");
  assertEquals((result.data?.[0] as any).player_count, 2);

  restoreMockAPI();
});

Deno.test("DatabaseService - createPlayer success", async () => {
  setupMockAPI();

  const dbService = new TestDatabaseService();
  const result = await dbService.createPlayer("Test Player", "room-123", false);

  assertEquals(result.success, true);
  assertExists(result.data);
  assertEquals(typeof result.data, "string"); // Should return player ID

  restoreMockAPI();
});

Deno.test("DatabaseService - getPlayerById existing player", async () => {
  setupMockAPI();

  const dbService = new TestDatabaseService();
  const result = await dbService.getPlayerById("existing-player");

  assertEquals(result.success, true);
  assertExists(result.data);
  assertEquals(result.data?.id, "existing-player");
  assertEquals(result.data?.name, "Test Player");

  restoreMockAPI();
});

Deno.test("DatabaseService - healthCheck success", async () => {
  setupMockAPI();

  const dbService = new TestDatabaseService();
  const result = await dbService.healthCheck();

  assertEquals(result.success, true);
  assertEquals(result.data, true);

  restoreMockAPI();
});

Deno.test("DatabaseService - error handling", async () => {
  setupMockAPI();

  // Override mock to throw error
  mockAPI.executeD1Query = async () => {
    throw new Error("Database connection failed");
  };

  const dbService = new TestDatabaseService();
  const result = await dbService.createRoom("Test Room", "host-123", 8);

  assertEquals(result.success, false);
  assertExists(result.error);
  assertEquals(result.error, "Database connection failed");

  restoreMockAPI();
});
