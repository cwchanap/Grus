#!/usr/bin/env -S deno run -A

/**
 * Simple test script for the leave room API endpoint
 * This tests the basic functionality of the /api/rooms/[id]/leave route
 */

import { RoomManager } from "./lib/room-manager.ts";

// Mock environment for testing
const mockEnv = {
  DB: null as any, // Will be set up in a real test environment
  GAME_STATE: null as any
};

async function testLeaveRoomAPI() {
  console.log("🧪 Testing Leave Room API...");
  
  try {
    // Test 1: Missing player ID
    console.log("Test 1: Missing player ID");
    const response1 = await fetch("http://localhost:8000/api/rooms/test-room/leave", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({})
    });
    
    if (response1.status === 400) {
      console.log("✅ Correctly rejected request with missing player ID");
    } else {
      console.log("❌ Should have rejected request with missing player ID");
    }

    // Test 2: Invalid room ID
    console.log("Test 2: Invalid room ID");
    const response2 = await fetch("http://localhost:8000/api/rooms/nonexistent-room/leave", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ playerId: "test-player" })
    });
    
    if (response2.status === 404) {
      console.log("✅ Correctly rejected request with invalid room ID");
    } else {
      console.log("❌ Should have rejected request with invalid room ID");
    }

    // Test 3: Player not in room
    console.log("Test 3: Player not in room");
    // This would require setting up a real room first
    console.log("⏭️  Skipping - requires database setup");

    console.log("🎉 Leave Room API tests completed");
    
  } catch (error) {
    console.error("❌ Test failed:", error);
  }
}

// Run tests if this script is executed directly
if (import.meta.main) {
  await testLeaveRoomAPI();
}

export { testLeaveRoomAPI };