// Drawing game specific types

import { BaseGameSettings, BaseGameState } from "../core/game.ts";
import { BaseClientMessage, BaseServerMessage } from "../core/websocket.ts";
import { ChatMessage } from "../core/room.ts";

export interface DrawingGameSettings extends BaseGameSettings {
  // Drawing game specific settings can be added here
}

export interface DrawingCommand {
  type: "start" | "move" | "end" | "clear";
  x?: number;
  y?: number;
  color?: string;
  size?: number;
  timestamp: number;
}

export interface DrawingGameData {
  currentDrawer: string;
  currentWord: string;
  drawingData: DrawingCommand[];
  correctGuesses: Array<{ playerId: string; timestamp: number }>;
}

export interface DrawingGameState extends BaseGameState<DrawingGameSettings, DrawingGameData> {
  gameType: "drawing";
  phase: "waiting" | "playing" | "results" | "finished";
  drawingPhase?: "drawing" | "guessing"; // Game-specific sub-phase within "playing"
}

// Extend chat message for drawing game specific features
export interface DrawingChatMessage extends ChatMessage {
  isGuess: boolean;
  isCorrect?: boolean;
}

// Drawing game specific WebSocket messages
export interface DrawingClientMessage extends BaseClientMessage {
  type:
    | "join-room"
    | "leave-room"
    | "chat"
    | "draw"
    | "guess"
    | "start-game"
    | "next-round"
    | "end-game"
    | "update-settings"
    | "ping";
}

export interface DrawingServerMessage extends BaseServerMessage {
  type:
    | "room-update"
    | "chat-message"
    | "draw-update"
    | "draw-update-batch"
    | "game-state"
    | "score-update"
    | "host-changed"
    | "settings-updated"
    | "error"
    | "pong";
}
