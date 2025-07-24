// Game-specific type definitions

export interface GameState {
  roomId: string;
  currentDrawer: string;
  currentWord: string;
  roundNumber: number;
  timeRemaining: number;
  phase: 'waiting' | 'drawing' | 'guessing' | 'results';
  players: PlayerState[];
  scores: Record<string, number>;
  drawingData: DrawingCommand[];
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

export interface DrawingCommand {
  type: 'start' | 'move' | 'end' | 'clear';
  x?: number;
  y?: number;
  color?: string;
  size?: number;
  timestamp: number;
}

export interface ChatMessage {
  id: string;
  playerId: string;
  playerName: string;
  message: string;
  timestamp: number;
  isGuess: boolean;
  isCorrect?: boolean;
}

export interface Room {
  id: string;
  name: string;
  hostId: string;
  maxPlayers: number;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface GameSession {
  id: string;
  roomId: string;
  winnerId?: string;
  totalRounds: number;
  startedAt?: string;
  endedAt?: string;
}

export interface Score {
  id: string;
  sessionId: string;
  playerId: string;
  points: number;
  correctGuesses: number;
}

// WebSocket message types
export interface ClientMessage {
  type: 'join-room' | 'leave-room' | 'chat' | 'draw' | 'guess' | 'start-game' | 'next-round' | 'end-game' | 'ping';
  roomId: string;
  playerId: string;
  data: any;
}

export interface ServerMessage {
  type: 'room-update' | 'chat-message' | 'draw-update' | 'game-state' | 'score-update' | 'error' | 'pong';
  roomId: string;
  data: any;
}