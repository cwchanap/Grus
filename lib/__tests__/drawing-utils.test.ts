import { assert, assertEquals, assertExists } from "$std/assert/mod.ts";
import {
  batchDrawingCommands,
  compressDrawingData,
  decompressDrawingData,
  deserializeDrawingCommand,
  DrawingCommandBuffer,
  DrawingCommandThrottler,
  optimizeDrawingCommands,
  serializeDrawingCommand,
  unbatchDrawingCommands,
  validateDrawingCommand,
} from "../games/drawing/drawing-utils.ts";
import { DrawingCommand } from "../../types/games/drawing.ts";

Deno.test("Drawing Utils - Serialization", () => {
  const command: DrawingCommand = {
    type: "start",
    x: 100,
    y: 200,
    color: "#ff0000",
    size: 5,
    timestamp: 1234567890,
  };

  const serialized = serializeDrawingCommand(command);
  assertEquals(serialized.type, "start");
  assertEquals(serialized.x, 100);
  assertEquals(serialized.y, 200);
  assertEquals(serialized.color, "#ff0000");
  assertEquals(serialized.size, 5);
  assertEquals(serialized.timestamp, 1234567890);

  const deserialized = deserializeDrawingCommand(serialized);
  assertEquals(deserialized, command);
});

Deno.test("Drawing Utils - Batching", () => {
  const commands: DrawingCommand[] = [
    { type: "start", x: 10, y: 20, color: "#000000", size: 3, timestamp: 1000 },
    { type: "move", x: 15, y: 25, color: "#000000", size: 3, timestamp: 1001 },
    { type: "end", timestamp: 1002 },
  ];

  const batch = batchDrawingCommands(commands);
  assertExists(batch.batchId);
  assertEquals(batch.commands.length, 3);
  assert(batch.timestamp > 0);

  const unbatched = unbatchDrawingCommands(batch);
  assertEquals(unbatched.length, 3);
  assertEquals(unbatched[0].type, "start");
  assertEquals(unbatched[1].type, "move");
  assertEquals(unbatched[2].type, "end");
});

Deno.test("Drawing Utils - Optimization", () => {
  const commands: DrawingCommand[] = [
    { type: "start", x: 10, y: 20, timestamp: 1000 },
    { type: "move", x: 10, y: 20, timestamp: 1001 }, // Duplicate position
    { type: "move", x: 10, y: 20, timestamp: 1002 }, // Another duplicate
    { type: "move", x: 15, y: 25, timestamp: 1003 },
    { type: "move", x: 15.5, y: 25.5, timestamp: 1004 }, // Very close point
    { type: "move", x: 20, y: 30, timestamp: 1005 },
    { type: "end", timestamp: 1006 },
  ];

  const optimized = optimizeDrawingCommands(commands);

  // Should remove duplicates and very close points
  assert(optimized.length < commands.length);
  assertEquals(optimized[0].type, "start");
  assertEquals(optimized[optimized.length - 1].type, "end");
});

Deno.test("Drawing Utils - Compression", () => {
  const commands: DrawingCommand[] = [
    { type: "start", x: 10.123456, y: 20.987654, color: "#ff0000", size: 5, timestamp: 1000 },
    { type: "move", x: 15.555555, y: 25.444444, color: "#ff0000", size: 5, timestamp: 1001 },
    { type: "end", timestamp: 1002 },
  ];

  const compressed = compressDrawingData(commands);
  assert(typeof compressed === "string");
  assert(compressed.length > 0);

  const decompressed = decompressDrawingData(compressed);
  assertEquals(decompressed.length, 3);
  assertEquals(decompressed[0].type, "start");

  // Check that coordinates are rounded
  assertEquals(decompressed[0].x, 10.1);
  assertEquals(decompressed[0].y, 21.0);
});

