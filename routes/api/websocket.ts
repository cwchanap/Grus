// WebSocket API route using the new core handler
import { Handlers } from "$fresh/server.ts";
import { CoreWebSocketHandler } from "../../lib/core/websocket-handler.ts";
import "../../lib/games/index.ts"; // Ensure games are registered

// Global WebSocket handler instance pinned on globalThis to survive dev HMR/module reloads
function getWebSocketHandler(): CoreWebSocketHandler {
  const g = globalThis as unknown as { __WS_HANDLER__?: CoreWebSocketHandler };
  if (!g.__WS_HANDLER__) {
    g.__WS_HANDLER__ = new CoreWebSocketHandler();
  }
  return g.__WS_HANDLER__;
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
