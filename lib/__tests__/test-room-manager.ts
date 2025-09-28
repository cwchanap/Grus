import { type CreateRoomResult, RoomManager } from "../core/room-manager.ts";
import { getAsyncDatabaseService } from "../database-factory.ts";

// Thin test wrapper that provides sensible defaults for tests
export class TestRoomManager extends RoomManager {
  override createRoom(
    params: {
      name?: string;
      hostName: string;
      gameType?: string;
      maxPlayers?: number;
      isPrivate?: boolean;
    },
  ): Promise<CreateRoomResult> {
    const {
      name = "Test Room",
      hostName,
      gameType = "drawing",
      maxPlayers = 8,
      isPrivate = false,
    } = params;
    return super.createRoom({
      name,
      hostName,
      gameType,
      maxPlayers,
      isPrivate,
    });
  }

  // Expose close for tests to cleanup underlying DB
  close(): void {
    const db = getAsyncDatabaseService();
    db.close();
  }
}
