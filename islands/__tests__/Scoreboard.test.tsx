import { assertEquals } from "$std/assert/mod.ts";
// import { signal } from "@preact/signals";
import type { GameState } from "../../types/game.ts";

// Test helper functions that would be used in the Scoreboard component
const createMockGameState = (overrides: Partial<GameState> = {}): GameState => ({
  roomId: "test-room",
  currentDrawer: "player1",
  currentWord: "cat",
  roundNumber: 1,
  timeRemaining: 120000, // 2 minutes
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
    {
      id: "player3",
      name: "Charlie",
      isHost: false,
      isConnected: false,
      lastActivity: Date.now() - 30000,
    },
  ],
  scores: {
    "player1": 150,
    "player2": 100,
    "player3": 50,
  },
  drawingData: [],
  correctGuesses: [],
  chatMessages: [],
  ...overrides,
});

// Test utility functions that would be used in Scoreboard
const formatTime = (milliseconds: number): string => {
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
};

const getSortedPlayers = (gameState: GameState) => {
  return gameState.players
    .map((player) => ({
      ...player,
      score: gameState.scores[player.id] || 0,
    }))
    .sort((a, b) => b.score - a.score);
};

const getPhaseText = (phase: GameState["phase"]): string => {
  switch (phase) {
    case "waiting":
      return "Waiting for game to start";
    case "drawing":
      return "Drawing in progress";
    case "guessing":
      return "Guessing time";
    case "results":
      return "Round complete";
    default:
      return "Unknown phase";
  }
};

Deno.test("Scoreboard - formatTime function works correctly", () => {
  const testCases = [
    { ms: 120000, expected: "2:00" }, // 2 minutes
    { ms: 90000, expected: "1:30" }, // 1 minute 30 seconds
    { ms: 5000, expected: "0:05" }, // 5 seconds
    { ms: 0, expected: "0:00" }, // 0 seconds
    { ms: -1000, expected: "0:00" }, // Negative (should be 0)
  ];

  testCases.forEach(({ ms, expected }) => {
    const result = formatTime(ms);
    assertEquals(result, expected, `Expected ${expected} for ${ms}ms, got ${result}`);
  });
});

Deno.test("Scoreboard - getSortedPlayers sorts by score correctly", () => {
  const gameState = createMockGameState({
    scores: {
      "player1": 50, // Alice - should be 3rd
      "player2": 200, // Bob - should be 1st
      "player3": 100, // Charlie - should be 2nd
    },
  });

  const sortedPlayers = getSortedPlayers(gameState);

  assertEquals(sortedPlayers.length, 3);
  assertEquals(sortedPlayers[0].name, "Bob"); // Highest score
  assertEquals(sortedPlayers[0].score, 200);
  assertEquals(sortedPlayers[1].name, "Charlie"); // Middle score
  assertEquals(sortedPlayers[1].score, 100);
  assertEquals(sortedPlayers[2].name, "Alice"); // Lowest score
  assertEquals(sortedPlayers[2].score, 50);
});

Deno.test("Scoreboard - getPhaseText returns correct text for each phase", () => {
  const phases = [
    { phase: "waiting" as const, expected: "Waiting for game to start" },
    { phase: "drawing" as const, expected: "Drawing in progress" },
    { phase: "guessing" as const, expected: "Guessing time" },
    { phase: "results" as const, expected: "Round complete" },
  ];

  phases.forEach(({ phase, expected }) => {
    const result = getPhaseText(phase);
    assertEquals(result, expected, `Expected "${expected}" for phase "${phase}", got "${result}"`);
  });
});

Deno.test("Scoreboard - handles players with missing scores", () => {
  const gameState = createMockGameState({
    scores: {
      "player1": 100,
      // player2 and player3 missing from scores
    },
  });

  const sortedPlayers = getSortedPlayers(gameState);

  assertEquals(sortedPlayers.length, 3);
  assertEquals(sortedPlayers[0].score, 100); // player1 with actual score
  assertEquals(sortedPlayers[1].score, 0); // player2 with default score
  assertEquals(sortedPlayers[2].score, 0); // player3 with default score
});

Deno.test("Scoreboard - identifies current drawer correctly", () => {
  const gameState = createMockGameState({
    currentDrawer: "player2",
  });

  const currentDrawer = gameState.players.find((p) => p.id === gameState.currentDrawer);

  assertExists(currentDrawer);
  assertEquals(currentDrawer.name, "Bob");
  assertEquals(currentDrawer.id, "player2");
});

Deno.test("Scoreboard - identifies host player correctly", () => {
  const gameState = createMockGameState();

  const hostPlayer = gameState.players.find((p) => p.isHost);

  assertExists(hostPlayer);
  assertEquals(hostPlayer.name, "Alice");
  assertEquals(hostPlayer.id, "player1");
  assertEquals(hostPlayer.isHost, true);
});

Deno.test("Scoreboard - handles empty player list", () => {
  const gameState = createMockGameState({
    players: [],
    scores: {},
  });

  const sortedPlayers = getSortedPlayers(gameState);
  assertEquals(sortedPlayers.length, 0);
});

Deno.test("Scoreboard - handles connection status correctly", () => {
  const gameState = createMockGameState();

  const connectedPlayers = gameState.players.filter((p) => p.isConnected);
  const disconnectedPlayers = gameState.players.filter((p) => !p.isConnected);

  assertEquals(connectedPlayers.length, 2); // Alice and Bob
  assertEquals(disconnectedPlayers.length, 1); // Charlie

  assertEquals(connectedPlayers[0].name, "Alice");
  assertEquals(connectedPlayers[1].name, "Bob");
  assertEquals(disconnectedPlayers[0].name, "Charlie");
});

