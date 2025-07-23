import { DrawingCommand } from "../types/game.ts";

/**
 * Utility functions for drawing command serialization and optimization
 */

export interface SerializedDrawingCommand {
  type: string;
  x?: number;
  y?: number;
  color?: string;
  size?: number;
  timestamp: number;
}

export interface DrawingCommandBatch {
  commands: SerializedDrawingCommand[];
  batchId: string;
  timestamp: number;
}

/**
 * Serialize a drawing command for network transmission
 */
export function serializeDrawingCommand(command: DrawingCommand): SerializedDrawingCommand {
  return {
    type: command.type,
    x: command.x,
    y: command.y,
    color: command.color,
    size: command.size,
    timestamp: command.timestamp,
  };
}

/**
 * Deserialize a drawing command from network data
 */
export function deserializeDrawingCommand(data: SerializedDrawingCommand): DrawingCommand {
  return {
    type: data.type as DrawingCommand['type'],
    x: data.x,
    y: data.y,
    color: data.color,
    size: data.size,
    timestamp: data.timestamp,
  };
}

/**
 * Batch multiple drawing commands for efficient network transmission
 */
export function batchDrawingCommands(
  commands: DrawingCommand[],
  batchId?: string
): DrawingCommandBatch {
  return {
    commands: commands.map(serializeDrawingCommand),
    batchId: batchId || generateBatchId(),
    timestamp: Date.now(),
  };
}

/**
 * Unbatch drawing commands from network data
 */
export function unbatchDrawingCommands(batch: DrawingCommandBatch): DrawingCommand[] {
  return batch.commands.map(deserializeDrawingCommand);
}

/**
 * Generate a unique batch ID
 */
