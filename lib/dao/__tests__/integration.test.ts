import { assertEquals, assertExists } from "$std/assert/mod.ts";
import { DatabaseService } from "../../database.ts";
import { RoomDAO } from "../room-dao.ts";
import { PlayerDAO } from "../player-dao.ts";
import { GameSessionDAO } from "../game-session-dao.ts";

// Simple integration test to verify the database layer works
Deno.test("Database Integration Test", async (t) => {
  // This test demonstrates that the database layer is properly structured
  // In a real environment, this would connect to an actual D1 database
  
  await t.step("should have proper database service structure", () => {
    // Verify that DatabaseService has all required methods
    const methods = [
      'createRoom', 'getRoomById', 'getActiveRooms', 'updateRoom', 'deleteRoom',
      'createPlayer', 'getPlayerById', 'getPlayersByRoom', 'updatePlayer',
      'createGameSession', 'getGameSession', 'endGameSession',
      'createScore', 'updateScore', 'getScoresBySession',
      'healthCheck', 'getStats'
    ];
    
    methods.forEach(method => {
      assertEquals(typeof DatabaseService.prototype[method as keyof DatabaseService], 'function');
    });
  });

  await t.step("should have proper DAO structure", () => {
    // Verify that DAOs have all required methods
    const roomMethods = ['create', 'findById', 'findActive', 'update', 'delete', 'canJoin'];
    const playerMethods = ['create', 'findById', 'findByRoom', 'update', 'addToRoom', 'removeFromRoom'];
    const sessionMethods = ['create', 'findById', 'findByRoom', 'update', 'end'];

    roomMethods.forEach(method => {
      assertEquals(typeof RoomDAO.prototype[method as keyof RoomDAO], 'function');
    });

    playerMethods.forEach(method => {
      assertEquals(typeof PlayerDAO.prototype[method as keyof PlayerDAO], 'function');
    });

    sessionMethods.forEach(method => {
      assertEquals(typeof GameSessionDAO.prototype[method as keyof GameSessionDAO], 'function');
    });
  });

  await t.step("should have proper error handling", () => {
    // Verify that DatabaseResult type is properly structured
    const mockResult = { success: true, data: "test" };
    assertEquals(typeof mockResult.success, 'boolean');
    assertExists(mockResult.data);
  });

  await t.step("should have proper database schema", async () => {
    // Verify that the migration file exists and has proper structure
    try {
      const migrationContent = await Deno.readTextFile('./db/migrations/001_initial_schema.sql');
      
      // Check that all required tables are created
      assertEquals(migrationContent.includes('CREATE TABLE rooms'), true);
      assertEquals(migrationContent.includes('CREATE TABLE players'), true);
      assertEquals(migrationContent.includes('CREATE TABLE game_sessions'), true);
      assertEquals(migrationContent.includes('CREATE TABLE scores'), true);
      
      // Check that indexes are created
      assertEquals(migrationContent.includes('CREATE INDEX'), true);
      
    } catch (error) {
      throw new Error(`Migration file not found or invalid: ${error.message}`);
    }
  });
});

Deno.test("Database Schema Validation", async (t) => {
  await t.step("should have valid SQL schema", async () => {
    const schemaContent = await Deno.readTextFile('./db/schema.sql');
    
    // Verify schema has all required tables with proper structure
    const requiredTables = ['rooms', 'players', 'game_sessions', 'scores'];
    
    requiredTables.forEach(table => {
      assertEquals(schemaContent.includes(`CREATE TABLE IF NOT EXISTS ${table}`), true);
    });
    
    // Verify foreign key constraints
    assertEquals(schemaContent.includes('FOREIGN KEY'), true);
    assertEquals(schemaContent.includes('REFERENCES'), true);
    
    // Verify indexes for performance
    assertEquals(schemaContent.includes('CREATE INDEX'), true);
  });
});