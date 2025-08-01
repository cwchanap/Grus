// Game state storage and management service
import { getConfig } from "../../config.ts";
import type { Env } from "../../../types/cloudflare.ts";
import type { GameState, PlayerState } from "../../../types/game.ts";
import type { GameStateStorage } from "../types/websocket-internal.ts";

export class GameStateService implements GameStateStorage {
  private env: Env;
  // Development mode in-memory storage
  private devGameStates: Map<string, GameState> = new Map();
  private devPlayerStates: Map<string, PlayerState> = new Map();

  constructor(env: Env) {
    this.env = env;
  }

  async getGameState(roomId: string): Promise<GameState | null> {
    try {
      // In development, use in-memory storage
      if (!this.env?.GAME_STATE) {
        console.log("Development mode: Using in-memory game state storage");
        return this.devGameStates.get(`game:${roomId}`) || null;
      }

      const gameStateStr = await this.env.GAME_STATE.get(`game:${roomId}`);
      return gameStateStr ? JSON.parse(gameStateStr) : null;
    } catch (error) {
      console.error("Error getting game state:", error);
      return null;
    }
  }

  async updateGameState(roomId: string, gameState: GameState): Promise<void> {
    try {
      // In development, use in-memory storage
      if (!this.env?.GAME_STATE) {
        console.log("Development mode: Storing game state in memory");
        this.devGameStates.set(`game:${roomId}`, gameState);
        return;
      }

      const config = getConfig();
      await this.env.GAME_STATE.put(
        `game:${roomId}`,
        JSON.stringify(gameState),
        { expirationTtl: config.kv.defaultTtl },
      );
    } catch (error) {
      console.error("Error updating game state:", error);
    }
  }

  async getPlayerState(playerId: string): Promise<PlayerState | null> {
    try {
      // In development, use in-memory storage
      if (!this.env?.GAME_STATE) {
        console.log("Development mode: Using in-memory player state storage");
        return this.devPlayerStates.get(`player:${playerId}`) || null;
      }

      const playerStateStr = await this.env.GAME_STATE.get(`player:${playerId}`);
      return playerStateStr ? JSON.parse(playerStateStr) : null;
    } catch (error) {
      console.error("Error getting player state:", error);
      return null;
    }
  }

  async updatePlayerState(playerId: string, playerState: PlayerState): Promise<void> {
    try {
      // In development, use in-memory storage
      if (!this.env?.GAME_STATE) {
        console.log("Development mode: Storing player state in memory");
        this.devPlayerStates.set(`player:${playerId}`, playerState);
        return;
      }

      const config = getConfig();
      await this.env.GAME_STATE.put(
        `player:${playerId}`,
        JSON.stringify(playerState),
        { expirationTtl: config.kv.defaultTtl },
      );
    } catch (error) {
      console.error("Error updating player state:", error);
    }
  }
}