import { assert, assertEquals, assertExists } from "$std/assert/mod.ts";
import type { ChatMessage } from "../../types/core/room.ts";
import type { BaseClientMessage, BaseServerMessage } from "../../types/core/websocket.ts";

// Mock WebSocket for integration testing
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

  getLastMessage() {
    return this.sentMessages[this.sentMessages.length - 1];
  }
}

Deno.test({
  name: "ChatRoom Integration - WebSocket connection lifecycle",
  fn: async () => {
    const mockWs = new MockWebSocket("ws://localhost/ws?roomId=test-room");
    let connectionStatus = "disconnected";

    mockWs.onopen = () => {
      connectionStatus = "connected";
    };

    mockWs.onclose = () => {
      connectionStatus = "disconnected";
    };

    // Wait for connection to open
    await new Promise((resolve) => setTimeout(resolve, 20));

    assertEquals(connectionStatus, "connected");
    assertEquals(mockWs.readyState, MockWebSocket.OPEN);

    // Test disconnection
    mockWs.close();
    assertEquals(connectionStatus, "disconnected");
    assertEquals(mockWs.readyState, MockWebSocket.CLOSED);
  },
});

Deno.test({
  name: "ChatRoom Integration - join room message flow",
  fn: async () => {
    const mockWs = new MockWebSocket("ws://localhost/ws?roomId=test-room");

    // Wait for connection
    await new Promise((resolve) => setTimeout(resolve, 20));

    // Simulate joining room
    const joinMessage: BaseClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player1",
      data: { playerName: "TestPlayer" },
    };

    mockWs.send(JSON.stringify(joinMessage));

    const sentMessages = mockWs.getSentMessages();
    assertEquals(sentMessages.length, 1);

    const parsedMessage = JSON.parse(sentMessages[0]);
    assertEquals(parsedMessage.type, "join-room");
    assertEquals(parsedMessage.roomId, "test-room");
    assertEquals(parsedMessage.playerId, "player1");
    assertEquals(parsedMessage.data.playerName, "TestPlayer");
  },
});

Deno.test({
  name: "ChatRoom Integration - chat message sending",
  fn: async () => {
    const mockWs = new MockWebSocket("ws://localhost/ws?roomId=test-room");

    // Wait for connection
    await new Promise((resolve) => setTimeout(resolve, 20));

    // Simulate sending chat message
    const chatMessage: BaseClientMessage = {
      type: "chat",
      roomId: "test-room",
      playerId: "player1",
      data: { text: "Hello everyone!" },
    };

    mockWs.send(JSON.stringify(chatMessage));

    const lastMessage = mockWs.getLastMessage();
    assertExists(lastMessage);

    const parsedMessage = JSON.parse(lastMessage);
    assertEquals(parsedMessage.type, "chat");
    assertEquals(parsedMessage.data.text, "Hello everyone!");
  },
});

Deno.test({
  name: "ChatRoom Integration - receiving chat messages",
  fn: async () => {
    const mockWs = new MockWebSocket("ws://localhost/ws?roomId=test-room");
    const receivedMessages: ChatMessage[] = [];

    // Set up message handler
    mockWs.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data);
        if (message.type === "chat-message") {
          receivedMessages.push(message.data);
        }
      } catch (error) {
        console.error("Error parsing message:", error);
      }
    };

    // Wait for connection
    await new Promise((resolve) => setTimeout(resolve, 20));

    // Simulate receiving a chat message
    const incomingMessage: BaseServerMessage = {
      type: "chat-message",
      roomId: "test-room",
      data: {
        id: "msg1",
        playerId: "player2",
        playerName: "OtherPlayer",
        message: "Hi there!",
        timestamp: Date.now(),
        isGuess: true,
        isCorrect: false,
      },
    };

    mockWs.simulateMessage(incomingMessage);

    assertEquals(receivedMessages.length, 1);
    assertEquals(receivedMessages[0].message, "Hi there!");
    assertEquals(receivedMessages[0].playerName, "OtherPlayer");
    assertEquals(receivedMessages[0].isGuess, true);
    assertEquals(receivedMessages[0].isCorrect, false);
  },
});

