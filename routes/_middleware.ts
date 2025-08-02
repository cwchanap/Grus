import { MiddlewareHandlerContext } from "$fresh/server.ts";

export async function handler(
  req: Request,
  ctx: MiddlewareHandlerContext,
) {
  // Skip CORS headers for WebSocket upgrade requests
  if (req.headers.get("Upgrade") === "websocket") {
    return await ctx.next();
  }

  // Add CORS headers for API routes
  if (ctx.destination === "route" && req.url.includes("/api/")) {
    const response = await ctx.next();

    // Skip CORS headers if response has immutable headers (like WebSocket upgrades)
    try {
      response.headers.set("Access-Control-Allow-Origin", "*");
      response.headers.set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS");
      response.headers.set("Access-Control-Allow-Headers", "Content-Type, Authorization");
    } catch (_error) {
      // Headers are immutable (e.g., WebSocket upgrade response), skip CORS
      console.log("Skipping CORS headers for immutable response");
    }

    return response;
  }

  // Handle preflight requests
  if (req.method === "OPTIONS") {
    return new Response(null, {
      status: 200,
      headers: {
        "Access-Control-Allow-Origin": "*",
        "Access-Control-Allow-Methods": "GET, POST, PUT, DELETE, OPTIONS",
        "Access-Control-Allow-Headers": "Content-Type, Authorization",
      },
    });
  }

  return await ctx.next();
}
