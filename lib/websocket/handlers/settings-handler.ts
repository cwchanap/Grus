// Game settings handler
import type { BaseClientMessage } from "../../../types/core/websocket.ts";
import type { MessageHandler, WebSocketConnection } from "../types/websocket-internal.ts";
import { ConnectionPool } from "../core/connection-pool.ts";
import { MessageValidator } from "../utils/message-validator.ts";
import type { GameStateStorage } from "../types/websocket-internal.ts";

export class SettingsHandler implements MessageHandler {
  private connectionPool: ConnectionPool;
  private validator: MessageValidator;
  private gameStateService: GameStateStorage;

  constructor(
    connectionPool: ConnectionPool,
    validator: MessageValidator,
    gameStateService: GameStateStorage,
  ) {
    this.connectionPool = connectionPool;
    this.validator = validator;
    this.gameStateService = gameStateService;
  }

  async handle(connection: WebSocketConnection, message: BaseClientMessage): Promise<void> {
    if (message.type !== "update-settings") {
      throw new Error(`Unsupported message type: ${message.type}`);
    }

    await this.handleUpdateSettings(connection, message);
  }

  private async handleUpdateSettings(
    _connection: WebSocketConnection,
    message: BaseClientMessage,
  ): Promise<void> {
    const { roomId, playerId, data: settings } = message;

    // Validate message
    if (!this.validator.validateRoomId(roomId)) {
      throw new Error("Invalid room ID");
    }

    if (!this.validator.validatePlayerId(playerId)) {
      throw new Error("Invalid player ID");
    }

    // Validate settings data
    if (!settings || typeof settings !== "object") {
      throw new Error("Invalid settings data");
    }

    if (
      typeof settings.maxRounds !== "number" || settings.maxRounds < 1 || settings.maxRounds > 10
    ) {
      throw new Error("Invalid maxRounds: must be between 1 and 10");
    }

    if (
      typeof settings.roundTimeSeconds !== "number" || settings.roundTimeSeconds < 60 ||
      settings.roundTimeSeconds > 90
    ) {
      throw new Error("Invalid roundTimeSeconds: must be between 60 and 90 seconds");
    }

    // Get current game state
    const gameState = await this.gameStateService.getGameState(roomId);
    if (!gameState) {
      throw new Error("Game state not found");
    }

    // Check if player is the host
    const player = gameState.players.find((p: any) => p.id === playerId);
    if (!player || !player.isHost) {
      throw new Error("Only the host can update game settings");
    }

    // Check if game is in waiting phase (settings can only be changed before game starts)
    if (gameState.phase !== "waiting") {
      throw new Error("Settings can only be changed before the game starts");
    }

    // Update game state with new settings
    const updatedGameState = {
      ...gameState,
      settings: {
        maxRounds: settings.maxRounds,
        roundTimeSeconds: settings.roundTimeSeconds,
      },
      timeRemaining: settings.roundTimeSeconds * 1000, // Update default time remaining
    };

    // Save updated game state
    await this.gameStateService.updateGameState(roomId, updatedGameState);

    console.log(`Settings updated for room ${roomId} by host ${playerId}:`, settings);

    // Broadcast settings update to all players in the room
    await this.connectionPool.broadcastToRoom(roomId, {
      type: "game-state",
      roomId,
      data: updatedGameState,
    });

    // Also send a specific settings update message
    await this.connectionPool.broadcastToRoom(roomId, {
      type: "settings-updated",
      roomId,
      data: {
        settings: updatedGameState.settings,
        updatedBy: player.name,
      },
    });
  }
}
