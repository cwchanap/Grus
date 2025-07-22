import { assertEquals, assertExists, assertFalse } from "$std/assert/mod.ts";
import { DatabaseService } from "../../database.ts";

// Define D1Database interface locally for testing
interface D1Database {
  prepare(query: string): any;
  dump(): Promise<ArrayBuffer>;
  batch<T = unknown>(statements: any[]): Promise<any[]>;
  exec(query: string): Promise<any>;
}

// Mock D1Database for testing
class MockD1Database implements D1Database {
  private data: Map<string, any[]> = new Map();
  private nextId = 1;

  constructor() {
    // Initialize tables
    this.data.set('rooms', []);
    this.data.set('players', []);
    this.data.set('game_sessions', []);
    this.data.set('scores', []);
  }

  prepare(query: string) {
    return new MockD1PreparedStatement(query, this.data);
  }

  dump(): Promise<ArrayBuffer> {
    throw new Error("Not implemented");
  }

  batch<T = unknown>(statements: any[]): Promise<any[]> {
    throw new Error("Not implemented");
  }

  exec(query: string): Promise<any> {
    throw new Error("Not implemented");
  }
}

class MockD1PreparedStatement {
  private boundValues: any[] = [];

  constructor(
    private query: string,
    private data: Map<string, any[]>
  ) {}

  async raw<T = unknown>(): Promise<T[]> {
    const result = await this.all<T>();
    return result.results || [];
  }

  bind(...values: any[]) {
    this.boundValues = values;
    return this;
  }

  async first<T = unknown>(): Promise<T | null> {
    const results = await this.all<T>();
    return results.results?.[0] || null;
  }

  async run(): Promise<any> {
    // Simple INSERT simulation
    if (this.query.includes('INSERT INTO rooms')) {
      const [id, name, hostId, maxPlayers, isActive] = this.boundValues;
      const rooms = this.data.get('rooms')!;
      rooms.push({
        id,
        name,
        host_id: hostId,
        max_players: maxPlayers,
        is_active: isActive,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString()
      });
      return { success: true };
    }

    if (this.query.includes('INSERT INTO players')) {
      const [id, name, roomId, isHost] = this.boundValues;
      const players = this.data.get('players')!;
      players.push({
        id,
        name,
        room_id: roomId,
        is_host: isHost,
        joined_at: new Date().toISOString()
      });
      return { success: true };
    }

    if (this.query.includes('UPDATE rooms')) {
      const rooms = this.data.get('rooms')!;
      const roomId = this.boundValues[this.boundValues.length - 1];
      const roomIndex = rooms.findIndex(r => r.id === roomId);
      if (roomIndex >= 0) {
        rooms[roomIndex].updated_at = new Date().toISOString();
        // Simple update simulation - would need more complex logic for real updates
      }
      return { success: true };
    }

    return { success: true };
  }

  async all<T = unknown>(): Promise<{ results: T[] }> {
    // Simple SELECT simulation
    if (this.query.includes('SELECT * FROM rooms WHERE id = ?')) {
      const rooms = this.data.get('rooms')!;
      const room = rooms.find(r => r.id === this.boundValues[0]);
      if (room) {
        const transformedRoom = {
          id: room.id,
          name: room.name,
          hostId: room.host_id,
          maxPlayers: room.max_players,
          isActive: room.is_active,
          createdAt: room.created_at,
          updatedAt: room.updated_at
        };
        return { results: [transformedRoom as T] };
      }
      return { results: [] };
    }

    if (this.query.includes('SELECT * FROM rooms WHERE is_active = true')) {
      const rooms = this.data.get('rooms')!;
      const activeRooms = rooms.filter(r => r.is_active);
      return { results: activeRooms as T[] };
    }

    if (this.query.includes('SELECT * FROM players WHERE id = ?')) {
      const players = this.data.get('players')!;
      const player = players.find(p => p.id === this.boundValues[0]);
      if (player) {
        const transformedPlayer = {
          id: player.id,
          name: player.name,
          roomId: player.room_id,
          isHost: player.is_host,
          joinedAt: player.joined_at
        };
        return { results: [transformedPlayer as T] };
      }
      return { results: [] };
    }

    if (this.query.includes('SELECT * FROM players WHERE room_id = ?')) {
      const players = this.data.get('players')!;
      const roomPlayers = players.filter(p => p.room_id === this.boundValues[0]);
      const transformedPlayers = roomPlayers.map(player => ({
        id: player.id,
        name: player.name,
        roomId: player.room_id,
        isHost: player.is_host,
        joinedAt: player.joined_at
      }));
      return { results: transformedPlayers as T[] };
    }

    if (this.query.includes('SELECT COUNT(*) as count')) {
      if (this.query.includes('FROM players WHERE room_id = ?')) {
        const players = this.data.get('players')!;
        const count = players.filter(p => p.room_id === this.boundValues[0]).length;
        return { results: [{ count } as T] };
      }
    }

    if (this.query.includes('SELECT 1 as test')) {
      return { results: [{ test: 1 } as T] };
    }

    return { results: [] };
  }
}

