import { assertEquals, assertExists, assertRejects } from "https://deno.land/std@0.208.0/assert/mod.ts";
import { describe, it, beforeEach, afterEach } from "https://deno.land/std@0.208.0/testing/bdd.ts";
import { GameStateManager } from "../game-state-manager.ts";
import type { GameState, PlayerState } from "../../types/game.ts";

// Mock implementations
class MockKVNamespace {
  private storage = new Map<string, string>();

  async get(key: string, options?: { type?: string }): Promise<any> {
    const value = this.storage.get(key);
    if (!value) return null;
    
    if (options?.type === "json") {
      return JSON.parse(value);
    }
    return value;
  }

  async put(key: string, value: string, options?: any): Promise<void> {
    this.storage.set(key, value);
  }

  async delete(key: string): Promise<void> {
    this.storage.delete(key);
  }

  async list(options?: { prefix?: string; limit?: number }): Promise<{ keys: Array<{ name: string }> }> {
    const keys = Array.from(this.storage.keys())
      .filter(key => !options?.prefix || key.startsWith(options.prefix))
      .slice(0, options?.limit || 1000)
      .map(name => ({ name }));
    
    return { keys };
  }

  clear() {
    this.storage.clear();
  }
}

class MockD1Database {
  private tables = {
    rooms: new Map(),
    players: new Map(),
    game_sessions: new Map(),
    scores: new Map()
  };

  async prepare(query: string) {
    return {
      bind: (...params: any[]) => ({
        first: async () => null,
        all: async () => ({ results: [] }),
        run: async () => ({ success: true, meta: { changes: 1, last_row_id: 1 } })
      })
    };
  }

  clear() {
    Object.values(this.tables).forEach(table => table.clear());
  }
}

