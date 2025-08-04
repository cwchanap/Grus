// WebSocket connection manager for handling multiple connections and rooms
import { Env } from "../../types/cloudflare.ts";
import { WebSocketServer } from "./core/websocket-server.ts";
import { getConfig } from "../config.ts";

export class WebSocketManager {
  private servers: Map<string, WebSocketServer> = new Map();
  private env: Env;
  private heartbeatInterval: number;
  private heartbeatTimer?: number;
  private lobbyConnections: Set<WebSocket> = new Set();

  constructor(env: Env, startHeartbeat = true) {
    this.env = env;
    const config = getConfig();
    this.heartbeatInterval = config.websocket.heartbeatInterval;
    if (startHeartbeat) {
      this.startHeartbeat();
    }
  }

  handleRequest(request: Request): Response {
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

  private handleWebSocketUpgrade(request: Request): Response {
    const url = new URL(request.url);
    const roomId = url.searchParams.get("roomId");

    // Handle lobby connections (no roomId)
    if (!roomId) {
      return this.handleLobbyWebSocket(request);
    }

    // Get or create server for this room
    let server = this.servers.get(roomId);
    if (!server) {
      server = new WebSocketServer(this.env);
      this.servers.set(roomId, server);
    }

    return server.handleWebSocketUpgrade(request);
  }

  private handleWebSocketInfo(): Response {
    const info = {
      activeRooms: this.servers.size,
      timestamp: new Date().toISOString(),
      status: "healthy",
    };

    return new Response(JSON.stringify(info), {
      headers: { "Content-Type": "application/json" },
    });
  }

  private startHeartbeat() {
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
    // Clean up inactive servers
    for (const [roomId, server] of this.servers.entries()) {
      // Check if room is still active
      const isActive = await this.isRoomActive(roomId);
      if (!isActive) {
        server.cleanup();
        this.servers.delete(roomId);
        console.log(`Cleaned up inactive room server: ${roomId}`);
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
        const roomResult = db.getRoomById(roomId);
        const playersResult = db.getPlayersByRoom(roomId);

        if (!roomResult.success || !roomResult.data) return false;
        if (!playersResult.success) return false;

        const playerCount = playersResult.data?.length || 0;
        return roomResult.data.isActive && playerCount > 0;
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
  getRoomConnectionCount(roomId: string): number {
    const server = this.servers.get(roomId);
    return server ? server.getRoomConnectionCount(roomId) : 0;
  }

  async broadcastToAllRooms(message: any): Promise<void> {
    for (const [roomId, server] of this.servers.entries()) {
      try {
        await server.broadcastToRoom(roomId, message);
      } catch (error) {
        console.error(`Error broadcasting to room ${roomId}:`, error);
      }
    }
  }

  getActiveRoomIds(): string[] {
    return Array.from(this.servers.keys());
  }

  cleanupRoom(roomId: string): void {
    const server = this.servers.get(roomId);
    if (server) {
      server.cleanup();
      this.servers.delete(roomId);
      console.log(`Cleaned up room server: ${roomId}`);
    }
  }

  getActiveRoomCount(): number {
    return this.servers.size;
  }

  private handleLobbyWebSocket(request: Request): Response {
    const upgradeHeader = request.headers.get("Upgrade");
    if (upgradeHeader !== "websocket") {
      return new Response("Expected Upgrade: websocket", { status: 426 });
    }

    // Check if we're in Cloudflare Workers environment
    if (typeof (globalThis as any).WebSocketPair !== "undefined") {
      // Cloudflare Workers environment
      const webSocketPair = new (globalThis as any).WebSocketPair();
      const [client, server] = Object.values(webSocketPair) as [WebSocket, WebSocket];

      // Accept the WebSocket connection
      (server as any).accept();

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
      // Deno environment - use Deno.upgradeWebSocket
      try {
        const { socket, response } = Deno.upgradeWebSocket(request);

        // Add to lobby connections
        this.lobbyConnections.add(socket);

        // Handle WebSocket events
        socket.addEventListener("message", (event: MessageEvent) => {
          this.handleLobbyMessage(socket, event.data);
        });

        socket.addEventListener("close", () => {
          this.lobbyConnections.delete(socket);
          console.log("Lobby WebSocket connection closed");
        });

        socket.addEventListener("error", (error: Event) => {
          console.error("Lobby WebSocket error:", error);
          this.lobbyConnections.delete(socket);
        });

        socket.addEventListener("open", () => {
          console.log("Lobby WebSocket connection opened");
          // Send welcome message
          this.sendLobbyMessage(socket, {
            type: "welcome",
            message: "Connected to lobby",
            timestamp: Date.now(),
          });
        });

        return response;
      } catch (error) {
        console.error("Failed to upgrade WebSocket in Deno:", error);
        return new Response("WebSocket upgrade failed", {
          status: 500,
          headers: { "Content-Type": "text/plain" },
        });
      }
    }
  }

  private async handleLobbyMessage(ws: WebSocket, data: string) {
    try {
      // Validate that data is a string and not empty
      if (typeof data !== "string" || data.length === 0) {
        console.error("Invalid lobby message data received:", typeof data, data);
        return;
      }

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
        const result = db.getActiveRooms(20);
        return result.success ? result.data || [] : [];
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

  async broadcastToRoomPublic(roomId: string, message: any): Promise<void> {
    const server = this.servers.get(roomId);
    if (server) {
      try {
        await server.broadcastToRoom(roomId, message);
      } catch (error) {
        console.error(`Error broadcasting to room ${roomId}:`, error);
      }
    }
  }

  cleanup(): void {
    this.stopHeartbeat();

    // Cleanup all servers
    for (const [_roomId, server] of this.servers.entries()) {
      server.cleanup();
    }
    this.servers.clear();

    // Close all lobby connections
    for (const ws of this.lobbyConnections) {
      try {
        ws.close(1000, "Server shutdown");
      } catch (error) {
        console.error("Error closing lobby connection:", error);
      }
    }
    this.lobbyConnections.clear();
  }
}
