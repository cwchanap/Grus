// Health check endpoint for monitoring

import { Handlers } from "$fresh/server.ts";
import { getKVService } from "../../lib/db/index.ts";
import { RoomManager } from "../../lib/core/room-manager.ts";

interface HealthCheck {
  status: string;
  latency?: number;
  error?: string;
}

interface HealthResponse {
  status: "ok" | "error";
  timestamp: string;
  environment: string;
  checks: {
    database: HealthCheck;
    kv_storage: HealthCheck;
    websocket: HealthCheck;
  };
  performance: {
    response_time_ms: number;
    uptime_seconds?: number;
  };
  version?: string;
}

// Primary database check now uses RoomManager over KV to ensure the main
// application storage path is healthy. This eliminates the legacy SQLite check.
async function checkDatabase(): Promise<HealthCheck> {
  const startTime = Date.now();
  try {
    const rm = new RoomManager();
    // Exercise a lightweight read path with built-in cleanup
    const result = await rm.getActiveRoomsWithCleanup(1);
    const latency = Date.now() - startTime;

    return {
      status: result.success ? "ok" : "error",
      latency,
      error: result.error,
    };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown error",
    };
  }
}

async function checkKVStorage(): Promise<HealthCheck> {
  const startTime = Date.now();
  try {
    const kvService = getKVService();
    const result = await kvService.healthCheck();
    const latency = Date.now() - startTime;

    return {
      status: result.success ? "ok" : "error",
      latency,
      error: result.error,
    };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown error",
    };
  }
}

function checkWebSocketSupport(): HealthCheck {
  try {
    // Prefer Deno's server-side upgrade API
    if (typeof Deno !== "undefined" && typeof Deno.upgradeWebSocket === "function") {
      return { status: "ok" };
    }
    // Fallback: presence of WebSocket constructor in global scope
    if (typeof WebSocket !== "undefined") {
      return { status: "ok" };
    }
    return { status: "error", error: "WebSocket not supported in this runtime" };
  } catch (error) {
    return {
      status: "error",
      error: error instanceof Error ? error.message : "Unknown error",
    };
  }
}

export const handler: Handlers<HealthResponse> = {
  async GET(_req, _ctx) {
    const startTime = Date.now();

    try {
      // Run all health checks in parallel
      const [dbCheck, kvCheck, wsCheck] = await Promise.all([
        checkDatabase(),
        checkKVStorage(),
        checkWebSocketSupport(),
      ]);

      const responseTime = Date.now() - startTime;

      // Determine overall status
      const allChecks = [dbCheck, kvCheck, wsCheck];
      const hasErrors = allChecks.some((check) => check.status === "error");

      const health: HealthResponse = {
        status: hasErrors ? "error" : "ok",
        timestamp: new Date().toISOString(),
        environment: Deno.env.get("ENVIRONMENT") || "unknown",
        checks: {
          database: dbCheck,
          kv_storage: kvCheck,
          websocket: wsCheck,
        },
        performance: {
          response_time_ms: responseTime,
          uptime_seconds: Math.floor(Date.now() / 1000), // Placeholder for actual uptime
        },
        version: Deno.env.get("VERSION") || "unknown",
      };

      const statusCode = hasErrors ? 503 : 200;

      return new Response(JSON.stringify(health, null, 2), {
        headers: {
          "Content-Type": "application/json",
          "Cache-Control": "no-cache, no-store, must-revalidate",
        },
        status: statusCode,
      });
    } catch (_error) {
      const responseTime = Date.now() - startTime;

      const health: HealthResponse = {
        status: "error",
        timestamp: new Date().toISOString(),
        environment: Deno.env.get("ENVIRONMENT") || "unknown",
        checks: {
          database: { status: "error", error: "Health check failed" },
          kv_storage: { status: "error", error: "Health check failed" },
          websocket: { status: "error", error: "Health check failed" },
        },
        performance: {
          response_time_ms: responseTime,
        },
      };

      return new Response(JSON.stringify(health, null, 2), {
        headers: {
          "Content-Type": "application/json",
          "Cache-Control": "no-cache, no-store, must-revalidate",
        },
        status: 503,
      });
    }
  },
};
