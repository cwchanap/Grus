import { useEffect, useState } from "preact/hooks";
import { signal } from "@preact/signals";
import type { GameState, PlayerState } from "../types/game.ts";
import type { JSX } from "preact";

interface ScoreboardProps {
  roomId: string;
  playerId: string;
  gameState: GameState;
  onGameStateUpdate?: (gameState: GameState) => void;
  className?: string;
}

// Global signal for WebSocket connection
const wsConnection = signal<WebSocket | null>(null);
const connectionStatus = signal<'connecting' | 'connected' | 'disconnected'>('disconnected');

export default function Scoreboard({
  roomId,
  playerId,
  gameState,
  onGameStateUpdate,
  className = ""
}: ScoreboardProps) {
  const [localGameState, setLocalGameState] = useState<GameState>(gameState);

  // Update local state when props change
  useEffect(() => {
    setLocalGameState(gameState);
  }, [gameState]);

  // WebSocket connection management
  useEffect(() => {
    // Skip WebSocket in development environment
    if (window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1') {
      console.log('WebSocket disabled in development environment');
      connectionStatus.value = 'disconnected';
      return;
    }

    let ws: WebSocket | null = null;
    let reconnectTimeout: number | null = null;
    let reconnectAttempts = 0;
    const maxReconnectAttempts = 5;

    const connectWebSocket = () => {
      try {
        connectionStatus.value = 'connecting';
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}/api/websocket?roomId=${roomId}`;
        
        ws = new WebSocket(wsUrl);
        wsConnection.value = ws;

        ws.onopen = () => {
          console.log('Scoreboard WebSocket connected');
          connectionStatus.value = 'connected';
          reconnectAttempts = 0;
          
          // Subscribe to game state updates
          ws?.send(JSON.stringify({
            type: 'join-room',
            roomId,
            playerId,
            data: { subscribeToGameState: true }
          }));
        };

        ws.onmessage = (event) => {
          try {
            const message = JSON.parse(event.data);
            
            if (message.type === 'game-state') {
              const updatedGameState: GameState = message.data;
              setLocalGameState(updatedGameState);
              
              if (onGameStateUpdate) {
                onGameStateUpdate(updatedGameState);
              }
            } else if (message.type === 'score-update') {
              // Handle individual score updates
              const { playerId: scoredPlayerId, newScore } = message.data;
              setLocalGameState(prev => ({
                ...prev,
                scores: {
                  ...prev.scores,
                  [scoredPlayerId]: newScore
                }
              }));
            }
          } catch (error) {
            console.error('Error parsing scoreboard message:', error);
          }
        };

        ws.onclose = () => {
          console.log('Scoreboard WebSocket disconnected');
          connectionStatus.value = 'disconnected';
          wsConnection.value = null;
          
          // Don't attempt to reconnect in development
          if (window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1') {
            return;
          }
          
          // Attempt to reconnect with exponential backoff
          if (reconnectAttempts < maxReconnectAttempts) {
            const delay = Math.min(1000 * Math.pow(2, reconnectAttempts), 30000);
            reconnectTimeout = setTimeout(() => {
              reconnectAttempts++;
              connectWebSocket();
            }, delay);
          }
        };

        ws.onerror = (error) => {
          console.error('Scoreboard WebSocket error:', error);
          connectionStatus.value = 'disconnected';
        };
      } catch (error) {
        console.error('Failed to connect scoreboard WebSocket:', error);
        connectionStatus.value = 'disconnected';
      }
    };

    connectWebSocket();

    return () => {
      if (reconnectTimeout) {
        clearTimeout(reconnectTimeout);
      }
      if (ws) {
        ws.close();
      }
    };
  }, [roomId, playerId, onGameStateUpdate]);

  // Format time remaining as MM:SS
  const formatTime = (milliseconds: number): string => {
    const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000));
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds.toString().padStart(2, '0')}`;
  };

  // Get sorted players by score
  const getSortedPlayers = (): (PlayerState & { score: number })[] => {
    return localGameState.players
      .map(player => ({
        ...player,
        score: localGameState.scores[player.id] || 0
      }))
      .sort((a, b) => b.score - a.score);
  };

  // Get current drawer info
  const getCurrentDrawer = (): PlayerState | null => {
    return localGameState.players.find(p => p.id === localGameState.currentDrawer) || null;
  };

  // Get phase display text
  const getPhaseText = (): string => {
    switch (localGameState.phase) {
      case 'waiting': return 'Waiting for game to start';
      case 'drawing': return 'Drawing in progress';
      case 'guessing': return 'Guessing time';
      case 'results': return 'Round complete';
      default: return 'Unknown phase';
    }
  };

  // Get phase color
  const getPhaseColor = (): string => {
    switch (localGameState.phase) {
      case 'waiting': return 'text-yellow-600 bg-yellow-50';
      case 'drawing': return 'text-blue-600 bg-blue-50';
      case 'guessing': return 'text-green-600 bg-green-50';
      case 'results': return 'text-purple-600 bg-purple-50';
      default: return 'text-gray-600 bg-gray-50';
    }
  };

  const sortedPlayers = getSortedPlayers();
  const currentDrawer = getCurrentDrawer();

  return (
    <div>Scoreboard temporarily disabled for testing</div>
  );
}