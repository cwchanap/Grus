// WebSocket module exports
export { WebSocketManager } from "./websocket-manager.ts";
export { WebSocketServer } from "./core/websocket-server.ts";
export { ConnectionPool } from "./core/connection-pool.ts";
export { MessageRouter } from "./core/message-router.ts";

// Services
export { GameStateService } from "./services/game-state-service.ts";
export { PlayerService } from "./services/player-service.ts";
export { TimerService } from "./services/timer-service.ts";

// Handlers
export { RoomHandler } from "./handlers/room-handler.ts";
export { ChatHandler } from "./handlers/chat-handler.ts";
export { DrawingHandler } from "./handlers/drawing-handler.ts";
export { GameHandler } from "./handlers/game-handler.ts";

// Utils
export { MessageValidator } from "./utils/message-validator.ts";
export { RateLimiter } from "./utils/rate-limiter.ts";
export { WordGenerator } from "./utils/word-generator.ts";

// Types
export type * from "./types/websocket-internal.ts";

// Legacy compatibility - re-export WebSocketServer as WebSocketHandler
export { WebSocketServer as WebSocketHandler } from "./core/websocket-server.ts";
