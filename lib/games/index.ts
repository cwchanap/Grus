// Game modules index - register all games here
import "./drawing/index.ts"; // This will register the drawing game

// Export game registry for use in other parts of the application
export { GameRegistry } from "../core/game-registry.ts";
export type { GameTypeInfo } from "../core/game-registry.ts";
