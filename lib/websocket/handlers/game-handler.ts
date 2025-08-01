// Game state management handler
import type { ClientMessage } from "../../../types/game.ts";
import type { WebSocketConnection, MessageHandler } from "../types/websocket-internal.ts";
import { ConnectionPool } from "../core/connection-pool.ts";
import { PlayerService } from "../services/player-service.ts";
import { GameStateService } from "../services/game-state-service.ts";
import { TimerService } from "../services/timer-service.ts";
import { WordGenerator } from "../utils/word-generator.ts";

export class GameHandler implements MessageHandler {
  private connectionPool: ConnectionPool;
  private playerService: PlayerService;
  private gameStateService: GameStateService;
  private timerService: TimerService;
  private wordGenerator: WordGenerator;

  constructor(
    connectionPool: ConnectionPool,
    playerService: PlayerService,
    gameStateService: GameStateService,
    timerService: TimerService,
    wordGenerator: WordGenerator,
  ) {
    this.connectionPool = connectionPool;
    this.playerService = playerService;
    this.gameStateService = gameStateService;
    this.timerService = timerService;
    this.wordGenerator = wordGenerator;
  }

  async handle(connection: WebSocketConnection, message: ClientMessage): Promise<void> {
    switch (message.type) {
      case "start-game":
        await this.handleStartGame(connection, message);
        break;
      case "next-round":
        await this.handleNextRound(connection, message);
        break;
      case "end-game":
        await this.handleEndGame(connection, message);
        break;
      case "ping":
        await this.handlePing(connection, message);
        break;
      default:
        throw new Error(`Unsupported message type: ${message.type}`);
    }
  }

  private async handleStartGame(_connection: WebSocketConnection, message: ClientMessage): Promise<void> {
    const { roomId, playerId } = message;

    try {
      // Verify player is host using database
      const isHost = await this.playerService.verifyPlayerIsHost(playerId, roomId);
      if (!isHost) {
        this.connectionPool.sendError(playerId, "Only host can start game");
        return;
      }

      // Get players from database
      const players = await this.playerService.getPlayersFromDatabase(roomId);
      if (!players || players.length < 2) {
        this.connectionPool.sendError(playerId, "Need at least 2 players to start game");
        return;
      }

      // Check if game is already started
      let gameState = await this.gameStateService.getGameState(roomId);
      if (gameState && gameState.phase !== "waiting" && gameState.phase !== "finished") {
        this.connectionPool.sendError(playerId, "Game is already in progress");
        return;
      }

      // Initialize new game state
      const firstDrawer = players[0].id;
      gameState = {
        roomId,
        phase: "drawing",
        roundNumber: 1,
        currentDrawer: firstDrawer,
        currentWord: this.wordGenerator.getRandomWord(),
        timeRemaining: 90000, // 90 seconds
        players: players.map((p) => ({
          id: p.id,
          name: p.name,
          isHost: p.isHost,
          isConnected: true,
          lastActivity: Date.now(),
        })),
        scores: players.reduce((acc, p) => {
          acc[p.id] = 0;
          return acc;
        }, {} as Record<string, number>),
        drawingData: [],
        correctGuesses: [],
        chatMessages: [],
      };

      // Save game state
      await this.gameStateService.updateGameState(roomId, gameState);

      // Start round timer
      this.timerService.startRoundTimer(roomId, async (roomId: string) => {
        await this.handleRoundTimer(roomId);
      });

      console.log(`Game started in room ${roomId} by host ${playerId}`);

      // Broadcast game state to all players
      await this.connectionPool.broadcastToRoom(roomId, {
        type: "game-state",
        roomId,
        data: {
          type: "game-started",
          gameState: gameState,
          currentDrawer: gameState.currentDrawer,
          roundNumber: gameState.roundNumber,
          timeRemaining: gameState.timeRemaining,
        },
      });

      // Send word to drawer only
      this.connectionPool.sendMessage(gameState.currentDrawer, {
        type: "game-state",
        roomId,
        data: {
          type: "drawing-word",
          word: gameState.currentWord,
        },
      });

      // Send success response to host
      this.connectionPool.sendMessage(playerId, {
        type: "game-state",
        roomId,
        data: {
          type: "game-start-success",
          message: "Game started successfully",
          gameState: gameState,
        },
      });
    } catch (error) {
      console.error("Error starting game:", error);
      this.connectionPool.sendError(playerId, "Failed to start game");
    }
  }

