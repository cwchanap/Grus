/**
 * Offline mode manager for preserving local state and handling offline scenarios
 */

import { signal } from "@preact/signals";
import type { ChatMessage, DrawingCommand, GameState } from "../types/game.ts";

export interface OfflineState {
  isOffline: boolean;
  lastSyncTime: number | null;
  pendingMessages: ChatMessage[];
  pendingDrawingCommands: DrawingCommand[];
  localGameState: Partial<GameState> | null;
  queuedActions: Array<{
    id: string;
    type: string;
    data: any;
    timestamp: number;
  }>;
}

// Global offline state
export const offlineState = signal<OfflineState>({
  isOffline: false,
  lastSyncTime: null,
  pendingMessages: [],
  pendingDrawingCommands: [],
  localGameState: null,
  queuedActions: [],
});

export class OfflineManager {
  private storageKey: string;
  private syncInterval: number | null = null;

  constructor(roomId: string) {
    this.storageKey = `offline_state_${roomId}`;
    this.loadOfflineState();
    this.setupOnlineOfflineHandlers();
  }

  private setupOnlineOfflineHandlers() {
    const handleOnline = () => {
      this.setOfflineStatus(false);
      this.syncPendingData();
    };

    const handleOffline = () => {
      this.setOfflineStatus(true);
      this.startOfflineMode();
    };

    globalThis.addEventListener("online", handleOnline);
    globalThis.addEventListener("offline", handleOffline);

    // Initial state
    this.setOfflineStatus(!navigator.onLine);
  }

  private setOfflineStatus(isOffline: boolean) {
    offlineState.value = {
      ...offlineState.value,
      isOffline,
    };
    this.saveOfflineState();
  }

  private startOfflineMode() {
    console.log("Entering offline mode - preserving local state");

    // Start periodic state saving
    this.syncInterval = setInterval(() => {
      this.saveOfflineState();
    }, 5000); // Save every 5 seconds
  }

  private stopOfflineMode() {
    if (this.syncInterval) {
      clearInterval(this.syncInterval);
      this.syncInterval = null;
    }
  }

  // Save offline state to localStorage
  private saveOfflineState() {
    try {
      const stateToSave = {
        ...offlineState.value,
        timestamp: Date.now(),
      };
      localStorage.setItem(this.storageKey, JSON.stringify(stateToSave));
    } catch (error) {
      console.error("Failed to save offline state:", error);
    }
  }

  // Load offline state from localStorage
  private loadOfflineState() {
    try {
      const saved = localStorage.getItem(this.storageKey);
      if (saved) {
        const parsedState = JSON.parse(saved);

        // Only restore if saved within last hour
        if (Date.now() - parsedState.timestamp < 3600000) {
          offlineState.value = {
            ...offlineState.value,
            ...parsedState,
            isOffline: !navigator.onLine,
          };
        }
      }
    } catch (error) {
      console.error("Failed to load offline state:", error);
    }
  }

  // Queue a chat message for later sending
  public queueChatMessage(message: ChatMessage) {
    const currentState = offlineState.value;
    offlineState.value = {
      ...currentState,
      pendingMessages: [...currentState.pendingMessages, message],
    };
    this.saveOfflineState();
  }

  // Queue drawing commands for later sending
  public queueDrawingCommands(commands: DrawingCommand[]) {
    const currentState = offlineState.value;
    offlineState.value = {
      ...currentState,
      pendingDrawingCommands: [...currentState.pendingDrawingCommands, ...commands],
    };
    this.saveOfflineState();
  }

  // Queue any action for later execution
  public queueAction(type: string, data: any) {
    const action = {
      id: crypto.randomUUID(),
      type,
      data,
      timestamp: Date.now(),
    };

    const currentState = offlineState.value;
    offlineState.value = {
      ...currentState,
      queuedActions: [...currentState.queuedActions, action],
    };
    this.saveOfflineState();
  }

  // Update local game state
  public updateLocalGameState(gameState: Partial<GameState>) {
    offlineState.value = {
      ...offlineState.value,
      localGameState: {
        ...offlineState.value.localGameState,
        ...gameState,
      },
    };
    this.saveOfflineState();
  }

  // Sync pending data when coming back online
  private async syncPendingData() {
    const state = offlineState.value;

    if (
      state.pendingMessages.length === 0 &&
      state.pendingDrawingCommands.length === 0 &&
      state.queuedActions.length === 0
    ) {
      return;
    }

    console.log("Syncing pending offline data...");

    try {
      // This would typically send data to the server
      // For now, we'll just clear the pending data
      await this.sendPendingData(state);

      // Clear pending data after successful sync
      offlineState.value = {
        ...offlineState.value,
        pendingMessages: [],
        pendingDrawingCommands: [],
        queuedActions: [],
        lastSyncTime: Date.now(),
      };

      this.saveOfflineState();
      this.stopOfflineMode();

      console.log("Offline data synced successfully");
    } catch (error) {
      console.error("Failed to sync offline data:", error);
    }
  }

  // Send pending data to server (placeholder implementation)
  private async sendPendingData(state: OfflineState): Promise<void> {
    // In a real implementation, this would:
    // 1. Send pending chat messages
    // 2. Send pending drawing commands
    // 3. Execute queued actions
    // 4. Reconcile local game state with server state

    return new Promise((resolve) => {
      // Simulate network delay
      setTimeout(resolve, 1000);
    });
  }

  // Get offline status
  public isOffline(): boolean {
    return offlineState.value.isOffline;
  }

  // Get pending data counts
  public getPendingCounts() {
    const state = offlineState.value;
    return {
      messages: state.pendingMessages.length,
      drawings: state.pendingDrawingCommands.length,
      actions: state.queuedActions.length,
    };
  }

  // Clear all offline data
  public clearOfflineData() {
    offlineState.value = {
      isOffline: !navigator.onLine,
      lastSyncTime: null,
      pendingMessages: [],
      pendingDrawingCommands: [],
      localGameState: null,
      queuedActions: [],
    };

    try {
      localStorage.removeItem(this.storageKey);
    } catch (error) {
      console.error("Failed to clear offline data:", error);
    }
  }

  // Destroy and cleanup
  public destroy() {
    this.stopOfflineMode();
    globalThis.removeEventListener("online", this.setupOnlineOfflineHandlers);
    globalThis.removeEventListener("offline", this.setupOnlineOfflineHandlers);
  }
}