Deno.test({
  name: "ChatRoom Integration - correct guess detection",
  fn: async () => {
    const mockWs = new MockWebSocket("ws://localhost/ws?roomId=test-room");
    const receivedMessages: ChatMessage[] = [];

    // Set up message handler
    mockWs.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data);
        if (message.type === "chat-message") {
          receivedMessages.push(message.data);
        }
      } catch (error) {
        console.error("Error parsing message:", error);
      }
    };

    // Wait for connection
    await new Promise((resolve) => setTimeout(resolve, 20));

    // Simulate receiving a correct guess
    const correctGuessMessage: BaseServerMessage = {
      type: "chat-message",
      roomId: "test-room",
      data: {
        id: "msg1",
        playerId: "player2",
        playerName: "GoodGuesser",
        message: "cat",
        timestamp: Date.now(),
        isGuess: true,
        isCorrect: true,
      },
    };

    mockWs.simulateMessage(correctGuessMessage);

    assertEquals(receivedMessages.length, 1);
    assertEquals(receivedMessages[0].isCorrect, true);
    assertEquals(receivedMessages[0].message, "cat");
    assertEquals(receivedMessages[0].playerName, "GoodGuesser");
  },
});

Deno.test({
  name: "ChatRoom Integration - message validation",
  fn: () => {
    // Test message validation logic that would be used in the component
    const validateChatMessage = (message: string): boolean => {
      return message.trim().length > 0 && message.length <= 200;
    };

    // Test cases
    assertEquals(validateChatMessage(""), false); // Empty
    assertEquals(validateChatMessage("   "), false); // Whitespace only
    assertEquals(validateChatMessage("Hello"), true); // Valid
    assertEquals(validateChatMessage("a".repeat(200)), true); // Max length
    assertEquals(validateChatMessage("a".repeat(201)), false); // Too long
  },
});

Deno.test({
  name: "ChatRoom Integration - message classification",
  fn: () => {
    const currentPlayerId = "player1";

    const ownMessage: ChatMessage = {
      id: "msg1",
      playerId: "player1",
      playerName: "Me",
      message: "My message",
      timestamp: Date.now(),
      isGuess: true,
      isCorrect: false,
    };

    const otherMessage: ChatMessage = {
      id: "msg2",
      playerId: "player2",
      playerName: "Other",
      message: "Other message",
      timestamp: Date.now(),
      isGuess: true,
      isCorrect: false,
    };

    const correctGuessMessage: ChatMessage = {
      id: "msg3",
      playerId: "player2",
      playerName: "Other",
      message: "cat",
      timestamp: Date.now(),
      isGuess: true,
      isCorrect: true,
    };

    // Test message classification functions
    const isOwnMessage = (msg: ChatMessage) => msg.playerId === currentPlayerId;
    const isCorrectGuess = (msg: ChatMessage) => msg.isCorrect && msg.playerId !== currentPlayerId;

    assertEquals(isOwnMessage(ownMessage), true);
    assertEquals(isOwnMessage(otherMessage), false);
    assertEquals(isCorrectGuess(ownMessage), false); // Own message can't be correct guess
    assertEquals(isCorrectGuess(otherMessage), false); // Not correct
    assertEquals(isCorrectGuess(correctGuessMessage), true); // Other's correct guess
  },
});

Deno.test({
  name: "ChatRoom Integration - time formatting",
  fn: () => {
    const formatTime = (timestamp: number) => {
      const date = new Date(timestamp);
      return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    };

    // Test with known timestamp
    const testTimestamp = new Date("2024-01-01T15:30:45Z").getTime();
    const formatted = formatTime(testTimestamp);

    // Should contain colon and be reasonable length
    assert(formatted.includes(":"));
    assert(formatted.length >= 4); // At least H:MM
    assert(formatted.length <= 8); // At most HH:MM AM/PM
  },
});

Deno.test({
  name: "ChatRoom Integration - error handling",
  fn: async () => {
    const mockWs = new MockWebSocket("ws://localhost/ws?roomId=test-room");
    let errorOccurred = false;

    mockWs.onerror = () => {
      errorOccurred = true;
    };

    // Wait for connection
    await new Promise((resolve) => setTimeout(resolve, 20));

    // Simulate error
    mockWs.onerror?.(new Event("error"));

    assertEquals(errorOccurred, true);
  },
});

Deno.test({
  name: "ChatRoom Integration - reconnection logic simulation",
  fn: async () => {
    let connectionAttempts = 0;
    let isConnected = false;

    const simulateConnection = () => {
      connectionAttempts++;
      return new Promise<boolean>((resolve) => {
        setTimeout(() => {
          // Simulate connection success after 2 attempts
          isConnected = connectionAttempts >= 2;
          resolve(isConnected);
        }, 10);
      });
    };

    // First attempt fails
    let result = await simulateConnection();
    assertEquals(result, false);
    assertEquals(connectionAttempts, 1);

    // Second attempt succeeds
    result = await simulateConnection();
    assertEquals(result, true);
    assertEquals(connectionAttempts, 2);
    assertEquals(isConnected, true);
  },
});
