// Room join/leave operations handler
import type { ClientMessage } from "../../../types/game.ts";
import type { WebSocketConnection, MessageHandler, WebSocketService } from "../types/websocket-internal.ts";
import { ConnectionPool } from "../core/connection-pool.ts";
import { MessageValidator } from "../utils/message-validator.ts";
import { PlayerService } from "../services/player-service.ts";
import { GameStateService } from "../services/game-state-service.ts";

export class RoomHandler implements MessageHandler {
  private connectionPool: ConnectionPool;
  private validator: MessageValidator;
  private playerService: PlayerService;
  private gameStateService: GameStateService;

  constructor(
    connectionPool: ConnectionPool,
    validator: MessageValidator,
    playerService: PlayerService,
    gameStateService: GameStateService,
  ) {
    this.connectionPool = connectionPool;
    this.validator = validator;
    this.playerService = playerService;
    this.gameStateService = gameStateService;
  }

  async handle(connection: WebSocketConnection, message: ClientMessage): Promise<void> {
    switch (message.type) {
      case "join-room":
        await this.handleJoinRoom(connection, message);
        break;
      case "leave-room":
        await this.handleLeaveRoom(connection, message);
        break;
      default:
        throw new Error(`Unsupported message type: ${message.type}`);
    }
  }

  private async handleJoinRoom(connection: WebSocketConnection, message: ClientMessage): Promise<void> {
    const { roomId, playerId, data } = message;
    const { playerName } = data;

    // Check if player is already in this room to prevent duplicate processing
    if (this.connectionPool.getPlayerRoom(playerId) === roomId && this.connectionPool.getConnection(playerId)) {
      return;
    }

    // Validate player name
    if (!this.validator.validatePlayerName(playerName)) {
      this.connectionPool.sendError(playerId, "Invalid player name");
      return;
    }

    // Check if room exists and has capacity
    const roomExists = await this.playerService.checkRoomExists(roomId);
    if (!roomExists) {
      this.connectionPool.sendError(playerId, "Room not found");
      return;
    }

    const roomCapacity = await this.playerService.checkRoomCapacity(roomId);
    if (!roomCapacity) {
      this.connectionPool.sendError(playerId, "Room is full");
      return;
    }

    // Add connection to room
    this.connectionPool.addConnection(playerId, roomId, connection.ws);

    // Update player state in KV
    try {
      await this.gameStateService.updatePlayerState(playerId, {
        id: playerId,
        name: playerName,
        isHost: false, // Will be determined by room logic
        isConnected: true,
        lastActivity: Date.now(),
      });
    } catch (error) {
      console.error("Error updating player state:", error);
    }

    // Broadcast room update to all players in room
    try {
      await this.connectionPool.broadcastToRoom(roomId, {
        type: "room-update",
        roomId,
        data: {
          type: "player-joined",
          playerId,
          playerName,
        },
      });
    } catch (error) {
      console.error("Error broadcasting room update:", error);
    }

    // Update and send current game state
    try {
      let gameState = await this.gameStateService.getGameState(roomId);

      // Small delay to ensure database operations are completed
      await new Promise((resolve) => setTimeout(resolve, 50));

      // Get all current players in the room (including the new player)
      const allPlayers = await this.playerService.getPlayersFromDatabase(roomId);

      // If no game state exists, create initial waiting state
      if (!gameState) {
        gameState = {
          roomId,
          phase: "waiting",
          roundNumber: 0,
          currentDrawer: "",
          currentWord: "",
          timeRemaining: 0,
          players: allPlayers?.map((p) => ({
            id: p.id,
            name: p.name,
            isHost: p.isHost,
            isConnected: true,
            lastActivity: Date.now(),
          })) || [],
          scores: allPlayers?.reduce((acc, p) => {
            acc[p.id] = 0;
            return acc;
          }, {} as Record<string, number>) || {},
          drawingData: [],
          correctGuesses: [],
          chatMessages: [],
        };
      } else if (allPlayers) {
        // Game state exists, update it with current players
        gameState.players = allPlayers.map((p) => ({
          id: p.id,
          name: p.name,
          isHost: p.isHost,
          isConnected: true,
          lastActivity: Date.now(),
        }));

        // Update scores to include new players (preserve existing scores)
        const newScores: Record<string, number> = {};
        allPlayers.forEach((p) => {
          newScores[p.id] = gameState!.scores[p.id] || 0;
        });
        gameState.scores = newScores;
      }

      // Save the updated game state
      await this.gameStateService.updateGameState(roomId, gameState);

      // Broadcast updated game state to ALL players in the room
      await this.connectionPool.broadcastToRoom(roomId, {
        type: "game-state",
        roomId,
        data: gameState,
      });

      // Also broadcast a room update to ensure UI updates
      await this.connectionPool.broadcastToRoom(roomId, {
        type: "room-update",
        roomId,
        data: {
          type: "player-joined",
          playerId,
          playerName,
          gameState: gameState,
        },
      });
    } catch (error) {
      console.error("Error getting/sending game state:", error);
    }
  }

  private async handleLeaveRoom(connection: WebSocketConnection, message: ClientMessage): Promise<void> {
    const { roomId, playerId } = message;
    await this.removePlayerFromRoom(playerId, roomId);
  }

  private async removePlayerFromRoom(playerId: string, roomId: string): Promise<void> {
    // Remove from connection pool
    this.connectionPool.removeConnection(playerId);

    // Update player state
    const playerState = await this.gameStateService.getPlayerState(playerId);
    if (playerState) {
      playerState.isConnected = false;
      await this.gameStateService.updatePlayerState(playerId, playerState);
    }

    // Broadcast player left to room
    await this.connectionPool.broadcastToRoom(roomId, {
      type: "room-update",
      roomId,
      data: {
        type: "player-left",
        playerId,
      },
    });
  }
}