import { assertEquals, assertExists } from "$std/testing/asserts.ts";
import { PokerGameEngine } from "../poker-engine.ts";
import {
  BettingRound,
  PokerAction,
  PokerClientMessage,
  PokerGameSettings,
  PokerGameState,
} from "../../../../types/games/poker.ts";
import { PlayerState } from "../../../../types/core/room.ts";

/**
 * Comprehensive tests for PokerGameEngine betting functionality
 */

function createTestGameState(
  roundNumber: number = 0,
  phase: "waiting" | "playing" | "results" | "finished" = "waiting",
): PokerGameState {
  const settings: PokerGameSettings = {
    roundTimeSeconds: 60,
    buyIn: 1000,
    smallBlind: 10,
    bigBlind: 20,
  };

  const players: PlayerState[] = [
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

  const engine = new PokerGameEngine();
  const gameState = engine.initializeGame("room1", players, settings);

  return {
    ...gameState,
    roundNumber,
    phase,
  };
}

Deno.test("PokerGameEngine - initializeGame creates correct initial state", () => {
  const engine = new PokerGameEngine();

  const settings: PokerGameSettings = {
    roundTimeSeconds: 60,
    buyIn: 1500,
    smallBlind: 15,
    bigBlind: 30,
  };

  const players: PlayerState[] = [
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
  ];

  const gameState = engine.initializeGame("test-room", players, settings);

  assertEquals(gameState.roomId, "test-room");
  assertEquals(gameState.gameType, "poker");
  assertEquals(gameState.phase, "waiting");
  assertEquals(gameState.players.length, 2);
  assertEquals(gameState.players[0].chips, 1500);
  assertEquals(gameState.players[1].chips, 1500);
  assertEquals(gameState.players[0].position, 0);
  assertEquals(gameState.players[1].position, 1);
  assertEquals(gameState.pot, 0);
  assertEquals(gameState.currentBet, 0);
  assertEquals(gameState.bettingRound, BettingRound.PRE_FLOP);
  assertEquals(gameState.communityCards.length, 0);
  assertEquals(gameState.roundNumber, 0);
});

Deno.test("PokerGameEngine - startGame deals cards and posts blinds", () => {
  const engine = new PokerGameEngine();
  const initialState = createTestGameState();

  const startedState = engine.startGame(initialState);

  assertEquals(startedState.phase, "playing");
  assertEquals(startedState.roundNumber, 1);
  assertEquals(startedState.bettingRound, BettingRound.PRE_FLOP);

  // Check that players have cards
  assertEquals(startedState.players[0].cards.length, 2);
  assertEquals(startedState.players[1].cards.length, 2);
  assertEquals(startedState.players[2].cards.length, 2);

  // Check blinds are posted
  assertEquals(startedState.players[0].bet, 10); // Small blind
  assertEquals(startedState.players[1].bet, 20); // Big blind
  assertEquals(startedState.pot, 30); // Sum of blinds

  // Current player should be after big blind
  assertEquals(startedState.currentPlayerIndex, 2);
});

Deno.test("PokerGameEngine - validateGameAction FOLD is always valid", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState();
  gameState.currentPlayerIndex = 0;

  const isValid = engine.validateGameAction(
    gameState,
    "p1",
    { action: PokerAction.FOLD },
  );

  assertEquals(isValid, true);
});

Deno.test("PokerGameEngine - validateGameAction CHECK valid when bet equals current bet", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState();
  gameState.currentPlayerIndex = 0;
  gameState.players[0].bet = 20;
  gameState.currentBet = 20;

  const isValid = engine.validateGameAction(
    gameState,
    "p1",
    { action: PokerAction.CHECK },
  );

  assertEquals(isValid, true);
});

Deno.test("PokerGameEngine - validateGameAction CHECK invalid when bet doesn't equal current bet", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState();
  gameState.currentPlayerIndex = 0;
  gameState.players[0].bet = 10;
  gameState.currentBet = 20;

  const isValid = engine.validateGameAction(
    gameState,
    "p1",
    { action: PokerAction.CHECK },
  );

  assertEquals(isValid, false);
});