Deno.test("DatabaseService - Room Operations", async (t) => {
  const mockDb = new MockD1Database();
  const dbService = new DatabaseService(mockDb);

  await t.step("should create a room successfully", async () => {
    const result = await dbService.createRoom({
      name: "Test Room",
      hostId: "host-123",
      maxPlayers: 8,
      isActive: true
    });

    assertEquals(result.success, true);
    assertExists(result.data);
  });

  await t.step("should retrieve room by id", async () => {
    // First create a room
    const createResult = await dbService.createRoom({
      name: "Test Room 2",
      hostId: "host-456",
      maxPlayers: 6,
      isActive: true
    });

    assertEquals(createResult.success, true);
    const roomId = createResult.data!;

    // Then retrieve it
    const getResult = await dbService.getRoomById(roomId);
    assertEquals(getResult.success, true);
    assertEquals(getResult.data?.name, "Test Room 2");
    assertEquals(getResult.data?.hostId, "host-456");
  });

  await t.step("should get active rooms", async () => {
    const result = await dbService.getActiveRooms();
    assertEquals(result.success, true);
    assertEquals(Array.isArray(result.data), true);
  });

  await t.step("should update room", async () => {
    const createResult = await dbService.createRoom({
      name: "Room to Update",
      hostId: "host-789",
      maxPlayers: 4,
      isActive: true
    });

    const roomId = createResult.data!;
    const updateResult = await dbService.updateRoom(roomId, {
      name: "Updated Room Name",
      maxPlayers: 10
    });

    assertEquals(updateResult.success, true);
  });
});

Deno.test("DatabaseService - Player Operations", async (t) => {
  const mockDb = new MockD1Database();
  const dbService = new DatabaseService(mockDb);

  await t.step("should create a player successfully", async () => {
    const result = await dbService.createPlayer({
      name: "Test Player",
      isHost: false
    });

    assertEquals(result.success, true);
    assertExists(result.data);
  });

  await t.step("should retrieve player by id", async () => {
    const createResult = await dbService.createPlayer({
      name: "Player 2",
      isHost: true
    });

    const playerId = createResult.data!;
    const getResult = await dbService.getPlayerById(playerId);
    
    assertEquals(getResult.success, true);
    assertEquals(getResult.data?.name, "Player 2");
    assertEquals(getResult.data?.isHost, true);
  });

  await t.step("should get players by room", async () => {
    const roomId = "test-room-for-players";

    // Create players in the room
    await dbService.createPlayer({
      name: "Room Player 1",
      roomId,
      isHost: true
    });

    await dbService.createPlayer({
      name: "Room Player 2",
      roomId,
      isHost: false
    });

    const playersResult = await dbService.getPlayersByRoom(roomId);
    assertEquals(playersResult.success, true);
    assertEquals(playersResult.data?.length, 2);
  });

  await t.step("should get player count for room", async () => {
    const roomResult = await dbService.createRoom({
      name: "Count Test Room",
      hostId: "host-456",
      maxPlayers: 8,
      isActive: true
    });
    const roomId = roomResult.data!;

    const countResult = await dbService.getPlayerCount(roomId);
    assertEquals(countResult.success, true);
    assertEquals(countResult.data, 0);
  });
});

Deno.test("DatabaseService - Error Handling", async (t) => {
  const mockDb = new MockD1Database();
  const dbService = new DatabaseService(mockDb);

  await t.step("should handle empty updates gracefully", async () => {
    const result = await dbService.updateRoom("non-existent-id", {});
    assertEquals(result.success, false);
    assertExists(result.error);
  });

  await t.step("should perform health check", async () => {
    const result = await dbService.healthCheck();
    assertEquals(result.success, true);
    assertEquals(result.data, true);
  });
});

Deno.test("DatabaseService - Stats", async (t) => {
  const mockDb = new MockD1Database();
  const dbService = new DatabaseService(mockDb);

  await t.step("should get database stats", async () => {
    const result = await dbService.getStats();
    assertEquals(result.success, true);
    assertExists(result.data);
    assertEquals(typeof result.data?.totalRooms, "number");
    assertEquals(typeof result.data?.activeRooms, "number");
    assertEquals(typeof result.data?.totalPlayers, "number");
    assertEquals(typeof result.data?.totalSessions, "number");
  });
});