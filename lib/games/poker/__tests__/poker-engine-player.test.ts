import { assertEquals } from "jsr:@std/assert";
import { PokerGameEngine } from "../poker-engine.ts";
import { PokerGameSettings } from "../../../../types/games/poker.ts";
import { PlayerState } from "../../../../types/core/room.ts";

Deno.test("PokerGameEngine - addPlayer adds new player correctly", () => {
  const engine = new PokerGameEngine();

  const settings: PokerGameSettings = {
    roundTimeSeconds: 60,
    maxRounds: 3,
    buyIn: 1000,
    smallBlind: 10,
    bigBlind: 20,
  };

  const initialPlayers: PlayerState[] = [
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

  // Initialize game
  const gameState = engine.initializeGame("room1", initialPlayers, settings);

  assertEquals(gameState.players.length, 2);
  assertEquals(gameState.players[0].chips, 1000);
  assertEquals(gameState.players[0].position, 0);
  assertEquals(gameState.players[1].position, 1);

  // Add a new player
  const newPlayer: PlayerState = {
    id: "p3",
    name: "Player 3",
    isHost: false,
    isConnected: true,
    lastActivity: Date.now(),
  };

  const updatedState = engine.addPlayer(gameState, newPlayer);

  // Verify new player was added
  assertEquals(updatedState.players.length, 3);
  assertEquals(updatedState.players[2].id, "p3");
  assertEquals(updatedState.players[2].name, "Player 3");
  assertEquals(updatedState.players[2].chips, 1000);
  assertEquals(updatedState.players[2].position, 2);
  assertEquals(updatedState.players[2].cards.length, 0);
  assertEquals(updatedState.players[2].bet, 0);
  assertEquals(updatedState.players[2].hasActed, false);
  assertEquals(updatedState.players[2].isAllIn, false);
  assertEquals(updatedState.players[2].isFolded, false);
});

Deno.test("PokerGameEngine - addPlayer does not add duplicate player", () => {
  const engine = new PokerGameEngine();

  const settings: PokerGameSettings = {
    roundTimeSeconds: 60,
    maxRounds: 3,
    buyIn: 1000,
    smallBlind: 10,
    bigBlind: 20,
  };

  const initialPlayers: PlayerState[] = [
    {
      id: "p1",
      name: "Player 1",
      isHost: true,
      isConnected: true,
      lastActivity: Date.now(),
    },
  ];

  const gameState = engine.initializeGame("room1", initialPlayers, settings);

  // Try to add the same player again
  const duplicatePlayer: PlayerState = {
    id: "p1",
    name: "Player 1 Again",
    isHost: false,
    isConnected: true,
    lastActivity: Date.now(),
  };

  const updatedState = engine.addPlayer(gameState, duplicatePlayer);

  // Should still have only 1 player
  assertEquals(updatedState.players.length, 1);
  assertEquals(updatedState.players[0].name, "Player 1"); // Original name preserved
});

Deno.test("PokerGameEngine - removePlayer removes player and reindexes positions", () => {
  const engine = new PokerGameEngine();

  const settings: PokerGameSettings = {
    roundTimeSeconds: 60,
    maxRounds: 3,
    buyIn: 1000,
    smallBlind: 10,
    bigBlind: 20,
  };

  const initialPlayers: PlayerState[] = [
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

  const gameState = engine.initializeGame("room1", initialPlayers, settings);

  // Remove middle player
  const updatedState = engine.removePlayer(gameState, "p2");

  // Verify player was removed
  assertEquals(updatedState.players.length, 2);
  assertEquals(updatedState.players[0].id, "p1");
  assertEquals(updatedState.players[0].position, 0);
  assertEquals(updatedState.players[1].id, "p3");
  assertEquals(updatedState.players[1].position, 1); // Position reindexed from 2 to 1
});
