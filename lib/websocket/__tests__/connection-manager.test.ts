/**
 * Tests for WebSocket connection manager
 */

import { assert, assertEquals, assertExists } from "https://deno.land/std@0.208.0/assert/mod.ts";
import { connectionState, WebSocketConnectionManager } from "../connection-manager.ts";

// Mock WebSocket constants
const WEBSOCKET_CONNECTING = 0;
const WEBSOCKET_OPEN = 1;
const WEBSOCKET_CLOSING = 2;
const WEBSOCKET_CLOSED = 3;

// Mock WebSocket for testing
class MockWebSocket {
  public readyState = WEBSOCKET_CONNECTING;
  public onopen: ((event: Event) => void) | null = null;
  public onclose: ((event: CloseEvent) => void) | null = null;
  public onmessage: ((event: MessageEvent) => void) | null = null;
  public onerror: ((event: Event) => void) | null = null;

  private sentMessages: string[] = [];

  constructor(public url: string) {
    // Simulate connection after a short delay
    setTimeout(() => {
      this.readyState = WEBSOCKET_OPEN;
      this.onopen?.(new Event("open"));
    }, 10);
  }

  send(data: string) {
    if (this.readyState === WEBSOCKET_OPEN) {
      this.sentMessages.push(data);
    } else {
      throw new Error("WebSocket is not open");
    }
  }

  close() {
    this.readyState = WEBSOCKET_CLOSED;
    this.onclose?.(new CloseEvent("close"));
  }

  // Test helpers
  simulateMessage(data: string) {
    if (this.onmessage) {
      this.onmessage(new MessageEvent("message", { data }));
    }
  }

  simulateError() {
    this.onerror?.(new Event("error"));
  }

  getSentMessages(): string[] {
    return [...this.sentMessages];
  }
}

// Mock global WebSocket with constants
(globalThis as any).WebSocket = MockWebSocket;
(globalThis as any).WebSocket.CONNECTING = WEBSOCKET_CONNECTING;
(globalThis as any).WebSocket.OPEN = WEBSOCKET_OPEN;
(globalThis as any).WebSocket.CLOSING = WEBSOCKET_CLOSING;
(globalThis as any).WebSocket.CLOSED = WEBSOCKET_CLOSED;

// Mock navigator.onLine
Object.defineProperty(navigator, "onLine", {
  writable: true,
  value: true,
});

// Mock window location
(globalThis as any).window = {
  location: {
    protocol: "https:",
    host: "test.example.com",
    hostname: "test.example.com",
  },
  addEventListener: () => {},
  removeEventListener: () => {},
};

Deno.test("WebSocketConnectionManager - Basic Connection", async () => {
  const manager = new WebSocketConnectionManager(
    "test-room",
    "test-player",
    "Test Player",
  );

  // Wait for connection
  await new Promise((resolve) => setTimeout(resolve, 50));

  // Check connection state
  assertEquals(connectionState.value.status, "connected");
  assertEquals(connectionState.value.reconnectAttempts, 0);
  assertExists(connectionState.value.lastConnected);

  manager.destroy();
});

Deno.test("WebSocketConnectionManager - Message Sending", async () => {
  const manager = new WebSocketConnectionManager(
    "test-room",
    "test-player",
    "Test Player",
  );

  // Wait for connection
  await new Promise((resolve) => setTimeout(resolve, 50));

  // Send a message
  const success = manager.sendMessage({
    type: "chat",
    roomId: "test-room",
    playerId: "test-player",
    data: { text: "Hello" },
  });

  assert(success);
  manager.destroy();
});

Deno.test("WebSocketConnectionManager - Message Handling", async () => {
  const manager = new WebSocketConnectionManager(
    "test-room",
    "test-player",
    "Test Player",
  );

  let receivedMessage: any = null;

  // Set up message handler
  manager.onMessage("test-message", (message) => {
    receivedMessage = message;
  });

  // Wait for connection
  await new Promise((resolve) => setTimeout(resolve, 50));

  // Simulate receiving a message
  const testMessage = {
    type: "test-message",
    data: { content: "test" },
  };

  // Access the internal WebSocket to simulate message
  const ws = (manager as any).ws as MockWebSocket;
  ws.simulateMessage(JSON.stringify(testMessage));

  // Check if message was handled
  assertEquals(receivedMessage, testMessage);

  manager.destroy();
});

