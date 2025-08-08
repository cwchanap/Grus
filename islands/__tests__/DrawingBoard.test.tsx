import { assert, assertEquals, assertExists } from "$std/assert/mod.ts";
import { DrawingCommand } from "../../types/games/drawing.ts";
import { BaseGameState } from "../../types/core/game.ts";
import { BaseClientMessage, BaseServerMessage } from "../../types/core/websocket.ts";

// Mock WebSocket for testing
class MockWebSocket {
  public readyState: number = WebSocket.OPEN;
  public onopen: ((event: Event) => void) | null = null;
  public onmessage: ((event: MessageEvent) => void) | null = null;
  public onclose: ((event: CloseEvent) => void) | null = null;
  public onerror: ((event: Event) => void) | null = null;

  private messageQueue: string[] = [];

  constructor(public url: string) {}

  send(data: string): void {
    this.messageQueue.push(data);
  }

  close(): void {
    this.readyState = WebSocket.CLOSED;
    if (this.onclose) {
      this.onclose(new CloseEvent("close", { wasClean: true }));
    }
  }

  // Test helper methods
  getLastMessage(): string | undefined {
    return this.messageQueue[this.messageQueue.length - 1];
  }

  getAllMessages(): string[] {
    return [...this.messageQueue];
  }

  clearMessages(): void {
    this.messageQueue = [];
  }

  simulateMessage(data: any): void {
    if (this.onmessage) {
      const event = new MessageEvent("message", { data: JSON.stringify(data) });
      this.onmessage(event);
    }
  }

  simulateOpen(): void {
    this.readyState = WebSocket.OPEN;
    if (this.onopen) {
      this.onopen(new Event("open"));
    }
  }

  simulateError(): void {
    if (this.onerror) {
      this.onerror(new Event("error"));
    }
  }
}

// Test utilities
function createMockGameState(overrides: Partial<BaseGameState> = {}): BaseGameState {
  return {
    roomId: "test-room",
    currentDrawer: "player1",
    currentWord: "test-word",
    roundNumber: 1,
    timeRemaining: 60,
    phase: "drawing",
    players: [
      {
        id: "player1",
        name: "Player 1",
        isHost: true,
        isConnected: true,
        lastActivity: Date.now(),
      },
      {
        id: "player2",
        name: "Player 2",
        isHost: false,
        isConnected: true,
        lastActivity: Date.now(),
      },
    ],
    scores: { player1: 0, player2: 0 },
    drawingData: [],
    correctGuesses: [],
    chatMessages: [],
    settings: { maxRounds: 5, roundTimeSeconds: 75 },
    ...overrides,
  };
}

function createDrawingCommand(overrides: Partial<DrawingCommand> = {}): DrawingCommand {
  return {
    type: "start",
    x: 100,
    y: 150,
    color: "#000000",
    size: 5,
    timestamp: Date.now(),
    ...overrides,
  };
}

Deno.test("DrawingBoard - WebSocket message handling", () => {
  // Test drawing command synchronization
  const drawingCommands: DrawingCommand[] = [];

  const mockHandleDrawingUpdate = (
    data: { command?: DrawingCommand; commands?: DrawingCommand[] },
  ) => {
    if (data.command) {
      drawingCommands.push(data.command);
    } else if (data.commands) {
      drawingCommands.push(...data.commands);
    }
  };

  // Test single command update
  const singleCommand = createDrawingCommand({ type: "start", x: 50, y: 75 });
  mockHandleDrawingUpdate({ command: singleCommand });

  assertEquals(drawingCommands.length, 1);
  assertEquals(drawingCommands[0].type, "start");
  assertEquals(drawingCommands[0].x, 50);
  assertEquals(drawingCommands[0].y, 75);

  // Test batch command update
  const batchCommands = [
    createDrawingCommand({ type: "move", x: 60, y: 85 }),
    createDrawingCommand({ type: "move", x: 70, y: 95 }),
    createDrawingCommand({ type: "end" }),
  ];

  mockHandleDrawingUpdate({ commands: batchCommands });

  assertEquals(drawingCommands.length, 4); // 1 + 3
  assertEquals(drawingCommands[1].type, "move");
  assertEquals(drawingCommands[2].type, "move");
  assertEquals(drawingCommands[3].type, "end");
});

Deno.test("DrawingBoard - WebSocket connection lifecycle", () => {
  const mockWs = new MockWebSocket("ws://localhost/ws?roomId=test&playerId=player1");

  // Test connection states
  let connectionStatus = "disconnected";

  mockWs.onopen = () => {
    connectionStatus = "connected";
  };

  mockWs.onclose = () => {
    connectionStatus = "disconnected";
  };

  mockWs.onerror = () => {
    connectionStatus = "error";
  };

  // Simulate connection
  assertEquals(connectionStatus, "disconnected");
  mockWs.simulateOpen();
  assertEquals(connectionStatus, "connected");

  // Simulate disconnection
  mockWs.close();
  assertEquals(connectionStatus, "disconnected");
});