  private async handleNextRound(_connection: WebSocketConnection, message: ClientMessage): Promise<void> {
    const { roomId, playerId } = message;

    // Verify player is host
    const playerState = await this.gameStateService.getPlayerState(playerId);
    if (!playerState || !playerState.isHost) {
      this.connectionPool.sendError(playerId, "Only host can start next round");
      return;
    }

    try {
      const gameState = await this.gameStateService.getGameState(roomId);
      if (!gameState) {
        this.connectionPool.sendError(playerId, "Game not found");
        return;
      }

      // Check if we can start next round
      if (gameState.phase !== "results" && gameState.phase !== "waiting") {
        this.connectionPool.sendError(playerId, "Cannot start next round during current phase");
        return;
      }

      // Start new round
      gameState.roundNumber += 1;
      gameState.phase = "drawing";
      gameState.timeRemaining = 90000;
      gameState.drawingData = [];
      gameState.correctGuesses = [];

      // Get next drawer
      const activePlayers = gameState.players.filter((p: any) => p.isConnected);
      const currentDrawerIndex = activePlayers.findIndex((p: any) => p.id === gameState.currentDrawer);
      const nextDrawerIndex = (currentDrawerIndex + 1) % activePlayers.length;
      gameState.currentDrawer = activePlayers[nextDrawerIndex].id;
      gameState.currentWord = this.wordGenerator.getRandomWord();

      await this.gameStateService.updateGameState(roomId, gameState);

      // Clear existing timer and start new one
      this.timerService.clearRoundTimer(roomId);
      this.timerService.startRoundTimer(roomId, async (roomId: string) => {
        await this.handleRoundTimer(roomId);
      });

      // Broadcast new round state
      await this.connectionPool.broadcastToRoom(roomId, {
        type: "game-state",
        roomId,
        data: {
          type: "round-started",
          gameState: gameState,
          currentDrawer: gameState.currentDrawer,
          roundNumber: gameState.roundNumber,
          timeRemaining: gameState.timeRemaining,
        },
      });

      // Send word to new drawer
      this.connectionPool.sendMessage(gameState.currentDrawer, {
        type: "game-state",
        roomId,
        data: {
          type: "drawing-word",
          word: gameState.currentWord,
        },
      });
    } catch (error) {
      console.error("Error starting next round:", error);
      this.connectionPool.sendError(playerId, "Failed to start next round");
    }
  }

  private async handleEndGame(_connection: WebSocketConnection, message: ClientMessage): Promise<void> {
    const { roomId, playerId } = message;

    // Verify player is host
    const playerState = await this.gameStateService.getPlayerState(playerId);
    if (!playerState || !playerState.isHost) {
      this.connectionPool.sendError(playerId, "Only host can end game");
      return;
    }

    try {
      const gameState = await this.gameStateService.getGameState(roomId);
      if (!gameState) {
        this.connectionPool.sendError(playerId, "Game not found");
        return;
      }

      // End the game
      gameState.phase = "finished";
      await this.gameStateService.updateGameState(roomId, gameState);

      // Clear round timer
      this.timerService.clearRoundTimer(roomId);

      // Broadcast final results
      await this.connectionPool.broadcastToRoom(roomId, {
        type: "game-state",
        roomId,
        data: {
          type: "game-ended",
          gameState: gameState,
          finalScores: gameState.scores,
        },
      });
    } catch (error) {
      console.error("Error ending game:", error);
      this.connectionPool.sendError(playerId, "Failed to end game");
    }
  }

  private handlePing(_connection: WebSocketConnection, message: ClientMessage): void {
    const { roomId, playerId, data } = message;

    // Send pong response
    this.connectionPool.sendMessage(playerId, {
      type: "game-state",
      roomId,
      data: {
        type: "pong",
        playerId,
        timestamp: Date.now(),
        originalTimestamp: data?.timestamp,
      },
    });
  }

  private async handleRoundTimer(roomId: string): Promise<void> {
    try {
      const gameState = await this.gameStateService.getGameState(roomId);
      if (!gameState || gameState.phase !== "drawing") {
        this.timerService.clearRoundTimer(roomId);
        return;
      }

      // Update time remaining
      const newTimeRemaining = Math.max(0, gameState.timeRemaining - 1000);
      gameState.timeRemaining = newTimeRemaining;

      await this.gameStateService.updateGameState(roomId, gameState);

      // Broadcast time update
      await this.connectionPool.broadcastToRoom(roomId, {
        type: "game-state",
        roomId,
        data: {
          type: "time-update",
          timeRemaining: newTimeRemaining,
        },
      });

      // End round if time is up
      if (newTimeRemaining <= 0) {
        await this.handleRoundTimeout(roomId);
      }
    } catch (error) {
      console.error("Error in round timer:", error);
      this.timerService.clearRoundTimer(roomId);
    }
  }

  private async handleRoundTimeout(roomId: string): Promise<void> {
    try {
      this.timerService.clearRoundTimer(roomId);

      const gameState = await this.gameStateService.getGameState(roomId);
      if (!gameState) return;

      gameState.phase = "results";
      await this.gameStateService.updateGameState(roomId, gameState);

      // Broadcast round ended
      await this.connectionPool.broadcastToRoom(roomId, {
        type: "game-state",
        roomId,
        data: {
          type: "round-ended",
          reason: "timeout",
          gameState: gameState,
          scores: gameState.scores,
        },
      });

      // If game is complete, broadcast final results
      if (gameState.phase === "results" && gameState.roundNumber >= 5) {
        await this.connectionPool.broadcastToRoom(roomId, {
          type: "game-state",
          roomId,
          data: {
            type: "game-completed",
            finalScores: gameState.scores,
          },
        });
      }
    } catch (error) {
      console.error("Error handling round timeout:", error);
    }
  }
}