Deno.test("Drawing Utils - Validation", () => {
  // Valid commands
  assert(validateDrawingCommand({
    type: "start",
    x: 100,
    y: 200,
    color: "#ff0000",
    size: 5,
    timestamp: 1000,
  }));

  assert(validateDrawingCommand({
    type: "clear",
    timestamp: 1000,
  }));

  // Invalid commands
  assert(!validateDrawingCommand(null));
  assert(!validateDrawingCommand({ type: "invalid", timestamp: 1000 }));
  assert(!validateDrawingCommand({ type: "start", timestamp: 1000 })); // Missing x, y
  assert(!validateDrawingCommand({ type: "start", x: 100, y: 200, timestamp: "invalid" }));
  assert(!validateDrawingCommand({ type: "start", x: -10, y: 200, timestamp: 1000 })); // Out of bounds
  assert(
    !validateDrawingCommand({ type: "start", x: 100, y: 200, color: "invalid", timestamp: 1000 }),
  );
  assert(!validateDrawingCommand({ type: "start", x: 100, y: 200, size: 100, timestamp: 1000 })); // Size too large
});

Deno.test("Drawing Utils - Throttler", async () => {
  const results: DrawingCommand[] = [];
  const throttler = new DrawingCommandThrottler(50); // 50ms interval

  const callback = (command: DrawingCommand) => {
    results.push(command);
  };

  // Send start command (should be immediate)
  throttler.throttle({ type: "start", x: 10, y: 20, timestamp: 1000 }, callback);
  assertEquals(results.length, 1);

  // Send move commands rapidly
  throttler.throttle({ type: "move", x: 15, y: 25, timestamp: 1001 }, callback);
  throttler.throttle({ type: "move", x: 20, y: 30, timestamp: 1002 }, callback);
  throttler.throttle({ type: "move", x: 25, y: 35, timestamp: 1003 }, callback);

  // Should only have start command so far
  assertEquals(results.length, 1);

  // Wait for throttle to flush
  await new Promise((resolve) => setTimeout(resolve, 60));

  // Should now have the last move command
  assertEquals(results.length, 2);
  assertEquals(results[1].type, "move");
  assertEquals(results[1].x, 25);

  // Send end command (should be immediate)
  throttler.throttle({ type: "end", timestamp: 1004 }, callback);
  assertEquals(results.length, 3);
  assertEquals(results[2].type, "end");

  throttler.destroy();
});

Deno.test("Drawing Utils - Buffer", () => {
  const flushedBatches: DrawingCommand[][] = [];
  const buffer = new DrawingCommandBuffer(
    (commands: DrawingCommand[]) => flushedBatches.push(commands),
    3, // maxBufferSize
    50, // flushInterval
  );

  // Add commands that don't trigger immediate flush
  buffer.add({ type: "move", x: 10, y: 20, timestamp: 1000 });
  buffer.add({ type: "move", x: 15, y: 25, timestamp: 1001 });

  // Should not have flushed yet
  assertEquals(flushedBatches.length, 0);

  // Add command that triggers flush (buffer full)
  buffer.add({ type: "move", x: 20, y: 30, timestamp: 1002 });

  // Should have flushed
  assertEquals(flushedBatches.length, 1);
  assertEquals(flushedBatches[0].length, 3);

  // Add command that triggers immediate flush
  buffer.add({ type: "clear", timestamp: 1003 });

  // Should have flushed immediately
  assertEquals(flushedBatches.length, 2);
  assertEquals(flushedBatches[1].length, 1);
  assertEquals(flushedBatches[1][0].type, "clear");

  buffer.destroy();
});

// ============================================================================
// Additional Edge Case Tests
// ============================================================================

Deno.test("Drawing Utils - Validation with boundary values", () => {
  // Test exact boundary values
  assert(validateDrawingCommand({
    type: "start",
    x: 0, // Min valid
    y: 0,
    timestamp: 1000,
  }));

  assert(validateDrawingCommand({
    type: "start",
    x: 2000, // Max valid
    y: 2000,
    timestamp: 1000,
  }));

  // Test just outside boundaries
  assert(!validateDrawingCommand({
    type: "start",
    x: -1, // Just below min
    y: 100,
    timestamp: 1000,
  }));

  assert(!validateDrawingCommand({
    type: "start",
    x: 100,
    y: 2001, // Just above max
    timestamp: 1000,
  }));

  // Test brush size boundaries
  assert(validateDrawingCommand({
    type: "start",
    x: 100,
    y: 100,
    size: 1, // Min valid
    timestamp: 1000,
  }));

  assert(validateDrawingCommand({
    type: "start",
    x: 100,
    y: 100,
    size: 50, // Max valid
    timestamp: 1000,
  }));

  assert(!validateDrawingCommand({
    type: "start",
    x: 100,
    y: 100,
    size: 0, // Below min
    timestamp: 1000,
  }));

  assert(!validateDrawingCommand({
    type: "start",
    x: 100,
    y: 100,
    size: 51, // Above max
    timestamp: 1000,
  }));
});

