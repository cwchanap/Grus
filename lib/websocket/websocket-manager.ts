// WebSocket connection manager for handling multiple connections and rooms
import { Env } from "../../types/cloudflare.ts";
import { WebSocketHandler } from "./websocket-handler.ts";
import { getConfig } from "../config.ts";

export class WebSocketManager {
  private handlers: Map<string, WebSocketHandler> = new Map();
  private env: Env;
  private heartbeatInterval: number;
  private heartbeatTimer?: number;

  constructor(env: Env, startHeartbeat = true) {
    this.env = env;
    const config = getConfig();
    this.heartbeatInterval = config.websocket.heartbeatInterval;
    if (startHeartbeat) {
      this.startHeartbeat();
    }
  }

  async handleRequest(request: Request): Promise<Response> {
    const url = new URL(request.url);
    
    // Handle WebSocket upgrade requests
    if (request.headers.get("Upgrade") === "websocket") {
      return this.handleWebSocketUpgrade(request);
    }

    // Handle HTTP requests for WebSocket info
    if (url.pathname === "/ws/info") {
      return this.handleWebSocketInfo();
    }

    return new Response("Not Found", { status: 404 });
  }

  private async handleWebSocketUpgrade(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const roomId = url.searchParams.get("roomId");
    
    if (!roomId) {
      return new Response("Room ID required", { status: 400 });
    }

    // Get or create handler for this room
    let handler = this.handlers.get(roomId);
    if (!handler) {
      handler = new WebSocketHandler(this.env);
      this.handlers.set(roomId, handler);
    }

    return handler.handleWebSocketUpgrade(request);
  }

  private handleWebSocketInfo(): Response {
    const info = {
      activeRooms: this.handlers.size,
      timestamp: new Date().toISOString(),
      status: "healthy"
    };

    return new Response(JSON.stringify(info), {
      headers: { "Content-Type": "application/json" }
    });
  }

  private startHeartbeat() {
    // In a real Cloudflare Worker, we'd use Durable Objects for this
    // For now, this is a placeholder for heartbeat functionality
    this.heartbeatTimer = setInterval(() => {
      this.performHeartbeat();
    }, this.heartbeatInterval);
  }

  stopHeartbeat() {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = undefined;
    }
  }

  private async performHeartbeat() {
    // Clean up inactive handlers
    const now = Date.now();
    const config = getConfig();
    
    for (const [roomId, handler] of this.handlers.entries()) {
      // Check if room is still active
      const isActive = await this.isRoomActive(roomId);
      if (!isActive) {
        this.handlers.delete(roomId);
        console.log(`Cleaned up inactive room handler: ${roomId}`);
      }
    }
  }

  private async isRoomActive(roomId: string): Promise<boolean> {
    try {
      // Check if room exists in database and has active players
      const stmt = this.env.DB.prepare(`
        SELECT r.id 
        FROM rooms r 
        LEFT JOIN players p ON r.id = p.room_id 
        WHERE r.id = ? AND r.is_active = 1
        GROUP BY r.id 
        HAVING COUNT(p.id) > 0
      `);
      
      const result = await stmt.bind(roomId).first();
      return result !== null;
    } catch (error) {
      console.error("Error checking room activity:", error);
      return false;
    }
  }

  // Utility methods for external use
  async getRoomConnectionCount(roomId: string): Promise<number> {
    const handler = this.handlers.get(roomId);
    if (!handler) return 0;
    
    // Access the private connections map through a public method
    return (handler as any).getConnectionCount?.() || 0;
  }

  async broadcastToAllRooms(message: any): Promise<void> {
    for (const [roomId, handler] of this.handlers.entries()) {
      try {
        // Broadcast to all connections in each room
        await (handler as any).broadcastToRoom?.(roomId, message);
      } catch (error) {
        console.error(`Error broadcasting to room ${roomId}:`, error);
      }
    }
  }

  // Get all active room IDs
  getActiveRoomIds(): string[] {
    return Array.from(this.handlers.keys());
  }

  // Clean up a specific room handler
  async cleanupRoom(roomId: string): Promise<void> {
    const handler = this.handlers.get(roomId);
    if (handler) {
      // Perform any cleanup needed
      this.handlers.delete(roomId);
      console.log(`Cleaned up room handler: ${roomId}`);
    }
  }

  getActiveRoomCount(): number {
    return this.handlers.size;
  }
}