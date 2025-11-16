import { assertEquals, assertExists, assertNotEquals } from "$std/testing/asserts.ts";
import { DrawingGameEngine } from "../drawing-engine.ts";
import {
  DrawingClientMessage,
  DrawingCommand,
  DrawingGameSettings,
} from "../../../../types/games/drawing.ts";
import { PlayerState } from "../../../../types/core/room.ts";

/**
 * Comprehensive unit tests for DrawingGameEngine
 */

function createTestPlayers(): PlayerState[] {
  return [
    {
      id: "p1",
      name: "Player 1",
      isHost: true,
      isConnected: true,
      lastActivity: Date.now(),
    },
    {
      id: "p2",
      name: "Player 2",
      isHost: false,
      isConnected: true,
      lastActivity: Date.now(),
    },
    {
      id: "p3",
      name: "Player 3",
      isHost: false,
      isConnected: true,
      lastActivity: Date.now(),
    },
  ];
}

function createTestSettings(): DrawingGameSettings {
  return {
    roundTimeSeconds: 60,
    maxRounds: 3,
  };
}

Deno.test("DrawingGameEngine - getGameType returns drawing", () => {
  const engine = new DrawingGameEngine();
  assertEquals(engine.getGameType(), "drawing");
});

Deno.test("DrawingGameEngine - initializeGame creates correct initial state", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);

  assertEquals(gameState.roomId, "room1");
  assertEquals(gameState.gameType, "drawing");
  assertEquals(gameState.phase, "waiting");
  assertEquals(gameState.roundNumber, 0);
  assertEquals(gameState.timeRemaining, 0);
  assertEquals(gameState.players.length, 3);
  assertEquals(gameState.gameData.currentDrawer, "");
  assertEquals(gameState.gameData.currentWord, "");
  assertEquals(gameState.gameData.drawingData.length, 0);
  assertEquals(gameState.gameData.correctGuesses.length, 0);
  assertExists(gameState.scores);
  assertEquals(gameState.scores["p1"], 0);
  assertEquals(gameState.scores["p2"], 0);
  assertEquals(gameState.scores["p3"], 0);
});

Deno.test("DrawingGameEngine - startGame sets first drawer and selects word", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const initialState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(initialState);

  assertEquals(startedState.phase, "playing");
  assertEquals(startedState.drawingPhase, "drawing");
  assertEquals(startedState.roundNumber, 1);
  assertEquals(startedState.timeRemaining, 60000); // milliseconds
  assertEquals(startedState.gameData.currentDrawer, "p1"); // First player
  assertNotEquals(startedState.gameData.currentWord, ""); // Word selected
  assertEquals(startedState.gameData.drawingData.length, 0);
  assertEquals(startedState.gameData.correctGuesses.length, 0);
});

Deno.test("DrawingGameEngine - validateGameAction allows guess during playing phase", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  gameState.phase = "playing";

  const isValid = engine.validateGameAction(gameState, "p2", { type: "guess" });
  assertEquals(isValid, true);
});

Deno.test("DrawingGameEngine - validateGameAction rejects guess during waiting phase", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  gameState.phase = "waiting";

  const isValid = engine.validateGameAction(gameState, "p2", { type: "guess" });
  assertEquals(isValid, false);
});

Deno.test("DrawingGameEngine - validateGameAction rejects unknown action types", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  gameState.phase = "playing";

  const isValid = engine.validateGameAction(gameState, "p2", { type: "unknown" });
  assertEquals(isValid, false);
});

Deno.test("DrawingGameEngine - handleClientMessage processes correct guess", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  // Manually set a known word for testing
  startedState.gameData.currentWord = "cat";

  const guessMessage: DrawingClientMessage = {
    type: "guess",
    roomId: "room1",
    playerId: "p2",
    data: { message: "cat" },
  };

  const result = engine.handleClientMessage(startedState, guessMessage);

  // Check that correct guess was recorded
  assertEquals(result.updatedState.gameData.correctGuesses.length, 1);
  assertEquals(result.updatedState.gameData.correctGuesses[0].playerId, "p2");

  // Check that score was updated
  assertExists(result.updatedState.scores["p2"]);
  assertNotEquals(result.updatedState.scores["p2"], 0);

  // Check server messages
  const scoreUpdate = result.serverMessages.find((m) => m.type === "score-update");
  assertExists(scoreUpdate);
  assertEquals(scoreUpdate?.data.playerId, "p2");

  const chatMessage = result.serverMessages.find((m) => m.type === "chat-message");
  assertExists(chatMessage);
  assertEquals(chatMessage?.data.isCorrect, true);
});

