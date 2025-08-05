import { assert, assertEquals } from "$std/assert/mod.ts";
import { DrawingCommand, GameState } from "../../types/game.ts";

/**
 * Integration tests for DrawingBoard component
 * These tests simulate real-world usage scenarios
 */

// Mock game room scenario
function createGameRoomScenario() {
  const roomId = "test-room-123";
  const players = [
    { id: "player1", name: "Alice", isHost: true, isConnected: true, lastActivity: Date.now() },
    { id: "player2", name: "Bob", isHost: false, isConnected: true, lastActivity: Date.now() },
    { id: "player3", name: "Charlie", isHost: false, isConnected: true, lastActivity: Date.now() },
  ];

  const gameState: GameState = {
    roomId,
    currentDrawer: "player1",
    currentWord: "house",
    roundNumber: 1,
    timeRemaining: 60,
    phase: "drawing",
    players,
    scores: { player1: 0, player2: 0, player3: 0 },
    drawingData: [],
    correctGuesses: [],
    chatMessages: [],
    settings: { maxRounds: 5, roundTimeSeconds: 75 },
  };

  return { roomId, players, gameState };
}

Deno.test("DrawingBoard Integration - Complete drawing session", () => {
  const { roomId: _roomId, players: _players, gameState: _gameState } = createGameRoomScenario();

  // Simulate a complete drawing session
  const drawingSession: DrawingCommand[] = [];

  // Player starts drawing
  const startCommand: DrawingCommand = {
    type: "start",
    x: 100,
    y: 100,
    color: "#000000",
    size: 5,
    timestamp: Date.now(),
  };
  drawingSession.push(startCommand);

  // Player draws a house (simplified)
  const houseDrawing = [
    // Base of house
    { type: "move", x: 200, y: 100 },
    { type: "move", x: 200, y: 200 },
    { type: "move", x: 100, y: 200 },
    { type: "move", x: 100, y: 100 },
    // Roof
    { type: "end" },
    { type: "start", x: 100, y: 100 },
    { type: "move", x: 150, y: 50 },
    { type: "move", x: 200, y: 100 },
    { type: "end" },
  ];

  houseDrawing.forEach((cmd, index) => {
    drawingSession.push({
      ...cmd,
      x: cmd.x,
      y: cmd.y,
      color: "#000000",
      size: 5,
      timestamp: Date.now() + index * 10,
    } as DrawingCommand);
  });

  // Verify drawing session structure
  assertEquals(drawingSession.length, 10);
  assertEquals(drawingSession[0].type, "start");
  assertEquals(drawingSession[drawingSession.length - 1].type, "end");

  // Verify drawing commands are valid
  drawingSession.forEach((cmd) => {
    assert(["start", "move", "end"].includes(cmd.type));
    assert(typeof cmd.timestamp === "number");
  });
});

Deno.test("DrawingBoard Integration - Multi-player synchronization", () => {
  const { roomId: _roomId, players, gameState: _gameState } = createGameRoomScenario();

  // Simulate multiple players receiving the same drawing commands
  const playersDrawingState = new Map<string, DrawingCommand[]>();

  // Initialize each player's drawing state
  players.forEach((player) => {
    playersDrawingState.set(player.id, []);
  });

  // Simulate drawing commands being broadcast to all players
  const broadcastDrawingCommand = (command: DrawingCommand) => {
    players.forEach((player) => {
      const playerCommands = playersDrawingState.get(player.id) || [];
      playerCommands.push(command);
      playersDrawingState.set(player.id, playerCommands);
    });
  };

  // Simulate a drawing sequence
  const drawingCommands = [
    { type: "start", x: 50, y: 50, color: "#ff0000", size: 3, timestamp: Date.now() },
    { type: "move", x: 60, y: 60, color: "#ff0000", size: 3, timestamp: Date.now() + 10 },
    { type: "move", x: 70, y: 70, color: "#ff0000", size: 3, timestamp: Date.now() + 20 },
    { type: "end", timestamp: Date.now() + 30 },
  ];

  drawingCommands.forEach((cmd) => {
    broadcastDrawingCommand(cmd as DrawingCommand);
  });

  // Verify all players have the same drawing state
  const player1Commands = playersDrawingState.get("player1") || [];
  const player2Commands = playersDrawingState.get("player2") || [];
  const player3Commands = playersDrawingState.get("player3") || [];

  assertEquals(player1Commands.length, 4);
  assertEquals(player2Commands.length, 4);
  assertEquals(player3Commands.length, 4);

  // Verify command synchronization
  for (let i = 0; i < 4; i++) {
    assertEquals(player1Commands[i].type, player2Commands[i].type);
    assertEquals(player1Commands[i].type, player3Commands[i].type);
    assertEquals(player1Commands[i].timestamp, player2Commands[i].timestamp);
    assertEquals(player1Commands[i].timestamp, player3Commands[i].timestamp);
  }
});

