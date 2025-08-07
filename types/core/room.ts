// Core room and player types - game agnostic

export interface Room {
  id: string;
  name: string;
  hostId: string;
  maxPlayers: number;
  isActive: boolean;
  gameType: string; // New field to support multiple game types
  createdAt: string;
  updatedAt: string;
}

export interface Player {
  id: string;
  name: string;
  roomId?: string;
  isHost: boolean;
  joinedAt: string;
}

export interface PlayerState {
  id: string;
  name: string;
  isHost: boolean;
  isConnected: boolean;
  lastActivity: number;
}

export interface ChatMessage {
  id: string;
  playerId: string;
  playerName: string;
  message: string;
  timestamp: number;
  isSystemMessage?: boolean;
}
