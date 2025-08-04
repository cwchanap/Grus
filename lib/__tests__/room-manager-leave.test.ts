// Unit tests for room manager leave functionality with host migration
import { assertEquals, assertExists } from "https://deno.land/std@0.208.0/assert/mod.ts";
import { TestRoomManager } from "./test-room-manager.ts";

Deno.test("Room Manager - Player Leave and Host Migration", async (t) => {
  const roomManager = new TestRoomManager();

  let roomId: string;
  let hostPlayerId: string;
  let player2Id: string;
  let player3Id: string;

  // Setup: Create room with host and additional players
  await t.step("setup room with multiple players", async () => {
    // Create room with host
    const createResult = await roomManager.createRoom({
      hostName: "Host Player",
      maxPlayers: 8,
    });

    if (!createResult.success) {
      console.error("Failed to create room:", createResult.error);
    }

    assertEquals(createResult.success, true);
    assertExists(createResult.data);

    roomId = createResult.data.roomId;
    hostPlayerId = createResult.data.playerId;

    // Add second player
    const joinResult2 = await roomManager.joinRoom({
      roomId,
      playerName: "Player 2",
    });
    assertEquals(joinResult2.success, true);
    assertExists(joinResult2.data);
    player2Id = joinResult2.data.playerId;

    // Add third player
    const joinResult3 = await roomManager.joinRoom({
      roomId,
      playerName: "Player 3",
    });
    assertEquals(joinResult3.success, true);
    assertExists(joinResult3.data);
    player3Id = joinResult3.data.playerId;

    // Verify room has 3 players
    const roomSummary = await roomManager.getRoomSummary(roomId);
    assertEquals(roomSummary.success, true);
    assertEquals(roomSummary.data?.playerCount, 3);
  });

  await t.step("non-host player leaves room", async () => {
    // Player 2 (non-host) leaves
    const leaveResult = await roomManager.leaveRoom(roomId, player2Id);
    assertEquals(leaveResult.success, true);
    assertEquals(leaveResult.data?.wasHost, false);
    assertEquals(leaveResult.data?.newHostId, undefined);
    assertEquals(leaveResult.data?.roomDeleted, false);
    assertEquals(leaveResult.data?.remainingPlayers?.length, 2);

    // Verify room still has host and player 3
    const roomSummary = await roomManager.getRoomSummary(roomId);
    assertEquals(roomSummary.success, true);
    assertEquals(roomSummary.data?.playerCount, 2);
    assertEquals(roomSummary.data?.host?.id, hostPlayerId);
  });

  await t.step("host leaves room - triggers host migration", async () => {
    // Host leaves, should transfer to player 3
    const leaveResult = await roomManager.leaveRoom(roomId, hostPlayerId);
    assertEquals(leaveResult.success, true);
    assertEquals(leaveResult.data?.wasHost, true);
    assertEquals(leaveResult.data?.newHostId, player3Id);
    assertEquals(leaveResult.data?.newHostName, "Player 3");
    assertEquals(leaveResult.data?.roomDeleted, false);
    assertEquals(leaveResult.data?.remainingPlayers?.length, 1);

    // Verify player 3 is now the host
    const roomSummary = await roomManager.getRoomSummary(roomId);
    assertEquals(roomSummary.success, true);
    assertEquals(roomSummary.data?.playerCount, 1);
    assertEquals(roomSummary.data?.host?.id, player3Id);
    assertEquals(roomSummary.data?.host?.name, "Player 3");
  });

  await t.step("last player leaves room - room gets deleted", async () => {
    // Last player leaves, room should be deleted
    const leaveResult = await roomManager.leaveRoom(roomId, player3Id);
    assertEquals(leaveResult.success, true);
    assertEquals(leaveResult.data?.wasHost, true);
    assertEquals(leaveResult.data?.roomDeleted, true);
    assertEquals(leaveResult.data?.remainingPlayers?.length, 0);

    // Verify room no longer exists
    const roomSummary = await roomManager.getRoomSummary(roomId);
    assertEquals(roomSummary.success, false);
    assertEquals(roomSummary.error, "Room not found");
  });

  await t.step("cleanup", async () => {
    // Close the in-memory database
    roomManager.close();
  });
});

Deno.test("Room Manager - Edge Cases for Player Leave", async (t) => {
  const roomManager = new TestRoomManager();

  await t.step("leave non-existent room", async () => {
    const leaveResult = await roomManager.leaveRoom("NONEXIST", "fake-player-id");
    assertEquals(leaveResult.success, false);
    assertEquals(leaveResult.error, "Room not found");
  });

  await t.step("leave room with non-existent player", async () => {
    // Create a room first
    const createResult = await roomManager.createRoom({
      hostName: "Test Host",
      maxPlayers: 8,
    });
    assertEquals(createResult.success, true);
    assertExists(createResult.data);

    const roomId = createResult.data.roomId;

    // Try to leave with non-existent player
    const leaveResult = await roomManager.leaveRoom(roomId, "fake-player-id");
    assertEquals(leaveResult.success, false);
    assertEquals(leaveResult.error, "Player not found in room");

    // Cleanup
    const hostPlayerId = createResult.data.playerId;
    await roomManager.leaveRoom(roomId, hostPlayerId);
  });

  await t.step("cleanup", async () => {
    // Close the in-memory database
    roomManager.close();
  });
});