Deno.test("PokerGameEngine - validateGameAction CALL valid when player has enough chips", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState();
  gameState.currentPlayerIndex = 0;
  gameState.players[0].bet = 0;
  gameState.players[0].chips = 1000;
  gameState.currentBet = 50;

  const isValid = engine.validateGameAction(
    gameState,
    "p1",
    { action: PokerAction.CALL },
  );

  assertEquals(isValid, true);
});

Deno.test("PokerGameEngine - validateGameAction CALL invalid when player doesn't have enough chips", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState();
  gameState.currentPlayerIndex = 0;
  gameState.players[0].bet = 0;
  gameState.players[0].chips = 30;
  gameState.currentBet = 50;

  const isValid = engine.validateGameAction(
    gameState,
    "p1",
    { action: PokerAction.CALL },
  );

  assertEquals(isValid, false);
});

Deno.test("PokerGameEngine - validateGameAction RAISE valid with sufficient amount", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState();
  gameState.currentPlayerIndex = 0;
  gameState.players[0].bet = 0;
  gameState.players[0].chips = 1000;
  gameState.currentBet = 20;

  const isValid = engine.validateGameAction(
    gameState,
    "p1",
    { action: PokerAction.RAISE, amount: 40 },
  );

  assertEquals(isValid, true);
});

Deno.test("PokerGameEngine - validateGameAction RAISE invalid with insufficient amount", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState();
  gameState.currentPlayerIndex = 0;
  gameState.players[0].bet = 0;
  gameState.players[0].chips = 1000;
  gameState.currentBet = 20;

  // Raise amount too small (must be at least double current bet)
  const isValid = engine.validateGameAction(
    gameState,
    "p1",
    { action: PokerAction.RAISE, amount: 15 },
  );

  assertEquals(isValid, false);
});

Deno.test("PokerGameEngine - validateGameAction ALL_IN is always valid", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState();
  gameState.currentPlayerIndex = 0;

  const isValid = engine.validateGameAction(
    gameState,
    "p1",
    { action: PokerAction.ALL_IN },
  );

  assertEquals(isValid, true);
});

Deno.test("PokerGameEngine - validateGameAction invalid when not player's turn", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState();
  gameState.currentPlayerIndex = 0; // Player 1's turn

  // Player 2 tries to act
  const isValid = engine.validateGameAction(
    gameState,
    "p2",
    { action: PokerAction.FOLD },
  );

  assertEquals(isValid, false);
});

Deno.test("PokerGameEngine - handleClientMessage FOLD marks player as folded", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState(1, "playing");
  gameState.currentPlayerIndex = 0;

  const message: PokerClientMessage = {
    type: "poker-action",
    roomId: "room1",
    playerId: "p1",
    data: { action: PokerAction.FOLD },
  };

  const result = engine.handleClientMessage(gameState, message);

  assertEquals(result.updatedState.players[0].isFolded, true);
  assertEquals(result.updatedState.players[0].hasActed, true);
  assertEquals(result.serverMessages.length, 1);
  assertEquals(result.serverMessages[0].type, "game-state");
});

Deno.test("PokerGameEngine - handleClientMessage CALL updates chips and pot", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState(1, "playing");
  gameState.currentPlayerIndex = 0;
  gameState.players[0].chips = 1000;
  gameState.players[0].bet = 0;
  gameState.currentBet = 50;
  gameState.pot = 50;

  const message: PokerClientMessage = {
    type: "poker-action",
    roomId: "room1",
    playerId: "p1",
    data: { action: PokerAction.CALL },
  };

  const result = engine.handleClientMessage(gameState, message);

  assertEquals(result.updatedState.players[0].chips, 950); // 1000 - 50
  assertEquals(result.updatedState.players[0].bet, 50);
  assertEquals(result.updatedState.pot, 100); // 50 + 50
});

