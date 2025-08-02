// Health check endpoint for monitoring

import { Handlers } from "$fresh/server.ts";
import { getDatabaseService, getKVService } from "../../lib/db/index.ts";

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

async function checkDatabase(): Promise<HealthCheck> {
  const startTime = Date.now();
  try {
    const dbService = getDatabaseService();
    const result = await dbService.healthCheck();
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
    // Check if WebSocket is available
    const pair = new WebSocketPair();
    pair[0].close();
    pair[1].close();
    return { status: "ok" };
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
