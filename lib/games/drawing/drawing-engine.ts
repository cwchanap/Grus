// Drawing game engine implementation
import { BaseGameEngine } from "../../core/game-engine.ts";
import {
  DrawingClientMessage,
  DrawingCommand,
  DrawingGameData,
  DrawingGameSettings,
  DrawingGameState,
  DrawingServerMessage,
} from "../../../types/games/drawing.ts";
import { PlayerState } from "../../../types/core/room.ts";
import { validateDrawingCommand } from "./drawing-utils.ts";
import { getConfig } from "../../config.ts";

/**
 * Server-side drawing command buffer for batching broadcasts
 */
class ServerDrawingCommandBuffer {
  private buffer: DrawingCommand[] = [];
  private roomId: string;
  private flushCallback: (commands: DrawingCommand[]) => void;
  private timeoutId: number | null = null;
  private config = getConfig();

  constructor(roomId: string, flushCallback: (commands: DrawingCommand[]) => void) {
    this.roomId = roomId;
    this.flushCallback = flushCallback;
  }

  add(command: DrawingCommand): void {
    this.buffer.push(command);
    console.log(
      "ServerDrawingCommandBuffer: Added command",
      command.type,
      "buffer size:",
      this.buffer.length,
    );

    // Flush immediately for critical commands or when buffer is full
    if (
      command.type === "start" ||
      command.type === "end" ||
      command.type === "clear" ||
      this.buffer.length >= this.config.drawing.maxBatchSize
    ) {
      console.log("ServerDrawingCommandBuffer: Immediate flush triggered");
      this.flush();
    } else if (this.timeoutId === null) {
      // Start debounce timer for non-critical commands
      console.log(
        "ServerDrawingCommandBuffer: Starting timeout for",
        this.config.drawing.serverDebounceMs,
        "ms",
      );
      this.timeoutId = setTimeout(() => {
        console.log("ServerDrawingCommandBuffer: Timeout triggered, flushing");
        this.flush();
      }, this.config.drawing.serverDebounceMs);
    }
  }

  flush(): void {
    if (this.buffer.length === 0) return;

    const commands = [...this.buffer];
    this.buffer = [];

    if (this.timeoutId !== null) {
      clearTimeout(this.timeoutId);
      this.timeoutId = null;
    }

    this.flushCallback(commands);
  }

  destroy(): void {
    if (this.timeoutId !== null) {
      clearTimeout(this.timeoutId);
      this.timeoutId = null;
    }
    this.flush(); // Flush any remaining commands
  }
}

export class DrawingGameEngine extends BaseGameEngine<
  DrawingGameState,
  DrawingGameSettings,
  DrawingClientMessage,
  DrawingServerMessage
