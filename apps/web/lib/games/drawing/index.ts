// Drawing game registration and exports
import { GameRegistry, GameTypeInfo } from "../../core/game-registry.ts";
import { DrawingGameEngine } from "./drawing-engine.ts";
import { DrawingGameSettings } from "../../../types/games/drawing.ts";

// Register the drawing game
const drawingGameInfo: GameTypeInfo = {
  id: "drawing",
  name: "Drawing & Guessing",
  description: "Draw pictures and guess what others are drawing",
  minPlayers: 2,
  maxPlayers: 8,
  defaultSettings: {
    maxRounds: 3,
    roundTimeSeconds: 75,
  } as DrawingGameSettings,
};

// Register the game with the registry
GameRegistry.getInstance().registerGame(
  drawingGameInfo,
  () => new DrawingGameEngine(),
);

// Export everything from this game module
export * from "./drawing-engine.ts";
export * from "./drawing-utils.ts";
export { drawingGameInfo };
