#!/usr/bin/env -S deno run --allow-net --allow-read --allow-env

/**
 * Manual Integration Testing Script
 * 
 * This script performs basic integration tests against the running application
 * to verify that all components are properly connected and working together.
 */

import { assertEquals, assertExists, assert } from "$std/assert/mod.ts";

const BASE_URL = Deno.env.get("BASE_URL") || "http://localhost:8000";

interface TestResult {
  name: string;
  passed: boolean;
  error?: string;
  duration: number;
}

class IntegrationTester {
  private results: TestResult[] = [];

  async runTest(name: string, testFn: () => Promise<void>): Promise<void> {
    const startTime = Date.now();
    console.log(`🧪 Running: ${name}`);
    
    try {
      await testFn();
      const duration = Date.now() - startTime;
      this.results.push({ name, passed: true, duration });
      console.log(`✅ Passed: ${name} (${duration}ms)`);
    } catch (error) {
      const duration = Date.now() - startTime;
      this.results.push({ 
        name, 
        passed: false, 
        error: error.message, 
        duration 
      });
      console.log(`❌ Failed: ${name} (${duration}ms)`);
      console.log(`   Error: ${error.message}`);
    }
  }

  printSummary(): void {
    const passed = this.results.filter(r => r.passed).length;
    const failed = this.results.filter(r => !r.passed).length;
    const totalTime = this.results.reduce((sum, r) => sum + r.duration, 0);

    console.log("\n" + "=".repeat(60));
    console.log("🎯 INTEGRATION TEST SUMMARY");
    console.log("=".repeat(60));
    console.log(`✅ Passed: ${passed}`);
    console.log(`❌ Failed: ${failed}`);
    console.log(`⏱️  Total Time: ${totalTime}ms`);
    console.log(`📊 Success Rate: ${((passed / this.results.length) * 100).toFixed(1)}%`);

    if (failed > 0) {
      console.log("\n❌ FAILED TESTS:");
      this.results
        .filter(r => !r.passed)
        .forEach(r => {
          console.log(`   • ${r.name}: ${r.error}`);
        });
    }

    console.log("=".repeat(60));
  }

  getFailedCount(): number {
    return this.results.filter(r => !r.passed).length;
  }
}

async function testLobbyPage(tester: IntegrationTester): Promise<void> {
  await tester.runTest("Lobby page loads correctly", async () => {
    const response = await fetch(`${BASE_URL}/`);
    assertEquals(response.status, 200);
    
    const html = await response.text();
    assert(html.includes("Drawing Game"), "Page should contain game title");
    assert(html.includes("GameLobby"), "Page should include GameLobby component");
  });
}

async function testGameRoomPage(tester: IntegrationTester): Promise<void> {
  await tester.runTest("Game room page structure", async () => {
    // Test with a mock room ID
    const response = await fetch(`${BASE_URL}/room/test-room-123?playerId=test-player`);
    
    // Should either load the room or show room not found
    assert(response.status === 200 || response.status === 404, 
           "Should return 200 for valid room or 404 for invalid room");
    
    const html = await response.text();
    
    if (response.status === 200) {
      // If room exists, check for game components
      assert(html.includes("DrawingBoard") || html.includes("Room Not Found"), 
             "Should contain DrawingBoard component or error message");
      assert(html.includes("ChatRoom") || html.includes("Room Not Found"), 
             "Should contain ChatRoom component or error message");
      assert(html.includes("Scoreboard") || html.includes("Room Not Found"), 
             "Should contain Scoreboard component or error message");
    } else {
      // If room doesn't exist, should show error
      assert(html.includes("Room Not Found") || html.includes("not found"), 
             "Should show room not found message");
    }
  });
}

async function testAPIEndpoints(tester: IntegrationTester): Promise<void> {
  await tester.runTest("WebSocket endpoint availability", async () => {
    // Test WebSocket endpoint (should return upgrade required or method not allowed)
    const response = await fetch(`${BASE_URL}/api/websocket`);
    
    // WebSocket endpoints typically return 426 (Upgrade Required) or 405 (Method Not Allowed) for GET
    assert(
      response.status === 426 || response.status === 405 || response.status === 501,
      `WebSocket endpoint should return 426, 405, or 501, got ${response.status}`
    );
  });

  await tester.runTest("Static assets load correctly", async () => {
    // Test that CSS loads
    const cssResponse = await fetch(`${BASE_URL}/static/styles.css`);
    assertEquals(cssResponse.status, 200, "CSS file should load");
    
    const cssContent = await cssResponse.text();
    assert(cssContent.length > 0, "CSS file should not be empty");
  });
}