function generateBatchId(): string {
  return `batch_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
}

/**
 * Optimize drawing commands by removing redundant data
 */
export function optimizeDrawingCommands(commands: DrawingCommand[]): DrawingCommand[] {
  const optimized: DrawingCommand[] = [];
  let lastCommand: DrawingCommand | null = null;

  for (const command of commands) {
    // Skip duplicate consecutive move commands with same position
    if (
      command.type === 'move' &&
      lastCommand?.type === 'move' &&
      command.x === lastCommand.x &&
      command.y === lastCommand.y
    ) {
      continue;
    }

    // Simplify path by reducing points that are very close together
    if (
      command.type === 'move' &&
      lastCommand?.type === 'move' &&
      command.x !== undefined &&
      command.y !== undefined &&
      lastCommand.x !== undefined &&
      lastCommand.y !== undefined
    ) {
      const distance = Math.sqrt(
        Math.pow(command.x - lastCommand.x, 2) + 
        Math.pow(command.y - lastCommand.y, 2)
      );
      
      // Skip points that are too close (less than 2 pixels apart)
      if (distance < 2) {
        continue;
      }
    }

    optimized.push(command);
    lastCommand = command;
  }

  return optimized;
}

/**
 * Compress drawing data for network transmission
 */
export function compressDrawingData(commands: DrawingCommand[]): string {
  const optimized = optimizeDrawingCommands(commands);
  const serialized = optimized.map(serializeDrawingCommand);
  
  // Simple compression: remove redundant properties
  const compressed = serialized.map(cmd => {
    const result: any = { t: cmd.type, ts: cmd.timestamp };
    if (cmd.x !== undefined) result.x = Math.round(cmd.x * 10) / 10; // Round to 1 decimal
    if (cmd.y !== undefined) result.y = Math.round(cmd.y * 10) / 10;
    if (cmd.color !== undefined) result.c = cmd.color;
    if (cmd.size !== undefined) result.s = cmd.size;
    return result;
  });

  return JSON.stringify(compressed);
}

/**
 * Decompress drawing data from network transmission
 */
export function decompressDrawingData(compressedData: string): DrawingCommand[] {
  try {
    const compressed = JSON.parse(compressedData);
    
    return compressed.map((cmd: any) => ({
      type: cmd.t,
      x: cmd.x,
      y: cmd.y,
      color: cmd.c,
      size: cmd.s,
      timestamp: cmd.ts,
    }));
  } catch (error) {
    console.error('Failed to decompress drawing data:', error);
    return [];
  }
}

/**
 * Validate drawing command data
 */
export function validateDrawingCommand(command: any): command is DrawingCommand {
  if (!command || typeof command !== 'object') {
    return false;
  }

  const validTypes = ['start', 'move', 'end', 'clear'];
  if (!validTypes.includes(command.type)) {
    return false;
  }

  if (typeof command.timestamp !== 'number') {
    return false;
  }

  // Validate coordinates for start and move commands
  if (command.type === 'start' || command.type === 'move') {
    if (typeof command.x !== 'number' || typeof command.y !== 'number') {
      return false;
    }
    
    // Basic bounds checking (assuming reasonable canvas size)
    if (command.x < 0 || command.x > 2000 || command.y < 0 || command.y > 2000) {
      return false;
    }
  }

  // Validate color format
  if (command.color !== undefined) {
    if (typeof command.color !== 'string' || !command.color.match(/^#[0-9A-Fa-f]{6}$/)) {
      return false;
    }
  }

  // Validate brush size
  if (command.size !== undefined) {
    if (typeof command.size !== 'number' || command.size < 1 || command.size > 50) {
      return false;
    }
  }

  return true;
}

/**
 * Throttle drawing commands to prevent spam
 */
export class DrawingCommandThrottler {
  private lastCommandTime = 0;
  private minInterval: number;
  private pendingCommand: DrawingCommand | null = null;
  private timeoutId: number | null = null;

  constructor(minIntervalMs = 16) { // ~60fps
    this.minInterval = minIntervalMs;
  }

  throttle(command: DrawingCommand, callback: (command: DrawingCommand) => void): void {
    const now = Date.now();
    
    if (command.type === 'start' || command.type === 'end' || command.type === 'clear') {
      // Always send these commands immediately
      this.clearPending();
      callback(command);
      this.lastCommandTime = now;
      return;
    }

    if (now - this.lastCommandTime >= this.minInterval) {
      // Send immediately if enough time has passed
      this.clearPending();
      callback(command);
      this.lastCommandTime = now;
    } else {
      // Queue the command to be sent later
      this.pendingCommand = command;
      
      if (this.timeoutId === null) {
        const delay = this.minInterval - (now - this.lastCommandTime);
        this.timeoutId = setTimeout(() => {
          if (this.pendingCommand) {
            callback(this.pendingCommand);
            this.lastCommandTime = Date.now();
          }
          this.clearPending();
        }, delay);
      }
    }
  }

  private clearPending(): void {
    this.pendingCommand = null;
    if (this.timeoutId !== null) {
      clearTimeout(this.timeoutId);
      this.timeoutId = null;
    }
  }

  destroy(): void {
    this.clearPending();
  }
}

/**
 * Drawing command buffer for handling network latency
 */
export class DrawingCommandBuffer {
  private buffer: DrawingCommand[] = [];
  private maxBufferSize: number;
  private flushInterval: number;
  private flushCallback: (commands: DrawingCommand[]) => void;
  private intervalId: number | null = null;

  constructor(
    flushCallback: (commands: DrawingCommand[]) => void,
    maxBufferSize = 50,
    flushIntervalMs = 100
  ) {
    this.flushCallback = flushCallback;
    this.maxBufferSize = maxBufferSize;
    this.flushInterval = flushIntervalMs;
    this.startFlushing();
  }

  add(command: DrawingCommand): void {
    this.buffer.push(command);
    
    // Flush immediately for critical commands or when buffer is full
    if (
      command.type === 'start' || 
      command.type === 'end' || 
      command.type === 'clear' ||
      this.buffer.length >= this.maxBufferSize
    ) {
      this.flush();
    }
  }

  flush(): void {
    if (this.buffer.length === 0) return;
    
    const commands = [...this.buffer];
    this.buffer = [];
    this.flushCallback(commands);
  }

  private startFlushing(): void {
    this.intervalId = setInterval(() => {
      this.flush();
    }, this.flushInterval);
  }

  destroy(): void {
    if (this.intervalId !== null) {
      clearInterval(this.intervalId);
      this.intervalId = null;
    }
    this.flush(); // Flush any remaining commands
  }
}