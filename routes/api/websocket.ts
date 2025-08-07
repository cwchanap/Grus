// WebSocket API route using the new core handler
import { Handlers } from "$fresh/server.ts";
import { CoreWebSocketHandler } from "../../lib/core/websocket-handler.ts";
import "../../lib/games/index.ts"; // Ensure games are registered

// Global WebSocket handler instance
let wsHandler: CoreWebSocketHandler | null = null;

function getWebSocketHandler(): CoreWebSocketHandler {
  if (!wsHandler) {
    wsHandler = new CoreWebSocketHandler();
  }
  return wsHandler;
}

export const handler: Handlers = {
  GET(req, _ctx) {
    const handler = getWebSocketHandler();
    return handler.handleWebSocketUpgrade(req);
  },

  POST(_req, _ctx) {
    return new Response("Method not allowed", { status: 405 });
  },
};
