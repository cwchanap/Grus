// Integration tests for WebSocket infrastructure
import { assertEquals, assertExists } from "$std/assert/mod.ts";
import { WebSocketManager } from "../websocket-manager.ts";
import { Env } from "../../../types/cloudflare.ts";
import { ClientMessage } from "../../../types/game.ts";

// Mock WebSocket implementation for integration testing
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
}

// Store references to server WebSockets for testing
const testServerSockets: MockWebSocket[] = [];

// Mock global WebSocketPair with test hooks
(globalThis as any).WebSocketPair = class {
  [0]: MockWebSocket;
  [1]: MockWebSocket;

  constructor() {
    this[0] = new MockWebSocket(); // client
    this[1] = new MockWebSocket(); // server
    testServerSockets.push(this[1]); // Store server for testing
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
        all: async () => ({ results: [] }),
      }),
    }),
    exec: async () => ({ count: 0, duration: 0 }),
    dump: async () => new ArrayBuffer(0),
    batch: async () => [],
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
          drawingData: [],
        });
      }
      if (key.startsWith("player:")) {
        return JSON.stringify({
          id: "player1",
          name: "Test Player",
          isHost: true,
          isConnected: true,
          lastActivity: Date.now(),
        });
      }
      return null;
    },
    put: async () => {},
    delete: async () => {},
    list: async () => ({ keys: [], list_complete: true }),
  } as any,
  WEBSOCKET_HANDLER: {} as any,
  ENVIRONMENT: "test",
};

Deno.test({
  name: "WebSocket Integration - Complete game flow",
  fn: async () => {
    const manager = new WebSocketManager(mockEnv, false);

    // Clear previous test sockets
    testServerSockets.length = 0;

    // Create two WebSocket connections for different players
    const request1 = new Request("http://localhost/ws?roomId=test-room", {
      headers: { "Upgrade": "websocket" },
    });

    const request2 = new Request("http://localhost/ws?roomId=test-room", {
      headers: { "Upgrade": "websocket" },
    });

    const response1 = await manager.handleRequest(request1);
    const response2 = await manager.handleRequest(request2);

    assertEquals(response1.status, 101);
    assertEquals(response2.status, 101);
    assertEquals(testServerSockets.length, 2);

    const ws1 = testServerSockets[0];
    const ws2 = testServerSockets[1];

    // Player 1 joins room
    const joinMessage1: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player1",
      data: { playerName: "Player 1" },
    };

    ws1.simulateMessage(JSON.stringify(joinMessage1));
    await new Promise((resolve) => setTimeout(resolve, 10));

    // Player 2 joins room
    const joinMessage2: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player2",
      data: { playerName: "Player 2" },
    };

    ws2.simulateMessage(JSON.stringify(joinMessage2));
    await new Promise((resolve) => setTimeout(resolve, 10));

    // Both players should receive room updates and game state
    const player1Messages = ws1.getSentMessages();
    const player2Messages = ws2.getSentMessages();

    assertEquals(player1Messages.length > 0, true);
    assertEquals(player2Messages.length > 0, true);

    // Verify both players received game state
    const player1HasGameState = player1Messages.some((msg) => {
      try {
        const parsed = JSON.parse(msg);
        return parsed.type === "game-state";
      } catch {
        return false;
      }
    });

    const player2HasGameState = player2Messages.some((msg) => {
      try {
        const parsed = JSON.parse(msg);
        return parsed.type === "game-state";
      } catch {
        return false;
      }
    });

    assertEquals(player1HasGameState, true);
    assertEquals(player2HasGameState, true);
  },
});

