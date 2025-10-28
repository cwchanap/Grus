import {
  assertEquals,
  assertExists,
  assertNotEquals,
} from "$std/testing/asserts.ts";
import { CoreWebSocketHandler } from "../websocket-handler.ts";
import { GameRegistry } from "../game-registry.ts";
import { BaseGameEngine } from "../game-engine.ts";
import { BaseGameSettings, BaseGameState } from "../../../types/core/game.ts";
import {
  BaseClientMessage,
  BaseServerMessage,
} from "../../../types/core/websocket.ts";
import { PlayerState } from "../../../types/core/room.ts";

// Mock WebSocket implementation for testing
class MockWebSocket {
  readyState: number = 1; // OPEN
  sent: string[] = [];
  eventListeners: Map<string, Function[]> = new Map();

  send(data: string): void {
    this.sent.push(data);
  }

  close(): void {
    this.readyState = 3; // CLOSED
  }

  addEventListener(event: string, handler: Function): void {
    if (!this.eventListeners.has(event)) {
      this.eventListeners.set(event, []);
    }
    this.eventListeners.get(event)!.push(handler);
  }

  // Helper to simulate receiving a message
  simulateMessage(data: string): void {
    const handlers = this.eventListeners.get("message") || [];
    handlers.forEach((handler) => handler({ data }));
  }

  // Helper to simulate disconnection
  simulateClose(): void {
    this.readyState = 3;
    const handlers = this.eventListeners.get("close") || [];
    handlers.forEach((handler) => handler());
  }

  // Helper to simulate error
  simulateError(error: Error): void {
    const handlers = this.eventListeners.get("error") || [];
    handlers.forEach((handler) => handler(error));
  }
}

// Mock game engine for testing
class TestGameEngine extends BaseGameEngine {
  getGameType(): string {
    return "test-game";
  }

