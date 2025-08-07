/**
 * Centralized WebSocket connection manager with reconnection, error handling, and offline support
 */

import { signal } from "@preact/signals";
import type { BaseClientMessage, BaseServerMessage } from "../../types/core/websocket.ts";

export type ConnectionStatus =
  | "connecting"
  | "connected"
  | "disconnected"
  | "reconnecting"
  | "offline";

export interface ConnectionManagerOptions {
  maxReconnectAttempts?: number;
  baseReconnectDelay?: number;
  maxReconnectDelay?: number;
  heartbeatInterval?: number;
  offlineThreshold?: number;
}

export interface ConnectionState {
  status: ConnectionStatus;
  reconnectAttempts: number;
  lastConnected: number | null;
  error: string | null;
  isOnline: boolean;
}

// Global connection state signals
export const connectionState = signal<ConnectionState>({
  status: "disconnected",
  reconnectAttempts: 0,
  lastConnected: null,
  error: null,
  isOnline: typeof navigator !== "undefined" ? navigator.onLine : true,
});

export class WebSocketConnectionManager {
  private ws: WebSocket | null = null;
  private roomId: string;
  private playerId: string;
  private playerName: string;
  private options: Required<ConnectionManagerOptions>;
  private reconnectTimeout: number | null = null;
  private heartbeatInterval: number | null = null;
  private messageQueue: BaseClientMessage[] = [];
  private messageHandlers: Map<string, (message: BaseServerMessage) => void> = new Map();
  private isDestroyed = false;

  constructor(
    roomId: string,
    playerId: string,
    playerName: string,
    options: ConnectionManagerOptions = {},
  ) {
    this.roomId = roomId;
    this.playerId = playerId;
    this.playerName = playerName;

    this.options = {
      maxReconnectAttempts: options.maxReconnectAttempts ?? 10,
      baseReconnectDelay: options.baseReconnectDelay ?? 1000,
      maxReconnectDelay: options.maxReconnectDelay ?? 30000,
      heartbeatInterval: options.heartbeatInterval ?? 30000,
      offlineThreshold: options.offlineThreshold ?? 5000,
    };

    // Listen for online/offline events (only in browser)
    if (typeof window !== "undefined") {
      this.setupOnlineOfflineHandlers();
    }

    // Start connection
    this.connect();
  }

  private setupOnlineOfflineHandlers() {
    const handleOnline = () => {
      connectionState.value = { ...connectionState.value, isOnline: true };
      if (connectionState.value.status === "offline") {
        this.connect();
      }
    };

    const handleOffline = () => {
      connectionState.value = {
        ...connectionState.value,
        isOnline: false,
        status: "offline",
      };
      this.cleanup();
    };

    if (typeof window !== "undefined") {
      globalThis.addEventListener("online", handleOnline);
      globalThis.addEventListener("offline", handleOffline);

      // Cleanup listeners when destroyed
      const originalDestroy = this.destroy.bind(this);
      this.destroy = () => {
        globalThis.removeEventListener("online", handleOnline);
        globalThis.removeEventListener("offline", handleOffline);
        originalDestroy();
      };
    }
  }

  private updateConnectionState(updates: Partial<ConnectionState>) {
    connectionState.value = { ...connectionState.value, ...updates };
  }

  private connect() {
    if (this.isDestroyed || (typeof navigator !== "undefined" && !navigator.onLine)) {
      this.updateConnectionState({ status: "offline" });
      return;
    }

    // Enable WebSocket in development environment
    console.log("Enabling WebSocket connection in development environment");

    this.cleanup();
    this.updateConnectionState({
      status: "connecting",
      error: null,
    });

    try {
      const protocol = typeof window !== "undefined" && globalThis.location.protocol === "https:"
        ? "wss:"
        : "ws:";
      const host = typeof window !== "undefined" ? globalThis.location.host : "localhost:8000";
      const wsUrl = `${protocol}//${host}/api/websocket?roomId=${this.roomId}`;

      this.ws = new WebSocket(wsUrl);
      this.setupWebSocketHandlers();
    } catch (error) {
      console.error("Failed to create WebSocket connection:", error);
      this.handleConnectionError(error instanceof Error ? error.message : "Connection failed");
    }
  }

  private setupWebSocketHandlers() {
    if (!this.ws) return;

    this.ws.onopen = () => {
      console.log("WebSocket connected");
      this.updateConnectionState({
        status: "connected",
        reconnectAttempts: 0,
        lastConnected: Date.now(),
        error: null,
      });

      // Join room
      this.sendMessage({
        type: "join-room",
        roomId: this.roomId,
        playerId: this.playerId,
        data: { playerName: this.playerName },
      });

      // Process queued messages
      this.processMessageQueue();

      // Start heartbeat
      this.startHeartbeat();
    };

    this.ws.onmessage = (event) => {
      try {
        const message: BaseServerMessage = JSON.parse(event.data);
        this.handleMessage(message);
      } catch (error) {
        console.error("Error parsing WebSocket message:", error);
      }
    };

    this.ws.onclose = (event) => {
      console.log("WebSocket disconnected:", event.code, event.reason);
      this.updateConnectionState({ status: "disconnected" });
      this.stopHeartbeat();

      if (!this.isDestroyed && (typeof navigator === "undefined" || navigator.onLine)) {
        this.scheduleReconnect();
      }
    };

    this.ws.onerror = (error) => {
      console.error("WebSocket error:", error);
      this.handleConnectionError("Connection error");
    };
  }