> {
  private serverBuffer: ServerDrawingCommandBuffer | null = null;
  private pendingServerMessages: DrawingServerMessage[] = [];

  getGameType(): string {
    return "drawing";
  }

  initializeGame(
    roomId: string,
    players: PlayerState[],
    settings: DrawingGameSettings,
  ): DrawingGameState {
    const gameData: DrawingGameData = {
      currentDrawer: "",
      currentWord: "",
      drawingData: [],
      correctGuesses: [],
    };

    return {
      roomId,
      gameType: "drawing",
      roundNumber: 0,
      timeRemaining: 0,
      phase: "waiting",
      players,
      scores: players.reduce((acc, player) => ({ ...acc, [player.id]: 0 }), {}),
      gameData,
      chatMessages: [],
      settings,
    };
  }

  override startGame(gameState: DrawingGameState): DrawingGameState {
    const updatedState = super.startGame(gameState);

    // Select first drawer
    const firstDrawer = gameState.players[0];
    if (firstDrawer) {
      updatedState.gameData = {
        ...updatedState.gameData,
        currentDrawer: firstDrawer.id,
        currentWord: this.selectRandomWord(),
        drawingData: [],
        correctGuesses: [],
      };
      updatedState.phase = "playing";
      updatedState.drawingPhase = "drawing";
    }

    return updatedState;
  }

  handleClientMessage(gameState: DrawingGameState, message: DrawingClientMessage): {
    updatedState: DrawingGameState;
    serverMessages: DrawingServerMessage[];
  } {
    const serverMessages: DrawingServerMessage[] = [];
    let updatedState = { ...gameState };

    // Initialize server buffer if not exists
    if (!this.serverBuffer) {
      this.serverBuffer = new ServerDrawingCommandBuffer(
        gameState.roomId,
        (commands: DrawingCommand[]) => {
          // Create batched draw-update message
          const batchedMessage: DrawingServerMessage = {
            type: "draw-update-batch",
            roomId: gameState.roomId,
            data: { commands },
          };
          console.log("ServerDrawingCommandBuffer: Flushing", commands.length, "commands");
          this.pendingServerMessages.push(batchedMessage);
        },
      );
    }

    switch (message.type) {
      case "draw":
        (Deno.env.get("DENO_ENV") !== "production") &&
          console.log("Received draw message:", message.data);
        console.log("DRAW DEBUG: serverBuffer exists?", !!this.serverBuffer);
        if (this.validateDrawingAction(gameState, message.playerId, message.data)) {
          console.log("DRAW DEBUG: Validation passed, about to add to buffer");
          const drawCommand = message.data as DrawingCommand;
          updatedState.gameData = {
            ...updatedState.gameData,
            drawingData: [...updatedState.gameData.drawingData, drawCommand],
          };

          // Add to server buffer for batched broadcasting
          console.log("DRAW DEBUG: Calling serverBuffer.add with command:", drawCommand);
          this.serverBuffer.add(drawCommand);
          console.log("DRAW DEBUG: serverBuffer.add completed");
        } else {
          console.log("DRAW DEBUG: Validation failed for drawing action");
        }
        break;

      case "guess":
        if (gameState.phase === "playing") {
          const guess = message.data.message?.toLowerCase().trim();
          const currentWord = updatedState.gameData.currentWord.toLowerCase();
          const isCorrect = guess === currentWord;

          if (
            isCorrect &&
            !updatedState.gameData.correctGuesses.find((g) => g.playerId === message.playerId)
          ) {
            // Add to correct guesses
            updatedState.gameData.correctGuesses.push({
              playerId: message.playerId,
              timestamp: Date.now(),
            });

            // Calculate and award points
            const points = this.calculateScore(updatedState, message.playerId, {
              type: "correct_guess",
            });
            updatedState.scores[message.playerId] = (updatedState.scores[message.playerId] || 0) +
              points;

            serverMessages.push({
              type: "score-update",
              roomId: gameState.roomId,
              data: {
                playerId: message.playerId,
                points,
                totalScore: updatedState.scores[message.playerId],
              },
            });
          }

          // Add chat message (filtered if it contains the answer)
          const chatMessage = {
            id: `msg_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
            playerId: message.playerId,
            playerName: gameState.players.find((p) => p.id === message.playerId)?.name || "Unknown",
            message: isCorrect ? "*** guessed correctly! ***" : message.data.message,
            timestamp: Date.now(),
            isGuess: true,
            isCorrect,
          };

          updatedState.chatMessages.push(chatMessage);

          serverMessages.push({
            type: "chat-message",
            roomId: gameState.roomId,
            data: chatMessage,
          });
        }
        break;

      case "next-round":
        if (
          message.playerId === gameState.gameData.currentDrawer ||
          gameState.players.find((p) => p.id === message.playerId)?.isHost
        ) {
          updatedState = this.nextRound(updatedState);

          serverMessages.push({
            type: "game-state",
            roomId: gameState.roomId,
            data: updatedState,
          });
        }
        break;

      case "chat": {
        // Handle regular chat messages
        const chatMessage = {
          id: `msg_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
          playerId: message.playerId,
          playerName: gameState.players.find((p) => p.id === message.playerId)?.name || "Unknown",
          message: message.data.message,
          timestamp: Date.now(),
          isSystemMessage: false,
        };

        updatedState.chatMessages.push(chatMessage);

        serverMessages.push({
          type: "chat-message",
          roomId: gameState.roomId,
          data: chatMessage,
        });
        break;
      }
    }

    // Collect any pending batched messages from the server buffer
    const batchedMessages = [...this.pendingServerMessages];
    this.pendingServerMessages = [];
    console.log(
      "handleClientMessage: Returning",
      serverMessages.length,
      "immediate messages and",
      batchedMessages.length,
      "batched messages",
    );

    return { updatedState, serverMessages: [...serverMessages, ...batchedMessages] };
  }

  validateGameAction(gameState: DrawingGameState, playerId: string, action: any): boolean {
    switch (action.type) {
      case "draw":
        return this.validateDrawingAction(gameState, playerId, action.data);
      case "guess":
        return gameState.phase === "playing";
      default:
        return false;
    }
  }

  private validateDrawingAction(
    gameState: DrawingGameState,
    playerId: string,
    drawCommand: any,
  ): boolean {
    console.log("VALIDATION DEBUG: playerId =", playerId);
    console.log("VALIDATION DEBUG: currentDrawer =", gameState.gameData.currentDrawer);
    console.log("VALIDATION DEBUG: phase =", gameState.phase);

    // Only the current drawer can draw
    if (playerId !== gameState.gameData.currentDrawer) {
      console.log("VALIDATION DEBUG: Player is not the current drawer");
      return false;
    }

    // Game must be in playing phase
    if (gameState.phase !== "playing") {
      console.log("VALIDATION DEBUG: Game is not in playing phase");
      return false;
    }

    // Validate the drawing command structure
    const isValid = validateDrawingCommand(drawCommand);
    console.log("VALIDATION DEBUG: Command structure valid =", isValid);
    return isValid;
  }

  calculateScore(gameState: DrawingGameState, _playerId: string, action: any): number {
    if (action.type === "correct_guess") {
      // Score based on how quickly the guess was made
      // Use milliseconds consistently
      const totalTimeMs = gameState.settings.roundTimeSeconds * 1000;
      const timeElapsedMs = totalTimeMs - gameState.timeRemaining;
      return Math.max(0, 100 - Math.floor(timeElapsedMs / totalTimeMs * 100));
    }

    return 0;
  }

  override endGame(gameState: DrawingGameState): DrawingGameState {
    // Clean up server buffer
    if (this.serverBuffer) {
      this.serverBuffer.destroy();
      this.serverBuffer = null;
    }
    this.pendingServerMessages = [];

    return super.endGame(gameState);
  }

  private nextRound(gameState: DrawingGameState): DrawingGameState {
    const currentDrawerIndex = gameState.players.findIndex((p) =>
      p.id === gameState.gameData.currentDrawer
    );
    const nextDrawerIndex = (currentDrawerIndex + 1) % gameState.players.length;
    const nextDrawer = gameState.players[nextDrawerIndex];

    const isGameFinished = gameState.roundNumber >= gameState.settings.maxRounds;

    if (isGameFinished) {
      return this.endGame(gameState);
    }

    return {
      ...gameState,
      roundNumber: gameState.roundNumber + 1,
      // Reset timer in milliseconds for the new round
      timeRemaining: gameState.settings.roundTimeSeconds * 1000,
      phase: "playing",
      drawingPhase: "drawing",
      gameData: {
        ...gameState.gameData,
        currentDrawer: nextDrawer.id,
        currentWord: this.selectRandomWord(),
        drawingData: [],
        correctGuesses: [],
      },
    };
  }

  private selectRandomWord(): string {
    // Simple word list - in a real implementation, this would be more sophisticated
    const words = [
      "cat",
      "dog",
      "house",
      "tree",
      "car",
      "sun",
      "moon",
      "star",
      "flower",
      "bird",
      "fish",
      "book",
      "chair",
      "table",
      "computer",
      "phone",
      "apple",
      "banana",
      "pizza",
      "cake",
    ];

    return words[Math.floor(Math.random() * words.length)];
  }
}