Deno.test("Drawing Utils - Validation with various color formats", () => {
  // Valid hex colors
  assert(validateDrawingCommand({
    type: "start",
    x: 100,
    y: 100,
    color: "#000000",
    timestamp: 1000,
  }));

  assert(validateDrawingCommand({
    type: "start",
    x: 100,
    y: 100,
    color: "#ABCDEF",
    timestamp: 1000,
  }));

  assert(validateDrawingCommand({
    type: "start",
    x: 100,
    y: 100,
    color: "#123abc",
    timestamp: 1000,
  }));

  // Invalid color formats
  assert(!validateDrawingCommand({
    type: "start",
    x: 100,
    y: 100,
    color: "#fff", // Too short
    timestamp: 1000,
  }));

  assert(!validateDrawingCommand({
    type: "start",
    x: 100,
    y: 100,
    color: "000000", // Missing #
    timestamp: 1000,
  }));

  assert(!validateDrawingCommand({
    type: "start",
    x: 100,
    y: 100,
    color: "#GGGGGG", // Invalid hex chars
    timestamp: 1000,
  }));

  assert(!validateDrawingCommand({
    type: "start",
    x: 100,
    y: 100,
    color: "rgb(255,0,0)", // Wrong format
    timestamp: 1000,
  }));
});

Deno.test("Drawing Utils - Validation with invalid types and structures", () => {
  // Not an object
  assert(!validateDrawingCommand("invalid"));
  assert(!validateDrawingCommand(123));
  assert(!validateDrawingCommand([]));
  assert(!validateDrawingCommand(undefined));

  // Missing required fields
  assert(!validateDrawingCommand({
    type: "start",
    x: 100,
    // Missing y
    timestamp: 1000,
  }));

  assert(!validateDrawingCommand({
    type: "move",
    // Missing x and y
    timestamp: 1000,
  }));

  // Invalid timestamp types
  assert(!validateDrawingCommand({
    type: "clear",
    timestamp: "1000", // String instead of number
  }));

  assert(!validateDrawingCommand({
    type: "clear",
    timestamp: null,
  }));

  // Invalid coordinate types
  assert(!validateDrawingCommand({
    type: "start",
    x: "100", // String instead of number
    y: 100,
    timestamp: 1000,
  }));

  assert(!validateDrawingCommand({
    type: "start",
    x: 100,
    y: NaN,
    timestamp: 1000,
  }));
});

Deno.test("Drawing Utils - Optimization with edge cases", () => {
  // Empty array
  const emptyOptimized = optimizeDrawingCommands([]);
  assertEquals(emptyOptimized.length, 0);

  // Single command
  const single = optimizeDrawingCommands([
    { type: "start", x: 10, y: 20, timestamp: 1000 },
  ]);
  assertEquals(single.length, 1);

  // Only non-move commands
  const nonMove = optimizeDrawingCommands([
    { type: "start", x: 10, y: 20, timestamp: 1000 },
    { type: "end", timestamp: 1001 },
    { type: "clear", timestamp: 1002 },
  ]);
  assertEquals(nonMove.length, 3);

  // All identical positions (should keep only first and last)
  const identical = optimizeDrawingCommands([
    { type: "start", x: 10, y: 20, timestamp: 1000 },
    { type: "move", x: 10, y: 20, timestamp: 1001 },
    { type: "move", x: 10, y: 20, timestamp: 1002 },
    { type: "move", x: 10, y: 20, timestamp: 1003 },
    { type: "end", timestamp: 1004 },
  ]);
  // Should remove duplicate moves but keep start and end
  assert(identical.length < 5);
  assertEquals(identical[0].type, "start");
  assertEquals(identical[identical.length - 1].type, "end");
});

