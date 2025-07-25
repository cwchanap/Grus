#!/usr/bin/env -S deno run -A

// Smoke tests for post-deployment validation

interface SmokeTestResult {
  name: string;
  success: boolean;
  error?: string;
  duration: number;
}

class SmokeTestRunner {
  private baseUrl: string;
  private results: SmokeTestResult[] = [];

  constructor(baseUrl: string) {
    this.baseUrl = baseUrl;
  }

  async runTest(name: string, testFn: () => Promise<void>): Promise<void> {
    const startTime = Date.now();
    console.log(`🧪 Running test: ${name}`);
    
    try {
      await testFn();
      const duration = Date.now() - startTime;
      this.results.push({ name, success: true, duration });
      console.log(`✅ ${name} - ${duration}ms`);
    } catch (error) {
      const duration = Date.now() - startTime;
      this.results.push({ 
        name, 
        success: false, 
        error: error.message, 
        duration 
      });
      console.log(`❌ ${name} - ${error.message} - ${duration}ms`);
    }
  }

  async testHealthCheck(): Promise<void> {
    const response = await fetch(`${this.baseUrl}/api/health`);
    if (!response.ok) {
      throw new Error(`Health check failed: ${response.status}`);
    }
    const data = await response.json();
    if (data.status !== 'ok') {
      throw new Error(`Health check returned: ${data.status}`);
    }
  }

  async testLobbyPage(): Promise<void> {
    const response = await fetch(this.baseUrl);
    if (!response.ok) {
      throw new Error(`Lobby page failed: ${response.status}`);
    }
    const html = await response.text();
    if (!html.includes('Game Lobby')) {
      throw new Error('Lobby page does not contain expected content');
    }
  }

  async testRoomCreation(): Promise<void> {
    const response = await fetch(`${this.baseUrl}/api/rooms`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        name: 'Smoke Test Room',
        hostName: 'Test Host'
      })
    });
    
    if (!response.ok) {
      throw new Error(`Room creation failed: ${response.status}`);
    }
    
    const data = await response.json();
    if (!data.roomId) {
      throw new Error('Room creation did not return roomId');
    }
  }

  async testDatabaseConnection(): Promise<void> {
    const response = await fetch(`${this.baseUrl}/api/rooms`);
    if (!response.ok) {
      throw new Error(`Database connection test failed: ${response.status}`);
    }
    
    const data = await response.json();
    if (!Array.isArray(data.rooms)) {
      throw new Error('Database query did not return expected format');
    }
  }

  async testWebSocketEndpoint(): Promise<void> {
    // Test that WebSocket endpoint is available
    const wsUrl = this.baseUrl.replace('https://', 'wss://').replace('http://', 'ws://');
    
    try {
      const ws = new WebSocket(`${wsUrl}/api/ws`);
      
      await new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          ws.close();
          reject(new Error('WebSocket connection timeout'));
        }, 5000);
        
        ws.onopen = () => {
          clearTimeout(timeout);
          ws.close();
          resolve(void 0);
        };
        
        ws.onerror = (error) => {
          clearTimeout(timeout);
          reject(new Error('WebSocket connection failed'));
        };
      });
    } catch (error) {
      throw new Error(`WebSocket test failed: ${error.message}`);
    }
  }

  async runAllTests(): Promise<boolean> {
    console.log(`🚀 Starting smoke tests for ${this.baseUrl}`);
    console.log('=' .repeat(50));

    await this.runTest('Health Check', () => this.testHealthCheck());
    await this.runTest('Lobby Page', () => this.testLobbyPage());
    await this.runTest('Room Creation', () => this.testRoomCreation());
    await this.runTest('Database Connection', () => this.testDatabaseConnection());
    await this.runTest('WebSocket Endpoint', () => this.testWebSocketEndpoint());

    console.log('=' .repeat(50));
    console.log('📊 Test Results:');
    
    const passed = this.results.filter(r => r.success).length;
    const failed = this.results.filter(r => !r.success).length;
    const totalDuration = this.results.reduce((sum, r) => sum + r.duration, 0);
    
    console.log(`✅ Passed: ${passed}`);
    console.log(`❌ Failed: ${failed}`);
    console.log(`⏱️  Total Duration: ${totalDuration}ms`);
    
    if (failed > 0) {
      console.log('\n❌ Failed Tests:');
      this.results
        .filter(r => !r.success)
        .forEach(r => console.log(`  - ${r.name}: ${r.error}`));
    }
    
    return failed === 0;
  }
}

async function main() {
  const args = Deno.args;
  const envArg = args.find(arg => arg.startsWith('--env='));
  const environment = envArg ? envArg.split('=')[1] : 'development';
  
  let baseUrl: string;
  
  switch (environment) {
    case 'staging':
      baseUrl = Deno.env.get('STAGING_URL') || 'https://multiplayer-drawing-game-dev.your-subdomain.workers.dev';
      break;
    case 'production':
      baseUrl = Deno.env.get('PRODUCTION_URL') || 'https://multiplayer-drawing-game-prod.your-subdomain.workers.dev';
      break;
    default:
      baseUrl = 'http://localhost:8000';
  }
  
  console.log(`🎯 Testing environment: ${environment}`);
  console.log(`🌐 Base URL: ${baseUrl}`);
  
  const runner = new SmokeTestRunner(baseUrl);
  const success = await runner.runAllTests();
  
  if (!success) {
    console.log('\n💥 Smoke tests failed!');
    Deno.exit(1);
  }
  
  console.log('\n🎉 All smoke tests passed!');
}

if (import.meta.main) {
  await main();
}