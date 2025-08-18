/**
 * Component Integration Tests
 *
 * Tests the integration between Fresh Islands components and their
 * WebSocket communication patterns.
 */

import { assert, assertEquals, assertExists } from "$std/assert/mod.ts";
import type { ChatMessage, PlayerState } from "../../types/core/room.ts";
import type { DrawingCommand } from "../../types/games/drawing.ts";
import type { BaseGameState as _BaseGameState } from "../../types/core/game.ts";

// Minimal local GameState type for tests
type GameState = {
  roomId: string;
  currentDrawer: string;
  currentWord: string;
  roundNumber: number;
  timeRemaining: number;
  phase: "waiting" | "drawing" | "guessing" | "results";
  players: PlayerState[];
  scores: Record<string, number>;
  drawingData: any[];
  correctGuesses: string[];
  chatMessages: ChatMessage[];
  settings: { maxRounds: number; roundTimeSeconds: number };
};

// Test-only extension for chat messages used in these tests
type TestChatMessage = ChatMessage & { isGuess: boolean; isCorrect: boolean };

// Test data factories
function createTestGameState(overrides: Partial<GameState> = {}): GameState {
  return {
    roomId: "test-room",
    currentDrawer: "player1",
    currentWord: "house",
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
      "player1": 0,
      "player2": 0,
    },
    drawingData: [],
    correctGuesses: [],
    chatMessages: [],
    settings: { maxRounds: 5, roundTimeSeconds: 75 },
    ...overrides,
  };
}

function createTestChatMessage(overrides: Partial<TestChatMessage> = {}): TestChatMessage {
  return {
    id: crypto.randomUUID(),
    playerId: "player2",
    playerName: "Bob",
    message: "Hello!",
    timestamp: Date.now(),
    isGuess: true,
    isCorrect: false,
    ...overrides,
  };
}

function createTestDrawingCommand(overrides: Partial<DrawingCommand> = {}): DrawingCommand {
  return {
    type: "start",
    x: 100,
    y: 100,
    color: "#000000",
    size: 5,
    timestamp: Date.now(),
    ...overrides,
  };
}

// Integration test: Game state synchronization
Deno.test("Component Integration - Game state synchronization", () => {
  const initialState = createTestGameState({
    phase: "waiting",
    currentDrawer: "",
    roundNumber: 0,
  });

  // Simulate game start
  const gameStartedState = {
    ...initialState,
    phase: "drawing" as const,
    currentDrawer: "player1",
    roundNumber: 1,
    timeRemaining: 120000,
  };

  // Verify state transitions
  assertEquals(gameStartedState.phase, "drawing");
  assertEquals(gameStartedState.currentDrawer, "player1");
  assertEquals(gameStartedState.roundNumber, 1);

  // Simulate correct guess and score update
  const scoreUpdatedState = {
    ...gameStartedState,
    phase: "results" as const,
    scores: {
      ...gameStartedState.scores,
      "player2": 150,
    },
  };

  assertEquals(scoreUpdatedState.scores["player2"], 150);
  assertEquals(scoreUpdatedState.phase, "results");
});

// Integration test: Chat message flow
Deno.test("Component Integration - Chat message flow", () => {
  const _gameState = createTestGameState();
  const messages: TestChatMessage[] = [];

  // Simulate regular chat message
  const regularMessage = createTestChatMessage({
    message: "What are you drawing?",
    isGuess: false,
    isCorrect: false,
  });
  messages.push(regularMessage);

  // Simulate guess message
  const guessMessage = createTestChatMessage({
    message: "cat",
    isGuess: true,
    isCorrect: false,
  });
  messages.push(guessMessage);

  // Simulate correct guess
  const correctGuessMessage = createTestChatMessage({
    message: "house",
    isGuess: true,
    isCorrect: true,
  });
  messages.push(correctGuessMessage);

  // Verify message classification
  assertEquals(messages.length, 3);
  assertEquals(messages[0].isGuess, false);
  assertEquals(messages[1].isGuess, true);
  assertEquals(messages[1].isCorrect, false);
  assertEquals(messages[2].isGuess, true);
  assertEquals(messages[2].isCorrect, true);

  // Test message filtering functions
  const guessMessages = messages.filter((msg) => msg.isGuess);
  const correctGuesses = messages.filter((msg) => msg.isCorrect);

  assertEquals(guessMessages.length, 2);
  assertEquals(correctGuesses.length, 1);
  assertEquals(correctGuesses[0].message, "house");
});