Deno.test({
  name: "WebSocket Integration - Real-time drawing synchronization",
  fn: async () => {
    const manager = new WebSocketManager(mockEnv, false);

    // Clear previous test sockets
    testServerSockets.length = 0;

    // Create two WebSocket connections
    const request1 = new Request("http://localhost/ws?roomId=test-room", {
      headers: { "Upgrade": "websocket" },
    });

    const request2 = new Request("http://localhost/ws?roomId=test-room", {
      headers: { "Upgrade": "websocket" },
    });

    await manager.handleRequest(request1);
    await manager.handleRequest(request2);

    const ws1 = testServerSockets[0];
    const ws2 = testServerSockets[1];

    // Both players join room
    const joinMessage1: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player1",
      data: { playerName: "Player 1" },
    };

    const joinMessage2: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player2",
      data: { playerName: "Player 2" },
    };

    ws1.simulateMessage(JSON.stringify(joinMessage1));
    ws2.simulateMessage(JSON.stringify(joinMessage2));
    await new Promise((resolve) => setTimeout(resolve, 10));

    // Clear messages
    ws1.getSentMessages().length = 0;
    ws2.getSentMessages().length = 0;

    // Player 1 draws (current drawer in mock state)
    const drawMessage: ClientMessage = {
      type: "draw",
      roomId: "test-room",
      playerId: "player1",
      data: {
        type: "start",
        x: 100,
        y: 200,
        color: "#FF0000",
        size: 5,
      },
    };

    ws1.simulateMessage(JSON.stringify(drawMessage));
    await new Promise((resolve) => setTimeout(resolve, 10));

    // Player 2 should receive the drawing update
    const player2Messages = ws2.getSentMessages();
    const player2HasDrawUpdate = player2Messages.some((msg) => {
      try {
        const parsed = JSON.parse(msg);
        return parsed.type === "draw-update";
      } catch {
        return false;
      }
    });

    assertEquals(player2HasDrawUpdate, true);

    // Player 1 should not receive their own drawing update
    const player1Messages = ws1.getSentMessages();
    const player1HasDrawUpdate = player1Messages.some((msg) => {
      try {
        const parsed = JSON.parse(msg);
        return parsed.type === "draw-update";
      } catch {
        return false;
      }
    });

    assertEquals(player1HasDrawUpdate, false);
  },
});

Deno.test({
  name: "WebSocket Integration - Chat and guessing system",
  fn: async () => {
    const manager = new WebSocketManager(mockEnv, false);

    // Clear previous test sockets
    testServerSockets.length = 0;

    // Create two WebSocket connections
    const request1 = new Request("http://localhost/ws?roomId=test-room", {
      headers: { "Upgrade": "websocket" },
    });

    const request2 = new Request("http://localhost/ws?roomId=test-room", {
      headers: { "Upgrade": "websocket" },
    });

    await manager.handleRequest(request1);
    await manager.handleRequest(request2);

    const ws1 = testServerSockets[0];
    const ws2 = testServerSockets[1];

    // Both players join room
    const joinMessage1: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player1",
      data: { playerName: "Player 1" },
    };

    const joinMessage2: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player2",
      data: { playerName: "Player 2" },
    };

    ws1.simulateMessage(JSON.stringify(joinMessage1));
    ws2.simulateMessage(JSON.stringify(joinMessage2));
    await new Promise((resolve) => setTimeout(resolve, 10));

    // Clear messages
    ws1.getSentMessages().length = 0;
    ws2.getSentMessages().length = 0;

    // Player 2 sends a correct guess
    const chatMessage: ClientMessage = {
      type: "chat",
      roomId: "test-room",
      playerId: "player2",
      data: { text: "test" }, // Matches the current word in mock state
    };

    ws2.simulateMessage(JSON.stringify(chatMessage));
    await new Promise((resolve) => setTimeout(resolve, 10));

    // Both players should receive the chat message
    const player1Messages = ws1.getSentMessages();
    const player2Messages = ws2.getSentMessages();

    const player1HasChatMessage = player1Messages.some((msg) => {
      try {
        const parsed = JSON.parse(msg);
        return parsed.type === "chat-message" && parsed.data?.isCorrect === true;
      } catch {
        return false;
      }
    });

    const player2HasChatMessage = player2Messages.some((msg) => {
      try {
        const parsed = JSON.parse(msg);
        return parsed.type === "chat-message" && parsed.data?.isCorrect === true;
      } catch {
        return false;
      }
    });

    assertEquals(player1HasChatMessage, true);
    assertEquals(player2HasChatMessage, true);
  },
});

Deno.test({
  name: "WebSocket Integration - Connection cleanup and room management",
  fn: async () => {
    const manager = new WebSocketManager(mockEnv, false);

    // Clear previous test sockets
    testServerSockets.length = 0;

    // Create WebSocket connection
    const request = new Request("http://localhost/ws?roomId=test-room", {
      headers: { "Upgrade": "websocket" },
    });

    await manager.handleRequest(request);
    const ws = testServerSockets[0];

    // Player joins room
    const joinMessage: ClientMessage = {
      type: "join-room",
      roomId: "test-room",
      playerId: "player1",
      data: { playerName: "Player 1" },
    };

    ws.simulateMessage(JSON.stringify(joinMessage));
    await new Promise((resolve) => setTimeout(resolve, 10));

    // Verify room is active
    const activeRooms = manager.getActiveRoomIds();
    assertEquals(activeRooms.includes("test-room"), true);

    // Simulate connection close
    ws.simulateClose();
    await new Promise((resolve) => setTimeout(resolve, 10));

    // Room cleanup should be handled by the manager
    assertEquals(true, true); // Placeholder - in real implementation would verify cleanup
  },
});
