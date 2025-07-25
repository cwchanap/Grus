// Health check endpoint for monitoring

import { HandlerContext } from "$fresh/server.ts";

interface HealthCheck {
  status: string;
  latency?: number;
  error?: string;
}

interface HealthResponse {
  status: 'ok' | 'error';
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

async function checkDatabase(ctx: HandlerContext): Promise<HealthCheck> {
  const startTime = Date.now();
  try {
    // Simple query to check database connectivity
    const result = await ctx.state.db.prepare("SELECT 1 as test").first();
    const latency = Date.now() - startTime;
    return { status: 'ok', latency };
  } catch (error) {
    return { status: 'error', error: error.message };
  }
}

async function checkKVStorage(ctx: HandlerContext): Promise<HealthCheck> {
  const startTime = Date.now();
  try {
    // Test KV read/write
    const testKey = 'health-check-' + Date.now();
    const testValue = 'test-' + Math.random();
    
    await ctx.state.gameState.put(testKey, testValue, { expirationTtl: 60 });
    const retrieved = await ctx.state.gameState.get(testKey);
    await ctx.state.gameState.delete(testKey);
    
    if (retrieved !== testValue) {
      throw new Error('KV read/write mismatch');
    }
    
    const latency = Date.now() - startTime;
    return { status: 'ok', latency };
  } catch (error) {
    return { status: 'error', error: error.message };
  }
}

async function checkWebSocketSupport(): Promise<HealthCheck> {
  try {
    // Check if WebSocket is available
    const pair = new WebSocketPair();
    pair[0].close();
    pair[1].close();
    return { status: 'ok' };
  } catch (error) {
    return { status: 'error', error: error.message };
  }
}

export async function handler(
  req: Request,
  ctx: HandlerContext,
): Promise<Response> {
  const startTime = Date.now();
  
  try {
    // Run all health checks in parallel
    const [dbCheck, kvCheck, wsCheck] = await Promise.all([
      checkDatabase(ctx),
      checkKVStorage(ctx),
      checkWebSocketSupport(),
    ]);
    
    const responseTime = Date.now() - startTime;
    
    // Determine overall status
    const allChecks = [dbCheck, kvCheck, wsCheck];
    const hasErrors = allChecks.some(check => check.status === 'error');
    
    const health: HealthResponse = {
      status: hasErrors ? 'error' : 'ok',
      timestamp: new Date().toISOString(),
      environment: Deno.env.get('ENVIRONMENT') || 'unknown',
      checks: {
        database: dbCheck,
        kv_storage: kvCheck,
        websocket: wsCheck,
      },
      performance: {
        response_time_ms: responseTime,
        uptime_seconds: Math.floor(Date.now() / 1000), // Placeholder for actual uptime
      },
      version: Deno.env.get('VERSION') || 'unknown',
    };
    
    const statusCode = hasErrors ? 503 : 200;
    
    return new Response(JSON.stringify(health, null, 2), {
      headers: { 
        'Content-Type': 'application/json',
        'Cache-Control': 'no-cache, no-store, must-revalidate',
      },
      status: statusCode,
    });
    
  } catch (error) {
    const responseTime = Date.now() - startTime;
    
    const health: HealthResponse = {
      status: 'error',
      timestamp: new Date().toISOString(),
      environment: Deno.env.get('ENVIRONMENT') || 'unknown',
      checks: {
        database: { status: 'error', error: 'Health check failed' },
        kv_storage: { status: 'error', error: 'Health check failed' },
        websocket: { status: 'error', error: 'Health check failed' },
      },
      performance: {
        response_time_ms: responseTime,
      },
    };
    
    return new Response(JSON.stringify(health, null, 2), {
      headers: { 
        'Content-Type': 'application/json',
        'Cache-Control': 'no-cache, no-store, must-revalidate',
      },
      status: 503,
    });
  }
}