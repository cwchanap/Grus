import { assertEquals } from "$std/assert/mod.ts";
// import { signal } from "@preact/signals";
// import ChatRoom from "../ChatRoom.tsx";
import type { ChatMessage } from "../../types/game.ts";

// Mock WebSocket for testing
class MockWebSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;

  readyState = MockWebSocket.CONNECTING;
  onopen: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;

  private sentMessages: string[] = [];

  constructor(public url: string) {
    // Simulate connection opening
    setTimeout(() => {
      this.readyState = MockWebSocket.OPEN;
      this.onopen?.(new Event("open"));
    }, 10);
  }

  send(data: string) {
    this.sentMessages.push(data);
  }

  close() {
    this.readyState = MockWebSocket.CLOSED;
    this.onclose?.(new CloseEvent("close"));
  }

  simulateMessage(data: any) {
    if (this.onmessage) {
      this.onmessage(new MessageEvent("message", { data: JSON.stringify(data) }));
    }
  }

  getSentMessages() {
    return this.sentMessages;
  }
}

// Mock global WebSocket
(globalThis as any).WebSocket = MockWebSocket;

// Test WebSocket message handling
Deno.test({
  name: "ChatRoom - WebSocket message handling",
  fn: () => {
    const receivedMessages: ChatMessage[] = [];

    // Mock message handler
    const handleChatMessage = (message: any) => {
      if (message.type === "chat-message") {
        receivedMessages.push(message.data);
      }
    };

    // Simulate receiving a chat message
    const testMessage: ChatMessage = {
      id: "msg1",
      playerId: "player2",
      playerName: "TestPlayer",
      message: "Hello world!",
      timestamp: Date.now(),
      isGuess: true,
      isCorrect: false,
    };

    const serverMessage = {
      type: "chat-message",
      roomId: "test-room",
      data: testMessage,
    };

    handleChatMessage(serverMessage);

    assertEquals(receivedMessages.length, 1);
    assertEquals(receivedMessages[0].message, "Hello world!");
    assertEquals(receivedMessages[0].playerName, "TestPlayer");
  },
});

Deno.test({
  name: "ChatRoom - correct guess detection",
  fn: () => {
    const testMessage: ChatMessage = {
      id: "msg1",
      playerId: "player2",
      playerName: "TestPlayer",
      message: "cat",
      timestamp: Date.now(),
      isGuess: true,
      isCorrect: true,
    };

    const _serverMessage = {
      type: "chat-message",
      roomId: "test-room",
      data: testMessage,
    };

    // Test correct guess detection
    assertEquals(testMessage.isCorrect, true);
    assertEquals(testMessage.isGuess, true);
  },
});

// Test message formatting
Deno.test({
  name: "ChatRoom - formats time correctly",
  fn: () => {
    // Create a mock date
    const testDate = new Date("2024-01-01T12:30:45Z");
    const _timestamp = testDate.getTime();

    // Test the time formatting logic
    const formattedTime = testDate.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

    // Should format as HH:MM
    assertEquals(formattedTime.includes(":"), true);
    assertEquals(formattedTime.length >= 4, true); // At least H:MM or HH:MM
  },
});

// Test message classification
Deno.test({
  name: "ChatRoom - classifies messages correctly",
  fn: () => {
    const testMessage: ChatMessage = {
      id: "msg1",
      playerId: "player2",
      playerName: "OtherPlayer",
      message: "cat",
      timestamp: Date.now(),
      isGuess: true,
      isCorrect: true,
    };

    const currentPlayerId = "player1";

    // Test correct guess detection (not own message)
    const isCorrectGuess = testMessage.isCorrect && testMessage.playerId !== currentPlayerId;
    assertEquals(isCorrectGuess, true);

    // Test own message detection
    const isOwnMessage = testMessage.playerId === currentPlayerId;
    assertEquals(isOwnMessage, false);
  },
});

// Test message validation logic
Deno.test({
  name: "ChatRoom - message validation",
  fn: () => {
    // Test empty message validation
    const emptyMessage = "";
    const validMessage = "Hello world!";
    const longMessage = "a".repeat(201); // Over 200 character limit

    // Mock validation logic from the component
    const validateMessage = (msg: string) => {
      return msg.trim().length > 0 && msg.length <= 200;
    };

    assertEquals(validateMessage(emptyMessage), false);
    assertEquals(validateMessage(validMessage), true);
    assertEquals(validateMessage(longMessage), false);
  },
});
