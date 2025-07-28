import { assertEquals, assertExists } from "$std/testing/asserts.ts";
import { KVService } from "../kv-service.ts";

// Mock the CloudflareAPI for KV operations
class MockCloudflareAPI {
  private storage: Map<string, string> = new Map();
  
  async kvGet(key: string): Promise<string | null> {
    return this.storage.get(key) || null;
  }
  
  async kvPut(key: string, value: string, options?: { expirationTtl?: number }): Promise<void> {
    this.storage.set(key, value);
    
    // Simulate TTL by removing after timeout (for testing)
    if (options?.expirationTtl && options.expirationTtl < 10) {
      setTimeout(() => this.storage.delete(key), options.expirationTtl * 1000);
    }
  }
  
  async kvDelete(key: string): Promise<void> {
    this.storage.delete(key);
  }
  
  async kvList(options: { prefix?: string; limit?: number } = {}): Promise<any> {
    const keys = Array.from(this.storage.keys());
    const filteredKeys = options.prefix 
      ? keys.filter(key => key.startsWith(options.prefix!))
      : keys;
    
    const limitedKeys = options.limit 
      ? filteredKeys.slice(0, options.limit)
      : filteredKeys;
    
    return {
      keys: limitedKeys.map(name => ({ name, expiration: null })),
      list_complete: true
    };
  }
  
  // Add some test data
  seedTestData() {
    this.storage.set("game_state:room-1", JSON.stringify({
      roomId: "room-1",
      currentRound: 1,
      players: ["player-1", "player-2"]
    }));
    
    this.storage.set("player_session:player-1", JSON.stringify({
      playerId: "player-1",
      roomId: "room-1",
      isActive: true
    }));
    
    this.storage.set("message:room-1:1234567890", JSON.stringify({
      id: "msg-1",
      roomId: "room-1",
      playerId: "player-1",
      message: "Hello!",
      timestamp: 1234567890
    }));
  }
  
  clear() {
    this.storage.clear();
  }
}

// Create a test KV service that uses our mock
class TestKVService extends KVService {
  constructor(mockAPI: MockCloudflareAPI) {
    super();
    // @ts-ignore - Override the api property for testing
    this.api = mockAPI;
  }
}

let mockAPI: MockCloudflareAPI;
let kvService: TestKVService;

function setupTest() {
  // Set up mock environment variables
  Deno.env.set('CLOUDFLARE_ACCOUNT_ID', 'test-account');
  Deno.env.set('CLOUDFLARE_API_TOKEN', 'test-token');
  Deno.env.set('DATABASE_ID', 'test-db-id');
  Deno.env.set('KV_NAMESPACE_ID', 'test-kv-id');
  
  mockAPI = new MockCloudflareAPI();
  kvService = new TestKVService(mockAPI);
}

function teardownTest() {
  mockAPI.clear();
  
  // Clean up environment variables
  Deno.env.delete('CLOUDFLARE_ACCOUNT_ID');
  Deno.env.delete('CLOUDFLARE_API_TOKEN');
  Deno.env.delete('DATABASE_ID');
  Deno.env.delete('KV_NAMESPACE_ID');
}

Deno.test("KVService - setGameState and getGameState", async () => {
  setupTest();
  
  const gameState = {
    roomId: "room-123",
    currentRound: 2,
    players: ["player-1", "player-2", "player-3"],
    isActive: true
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
  
  teardownTest();
});

Deno.test("KVService - getGameState non-existent", async () => {
  setupTest();
  
  const result = await kvService.getGameState("non-existent-room");
  assertEquals(result.success, true);
  assertEquals(result.data, null);
  
  teardownTest();
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
  
  teardownTest();
});

Deno.test("KVService - setPlayerSession and getPlayerSession", async () => {
  setupTest();
  
  const session = {
    playerId: "player-789",
    roomId: "room-123",
    isActive: true,
    joinedAt: Date.now()
  };
  
  // Set player session
  const setResult = await kvService.setPlayerSession("player-789", session, 3600);
  assertEquals(setResult.success, true);
  
  // Get player session
  const getResult = await kvService.getPlayerSession("player-789");
  assertEquals(getResult.success, true);
  assertEquals(getResult.data?.playerId, "player-789");
  assertEquals(getResult.data?.roomId, "room-123");
  
  teardownTest();
});

Deno.test("KVService - cacheRoomData", async () => {
  setupTest();
  
  const roomData = {
    id: "room-999",
    name: "Cached Room",
    playerCount: 5,
    isActive: true
  };
  
  // Cache room data
  const cacheResult = await kvService.cacheRoomData("room-999", roomData, 300);
  assertEquals(cacheResult.success, true);
  
  // Get cached data
  const getResult = await kvService.getCachedRoomData("room-999");
  assertEquals(getResult.success, true);
  assertEquals(getResult.data?.name, "Cached Room");
  assertEquals(getResult.data?.playerCount, 5);
  
  teardownTest();
});

Deno.test("KVService - saveDrawingData", async () => {
  setupTest();
  
  const drawingData = {
    roomId: "room-art",
    playerId: "artist-1",
    strokes: [
      { x: 100, y: 100, color: "#000000" },
      { x: 150, y: 150, color: "#ff0000" }
    ],
    timestamp: Date.now()
  };
  
  const result = await kvService.saveDrawingData("room-art", drawingData);
  assertEquals(result.success, true);
  
  teardownTest();
});

Deno.test("KVService - cacheMessage", async () => {
  setupTest();
  
  const message = {
    id: "msg-123",
    roomId: "room-chat",
    playerId: "player-1",
    message: "Hello everyone!",
    timestamp: Date.now()
  };
  
  const result = await kvService.cacheMessage("room-chat", message);
  assertEquals(result.success, true);
  
  teardownTest();
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
  
  teardownTest();
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
  
  teardownTest();
});

Deno.test("KVService - healthCheck", async () => {
  setupTest();
  
  const result = await kvService.healthCheck();
  assertEquals(result.success, true);
  assertEquals(result.data, true);
  
  teardownTest();
});

Deno.test("KVService - error handling", async () => {
  setupTest();
  
  // Override mock to throw error
  mockAPI.kvGet = async () => {
    throw new Error("KV connection failed");
  };
  
  const result = await kvService.get("test-key");
  assertEquals(result.success, false);
  assertExists(result.error);
  assertEquals(result.error, "KV connection failed");
  
  teardownTest();
});