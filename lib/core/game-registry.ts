// Game registry for managing different game types
import { GameEngine } from "./game-engine.ts";
import { BaseGameSettings, BaseGameState } from "../../types/core/game.ts";
import { BaseClientMessage, BaseServerMessage } from "../../types/core/websocket.ts";

export interface GameTypeInfo {
  id: string;
  name: string;
  description: string;
  minPlayers: number;
  maxPlayers: number;
  defaultSettings: BaseGameSettings;
}

export class GameRegistry {
  private static instance: GameRegistry;
  private engines: Map<string, () => GameEngine> = new Map();
  private gameTypes: Map<string, GameTypeInfo> = new Map();

  private constructor() {}

  static getInstance(): GameRegistry {
    if (!GameRegistry.instance) {
      GameRegistry.instance = new GameRegistry();
    }
    return GameRegistry.instance;
  }

  registerGame<
    TGameState extends BaseGameState,
    TSettings extends BaseGameSettings,
    TClientMessage extends BaseClientMessage,
    TServerMessage extends BaseServerMessage,
  >(
    gameInfo: GameTypeInfo,
    engineFactory: () => GameEngine<TGameState, TSettings, TClientMessage, TServerMessage>,
  ): void {
    this.gameTypes.set(gameInfo.id, gameInfo);
    this.engines.set(gameInfo.id, engineFactory);
  }

  getGameEngine(gameType: string): GameEngine | null {
    const engineFactory = this.engines.get(gameType);
    return engineFactory ? engineFactory() : null;
  }

  getGameType(gameType: string): GameTypeInfo | null {
    return this.gameTypes.get(gameType) || null;
  }

  getAllGameTypes(): GameTypeInfo[] {
    return Array.from(this.gameTypes.values());
  }

  isValidGameType(gameType: string): boolean {
    return this.gameTypes.has(gameType);
  }

  getDefaultSettings(gameType: string): BaseGameSettings | null {
    const gameInfo = this.gameTypes.get(gameType);
    return gameInfo ? gameInfo.defaultSettings : null;
  }
}
