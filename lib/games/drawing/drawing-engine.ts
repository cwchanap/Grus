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

export class DrawingGameEngine extends BaseGameEngine<
  DrawingGameState,
  DrawingGameSettings,
  DrawingClientMessage,
  DrawingServerMessage
> {
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

    switch (message.type) {
      case "draw":
        if (this.validateDrawingAction(gameState, message.playerId, message.data)) {
          const drawCommand = message.data as DrawingCommand;
          updatedState.gameData = {
            ...updatedState.gameData,
            drawingData: [...updatedState.gameData.drawingData, drawCommand],
          };

          serverMessages.push({
            type: "draw-update",
            roomId: gameState.roomId,
            data: drawCommand,
          });
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

      case "chat":
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

    return { updatedState, serverMessages };
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
    // Only the current drawer can draw
    if (playerId !== gameState.gameData.currentDrawer) {
      return false;
    }

    // Game must be in playing phase
    if (gameState.phase !== "playing") {
      return false;
    }

    // Validate the drawing command structure
    return validateDrawingCommand(drawCommand);
  }

  calculateScore(gameState: DrawingGameState, playerId: string, action: any): number {
    if (action.type === "correct_guess") {
      // Score based on how quickly the guess was made
      const timeElapsed = gameState.settings.roundTimeSeconds - gameState.timeRemaining;
      const totalTime = gameState.settings.roundTimeSeconds;
      return Math.max(0, 100 - Math.floor(timeElapsed / totalTime * 100));
    }

    return 0;
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
      timeRemaining: gameState.settings.roundTimeSeconds,
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