Deno.test("DrawingBoard - Drawing command transmission", () => {
  const mockWs = new MockWebSocket("ws://localhost/ws?roomId=test&playerId=player1");
  mockWs.simulateOpen();

  const roomId = "test-room";
  const playerId = "player1";

  // Simulate sending a drawing command
  const command = createDrawingCommand({ type: "start", x: 100, y: 200 });

  const message: ClientMessage = {
    type: "draw",
    roomId,
    playerId,
    data: { command },
  };

  mockWs.send(JSON.stringify(message));

  // Verify message was sent
  const lastMessage = mockWs.getLastMessage();
  assertExists(lastMessage);

  const parsedMessage = JSON.parse(lastMessage);
  assertEquals(parsedMessage.type, "draw");
  assertEquals(parsedMessage.roomId, roomId);
  assertEquals(parsedMessage.playerId, playerId);
  assertEquals(parsedMessage.data.command.type, "start");
  assertEquals(parsedMessage.data.command.x, 100);
  assertEquals(parsedMessage.data.command.y, 200);
});

Deno.test("DrawingBoard - Batch drawing command transmission", () => {
  const mockWs = new MockWebSocket("ws://localhost/ws?roomId=test&playerId=player1");
  mockWs.simulateOpen();

  const roomId = "test-room";
  const playerId = "player1";

  // Simulate sending batch commands
  const commands = [
    createDrawingCommand({ type: "start", x: 10, y: 20 }),
    createDrawingCommand({ type: "move", x: 15, y: 25 }),
    createDrawingCommand({ type: "move", x: 20, y: 30 }),
    createDrawingCommand({ type: "end" }),
  ];

  const message: ClientMessage = {
    type: "draw",
    roomId,
    playerId,
    data: { commands },
  };

  mockWs.send(JSON.stringify(message));

  // Verify batch message was sent
  const lastMessage = mockWs.getLastMessage();
  assertExists(lastMessage);

  const parsedMessage = JSON.parse(lastMessage);
  assertEquals(parsedMessage.type, "draw");
  assertEquals(parsedMessage.data.commands.length, 4);
  assertEquals(parsedMessage.data.commands[0].type, "start");
  assertEquals(parsedMessage.data.commands[1].type, "move");
  assertEquals(parsedMessage.data.commands[2].type, "move");
  assertEquals(parsedMessage.data.commands[3].type, "end");
});

Deno.test("DrawingBoard - Drawing permissions based on game state", () => {
  // Test drawer permissions
  const gameStateAsDrawer = createMockGameState({
    currentDrawer: "player1",
    phase: "drawing",
  });

  const gameStateAsWatcher = createMockGameState({
    currentDrawer: "player2",
    phase: "drawing",
  });

  const gameStateWaiting = createMockGameState({
    currentDrawer: "player1",
    phase: "waiting",
  });

  // Mock permission checking logic
  const checkDrawingPermissions = (playerId: string, gameState: BaseGameState) => {
    // Note: Drawing permissions would be handled by the drawing game engine
    return gameState.phase === "playing";
  };

  // Test permissions
  assert(checkDrawingPermissions("player1", gameStateAsDrawer));
  assert(!checkDrawingPermissions("player1", gameStateAsWatcher));
  assert(!checkDrawingPermissions("player1", gameStateWaiting));
  assert(!checkDrawingPermissions("player2", gameStateAsDrawer));
});

