// API route for joining a specific room
import { Handlers } from "$fresh/server.ts";
import { RoomManager } from "../../../../lib/core/room-manager.ts";

export const handler: Handlers = {
  // POST /api/rooms/[id]/join - Join a room
  async POST(req, ctx) {
    try {
      const _isDevelopment = Deno.env.get("DENO_ENV") === "development";

      const roomId = ctx.params.id;
      const body = await req.json();
      const { playerName } = body;

      if (!playerName) {
        return new Response(JSON.stringify({ error: "Player name is required" }), {
          status: 400,
          headers: { "Content-Type": "application/json" },
        });
      }

      const roomManager = new RoomManager();
      const result = await roomManager.joinRoom({
        roomId,
        playerName: playerName.trim(),
      });

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
          playerId: result.data?.playerId,
          roomId,
        }),
        {
          headers: { "Content-Type": "application/json" },
        },
      );
    } catch (error) {
      console.error("Error joining room:", error);
      return new Response(JSON.stringify({ error: "Internal server error" }), {
        status: 500,
        headers: { "Content-Type": "application/json" },
      });
    }
  },
};
