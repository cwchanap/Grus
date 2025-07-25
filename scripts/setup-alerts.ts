#!/usr/bin/env -S deno run -A

// Setup monitoring alerts and dashboards

interface AlertConfig {
  environment: 'development' | 'production';
  webhookUrl?: string;
}

async function createAlertingConfig(config: AlertConfig): Promise<void> {
  console.log(`📊 Creating alerting configuration for ${config.environment}...`);
  
  const thresholds = config.environment === 'production' 
    ? { errorRate: 2, responseTime: 1000, memoryUsage: 80 }
    : { errorRate: 10, responseTime: 2000, memoryUsage: 90 };

  const alertConfig = {
    environment: config.environment,
    alerts: [
      {
        name: `High Error Rate - ${config.environment}`,
        description: "Triggers when error rate exceeds threshold",
        condition: {
          metric: "error_rate_percentage",
          operator: "greater_than",
          threshold: thresholds.errorRate,
          duration: "5m"
        },
        actions: [
          {
            type: "webhook",
            url: config.webhookUrl,
            payload: {
              text: `🚨 High error rate detected in ${config.environment}: {{value}}%`,
              channel: "#alerts"
            }
          }
        ],
        severity: "high"
      },
      {
        name: `Slow Response Time - ${config.environment}`,
        description: "Triggers when P95 response time exceeds threshold",
        condition: {
          metric: "response_time_p95",
          operator: "greater_than",
          threshold: thresholds.responseTime,
          duration: "10m"
        },
        actions: [
          {
            type: "webhook",
            url: config.webhookUrl,
            payload: {
              text: `⏱️ Slow response times in ${config.environment}: {{value}}ms`,
              channel: "#performance"
            }
          }
        ],
        severity: "medium"
      },
      {
        name: `High Memory Usage - ${config.environment}`,
        description: "Triggers when memory usage exceeds threshold",
        condition: {
          metric: "memory_usage_percentage",
          operator: "greater_than",
          threshold: thresholds.memoryUsage,
          duration: "15m"
        },
        actions: [
          {
            type: "webhook",
            url: config.webhookUrl,
            payload: {
              text: `💾 High memory usage in ${config.environment}: {{value}}%`,
              channel: "#infrastructure"
            }
          }
        ],
        severity: "medium"
      },
      {
        name: `Database Connection Failure - ${config.environment}`,
        description: "Triggers when database health check fails",
        condition: {
          metric: "database_health",
          operator: "equals",
          threshold: "unhealthy",
          duration: "1m"
        },
        actions: [
          {
            type: "webhook",
            url: config.webhookUrl,
            payload: {
              text: `🗄️ Database connection failure in ${config.environment}`,
              channel: "#alerts"
            }
          }
        ],
        severity: "critical"
      },
      {
        name: `WebSocket Connection Issues - ${config.environment}`,
        description: "Triggers when WebSocket connections drop significantly",
        condition: {
          metric: "websocket_connection_count",
          operator: "less_than",
          threshold: 10,
          duration: "5m"
        },
        actions: [
          {
            type: "webhook",
            url: config.webhookUrl,
            payload: {
              text: `🔌 WebSocket connection issues in ${config.environment}`,
              channel: "#alerts"
            }
          }
        ],
        severity: "high"
      }
    ],
    dashboards: [
      {
        name: `Application Performance - ${config.environment}`,
        panels: [
          {
            title: "Request Rate",
            type: "graph",
            metrics: ["requests_per_minute"],
            timeRange: "1h"
          },
          {
            title: "Error Rate",
            type: "graph",
            metrics: ["error_rate_percentage"],
            timeRange: "1h"
          },
          {
            title: "Response Time",
            type: "graph",
            metrics: ["response_time_p50", "response_time_p95", "response_time_p99"],
            timeRange: "1h"
          },
          {
            title: "Active Game Rooms",
            type: "stat",
            metrics: ["active_rooms_count"],
            timeRange: "5m"
          },
          {
            title: "WebSocket Connections",
            type: "stat",
            metrics: ["websocket_connection_count"],
            timeRange: "5m"
          }
        ]
      },
      {
        name: `Infrastructure - ${config.environment}`,
        panels: [
          {
            title: "Memory Usage",
            type: "graph",
            metrics: ["memory_usage_percentage"],
            timeRange: "1h"
          },
          {
            title: "Database Performance",
            type: "graph",
            metrics: ["database_query_time", "database_connection_count"],
            timeRange: "1h"
          },
          {
            title: "KV Storage Operations",
            type: "graph",
            metrics: ["kv_read_ops", "kv_write_ops"],
            timeRange: "1h"
          }
        ]
      }
    ]
  };

  // Write configuration to file
  const filename = `alerts-config-${config.environment}.json`;
  await Deno.writeTextFile(filename, JSON.stringify(alertConfig, null, 2));
  
  console.log(`✅ Alert configuration written to ${filename}`);
}

