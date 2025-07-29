import { assert, assertEquals, assertExists } from "$std/assert/mod.ts";
import { DrawingCommand } from "../../types/game.ts";
import {
  deserializeDrawingCommand,
  serializeDrawingCommand,
  validateDrawingCommand,
} from "../../lib/drawing-utils.ts";

Deno.test("DrawingEngine Integration - Drawing command flow", () => {
  // Test the complete flow of drawing commands
  const commands: DrawingCommand[] = [];

  const mockOnDrawingCommand = (command: DrawingCommand) => {
    commands.push(command);
  };

  // Simulate a drawing sequence
  const startCommand: DrawingCommand = {
    type: "start",
    x: 100,
    y: 150,
    color: "#ff0000",
    size: 5,
    timestamp: Date.now(),
  };

  const moveCommand: DrawingCommand = {
    type: "move",
    x: 110,
    y: 160,
    color: "#ff0000",
    size: 5,
    timestamp: Date.now() + 10,
  };

  const endCommand: DrawingCommand = {
    type: "end",
    timestamp: Date.now() + 20,
  };

  // Validate commands
  assert(validateDrawingCommand(startCommand));
  assert(validateDrawingCommand(moveCommand));
  assert(validateDrawingCommand(endCommand));

  // Test serialization
  const serializedStart = serializeDrawingCommand(startCommand);
  const deserializedStart = deserializeDrawingCommand(serializedStart);
  assertEquals(deserializedStart, startCommand);

  // Simulate command processing
  mockOnDrawingCommand(startCommand);
  mockOnDrawingCommand(moveCommand);
  mockOnDrawingCommand(endCommand);

  assertEquals(commands.length, 3);
  assertEquals(commands[0].type, "start");
  assertEquals(commands[1].type, "move");
  assertEquals(commands[2].type, "end");
});

Deno.test("DrawingEngine Integration - Clear command", () => {
  const clearCommand: DrawingCommand = {
    type: "clear",
    timestamp: Date.now(),
  };

  assert(validateDrawingCommand(clearCommand));

  const serialized = serializeDrawingCommand(clearCommand);
  const deserialized = deserializeDrawingCommand(serialized);

  // Check essential properties
  assertEquals(deserialized.type, clearCommand.type);
  assertEquals(deserialized.timestamp, clearCommand.timestamp);
});

Deno.test("DrawingEngine Integration - Invalid commands are rejected", () => {
  // Test various invalid commands
  assert(!validateDrawingCommand(null));
  assert(!validateDrawingCommand({}));
  assert(!validateDrawingCommand({ type: "invalid", timestamp: Date.now() }));
  assert(!validateDrawingCommand({ type: "start", timestamp: Date.now() })); // Missing x, y
  assert(!validateDrawingCommand({ type: "start", x: -10, y: 20, timestamp: Date.now() })); // Out of bounds
  assert(
    !validateDrawingCommand({
      type: "start",
      x: 10,
      y: 20,
      color: "invalid",
      timestamp: Date.now(),
    }),
  ); // Invalid color
});

Deno.test("DrawingEngine Integration - Drawing tool interface", () => {
  // Test drawing tool configuration
  interface DrawingTool {
    color: string;
    size: number;
    type: "brush";
  }

  const defaultTool: DrawingTool = {
    color: "#000000",
    size: 5,
    type: "brush",
  };

  // Test tool validation
  assert(defaultTool.color.match(/^#[0-9A-Fa-f]{6}$/));
  assert(defaultTool.size >= 1 && defaultTool.size <= 50);
  assertEquals(defaultTool.type, "brush");

  // Test different tool configurations
  const redTool: DrawingTool = { ...defaultTool, color: "#ff0000" };
  const largeBrush: DrawingTool = { ...defaultTool, size: 20 };

  assert(redTool.color === "#ff0000");
  assert(largeBrush.size === 20);
});