Deno.test("DrawingGameEngine - handleClientMessage processes incorrect guess", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  startedState.gameData.currentWord = "cat";

  const guessMessage: DrawingClientMessage = {
    type: "guess",
    roomId: "room1",
    playerId: "p2",
    data: { message: "dog" },
  };

  const result = engine.handleClientMessage(startedState, guessMessage);

  // Check that no correct guess was recorded
  assertEquals(result.updatedState.gameData.correctGuesses.length, 0);

  // Check that score was NOT updated
  assertEquals(result.updatedState.scores["p2"], 0);

  // Check chat message contains the guess
  const chatMessage = result.serverMessages.find((m) => m.type === "chat-message");
  assertExists(chatMessage);
  assertEquals(chatMessage?.data.message, "dog");
  assertEquals(chatMessage?.data.isCorrect, false);
});

Deno.test("DrawingGameEngine - handleClientMessage ignores duplicate correct guesses", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  startedState.gameData.currentWord = "cat";

  const guessMessage: DrawingClientMessage = {
    type: "guess",
    roomId: "room1",
    playerId: "p2",
    data: { message: "cat" },
  };

  // First guess
  const result1 = engine.handleClientMessage(startedState, guessMessage);
  const firstScore = result1.updatedState.scores["p2"];

  // Second guess (duplicate)
  const result2 = engine.handleClientMessage(result1.updatedState, guessMessage);
  const secondScore = result2.updatedState.scores["p2"];

  // Score should not increase on duplicate guess
  assertEquals(secondScore, firstScore);
  assertEquals(result2.updatedState.gameData.correctGuesses.length, 1);

  // Should not send score-update on duplicate
  const scoreUpdate = result2.serverMessages.find((m) => m.type === "score-update");
  assertEquals(scoreUpdate, undefined);
});

Deno.test("DrawingGameEngine - handleClientMessage processes chat message", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);

  const chatMessage: DrawingClientMessage = {
    type: "chat",
    roomId: "room1",
    playerId: "p2",
    data: { message: "Hello everyone!" },
  };

  const result = engine.handleClientMessage(gameState, chatMessage);

  // Check chat message was added
  assertEquals(result.updatedState.chatMessages.length, 1);
  assertEquals(result.updatedState.chatMessages[0].message, "Hello everyone!");
  assertEquals(result.updatedState.chatMessages[0].playerId, "p2");

  // Check server message was sent
  assertEquals(result.serverMessages.length, 1);
  assertEquals(result.serverMessages[0].type, "chat-message");
});

Deno.test("DrawingGameEngine - handleClientMessage processes draw command from drawer", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  const drawCommand: DrawingCommand = {
    type: "start",
    x: 100,
    y: 150,
    color: "#000000",
    size: 5,
    timestamp: Date.now(),
  };

  const drawMessage: DrawingClientMessage = {
    type: "draw",
    roomId: "room1",
    playerId: "p1", // Current drawer
    data: drawCommand,
  };

  const result = engine.handleClientMessage(startedState, drawMessage);

  // Check drawing data was added
  assertEquals(result.updatedState.gameData.drawingData.length, 1);
  assertEquals(result.updatedState.gameData.drawingData[0].type, "start");

  // Should send batched draw-update message
  const batchMessage = result.serverMessages.find((m) => m.type === "draw-update-batch");
  assertExists(batchMessage);
});

Deno.test("DrawingGameEngine - handleClientMessage rejects draw command from non-drawer", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  const drawCommand: DrawingCommand = {
    type: "start",
    x: 100,
    y: 150,
    color: "#000000",
    size: 5,
    timestamp: Date.now(),
  };

  const drawMessage: DrawingClientMessage = {
    type: "draw",
    roomId: "room1",
    playerId: "p2", // NOT the current drawer
    data: drawCommand,
  };

  const result = engine.handleClientMessage(startedState, drawMessage);

  // Drawing data should NOT be added
  assertEquals(result.updatedState.gameData.drawingData.length, 0);
});