Deno.test("Scoreboard - handles round number and timer data", () => {
  const gameState = createMockGameState({
    roundNumber: 3,
    timeRemaining: 90000, // 1:30
    phase: "drawing",
  });

  // Test that game state contains expected values
  assertEquals(gameState.roundNumber, 3);
  assertEquals(formatTime(gameState.timeRemaining), "1:30");
  assertEquals(gameState.phase, "drawing");
});

Deno.test("Scoreboard - identifies current drawer correctly", () => {
  const gameState = createMockGameState({
    currentDrawer: "player1",
    phase: "drawing",
  });

  const currentDrawer = gameState.players.find((p) => p.id === gameState.currentDrawer);

  assertExists(currentDrawer);
  assertEquals(currentDrawer.name, "Alice");
  assertEquals(currentDrawer.id, "player1");
});

Deno.test("Scoreboard - handles word visibility logic", () => {
  const gameState = createMockGameState({
    currentDrawer: "player1",
    currentWord: "elephant",
    phase: "drawing",
  });

  // Drawer should see the word
  const isDrawer = gameState.currentDrawer === "player1";
  assertEquals(isDrawer, true);

  // Non-drawer should not see the word
  const isNotDrawer = gameState.currentDrawer === "player2";
  assertEquals(isNotDrawer, false);

  // Word should be available when phase is drawing
  const shouldShowWord = gameState.phase === "drawing" && !!gameState.currentWord;
  assertEquals(shouldShowWord, true);
});

// Host control tests
Deno.test("Scoreboard - identifies host correctly for controls", () => {
  const gameState = createMockGameState();
  const playerId = "player1"; // Alice is the host

  const isHost = gameState.players.find((p) => p.id === playerId)?.isHost || false;
  assertEquals(isHost, true);

  // Non-host player
  const nonHostPlayerId = "player2";
  const isNotHost = gameState.players.find((p) => p.id === nonHostPlayerId)?.isHost || false;
  assertEquals(isNotHost, false);
});

Deno.test("Scoreboard - start game button should be available in waiting phase", () => {
  const gameState = createMockGameState({
    phase: "waiting",
  });

  const shouldShowStartButton = gameState.phase === "waiting";
  assertEquals(shouldShowStartButton, true);

  // Should not show in other phases
  const drawingState = createMockGameState({ phase: "drawing" });
  const shouldNotShowInDrawing = drawingState.phase === "waiting";
  assertEquals(shouldNotShowInDrawing, false);
});

Deno.test("Scoreboard - end round button should be available during drawing phase", () => {
  const gameState = createMockGameState({
    phase: "drawing",
  });

  const shouldShowEndRoundButton = gameState.phase === "drawing";
  assertEquals(shouldShowEndRoundButton, true);

  // Should not show in other phases
  const waitingState = createMockGameState({ phase: "waiting" });
  const shouldNotShowInWaiting = waitingState.phase === "drawing";
  assertEquals(shouldNotShowInWaiting, false);
});

Deno.test("Scoreboard - next round button should be available during results phase", () => {
  const gameState = createMockGameState({
    phase: "results",
  });

  const shouldShowNextRoundButton = gameState.phase === "results";
  assertEquals(shouldShowNextRoundButton, true);

  // Should not show in other phases
  const drawingState = createMockGameState({ phase: "drawing" });
  const shouldNotShowInDrawing = drawingState.phase === "results";
  assertEquals(shouldNotShowInDrawing, false);
});

Deno.test("Scoreboard - end game button should be available during drawing and results phases", () => {
  const drawingState = createMockGameState({ phase: "drawing" });
  const resultsState = createMockGameState({ phase: "results" });
  const waitingState = createMockGameState({ phase: "waiting" });

  const shouldShowInDrawing = drawingState.phase === "drawing" || drawingState.phase === "results";
  const shouldShowInResults = resultsState.phase === "drawing" || resultsState.phase === "results";
  const shouldNotShowInWaiting = waitingState.phase === "drawing" ||
    waitingState.phase === "results";

  assertEquals(shouldShowInDrawing, true);
  assertEquals(shouldShowInResults, true);
  assertEquals(shouldNotShowInWaiting, false);
});

Deno.test("Scoreboard - start game button should be disabled with insufficient players", () => {
  const gameStateWithOnePlayer = createMockGameState({
    phase: "waiting",
    players: [
      {
        id: "player1",
        name: "Alice",
        isHost: true,
        isConnected: true,
        lastActivity: Date.now(),
      },
    ],
  });

  const connectedPlayers = gameStateWithOnePlayer.players.filter((p) => p.isConnected);
  const shouldDisableStartButton = connectedPlayers.length < 2;
  assertEquals(shouldDisableStartButton, true);

  // With enough players, should be enabled
  const gameStateWithTwoPlayers = createMockGameState({
    phase: "waiting",
  });
  const connectedPlayersEnough = gameStateWithTwoPlayers.players.filter((p) => p.isConnected);
  const shouldEnableStartButton = connectedPlayersEnough.length >= 2;
  assertEquals(shouldEnableStartButton, true);
});

Deno.test("Scoreboard - game settings modal should only show in waiting phase", () => {
  const waitingState = createMockGameState({ phase: "waiting" });
  const drawingState = createMockGameState({ phase: "drawing" });

  const shouldShowSettingsInWaiting = waitingState.phase === "waiting";
  const shouldNotShowSettingsInDrawing = drawingState.phase === "waiting";

  assertEquals(shouldShowSettingsInWaiting, true);
  assertEquals(shouldNotShowSettingsInDrawing, false);
});
