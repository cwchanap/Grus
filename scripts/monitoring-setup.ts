#!/usr/bin/env -S deno run -A

// Monitoring and alerting setup for Cloudflare Workers

interface MonitoringConfig {
  environment: 'development' | 'production';
  webhookUrl?: string;
  alertThresholds: {
    errorRate: number;
    responseTime: number;
    memoryUsage: number;
  };
}

async function setupCloudflareAnalytics(config: MonitoringConfig): Promise<void> {
  console.log("📊 Setting up Cloudflare Analytics...");
  
  // This would typically involve API calls to configure Cloudflare Analytics
  // For now, we'll create the configuration that can be applied manually
  
  const analyticsConfig = {
    environment: config.environment,
    metrics: [
      'requests_per_minute',
      'error_rate',
      'response_time_p95',
      'memory_usage',
      'cpu_usage'
    ],
    alerts: [
      {
        name: `High Error Rate - ${config.environment}`,
        condition: `error_rate > ${config.alertThresholds.errorRate}`,
        action: 'webhook',
        webhook: config.webhookUrl
      },
      {
        name: `High Response Time - ${config.environment}`,
        condition: `response_time_p95 > ${config.alertThresholds.responseTime}`,
        action: 'webhook',
        webhook: config.webhookUrl
      },
      {
        name: `High Memory Usage - ${config.environment}`,
        condition: `memory_usage > ${config.alertThresholds.memoryUsage}`,
        action: 'webhook',
        webhook: config.webhookUrl
      }
    ]
  };
  
  // Write configuration to file for manual setup
  await Deno.writeTextFile(
    `monitoring-config-${config.environment}.json`,
    JSON.stringify(analyticsConfig, null, 2)
  );
  
  console.log(`✅ Analytics configuration written to monitoring-config-${config.environment}.json`);
  console.log("📋 Manual setup required:");
  console.log("1. Go to Cloudflare Dashboard > Analytics & Logs");
  console.log("2. Configure the alerts using the generated configuration");
  console.log("3. Set up webhook notifications");
}

async function createHealthCheckEndpoint(): Promise<void> {
  console.log("🏥 Creating health check endpoint...");
  
  const healthCheckCode = `
// Health check endpoint for monitoring
export async function handleHealthCheck(request: Request): Promise<Response> {
  const startTime = Date.now();
  
  try {
    // Check database connectivity
    const dbCheck = await checkDatabase();
    
    // Check KV storage
    const kvCheck = await checkKVStorage();
    
    // Check WebSocket capability
    const wsCheck = await checkWebSocketSupport();
    
    const responseTime = Date.now() - startTime;
    
    const health = {
      status: 'ok',
      timestamp: new Date().toISOString(),
      environment: Deno.env.get('ENVIRONMENT') || 'unknown',
      checks: {
        database: dbCheck,
        kv_storage: kvCheck,
        websocket: wsCheck
      },
      performance: {
        response_time_ms: responseTime,
        memory_usage: getMemoryUsage()
      }
    };
    
    return new Response(JSON.stringify(health), {
      headers: { 'Content-Type': 'application/json' },
      status: 200
    });
    
  } catch (error) {
    const responseTime = Date.now() - startTime;
    
    const health = {
      status: 'error',
      timestamp: new Date().toISOString(),
      environment: Deno.env.get('ENVIRONMENT') || 'unknown',
      error: error.message,
      performance: {
        response_time_ms: responseTime
      }
    };
    
    return new Response(JSON.stringify(health), {
      headers: { 'Content-Type': 'application/json' },
      status: 503
    });
  }
}

async function checkDatabase(): Promise<{ status: string; latency?: number }> {
  const startTime = Date.now();
  try {
    // Simple query to check database connectivity
    const result = await DB.prepare("SELECT 1 as test").first();
    const latency = Date.now() - startTime;
    return { status: 'ok', latency };
  } catch (error) {
    return { status: 'error', error: error.message };
  }
}

async function checkKVStorage(): Promise<{ status: string; latency?: number }> {
  const startTime = Date.now();
  try {
    // Test KV read/write
    const testKey = 'health-check-' + Date.now();
    await GAME_STATE.put(testKey, 'test', { expirationTtl: 60 });
    await GAME_STATE.get(testKey);
    await GAME_STATE.delete(testKey);
    const latency = Date.now() - startTime;
    return { status: 'ok', latency };
  } catch (error) {
    return { status: 'error', error: error.message };
  }
}

async function checkWebSocketSupport(): Promise<{ status: string }> {
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

function getMemoryUsage(): number {
  // This is a placeholder - actual memory usage would need to be tracked
  // differently in Cloudflare Workers
  return 0;
}
`;
  
  await Deno.writeTextFile('routes/api/health.ts', healthCheckCode);
  console.log("✅ Health check endpoint created at routes/api/health.ts");
}

async function setupLogAggregation(config: MonitoringConfig): Promise<void> {
  console.log("📝 Setting up log aggregation...");
  
  const logConfig = {
    environment: config.environment,
    logLevel: config.environment === 'production' ? 'warn' : 'info',
    destinations: [
      {
        type: 'cloudflare_analytics',
        enabled: true
      },
      {
        type: 'webhook',
        url: config.webhookUrl,
        enabled: !!config.webhookUrl,
        filter: 'error'
      }
    ]
  };
  
  await Deno.writeTextFile(
    `log-config-${config.environment}.json`,
    JSON.stringify(logConfig, null, 2)
  );
  
  console.log(`✅ Log configuration written to log-config-${config.environment}.json`);
}

async function main(): Promise<void> {
  const args = Deno.args;
  const environment = args.includes('--prod') ? 'production' : 'development';
  const webhookUrl = Deno.env.get('SLACK_WEBHOOK_URL');
  
  const config: MonitoringConfig = {
    environment,
    webhookUrl,
    alertThresholds: {
      errorRate: environment === 'production' ? 5 : 10, // percentage
      responseTime: environment === 'production' ? 1000 : 2000, // milliseconds
      memoryUsage: 80 // percentage
    }
  };
  
  console.log(`🔧 Setting up monitoring for ${environment} environment`);
  
  try {
    await setupCloudflareAnalytics(config);
    await createHealthCheckEndpoint();
    await setupLogAggregation(config);
    
    console.log("✅ Monitoring setup completed!");
    console.log("\n📋 Next steps:");
    console.log("1. Review generated configuration files");
    console.log("2. Apply Cloudflare Analytics settings manually");
    console.log("3. Configure webhook URLs for alerts");
    console.log("4. Test health check endpoint after deployment");
    
  } catch (error) {
    console.error(`❌ Monitoring setup failed: ${error.message}`);
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}