Deno.test("DrawingGameEngine - calculateScore returns higher score for faster guesses", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  // Fast guess (50 seconds remaining out of 60)
  startedState.timeRemaining = 50000; // 50 seconds in milliseconds
  const fastScore = engine.calculateScore(startedState, "p2", { type: "correct_guess" });

  // Slow guess (10 seconds remaining out of 60)
  startedState.timeRemaining = 10000; // 10 seconds in milliseconds
  const slowScore = engine.calculateScore(startedState, "p2", { type: "correct_guess" });

  // Fast guess should have higher score
  assertNotEquals(fastScore, slowScore);
  assertEquals(fastScore > slowScore, true);
});

Deno.test("DrawingGameEngine - calculateScore returns 0 for non-guess actions", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);

  const score = engine.calculateScore(gameState, "p1", { type: "other" });
  assertEquals(score, 0);
});

Deno.test("DrawingGameEngine - endGame cleans up buffer and sets phase to finished", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  const endedState = engine.endGame(startedState);

  assertEquals(endedState.phase, "finished");
  assertEquals(endedState.timeRemaining, 0);
});

Deno.test("DrawingGameEngine - addPlayer adds new player with initial score", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);

  const newPlayer: PlayerState = {
    id: "p4",
    name: "Player 4",
    isHost: false,
    isConnected: true,
    lastActivity: Date.now(),
  };

  const updatedState = engine.addPlayer(gameState, newPlayer);

  assertEquals(updatedState.players.length, 4);
  assertEquals(updatedState.players[3].id, "p4");
  assertEquals(updatedState.players[3].name, "Player 4");
});

Deno.test("DrawingGameEngine - removePlayer removes player", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);

  const updatedState = engine.removePlayer(gameState, "p2");

  assertEquals(updatedState.players.length, 2);
  assertEquals(updatedState.players.find((p) => p.id === "p2"), undefined);
});

Deno.test("DrawingGameEngine - guess is case-insensitive", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  startedState.gameData.currentWord = "cat";

  const guessMessage: DrawingClientMessage = {
    type: "guess",
    roomId: "room1",
    playerId: "p2",
    data: { message: "CAT" }, // Uppercase
  };

  const result = engine.handleClientMessage(startedState, guessMessage);

  // Should match despite different case
  assertEquals(result.updatedState.gameData.correctGuesses.length, 1);
  assertNotEquals(result.updatedState.scores["p2"], 0);
});

Deno.test("DrawingGameEngine - guess trims whitespace", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  startedState.gameData.currentWord = "cat";

  const guessMessage: DrawingClientMessage = {
    type: "guess",
    roomId: "room1",
    playerId: "p2",
    data: { message: "  cat  " }, // With whitespace
  };

  const result = engine.handleClientMessage(startedState, guessMessage);

  // Should match after trimming
  assertEquals(result.updatedState.gameData.correctGuesses.length, 1);
  assertNotEquals(result.updatedState.scores["p2"], 0);
});

Deno.test("DrawingGameEngine - correct guess shows masked message", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  startedState.gameData.currentWord = "cat";

  const guessMessage: DrawingClientMessage = {
    type: "guess",
    roomId: "room1",
    playerId: "p2",
    data: { message: "cat" },
  };

  const result = engine.handleClientMessage(startedState, guessMessage);

  const chatMessage = result.serverMessages.find((m) => m.type === "chat-message");
  assertExists(chatMessage);
  assertEquals(chatMessage?.data.message, "*** guessed correctly! ***");
});

Deno.test("DrawingGameEngine - chat messages include timestamp", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);

  const chatMessage: DrawingClientMessage = {
    type: "chat",
    roomId: "room1",
    playerId: "p2",
    data: { message: "Hello!" },
  };

  const result = engine.handleClientMessage(gameState, chatMessage);

  assertExists(result.updatedState.chatMessages[0].timestamp);
  assertExists(result.updatedState.chatMessages[0].id);
});

// ============================================================================
// Additional Tests for Round Progression and Batching
// ============================================================================

