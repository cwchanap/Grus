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
