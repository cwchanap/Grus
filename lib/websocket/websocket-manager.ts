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

    // Handle lobby connections (no roomId)
    if (!roomId) {
      return this.handleLobbyWebSocket(request);
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
      status: "healthy",
    };

    return new Response(JSON.stringify(info), {
      headers: { "Content-Type": "application/json" },
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
      }
    }
  }

  private async isRoomActive(roomId: string): Promise<boolean> {
    try {
      // In development, we don't have access to Cloudflare DB
      // Use the database service instead
      if (!this.env?.DB) {
        const { getDatabaseService } = await import("../database-factory.ts");
        const db = getDatabaseService();
        const roomResult = await db.getRoomById(roomId);
        const playersResult = await db.getPlayersByRoom(roomId);

        return roomResult.success &&
          roomResult.data !== null &&
          playersResult.success &&
          (playersResult.data?.length || 0) > 0;
      }

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
    }
  }

  getActiveRoomCount(): number {
    return this.handlers.size;
  }

  // Lobby WebSocket connections
  private lobbyConnections: Set<WebSocket> = new Set();

  private async handleLobbyWebSocket(request: Request): Promise<Response> {
    const upgradeHeader = request.headers.get("Upgrade");
    if (upgradeHeader !== "websocket") {
      return new Response("Expected Upgrade: websocket", { status: 426 });
    }

    // Check if we're in Cloudflare Workers environment
    if (typeof WebSocketPair !== "undefined") {
      // Cloudflare Workers environment
      const webSocketPair = new WebSocketPair();
      const [client, server] = Object.values(webSocketPair) as [WebSocket, WebSocket];

      // Accept the WebSocket connection
      server.accept();

      // Add to lobby connections
      this.lobbyConnections.add(server);

      // Handle WebSocket events
      server.addEventListener("message", (event: MessageEvent) => {
        this.handleLobbyMessage(server, event.data);
      });

      server.addEventListener("close", () => {
        this.lobbyConnections.delete(server);
      });

      server.addEventListener("error", (error: Event) => {
        console.error("Lobby WebSocket error:", error);
        this.lobbyConnections.delete(server);
      });

      return new Response(
        null,
        {
          status: 101,
          webSocket: client,
        } as ResponseInit & { webSocket: WebSocket },
      );
    } else {
      // Deno environment - Use Deno's native WebSocket upgrade
      try {
        const { socket, response } = Deno.upgradeWebSocket(request);

        // Add to lobby connections
        this.lobbyConnections.add(socket);

        // Handle WebSocket events
        socket.addEventListener("open", () => {
          // Lobby connection opened
        });

        socket.addEventListener("message", (event: MessageEvent) => {
          this.handleLobbyMessage(socket, event.data);
        });

        socket.addEventListener("close", () => {
          this.lobbyConnections.delete(socket);
        });

        socket.addEventListener("error", (error: Event) => {
          console.error("Lobby WebSocket error:", error);
          this.lobbyConnections.delete(socket);
        });

        return response;
      } catch (error) {
        console.error("Failed to upgrade lobby WebSocket in Deno environment:", error);
        return new Response("WebSocket upgrade failed", {
          status: 500,
          headers: { "Content-Type": "text/plain" },
        });
      }
    }
  }

  private async handleLobbyMessage(ws: WebSocket, data: string) {
    try {
      const message = JSON.parse(data);

      if (message.type === "subscribe-lobby") {
        // Client wants to subscribe to lobby updates
        // Send current room list
        const rooms = await this.getLobbyRooms();
        this.sendLobbyMessage(ws, {
          type: "lobby-update",
          data: { rooms },
        });
      }
    } catch (error) {
      console.error("Error handling lobby message:", error);
    }
  }

  private async getLobbyRooms() {
    try {
      // In development, we don't have access to Cloudflare DB
      // Use the database service instead
      if (!this.env?.DB) {
        const { getDatabaseService } = await import("../database-factory.ts");
        const db = getDatabaseService();
        const result = await db.getActiveRooms(20);
        return result.success ? (result.data || []) : [];
      }

      // Get active rooms from database
      const stmt = this.env.DB.prepare(`
        SELECT r.*, COUNT(p.id) as player_count
        FROM rooms r
        LEFT JOIN players p ON r.id = p.room_id
        WHERE r.is_active = 1
        GROUP BY r.id
        ORDER BY r.created_at DESC
        LIMIT 20
      `);

      const result = await stmt.all();
      return result.results || [];
    } catch (error) {
      console.error("Error getting lobby rooms:", error);
      return [];
    }
  }

  private sendLobbyMessage(ws: WebSocket, message: any) {
    if (ws.readyState === WebSocket.OPEN) {
      try {
        ws.send(JSON.stringify(message));
      } catch (error) {
        console.error("Error sending lobby message:", error);
        this.lobbyConnections.delete(ws);
      }
    }
  }

  // Broadcast lobby updates to all lobby connections
  async broadcastLobbyUpdate(): Promise<void> {
    const rooms = await this.getLobbyRooms();
    const message = {
      type: "lobby-update",
      data: { rooms },
    };

    const messageStr = JSON.stringify(message);

    // Send to all lobby connections
    for (const ws of this.lobbyConnections) {
      if (ws.readyState === WebSocket.OPEN) {
        try {
          ws.send(messageStr);
        } catch (error) {
          console.error("Error broadcasting lobby update:", error);
          this.lobbyConnections.delete(ws);
        }
      } else {
        this.lobbyConnections.delete(ws);
      }
    }
  }

  // Broadcast message to specific room
  async broadcastToRoomPublic(roomId: string, message: any): Promise<void> {
    const handler = this.handlers.get(roomId);
    if (handler) {
      try {
        await (handler as any).broadcastToRoomPublic(roomId, message);
      } catch (error) {
        console.error(`Error broadcasting to room ${roomId}:`, error);
      }
    }
  }
}
