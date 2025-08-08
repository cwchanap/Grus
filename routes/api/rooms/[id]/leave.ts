// API route for leaving a specific room
import { Handlers } from "$fresh/server.ts";
import { RoomManager } from "../../../../lib/core/room-manager.ts";
import { Env } from "../../../../types/cloudflare.ts";

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

      // Note: WebSocket updates are now handled by the CoreWebSocketHandler
      // through real-time connections, so no need for explicit broadcasting here

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
