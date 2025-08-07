// Core WebSocket message types - game agnostic

export interface BaseClientMessage {
  type: string;
  roomId: string;
  playerId: string;
  data: any;
}

export interface BaseServerMessage {
  type: string;
  roomId: string;
  data: any;
}

// Common message types that all games should support
export interface CoreClientMessage extends BaseClientMessage {
  type:
    | "join-room"
    | "leave-room"
    | "chat"
    | "start-game"
    | "end-game"
    | "update-settings"
    | "ping";
}

export interface CoreServerMessage extends BaseServerMessage {
  type:
    | "room-update"
    | "chat-message"
    | "game-state"
    | "score-update"
    | "host-changed"
    | "settings-updated"
    | "error"
    | "pong";
}
