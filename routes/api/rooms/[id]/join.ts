// API route for joining a specific room
import { Handlers } from "$fresh/server.ts";
import { RoomManager } from "../../../../lib/room-manager.ts";
import { WebSocketManager } from "../../../../lib/websocket/websocket-manager.ts";
import { Env } from "../../../../types/cloudflare.ts";

// Global WebSocket manager instance
let wsManager: WebSocketManager | null = null;

function getWebSocketManager(env: Env): WebSocketManager {
  if (!wsManager) {
    wsManager = new WebSocketManager(env);
  }
  return wsManager;
}

export const handler: Handlers = {
  // POST /api/rooms/[id]/join - Join a room
  async POST(req, ctx) {
    try {
      const env = (ctx.state as any).env as Env;
      if (!env?.DB) {
        return new Response(JSON.stringify({ error: "Database not available" }), {
          status: 500,
          headers: { "Content-Type": "application/json" }
        });
      }

      const roomId = ctx.params.id;
      const body = await req.json();
      const { playerName } = body;

      if (!playerName) {
        return new Response(JSON.stringify({ error: "Player name is required" }), {
          status: 400,
          headers: { "Content-Type": "application/json" }
        });
      }

      const roomManager = new RoomManager(env.DB);
      const result = await roomManager.joinRoom({
        roomId,
        playerName: playerName.trim()
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
        playerId: result.data?.playerId,
        roomId
      }), {
        headers: { "Content-Type": "application/json" }
      });
    } catch (error) {
      console.error("Error joining room:", error);
      return new Response(JSON.stringify({ error: "Internal server error" }), {
        status: 500,
        headers: { "Content-Type": "application/json" }
      });
    }
  }
};