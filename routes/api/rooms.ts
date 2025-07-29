// API routes for room operations
import { Handlers } from "$fresh/server.ts";
import { getDatabaseService } from "../../lib/database-factory.ts";
import { getKVService } from "../../lib/kv-service.ts";
import { RoomManager, RoomSummary } from "../../lib/room-manager.ts";

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
      const { name, hostName, maxPlayers } = body;

      if (!name || !hostName) {
        return new Response(JSON.stringify({ error: "Room name and host name are required" }), {
          status: 400,
          headers: { "Content-Type": "application/json" },
        });
      }

      const dbService = getDatabaseService();

      // Create room
      const roomResult = await dbService.createRoom(name.trim(), "system", maxPlayers || 8);
      if (!roomResult.success) {
        return new Response(JSON.stringify({ error: roomResult.error }), {
          status: 400,
          headers: { "Content-Type": "application/json" },
        });
      }

      // Create host player
      const playerResult = await dbService.createPlayer(hostName.trim(), roomResult.data!, true);
      if (!playerResult.success) {
        return new Response(JSON.stringify({ error: playerResult.error }), {
          status: 400,
          headers: { "Content-Type": "application/json" },
        });
      }

      // Generate room code (simple implementation)
      const roomCode = Math.random().toString(36).substring(2, 8).toUpperCase();

      return new Response(
        JSON.stringify({
          roomId: roomResult.data,
          playerId: playerResult.data,
          code: roomCode,
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