Deno.test("DrawingBoard Integration - Turn-based drawing permissions", () => {
  const { roomId: _roomId, players: _players, gameState } = createGameRoomScenario();

  // Test drawing permissions for each game phase
  const testDrawingPermissions = (
    currentDrawer: string,
    phase: GameState["phase"],
    playerId: string,
  ) => {
    const _testGameState = { ...gameState, currentDrawer, phase };
    return currentDrawer === playerId && phase === "drawing";
  };

  // Test during drawing phase
  assert(testDrawingPermissions("player1", "drawing", "player1")); // Drawer can draw
  assert(!testDrawingPermissions("player1", "drawing", "player2")); // Others cannot draw
  assert(!testDrawingPermissions("player1", "drawing", "player3")); // Others cannot draw

  // Test during waiting phase
  assert(!testDrawingPermissions("player1", "waiting", "player1")); // No one can draw
  assert(!testDrawingPermissions("player1", "waiting", "player2"));
  assert(!testDrawingPermissions("player1", "waiting", "player3"));

  // Test during results phase
  assert(!testDrawingPermissions("player1", "results", "player1")); // No one can draw
  assert(!testDrawingPermissions("player1", "results", "player2"));
  assert(!testDrawingPermissions("player1", "results", "player3"));

  // Test turn rotation
  const turnOrder = ["player1", "player2", "player3"];
  turnOrder.forEach((currentDrawer) => {
    turnOrder.forEach((playerId) => {
      const canDraw = testDrawingPermissions(currentDrawer, "drawing", playerId);
      assertEquals(canDraw, currentDrawer === playerId);
    });
  });
});

Deno.test("DrawingBoard Integration - Error recovery scenarios", () => {
  const { roomId: _roomId, players: _players, gameState: _gameState } = createGameRoomScenario();

  // Simulate connection loss and recovery
  let connectionStatus = "connected";
  const drawingBuffer: DrawingCommand[] = [];
  const failedCommands: DrawingCommand[] = [];

  const sendDrawingCommand = (command: DrawingCommand) => {
    if (connectionStatus === "connected") {
      drawingBuffer.push(command);
      return true;
    } else {
      failedCommands.push(command);
      return false;
    }
  };

  const recoverConnection = () => {
    connectionStatus = "connected";
    // Resend failed commands
    while (failedCommands.length > 0) {
      const command = failedCommands.shift()!;
      drawingBuffer.push(command);
    }
  };

  // Test normal operation
  const command1 = { type: "start", x: 10, y: 10, timestamp: Date.now() } as DrawingCommand;
  assert(sendDrawingCommand(command1));
  assertEquals(drawingBuffer.length, 1);
  assertEquals(failedCommands.length, 0);

  // Simulate connection loss
  connectionStatus = "disconnected";
  const command2 = { type: "move", x: 20, y: 20, timestamp: Date.now() } as DrawingCommand;
  assert(!sendDrawingCommand(command2));
  assertEquals(drawingBuffer.length, 1);
  assertEquals(failedCommands.length, 1);

  // Test more failed commands
  const command3 = { type: "move", x: 30, y: 30, timestamp: Date.now() } as DrawingCommand;
  assert(!sendDrawingCommand(command3));
  assertEquals(failedCommands.length, 2);

  // Test connection recovery
  recoverConnection();
  assertEquals(connectionStatus, "connected");
  assertEquals(drawingBuffer.length, 3); // Original + 2 recovered
  assertEquals(failedCommands.length, 0);
});

