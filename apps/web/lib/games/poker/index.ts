// Poker game registration and exports
import { GameRegistry, GameTypeInfo } from "../../core/game-registry.ts";
import { PokerGameEngine } from "./poker-engine.ts";
import { PokerGameSettings } from "../../../types/games/poker.ts";

// Register the poker game
const pokerGameInfo: GameTypeInfo = {
  id: "poker",
  name: "Texas Hold'em Poker",
  description: "Classic Texas Hold'em poker with betting rounds",
  minPlayers: 2,
  maxPlayers: 8,
  defaultSettings: {
    buyIn: 1000,
    smallBlind: 10,
    bigBlind: 20,
    roundTimeSeconds: 300,
    maxRounds: 10,
  } as PokerGameSettings,
};

// Register the game with the registry
GameRegistry.getInstance().registerGame(
  pokerGameInfo,
  () => new PokerGameEngine(),
);

// Export everything from this game module
export * from "./poker-engine.ts";
export { pokerGameInfo };
