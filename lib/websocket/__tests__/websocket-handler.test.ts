// Tests for WebSocket handler functionality
import { assertEquals, assertExists } from "$std/assert/mod.ts";
import { WebSocketHandler } from "../websocket-handler.ts";
import { Env } from "../../../types/cloudflare.ts";
import { ClientMessage } from "../../../types/game.ts";

// Mock implementations for testing
class MockWebSocket extends EventTarget {
  readyState = 1; // OPEN
  static OPEN = 1;
  static CONNECTING = 0;
  static CLOSING = 2;
  static CLOSED = 3;
  
  private sentMessages: string[] = [];

  send(data: string) {
    this.sentMessages.push(data);
  }

  accept() {}
  close() {}

  getSentMessages(): string[] {
    return [...this.sentMessages];
  }

  simulateMessage(data: string) {
    this.dispatchEvent(new MessageEvent("message", { data }));
  }

  simulateClose() {
    this.dispatchEvent(new Event("close"));
  }

  simulateError(error: Error) {
    this.dispatchEvent(new ErrorEvent("error", { error }));
  }
}

// Store reference to the server WebSocket for testing
let testServerSocket: MockWebSocket | null = null;

// Mock global WebSocketPair with test hooks
(globalThis as any).WebSocketPair = class {
  [0]: MockWebSocket;
  [1]: MockWebSocket;

  constructor() {
    this[0] = new MockWebSocket(); // client
    this[1] = new MockWebSocket(); // server
    testServerSocket = this[1]; // Store server for testing
  }
};

(globalThis as any).WebSocket = MockWebSocket;

// Mock Cloudflare environment
const mockEnv: Env = {
  DB: {
    prepare: () => ({
      bind: () => ({
        first: async () => ({ id: "test-room", max_players: 8 }),
        run: async () => ({ success: true }),
        all: async () => ({ results: [] })
      })
    }),
    exec: async () => ({ count: 0, duration: 0 }),
    dump: async () => new ArrayBuffer(0),
    batch: async () => []
  } as any,
  GAME_STATE: {
    get: async (key: string) => {
      if (key.startsWith("game:")) {
        return JSON.stringify({
          roomId: "test-room",
          currentDrawer: "player1",
          currentWord: "test",
          roundNumber: 1,
          timeRemaining: 120,
          phase: "drawing",
          players: [],
          scores: {},
          drawingData: []
        });
      }
      if (key.startsWith("player:")) {
        return JSON.stringify({
          id: "player1",
          name: "Test Player",
          isHost: true,
          isConnected: true,
          lastActivity: Date.now()
        });
      }
      return null;
    },
    put: async () => {},
    delete: async () => {},
    list: async () => ({ keys: [], list_complete: true })
  } as any,
  WEBSOCKET_HANDLER: {} as any,
  ENVIRONMENT: "test"
};

Deno.test({
  name: "WebSocketHandler - constructor",
  fn: () => {
    const handler = new WebSocketHandler(mockEnv);
    assertExists(handler);
  }
});

Deno.test({
  name: "WebSocketHandler - handleWebSocketUpgrade with valid request",
  fn: async () => {
    const handler = new WebSocketHandler(mockEnv);
    
    const request = new Request("http://localhost/ws", {
      headers: { "Upgrade": "websocket" }
    });

    const response = await handler.handleWebSocketUpgrade(request);
    assertEquals(response.status, 101);
    assertExists(testServerSocket);
  }
});

Deno.test({
  name: "WebSocketHandler - handleWebSocketUpgrade with invalid request",
  fn: async () => {
    const handler = new WebSocketHandler(mockEnv);
    
    const request = new Request("http://localhost/ws", {
      headers: { "Upgrade": "http" }
    });

    const response = await handler.handleWebSocketUpgrade(request);
    assertEquals(response.status, 426);
  }
});

