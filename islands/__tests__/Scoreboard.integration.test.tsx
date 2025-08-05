import { assertEquals } from "$std/assert/mod.ts";
// import { signal } from "@preact/signals";
import type { GameState } from "../../types/game.ts";

// Test WebSocket message handling logic that would be used in Scoreboard
const createMockGameState = (overrides: Partial<GameState> = {}): GameState => ({
  roomId: "test-room",
  currentDrawer: "player1",
  currentWord: "cat",
  roundNumber: 1,
  timeRemaining: 120000,
  phase: "drawing",
  players: [
    {
      id: "player1",
      name: "Alice",
      isHost: true,
      isConnected: true,
      lastActivity: Date.now(),
    },
    {
      id: "player2",
      name: "Bob",
      isHost: false,
      isConnected: true,
      lastActivity: Date.now(),
    },
  ],
  scores: {
    "player1": 100,
    "player2": 50,
  },
  drawingData: [],
  correctGuesses: [],
  chatMessages: [],
  settings: { maxRounds: 5, roundTimeSeconds: 75 },
  ...overrides,
});

// Mock WebSocket message handler logic
const handleWebSocketMessage = (message: any, currentGameState: GameState): GameState => {
  if (message.type === "game-state") {
    return message.data;
  } else if (message.type === "score-update") {
    const { playerId, newScore } = message.data;
    return {
      ...currentGameState,
      scores: {
        ...currentGameState.scores,
        [playerId]: newScore,
      },
    };
  }
  return currentGameState;
};

// Mock WebSocket URL construction
const constructWebSocketUrl = (roomId: string, hostname: string, protocol: string): string => {
  const wsProtocol = protocol === "https:" ? "wss:" : "ws:";
  return `${wsProtocol}//${hostname}/api/websocket?roomId=${roomId}`;
};

// Mock subscription message creation
const createSubscriptionMessage = (roomId: string, playerId: string) => ({
  type: "join-room",
  roomId,
  playerId,
  data: { subscribeToGameState: true },
});

Deno.test("Scoreboard Integration - WebSocket URL construction", () => {
  const roomId = "test-room";

  // Test HTTPS protocol
  const httpsUrl = constructWebSocketUrl(roomId, "example.com", "https:");
  assertEquals(httpsUrl, "wss://example.com/api/websocket?roomId=test-room");

  // Test HTTP protocol
  const httpUrl = constructWebSocketUrl(roomId, "localhost:8000", "http:");
  assertEquals(httpUrl, "ws://localhost:8000/api/websocket?roomId=test-room");
});

Deno.test("Scoreboard Integration - subscription message creation", () => {
  const roomId = "test-room";
  const playerId = "player123";

  const message = createSubscriptionMessage(roomId, playerId);

  assertEquals(message.type, "join-room");
  assertEquals(message.roomId, roomId);
  assertEquals(message.playerId, playerId);
  assertEquals(message.data.subscribeToGameState, true);
});

Deno.test("Scoreboard Integration - handles game state update messages", () => {
  const initialGameState = createMockGameState({
    roundNumber: 1,
    currentDrawer: "player1",
  });

  const newGameState = createMockGameState({
    roundNumber: 2,
    currentDrawer: "player2",
    scores: {
      "player1": 150,
      "player2": 200,
    },
  });

  const message = {
    type: "game-state",
    data: newGameState,
  };

  const result = handleWebSocketMessage(message, initialGameState);

  assertEquals(result.roundNumber, 2);
  assertEquals(result.currentDrawer, "player2");
  assertEquals(result.scores["player2"], 200);
});

Deno.test("Scoreboard Integration - handles score update messages", () => {
  const initialGameState = createMockGameState({
    scores: {
      "player1": 100,
      "player2": 50,
    },
  });

  const message = {
    type: "score-update",
    data: {
      playerId: "player2",
      newScore: 250,
    },
  };

  const result = handleWebSocketMessage(message, initialGameState);

  assertEquals(result.scores["player1"], 100); // Unchanged
  assertEquals(result.scores["player2"], 250); // Updated
});

Deno.test("Scoreboard Integration - handles unknown message types", () => {
  const initialGameState = createMockGameState();

  const message = {
    type: "unknown-message-type",
    data: { someData: "test" },
  };

  const result = handleWebSocketMessage(message, initialGameState);

  // Should return unchanged state for unknown message types
  assertEquals(result, initialGameState);
});

Deno.test("Scoreboard Integration - handles score update for new player", () => {
  const initialGameState = createMockGameState({
    scores: {
      "player1": 100,
      "player2": 50,
    },
  });

  const message = {
    type: "score-update",
    data: {
      playerId: "player3", // New player not in initial scores
      newScore: 75,
    },
  };

  const result = handleWebSocketMessage(message, initialGameState);

  assertEquals(result.scores["player1"], 100); // Unchanged
  assertEquals(result.scores["player2"], 50); // Unchanged
  assertEquals(result.scores["player3"], 75); // New player added
});

Deno.test("Scoreboard Integration - preserves other game state during score updates", () => {
  const initialGameState = createMockGameState({
    roundNumber: 3,
    currentDrawer: "player1",
    phase: "drawing",
    timeRemaining: 90000,
    scores: {
      "player1": 100,
      "player2": 50,
    },
  });

  const message = {
    type: "score-update",
    data: {
      playerId: "player2",
      newScore: 150,
    },
  };

  const result = handleWebSocketMessage(message, initialGameState);

  // All other properties should remain unchanged
  assertEquals(result.roundNumber, 3);
  assertEquals(result.currentDrawer, "player1");
  assertEquals(result.phase, "drawing");
  assertEquals(result.timeRemaining, 90000);
  assertEquals(result.players, initialGameState.players);
  assertEquals(result.drawingData, initialGameState.drawingData);

  // Only scores should be updated
  assertEquals(result.scores["player2"], 150);
});
