// Legacy WebSocket handler - now uses the new modular architecture
// This file provides backward compatibility for existing code
import "../../types/websocket.ts";
import { Env } from "../../types/cloudflare.ts";
import { WebSocketServer } from "./core/websocket-server.ts";

/**
 * @deprecated Use WebSocketServer from ./core/websocket-server.ts instead
 * This class is maintained for backward compatibility
 */
export class WebSocketHandler {
  private server: WebSocketServer;

  constructor(env: Env) {
    this.server = new WebSocketServer(env);
  }

  handleWebSocketUpgrade(request: Request): Response {
    return this.server.handleWebSocketUpgrade(request);
  }

  // Legacy compatibility methods - delegate to new server
  getConnectionCount(): number {
    return this.server.getConnectionCount();
  }

  getRoomConnectionCount(roomId: string): number {
    return this.server.getRoomConnectionCount(roomId);
  }

  getActiveRoomIds(): string[] {
    return this.server.getActiveRoomIds();
  }

  async broadcastToRoomPublic(roomId: string, message: any): Promise<void> {
    await this.server.broadcastToRoom(roomId, message);
  }

  cleanup(): void {
    this.server.cleanup();
  }
}