Deno.test("WebSocketConnectionManager - Reconnection Logic", async () => {
  const manager = new WebSocketConnectionManager(
    "test-room",
    "test-player",
    "Test Player",
    { maxReconnectAttempts: 2, baseReconnectDelay: 100 },
  );

  // Wait for initial connection
  await new Promise((resolve) => setTimeout(resolve, 50));
  assertEquals(connectionState.value.status, "connected");

  // Simulate connection loss
  const ws = (manager as any).ws as MockWebSocket;
  ws.close();

  // Wait for reconnection attempt
  await new Promise((resolve) => setTimeout(resolve, 150));

  // Should be reconnecting
  assert(["reconnecting", "connecting", "connected"].includes(connectionState.value.status));

  manager.destroy();
});

Deno.test("WebSocketConnectionManager - Offline Handling", async () => {
  // Set navigator offline
  Object.defineProperty(navigator, "onLine", { value: false });

  const manager = new WebSocketConnectionManager(
    "test-room",
    "test-player",
    "Test Player",
  );

  // Wait a bit
  await new Promise((resolve) => setTimeout(resolve, 50));

  // Should be offline
  assertEquals(connectionState.value.status, "offline");
  assertEquals(connectionState.value.isOnline, false);

  // Reset navigator online
  Object.defineProperty(navigator, "onLine", { value: true });

  manager.destroy();
});

Deno.test("WebSocketConnectionManager - Message Queuing", async () => {
  const manager = new WebSocketConnectionManager(
    "test-room",
    "test-player",
    "Test Player",
  );

  // Send message before connection is established
  const success = manager.sendMessage({
    type: "chat",
    roomId: "test-room",
    playerId: "test-player",
    data: { text: "Queued message" },
  });

  // Should return true (queued)
  assert(success);

  // Wait for connection and message processing
  await new Promise((resolve) => setTimeout(resolve, 100));

  // Check that message was sent after connection
  const ws = (manager as any).ws as MockWebSocket;
  const sentMessages = ws.getSentMessages();

  // Should have join-room and chat messages
  assert(sentMessages.length >= 2);
  assert(sentMessages.some((msg) => JSON.parse(msg).type === "chat"));

  manager.destroy();
});

Deno.test("WebSocketConnectionManager - Error Handling", async () => {
  const manager = new WebSocketConnectionManager(
    "test-room",
    "test-player",
    "Test Player",
  );

  // Wait for connection
  await new Promise((resolve) => setTimeout(resolve, 50));

  // Simulate WebSocket error
  const ws = (manager as any).ws as MockWebSocket;
  ws.simulateError();

  // Wait for error handling
  await new Promise((resolve) => setTimeout(resolve, 50));

  // Should handle error gracefully
  assertEquals(connectionState.value.status, "disconnected");
  assertExists(connectionState.value.error);

  manager.destroy();
});

Deno.test("WebSocketConnectionManager - Heartbeat", async () => {
  const manager = new WebSocketConnectionManager(
    "test-room",
    "test-player",
    "Test Player",
    { heartbeatInterval: 100 }, // Short interval for testing
  );

  // Wait for connection and first heartbeat
  await new Promise((resolve) => setTimeout(resolve, 200));

  const ws = (manager as any).ws as MockWebSocket;
  const sentMessages = ws.getSentMessages();

  // Should have sent at least one ping message
  const pingMessages = sentMessages.filter((msg) => {
    try {
      return JSON.parse(msg).type === "ping";
    } catch {
      return false;
    }
  });

  assert(pingMessages.length >= 1);

  manager.destroy();
});

Deno.test("WebSocketConnectionManager - Development Environment", async () => {
  // Mock development environment
  (globalThis as any).window.location.hostname = "localhost";

  const manager = new WebSocketConnectionManager(
    "test-room",
    "test-player",
    "Test Player",
  );

  // Wait a bit
  await new Promise((resolve) => setTimeout(resolve, 50));

  // Should be disconnected in development
  assertEquals(connectionState.value.status, "disconnected");
  assertEquals(connectionState.value.error, "Development mode");

  // Reset hostname
  (globalThis as any).window.location.hostname = "test.example.com";

  manager.destroy();
});