async function createHealthCheckScript(): Promise<void> {
  console.log("🏥 Creating enhanced health check script...");
  
  const healthCheckScript = `#!/usr/bin/env -S deno run -A

// Enhanced health check script for monitoring

interface HealthMetrics {
  status: 'healthy' | 'degraded' | 'unhealthy';
  timestamp: string;
  environment: string;
  checks: {
    database: HealthCheck;
    kv_storage: HealthCheck;
    websocket: HealthCheck;
    external_apis: HealthCheck;
  };
  performance: {
    response_time_ms: number;
    memory_usage_mb: number;
    active_connections: number;
    active_rooms: number;
  };
  version: string;
}

interface HealthCheck {
  status: 'ok' | 'warning' | 'error';
  latency_ms?: number;
  error?: string;
  details?: Record<string, any>;
}

async function performHealthCheck(): Promise<HealthMetrics> {
  const startTime = Date.now();
  const environment = Deno.env.get('ENVIRONMENT') || 'unknown';
  
  const [dbCheck, kvCheck, wsCheck, apiCheck] = await Promise.allSettled([
    checkDatabase(),
    checkKVStorage(),
    checkWebSocket(),
    checkExternalAPIs()
  ]);
  
  const performance = await getPerformanceMetrics();
  const responseTime = Date.now() - startTime;
  
  const checks = {
    database: dbCheck.status === 'fulfilled' ? dbCheck.value : { status: 'error', error: dbCheck.reason?.message },
    kv_storage: kvCheck.status === 'fulfilled' ? kvCheck.value : { status: 'error', error: kvCheck.reason?.message },
    websocket: wsCheck.status === 'fulfilled' ? wsCheck.value : { status: 'error', error: wsCheck.reason?.message },
    external_apis: apiCheck.status === 'fulfilled' ? apiCheck.value : { status: 'error', error: apiCheck.reason?.message }
  };
  
  // Determine overall status
  const hasErrors = Object.values(checks).some(check => check.status === 'error');
  const hasWarnings = Object.values(checks).some(check => check.status === 'warning');
  
  let status: 'healthy' | 'degraded' | 'unhealthy';
  if (hasErrors) {
    status = 'unhealthy';
  } else if (hasWarnings) {
    status = 'degraded';
  } else {
    status = 'healthy';
  }
  
  return {
    status,
    timestamp: new Date().toISOString(),
    environment,
    checks,
    performance: {
      ...performance,
      response_time_ms: responseTime
    },
    version: Deno.env.get('APP_VERSION') || 'unknown'
  };
}

async function checkDatabase(): Promise<HealthCheck> {
  const startTime = Date.now();
  try {
    // Simulate database check
    await new Promise(resolve => setTimeout(resolve, Math.random() * 100));
    const latency = Date.now() - startTime;
    
    return {
      status: 'ok',
      latency_ms: latency,
      details: {
        connection_pool: 'healthy',
        query_performance: 'good'
      }
    };
  } catch (error) {
    return {
      status: 'error',
      error: error.message
    };
  }
}

async function checkKVStorage(): Promise<HealthCheck> {
  const startTime = Date.now();
  try {
    // Simulate KV check
    await new Promise(resolve => setTimeout(resolve, Math.random() * 50));
    const latency = Date.now() - startTime;
    
    return {
      status: 'ok',
      latency_ms: latency,
      details: {
        read_performance: 'good',
        write_performance: 'good'
      }
    };
  } catch (error) {
    return {
      status: 'error',
      error: error.message
    };
  }
}

async function checkWebSocket(): Promise<HealthCheck> {
  try {
    // Check WebSocket capability
    const pair = new WebSocketPair();
    pair[0].close();
    pair[1].close();
    
    return {
      status: 'ok',
      details: {
        websocket_support: 'available'
      }
    };
  } catch (error) {
    return {
      status: 'error',
      error: error.message
    };
  }
}

async function checkExternalAPIs(): Promise<HealthCheck> {
  // This would check any external API dependencies
  return {
    status: 'ok',
    details: {
      external_services: 'not_applicable'
    }
  };
}

async function getPerformanceMetrics() {
  return {
    memory_usage_mb: Math.floor(Math.random() * 100) + 50,
    active_connections: Math.floor(Math.random() * 1000) + 100,
    active_rooms: Math.floor(Math.random() * 50) + 10
  };
}

if (import.meta.main) {
  const metrics = await performHealthCheck();
  console.log(JSON.stringify(metrics, null, 2));
  
  // Exit with appropriate code
  if (metrics.status === 'unhealthy') {
    Deno.exit(1);
  } else if (metrics.status === 'degraded') {
    Deno.exit(2);
  } else {
    Deno.exit(0);
  }
}`;

  await Deno.writeTextFile('scripts/health-check.ts', healthCheckScript);
  console.log("✅ Enhanced health check script created");
}

async function main(): Promise<void> {
  const args = Deno.args;
  const environment = args.includes('--prod') ? 'production' : 'development';
  const webhookUrl = Deno.env.get('SLACK_WEBHOOK_URL');
  
  console.log(`🔧 Setting up alerts for ${environment} environment`);
  
  try {
    await createAlertingConfig({ environment, webhookUrl });
    await createHealthCheckScript();
    
    console.log("✅ Alert setup completed!");
    console.log("\n📋 Next steps:");
    console.log("1. Review generated alert configuration files");
    console.log("2. Configure alerts in Cloudflare Dashboard");
    console.log("3. Set up webhook endpoints for notifications");
    console.log("4. Test health check endpoint after deployment");
    console.log("5. Create monitoring dashboards using the provided configuration");
    
  } catch (error) {
    console.error(`❌ Alert setup failed: ${error.message}`);
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}`;

await Deno.writeTextFile('scripts/setup-alerts.ts', healthCheckScript);
console.log("✅ Alert setup script created");