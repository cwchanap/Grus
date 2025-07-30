// WebSocket API route for handling WebSocket connections
import { Handlers } from "$fresh/server.ts";
import { WebSocketManager } from "../../lib/websocket/websocket-manager.ts";
import { Env } from "../../types/cloudflare.ts";

// Global WebSocket manager instance
let wsManager: WebSocketManager | null = null;

function getWebSocketManager(env: Env): WebSocketManager {
  if (!wsManager) {
    wsManager = new WebSocketManager(env);
  }
  return wsManager;
}

export const handler: Handlers = {
  GET(req, ctx) {
    // Get Cloudflare environment from context
    const env = (ctx.state as any).env as Env;

    // In development mode, create a mock environment
    const mockEnv: Env = env || {
      DB: null as any,
      GAME_STATE: null as any,
    };

    console.log("WebSocket connection request received", {
      url: req.url,
      hasEnv: !!env,
      isDevelopment: !env
    });

    const manager = getWebSocketManager(mockEnv);
    return manager.handleRequest(req);
  },

  POST(req, ctx) {
    // Handle WebSocket-related POST requests (if needed)
    return new Response("Method not allowed", { status: 405 });
  },
};
