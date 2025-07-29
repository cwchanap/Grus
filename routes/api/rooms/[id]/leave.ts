// API route for leaving a specific room
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
  // POST /api/rooms/[id]/leave - Leave a room
  async POST(req, ctx) {
    try {
      const env = (ctx.state as any).env as Env;
      if (!env?.DB) {
        return new Response(JSON.stringify({ error: "Database not available" }), {
          status: 500,
          headers: { "Content-Type": "application/json" },
        });
      }

      const roomId = ctx.params.id;
      const body = await req.json();
      const { playerId } = body;

      if (!playerId) {
        return new Response(JSON.stringify({ error: "Player ID is required" }), {
          status: 400,
          headers: { "Content-Type": "application/json" },
        });
      }

      const roomManager = new RoomManager();
      const result = await roomManager.leaveRoom(roomId, playerId);

      if (!result.success) {
        return new Response(JSON.stringify({ error: result.error }), {
          status: 400,
          headers: { "Content-Type": "application/json" },
        });
      }

      // Broadcast lobby update and room update
      const wsManager = getWebSocketManager(env);
      await wsManager.broadcastLobbyUpdate();

      // If there was a host transfer, broadcast room update
      if (result.data?.newHostId) {
        await wsManager.broadcastToRoomPublic(roomId, {
          type: "room-update",
          roomId,
          data: {
            type: "host-transferred",
            newHostId: result.data.newHostId,
            leftPlayerId: playerId,
          },
        });
      } else {
        // Broadcast player left
        await wsManager.broadcastToRoomPublic(roomId, {
          type: "room-update",
          roomId,
          data: {
            type: "player-left",
            playerId,
          },
        });
      }

      return new Response(
        JSON.stringify({
          success: true,
          wasHost: result.data?.wasHost || false,
          newHostId: result.data?.newHostId,
        }),
        {
          headers: { "Content-Type": "application/json" },
        },
      );
    } catch (error) {
      console.error("Error leaving room:", error);
      return new Response(JSON.stringify({ error: "Internal server error" }), {
        status: 500,
        headers: { "Content-Type": "application/json" },
      });
    }
  },
};