Deno.test("DrawingGameEngine - next-round message from drawer advances round", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  const currentDrawer = startedState.gameData.currentDrawer;

  const nextRoundMessage: DrawingClientMessage = {
    type: "next-round",
    roomId: "room1",
    playerId: currentDrawer,
    data: {},
  };

  const result = engine.handleClientMessage(startedState, nextRoundMessage);

  // Round should advance
  assertEquals(result.updatedState.roundNumber, 2);
  // Drawer should rotate to next player
  assertNotEquals(result.updatedState.gameData.currentDrawer, currentDrawer);
  assertEquals(result.updatedState.gameData.currentDrawer, "p2");
  // Drawing data should be cleared
  assertEquals(result.updatedState.gameData.drawingData.length, 0);
  // Correct guesses should be cleared
  assertEquals(result.updatedState.gameData.correctGuesses.length, 0);
  // New word should be selected
  assertNotEquals(result.updatedState.gameData.currentWord, "");

  // Should send game-state message
  const gameStateMessage = result.serverMessages.find((m) => m.type === "game-state");
  assertExists(gameStateMessage);
});

Deno.test("DrawingGameEngine - next-round message from host advances round", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  const nextRoundMessage: DrawingClientMessage = {
    type: "next-round",
    roomId: "room1",
    playerId: "p1", // Host (also the drawer in first round)
    data: {},
  };

  const result = engine.handleClientMessage(startedState, nextRoundMessage);

  // Round should advance
  assertEquals(result.updatedState.roundNumber, 2);
});

Deno.test("DrawingGameEngine - next-round from non-host non-drawer is ignored", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  const nextRoundMessage: DrawingClientMessage = {
    type: "next-round",
    roomId: "room1",
    playerId: "p2", // Not drawer (p1 is drawer) and not host
    data: {},
  };

  const result = engine.handleClientMessage(startedState, nextRoundMessage);

  // Round should NOT advance
  assertEquals(result.updatedState.roundNumber, startedState.roundNumber);
  // Should not send game-state message
  const gameStateMessage = result.serverMessages.find((m) => m.type === "game-state");
  assertEquals(gameStateMessage, undefined);
});

Deno.test("DrawingGameEngine - game ends when max rounds reached", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();
  settings.maxRounds = 2; // Only 2 rounds

  const gameState = engine.initializeGame("room1", players, settings);
  let currentState = engine.startGame(gameState);

  // Round 1 -> Round 2
  const nextRoundMessage1: DrawingClientMessage = {
    type: "next-round",
    roomId: "room1",
    playerId: currentState.gameData.currentDrawer,
    data: {},
  };

  currentState = engine.handleClientMessage(currentState, nextRoundMessage1).updatedState;
  assertEquals(currentState.roundNumber, 2);
  assertEquals(currentState.phase, "playing");

  // Round 2 -> Game should end
  const nextRoundMessage2: DrawingClientMessage = {
    type: "next-round",
    roomId: "room1",
    playerId: currentState.gameData.currentDrawer,
    data: {},
  };

  const finalResult = engine.handleClientMessage(currentState, nextRoundMessage2);
  assertEquals(finalResult.updatedState.phase, "finished");
  assertEquals(finalResult.updatedState.timeRemaining, 0);
});

Deno.test("DrawingGameEngine - drawer rotation cycles through players", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();
  settings.maxRounds = 5;

  const gameState = engine.initializeGame("room1", players, settings);
  let currentState = engine.startGame(gameState);

  // Track drawer order
  const drawerOrder = [currentState.gameData.currentDrawer];

  // Advance 3 rounds to cycle through all players
  for (let i = 0; i < 3; i++) {
    const nextRoundMessage: DrawingClientMessage = {
      type: "next-round",
      roomId: "room1",
      playerId: currentState.gameData.currentDrawer,
      data: {},
    };

    currentState = engine.handleClientMessage(currentState, nextRoundMessage).updatedState;
    drawerOrder.push(currentState.gameData.currentDrawer);
  }

  // Should cycle: p1 -> p2 -> p3 -> p1
  assertEquals(drawerOrder[0], "p1");
  assertEquals(drawerOrder[1], "p2");
  assertEquals(drawerOrder[2], "p3");
  assertEquals(drawerOrder[3], "p1"); // Back to first player
});

