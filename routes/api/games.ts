// API route for game type operations
import { Handlers } from "$fresh/server.ts";
import { GameRegistry } from "../../lib/core/game-registry.ts";
import "../../lib/games/index.ts"; // Ensure games are registered

export const handler: Handlers = {
  // GET /api/games - List available game types
  async GET(_req, _ctx) {
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

  // GET /api/games/:gameType - Get game type info
  async GET(req, ctx) {
    try {
      const url = new URL(req.url);
      const gameType = url.pathname.split("/").pop();

      if (!gameType) {
        return new Response(JSON.stringify({ error: "Game type is required" }), {
          status: 400,
          headers: { "Content-Type": "application/json" },
        });
      }

      const gameRegistry = GameRegistry.getInstance();
      const gameInfo = gameRegistry.getGameType(gameType);

      if (!gameInfo) {
        return new Response(JSON.stringify({ error: "Game type not found" }), {
          status: 404,
          headers: { "Content-Type": "application/json" },
        });
      }

      return new Response(JSON.stringify({ gameInfo }), {
        headers: { "Content-Type": "application/json" },
      });
    } catch (error) {
      console.error("Error getting game type info:", error);
      return new Response(JSON.stringify({ error: "Internal server error" }), {
        status: 500,
        headers: { "Content-Type": "application/json" },
      });
    }
  },
};
