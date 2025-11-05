#!/usr/bin/env -S deno run --allow-all --unstable-kv

// Quick test to verify game-in-progress blocking logic
import { CoreWebSocketHandler } from "./lib/core/websocket-handler.ts";
import { RoomManager } from "./lib/core/room-manager.ts";
import "./lib/games/index.ts";

console.log("Testing game-in-progress join blocking...\n");

// Create WebSocket handler (simulates the global singleton)
const wsHandler = new CoreWebSocketHandler();

// Store it globally like in production
(globalThis as any).__WS_HANDLER__ = wsHandler;

// Create a room
const roomManager = new RoomManager();
const createResult = await roomManager.createRoom({
  name: "Test Room",
  hostName: "Host Player",
  gameType: "drawing",
});

if (!createResult.success || !createResult.data) {
  console.error("❌ Failed to create room:", createResult.error);
  Deno.exit(1);
}

const roomId = createResult.data.roomId;
console.log("✅ Room created:", roomId);

// Check initial state - should be joinable
let summary = await roomManager.getRoomSummary(roomId);
if (!summary.success || !summary.data) {
  console.error("❌ Failed to get room summary");
  Deno.exit(1);
}

console.log("Initial state:");
console.log("  - canJoin:", summary.data.canJoin);
console.log("  - canJoinReason:", summary.data.canJoinReason || "none");
console.log("  - Game phase:", wsHandler.getGamePhase(roomId));

if (!summary.data.canJoin) {
  console.error("❌ Room should be joinable initially");
  Deno.exit(1);
}
console.log("✅ Room is joinable before game starts\n");

// Simulate starting a game by setting a game state
const mockGameState: { phase: "waiting" | "playing" | "results" | "finished"; [key: string]: any } = {
  roomId,
  gameType: "drawing",
  phase: "playing",
  roundNumber: 1,
  timeRemaining: 60,
  players: [],
  scores: {},
  gameData: {},
  chatMessages: [],
  settings: { maxRounds: 3, roundTimeSeconds: 60 },
};

// Access the private gameStates map (for testing only)
(wsHandler as any).gameStates.set(roomId, mockGameState);

console.log("Game started (phase: playing)");

// Check state after game started - should NOT be joinable
summary = await roomManager.getRoomSummary(roomId);
if (!summary.success || !summary.data) {
  console.error("❌ Failed to get room summary after game start");
  Deno.exit(1);
}

console.log("After game start:");
console.log("  - canJoin:", summary.data.canJoin);
console.log("  - canJoinReason:", summary.data.canJoinReason || "none");
console.log("  - Game phase:", wsHandler.getGamePhase(roomId));

if (summary.data.canJoin) {
  console.error("❌ Room should NOT be joinable during active game");
  Deno.exit(1);
}

if (summary.data.canJoinReason !== "game-in-progress") {
  console.error(
    "❌ canJoinReason should be 'game-in-progress', got:",
    summary.data.canJoinReason,
  );
  Deno.exit(1);
}

console.log("✅ Room correctly blocked during active game\n");

// Test the WebSocket handler's canJoinDuringGameState method
if (wsHandler.canJoinDuringGameState(roomId)) {
  console.error("❌ canJoinDuringGameState should return false during playing");
  Deno.exit(1);
}
console.log("✅ WebSocket handler correctly prevents join during game\n");

// Set game to finished state
mockGameState.phase = "finished";
(wsHandler as any).gameStates.set(roomId, mockGameState);

console.log("Game finished (phase: finished)");

// Should be joinable again after game finishes
summary = await roomManager.getRoomSummary(roomId);
if (!summary.success || !summary.data) {
  console.error("❌ Failed to get room summary after game finished");
  Deno.exit(1);
}

console.log("After game finished:");
console.log("  - canJoin:", summary.data.canJoin);
console.log("  - canJoinReason:", summary.data.canJoinReason || "none");
console.log("  - Game phase:", wsHandler.getGamePhase(roomId));

if (!summary.data.canJoin) {
  console.error("❌ Room should be joinable after game finishes");
  Deno.exit(1);
}
console.log("✅ Room is joinable after game finishes\n");

// Cleanup
wsHandler.cleanup();

console.log("🎉 All tests passed!");
