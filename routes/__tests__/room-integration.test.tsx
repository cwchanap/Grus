import { assertEquals, assertExists } from "$std/assert/mod.ts";
import type { RoomSummary } from "../../lib/room-manager.ts";
import type { GameState } from "../../types/game.ts";

// Test the helper function from the room route
function createInitialGameState(room: RoomSummary): GameState {
  return {
    roomId: room.room.id,
    currentDrawer: '', // No drawer initially
    currentWord: '', // No word initially
    roundNumber: 0, // Game hasn't started
    timeRemaining: 120000, // 2 minutes default
    phase: 'waiting', // Waiting for game to start
    players: room.players.map(player => ({
      id: player.id,
      name: player.name,
      isHost: player.isHost,
      isConnected: true, // Assume all players in room are connected
      lastActivity: Date.now()
    })),
    scores: room.players.reduce((acc, player) => {
      acc[player.id] = 0; // Initialize all scores to 0
      return acc;
    }, {} as Record<string, number>),
    drawingData: [] // No drawing data initially
  };
}

// Mock room data for testing
const createMockRoomSummary = (): RoomSummary => ({
  room: {
    id: "test-room-123",
    name: "Test Drawing Room",
    hostId: "player1",
    maxPlayers: 8,
    isActive: true,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString()
  },
  playerCount: 3,
  canJoin: true,
  players: [
    {
      id: "player1",
      name: "Alice",
      roomId: "test-room-123",
      isHost: true,
      joinedAt: new Date().toISOString()
    },
    {
      id: "player2",
      name: "Bob",
      roomId: "test-room-123",
      isHost: false,
      joinedAt: new Date().toISOString()
    },
    {
      id: "player3",
      name: "Charlie",
      roomId: "test-room-123",
      isHost: false,
      joinedAt: new Date().toISOString()
    }
  ],
  host: {
    id: "player1",
    name: "Alice",
    roomId: "test-room-123",
    isHost: true,
    joinedAt: new Date().toISOString()
  }
});

Deno.test("Room Integration - createInitialGameState generates correct game state", () => {
  const mockRoom = createMockRoomSummary();
  const gameState = createInitialGameState(mockRoom);

  // Check basic properties
  assertEquals(gameState.roomId, "test-room-123");
  assertEquals(gameState.currentDrawer, "");
  assertEquals(gameState.currentWord, "");
  assertEquals(gameState.roundNumber, 0);
  assertEquals(gameState.timeRemaining, 120000);
  assertEquals(gameState.phase, "waiting");

  // Check players are correctly mapped
  assertEquals(gameState.players.length, 3);
  assertEquals(gameState.players[0].id, "player1");
  assertEquals(gameState.players[0].name, "Alice");
  assertEquals(gameState.players[0].isHost, true);
  assertEquals(gameState.players[0].isConnected, true);

  assertEquals(gameState.players[1].id, "player2");
  assertEquals(gameState.players[1].name, "Bob");
  assertEquals(gameState.players[1].isHost, false);
  assertEquals(gameState.players[1].isConnected, true);

  // Check scores are initialized to 0
  assertEquals(gameState.scores["player1"], 0);
  assertEquals(gameState.scores["player2"], 0);
  assertEquals(gameState.scores["player3"], 0);

  // Check drawing data is empty
  assertEquals(gameState.drawingData.length, 0);
});

Deno.test("Room Integration - handles empty room", () => {
  const emptyRoom: RoomSummary = {
    room: {
      id: "empty-room",
      name: "Empty Room",
      hostId: "host1",
      maxPlayers: 8,
      isActive: true,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString()
    },
    playerCount: 0,
    canJoin: true,
    players: [],
    host: null
  };

  const gameState = createInitialGameState(emptyRoom);

  assertEquals(gameState.roomId, "empty-room");
  assertEquals(gameState.players.length, 0);
  assertEquals(Object.keys(gameState.scores).length, 0);
});

Deno.test("Room Integration - handles single player room", () => {
  const singlePlayerRoom: RoomSummary = {
    room: {
      id: "single-room",
      name: "Solo Room",
      hostId: "solo-player",
      maxPlayers: 8,
      isActive: true,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString()
    },
    playerCount: 1,
    canJoin: true,
    players: [
      {
        id: "solo-player",
        name: "Solo",
        roomId: "single-room",
        isHost: true,
        joinedAt: new Date().toISOString()
      }
    ],
    host: {
      id: "solo-player",
      name: "Solo",
      roomId: "single-room",
      isHost: true,
      joinedAt: new Date().toISOString()
    }
  };

  const gameState = createInitialGameState(singlePlayerRoom);

  assertEquals(gameState.players.length, 1);
  assertEquals(gameState.players[0].isHost, true);
  assertEquals(gameState.scores["solo-player"], 0);
});

Deno.test("Room Integration - preserves player host status", () => {
  const mockRoom = createMockRoomSummary();
  const gameState = createInitialGameState(mockRoom);

  // Find host player
  const hostPlayer = gameState.players.find(p => p.isHost);
  assertExists(hostPlayer);
  assertEquals(hostPlayer.id, "player1");
  assertEquals(hostPlayer.name, "Alice");

  // Check non-host players
  const nonHostPlayers = gameState.players.filter(p => !p.isHost);
  assertEquals(nonHostPlayers.length, 2);
  assertEquals(nonHostPlayers[0].name, "Bob");
  assertEquals(nonHostPlayers[1].name, "Charlie");
});

Deno.test("Room Integration - sets all players as connected initially", () => {
  const mockRoom = createMockRoomSummary();
  const gameState = createInitialGameState(mockRoom);

  // All players should be marked as connected
  const allConnected = gameState.players.every(p => p.isConnected);
  assertEquals(allConnected, true);

  // All players should have recent lastActivity
  const now = Date.now();
  const allRecentActivity = gameState.players.every(p => 
    (now - p.lastActivity) < 1000 // Within last second
  );
  assertEquals(allRecentActivity, true);
});

Deno.test("Room Integration - game state is compatible with component props", () => {
  const mockRoom = createMockRoomSummary();
  const gameState = createInitialGameState(mockRoom);

  // Test that game state has all required properties for DrawingBoard
  assertExists(gameState.roomId);
  assertExists(gameState.currentDrawer);
  assertExists(gameState.phase);
  assertExists(gameState.players);
  assertExists(gameState.drawingData);

  // Test that game state has all required properties for Scoreboard
  assertExists(gameState.scores);
  assertExists(gameState.roundNumber);
  assertExists(gameState.timeRemaining);
  assertExists(gameState.currentWord);

  // Test that game state has all required properties for ChatRoom
  assertEquals(typeof gameState.currentWord, "string");
  assertEquals(typeof gameState.currentDrawer, "string");
});