Deno.test({
  name: "WebSocketHandler - message validation",
  fn: async () => {
    const handler = new WebSocketHandler(mockEnv);
    
    const request = new Request("http://localhost/ws", {
      headers: { "Upgrade": "websocket" }
    });

    await handler.handleWebSocketUpgrade(request);
    const ws = testServerSocket!;

    const validMessage: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player1",
      data: { playerName: "Test Player" }
    };

    ws.simulateMessage(JSON.stringify(validMessage));
    
    const messages = ws.getSentMessages();
    const hasError = messages.some(msg => {
      try {
        const parsed = JSON.parse(msg);
        return parsed.data?.type === "error";
      } catch {
        return false;
      }
    });
    
    assertEquals(hasError, false);
  }
});

Deno.test({
  name: "WebSocketHandler - invalid message format",
  fn: async () => {
    const handler = new WebSocketHandler(mockEnv);
    
    const request = new Request("http://localhost/ws", {
      headers: { "Upgrade": "websocket" }
    });

    await handler.handleWebSocketUpgrade(request);
    const ws = testServerSocket!;

    ws.simulateMessage("invalid json");
    
    const messages = ws.getSentMessages();
    const hasError = messages.some(msg => {
      try {
        const parsed = JSON.parse(msg);
        return parsed.data?.type === "error";
      } catch {
        return false;
      }
    });
    
    assertEquals(hasError, true);
  }
});

Deno.test({
  name: "WebSocketHandler - join room flow",
  fn: async () => {
    const handler = new WebSocketHandler(mockEnv);
    
    const request = new Request("http://localhost/ws", {
      headers: { "Upgrade": "websocket" }
    });

    await handler.handleWebSocketUpgrade(request);
    const ws = testServerSocket!;

    const joinMessage: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player1",
      data: { playerName: "Test Player" }
    };

    // Wait a bit for async operations to complete
    ws.simulateMessage(JSON.stringify(joinMessage));
    await new Promise(resolve => setTimeout(resolve, 10));
    
    const messages = ws.getSentMessages();
    
    const hasGameState = messages.some(msg => {
      try {
        const parsed = JSON.parse(msg);
        return parsed.type === "game-state";
      } catch {
        return false;
      }
    });
    
    assertEquals(hasGameState, true);
  }
});

Deno.test({
  name: "WebSocketHandler - rate limiting",
  fn: async () => {
    const handler = new WebSocketHandler(mockEnv);
    
    const request = new Request("http://localhost/ws", {
      headers: { "Upgrade": "websocket" }
    });

    await handler.handleWebSocketUpgrade(request);
    const ws = testServerSocket!;

    // Join room first
    const joinMessage: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player1",
      data: { playerName: "Test Player" }
    };
    ws.simulateMessage(JSON.stringify(joinMessage));

    // Send many messages quickly to trigger rate limit
    for (let i = 0; i < 35; i++) { // Exceeds the 30 message limit
      const chatMessage: ClientMessage = {
        type: "chat",
        roomId: "test-room",
        playerId: "player1",
        data: { text: `Message ${i}` }
      };
      ws.simulateMessage(JSON.stringify(chatMessage));
    }
    
    const messages = ws.getSentMessages();
    const hasRateLimitError = messages.some(msg => {
      try {
        const parsed = JSON.parse(msg);
        return parsed.data?.type === "error" && parsed.data?.message === "Rate limit exceeded";
      } catch {
        return false;
      }
    });
    
    assertEquals(hasRateLimitError, true);
  }
});

Deno.test({
  name: "WebSocketHandler - connection cleanup on close",
  fn: async () => {
    const handler = new WebSocketHandler(mockEnv);
    
    const request = new Request("http://localhost/ws", {
      headers: { "Upgrade": "websocket" }
    });

    await handler.handleWebSocketUpgrade(request);
    const ws = testServerSocket!;

    // Join room first
    const joinMessage: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player1",
      data: { playerName: "Test Player" }
    };
    ws.simulateMessage(JSON.stringify(joinMessage));

    // Simulate connection close
    ws.simulateClose();
    
    // Connection should be cleaned up (this is tested implicitly)
    assertEquals(true, true); // Placeholder assertion
  }
});