Deno.test("Drawing Utils - Decompression error handling", () => {
  // Invalid JSON
  const invalid1 = decompressDrawingData("not json");
  assertEquals(invalid1.length, 0);

  // Empty string
  const invalid2 = decompressDrawingData("");
  assertEquals(invalid2.length, 0);

  // Malformed JSON
  const invalid3 = decompressDrawingData("{broken");
  assertEquals(invalid3.length, 0);

  // Valid JSON but wrong structure (non-array)
  const invalid4 = decompressDrawingData(JSON.stringify({ not: "array" }));
  assertEquals(invalid4.length, 0);
});

Deno.test("Drawing Utils - Batch ID uniqueness", () => {
  const commands: DrawingCommand[] = [
    { type: "start", x: 10, y: 20, timestamp: 1000 },
  ];

  const batch1 = batchDrawingCommands(commands);
  const batch2 = batchDrawingCommands(commands);
  const batch3 = batchDrawingCommands(commands);

  // All batch IDs should be unique
  assert(batch1.batchId !== batch2.batchId);
  assert(batch2.batchId !== batch3.batchId);
  assert(batch1.batchId !== batch3.batchId);

  // All should start with 'batch_'
  assert(batch1.batchId.startsWith("batch_"));
  assert(batch2.batchId.startsWith("batch_"));
  assert(batch3.batchId.startsWith("batch_"));
});

Deno.test("Drawing Utils - Batch with custom ID", () => {
  const commands: DrawingCommand[] = [
    { type: "start", x: 10, y: 20, timestamp: 1000 },
  ];

  const customId = "custom_batch_123";
  const batch = batchDrawingCommands(commands, customId);

  assertEquals(batch.batchId, customId);
});

Deno.test("Drawing Utils - Throttler with end command", () => {
  const results: DrawingCommand[] = [];
  const throttler = new DrawingCommandThrottler(50);

  const callback = (command: DrawingCommand) => {
    results.push(command);
  };

  // Send move commands
  throttler.throttle({ type: "move", x: 10, y: 20, timestamp: 1000 }, callback);
  throttler.throttle({ type: "move", x: 15, y: 25, timestamp: 1001 }, callback);

  // Send end command (should be immediate)
  throttler.throttle({ type: "end", timestamp: 1002 }, callback);

  assertEquals(results.length, 2); // First move and end
  assertEquals(results[0].type, "move");
  assertEquals(results[1].type, "end");

  throttler.destroy();
});

Deno.test("Drawing Utils - Throttler with clear command", () => {
  const results: DrawingCommand[] = [];
  const throttler = new DrawingCommandThrottler(50);

  const callback = (command: DrawingCommand) => {
    results.push(command);
  };

  // Send move commands
  throttler.throttle({ type: "move", x: 10, y: 20, timestamp: 1000 }, callback);
  throttler.throttle({ type: "move", x: 15, y: 25, timestamp: 1001 }, callback);

  // Send clear command (should be immediate and clear pending)
  throttler.throttle({ type: "clear", timestamp: 1002 }, callback);

  assertEquals(results.length, 2); // First move and clear
  assertEquals(results[0].type, "move");
  assertEquals(results[1].type, "clear");

  throttler.destroy();
});

Deno.test("Drawing Utils - Buffer with timer-based flush", async () => {
  const flushedBatches: DrawingCommand[][] = [];
  const buffer = new DrawingCommandBuffer(
    (commands: DrawingCommand[]) => flushedBatches.push(commands),
    10, // Large buffer size to prevent size-based flush
    50, // flushInterval
  );

  // Add some move commands (won't trigger immediate flush)
  buffer.add({ type: "move", x: 10, y: 20, timestamp: 1000 });
  buffer.add({ type: "move", x: 15, y: 25, timestamp: 1001 });

  // Should not have flushed yet
  assertEquals(flushedBatches.length, 0);

  // Wait for timer-based flush
  await new Promise((resolve) => setTimeout(resolve, 60));

  // Should have flushed due to timer
  assertEquals(flushedBatches.length, 1);
  assertEquals(flushedBatches[0].length, 2);

  buffer.destroy();
});