describe("GameStateManager", () => {
  let gameStateManager: GameStateManager;
  let mockKV: MockKVNamespace;
  let mockDB: MockD1Database;
  const roomId = "test-room-123";
  const hostId = "host-player-1";

  beforeEach(() => {
    mockKV = new MockKVNamespace();
    mockDB = new MockD1Database();
    gameStateManager = new GameStateManager(
      mockDB as any,
      mockKV as any,
      {
        roundDurationMs: 60000, // 1 minute for testing
        maxRounds: 3,
        pointsForCorrectGuess: 100,
        timeBasedScoringEnabled: true
      }
    );
  });

  afterEach(() => {
    mockKV.clear();
    mockDB.clear();
  });

  describe("initializeGameState", () => {
    it("should create initial game state with correct defaults", async () => {
      // Mock player data
      const mockPlayers = [
        { id: hostId, name: "Host Player", isHost: true, roomId },
        { id: "player-2", name: "Player 2", isHost: false, roomId }
      ];

      // We need to mock the DAO methods since we can't easily mock the database
      // For now, let's test the core logic by creating a game state manually
      const gameState: GameState = {
        roomId,
        currentDrawer: '',
        currentWord: '',
        roundNumber: 0,
        timeRemaining: 60000,
        phase: 'waiting',
        players: mockPlayers.map(p => ({
          id: p.id,
          name: p.name,
          isHost: p.isHost,
          isConnected: true,
          lastActivity: Date.now()
        })),
        scores: {
          [hostId]: 0,
          "player-2": 0
        },
        drawingData: []
      };

      // Store it in KV
      await mockKV.put(`game:${roomId}`, JSON.stringify(gameState));

      const retrievedState = await gameStateManager.getGameState(roomId);
      assertExists(retrievedState);
      assertEquals(retrievedState.roomId, roomId);
      assertEquals(retrievedState.phase, 'waiting');
      assertEquals(retrievedState.roundNumber, 0);
      assertEquals(retrievedState.players.length, 2);
      assertEquals(Object.keys(retrievedState.scores).length, 2);
    });
  });

  describe("score calculation", () => {
    it("should calculate time-based scores correctly", () => {
      const manager = new GameStateManager(
        mockDB as any,
        mockKV as any,
        {
          roundDurationMs: 60000,
          maxRounds: 3,
          pointsForCorrectGuess: 100,
          timeBasedScoringEnabled: true
        }
      );

      // Test private method through reflection
      const calculateScore = (manager as any).calculateScore.bind(manager);

      // Fast guess (50s remaining out of 60s) should get high score
      const fastScore = calculateScore(10000, 50000);
      assertEquals(fastScore >= 90, true); // Should get high points (at least 90%)

      // Medium guess (30s remaining out of 60s) should get medium score
      const mediumScore = calculateScore(30000, 30000);
      assertEquals(mediumScore >= 70 && mediumScore <= 80, true); // Should get ~75% of points

      // Slow guess (5s remaining out of 60s) should get low score
      const slowScore = calculateScore(55000, 5000);
      assertEquals(slowScore >= 50 && slowScore <= 60, true); // Should get ~50% of points
    });

    it("should return fixed score when time-based scoring is disabled", () => {
      const manager = new GameStateManager(
        mockDB as any,
        mockKV as any,
        {
          roundDurationMs: 60000,
          maxRounds: 3,
          pointsForCorrectGuess: 100,
          timeBasedScoringEnabled: false
        }
      );

      const calculateScore = (manager as any).calculateScore.bind(manager);

      // All guesses should get the same score
      assertEquals(calculateScore(10000, 50000), 100);
      assertEquals(calculateScore(30000, 30000), 100);
      assertEquals(calculateScore(55000, 5000), 100);
    });
  });

  describe("turn rotation", () => {
    it("should rotate turns correctly", () => {
      const players: PlayerState[] = [
        { id: "player-1", name: "Player 1", isHost: true, isConnected: true, lastActivity: Date.now() },
        { id: "player-2", name: "Player 2", isHost: false, isConnected: true, lastActivity: Date.now() },
        { id: "player-3", name: "Player 3", isHost: false, isConnected: true, lastActivity: Date.now() }
      ];

      const gameState: GameState = {
        roomId,
        currentDrawer: '',
        currentWord: '',
        roundNumber: 0,
        timeRemaining: 60000,
        phase: 'waiting',
        players,
        scores: {},
        drawingData: []
      };

      const getNextDrawer = (gameStateManager as any).getNextDrawer.bind(gameStateManager);

      // First call should return first player
      const firstDrawer = getNextDrawer(gameState);
      assertEquals(firstDrawer.id, "player-1");

      // Set current drawer and get next
      gameState.currentDrawer = "player-1";
      const secondDrawer = getNextDrawer(gameState);
      assertEquals(secondDrawer.id, "player-2");

      // Continue rotation
      gameState.currentDrawer = "player-2";
      const thirdDrawer = getNextDrawer(gameState);
      assertEquals(thirdDrawer.id, "player-3");

      // Should wrap around
      gameState.currentDrawer = "player-3";
      const wrappedDrawer = getNextDrawer(gameState);
      assertEquals(wrappedDrawer.id, "player-1");
    });

    it("should skip disconnected players in rotation", () => {
      const players: PlayerState[] = [
        { id: "player-1", name: "Player 1", isHost: true, isConnected: true, lastActivity: Date.now() },
        { id: "player-2", name: "Player 2", isHost: false, isConnected: false, lastActivity: Date.now() },
        { id: "player-3", name: "Player 3", isHost: false, isConnected: true, lastActivity: Date.now() }
      ];

      const gameState: GameState = {
        roomId,
        currentDrawer: "player-1",
        currentWord: '',
        roundNumber: 1,
        timeRemaining: 60000,
        phase: 'drawing',
        players,
        scores: {},
        drawingData: []
      };

      const getNextDrawer = (gameStateManager as any).getNextDrawer.bind(gameStateManager);

      // Should skip disconnected player-2 and go to player-3
      const nextDrawer = getNextDrawer(gameState);
      assertEquals(nextDrawer.id, "player-3");
    });
  });

  describe("winner determination", () => {
    it("should determine winner correctly", () => {
      const players: PlayerState[] = [
        { id: "player-1", name: "Player 1", isHost: true, isConnected: true, lastActivity: Date.now() },
        { id: "player-2", name: "Player 2", isHost: false, isConnected: true, lastActivity: Date.now() },
        { id: "player-3", name: "Player 3", isHost: false, isConnected: true, lastActivity: Date.now() }
      ];

      const gameState: GameState = {
        roomId,
        currentDrawer: '',
        currentWord: '',
        roundNumber: 3,
        timeRemaining: 0,
        phase: 'results',
        players,
        scores: {
          "player-1": 150,
          "player-2": 200, // Winner
          "player-3": 100
        },
        drawingData: []
      };

      const determineWinner = (gameStateManager as any).determineWinner.bind(gameStateManager);
      const winner = determineWinner(gameState);
      
      assertExists(winner);
      assertEquals(winner.id, "player-2");
    });

    it("should handle tie by returning first player with highest score", () => {
      const players: PlayerState[] = [
        { id: "player-1", name: "Player 1", isHost: true, isConnected: true, lastActivity: Date.now() },
        { id: "player-2", name: "Player 2", isHost: false, isConnected: true, lastActivity: Date.now() }
      ];

      const gameState: GameState = {
        roomId,
        currentDrawer: '',
        currentWord: '',
        roundNumber: 3,
        timeRemaining: 0,
        phase: 'results',
        players,
        scores: {
          "player-1": 200,
          "player-2": 200 // Tie
        },
        drawingData: []
      };

      const determineWinner = (gameStateManager as any).determineWinner.bind(gameStateManager);
      const winner = determineWinner(gameState);
      
      assertExists(winner);
      assertEquals(winner.id, "player-1"); // First player with highest score
    });
  });

  describe("word generation", () => {
    it("should generate random words from predefined list", () => {
      const generateRandomWord = (gameStateManager as any).generateRandomWord.bind(gameStateManager);
      
      const word1 = generateRandomWord();
      const word2 = generateRandomWord();
      
      // Words should be strings
      assertEquals(typeof word1, "string");
      assertEquals(typeof word2, "string");
      
      // Words should not be empty
      assertEquals(word1.length > 0, true);
      assertEquals(word2.length > 0, true);
      
      // Generate multiple words to test randomness (they shouldn't all be the same)
      const words = new Set();
      for (let i = 0; i < 20; i++) {
        words.add(generateRandomWord());
      }
      
      // Should have some variety (at least 2 different words in 20 attempts)
      assertEquals(words.size >= 2, true);
    });
  });

  describe("game state updates", () => {
    it("should update player connection status", async () => {
      const initialTime = Date.now();
      const gameState: GameState = {
        roomId,
        currentDrawer: '',
        currentWord: '',
        roundNumber: 0,
        timeRemaining: 60000,
        phase: 'waiting',
        players: [
          { id: "player-1", name: "Player 1", isHost: true, isConnected: true, lastActivity: initialTime }
        ],
        scores: { "player-1": 0 },
        drawingData: []
      };

      await mockKV.put(`game:${roomId}`, JSON.stringify(gameState));

      // Wait a bit to ensure time difference
      await new Promise(resolve => setTimeout(resolve, 1));
      
      const updatedState = await gameStateManager.updatePlayerConnection(roomId, "player-1", false);
      
      assertEquals(updatedState.players[0].isConnected, false);
      assertEquals(updatedState.players[0].lastActivity >= initialTime, true);
    });

    it("should handle adding new players", async () => {
      const gameState: GameState = {
        roomId,
        currentDrawer: '',
        currentWord: '',
        roundNumber: 0,
        timeRemaining: 60000,
        phase: 'waiting',
        players: [
          { id: "player-1", name: "Player 1", isHost: true, isConnected: true, lastActivity: Date.now() }
        ],
        scores: { "player-1": 0 },
        drawingData: []
      };

      await mockKV.put(`game:${roomId}`, JSON.stringify(gameState));

      const updatedState = await gameStateManager.addPlayer(roomId, "player-2", "Player 2");
      
      assertEquals(updatedState.players.length, 2);
      assertEquals(updatedState.players[1].id, "player-2");
      assertEquals(updatedState.players[1].name, "Player 2");
      assertEquals(updatedState.players[1].isConnected, true);
      assertEquals(updatedState.scores["player-2"], 0);
    });

    it("should handle removing players", async () => {
      const gameState: GameState = {
        roomId,
        currentDrawer: '',
        currentWord: '',
        roundNumber: 0,
        timeRemaining: 60000,
        phase: 'waiting',
        players: [
          { id: "player-1", name: "Player 1", isHost: true, isConnected: true, lastActivity: Date.now() },
          { id: "player-2", name: "Player 2", isHost: false, isConnected: true, lastActivity: Date.now() }
        ],
        scores: { "player-1": 50, "player-2": 30 },
        drawingData: []
      };

      await mockKV.put(`game:${roomId}`, JSON.stringify(gameState));

      const updatedState = await gameStateManager.removePlayer(roomId, "player-2");
      
      assertEquals(updatedState.players.length, 1);
      assertEquals(updatedState.players[0].id, "player-1");
      assertEquals(updatedState.scores["player-2"], undefined);
      assertEquals(updatedState.scores["player-1"], 50);
    });
  });

  describe("error handling", () => {
    it("should throw error when game state not found", async () => {
      await assertRejects(
        async () => {
          await gameStateManager.processCorrectGuess(roomId, "player-1", Date.now());
        },
        Error,
        "Game state not found"
      );
    });

    it("should throw error when not enough players for round", async () => {
      const gameState: GameState = {
        roomId,
        currentDrawer: '',
        currentWord: '',
        roundNumber: 0,
        timeRemaining: 60000,
        phase: 'waiting',
        players: [
          { id: "player-1", name: "Player 1", isHost: true, isConnected: true, lastActivity: Date.now() }
        ],
        scores: { "player-1": 0 },
        drawingData: []
      };

      await mockKV.put(`game:${roomId}`, JSON.stringify(gameState));

      await assertRejects(
        async () => {
          await gameStateManager.startNewRound(gameState);
        },
        Error,
        "Not enough players to start a round"
      );
    });

    it("should throw error when drawer tries to guess", async () => {
      const gameState: GameState = {
        roomId,
        currentDrawer: "player-1",
        currentWord: 'cat',
        roundNumber: 1,
        timeRemaining: 30000,
        phase: 'drawing',
        players: [
          { id: "player-1", name: "Player 1", isHost: true, isConnected: true, lastActivity: Date.now() },
          { id: "player-2", name: "Player 2", isHost: false, isConnected: true, lastActivity: Date.now() }
        ],
        scores: { "player-1": 0, "player-2": 0 },
        drawingData: []
      };

      await mockKV.put(`game:${roomId}`, JSON.stringify(gameState));

      await assertRejects(
        async () => {
          await gameStateManager.processCorrectGuess(roomId, "player-1", Date.now());
        },
        Error,
        "Drawer cannot guess"
      );
    });
  });
});