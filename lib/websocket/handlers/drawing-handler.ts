// Drawing operations handler
import type { ClientMessage } from "../../../types/game.ts";
import type { MessageHandler, WebSocketConnection } from "../types/websocket-internal.ts";
import { ConnectionPool } from "../core/connection-pool.ts";
import { MessageValidator } from "../utils/message-validator.ts";
import { GameStateService } from "../services/game-state-service.ts";

export class DrawingHandler implements MessageHandler {
  private connectionPool: ConnectionPool;
  private validator: MessageValidator;
  private gameStateService: GameStateService;

  constructor(
    connectionPool: ConnectionPool,
    validator: MessageValidator,
    gameStateService: GameStateService,
  ) {
    this.connectionPool = connectionPool;
    this.validator = validator;
    this.gameStateService = gameStateService;
  }

  async handle(connection: WebSocketConnection, message: ClientMessage): Promise<void> {
    if (message.type !== "draw") {
      throw new Error(`Unsupported message type: ${message.type}`);
    }

    await this.handleDrawMessage(connection, message);
  }

  private async handleDrawMessage(
    _connection: WebSocketConnection,
    message: ClientMessage,
  ): Promise<void> {
    const { roomId, playerId, data } = message;

    // Verify player is current drawer
    const gameState = await this.gameStateService.getGameState(roomId);
    if (!gameState || gameState.currentDrawer !== playerId) {
      this.connectionPool.sendError(playerId, "Not your turn to draw");
      return;
    }

    // Validate drawing command
    if (!this.validator.validateDrawingCommand(data)) {
      this.connectionPool.sendError(playerId, "Invalid drawing command");
      return;
    }

    // Add timestamp to drawing command
    const drawingCommand = {
      ...data,
      timestamp: Date.now(),
    };

    // Update game state with drawing data
    if (gameState.drawingData) {
      gameState.drawingData.push(drawingCommand);
    } else {
      gameState.drawingData = [drawingCommand];
    }

    await this.gameStateService.updateGameState(roomId, gameState);

    // Broadcast drawing update to all players except sender
    await this.connectionPool.broadcastToRoom(
      roomId,
      {
        type: "draw-update",
        roomId,
        data: drawingCommand,
      },
      playerId,
    );
  }
}
