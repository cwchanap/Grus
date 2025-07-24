import { assertEquals, assertExists } from "https://deno.land/std@0.208.0/assert/mod.ts";
import { GameStateManager } from "../game-state-manager.ts";
import type { GameState } from "../../types/game.ts";

// Mock D1Database and KVNamespace for testing
const mockDB = {
  prepare: (query: string) => ({
    bind: (...args: any[]) => ({
      first: async () => null,
      all: async () => ({ results: [] }),
      run: async () => ({ success: true })
    })
  })
} as any;

const mockKV = {
  storage: new Map<string, string>(),
  get: async function(key: string, options?: any) {
    const value = this.storage.get(key);
    if (!value) return null;
    if (options?.type === "json") {
      return JSON.parse(value);
    }
    return value;
  },
  put: async function(key: string, value: string, options?: any) {
    this.storage.set(key, value);
  },
  delete: async function(key: string) {
    this.storage.delete(key);
  }
} as any;

Deno.test("Game Flow - Turn Management", async () => {
  const gameManager = new GameStateManager(mockDB, mockKV);
  
  // Mock game state with multiple players
  const mockGameState: GameState = {
    roomId: "test-room",
    currentDrawer: "",
    currentWord: "",
    roundNumber: 0,
    timeRemaining: 120000,
    phase: "waiting",
    players: [
      { id: "player1", name: "Alice", isHost: true, isConnected: true, lastActivity: Date.now() },
      { id: "player2", name: "Bob", isHost: false, isConnected: true, lastActivity: Date.now() },
      { id: "player3", name: "Charlie", isHost: false, isConnected: true, lastActivity: Date.now() }
    ],
    scores: { "player1": 0, "player2": 0, "player3": 0 },
    drawingData: []
  };

  // Mock KV to return our test game state
  // Pre-populate the mock KV with initial game state
  await mockKV.put("game:test-room", JSON.stringify(mockGameState));

  // Test starting a new round
  const updatedState = await gameManager.startNewRound(mockGameState);
  
  // Verify round started correctly
  assertEquals(updatedState.roundNumber, 1);
  assertEquals(updatedState.phase, "drawing");
  assertExists(updatedState.currentDrawer);
  assertExists(updatedState.currentWord);
  assertEquals(updatedState.timeRemaining, 120000);
  assertEquals(updatedState.drawingData.length, 0);
});

Deno.test("Game Flow - Correct Guess Processing", async () => {
  const gameManager = new GameStateManager(mockDB, mockKV);
  
  const mockGameState: GameState = {
    roomId: "test-room",
    currentDrawer: "player1",
    currentWord: "cat",
    roundNumber: 1,
    timeRemaining: 60000,
    phase: "drawing",
    players: [
      { id: "player1", name: "Alice", isHost: true, isConnected: true, lastActivity: Date.now() },
      { id: "player2", name: "Bob", isHost: false, isConnected: true, lastActivity: Date.now() }
    ],
    scores: { "player1": 0, "player2": 0 },
    drawingData: []
  };

  // Pre-populate the mock KV with game state
  await mockKV.put("game:test-room", JSON.stringify(mockGameState));

  // Test processing correct guess
  const updatedState = await gameManager.processCorrectGuess("test-room", "player2", Date.now());
  
  // Verify scores were updated
  assertEquals(updatedState.phase, "results");
  assertEquals(updatedState.scores["player2"] > 0, true); // Guesser got points
  assertEquals(updatedState.scores["player1"] > 0, true); // Drawer got points too
});

Deno.test("Game Flow - Turn Rotation", async () => {
  const gameManager = new GameStateManager(mockDB, mockKV);
  
  let currentDrawer = "player1";
  let roundNumber = 0;
  const mockGameState: GameState = {
    roomId: "test-room",
    currentDrawer,
    currentWord: "test",
    roundNumber,
    timeRemaining: 120000,
    phase: "waiting",
    players: [
      { id: "player1", name: "Alice", isHost: true, isConnected: true, lastActivity: Date.now() },
      { id: "player2", name: "Bob", isHost: false, isConnected: true, lastActivity: Date.now() },
      { id: "player3", name: "Charlie", isHost: false, isConnected: true, lastActivity: Date.now() }
    ],
    scores: { "player1": 0, "player2": 0, "player3": 0 },
    drawingData: []
  };

  // Test multiple rounds to verify turn rotation
  for (let round = 1; round <= 3; round++) {
    // Update the mock KV with current state
    await mockKV.put("game:test-room", JSON.stringify({ ...mockGameState, currentDrawer, roundNumber }));

    const updatedState = await gameManager.startNewRound({ ...mockGameState, currentDrawer, roundNumber });
    
    // Verify drawer rotated
    assertEquals(updatedState.roundNumber, round);
    assertEquals(updatedState.phase, "drawing");
    
    // Update for next iteration
    const previousDrawer = currentDrawer;
    currentDrawer = updatedState.currentDrawer;
    roundNumber = updatedState.roundNumber;
    
    // Verify it's a different player (unless only 1 player)
    if (mockGameState.players.length > 1 && round > 1) {
      assertEquals(currentDrawer !== previousDrawer, true);
    }
  }
});

Deno.test("Game Flow - Game Completion", async () => {
  const gameManager = new GameStateManager(mockDB, mockKV, { maxRounds: 3 });
  
  const mockGameState: GameState = {
    roomId: "test-room",
    currentDrawer: "player1",
    currentWord: "test",
    roundNumber: 3, // At max rounds
    timeRemaining: 120000,
    phase: "drawing",
    players: [
      { id: "player1", name: "Alice", isHost: true, isConnected: true, lastActivity: Date.now() },
      { id: "player2", name: "Bob", isHost: false, isConnected: true, lastActivity: Date.now() }
    ],
    scores: { "player1": 150, "player2": 200 },
    drawingData: []
  };

  // Pre-populate the mock KV with game state
  await mockKV.put("game:test-room", JSON.stringify(mockGameState));

  // Test ending the game
  const finalState = await gameManager.endRound("test-room");
  
  // Verify game ended
  assertEquals(finalState.phase, "results");
  assertEquals(finalState.currentDrawer, "");
  assertEquals(finalState.currentWord, "");
});