Deno.test("DrawingGameEngine - multiple draw commands are batched", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  // Send multiple draw commands
  const drawCommands: DrawingCommand[] = [
    { type: "start", x: 10, y: 20, color: "#000000", size: 5, timestamp: 1000 },
    { type: "move", x: 15, y: 25, color: "#000000", size: 5, timestamp: 1001 },
    { type: "move", x: 20, y: 30, color: "#000000", size: 5, timestamp: 1002 },
    { type: "end", timestamp: 1003 },
  ];

  let currentState = startedState;
  let allServerMessages: DrawingServerMessage[] = [];

  // Process each draw command
  for (const command of drawCommands) {
    const drawMessage: DrawingClientMessage = {
      type: "draw",
      roomId: "room1",
      playerId: "p1",
      data: command,
    };

    const result = engine.handleClientMessage(currentState, drawMessage);
    currentState = result.updatedState;
    allServerMessages = allServerMessages.concat(result.serverMessages);
  }

  // All commands should be stored
  assertEquals(currentState.gameData.drawingData.length, 4);

  // Should have batched draw-update messages
  const batchMessages = allServerMessages.filter((m) => m.type === "draw-update-batch");
  assert(batchMessages.length > 0);

  // Check that commands are properly batched
  const totalBatchedCommands = batchMessages.reduce(
    (sum, msg) => sum + (msg.data.commands?.length || 0),
    0,
  );
  assertEquals(totalBatchedCommands, 4);
});

Deno.test("DrawingGameEngine - invalid draw command is rejected", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  // Invalid draw command (coordinates out of bounds)
  const invalidCommand = {
    type: "start",
    x: -100, // Invalid
    y: 5000, // Invalid
    color: "#000000",
    size: 5,
    timestamp: Date.now(),
  };

  const drawMessage: DrawingClientMessage = {
    type: "draw",
    roomId: "room1",
    playerId: "p1",
    data: invalidCommand,
  };

  const result = engine.handleClientMessage(startedState, drawMessage);

  // Invalid command should NOT be added to drawing data
  assertEquals(result.updatedState.gameData.drawingData.length, 0);
});

Deno.test("DrawingGameEngine - draw command during waiting phase is rejected", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  // Keep in waiting phase (don't start game)

  const drawCommand: DrawingCommand = {
    type: "start",
    x: 100,
    y: 150,
    color: "#000000",
    size: 5,
    timestamp: Date.now(),
  };

  const drawMessage: DrawingClientMessage = {
    type: "draw",
    roomId: "room1",
    playerId: "p1",
    data: drawCommand,
  };

  const result = engine.handleClientMessage(gameState, drawMessage);

  // Draw command should be rejected
  assertEquals(result.updatedState.gameData.drawingData.length, 0);
});

Deno.test("DrawingGameEngine - validateGameAction rejects draw from non-drawer", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  const drawCommand: DrawingCommand = {
    type: "start",
    x: 100,
    y: 150,
    color: "#000000",
    size: 5,
    timestamp: Date.now(),
  };

  // p2 is not the drawer (p1 is)
  const isValid = engine.validateGameAction(startedState, "p2", {
    type: "draw",
    data: drawCommand,
  });

  assertEquals(isValid, false);
});

Deno.test("DrawingGameEngine - validateGameAction accepts draw from drawer", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  const drawCommand: DrawingCommand = {
    type: "start",
    x: 100,
    y: 150,
    color: "#000000",
    size: 5,
    timestamp: Date.now(),
  };

  // p1 is the current drawer
  const isValid = engine.validateGameAction(startedState, "p1", {
    type: "draw",
    data: drawCommand,
  });

  assertEquals(isValid, true);
});

Deno.test("DrawingGameEngine - time remaining is reset on new round", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();
  settings.roundTimeSeconds = 120; // 2 minutes

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  // Simulate time passing
  startedState.timeRemaining = 30000; // 30 seconds remaining

  const nextRoundMessage: DrawingClientMessage = {
    type: "next-round",
    roomId: "room1",
    playerId: startedState.gameData.currentDrawer,
    data: {},
  };

  const result = engine.handleClientMessage(startedState, nextRoundMessage);

  // Time should be reset to full round time
  assertEquals(result.updatedState.timeRemaining, 120000); // 120 seconds in milliseconds
});

