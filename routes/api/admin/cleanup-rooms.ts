import { Handlers } from "$fresh/server.ts";
import { RoomManager } from "../../../lib/core/room-manager.ts";

export const handler: Handlers = {
  async POST(_req) {
    const isDev = Deno.env.get("DENO_ENV") !== "production";
    if (!isDev) {
      return new Response(JSON.stringify({ error: "Forbidden" }), {
        status: 403,
        headers: { "Content-Type": "application/json" },
      });
    }

    try {
      const roomManager = new RoomManager();
      // Purge the entire rooms table (dev-only)
      const result = roomManager.purgeAllRooms();

      if (!result.success) {
        return new Response(JSON.stringify({ error: result.error || "Cleanup failed" }), {
          status: 500,
          headers: { "Content-Type": "application/json" },
        });
      }

      return new Response(
        JSON.stringify({ success: true, cleanedCount: result.cleanedCount }),
        { headers: { "Content-Type": "application/json" } },
      );
    } catch (error) {
      console.error("Error during cleanup:", error);
      return new Response(JSON.stringify({ error: "Internal server error" }), {
        status: 500,
        headers: { "Content-Type": "application/json" },
      });
    }
  },

  // Disallow other methods explicitly
  async GET() {
    return new Response(JSON.stringify({ error: "Method Not Allowed" }), {
      status: 405,
      headers: { "Content-Type": "application/json" },
    });
  },
};