Deno.test("DrawingBoard - Real-time synchronization flow", () => {
  const mockWs = new MockWebSocket("ws://localhost/ws?roomId=test&playerId=player1");
  mockWs.simulateOpen();

  const receivedCommands: DrawingCommand[] = [];
  const sentMessages: ClientMessage[] = [];

  // Mock message handlers
  const handleIncomingMessage = (message: ServerMessage) => {
    if (message.type === "draw-update") {
      if (message.data.command) {
        receivedCommands.push(message.data.command);
      } else if (message.data.commands) {
        receivedCommands.push(...message.data.commands);
      }
    }
  };

  const sendDrawingCommand = (command: DrawingCommand) => {
    const message: ClientMessage = {
      type: "draw",
      roomId: "test-room",
      playerId: "player1",
      data: { command },
    };
    sentMessages.push(message);
    mockWs.send(JSON.stringify(message));
  };

  // Simulate drawing sequence
  const startCommand = createDrawingCommand({ type: "start", x: 50, y: 100 });
  const moveCommand1 = createDrawingCommand({ type: "move", x: 55, y: 105 });
  const moveCommand2 = createDrawingCommand({ type: "move", x: 60, y: 110 });
  const endCommand = createDrawingCommand({ type: "end" });

  // Send commands
  sendDrawingCommand(startCommand);
  sendDrawingCommand(moveCommand1);
  sendDrawingCommand(moveCommand2);
  sendDrawingCommand(endCommand);

  // Verify commands were sent
  assertEquals(sentMessages.length, 4);
  assertEquals(mockWs.getAllMessages().length, 4);

  // Simulate receiving commands from another player
  const incomingMessage: ServerMessage = {
    type: "draw-update",
    roomId: "test-room",
    data: {
      commands: [
        createDrawingCommand({ type: "start", x: 200, y: 300 }),
        createDrawingCommand({ type: "move", x: 210, y: 310 }),
        createDrawingCommand({ type: "end" }),
      ],
    },
  };

  handleIncomingMessage(incomingMessage);

  // Verify commands were received
  assertEquals(receivedCommands.length, 3);
  assertEquals(receivedCommands[0].type, "start");
  assertEquals(receivedCommands[1].type, "move");
  assertEquals(receivedCommands[2].type, "end");
});

Deno.test("DrawingBoard - Connection error handling", () => {
  const mockWs = new MockWebSocket("ws://localhost/ws?roomId=test&playerId=player1");

  let connectionStatus = "connecting";
  let errorCount = 0;

  mockWs.onopen = () => {
    connectionStatus = "connected";
  };

  mockWs.onerror = () => {
    connectionStatus = "error";
    errorCount++;
  };

  mockWs.onclose = () => {
    connectionStatus = "disconnected";
  };

  // Test error handling
  assertEquals(connectionStatus, "connecting");
  assertEquals(errorCount, 0);

  mockWs.simulateError();
  assertEquals(connectionStatus, "error");
  assertEquals(errorCount, 1);

  // Test recovery
  mockWs.simulateOpen();
  assertEquals(connectionStatus, "connected");
});

Deno.test("DrawingBoard - Message validation and error handling", () => {
  const validMessages: any[] = [];
  const invalidMessages: any[] = [];

  const handleMessage = (data: string) => {
    try {
      const message = JSON.parse(data);

      // Basic message validation
      if (message.type && message.roomId && typeof message.data === "object") {
        validMessages.push(message);
      } else {
        invalidMessages.push(message);
      }
    } catch (error) {
      invalidMessages.push({
        error: error instanceof Error ? error.message : "Unknown error",
        data,
      });
    }
  };

  // Test valid messages
  const validMessage = {
    type: "draw-update",
    roomId: "test-room",
    data: { command: createDrawingCommand() },
  };

  handleMessage(JSON.stringify(validMessage));
  assertEquals(validMessages.length, 1);
  assertEquals(invalidMessages.length, 0);

  // Test invalid JSON
  handleMessage("invalid json");
  assertEquals(validMessages.length, 1);
  assertEquals(invalidMessages.length, 1);

  // Test invalid message structure
  const invalidMessage = { type: "draw-update" }; // Missing roomId and data
  handleMessage(JSON.stringify(invalidMessage));
  assertEquals(validMessages.length, 1);
  assertEquals(invalidMessages.length, 2);
});

Deno.test("DrawingBoard - Performance optimization", () => {
  // Test drawing command throttling simulation
  const commands: DrawingCommand[] = [];
  const throttledCommands: DrawingCommand[] = [];

  let lastThrottleTime = 0;
  const throttleInterval = 16; // ~60fps

  const throttleCommand = (command: DrawingCommand) => {
    commands.push(command);

    const now = Date.now();
    if (command.type === "start" || command.type === "end" || command.type === "clear") {
      // Always send critical commands immediately
      throttledCommands.push(command);
      lastThrottleTime = now;
    } else if (now - lastThrottleTime >= throttleInterval) {
      // Send move commands at throttled rate
      throttledCommands.push(command);
      lastThrottleTime = now;
    }
    // Otherwise, command is dropped for throttling
  };

  // Simulate rapid drawing
  const startTime = Date.now();
  for (let i = 0; i < 100; i++) {
    const command = createDrawingCommand({
      type: i === 0 ? "start" : i === 99 ? "end" : "move",
      x: i,
      y: i,
      timestamp: startTime + i,
    });
    throttleCommand(command);
  }

  // Verify throttling worked
  assertEquals(commands.length, 100);
  assert(throttledCommands.length < commands.length);
  assert(throttledCommands.length >= 2); // At least start and end

  // Verify critical commands are preserved
  assertEquals(throttledCommands[0].type, "start");
  assertEquals(throttledCommands[throttledCommands.length - 1].type, "end");
});