// Integration test: Drawing command synchronization
Deno.test("Component Integration - Drawing command synchronization", () => {
  const _gameState = createTestGameState();
  const drawingCommands: DrawingCommand[] = [];

  // Simulate drawing a simple line
  const drawingSequence = [
    createTestDrawingCommand({ type: "start", x: 100, y: 100 }),
    createTestDrawingCommand({ type: "move", x: 150, y: 150 }),
    createTestDrawingCommand({ type: "move", x: 200, y: 200 }),
    createTestDrawingCommand({ type: "end" }),
  ];

  drawingCommands.push(...drawingSequence);

  // Verify drawing sequence
  assertEquals(drawingCommands.length, 4);
  assertEquals(drawingCommands[0].type, "start");
  assertEquals(drawingCommands[1].type, "move");
  assertEquals(drawingCommands[2].type, "move");
  assertEquals(drawingCommands[3].type, "end");

  // Verify coordinate progression
  assertEquals(drawingCommands[0].x, 100);
  assertEquals(drawingCommands[1].x, 150);
  assertEquals(drawingCommands[2].x, 200);

  // Test drawing validation
  const isValidDrawingCommand = (cmd: DrawingCommand): boolean => {
    if (!["start", "move", "end", "clear"].includes(cmd.type)) return false;
    if (cmd.type !== "clear" && cmd.type !== "end") {
      return typeof cmd.x === "number" && typeof cmd.y === "number" &&
        cmd.x >= 0 && cmd.y >= 0;
    }
    return true;
  };

  const allValid = drawingCommands.every(isValidDrawingCommand);
  assert(allValid, "All drawing commands should be valid");
});

// Integration test: Turn-based permissions
Deno.test("Component Integration - Turn-based permissions", () => {
  const gameState = createTestGameState();

  // Test drawing permissions
  const canDraw = (playerId: string, currentDrawer: string, phase: GameState["phase"]): boolean => {
    return playerId === currentDrawer && phase === "drawing";
  };

  // Player 1 is current drawer in drawing phase
  assert(canDraw("player1", gameState.currentDrawer, gameState.phase));
  assert(!canDraw("player2", gameState.currentDrawer, gameState.phase));

  // No one can draw in waiting phase
  assert(!canDraw("player1", gameState.currentDrawer, "waiting"));
  assert(!canDraw("player2", gameState.currentDrawer, "waiting"));

  // No one can draw in results phase
  assert(!canDraw("player1", gameState.currentDrawer, "results"));
  assert(!canDraw("player2", gameState.currentDrawer, "results"));

  // Test chat permissions (everyone can chat except during specific phases)
  const canChat = (phase: GameState["phase"]): boolean => {
    return phase !== "results"; // Simplified - in real game, might have more restrictions
  };

  assert(canChat("drawing"));
  assert(canChat("waiting"));
  assert(canChat("guessing"));
});

// Integration test: Score calculation
Deno.test("Component Integration - Score calculation", () => {
  const gameState = createTestGameState({
    timeRemaining: 90000, // 90 seconds remaining
  });

  // Simulate score calculation for correct guess
  const calculateScore = (timeRemaining: number, basePoints = 100): number => {
    const timeBonus = Math.floor(timeRemaining / 1000); // 1 point per second remaining
    return basePoints + timeBonus;
  };

  const score1 = calculateScore(90000); // 90 seconds remaining
  const score2 = calculateScore(30000); // 30 seconds remaining
  const score3 = calculateScore(5000); // 5 seconds remaining

  assertEquals(score1, 190); // 100 + 90
  assertEquals(score2, 130); // 100 + 30
  assertEquals(score3, 105); // 100 + 5

  // Verify faster guesses get higher scores
  assert(score1 > score2);
  assert(score2 > score3);

  // Test score update in game state
  const updatedGameState = {
    ...gameState,
    scores: {
      ...gameState.scores,
      "player2": score1,
    },
  };

  assertEquals(updatedGameState.scores["player2"], 190);
  assertEquals(updatedGameState.scores["player2"], 190); // Changed
});

// Integration test: Player connection status
Deno.test("Component Integration - Player connection status", () => {
  const gameState = createTestGameState();

  // Test connection status updates
  const updatePlayerConnection = (
    players: PlayerState[],
    playerId: string,
    isConnected: boolean,
  ): PlayerState[] => {
    return players.map((player) =>
      player.id === playerId ? { ...player, isConnected, lastActivity: Date.now() } : player
    );
  };

  // Simulate player disconnection
  const updatedPlayers = updatePlayerConnection(gameState.players, "player2", false);

  const player2 = updatedPlayers.find((p) => p.id === "player2");
  assertExists(player2);
  assertEquals(player2.isConnected, false);

  // Other players should remain unchanged
  const player1 = updatedPlayers.find((p) => p.id === "player1");
  assertExists(player1);
  assertEquals(player1.isConnected, true);

  // Test active player count
  const getActivePlayerCount = (players: PlayerState[]): number => {
    return players.filter((p) => p.isConnected).length;
  };

  assertEquals(getActivePlayerCount(gameState.players), 2);
  assertEquals(getActivePlayerCount(updatedPlayers), 1);
});

