import { assertEquals, assertExists, assertNotEquals } from "$std/testing/asserts.ts";
import { GameRegistry, GameTypeInfo } from "../game-registry.ts";
import { BaseGameEngine } from "../game-engine.ts";
import { BaseGameSettings, BaseGameState } from "../../../types/core/game.ts";
import { BaseClientMessage, BaseServerMessage } from "../../../types/core/websocket.ts";
import { PlayerState } from "../../../types/core/room.ts";

// Mock game engine for testing
class MockGameEngine extends BaseGameEngine {
  getGameType(): string {
    return "mock";
  }

  initializeGame(
    roomId: string,
    players: PlayerState[],
    settings: BaseGameSettings,
  ): BaseGameState {
    return {
      roomId,
      gameType: this.getGameType(),
      players,
      settings,
      phase: "waiting",
      roundNumber: 0,
      timeRemaining: 0,
      scores: {},
      gameData: {},
      chatMessages: [],
    };
  }

  handleClientMessage(gameState: BaseGameState, _message: BaseClientMessage): {
    updatedState: BaseGameState;
    serverMessages: BaseServerMessage[];
  } {
    return {
      updatedState: gameState,
      serverMessages: [],
    };
  }

  validateGameAction(_gameState: BaseGameState, _playerId: string, _action: any): boolean {
    return true;
  }

  calculateScore(_gameState: BaseGameState, _playerId: string, _action: any): number {
    return 0;
  }
}

// Another mock game engine for multiple registration testing
class AnotherMockGameEngine extends BaseGameEngine {
  getGameType(): string {
    return "another-mock";
  }

  initializeGame(
    roomId: string,
    players: PlayerState[],
    settings: BaseGameSettings,
  ): BaseGameState {
    return {
      roomId,
      gameType: this.getGameType(),
      players,
      settings,
      phase: "waiting",
      roundNumber: 0,
      timeRemaining: 0,
      scores: {},
      gameData: {},
      chatMessages: [],
    };
  }

  handleClientMessage(gameState: BaseGameState, _message: BaseClientMessage): {
    updatedState: BaseGameState;
    serverMessages: BaseServerMessage[];
  } {
    return {
      updatedState: gameState,
      serverMessages: [],
    };
  }

  validateGameAction(_gameState: BaseGameState, _playerId: string, _action: any): boolean {
    return false;
  }

  calculateScore(_gameState: BaseGameState, _playerId: string, _action: any): number {
    return 100;
  }
}

// Helper to create a fresh registry for each test
function createTestRegistry(): GameRegistry {
  // Reset the singleton instance for testing
  GameRegistry.__resetForTesting();
  return GameRegistry.getInstance();
}

Deno.test("GameRegistry - singleton instance", () => {
  // Reset to ensure clean state for this test
  GameRegistry.__resetForTesting();

  const registry1 = GameRegistry.getInstance();
  const registry2 = GameRegistry.getInstance();

  assertEquals(registry1, registry2, "Should return the same instance");
  assertExists(registry1);
});

Deno.test("GameRegistry - registerGame adds game type and engine factory", () => {
  const registry = createTestRegistry();

  const gameInfo: GameTypeInfo = {
    id: "mock",
    name: "Mock Game",
    description: "A mock game for testing",
    minPlayers: 2,
    maxPlayers: 8,
    defaultSettings: {
      roundTimeSeconds: 60,
      maxRounds: 3,
    },
  };

  registry.registerGame(gameInfo, () => new MockGameEngine());

  // Verify game type was registered
  const retrievedGameType = registry.getGameType("mock");
  assertExists(retrievedGameType);
  assertEquals(retrievedGameType.id, "mock");
  assertEquals(retrievedGameType.name, "Mock Game");
  assertEquals(retrievedGameType.description, "A mock game for testing");
  assertEquals(retrievedGameType.minPlayers, 2);
  assertEquals(retrievedGameType.maxPlayers, 8);

  // Verify engine can be retrieved
  const engine = registry.getGameEngine("mock");
  assertExists(engine);
  assertEquals(engine.getGameType(), "mock");
});

Deno.test("GameRegistry - getGameEngine returns null for unregistered game type", () => {
  const registry = createTestRegistry();

  const engine = registry.getGameEngine("nonexistent");
  assertEquals(engine, null);
});

Deno.test("GameRegistry - getGameType returns null for unregistered game type", () => {
  const registry = createTestRegistry();

  const gameType = registry.getGameType("nonexistent");
  assertEquals(gameType, null);
});

Deno.test("GameRegistry - isValidGameType returns true for registered game", () => {
  const registry = createTestRegistry();

  const gameInfo: GameTypeInfo = {
    id: "mock",
    name: "Mock Game",
    description: "Test game",
    minPlayers: 2,
    maxPlayers: 8,
    defaultSettings: { roundTimeSeconds: 60, maxRounds: 3 },
  };

  registry.registerGame(gameInfo, () => new MockGameEngine());

  assertEquals(registry.isValidGameType("mock"), true);
});

Deno.test("GameRegistry - isValidGameType returns false for unregistered game", () => {
  const registry = createTestRegistry();

  assertEquals(registry.isValidGameType("nonexistent"), false);
});

Deno.test("GameRegistry - getAllGameTypes returns empty array when no games registered", () => {
  const registry = createTestRegistry();

  const allTypes = registry.getAllGameTypes();
  assertEquals(allTypes.length, 0);
});

