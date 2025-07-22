/// <reference no-default-lib="true" />
/// <reference lib="dom" />
/// <reference lib="dom.iterable" />
/// <reference lib="dom.asynciterable" />
/// <reference lib="deno.ns" />
/// <reference lib="webworker" />

import "$std/dotenv/load.ts";

import { start } from "$fresh/server.ts";
import manifest from "./fresh.gen.ts";
import config from "./fresh.config.ts";
import type { Env } from "./types/cloudflare.ts";
import { getConfig } from "./lib/config.ts";

// Cloudflare Workers entry point
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    // Initialize configuration with environment
    const appConfig = getConfig(env.ENVIRONMENT);
    
    // Add Cloudflare bindings to the request context
    const requestWithEnv = new Request(request.url, {
      method: request.method,
      headers: request.headers,
      body: request.body,
    });
    
    // Add environment bindings to global context for Fresh to access
    (globalThis as any).CF_ENV = env;
    (globalThis as any).APP_CONFIG = appConfig;
    
    try {
      // Handle WebSocket upgrade requests
      if (request.headers.get("upgrade") === "websocket") {
        return handleWebSocket(request, env);
      }
      
      // Handle regular HTTP requests through Fresh
      return await start(manifest, config).fetch(requestWithEnv);
    } catch (error) {
      console.error("Error handling request:", error);
      return new Response("Internal Server Error", { status: 500 });
    }
  },
};

// WebSocket handler for real-time communication
async function handleWebSocket(request: Request, env: Env): Promise<Response> {
  const upgradeHeader = request.headers.get("Upgrade");
  if (upgradeHeader !== "websocket") {
    return new Response("Expected Upgrade: websocket", { status: 426 });
  }

  // Create WebSocket pair
  const webSocketPair = new WebSocketPair();
  const [client, server] = Object.values(webSocketPair);

  // Accept the WebSocket connection
  server.accept();

  // Handle WebSocket events
  server.addEventListener("message", (event) => {
    try {
      const message = JSON.parse(event.data as string);
      console.log("Received WebSocket message:", message);
      
      // Echo back for now (will be replaced with proper game logic)
      server.send(JSON.stringify({
        type: "echo",
        data: message
      }));
    } catch (error) {
      console.error("Error handling WebSocket message:", error);
    }
  });

  server.addEventListener("close", () => {
    console.log("WebSocket connection closed");
  });

  server.addEventListener("error", (error) => {
    console.error("WebSocket error:", error);
  });

  return new Response(null, {
    status: 101,
    webSocket: client,
  });
}

// For local development
if (import.meta.main) {
  await start(manifest, config);
}