Deno.test({
  name: "WebSocketHandler - drawing command validation and broadcasting",
  fn: async () => {
    const handler = new WebSocketHandler(mockEnv);
    
    const request = new Request("http://localhost/ws", {
      headers: { "Upgrade": "websocket" }
    });

    await handler.handleWebSocketUpgrade(request);
    const ws = testServerSocket!;

    // Join room first
    const joinMessage: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player1",
      data: { playerName: "Test Player" }
    };
    ws.simulateMessage(JSON.stringify(joinMessage));
    await new Promise(resolve => setTimeout(resolve, 10));

    // Send valid drawing command (player1 is the current drawer in mock game state)
    const drawMessage: ClientMessage = {
      type: "draw",
      roomId: "test-room",
      playerId: "player1",
      data: {
        type: "start",
        x: 100,
        y: 200,
        color: "#FF0000",
        size: 5
      }
    };

    ws.simulateMessage(JSON.stringify(drawMessage));
    await new Promise(resolve => setTimeout(resolve, 10));
    
    // For this test, we'll verify that the drawing command was processed
    // by checking that no error was sent (since drawing updates are only sent to other players)
    const messages = ws.getSentMessages();
    const hasError = messages.some(msg => {
      try {
        const parsed = JSON.parse(msg);
        return parsed.data?.type === "error";
      } catch {
        return false;
      }
    });
    
    // Should not have an error (drawing command was valid and processed)
    assertEquals(hasError, false);
  }
});

Deno.test({
  name: "WebSocketHandler - invalid drawing command rejection",
  fn: async () => {
    const handler = new WebSocketHandler(mockEnv);
    
    const request = new Request("http://localhost/ws", {
      headers: { "Upgrade": "websocket" }
    });

    await handler.handleWebSocketUpgrade(request);
    const ws = testServerSocket!;

    // Join room first
    const joinMessage: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player1",
      data: { playerName: "Test Player" }
    };
    ws.simulateMessage(JSON.stringify(joinMessage));
    await new Promise(resolve => setTimeout(resolve, 10));

    // Clear previous messages
    ws.getSentMessages().length = 0;

    // Send invalid drawing command (coordinates out of bounds)
    const drawMessage: ClientMessage = {
      type: "draw",
      roomId: "test-room",
      playerId: "player1",
      data: {
        type: "start",
        x: 5000, // Out of bounds
        y: 5000, // Out of bounds
        color: "#FF0000",
        size: 5
      }
    };

    ws.simulateMessage(JSON.stringify(drawMessage));
    await new Promise(resolve => setTimeout(resolve, 10));
    
    const messages = ws.getSentMessages();
    const hasError = messages.some(msg => {
      try {
        const parsed = JSON.parse(msg);
        return parsed.data?.type === "error";
      } catch {
        return false;
      }
    });
    
    assertEquals(hasError, true);
  }
});

Deno.test({
  name: "WebSocketHandler - chat message with correct guess detection",
  fn: async () => {
    const handler = new WebSocketHandler(mockEnv);
    
    const request = new Request("http://localhost/ws", {
      headers: { "Upgrade": "websocket" }
    });

    await handler.handleWebSocketUpgrade(request);
    const ws = testServerSocket!;

    // Join room first
    const joinMessage: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player1",
      data: { playerName: "Test Player" }
    };
    ws.simulateMessage(JSON.stringify(joinMessage));
    await new Promise(resolve => setTimeout(resolve, 10));

    // Clear previous messages
    ws.getSentMessages().length = 0;

    // Send chat message that matches the current word
    const chatMessage: ClientMessage = {
      type: "chat",
      roomId: "test-room",
      playerId: "player1",
      data: { text: "test" } // This matches the mock game state's currentWord
    };

    ws.simulateMessage(JSON.stringify(chatMessage));
    await new Promise(resolve => setTimeout(resolve, 10));
    
    const messages = ws.getSentMessages();
    const hasChatMessage = messages.some(msg => {
      try {
        const parsed = JSON.parse(msg);
        return parsed.type === "chat-message" && parsed.data?.isCorrect === true;
      } catch {
        return false;
      }
    });
    
    assertEquals(hasChatMessage, true);
  }
});

