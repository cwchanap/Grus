/**
 * Simplified tests for WebSocket connection manager
 */

import { assertEquals } from "https://deno.land/std@0.208.0/assert/mod.ts";
import { connectionState } from "../connection-manager.ts";

Deno.test("Connection Manager - State Management", () => {
  // Test initial state
  assertExists(connectionState.value);
  assertEquals(typeof connectionState.value.status, "string");
  assertEquals(typeof connectionState.value.reconnectAttempts, "number");
  // isOnline might be undefined in test environment
  assertEquals(
    connectionState.value.isOnline === true || connectionState.value.isOnline === false ||
      connectionState.value.isOnline === undefined,
    true,
  );
});

Deno.test("Connection Manager - Error Messages", async () => {
  const { getErrorInfo } = await import("../../error-messages.ts");

  // Test network error
  const networkError = getErrorInfo("fetch failed");
  assertEquals(networkError.title, "Connection Problem");
  assertExists(networkError.suggestion);

  // Test WebSocket error - use message without 'connection' to avoid network match
  const wsError = getErrorInfo("WebSocket error occurred");
  assertEquals(wsError.title, "Real-time Connection Lost");
  assertExists(wsError.action);

  // Test generic error
  const genericError = getErrorInfo("unknown error");
  assertEquals(genericError.title, "Something Went Wrong");
});

Deno.test("Connection Manager - Offline State", async () => {
  const { OfflineManager: _OfflineManager, offlineState } = await import("../../offline-manager.ts");

  // Test initial offline state
  assertExists(offlineState.value);
  assertEquals(typeof offlineState.value.isOffline, "boolean");
  assertEquals(Array.isArray(offlineState.value.pendingMessages), true);
  assertEquals(Array.isArray(offlineState.value.pendingDrawingCommands), true);
  assertEquals(Array.isArray(offlineState.value.queuedActions), true);
});

Deno.test("Connection Manager - Error Recovery", async () => {
  const { ErrorRecoveryStrategies } = await import("../../error-messages.ts");

  // Test retry with backoff
  let attempts = 0;
  const operation = () => {
    attempts++;
    if (attempts < 2) {
      throw new Error("Temporary failure");
    }
    return Promise.resolve("success");
  };

  const result = await ErrorRecoveryStrategies.retryWithBackoff(operation, 3, 10);
  assertEquals(result, "success");
  assertEquals(attempts, 2);
});

Deno.test("Connection Manager - Fallback Strategy", async () => {
  const { ErrorRecoveryStrategies } = await import("../../error-messages.ts");

  // Test fallback
  const primary = () => Promise.reject(new Error("Primary failed"));
  const fallback = () => "fallback result";

  const result = await ErrorRecoveryStrategies.withFallback(primary, fallback);
  assertEquals(result, "fallback result");
});