Deno.test("PokerGameEngine - handleClientMessage RAISE updates chips, pot, and current bet", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState(1, "playing");
  gameState.currentPlayerIndex = 0;
  gameState.players[0].chips = 1000;
  gameState.players[0].bet = 0;
  gameState.currentBet = 20;
  gameState.pot = 20;

  const message: PokerClientMessage = {
    type: "poker-action",
    roomId: "room1",
    playerId: "p1",
    data: { action: PokerAction.RAISE, amount: 100 },
  };

  const result = engine.handleClientMessage(gameState, message);

  assertEquals(result.updatedState.players[0].chips, 900); // 1000 - 100
  assertEquals(result.updatedState.players[0].bet, 100);
  assertEquals(result.updatedState.pot, 120); // 20 + 100
  assertEquals(result.updatedState.currentBet, 100);
});

Deno.test("PokerGameEngine - handleClientMessage ALL_IN sets player all-in", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState(1, "playing");
  gameState.currentPlayerIndex = 0;
  gameState.players[0].chips = 500;
  gameState.players[0].bet = 0;
  gameState.currentBet = 20;
  gameState.pot = 20;

  const message: PokerClientMessage = {
    type: "poker-action",
    roomId: "room1",
    playerId: "p1",
    data: { action: PokerAction.ALL_IN },
  };

  const result = engine.handleClientMessage(gameState, message);

  assertEquals(result.updatedState.players[0].chips, 0);
  assertEquals(result.updatedState.players[0].bet, 500);
  assertEquals(result.updatedState.players[0].isAllIn, true);
  assertEquals(result.updatedState.pot, 520); // 20 + 500
});

Deno.test("PokerGameEngine - handleClientMessage invalid action returns unchanged state", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState(1, "playing");
  gameState.currentPlayerIndex = 1; // Player 2's turn

  // Player 1 tries to act when it's not their turn
  const message: PokerClientMessage = {
    type: "poker-action",
    roomId: "room1",
    playerId: "p1",
    data: { action: PokerAction.FOLD },
  };

  const result = engine.handleClientMessage(gameState, message);

  // State should be unchanged
  assertEquals(result.updatedState, gameState);
  assertEquals(result.serverMessages.length, 0);
});

Deno.test("PokerGameEngine - getGameType returns poker", () => {
  const engine = new PokerGameEngine();
  assertEquals(engine.getGameType(), "poker");
});

Deno.test("PokerGameEngine - calculateScore returns 0", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState();
  const score = engine.calculateScore(gameState, "p1", {});
  assertEquals(score, 0);
});

Deno.test("PokerGameEngine - endGame sets phase to finished", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState(1, "playing");

  const endedState = engine.endGame(gameState);

  assertEquals(endedState.phase, "finished");
  assertEquals(endedState.timeRemaining, 0);
});

Deno.test("PokerGameEngine - players start with correct chip counts", () => {
  const engine = new PokerGameEngine();

  const settings: PokerGameSettings = {
    roundTimeSeconds: 60,
    buyIn: 2000,
    smallBlind: 25,
    bigBlind: 50,
  };

  const players: PlayerState[] = [
    {
      id: "p1",
      name: "Player 1",
      isHost: true,
      isConnected: true,
      lastActivity: Date.now(),
    },
  ];

  const gameState = engine.initializeGame("room1", players, settings);

  assertEquals(gameState.players[0].chips, 2000);
});

Deno.test("PokerGameEngine - all players start with no bets", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState();

  gameState.players.forEach((player) => {
    assertEquals(player.bet, 0);
    assertEquals(player.hasActed, false);
    assertEquals(player.isFolded, false);
    assertEquals(player.isAllIn, false);
  });
});

Deno.test("PokerGameEngine - community cards start empty", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState();

  assertEquals(gameState.communityCards.length, 0);
});

Deno.test("PokerGameEngine - initial betting round is PRE_FLOP", () => {
  const engine = new PokerGameEngine();
  const gameState = createTestGameState();

  assertEquals(gameState.bettingRound, BettingRound.PRE_FLOP);
});