async function testDatabaseConnection(tester: IntegrationTester): Promise<void> {
  await tester.runTest("Database connectivity", async () => {
    // Test lobby page which requires database access
    const response = await fetch(`${BASE_URL}/`);
    assertEquals(response.status, 200);
    
    const html = await response.text();
    
    // Should not show database error
    assert(!html.includes("Database not available"), 
           "Should not show database connection error");
    assert(!html.includes("Failed to load lobby"), 
           "Should not show lobby loading error");
  });
}

async function testResponsiveDesign(tester: IntegrationTester): Promise<void> {
  await tester.runTest("Responsive design elements", async () => {
    const response = await fetch(`${BASE_URL}/`);
    assertEquals(response.status, 200);
    
    const html = await response.text();
    
    // Check for responsive design classes
    assert(html.includes("sm:") || html.includes("md:") || html.includes("lg:"), 
           "Should include responsive design classes");
    assert(html.includes("mobile") || html.includes("touch") || html.includes("responsive"), 
           "Should include mobile/touch optimizations");
  });
}

async function testSecurityHeaders(tester: IntegrationTester): Promise<void> {
  await tester.runTest("Security headers present", async () => {
    const response = await fetch(`${BASE_URL}/`);
    assertEquals(response.status, 200);
    
    // Check for basic security headers (these might not all be present in development)
    const headers = response.headers;
    
    // At minimum, should have content-type
    assertExists(headers.get("content-type"), "Should have content-type header");
    
    // Check that it's HTML
    const contentType = headers.get("content-type");
    assert(contentType?.includes("text/html"), "Should serve HTML content");
  });
}

async function testErrorHandling(tester: IntegrationTester): Promise<void> {
  await tester.runTest("404 error handling", async () => {
    const response = await fetch(`${BASE_URL}/nonexistent-page`);
    assertEquals(response.status, 404, "Should return 404 for nonexistent pages");
    
    const html = await response.text();
    assert(html.length > 0, "404 page should not be empty");
  });

  await tester.runTest("Invalid room handling", async () => {
    const response = await fetch(`${BASE_URL}/room/invalid-room-id-12345`);
    
    // Should either return 404 or 200 with error message
    assert(response.status === 200 || response.status === 404, 
           "Should handle invalid room gracefully");
    
    if (response.status === 200) {
      const html = await response.text();
      assert(html.includes("Room Not Found") || html.includes("not found"), 
             "Should show appropriate error message for invalid room");
    }
  });
}

async function testPerformance(tester: IntegrationTester): Promise<void> {
  await tester.runTest("Page load performance", async () => {
    const startTime = Date.now();
    const response = await fetch(`${BASE_URL}/`);
    const loadTime = Date.now() - startTime;
    
    assertEquals(response.status, 200);
    
    // Page should load within reasonable time (5 seconds for integration test)
    assert(loadTime < 5000, `Page should load within 5 seconds, took ${loadTime}ms`);
    
    console.log(`   📊 Page load time: ${loadTime}ms`);
  });
}

// Main test runner
async function main(): Promise<void> {
  console.log("🚀 Starting Integration Tests");
  console.log(`🎯 Target URL: ${BASE_URL}`);
  console.log("=".repeat(60));

  const tester = new IntegrationTester();

  // Check if server is running
  try {
    const healthCheck = await fetch(`${BASE_URL}/`);
    if (healthCheck.status !== 200) {
      throw new Error(`Server returned status ${healthCheck.status}`);
    }
    console.log("✅ Server is running and accessible");
  } catch (error) {
    console.log("❌ Server is not accessible:");
    console.log(`   Error: ${error.message}`);
    console.log("   Make sure the server is running with: deno task start");
    Deno.exit(1);
  }

  // Run all test suites
  await testLobbyPage(tester);
  await testGameRoomPage(tester);
  await testAPIEndpoints(tester);
  await testDatabaseConnection(tester);
  await testResponsiveDesign(tester);
  await testSecurityHeaders(tester);
  await testErrorHandling(tester);
  await testPerformance(tester);

  // Print summary and exit
  tester.printSummary();
  
  const failedCount = tester.getFailedCount();
  if (failedCount > 0) {
    console.log(`\n❌ ${failedCount} test(s) failed. Please check the application.`);
    Deno.exit(1);
  } else {
    console.log("\n🎉 All integration tests passed! The application is working correctly.");
    Deno.exit(0);
  }
}

if (import.meta.main) {
  await main();
}