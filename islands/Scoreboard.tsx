import { useEffect, useState } from "preact/hooks";
import { signal } from "@preact/signals";
import type { GameState, PlayerState } from "../types/game.ts";

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
    <div class={`scoreboard ${className}`}>
      {/* Header with connection status */}
      <div class="scoreboard-header mb-4">
        <div class="flex justify-between items-center mb-2">
          <h3 class="text-lg font-semibold text-gray-800">Scoreboard</h3>
          <div class="flex items-center space-x-2">
            <div class={`w-2 h-2 rounded-full ${
              connectionStatus.value === 'connected' ? 'bg-green-500' : 
              connectionStatus.value === 'connecting' ? 'bg-yellow-500' : 'bg-red-500'
            }`}></div>
            <span class="text-xs text-gray-500">
              {connectionStatus.value === 'connected' ? 'Live' : 
               connectionStatus.value === 'connecting' ? 'Connecting...' : 'Offline'}
            </span>
          </div>
        </div>
      </div>

      {/* Game Status */}
      <div class="game-status mb-4 p-3 rounded-lg border">
        <div class={`text-sm font-medium px-2 py-1 rounded-full inline-block mb-2 ${getPhaseColor()}`}>
          {getPhaseText()}
        </div>
        
        {/* Round and Timer */}
        <div class="flex justify-between items-center text-sm text-gray-600">
          <span>Round {localGameState.roundNumber}</span>
          {localGameState.phase === 'drawing' && (
            <div class="flex items-center space-x-2">
              <span class="text-xs">⏱️</span>
              <span class="font-mono font-medium">
                {formatTime(localGameState.timeRemaining)}
              </span>
            </div>
          )}
        </div>

        {/* Current Drawer */}
        {currentDrawer && localGameState.phase === 'drawing' && (
          <div class="mt-2 text-sm">
            <span class="text-gray-600">Current drawer: </span>
            <span class="font-medium text-blue-600">
              {currentDrawer.id === playerId ? 'You' : currentDrawer.name}
              {currentDrawer.isHost && ' 👑'}
            </span>
          </div>
        )}

        {/* Current Word (for drawer only) */}
        {localGameState.currentWord && localGameState.currentDrawer === playerId && localGameState.phase === 'drawing' && (
          <div class="mt-2 p-2 bg-blue-100 rounded border-l-4 border-blue-500">
            <div class="text-xs text-blue-700 font-medium">Your word:</div>
            <div class="text-lg font-bold text-blue-800">{localGameState.currentWord}</div>
          </div>
        )}
      </div>

      {/* Player Scores */}
      <div class="player-scores">
        <h4 class="text-sm font-medium text-gray-700 mb-3">Players & Scores</h4>
        <div class="space-y-2">
          {sortedPlayers.map((player, index) => (
            <div
              key={player.id}
              class={`flex items-center justify-between p-3 rounded-lg border transition-colors ${
                player.id === playerId 
                  ? 'bg-blue-50 border-blue-200' 
                  : 'bg-white border-gray-200 hover:bg-gray-50'
              }`}
            >
              <div class="flex items-center space-x-3">
                {/* Rank */}
                <div class={`w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold ${
                  index === 0 ? 'bg-yellow-400 text-yellow-900' :
                  index === 1 ? 'bg-gray-300 text-gray-700' :
                  index === 2 ? 'bg-orange-300 text-orange-800' :
                  'bg-gray-100 text-gray-600'
                }`}>
                  {index + 1}
                </div>

                {/* Player Info */}
                <div class="flex flex-col">
                  <div class="flex items-center space-x-2">
                    <span class={`font-medium ${
                      player.id === playerId ? 'text-blue-800' : 'text-gray-800'
                    }`}>
                      {player.name}
                      {player.id === playerId && ' (You)'}
                    </span>
                    {player.isHost && <span class="text-xs">👑</span>}
                    {player.id === localGameState.currentDrawer && localGameState.phase === 'drawing' && (
                      <span class="text-xs bg-blue-100 text-blue-700 px-1 rounded">Drawing</span>
                    )}
                  </div>
                  
                  {/* Connection Status */}
                  <div class="flex items-center space-x-1 text-xs">
                    <div class={`w-1.5 h-1.5 rounded-full ${
                      player.isConnected ? 'bg-green-500' : 'bg-red-500'
                    }`}></div>
                    <span class={player.isConnected ? 'text-green-600' : 'text-red-600'}>
                      {player.isConnected ? 'Online' : 'Offline'}
                    </span>
                  </div>
                </div>
              </div>

              {/* Score */}
              <div class="text-right">
                <div class="text-lg font-bold text-gray-800">
                  {player.score}
                </div>
                <div class="text-xs text-gray-500">points</div>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Game Controls (for host) */}
      {localGameState.players.find(p => p.id === playerId)?.isHost && (
        <div class="game-controls mt-4 p-3 bg-gray-50 rounded-lg border">
          <h4 class="text-sm font-medium text-gray-700 mb-2">Host Controls</h4>
          <div class="flex flex-col space-y-2">
            {localGameState.phase === 'waiting' && (
              <button
                class="px-3 py-2 bg-green-600 text-white rounded hover:bg-green-700 transition-colors text-sm font-medium"
                disabled={connectionStatus.value !== 'connected' || sortedPlayers.length < 2}
              >
                Start Game
              </button>
            )}
            
            {localGameState.phase === 'drawing' && (
              <button
                class="px-3 py-2 bg-orange-600 text-white rounded hover:bg-orange-700 transition-colors text-sm font-medium"
                disabled={connectionStatus.value !== 'connected'}
              >
                End Round
              </button>
            )}

            {localGameState.phase === 'results' && (
              <button
                class="px-3 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors text-sm font-medium"
                disabled={connectionStatus.value !== 'connected'}
              >
                Next Round
              </button>
            )}
          </div>
          
          {sortedPlayers.length < 2 && localGameState.phase === 'waiting' && (
            <div class="text-xs text-gray-500 mt-2">
              Need at least 2 players to start
            </div>
          )}
        </div>
      )}

      {/* Debug Info (development only) */}
      {false && ( // Debug info disabled in production
        <div class="debug-info mt-4 p-2 bg-gray-100 rounded text-xs text-gray-600">
          <div>Room: {roomId}</div>
          <div>Player: {playerId}</div>
          <div>Phase: {localGameState.phase}</div>
          <div>Round: {localGameState.roundNumber}</div>
          <div>Drawer: {localGameState.currentDrawer}</div>
          <div>Word: {localGameState.currentWord || 'None'}</div>
          <div>Time: {formatTime(localGameState.timeRemaining)}</div>
          <div>Connection: {connectionStatus.value}</div>
        </div>
      )}
    </div>
  );
}