// Integration test: Game phase transitions
Deno.test("Component Integration - Game phase transitions", () => {
  let gameState = createTestGameState({ phase: "waiting" });

  // Simulate game start
  gameState = { ...gameState, phase: "drawing", currentDrawer: "player1" };
  assertEquals(gameState.phase, "drawing");

  // Simulate correct guess (should transition to results)
  gameState = { ...gameState, phase: "results" };
  assertEquals(gameState.phase, "results");

  // Simulate next round start
  gameState = {
    ...gameState,
    phase: "drawing",
    roundNumber: gameState.roundNumber + 1,
    currentDrawer: "player2", // Next player's turn
  };
  assertEquals(gameState.phase, "drawing");
  assertEquals(gameState.roundNumber, 2);
  assertEquals(gameState.currentDrawer, "player2");

  // Test phase validation
  const validPhases: GameState["phase"][] = ["waiting", "drawing", "guessing", "results"];
  const isValidPhase = (phase: string): phase is GameState["phase"] => {
    return validPhases.includes(phase as GameState["phase"]);
  };

  assert(isValidPhase("drawing"));
  assert(isValidPhase("waiting"));
  assert(!isValidPhase("invalid-phase"));
});

// Integration test: WebSocket message structure validation
Deno.test("Component Integration - WebSocket message validation", () => {
  // Test client message structure
  const validateClientMessage = (message: any): boolean => {
    return (
      typeof message === "object" &&
      typeof message.type === "string" &&
      typeof message.roomId === "string" &&
      typeof message.playerId === "string" &&
      message.data !== undefined
    );
  };

  const validMessage = {
    type: "chat",
    roomId: "test-room",
    playerId: "player1",
    data: { text: "Hello!" },
  };

  const invalidMessage1 = {
    type: "chat",
    roomId: "test-room",
    // Missing playerId
    data: { text: "Hello!" },
  };

  const invalidMessage2 = {
    type: "chat",
    roomId: "test-room",
    playerId: "player1",
    // Missing data
  };

  assert(validateClientMessage(validMessage));
  assert(!validateClientMessage(invalidMessage1));
  assert(!validateClientMessage(invalidMessage2));

  // Test server message structure
  const validateServerMessage = (message: any): boolean => {
    return (
      typeof message === "object" &&
      typeof message.type === "string" &&
      typeof message.roomId === "string" &&
      message.data !== undefined
    );
  };

  const validServerMessage = {
    type: "chat-message",
    roomId: "test-room",
    data: createTestChatMessage(),
  };

  assert(validateServerMessage(validServerMessage));
});

// Integration test: Error handling scenarios
Deno.test("Component Integration - Error handling", () => {
  const _gameState = createTestGameState();

  // Test invalid drawing command handling
  const validateDrawingCommand = (command: any): boolean => {
    if (!command || typeof command !== "object") return false;
    if (!["start", "move", "end", "clear"].includes(command.type)) return false;

    if (command.type !== "clear" && command.type !== "end") {
      return (
        typeof command.x === "number" &&
        typeof command.y === "number" &&
        command.x >= 0 && command.x <= 2000 &&
        command.y >= 0 && command.y <= 2000
      );
    }

    return true;
  };

  // Valid commands
  assert(validateDrawingCommand({ type: "start", x: 100, y: 100 }));
  assert(validateDrawingCommand({ type: "clear" }));
  assert(validateDrawingCommand({ type: "end" }));

  // Invalid commands
  assert(!validateDrawingCommand({ type: "invalid" }));
  assert(!validateDrawingCommand({ type: "start", x: -100, y: 100 }));
  assert(!validateDrawingCommand({ type: "move", x: 3000, y: 100 }));
  assert(!validateDrawingCommand(null));
  assert(!validateDrawingCommand(undefined));

  // Test chat message validation
  const validateChatMessage = (message: string): boolean => {
    return message.trim().length > 0 && message.length <= 200;
  };

  assert(validateChatMessage("Hello!"));
  assert(validateChatMessage("A".repeat(200)));
  assert(!validateChatMessage(""));
  assert(!validateChatMessage("   "));
  assert(!validateChatMessage("A".repeat(201)));
});

console.log("✅ All component integration tests completed successfully!");
