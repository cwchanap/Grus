import { assertEquals, assertExists } from "$std/assert/mod.ts";
import { PlayerDAO } from "../player-dao.ts";

// Define D1Database interface locally for testing
interface D1Database {
  prepare(query: string): any;
  dump(): Promise<ArrayBuffer>;
  batch<T = unknown>(statements: any[]): Promise<any[]>;
  exec(query: string): Promise<any>;
}

// Simple mock for testing DAO layer
class MockD1Database implements D1Database {
  private players: any[] = [];

  prepare(query: string) {
    return {
      bind: (...values: any[]) => ({
        run: async () => {
          if (query.includes('INSERT INTO players')) {
            const [id, name, roomId, isHost] = values;
            this.players.push({
              id, name, room_id: roomId, is_host: isHost,
              joined_at: new Date().toISOString()
            });
          }
          if (query.includes('UPDATE players')) {
            const playerId = values[values.length - 1];
            const playerIndex = this.players.findIndex(p => p.id === playerId);
            if (playerIndex >= 0) {
              // Simple update simulation
              if (query.includes('room_id = ?')) {
                this.players[playerIndex].room_id = values[0];
              }
              if (query.includes('is_host = ?')) {
                this.players[playerIndex].is_host = values[0];
              }
            }
          }
          return { success: true };
        },
        first: async () => {
          if (query.includes('SELECT * FROM players WHERE id = ?')) {
            const player = this.players.find(p => p.id === values[0]);
            return player ? {
              id: player.id,
              name: player.name,
              roomId: player.room_id,
              isHost: player.is_host,
              joinedAt: player.joined_at
            } : null;
          }
          if (query.includes('SELECT COUNT(*) as count FROM players WHERE room_id = ?')) {
            const count = this.players.filter(p => p.room_id === values[0]).length;
            return { count };
          }
          return null;
        },
        all: async () => {
          if (query.includes('SELECT * FROM players WHERE room_id = ?')) {
            const roomPlayers = this.players
              .filter(p => p.room_id === values[0])
              .map(player => ({
                id: player.id,
                name: player.name,
                roomId: player.room_id,
                isHost: player.is_host,
                joinedAt: player.joined_at
              }));
            return { results: roomPlayers };
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

Deno.test("PlayerDAO", async (t) => {
  const mockDb = new MockD1Database();
  const playerDAO = new PlayerDAO(mockDb);

  await t.step("should create a player", async () => {
    const result = await playerDAO.create({
      name: "Test Player",
      isHost: false
    });

    assertEquals(result.success, true);
    assertExists(result.data);
  });

  await t.step("should find player by id", async () => {
    const createResult = await playerDAO.create({
      name: "Findable Player",
      isHost: true
    });

    const playerId = createResult.data!;
    const findResult = await playerDAO.findById(playerId);

    assertEquals(findResult.success, true);
    assertEquals(findResult.data?.name, "Findable Player");
    assertEquals(findResult.data?.isHost, true);
  });

  await t.step("should find players by room", async () => {
    const roomId = "test-room-123";
    
    await playerDAO.create({
      name: "Room Player 1",
      roomId,
      isHost: true
    });

    await playerDAO.create({
      name: "Room Player 2", 
      roomId,
      isHost: false
    });

    const result = await playerDAO.findByRoom(roomId);
    assertEquals(result.success, true);
    assertEquals(result.data?.length, 2);
  });

  await t.step("should get player count for room", async () => {
    const roomId = "count-room-456";
    
    await playerDAO.create({
      name: "Count Player 1",
      roomId,
      isHost: false
    });

    const countResult = await playerDAO.getCount(roomId);
    assertEquals(countResult.success, true);
    assertEquals(countResult.data, 1);
  });

  await t.step("should add player to room", async () => {
    const createResult = await playerDAO.create({
      name: "Movable Player",
      isHost: false
    });

    const playerId = createResult.data!;
    const roomId = "new-room-789";

    const addResult = await playerDAO.addToRoom(playerId, roomId);
    assertEquals(addResult.success, true);

    const findResult = await playerDAO.findById(playerId);
    assertEquals(findResult.data?.roomId, roomId);
  });

  await t.step("should find host in room", async () => {
    const roomId = "host-room-999";
    
    await playerDAO.create({
      name: "Regular Player",
      roomId,
      isHost: false
    });

    const hostCreateResult = await playerDAO.create({
      name: "Host Player",
      roomId,
      isHost: true
    });

    const hostResult = await playerDAO.findHost(roomId);
    assertEquals(hostResult.success, true);
    assertEquals(hostResult.data?.name, "Host Player");
    assertEquals(hostResult.data?.isHost, true);
  });

  await t.step("should validate player in room", async () => {
    const roomId = "validation-room";
    const createResult = await playerDAO.create({
      name: "Validation Player",
      roomId,
      isHost: false
    });

    const playerId = createResult.data!;
    const validationResult = await playerDAO.validatePlayerInRoom(playerId, roomId);
    
    assertEquals(validationResult.success, true);
    assertEquals(validationResult.data, true);
  });
});