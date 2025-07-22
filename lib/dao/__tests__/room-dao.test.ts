import { assertEquals, assertExists } from "$std/assert/mod.ts";
import { RoomDAO } from "../room-dao.ts";

// Define D1Database interface locally for testing
interface D1Database {
  prepare(query: string): any;
  dump(): Promise<ArrayBuffer>;
  batch<T = unknown>(statements: any[]): Promise<any[]>;
  exec(query: string): Promise<any>;
}

// Simple mock for testing DAO layer
class MockD1Database implements D1Database {
  private rooms: any[] = [];
  private players: any[] = [];

  prepare(query: string) {
    return {
      bind: (...values: any[]) => ({
        run: async () => {
          if (query.includes('INSERT INTO rooms')) {
            const [id, name, hostId, maxPlayers, isActive] = values;
            this.rooms.push({
              id, name, host_id: hostId, max_players: maxPlayers, 
              is_active: isActive, created_at: new Date().toISOString(),
              updated_at: new Date().toISOString()
            });
          }
          return { success: true };
        },
        first: async () => {
          if (query.includes('SELECT * FROM rooms WHERE id = ?')) {
            const room = this.rooms.find(r => r.id === values[0]);
            return room ? {
              id: room.id,
              name: room.name,
              hostId: room.host_id,
              maxPlayers: room.max_players,
              isActive: room.is_active,
              createdAt: room.created_at,
              updatedAt: room.updated_at
            } : null;
          }
          if (query.includes('COUNT(p.id) as player_count')) {
            const room = this.rooms.find(r => r.id === values[0]);
            if (!room) return null;
            const playerCount = this.players.filter(p => p.room_id === values[0]).length;
            return {
              id: room.id,
              name: room.name,
              hostId: room.host_id,
              maxPlayers: room.max_players,
              isActive: room.is_active,
              createdAt: room.created_at,
              updatedAt: room.updated_at,
              player_count: playerCount
            };
          }
          return null;
        },
        all: async () => {
          if (query.includes('SELECT * FROM rooms WHERE is_active = true')) {
            const activeRooms = this.rooms
              .filter(r => r.is_active)
              .map(room => ({
                id: room.id,
                name: room.name,
                hostId: room.host_id,
                maxPlayers: room.max_players,
                isActive: room.is_active,
                createdAt: room.created_at,
                updatedAt: room.updated_at
              }));
            return { results: activeRooms };
          }
          return { results: [] };
        }
      })
    };
  }

  dump(): Promise<ArrayBuffer> { throw new Error("Not implemented"); }
  batch<T = unknown>(statements: any[]): Promise<any[]> { throw new Error("Not implemented"); }
  exec(query: string): Promise<any> { throw new Error("Not implemented"); }
}

Deno.test("RoomDAO", async (t) => {
  const mockDb = new MockD1Database();
  const roomDAO = new RoomDAO(mockDb);

  await t.step("should create a room", async () => {
    const result = await roomDAO.create({
      name: "Test Room",
      hostId: "host-123",
      maxPlayers: 8,
      isActive: true
    });

    assertEquals(result.success, true);
    assertExists(result.data);
  });

  await t.step("should find room by id", async () => {
    const createResult = await roomDAO.create({
      name: "Findable Room",
      hostId: "host-456",
      maxPlayers: 6,
      isActive: true
    });

    const roomId = createResult.data!;
    const findResult = await roomDAO.findById(roomId);

    assertEquals(findResult.success, true);
    assertEquals(findResult.data?.name, "Findable Room");
    assertEquals(findResult.data?.hostId, "host-456");
  });

  await t.step("should find active rooms", async () => {
    const result = await roomDAO.findActive();
    assertEquals(result.success, true);
    assertEquals(Array.isArray(result.data), true);
  });

  await t.step("should check if room can be joined", async () => {
    const createResult = await roomDAO.create({
      name: "Joinable Room",
      hostId: "host-789",
      maxPlayers: 4,
      isActive: true
    });

    const roomId = createResult.data!;
    const canJoinResult = await roomDAO.canJoin(roomId);

    assertEquals(canJoinResult.success, true);
    assertEquals(canJoinResult.data, true);
  });

  await t.step("should get room summary", async () => {
    const createResult = await roomDAO.create({
      name: "Summary Room",
      hostId: "host-summary",
      maxPlayers: 8,
      isActive: true
    });

    const roomId = createResult.data!;
    const summaryResult = await roomDAO.getRoomSummary(roomId);

    assertEquals(summaryResult.success, true);
    assertExists(summaryResult.data);
    assertEquals(summaryResult.data?.room.name, "Summary Room");
    assertEquals(summaryResult.data?.playerCount, 0);
    assertEquals(summaryResult.data?.canJoin, true);
  });
});