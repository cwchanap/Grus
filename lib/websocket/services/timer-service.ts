// Round timer management service
import type { TimerManager } from "../types/websocket-internal.ts";

export class TimerService implements TimerManager {
  private roundTimers: Map<string, number> = new Map();

  startRoundTimer(roomId: string, callback: (roomId: string) => Promise<void>): void {
    // Clear existing timer if any
    this.clearRoundTimer(roomId);

    const timer = setInterval(async () => {
      try {
        await callback(roomId);
      } catch (error) {
        console.error("Error in round timer callback:", error);
        this.clearRoundTimer(roomId);
      }
    }, 1000); // Update every second

    this.roundTimers.set(roomId, timer);
  }

  clearRoundTimer(roomId: string): void {
    const timer = this.roundTimers.get(roomId);
    if (timer) {
      clearInterval(timer);
      this.roundTimers.delete(roomId);
    }
  }

  cleanup(): void {
    // Clear all round timers
    for (const [_roomId, timer] of this.roundTimers.entries()) {
      clearInterval(timer);
    }
    this.roundTimers.clear();
  }
}