  private handleMessage(message: BaseServerMessage) {
    // Handle system messages
    if (message.type === "error") {
      this.updateConnectionState({ error: message.data?.message || "Unknown error" });
      return;
    }

    if (message.type === "pong") {
      // Heartbeat response received
      return;
    }

    // Route message to registered handlers
    const handler = this.messageHandlers.get(message.type);
    if (handler) {
      handler(message);
    } else {
      console.warn("No handler registered for message type:", message.type);
    }
  }

  private handleConnectionError(error: string) {
    this.updateConnectionState({
      status: "disconnected",
      error,
    });
    this.stopHeartbeat();

    if (!this.isDestroyed && (typeof navigator === "undefined" || navigator.onLine)) {
      this.scheduleReconnect();
    }
  }

  private scheduleReconnect() {
    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout);
    }

    const currentAttempts = connectionState.value.reconnectAttempts;

    if (currentAttempts >= this.options.maxReconnectAttempts) {
      this.updateConnectionState({
        status: "disconnected",
        error: "Max reconnection attempts reached",
      });
      return;
    }

    // Exponential backoff with jitter
    const baseDelay = this.options.baseReconnectDelay;
    const exponentialDelay = Math.min(
      baseDelay * Math.pow(2, currentAttempts),
      this.options.maxReconnectDelay,
    );

    // Add jitter (±25%)
    const jitter = exponentialDelay * 0.25 * (Math.random() - 0.5);
    const delay = Math.max(1000, exponentialDelay + jitter);

    this.updateConnectionState({
      status: "reconnecting",
      reconnectAttempts: currentAttempts + 1,
    });

    console.log(
      `Reconnecting in ${Math.round(delay)}ms (attempt ${
        currentAttempts + 1
      }/${this.options.maxReconnectAttempts})`,
    );

    this.reconnectTimeout = setTimeout(() => {
      this.connect();
    }, delay);
  }

  private startHeartbeat() {
    this.stopHeartbeat();

    this.heartbeatInterval = setInterval(() => {
      if (this.ws?.readyState === WebSocket.OPEN) {
        this.sendMessage({
          type: "ping",
          roomId: this.roomId,
          playerId: this.playerId,
          data: { timestamp: Date.now() },
        });
      }
    }, this.options.heartbeatInterval);
  }

  private stopHeartbeat() {
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval);
      this.heartbeatInterval = null;
    }
  }

  private processMessageQueue() {
    while (this.messageQueue.length > 0) {
      const message = this.messageQueue.shift();
      if (message) {
        this.sendMessage(message);
      }
    }
  }

  private cleanup() {
    this.stopHeartbeat();

    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout);
      this.reconnectTimeout = null;
    }

    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  private isDevelopmentEnvironment(): boolean {
    return typeof window !== "undefined" && (
      globalThis.location.hostname === "localhost" ||
      globalThis.location.hostname === "127.0.0.1" ||
      globalThis.location.hostname.includes("dev")
    );
  }

  // Public API
  public sendMessage(message: BaseClientMessage): boolean {
    if (typeof navigator !== "undefined" && !navigator.onLine) {
      this.updateConnectionState({ status: "offline" });
      return false;
    }

    if (this.ws?.readyState === WebSocket.OPEN) {
      try {
        this.ws.send(JSON.stringify(message));
        return true;
      } catch (error) {
        console.error("Error sending message:", error);
        this.handleConnectionError("Send failed");
        return false;
      }
    } else {
      // Queue message for later if we're connecting/reconnecting
      if (
        connectionState.value.status === "connecting" ||
        connectionState.value.status === "reconnecting"
      ) {
        this.messageQueue.push(message);
        return true;
      }
      return false;
    }
  }

  public onMessage(type: string, handler: (message: BaseServerMessage) => void) {
    this.messageHandlers.set(type, handler);
  }

  public offMessage(type: string) {
    this.messageHandlers.delete(type);
  }

  public forceReconnect() {
    this.updateConnectionState({ reconnectAttempts: 0 });
    this.connect();
  }

  public getConnectionState(): ConnectionState {
    return connectionState.value;
  }

  public isConnected(): boolean {
    return connectionState.value.status === "connected";
  }

  public destroy() {
    this.isDestroyed = true;
    this.cleanup();
    this.messageHandlers.clear();
    this.messageQueue.length = 0;
  }
}
