// API route for game type operations
import { Handlers } from "$fresh/server.ts";
import { GameRegistry } from "../../lib/core/game-registry.ts";
import "../../lib/games/index.ts"; // Ensure games are registered

export const handler: Handlers = {
  // GET /api/games - List available game types
  GET(_req, _ctx) {
    try {
      const gameRegistry = GameRegistry.getInstance();
      const gameTypes = gameRegistry.getAllGameTypes();

      return new Response(JSON.stringify({ gameTypes }), {
        headers: { "Content-Type": "application/json" },
      });
    } catch (error) {
      console.error("Error listing game types:", error);
      return new Response(JSON.stringify({ error: "Internal server error" }), {
        status: 500,
        headers: { "Content-Type": "application/json" },
      });
    }
  },
};
