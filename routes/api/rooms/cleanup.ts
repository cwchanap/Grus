// API endpoint for room cleanup operations
import { Handlers } from "$fresh/server.ts";
import { RoomManager } from "../../../lib/room-manager.ts";

export const handler: Handlers = {
  // DELETE /api/rooms/cleanup - Clean up empty rooms
  async DELETE(req, _ctx) {
    try {
      const url = new URL(req.url);
      const limit = parseInt(url.searchParams.get("limit") || "50");

      const roomManager = new RoomManager();
      const result = await roomManager.cleanupEmptyRooms(limit);

      if (!result.success) {
        return new Response(JSON.stringify({ error: result.error }), {
          status: 500,
          headers: { "Content-Type": "application/json" },
        });
      }

      return new Response(JSON.stringify({ 
        message: `Cleaned up ${result.cleanedCount} empty rooms`,
        cleanedCount: result.cleanedCount 
      }), {
        headers: { "Content-Type": "application/json" },
      });
    } catch (error) {
      console.error("Error cleaning up rooms:", error);
      return new Response(JSON.stringify({ error: "Internal server error" }), {
        status: 500,
        headers: { "Content-Type": "application/json" },
      });
    }
  },

  // GET /api/rooms/cleanup - Get cleanup statistics
  async GET(_req, _ctx) {
    try {
      const roomManager = new RoomManager();
      
      // Perform a dry run to see how many rooms would be cleaned up
      const result = await roomManager.cleanupEmptyRooms(0); // Limit 0 means dry run
      
      return new Response(JSON.stringify({
        message: "Cleanup statistics",
        potentialCleanupCount: result.cleanedCount,
      }), {
        headers: { "Content-Type": "application/json" },
      });
    } catch (error) {
      console.error("Error getting cleanup statistics:", error);
      return new Response(JSON.stringify({ error: "Internal server error" }), {
        status: 500,
        headers: { "Content-Type": "application/json" },
      });
    }
  },
};