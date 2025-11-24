import { assertEquals } from "$std/testing/asserts.ts";
import { getKVRoomService } from "../index.ts";

Deno.test("KVRoomService - updatePlayer functionality", async () => {
  const db = getKVRoomService();

  // Create a test room
  const roomResult = await db.createRoom("Test Room", "temp-host", 8);
  assertEquals(roomResult.success, true);
  const roomId = roomResult.data!;

  // Create two players
  const player1Result = await db.createPlayer("Player 1", roomId, true);
  assertEquals(player1Result.success, true);
  const player1Id = player1Result.data!;

  const player2Result = await db.createPlayer("Player 2", roomId, false);
  assertEquals(player2Result.success, true);
  const player2Id = player2Result.data!;

  // Verify initial host status
  const initialPlayer1 = await db.getPlayerById(player1Id);
  assertEquals(initialPlayer1.success, true);
  assertEquals(initialPlayer1.data?.isHost, true);

  const initialPlayer2 = await db.getPlayerById(player2Id);
  assertEquals(initialPlayer2.success, true);
  assertEquals(initialPlayer2.data?.isHost, false);

  // Transfer host from player1 to player2
  const updateResult1 = await db.updatePlayer(player1Id, { isHost: false });
  assertEquals(updateResult1.success, true);

  const updateResult2 = await db.updatePlayer(player2Id, { isHost: true });
  assertEquals(updateResult2.success, true);

  // Verify host transfer
  const updatedPlayer1 = await db.getPlayerById(player1Id);
  assertEquals(updatedPlayer1.success, true);
  assertEquals(updatedPlayer1.data?.isHost, false);

  const updatedPlayer2 = await db.getPlayerById(player2Id);
  assertEquals(updatedPlayer2.success, true);
  assertEquals(updatedPlayer2.data?.isHost, true);

  // Test name update
  const nameUpdateResult = await db.updatePlayer(player1Id, { name: "Updated Player 1" });
  assertEquals(nameUpdateResult.success, true);

  const updatedNamePlayer = await db.getPlayerById(player1Id);
  assertEquals(updatedNamePlayer.success, true);
  assertEquals(updatedNamePlayer.data?.name, "Updated Player 1");

  // Cleanup
  await db.removePlayer(player1Id);
  await db.removePlayer(player2Id);
  await db.deleteRoom(roomId);
  db.close();
});