Deno.test("Drawing Utils - Buffer destroy flushes remaining commands", () => {
  const flushedBatches: DrawingCommand[][] = [];
  const buffer = new DrawingCommandBuffer(
    (commands: DrawingCommand[]) => flushedBatches.push(commands),
    10, // Large buffer size
    1000, // Long interval
  );

  // Add commands
  buffer.add({ type: "move", x: 10, y: 20, timestamp: 1000 });
  buffer.add({ type: "move", x: 15, y: 25, timestamp: 1001 });

  // Should not have flushed yet
  assertEquals(flushedBatches.length, 0);

  // Destroy should flush remaining commands
  buffer.destroy();

  assertEquals(flushedBatches.length, 1);
  assertEquals(flushedBatches[0].length, 2);
});

Deno.test("Drawing Utils - Compression preserves command types", () => {
  const commands: DrawingCommand[] = [
    { type: "start", x: 10, y: 20, color: "#ff0000", size: 5, timestamp: 1000 },
    { type: "move", x: 15, y: 25, color: "#ff0000", size: 5, timestamp: 1001 },
    { type: "move", x: 20, y: 30, color: "#ff0000", size: 5, timestamp: 1002 },
    { type: "end", timestamp: 1003 },
    { type: "clear", timestamp: 1004 },
  ];

  const compressed = compressDrawingData(commands);
  const decompressed = decompressDrawingData(compressed);

  // Should preserve all command types
  const types = decompressed.map((cmd) => cmd.type);
  assert(types.includes("start"));
  assert(types.includes("move"));
  assert(types.includes("end"));
  assert(types.includes("clear"));
});

Deno.test("Drawing Utils - Serialization handles optional fields", () => {
  // Command with all fields
  const fullCommand: DrawingCommand = {
    type: "start",
    x: 100,
    y: 200,
    color: "#ff0000",
    size: 5,
    timestamp: 1000,
  };

  const fullSerialized = serializeDrawingCommand(fullCommand);
  assertEquals(fullSerialized.color, "#ff0000");
  assertEquals(fullSerialized.size, 5);

  // Command with minimal fields (end command)
  const minimalCommand: DrawingCommand = {
    type: "end",
    timestamp: 1000,
  };

  const minimalSerialized = serializeDrawingCommand(minimalCommand);
  assertEquals(minimalSerialized.type, "end");
  assertEquals(minimalSerialized.timestamp, 1000);
  assertEquals(minimalSerialized.x, undefined);
  assertEquals(minimalSerialized.y, undefined);
});

Deno.test("Drawing Utils - Optimization preserves critical commands", () => {
  const commands: DrawingCommand[] = [
    { type: "start", x: 10, y: 20, timestamp: 1000 },
    { type: "clear", timestamp: 1001 },
    { type: "start", x: 10, y: 20, timestamp: 1002 },
    { type: "end", timestamp: 1003 },
  ];

  const optimized = optimizeDrawingCommands(commands);

  // All commands should be preserved (no moves to optimize)
  assertEquals(optimized.length, 4);
  assertEquals(optimized[0].type, "start");
  assertEquals(optimized[1].type, "clear");
  assertEquals(optimized[2].type, "start");
  assertEquals(optimized[3].type, "end");
});

Deno.test("Drawing Utils - Buffer triggers immediate flush for start command", () => {
  const flushedBatches: DrawingCommand[][] = [];
  const buffer = new DrawingCommandBuffer(
    (commands: DrawingCommand[]) => flushedBatches.push(commands),
    10, // Large buffer
    1000, // Long interval
  );

  buffer.add({ type: "start", x: 10, y: 20, timestamp: 1000 });

  // Should flush immediately for start command
  assertEquals(flushedBatches.length, 1);
  assertEquals(flushedBatches[0][0].type, "start");

  buffer.destroy();
});

Deno.test("Drawing Utils - Buffer triggers immediate flush for end command", () => {
  const flushedBatches: DrawingCommand[][] = [];
  const buffer = new DrawingCommandBuffer(
    (commands: DrawingCommand[]) => flushedBatches.push(commands),
    10, // Large buffer
    1000, // Long interval
  );

  buffer.add({ type: "move", x: 10, y: 20, timestamp: 1000 });
  buffer.add({ type: "end", timestamp: 1001 });

  // Should have 1 flush containing move+end batch
  assertEquals(flushedBatches.length, 1);
  assertEquals(flushedBatches[0].length, 2);

  buffer.destroy();
});
