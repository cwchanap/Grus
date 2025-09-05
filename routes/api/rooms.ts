// API routes for room operations
import { Handlers } from "$fresh/server.ts";
import { RoomManager } from "../../lib/core/room-manager.ts";

export const handler: Handlers = {
  // GET /api/rooms - List active rooms
  async GET(req, _ctx) {
    try {
      const url = new URL(req.url);
      const limit = parseInt(url.searchParams.get("limit") || "20");

      const roomManager = new RoomManager();

      // Get active rooms with automatic cleanup
      const result = await roomManager.getActiveRoomsWithCleanup(limit);

      if (!result.success) {
        return new Response(JSON.stringify({ error: result.error }), {
          status: 500,
          headers: { "Content-Type": "application/json" },
        });
      }

      return new Response(JSON.stringify({ rooms: result.data || [] }), {
        headers: { "Content-Type": "application/json" },
      });
    } catch (error) {
      console.error("Error listing rooms:", error);
      return new Response(JSON.stringify({ error: "Internal server error" }), {
        status: 500,
        headers: { "Content-Type": "application/json" },
      });
    }
  },

  // POST /api/rooms - Create a new room
  async POST(req, _ctx) {
    try {
      const body = await req.json();
      const { name, hostName, gameType, maxPlayers, isPrivate } = body;

      if (!name || !hostName) {
        return new Response(JSON.stringify({ error: "Room name and host name are required" }), {
          status: 400,
          headers: { "Content-Type": "application/json" },
        });
      }

      const roomManager = new RoomManager();

      // Create room using room manager
      const result = await roomManager.createRoom({
        name: name.trim(),
        hostName: hostName.trim(),
        gameType: gameType || "drawing",
        maxPlayers: maxPlayers || 8,
        isPrivate: isPrivate || false,
      });

      if (!result.success || !result.data) {
        return new Response(JSON.stringify({ error: result.error }), {
          status: 400,
          headers: { "Content-Type": "application/json" },
        });
      }

      return new Response(
        JSON.stringify({
          roomId: result.data.roomId,
          playerId: result.data.playerId,
          room: result.data.room,
        }),
        {
          status: 201,
          headers: { "Content-Type": "application/json" },
        },
      );
    } catch (error) {
      console.error("Error creating room:", error);
      return new Response(JSON.stringify({ error: "Internal server error" }), {
        status: 500,
        headers: { "Content-Type": "application/json" },
      });
    }
  },
};