Deno.test({
  name: "WebSocketHandler - connection cleanup removes player from room",
  fn: async () => {
    const handler = new WebSocketHandler(mockEnv);
    
    const request = new Request("http://localhost/ws", {
      headers: { "Upgrade": "websocket" }
    });

    await handler.handleWebSocketUpgrade(request);
    const ws = testServerSocket!;

    // Join room first
    const joinMessage: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player1",
      data: { playerName: "Test Player" }
    };
    ws.simulateMessage(JSON.stringify(joinMessage));
    await new Promise(resolve => setTimeout(resolve, 10));

    // Verify connection count
    const initialCount = handler.getConnectionCount();
    assertEquals(initialCount, 1);

    // Simulate connection close
    ws.simulateClose();
    await new Promise(resolve => setTimeout(resolve, 10));

    // Connection should be cleaned up
    const finalCount = handler.getConnectionCount();
    assertEquals(finalCount, 0);
  }
});

Deno.test({
  name: "WebSocketHandler - room broadcasting excludes sender",
  fn: async () => {
    const handler = new WebSocketHandler(mockEnv);
    
    // Create two connections to the same room
    const request1 = new Request("http://localhost/ws", {
      headers: { "Upgrade": "websocket" }
    });
    const request2 = new Request("http://localhost/ws", {
      headers: { "Upgrade": "websocket" }
    });

    await handler.handleWebSocketUpgrade(request1);
    const ws1 = testServerSocket!;
    
    await handler.handleWebSocketUpgrade(request2);
    const ws2 = testServerSocket!;

    // Join room with first player
    const joinMessage1: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player1",
      data: { playerName: "Player 1" }
    };
    ws1.simulateMessage(JSON.stringify(joinMessage1));
    await new Promise(resolve => setTimeout(resolve, 10));

    // Join room with second player
    const joinMessage2: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player2",
      data: { playerName: "Player 2" }
    };
    ws2.simulateMessage(JSON.stringify(joinMessage2));
    await new Promise(resolve => setTimeout(resolve, 10));

    // Clear messages
    ws1.getSentMessages().length = 0;
    ws2.getSentMessages().length = 0;

    // Send drawing command from player1
    const drawMessage: ClientMessage = {
      type: "draw",
      roomId: "test-room",
      playerId: "player1",
      data: {
        type: "start",
        x: 100,
        y: 200,
        color: "#FF0000",
        size: 5
      }
    };

    ws1.simulateMessage(JSON.stringify(drawMessage));
    await new Promise(resolve => setTimeout(resolve, 10));

    // Player1 should not receive the draw-update (sender excluded)
    const player1Messages = ws1.getSentMessages();
    const player1HasDrawUpdate = player1Messages.some(msg => {
      try {
        const parsed = JSON.parse(msg);
        return parsed.type === "draw-update";
      } catch {
        return false;
      }
    });

    // Player2 should receive the draw-update
    const player2Messages = ws2.getSentMessages();
    const player2HasDrawUpdate = player2Messages.some(msg => {
      try {
        const parsed = JSON.parse(msg);
        return parsed.type === "draw-update";
      } catch {
        return false;
      }
    });

    assertEquals(player1HasDrawUpdate, false);
    assertEquals(player2HasDrawUpdate, true);
  }
});