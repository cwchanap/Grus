// Room join/leave operations handler
import type { ClientMessage } from "../../../types/game.ts";
import type {
  MessageHandler,
  WebSocketConnection,
  WebSocketService as _WebSocketService,
} from "../types/websocket-internal.ts";
import { ConnectionPool } from "../core/connection-pool.ts";
import { MessageValidator } from "../utils/message-validator.ts";
import { PlayerService } from "../services/player-service.ts";
import { GameStateService } from "../services/game-state-service.ts";
import { RoomManager } from "../../room-manager.ts";

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

  private async handleJoinRoom(
    connection: WebSocketConnection,
    message: ClientMessage,
  ): Promise<void> {
    const { roomId, playerId, data } = message;
    const { playerName } = data;

    console.log(
      `Processing join room request: playerId=${playerId}, roomId=${roomId}, playerName=${playerName}`,
    );

    // Check if player is already in this room to prevent duplicate processing
    const existingRoom = this.connectionPool.getPlayerRoom(playerId);
    const existingConnection = this.connectionPool.getConnection(playerId);

    if (existingRoom === roomId && existingConnection) {
      console.log(`Player ${playerId} already connected to room ${roomId}, skipping join`);

      // Still send the current game state to ensure the client is up to date
      try {
        const gameState = await this.gameStateService.getGameState(roomId);
        if (gameState) {
          this.connectionPool.sendMessage(playerId, {
            type: "game-state",
            roomId,
            data: gameState,
          });
        }
      } catch (error) {
        console.error("Error sending game state to existing connection:", error);
      }

      return;
    }

    // If player is in a different room, remove them first
    if (existingRoom && existingRoom !== roomId) {
      console.log(`Player ${playerId} switching from room ${existingRoom} to ${roomId}`);
      await this.removePlayerFromRoom(playerId, existingRoom);
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

    // Update the connection object with player and room info FIRST
    connection.playerId = playerId;
    connection.roomId = roomId;

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
      this.connectionPool.broadcastToRoom(roomId, {
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
          settings: { maxRounds: 5, roundTimeSeconds: 75 },
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
      this.connectionPool.broadcastToRoom(roomId, {
        type: "game-state",
        roomId,
        data: gameState,
      });

      // Also broadcast a room update to ensure UI updates
      this.connectionPool.broadcastToRoom(roomId, {
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

  private async handleLeaveRoom(
    connection: WebSocketConnection,
    message: ClientMessage,
  ): Promise<void> {
    const { roomId, playerId } = message;
    await this.removePlayerFromRoom(playerId, roomId);

    // Clear the connection info since the player left
    connection.playerId = "";
    connection.roomId = "";
  }

  async removePlayerFromRoom(playerId: string, roomId: string): Promise<void> {
    try {
      // Get player info before removal for broadcasting
      const playerState = await this.gameStateService.getPlayerState(playerId);
      const playerName = playerState?.name || "Unknown Player";

      // Always remove from connection pool first to prevent further messages
      this.connectionPool.removeConnection(playerId);

      // Use room manager to handle the leave logic (including host migration)
      const roomManager = new RoomManager();
      const leaveResult = await roomManager.leaveRoom(roomId, playerId);

      if (!leaveResult.success) {
        console.error(
          `Failed to remove player ${playerId} from room ${roomId}:`,
          leaveResult.error,
        );

        // Update player state to disconnected even if database removal failed
        if (playerState) {
          try {
            playerState.isConnected = false;
            await this.gameStateService.updatePlayerState(playerId, playerState);
          } catch (stateError) {
            console.error(`Failed to update player state for ${playerId}:`, stateError);
          }
        }

        // Broadcast a basic player left message even if database operation failed
        this.connectionPool.broadcastToRoom(roomId, {
          type: "room-update",
          roomId,
          data: {
            type: "player-left",
            playerId,
            playerName,
            wasHost: false,
          },
        });

        return;
      }

      // Update player state to disconnected
      if (playerState) {
        try {
          playerState.isConnected = false;
          await this.gameStateService.updatePlayerState(playerId, playerState);
        } catch (stateError) {
          console.error(`Failed to update player state for ${playerId}:`, stateError);
        }
      }

      // Safely extract data from leave result
      const leaveData = leaveResult.data;
      if (!leaveData) {
        console.error(`No data returned from leaveRoom for player ${playerId} in room ${roomId}`);
        return;
      }

      const { wasHost, newHostId, newHostName, roomDeleted, remainingPlayers } = leaveData;

      // If room was deleted, no need to broadcast
      if (roomDeleted) {
        console.log(`Room ${roomId} was deleted after player ${playerId} left`);
        return;
      }

      // Update game state with remaining players
      const gameState = await this.gameStateService.getGameState(roomId);
      if (gameState && remainingPlayers && Array.isArray(remainingPlayers)) {
        // Update players list in game state
        gameState.players = remainingPlayers.map((p) => ({
          id: p.id,
          name: p.name,
          isHost: p.isHost,
          isConnected: true,
          lastActivity: Date.now(),
        }));

        // Remove the leaving player from scores
        if (gameState.scores[playerId] !== undefined) {
          delete gameState.scores[playerId];
        }

        // If the leaving player was the current drawer, reset the game state
        if (gameState.currentDrawer === playerId) {
          gameState.currentDrawer = "";
          gameState.currentWord = "";
          gameState.phase = "waiting";
          gameState.drawingData = [];
          gameState.correctGuesses = [];
        }

        // Save updated game state
        await this.gameStateService.updateGameState(roomId, gameState);

        // Broadcast updated game state to all remaining players
        this.connectionPool.broadcastToRoom(roomId, {
          type: "game-state",
          roomId,
          data: gameState,
        });
      }

      // Prepare the room update message
      const roomUpdateData: any = {
        type: "player-left",
        playerId,
        playerName,
        wasHost,
      };

      // Add host migration info if applicable
      if (wasHost && newHostId && newHostName) {
        roomUpdateData.hostMigration = {
          newHostId,
          newHostName,
        };
      }

      // Broadcast player left update to all remaining players
      console.log(`Broadcasting player-left message to room ${roomId}:`, roomUpdateData);
      this.connectionPool.broadcastToRoom(roomId, {
        type: "room-update",
        roomId,
        data: roomUpdateData,
      });

      // If there was a host migration, send a specific host-changed message
      if (wasHost && newHostId && newHostName) {
        const hostChangedData = {
          type: "host-changed",
          oldHostId: playerId,
          oldHostName: playerName,
          newHostId,
          newHostName,
        };

        console.log(`Broadcasting host-changed message to room ${roomId}:`, hostChangedData);
        this.connectionPool.broadcastToRoom(roomId, {
          type: "room-update",
          roomId,
          data: hostChangedData,
        });

        console.log(
          `Host migration completed: ${playerName} (${playerId}) -> ${newHostName} (${newHostId}) in room ${roomId}`,
        );
      }

      console.log(
        `Player ${playerName} (${playerId}) left room ${roomId}. Remaining players: ${
          remainingPlayers?.length || 0
        }`,
      );
    } catch (error) {
      console.error(`Error removing player ${playerId} from room ${roomId}:`, error);
    }
  }
}
