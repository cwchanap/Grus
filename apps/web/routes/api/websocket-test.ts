// Simple WebSocket test route for debugging connectivity
import { Handlers } from "$fresh/server.ts";

export const handler: Handlers = {
  GET(req) {
    const upgradeHeader = req.headers.get("Upgrade");
    if (upgradeHeader !== "websocket") {
      return new Response("Expected Upgrade: websocket", { status: 426 });
    }

    try {
      const { socket, response } = Deno.upgradeWebSocket(req);

      socket.addEventListener("open", () => {
        // Send welcome message
        socket.send(JSON.stringify({
          type: "welcome",
          message: "WebSocket test connection established",
          timestamp: Date.now(),
        }));
      });

      socket.addEventListener("message", (event) => {
        try {
          const data = JSON.parse(event.data);

          // Echo back the message with a timestamp
          socket.send(JSON.stringify({
            type: "echo",
            originalMessage: data,
            timestamp: Date.now(),
          }));
        } catch (_error) {
          socket.send(JSON.stringify({
            type: "error",
            message: "Invalid JSON",
            timestamp: Date.now(),
          }));
        }
      });

      socket.addEventListener("close", () => {
        // Connection closed
      });

      socket.addEventListener("error", (error) => {
        console.error("WebSocket test error:", error);
      });

      return response;
    } catch (error) {
      console.error("Failed to upgrade WebSocket:", error);
      return new Response("WebSocket upgrade failed", {
        status: 500,
        headers: { "Content-Type": "text/plain" },
      });
    }
  },
};
