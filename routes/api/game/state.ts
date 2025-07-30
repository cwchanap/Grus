import { Handlers } from "$fresh/server.ts";
import { GameState } from "../../../types/game.ts";
import { Env } from "../../../types/cloudflare.ts";

interface GameStateRequest {
  roomId: string;
}

interface GameStateResponse {
  success: boolean;
  error?: string;
  gameState?: GameState;
}

export const handler: Handlers = {
  async POST(req, ctx) {
    try {
      const body: GameStateRequest = await req.json();
      const { roomId } = body;

      if (!roomId) {
        return new Response(
          JSON.stringify({
            success: false,
            error: "Room ID is required",
          }),
          {
            status: 400,
            headers: { "Content-Type": "application/json" },
          }
        );
      }

      // Try to get game state from KV store
      const env = (ctx.state as any).env as Env;
      let gameState: GameState | null = null;

      if (env?.GAME_STATE) {
        try {
          const gameStateStr = await env.GAME_STATE.get(`game:${roomId}`);
          if (gameStateStr) {
            gameState = JSON.parse(gameStateStr);
          }
        } catch (error) {
          console.error("Error getting game state from KV:", error);
        }
      }

      if (!gameState) {
        return new Response(
          JSON.stringify({
            success: false,
            error: "Game state not found",
          }),
          {
            status: 404,
            headers: { "Content-Type": "application/json" },
          }
        );
      }

      return new Response(
        JSON.stringify({
          success: true,
          gameState,
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }
      );

    } catch (error) {
      console.error("Error getting game state via REST API:", error);
      return new Response(
        JSON.stringify({
          success: false,
          error: "Internal server error",
        }),
        {
          status: 500,
          headers: { "Content-Type": "application/json" },
        }
      );
    }
  },
};