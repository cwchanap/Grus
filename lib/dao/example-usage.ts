#!/usr/bin/env -S deno run -A

/**
 * Example usage of the database layer for the multiplayer drawing game
 * This demonstrates how to use the DAOs and database service
 */

import { RoomDAO } from "./room-dao.ts";
import { PlayerDAO } from "./player-dao.ts";
import { GameSessionDAO } from "./game-session-dao.ts";
import type { D1Database } from "../../types/cloudflare.ts";

// This would normally come from Cloudflare Workers environment
// For this example, we'll show the interface
interface ExampleEnvironment {
  DB: D1Database;
}

async function exampleUsage(env: ExampleEnvironment) {
  // Initialize DAOs
  const roomDAO = new RoomDAO(env.DB);
  const playerDAO = new PlayerDAO(env.DB);
  const sessionDAO = new GameSessionDAO(env.DB);

  console.log("🎮 Multiplayer Drawing Game - Database Example");
  console.log("=" .repeat(50));

  // 1. Create a new room
  console.log("\n1. Creating a new room...");
  const roomResult = await roomDAO.create({
    name: "Awesome Drawing Room",
    hostId: "player-123",
    maxPlayers: 8,
    isActive: true
  });

  if (!roomResult.success) {
    console.error("Failed to create room:", roomResult.error);
    return;
  }

  const roomId = roomResult.data!;
  console.log(`✅ Room created with ID: ${roomId}`);

  // 2. Add players to the room
  console.log("\n2. Adding players to the room...");
  const hostResult = await playerDAO.create({
    name: "Alice (Host)",
    roomId,
    isHost: true
  });

  const player2Result = await playerDAO.create({
    name: "Bob",
    roomId,
    isHost: false
  });

  const player3Result = await playerDAO.create({
    name: "Charlie",
    roomId,
    isHost: false
  });

  if (hostResult.success && player2Result.success && player3Result.success) {
    console.log("✅ Added 3 players to the room");
  }

  // 3. Get room summary
  console.log("\n3. Getting room summary...");
  const summaryResult = await roomDAO.getRoomSummary(roomId);
  if (summaryResult.success && summaryResult.data) {
    const { room, playerCount, canJoin } = summaryResult.data;
    console.log(`📊 Room: ${room.name}`);
    console.log(`👥 Players: ${playerCount}/${room.maxPlayers}`);
    console.log(`🚪 Can join: ${canJoin ? 'Yes' : 'No'}`);
  }

  // 4. Start a game session
  console.log("\n4. Starting a game session...");
  const sessionResult = await sessionDAO.create({
    roomId,
    totalRounds: 5
  });

  if (sessionResult.success) {
    console.log(`🎯 Game session started with ID: ${sessionResult.data}`);
  }

  // 5. Get players in room
  console.log("\n5. Getting players in room...");
  const playersResult = await playerDAO.findByRoom(roomId);
  if (playersResult.success && playersResult.data) {
    console.log("👥 Players in room:");
    playersResult.data.forEach(player => {
      console.log(`  - ${player.name} ${player.isHost ? '(Host)' : ''}`);
    });
  }

  // 6. Find the host
  console.log("\n6. Finding room host...");
  const hostFindResult = await playerDAO.findHost(roomId);
  if (hostFindResult.success && hostFindResult.data) {
    console.log(`👑 Host: ${hostFindResult.data.name}`);
  }

  // 7. Check if room can be joined
  console.log("\n7. Checking if room can be joined...");
  const canJoinResult = await roomDAO.canJoin(roomId);
  if (canJoinResult.success) {
    console.log(`🚪 Room can be joined: ${canJoinResult.data ? 'Yes' : 'No'}`);
  }

  console.log("\n✨ Database operations completed successfully!");
  console.log("\nThis demonstrates the complete database layer for:");
  console.log("- Room management (create, find, update, delete)");
  console.log("- Player management (create, join rooms, host privileges)");
  console.log("- Game session tracking");
  console.log("- Score management (ready for implementation)");
  console.log("- Error handling with DatabaseResult pattern");
  console.log("- Proper data validation and constraints");
}

// Export for use in actual Cloudflare Workers
export { exampleUsage };

// Example of how this would be used in a Cloudflare Worker
export default {
  async fetch(request: Request, env: ExampleEnvironment): Promise<Response> {
    // In a real worker, you'd handle different routes
    if (request.url.includes('/example')) {
      await exampleUsage(env);
      return new Response('Database example completed - check logs');
    }
    
    return new Response('Multiplayer Drawing Game API');
  }
};

if (import.meta.main) {
  console.log("This is an example of how to use the database layer.");
  console.log("In a real Cloudflare Worker, you would have access to env.DB");
  console.log("Run this in the context of a Cloudflare Worker to see it in action.");
}