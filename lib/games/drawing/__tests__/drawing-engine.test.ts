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
