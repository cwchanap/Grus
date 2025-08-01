// Rate limiting utility for WebSocket messages
import { getConfig } from "../../config.ts";
import type { RateLimitInfo } from "../types/websocket-internal.ts";

export class RateLimiter {
  private rateLimits: Map<string, RateLimitInfo> = new Map();

  checkRateLimit(playerId: string, messageType: string): boolean {
    const config = getConfig();
    const now = Date.now();
    const limit = this.rateLimits.get(playerId) || {
      messages: 0,
      drawing: 0,
      lastReset: now,
    };

    // Reset counters every minute
    if (now - limit.lastReset > 60000) {
      limit.messages = 0;
      limit.drawing = 0;
      limit.lastReset = now;
    }

    // Check limits based on message type
    if (messageType === "draw") {
      if (limit.drawing >= config.security.rateLimitDrawing) {
        return false;
      }
      limit.drawing++;
    } else {
      if (limit.messages >= config.security.rateLimitMessages) {
        return false;
      }
      limit.messages++;
    }

    this.rateLimits.set(playerId, limit);
    return true;
  }

  cleanup(): void {
    this.rateLimits.clear();
  }
}