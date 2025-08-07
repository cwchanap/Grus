// API route for leaving a specific room
import { Handlers } from "$fresh/server.ts";
import { RoomManager } from "../../../../lib/core/room-manager.ts";
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
      // In development, we don't have Cloudflare env, so we skip the DB check
      const env = (ctx.state as any).env as Env;
      const isDevelopment = Deno.env.get("DENO_ENV") !== "production";

      if (!isDevelopment && !env?.DB) {
        return new Response(JSON.stringify({ error: "Database not available" }), {
          status: 500,
          headers: { "Content-Type": "application/json" },
        });
      }

      const roomId = ctx.params.id;
      const body = await req.json();
      const { playerId } = body;

      if (!playerId || playerId.trim() === "") {
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

      // Only broadcast WebSocket updates in production with proper env
      if (!isDevelopment && env?.DB) {
        try {
          const wsManager = getWebSocketManager(env);
          await wsManager.broadcastLobbyUpdate();

          // If room was deleted, no need to broadcast room updates
          if (!result.data?.roomDeleted) {
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
          }
        } catch (wsError) {
          console.error("WebSocket broadcast error:", wsError);
          // Don't fail the request if WebSocket fails
        }
      }

      return new Response(
        JSON.stringify({
          success: true,
          wasHost: result.data?.wasHost || false,
          newHostId: result.data?.newHostId,
          roomDeleted: result.data?.roomDeleted || false,
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
