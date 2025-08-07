// Chat message handling
import type { BaseClientMessage } from "../../../types/core/websocket.ts";
import type { MessageHandler, WebSocketConnection } from "../types/websocket-internal.ts";
import { ConnectionPool } from "../core/connection-pool.ts";
import { MessageValidator } from "../utils/message-validator.ts";
import { GameStateService } from "../services/game-state-service.ts";

export class ChatHandler implements MessageHandler {
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

  async handle(connection: WebSocketConnection, message: BaseClientMessage): Promise<void> {
    if (message.type !== "chat" && message.type !== "guess") {
      throw new Error(`Unsupported message type: ${message.type}`);
    }

    await this.handleChatMessage(connection, message);
  }

  private async handleChatMessage(
    _connection: WebSocketConnection,
    message: BaseClientMessage,
  ): Promise<void> {
    const { roomId, playerId, data } = message;
    const { text } = data;

    // Chat messages are always allowed regardless of game state
    if (!this.validator.validateChatMessage(text)) {
      this.connectionPool.sendError(playerId, "Invalid chat message");
      return;
    }

    // Get player info
    const playerState = await this.gameStateService.getPlayerState(playerId);
    if (!playerState) {
      this.connectionPool.sendError(playerId, "Player not found");
      return;
    }

    // Get current game state
    const gameState = await this.gameStateService.getGameState(roomId);
    if (!gameState) {
      this.connectionPool.sendError(playerId, "Game not found");
      return;
    }

    // Create chat message
    const chatMessage = {
      id: crypto.randomUUID(),
      playerId,
      playerName: playerState.name,
      message: text,
      timestamp: Date.now(),
      isGuess: gameState.phase === "drawing", // Only guesses during drawing phase
      isCorrect: false,
    };

    // Check if it's a correct guess (only during drawing phase)
    if (
      gameState.phase === "drawing" &&
      gameState.currentWord &&
      playerId !== gameState.currentDrawer &&
      text.toLowerCase().trim() === gameState.currentWord.toLowerCase()
    ) {
      await this.handleCorrectGuess(roomId, playerId, playerState.name, gameState, chatMessage);
      return;
    }

    // Always broadcast chat message to room (regardless of game phase)
    await this.connectionPool.broadcastToRoom(roomId, {
      type: "chat-message",
      roomId,
      data: chatMessage,
    });
  }

  private async handleCorrectGuess(
    roomId: string,
    playerId: string,
    playerName: string,
    gameState: any,
    chatMessage: any,
  ): Promise<void> {
    chatMessage.isCorrect = true;

    try {
      // Update game state for correct guess
      gameState.phase = "results";
      if (!gameState.correctGuesses) {
        gameState.correctGuesses = [];
      }
      gameState.correctGuesses.push({
        playerId,
        timestamp: Date.now(),
      });

      // Update scores
      const basePoints = 100;
      const timeBonus = Math.floor((gameState.timeRemaining / 1000) * 2);
      const totalPoints = basePoints + timeBonus;

      if (!gameState.scores[playerId]) {
        gameState.scores[playerId] = 0;
      }
      gameState.scores[playerId] += totalPoints;

      // Award points to drawer too
      if (gameState.currentDrawer && !gameState.scores[gameState.currentDrawer]) {
        gameState.scores[gameState.currentDrawer] = 0;
      }
      if (gameState.currentDrawer) {
        gameState.scores[gameState.currentDrawer] += 50; // Drawer bonus
      }

      await this.gameStateService.updateGameState(roomId, gameState);

      // Broadcast correct guess and score update
      await this.connectionPool.broadcastToRoom(roomId, {
        type: "game-state",
        roomId,
        data: {
          type: "correct-guess",
          playerId,
          playerName,
          word: gameState.currentWord,
          scores: gameState.scores,
          gameState: gameState,
        },
      });

      // Broadcast chat message with correct flag
      await this.connectionPool.broadcastToRoom(roomId, {
        type: "chat-message",
        roomId,
        data: chatMessage,
      });

      // Auto-advance to results phase after a short delay
      setTimeout(async () => {
        try {
          const currentGameState = await this.gameStateService.getGameState(roomId);
          if (!currentGameState) return;

          currentGameState.phase = "results";
          await this.gameStateService.updateGameState(roomId, currentGameState);

          await this.connectionPool.broadcastToRoom(roomId, {
            type: "game-state",
            roomId,
            data: {
              type: "round-ended",
              reason: "correct-guess",
              gameState: currentGameState,
              scores: currentGameState.scores,
            },
          });

          // Check if game is complete
          if (currentGameState.roundNumber >= 5) {
            // Assuming 5 rounds max
            await this.connectionPool.broadcastToRoom(roomId, {
              type: "game-state",
              roomId,
              data: {
                type: "game-completed",
                finalScores: currentGameState.scores,
              },
            });
          }
        } catch (error) {
          console.error("Error ending round after correct guess:", error);
        }
      }, 2000); // 2 second delay to show the correct answer
    } catch (error) {
      console.error("Error processing correct guess:", error);
      // Continue to send as regular chat message
      await this.connectionPool.broadcastToRoom(roomId, {
        type: "chat-message",
        roomId,
        data: chatMessage,
      });
    }
  }
}