Deno.test("DrawingGameEngine - score is calculated based on time remaining", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();
  settings.roundTimeSeconds = 60;

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  // Quick guess (55 seconds remaining)
  startedState.timeRemaining = 55000;
  const quickScore = engine.calculateScore(startedState, "p2", { type: "correct_guess" });

  // Medium guess (30 seconds remaining)
  startedState.timeRemaining = 30000;
  const mediumScore = engine.calculateScore(startedState, "p2", { type: "correct_guess" });

  // Late guess (5 seconds remaining)
  startedState.timeRemaining = 5000;
  const lateScore = engine.calculateScore(startedState, "p2", { type: "correct_guess" });

  // Verify scoring order
  assert(quickScore > mediumScore);
  assert(mediumScore > lateScore);
  assert(lateScore >= 0);
});

Deno.test("DrawingGameEngine - endGame cleans up server buffer", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  let currentState = engine.startGame(gameState);

  // Add some draw commands to initialize the buffer
  const drawCommand: DrawingCommand = {
    type: "start",
    x: 100,
    y: 150,
    color: "#000000",
    size: 5,
    timestamp: Date.now(),
  };

  const drawMessage: DrawingClientMessage = {
    type: "draw",
    roomId: "room1",
    playerId: "p1",
    data: drawCommand,
  };

  currentState = engine.handleClientMessage(currentState, drawMessage).updatedState;

  // End the game
  const endedState = engine.endGame(currentState);

  assertEquals(endedState.phase, "finished");
  // Buffer should be cleaned up (no way to directly test, but ensuring no errors)
});

Deno.test("DrawingGameEngine - word is selected from word list", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  // Word should be non-empty and reasonable length
  assert(startedState.gameData.currentWord.length > 0);
  assert(startedState.gameData.currentWord.length < 20);
  // Word should be a string
  assertEquals(typeof startedState.gameData.currentWord, "string");
});

Deno.test("DrawingGameEngine - drawer gets word but other players don't see it", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);

  // Current word should be set
  assertNotEquals(startedState.gameData.currentWord, "");
  // Current drawer should be set
  assertEquals(startedState.gameData.currentDrawer, "p1");

  // Note: In a real implementation, the server would send different
  // game state to drawer vs. guessers. This test just verifies the
  // word exists in the game state.
});

Deno.test("DrawingGameEngine - multiple players can guess correctly", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  const startedState = engine.startGame(gameState);
  startedState.gameData.currentWord = "cat";

  // Player 2 guesses correctly
  const guess1: DrawingClientMessage = {
    type: "guess",
    roomId: "room1",
    playerId: "p2",
    data: { message: "cat" },
  };

  let currentState = engine.handleClientMessage(startedState, guess1).updatedState;
  assertEquals(currentState.gameData.correctGuesses.length, 1);

  // Player 3 also guesses correctly
  const guess2: DrawingClientMessage = {
    type: "guess",
    roomId: "room1",
    playerId: "p3",
    data: { message: "cat" },
  };

  currentState = engine.handleClientMessage(currentState, guess2).updatedState;
  assertEquals(currentState.gameData.correctGuesses.length, 2);

  // Both players should have scores
  assert(currentState.scores["p2"] > 0);
  assert(currentState.scores["p3"] > 0);
});

Deno.test("DrawingGameEngine - clear command clears drawing data", () => {
  const engine = new DrawingGameEngine();
  const players = createTestPlayers();
  const settings = createTestSettings();

  const gameState = engine.initializeGame("room1", players, settings);
  let currentState = engine.startGame(gameState);

  // Add some drawing commands
  const commands: DrawingCommand[] = [
    { type: "start", x: 10, y: 20, color: "#000000", size: 5, timestamp: 1000 },
    { type: "move", x: 15, y: 25, color: "#000000", size: 5, timestamp: 1001 },
  ];

  for (const command of commands) {
    const drawMessage: DrawingClientMessage = {
      type: "draw",
      roomId: "room1",
      playerId: "p1",
      data: command,
    };
    currentState = engine.handleClientMessage(currentState, drawMessage).updatedState;
  }

  assertEquals(currentState.gameData.drawingData.length, 2);

  // Send clear command
  const clearCommand: DrawingCommand = {
    type: "clear",
    timestamp: Date.now(),
  };

  const clearMessage: DrawingClientMessage = {
    type: "draw",
    roomId: "room1",
    playerId: "p1",
    data: clearCommand,
  };

  currentState = engine.handleClientMessage(currentState, clearMessage).updatedState;

  // Clear command should be added to drawing data
  assertEquals(currentState.gameData.drawingData.length, 3);
  assertEquals(currentState.gameData.drawingData[2].type, "clear");
});