Deno.test("GameRegistry - getAllGameTypes returns all registered games", () => {
  const registry = createTestRegistry();

  const gameInfo1: GameTypeInfo = {
    id: "mock1",
    name: "Mock Game 1",
    description: "First mock game",
    minPlayers: 2,
    maxPlayers: 4,
    defaultSettings: { roundTimeSeconds: 30, maxRounds: 5 },
  };

  const gameInfo2: GameTypeInfo = {
    id: "mock2",
    name: "Mock Game 2",
    description: "Second mock game",
    minPlayers: 3,
    maxPlayers: 6,
    defaultSettings: { roundTimeSeconds: 90, maxRounds: 2 },
  };

  registry.registerGame(gameInfo1, () => new MockGameEngine());
  registry.registerGame(gameInfo2, () => new AnotherMockGameEngine());

  const allTypes = registry.getAllGameTypes();
  assertEquals(allTypes.length, 2);

  // Check that both games are present
  const ids = allTypes.map((g) => g.id);
  assertEquals(ids.includes("mock1"), true);
  assertEquals(ids.includes("mock2"), true);
});

Deno.test("GameRegistry - getDefaultSettings returns correct settings for registered game", () => {
  const registry = createTestRegistry();

  const gameInfo: GameTypeInfo = {
    id: "mock",
    name: "Mock Game",
    description: "Test game",
    minPlayers: 2,
    maxPlayers: 8,
    defaultSettings: {
      roundTimeSeconds: 120,
      maxRounds: 7,
    },
  };

  registry.registerGame(gameInfo, () => new MockGameEngine());

  const settings = registry.getDefaultSettings("mock");
  assertExists(settings);
  assertEquals(settings.roundTimeSeconds, 120);
  assertEquals(settings.maxRounds, 7);
});

Deno.test("GameRegistry - getDefaultSettings returns null for unregistered game", () => {
  const registry = createTestRegistry();

  const settings = registry.getDefaultSettings("nonexistent");
  assertEquals(settings, null);
});

Deno.test("GameRegistry - engine factory creates new instances each time", () => {
  const registry = createTestRegistry();

  const gameInfo: GameTypeInfo = {
    id: "mock",
    name: "Mock Game",
    description: "Test game",
    minPlayers: 2,
    maxPlayers: 8,
    defaultSettings: { roundTimeSeconds: 60, maxRounds: 3 },
  };

  registry.registerGame(gameInfo, () => new MockGameEngine());

  const engine1 = registry.getGameEngine("mock");
  const engine2 = registry.getGameEngine("mock");

  assertExists(engine1);
  assertExists(engine2);
  // Should be different instances
  assertNotEquals(engine1, engine2);
});

Deno.test("GameRegistry - re-registering same game type overwrites previous registration", () => {
  const registry = createTestRegistry();

  const gameInfo1: GameTypeInfo = {
    id: "mock",
    name: "Original Mock Game",
    description: "Original description",
    minPlayers: 2,
    maxPlayers: 4,
    defaultSettings: { roundTimeSeconds: 30, maxRounds: 5 },
  };

  const gameInfo2: GameTypeInfo = {
    id: "mock",
    name: "Updated Mock Game",
    description: "Updated description",
    minPlayers: 3,
    maxPlayers: 10,
    defaultSettings: { roundTimeSeconds: 90, maxRounds: 2 },
  };

  registry.registerGame(gameInfo1, () => new MockGameEngine());
  registry.registerGame(gameInfo2, () => new AnotherMockGameEngine());

  const retrievedGameType = registry.getGameType("mock");
  assertExists(retrievedGameType);
  assertEquals(retrievedGameType.name, "Updated Mock Game");
  assertEquals(retrievedGameType.description, "Updated description");
  assertEquals(retrievedGameType.minPlayers, 3);
  assertEquals(retrievedGameType.maxPlayers, 10);
  assertEquals(retrievedGameType.defaultSettings.roundTimeSeconds, 90);
  assertEquals(retrievedGameType.defaultSettings.maxRounds, 2);

  // Should only have one game type
  const allTypes = registry.getAllGameTypes();
  assertEquals(allTypes.length, 1);
});

Deno.test("GameRegistry - handles multiple game types independently", () => {
  const registry = createTestRegistry();

  const drawingInfo: GameTypeInfo = {
    id: "drawing",
    name: "Drawing Game",
    description: "Draw and guess",
    minPlayers: 2,
    maxPlayers: 8,
    defaultSettings: { roundTimeSeconds: 60, maxRounds: 3 },
  };

  const pokerInfo: GameTypeInfo = {
    id: "poker",
    name: "Poker Game",
    description: "Texas Hold'em",
    minPlayers: 2,
    maxPlayers: 6,
    defaultSettings: { roundTimeSeconds: 120, maxRounds: 10 },
  };

  registry.registerGame(drawingInfo, () => new MockGameEngine());
  registry.registerGame(pokerInfo, () => new AnotherMockGameEngine());

  // Verify both are registered independently
  assertEquals(registry.isValidGameType("drawing"), true);
  assertEquals(registry.isValidGameType("poker"), true);

  const drawingType = registry.getGameType("drawing");
  const pokerType = registry.getGameType("poker");

  assertExists(drawingType);
  assertExists(pokerType);
  assertEquals(drawingType.name, "Drawing Game");
  assertEquals(pokerType.name, "Poker Game");
});
