// Core game engine abstraction
import { BaseGameSettings, BaseGameState } from "../../types/core/game.ts";
import { BaseClientMessage, BaseServerMessage } from "../../types/core/websocket.ts";
import { PlayerState } from "../../types/core/room.ts";

export interface GameEngine<
  TGameState extends BaseGameState = BaseGameState,
  TSettings extends BaseGameSettings = BaseGameSettings,
  TClientMessage extends BaseClientMessage = BaseClientMessage,
  TServerMessage extends BaseServerMessage = BaseServerMessage,
> {
  // Game lifecycle
  initializeGame(roomId: string, players: PlayerState[], settings: TSettings): TGameState;
  startGame(gameState: TGameState): TGameState;
  endGame(gameState: TGameState): TGameState;

  // Message handling
  handleClientMessage(gameState: TGameState, message: TClientMessage): {
    updatedState: TGameState;
    serverMessages: TServerMessage[];
  };

  // Game state management
  updateGameState(gameState: TGameState, deltaTime: number): TGameState;
  validateGameAction(gameState: TGameState, playerId: string, action: any): boolean;

  // Player management
  addPlayer(gameState: TGameState, player: PlayerState): TGameState;
  removePlayer(gameState: TGameState, playerId: string): TGameState;

  // Scoring
  calculateScore(gameState: TGameState, playerId: string, action: any): number;

  // Game type identifier
  getGameType(): string;
}

export abstract class BaseGameEngine<
  TGameState extends BaseGameState = BaseGameState,
  TSettings extends BaseGameSettings = BaseGameSettings,
  TClientMessage extends BaseClientMessage = BaseClientMessage,
  TServerMessage extends BaseServerMessage = BaseServerMessage,
> implements GameEngine<TGameState, TSettings, TClientMessage, TServerMessage> {
  abstract getGameType(): string;

  abstract initializeGame(roomId: string, players: PlayerState[], settings: TSettings): TGameState;

  startGame(gameState: TGameState): TGameState {
    return {
      ...gameState,
      phase: "playing" as any,
      roundNumber: 1,
      timeRemaining: gameState.settings.roundTimeSeconds,
    };
  }

  endGame(gameState: TGameState): TGameState {
    return {
      ...gameState,
      phase: "finished" as any,
      timeRemaining: 0,
    };
  }

  abstract handleClientMessage(gameState: TGameState, message: TClientMessage): {
    updatedState: TGameState;
    serverMessages: TServerMessage[];
  };

  updateGameState(gameState: TGameState, deltaTime: number): TGameState {
    if (gameState.phase === "playing" && gameState.timeRemaining > 0) {
      return {
        ...gameState,
        timeRemaining: Math.max(0, gameState.timeRemaining - deltaTime),
      };
    }
    return gameState;
  }

  abstract validateGameAction(gameState: TGameState, playerId: string, action: any): boolean;

  addPlayer(gameState: TGameState, player: PlayerState): TGameState {
    return {
      ...gameState,
      players: [...gameState.players, player],
    };
  }

  removePlayer(gameState: TGameState, playerId: string): TGameState {
    return {
      ...gameState,
      players: gameState.players.filter((p) => p.id !== playerId),
    };
  }

  abstract calculateScore(gameState: TGameState, playerId: string, action: any): number;
}