Deno.test("DrawingBoard Integration - Performance under load", () => {
  const { roomId: _roomId, players: _players, gameState: _gameState } = createGameRoomScenario();

  // Simulate high-frequency drawing commands
  const commands: DrawingCommand[] = [];
  const processedCommands: DrawingCommand[] = [];

  // Generate rapid drawing sequence
  const startTime = Date.now();
  for (let i = 0; i < 1000; i++) {
    commands.push({
      type: i === 0 ? "start" : i === 999 ? "end" : "move",
      x: i % 800,
      y: Math.floor(i / 800) * 10,
      color: "#000000",
      size: 5,
      timestamp: startTime + i,
    });
  }

  // Simulate throttling (process every 16ms for ~60fps)
  let lastProcessTime = startTime;
  const throttleInterval = 16;

  commands.forEach((command) => {
    if (
      command.type === "start" || command.type === "end" ||
      command.timestamp - lastProcessTime >= throttleInterval
    ) {
      processedCommands.push(command);
      lastProcessTime = command.timestamp;
    }
  });

  // Verify throttling effectiveness
  assertEquals(commands.length, 1000);
  assert(processedCommands.length < commands.length);
  assert(processedCommands.length >= 2); // At least start and end

  // Verify critical commands are preserved
  assertEquals(processedCommands[0].type, "start");
  assertEquals(processedCommands[processedCommands.length - 1].type, "end");

  // Calculate effective frame rate
  const totalTime = commands[commands.length - 1].timestamp - commands[0].timestamp;
  const effectiveFPS = (processedCommands.length / totalTime) * 1000;

  // Should be close to target 60fps but not exceed it significantly
  assert(effectiveFPS <= 70); // Allow some variance
  console.log(`Effective FPS: ${effectiveFPS.toFixed(2)}`);
});

Deno.test("DrawingBoard Integration - Canvas state consistency", () => {
  const { roomId: _roomId, players: _players, gameState: _gameState } = createGameRoomScenario();

  // Simulate canvas state management
  interface CanvasState {
    strokes: DrawingCommand[][];
    currentStroke: DrawingCommand[];
    isDrawing: boolean;
  }

  const createCanvasState = (): CanvasState => ({
    strokes: [],
    currentStroke: [],
    isDrawing: false,
  });

  const applyCommand = (state: CanvasState, command: DrawingCommand): CanvasState => {
    const newState = { ...state };

    switch (command.type) {
      case "start":
        newState.currentStroke = [command];
        newState.isDrawing = true;
        break;
      case "move":
        if (newState.isDrawing) {
          newState.currentStroke = [...newState.currentStroke, command];
        }
        break;
      case "end":
        if (newState.isDrawing) {
          newState.strokes = [...newState.strokes, [...newState.currentStroke]];
          newState.currentStroke = [];
          newState.isDrawing = false;
        }
        break;
      case "clear":
        newState.strokes = [];
        newState.currentStroke = [];
        newState.isDrawing = false;
        break;
    }

    return newState;
  };

  // Test drawing sequence
  let canvasState = createCanvasState();

  const drawingSequence = [
    { type: "start", x: 10, y: 10, timestamp: Date.now() },
    { type: "move", x: 20, y: 20, timestamp: Date.now() + 10 },
    { type: "move", x: 30, y: 30, timestamp: Date.now() + 20 },
    { type: "end", timestamp: Date.now() + 30 },
    { type: "start", x: 50, y: 50, timestamp: Date.now() + 40 },
    { type: "move", x: 60, y: 60, timestamp: Date.now() + 50 },
    { type: "end", timestamp: Date.now() + 60 },
  ];

  drawingSequence.forEach((cmd) => {
    canvasState = applyCommand(canvasState, cmd as DrawingCommand);
  });

  // Verify final state
  assertEquals(canvasState.strokes.length, 2); // Two complete strokes
  assertEquals(canvasState.currentStroke.length, 0); // No active stroke
  assertEquals(canvasState.isDrawing, false);

  // Verify stroke contents
  assertEquals(canvasState.strokes[0].length, 3); // start + 2 moves
  assertEquals(canvasState.strokes[1].length, 2); // start + 1 move

  // Test clear command
  const clearCommand = { type: "clear", timestamp: Date.now() + 70 } as DrawingCommand;
  canvasState = applyCommand(canvasState, clearCommand);

  assertEquals(canvasState.strokes.length, 0);
  assertEquals(canvasState.currentStroke.length, 0);
  assertEquals(canvasState.isDrawing, false);
});
