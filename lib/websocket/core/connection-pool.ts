// WebSocket connection pool management
import type {
  RoomConnections as _RoomConnections,
  WebSocketConnection,
} from "../types/websocket-internal.ts";
import type { ServerMessage } from "../../../types/game.ts";

export class ConnectionPool {
  private connections: Map<string, WebSocketConnection> = new Map();
  private playerRooms: Map<string, string> = new Map();
  private roomConnections: Map<string, Set<string>> = new Map();

  addConnection(playerId: string, roomId: string, ws: WebSocket): void {
    console.log(`Adding connection: playerId=${playerId}, roomId=${roomId}`);

    const connection: WebSocketConnection = {
      ws,
      playerId,
      roomId,
      lastActivity: Date.now(),
    };

    this.connections.set(playerId, connection);
    this.playerRooms.set(playerId, roomId);

    if (!this.roomConnections.has(roomId)) {
      this.roomConnections.set(roomId, new Set());
    }
    this.roomConnections.get(roomId)!.add(playerId);

    console.log(
      `Connection added. Total connections: ${this.connections.size}, Room ${roomId} connections: ${
        this.roomConnections.get(roomId)?.size || 0
      }`,
    );
  }

  removeConnection(playerId: string): string | null {
    console.log(`Removing connection: playerId=${playerId}`);

    const connection = this.connections.get(playerId);
    if (!connection) {
      console.log(`No connection found for playerId=${playerId}`);
      return null;
    }

    const roomId = connection.roomId;

    this.connections.delete(playerId);
    this.playerRooms.delete(playerId);

    const roomConnections = this.roomConnections.get(roomId);
    if (roomConnections) {
      roomConnections.delete(playerId);
      if (roomConnections.size === 0) {
        this.roomConnections.delete(roomId);
        console.log(`Room ${roomId} has no more connections, removing from room connections`);
      }
    }

    console.log(
      `Connection removed. Total connections: ${this.connections.size}, Room ${roomId} connections: ${
        this.roomConnections.get(roomId)?.size || 0
      }`,
    );
    return roomId;
  }

  getConnection(playerId: string): WebSocketConnection | null {
    return this.connections.get(playerId) || null;
  }

  getRoomConnections(roomId: string): string[] {
    const roomConnections = this.roomConnections.get(roomId);
    return roomConnections ? Array.from(roomConnections) : [];
  }

  getPlayerRoom(playerId: string): string | null {
    return this.playerRooms.get(playerId) || null;
  }

  broadcastToRoom(
    roomId: string,
    message: ServerMessage,
    excludePlayerId?: string,
  ): void {
    console.log(`Broadcasting message to room ${roomId}:`, message);
    const roomConnections = this.roomConnections.get(roomId);
    if (!roomConnections) return;

    const messageStr = JSON.stringify(message);
    const failedConnections: string[] = [];

    for (const playerId of roomConnections) {
      if (excludePlayerId && playerId === excludePlayerId) continue;

      const connection = this.connections.get(playerId);
      if (connection && connection.ws.readyState === WebSocket.OPEN) {
        try {
          connection.ws.send(messageStr);
          connection.lastActivity = Date.now();
        } catch (error) {
          console.error(`Error sending message to player ${playerId}:`, error);
          failedConnections.push(playerId);
        }
      } else {
        failedConnections.push(playerId);
      }
    }

    // Clean up failed connections
    for (const playerId of failedConnections) {
      this.removeConnection(playerId);
    }
  }

  sendMessage(playerId: string, message: ServerMessage): boolean {
    const connection = this.connections.get(playerId);
    if (!connection || connection.ws.readyState !== WebSocket.OPEN) {
      return false;
    }

    try {
      connection.ws.send(JSON.stringify(message));
      connection.lastActivity = Date.now();
      return true;
    } catch (error) {
      console.error(`Error sending message to player ${playerId}:`, error);
      this.removeConnection(playerId);
      return false;
    }
  }

  sendError(playerId: string, error: string): void {
    const connection = this.connections.get(playerId);
    const roomId = connection?.roomId || "";

    this.sendMessage(playerId, {
      type: "room-update",
      roomId,
      data: {
        type: "error",
        message: error,
        roomId, // Include roomId in data for component filtering
      },
    });
  }

  getConnectionCount(): number {
    return this.connections.size;
  }

  getRoomConnectionCount(roomId: string): number {
    const roomConnections = this.roomConnections.get(roomId);
    return roomConnections ? roomConnections.size : 0;
  }

  getActiveRoomIds(): string[] {
    return Array.from(this.roomConnections.keys());
  }

  cleanup(): void {
    // Close all connections
    for (const [playerId, connection] of this.connections.entries()) {
      try {
        connection.ws.close(1000, "Server shutdown");
      } catch (error) {
        console.error(`Error closing connection for player ${playerId}:`, error);
      }
    }

    // Clear all maps
    this.connections.clear();
    this.playerRooms.clear();
    this.roomConnections.clear();
  }
}
