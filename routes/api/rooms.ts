// API routes for room operations
import { Handlers } from "$fresh/server.ts";
import { RoomManager } from "../../lib/room-manager.ts";
import { WebSocketManager } from "../../lib/websocket/websocket-manager.ts";
import { Env } from "../../types/cloudflare.ts";

// Global WebSocket manager instance
let wsManager: WebSocketManager | null = null;

function getWebSocketManager(env: Env): WebSocketManager {
  if (!wsManager) {
    wsManager = new WebSocketManager(env);
  }
  return wsManager;
}

export const handler: Handlers = {
  // GET /api/rooms - List active rooms
  async GET(req, ctx) {
    try {
      const env = (ctx.state as any).env as Env;
      if (!env?.DB) {
        return new Response(JSON.stringify({ error: "Database not available" }), {
          status: 500,
          headers: { "Content-Type": "application/json" }
        });
      }

      const url = new URL(req.url);
      const limit = parseInt(url.searchParams.get("limit") || "20");
      const offset = parseInt(url.searchParams.get("offset") || "0");

      const roomManager = new RoomManager(env.DB);
      const result = await roomManager.listRooms({ limit, offset });

      if (!result.success) {
        return new Response(JSON.stringify({ error: result.error }), {
          status: 500,
          headers: { "Content-Type": "application/json" }
        });
      }

      return new Response(JSON.stringify({ rooms: result.data }), {
        headers: { "Content-Type": "application/json" }
      });
    } catch (error) {
      console.error("Error listing rooms:", error);
      return new Response(JSON.stringify({ error: "Internal server error" }), {
        status: 500,
        headers: { "Content-Type": "application/json" }
      });
    }
  },

  // POST /api/rooms - Create a new room
  async POST(req, ctx) {
    try {
      const env = (ctx.state as any).env as Env;
      if (!env?.DB) {
        return new Response(JSON.stringify({ error: "Database not available" }), {
          status: 500,
          headers: { "Content-Type": "application/json" }
        });
      }

      const body = await req.json();
      const { name, hostName, maxPlayers } = body;

      if (!name || !hostName) {
        return new Response(JSON.stringify({ error: "Room name and host name are required" }), {
          status: 400,
          headers: { "Content-Type": "application/json" }
        });
      }

      const roomManager = new RoomManager(env.DB);
      const result = await roomManager.createRoom({
        name: name.trim(),
        hostName: hostName.trim(),
        maxPlayers: maxPlayers || 8
      });

      if (!result.success) {
        return new Response(JSON.stringify({ error: result.error }), {
          status: 400,
          headers: { "Content-Type": "application/json" }
        });
      }

      // Broadcast lobby update
      const wsManager = getWebSocketManager(env);
      await wsManager.broadcastLobbyUpdate();

      return new Response(JSON.stringify({ 
        roomId: result.data?.roomId,
        playerId: result.data?.playerId
      }), {
        status: 201,
        headers: { "Content-Type": "application/json" }
      });
    } catch (error) {
      console.error("Error creating room:", error);
      return new Response(JSON.stringify({ error: "Internal server error" }), {
        status: 500,
        headers: { "Content-Type": "application/json" }
      });
    }
  }
};