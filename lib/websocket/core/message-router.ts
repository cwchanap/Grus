// Message routing and validation
import type { ClientMessage } from "../../../types/game.ts";
import type { WebSocketConnection, MessageHandler } from "../types/websocket-internal.ts";
import { MessageValidator } from "../utils/message-validator.ts";
import { RateLimiter } from "../utils/rate-limiter.ts";
import { ConnectionPool } from "./connection-pool.ts";

export class MessageRouter {
  private handlers: Map<string, MessageHandler> = new Map();
  private validator: MessageValidator;
  private rateLimiter: RateLimiter;
  private connectionPool: ConnectionPool;

  constructor(connectionPool: ConnectionPool) {
    this.connectionPool = connectionPool;
    this.validator = new MessageValidator();
    this.rateLimiter = new RateLimiter();
  }

  registerHandler(messageType: string, handler: MessageHandler): void {
    this.handlers.set(messageType, handler);
  }

  async routeMessage(connection: WebSocketConnection, data: string): Promise<void> {
    try {
      // Validate data before parsing
      if (!data || typeof data !== 'string' || data.length === 0) {
        console.error("Invalid message data:", typeof data, data);
        return;
      }

      let message: ClientMessage;
      try {
        message = JSON.parse(data);
      } catch (parseError) {
        console.error("JSON parse error:", parseError, "Data:", data);
        if (connection.playerId) {
          this.connectionPool.sendError(connection.playerId, "Invalid JSON format");
        }
        return;
      }

      // Validate message structure
      if (!this.validator.validateClientMessage(message)) {
        console.error("Message validation failed:", {
          message,
          hasType: 'type' in message,
          hasRoomId: 'roomId' in message,
          hasPlayerId: 'playerId' in message,
          hasData: 'data' in message,
          typeOfMessage: typeof message
        });
        if (connection.playerId) {
          this.connectionPool.sendError(connection.playerId, "Invalid message format");
        }
        return;
      }

      // Check rate limits
      if (!this.rateLimiter.checkRateLimit(message.playerId, message.type)) {
        if (connection.playerId) {
          this.connectionPool.sendError(connection.playerId, "Rate limit exceeded");
        }
        return;
      }

      // Route to appropriate handler
      const handler = this.handlers.get(message.type);
      if (!handler) {
        if (connection.playerId) {
          this.connectionPool.sendError(connection.playerId, "Unknown message type");
        }
        return;
      }

      await handler.handle(connection, message);
    } catch (error) {
      console.error("Message routing error:", error instanceof Error ? error.message : error);
      if (connection.playerId) {
        this.connectionPool.sendError(connection.playerId, "Internal server error");
      }
    }
  }

  cleanup(): void {
    this.rateLimiter.cleanup();
    this.handlers.clear();
  }
}