  initializeGame(
    roomId: string,
    players: PlayerState[],
    settings: BaseGameSettings,
  ): BaseGameState {
    return {
      roomId,
      gameType: "test-game",
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

  handleClientMessage(
    gameState: BaseGameState,
    message: BaseClientMessage,
  ): {
    updatedState: BaseGameState;
    serverMessages: BaseServerMessage[];
  } {
    // Simple test behavior - echo message back
    return {
      updatedState: { ...gameState },
      serverMessages: [{
        type: "test-response",
        roomId: gameState.roomId,
        data: { received: message.type },
      }],
    };
  }

  validateGameAction(
    _gameState: BaseGameState,
    _playerId: string,
    _action: any,
  ): boolean {
    return true;
  }

  calculateScore(
    _gameState: BaseGameState,
    _playerId: string,
    _action: any,
  ): number {
    return 10;
  }
}

/**
 * Unit tests for CoreWebSocketHandler
 */

Deno.test("CoreWebSocketHandler - constructor initializes properly", () => {
  const handler = new CoreWebSocketHandler();

  assertExists(handler);
  assertEquals(handler.getConnectionCount(), 0);
  assertEquals(handler.getActiveRoomIds().length, 0);
});

Deno.test("CoreWebSocketHandler - handleWebSocketUpgrade rejects non-websocket requests", () => {
  const handler = new CoreWebSocketHandler();
  const request = new Request("http://localhost:3000/ws", {
    headers: { "Upgrade": "http/2" },
  });

  const response = handler.handleWebSocketUpgrade(request);

  assertEquals(response.status, 426);
});

Deno.test("CoreWebSocketHandler - getConnectionCount returns correct count", () => {
  const handler = new CoreWebSocketHandler();

  assertEquals(handler.getConnectionCount(), 0);
});

Deno.test("CoreWebSocketHandler - getRoomConnectionCount returns 0 for non-existent room", () => {
  const handler = new CoreWebSocketHandler();

  assertEquals(handler.getRoomConnectionCount("non-existent-room"), 0);
});

Deno.test("CoreWebSocketHandler - getActiveRoomIds returns empty array initially", () => {
  const handler = new CoreWebSocketHandler();

  const roomIds = handler.getActiveRoomIds();
  assertEquals(roomIds.length, 0);
});

Deno.test("CoreWebSocketHandler - cleanup clears all data", () => {
  const handler = new CoreWebSocketHandler();

  handler.cleanup();

  assertEquals(handler.getConnectionCount(), 0);
  assertEquals(handler.getActiveRoomIds().length, 0);
});

Deno.test("CoreWebSocketHandler - getGamePhase returns null for non-existent game", () => {
  const handler = new CoreWebSocketHandler();

  const phase = handler.getGamePhase("non-existent-room");
  assertEquals(phase, null);
});

Deno.test("CoreWebSocketHandler - canJoinDuringGameState returns true when no game exists", () => {
  const handler = new CoreWebSocketHandler();

  const canJoin = handler.canJoinDuringGameState("non-existent-room");
  assertEquals(canJoin, true);
});

Deno.test("CoreWebSocketHandler - canJoinDuringGameState returns true for waiting phase", () => {
  const handler = new CoreWebSocketHandler();

  // Manually set a game state in waiting phase
  const gameState: BaseGameState = {
    roomId: "test-room",
    players: [],
    settings: { roundTimeSeconds: 60, maxRounds: 3 },
    phase: "waiting",
    roundNumber: 0,
    timeRemaining: 0,
    gameType: "test-game",
    scores: {},
    gameData: {},
    chatMessages: [],
  };

  // Access private gameStates map (testing internals)
  handler.__testSetGameState("test-room", gameState);

  const canJoin = handler.canJoinDuringGameState("test-room");
  assertEquals(canJoin, true);
});

Deno.test("CoreWebSocketHandler - canJoinDuringGameState returns false for playing phase", () => {
  const handler = new CoreWebSocketHandler();

  const gameState: BaseGameState = {
    roomId: "test-room",
    players: [],
    settings: { roundTimeSeconds: 60, maxRounds: 3 },
    phase: "playing",
    roundNumber: 1,
    timeRemaining: 60000,
    gameType: "test-game",
    scores: {},
    gameData: {},
    chatMessages: [],
  };

  handler.__testSetGameState("test-room", gameState);

  const canJoin = handler.canJoinDuringGameState("test-room");
  assertEquals(canJoin, false);
});

Deno.test("CoreWebSocketHandler - canJoinDuringGameState returns true for finished phase", () => {
  const handler = new CoreWebSocketHandler();

  const gameState: BaseGameState = {
    roomId: "test-room",
    players: [],
    settings: { roundTimeSeconds: 60, maxRounds: 3 },
    phase: "finished",
    roundNumber: 3,
    timeRemaining: 0,
    gameType: "test-game",
    scores: {},
    gameData: {},
    chatMessages: [],
  };

  handler.__testSetGameState("test-room", gameState);

  const canJoin = handler.canJoinDuringGameState("test-room");
  assertEquals(canJoin, true);
});

Deno.test("CoreWebSocketHandler - sendMessage sends JSON to open websocket", () => {
  const handler = new CoreWebSocketHandler();
  const mockWs = new MockWebSocket();

  const connection = {
    ws: mockWs as unknown as WebSocket,
    playerId: "player1",
    roomId: "room1",
    lastActivity: Date.now(),
  };

  const message: BaseServerMessage = {
    type: "test",
    roomId: "room1",
    data: { foo: "bar" },
  };

  handler.__testSendMessage(connection, message);

  assertEquals(mockWs.sent.length, 1);
  const sentData = JSON.parse(mockWs.sent[0]);
  assertEquals(sentData.type, "test");
  assertEquals(sentData.roomId, "room1");
  assertEquals(sentData.data.foo, "bar");
});

Deno.test("CoreWebSocketHandler - sendMessage doesn't send to closed websocket", () => {
  const handler = new CoreWebSocketHandler();
  const mockWs = new MockWebSocket();
  mockWs.readyState = 3; // CLOSED

  const connection = {
    ws: mockWs as unknown as WebSocket,
    playerId: "player1",
    roomId: "room1",
    lastActivity: Date.now(),
  };

  const message: BaseServerMessage = {
    type: "test",
    roomId: "room1",
    data: {},
  };

  handler.__testSendMessage(connection, message);

  assertEquals(mockWs.sent.length, 0);
});

Deno.test("CoreWebSocketHandler - sendError sends error message", () => {
  const handler = new CoreWebSocketHandler();
  const mockWs = new MockWebSocket();

  const connection = {
    ws: mockWs as unknown as WebSocket,
    playerId: "player1",
    roomId: "room1",
    lastActivity: Date.now(),
  };

  handler.__testSendError(connection, "Test error message");

  assertEquals(mockWs.sent.length, 1);
  const sentData = JSON.parse(mockWs.sent[0]);
  assertEquals(sentData.type, "error");
  assertEquals(sentData.data.error, "Test error message");
});

Deno.test("CoreWebSocketHandler - handlePing sends pong response", () => {
  const handler = new CoreWebSocketHandler();
  const mockWs = new MockWebSocket();

  const connection = {
    ws: mockWs as unknown as WebSocket,
    playerId: "player1",
    roomId: "room1",
    lastActivity: Date.now(),
  };

  const pingMessage: BaseClientMessage = {
    type: "ping",
    roomId: "room1",
    playerId: "player1",
    data: {},
  };

  handler.__testHandlePing(connection, pingMessage);

  assertEquals(mockWs.sent.length, 1);
  const sentData = JSON.parse(mockWs.sent[0]);
  assertEquals(sentData.type, "pong");
  assertExists(sentData.data.timestamp);
});

Deno.test("CoreWebSocketHandler - broadcastToRoom sends to all room connections", async () => {
  const handler = new CoreWebSocketHandler();
  const mockWs1 = new MockWebSocket();
  const mockWs2 = new MockWebSocket();

  const connection1 = {
    ws: mockWs1 as unknown as WebSocket,
    playerId: "player1",
    roomId: "room1",
    lastActivity: Date.now(),
  };

  const connection2 = {
    ws: mockWs2 as unknown as WebSocket,
    playerId: "player2",
    roomId: "room1",
    lastActivity: Date.now(),
  };

  // Manually add connections
  handler.__testSetConnection("player1", connection1);
  handler.__testSetConnection("player2", connection2);
  handler.__testSetRoomConnection("room1", new Set(["player1", "player2"]));

  const message: BaseServerMessage = {
    type: "test-broadcast",
    roomId: "room1",
    data: { message: "Hello everyone" },
  };

  await handler.__testBroadcastToRoom("room1", message);

  // Both connections should have received the message
  assertEquals(mockWs1.sent.length, 1);
  assertEquals(mockWs2.sent.length, 1);

  const sentData1 = JSON.parse(mockWs1.sent[0]);
  const sentData2 = JSON.parse(mockWs2.sent[0]);

  assertEquals(sentData1.type, "test-broadcast");
  assertEquals(sentData2.type, "test-broadcast");
  assertEquals(sentData1.data.message, "Hello everyone");
  assertEquals(sentData2.data.message, "Hello everyone");
});

Deno.test("CoreWebSocketHandler - broadcastToRoom handles missing connections gracefully", async () => {
  const handler = new CoreWebSocketHandler();

  // Set up room connections but no actual WebSocket connections
  handler.__testSetRoomConnection("room1", new Set(["player1", "player2"]));

  const message: BaseServerMessage = {
    type: "test-broadcast",
    roomId: "room1",
    data: {},
  };

  // Should not throw error
  await handler.__testBroadcastToRoom("room1", message);

  // Test passes if no error is thrown
  assertEquals(true, true);
});

Deno.test("CoreWebSocketHandler - broadcastToRoom handles non-existent room", async () => {
  const handler = new CoreWebSocketHandler();

  const message: BaseServerMessage = {
    type: "test-broadcast",
    roomId: "non-existent-room",
    data: {},
  };

  // Should not throw error
  await handler.__testBroadcastToRoom("non-existent-room", message);

  // Test passes if no error is thrown
  assertEquals(true, true);
});

Deno.test("CoreWebSocketHandler - handleGameSpecificMessage sends error when no game exists", async () => {
  const handler = new CoreWebSocketHandler();
  const mockWs = new MockWebSocket();

  const connection = {
    ws: mockWs as unknown as WebSocket,
    playerId: "player1",
    roomId: "room1",
    lastActivity: Date.now(),
  };

  const message: BaseClientMessage = {
    type: "custom-game-action",
    roomId: "room1",
    playerId: "player1",
    data: {},
  };

  await handler.__testHandleGameSpecificMessage(connection, message);

  assertEquals(mockWs.sent.length, 1);
  const sentData = JSON.parse(mockWs.sent[0]);
  assertEquals(sentData.type, "error");
  assertEquals(sentData.data.error, "No active game");
});

Deno.test("CoreWebSocketHandler - handleGameSpecificMessage delegates to game engine", async () => {
  // Reset registry for test isolation
  GameRegistry.__resetForTesting();

  // Register test game engine
  const registry = GameRegistry.getInstance();
  registry.registerGame(
    {
      id: "test-game",
      name: "Test Game",
      description: "Test",
      minPlayers: 2,
      maxPlayers: 8,
      defaultSettings: { roundTimeSeconds: 60, maxRounds: 3 },
    },
    () => new TestGameEngine(),
  );

  const handler = new CoreWebSocketHandler();
  const mockWs = new MockWebSocket();

  const connection = {
    ws: mockWs as unknown as WebSocket,
    playerId: "player1",
    roomId: "room1",
    lastActivity: Date.now(),
  };

  // Set up game state
  const gameState: BaseGameState = {
    roomId: "room1",
    players: [],
    settings: { roundTimeSeconds: 60, maxRounds: 3 },
    phase: "playing",
    roundNumber: 1,
    timeRemaining: 30000,
    gameType: "test-game",
    scores: {},
    gameData: {},
    chatMessages: [],
  };

  handler.__testSetGameState("room1", gameState);
  handler.__testSetConnection("player1", connection);
  handler.__testSetRoomConnection("room1", new Set(["player1"]));

  const message: BaseClientMessage = {
    type: "custom-game-action",
    roomId: "room1",
    playerId: "player1",
    data: { action: "test" },
  };

  await handler.__testHandleGameSpecificMessage(connection, message);

  // Should have received a response from the game engine
  assertEquals(mockWs.sent.length, 1);
  const sentData = JSON.parse(mockWs.sent[0]);
  assertEquals(sentData.type, "test-response");
});

Deno.test("CoreWebSocketHandler - multiple handlers maintain separate state", () => {
  const handler1 = new CoreWebSocketHandler();
  const handler2 = new CoreWebSocketHandler();

  // These should be different instances with separate state
  assertNotEquals(handler1, handler2);

  assertEquals(handler1.getConnectionCount(), 0);
  assertEquals(handler2.getConnectionCount(), 0);
});

Deno.test("CoreWebSocketHandler - cleanup closes all WebSocket connections", () => {
  const handler = new CoreWebSocketHandler();
  const mockWs1 = new MockWebSocket();
  const mockWs2 = new MockWebSocket();

  const connection1 = {
    ws: mockWs1 as unknown as WebSocket,
    playerId: "player1",
    roomId: "room1",
    lastActivity: Date.now(),
  };

  const connection2 = {
    ws: mockWs2 as unknown as WebSocket,
    playerId: "player2",
    roomId: "room1",
    lastActivity: Date.now(),
  };

  handler.__testSetConnection("player1", connection1);
  handler.__testSetConnection("player2", connection2);

  assertEquals(mockWs1.readyState, 1); // OPEN
  assertEquals(mockWs2.readyState, 1); // OPEN

  handler.cleanup();

  assertEquals(mockWs1.readyState, 3); // CLOSED
  assertEquals(mockWs2.readyState, 3); // CLOSED
  assertEquals(handler.getConnectionCount(), 0);
});

Deno.test("CoreWebSocketHandler - getGamePhase returns correct phase for active game", () => {
  const handler = new CoreWebSocketHandler();

  const gameState: BaseGameState = {
    roomId: "room1",
    players: [],
    settings: { roundTimeSeconds: 60, maxRounds: 3 },
    phase: "playing",
    roundNumber: 2,
    timeRemaining: 45000,
    gameType: "test-game",
    scores: {},
    gameData: {},
    chatMessages: [],
  };

  handler.__testSetGameState("room1", gameState);

  const phase = handler.getGamePhase("room1");
  assertEquals(phase, "playing");
});

Deno.test("CoreWebSocketHandler - canJoinDuringGameState returns false for results phase", () => {
  const handler = new CoreWebSocketHandler();

  const gameState: BaseGameState = {
    roomId: "room1",
    players: [],
    settings: { roundTimeSeconds: 60, maxRounds: 3 },
    phase: "results",
    roundNumber: 3,
    timeRemaining: 0,
    gameType: "test-game",
    scores: {},
    gameData: {},
    chatMessages: [],
  };

  handler.__testSetGameState("room1", gameState);

  const canJoin = handler.canJoinDuringGameState("room1");
  assertEquals(canJoin, false);
});
