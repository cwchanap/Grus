import { type CreateRoomResult, RoomManager } from "../core/room-manager.ts";
import { getAsyncDatabaseService } from "../database-factory.ts";

// Thin test wrapper that provides sensible defaults for tests
export class TestRoomManager extends RoomManager {
  override createRoom(
    params: { hostName: string; maxPlayers?: number },
  ): Promise<CreateRoomResult> {
    const { hostName, maxPlayers = 8 } = params;
    return super.createRoom({
      name: "Test Room",
      hostName,
      gameType: "drawing",
      maxPlayers,
    });
  }

  // Expose close for tests to cleanup underlying DB
  close(): void {
    const db = getAsyncDatabaseService();
    db.close();
  }
}
