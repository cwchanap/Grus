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
      {/* Header with connection status - Mobile responsive */}
      <div class="scoreboard-header mb-3 sm:mb-4">
        <div class="flex justify-between items-center mb-2">
          <h3 class="text-base sm:text-lg font-semibold text-gray-800">Scoreboard</h3>
          <div class="flex items-center space-x-1 sm:space-x-2">
            <div class={`w-2 h-2 rounded-full ${
              connectionStatus.value === 'connected' ? 'bg-green-500' : 
              connectionStatus.value === 'connecting' ? 'bg-yellow-500' : 'bg-red-500'
            }`}></div>
            <span class="text-xs text-gray-500">
              <span class="hidden sm:inline">
                {connectionStatus.value === 'connected' ? 'Live' : 
                 connectionStatus.value === 'connecting' ? 'Connecting...' : 'Offline'}
              </span>
              <span class="sm:hidden">
                {connectionStatus.value === 'connected' ? '●' : 
                 connectionStatus.value === 'connecting' ? '○' : '×'}
              </span>
            </span>
          </div>
        </div>
      </div>

      {/* Game Status - Mobile responsive */}
      <div class="game-status mb-3 sm:mb-4 p-2 sm:p-3 rounded-lg border">
        <div class={`text-xs sm:text-sm font-medium px-2 py-1 rounded-full inline-block mb-2 ${getPhaseColor()}`}>
          {getPhaseText()}
        </div>
        
        {/* Round and Timer */}
        <div class="flex justify-between items-center text-xs sm:text-sm text-gray-600">
          <span>Round {localGameState.roundNumber}</span>
          {localGameState.phase === 'drawing' && (
            <div class="flex items-center space-x-1 sm:space-x-2">
              <span class="text-xs">⏱️</span>
              <span class="font-mono font-medium text-xs sm:text-sm">
                {formatTime(localGameState.timeRemaining)}
              </span>
            </div>
          )}
        </div>

        {/* Current Drawer */}
        {currentDrawer && localGameState.phase === 'drawing' && (
          <div class="mt-2 text-xs sm:text-sm">
            <span class="text-gray-600 hidden sm:inline">Current drawer: </span>
            <span class="text-gray-600 sm:hidden">Drawing: </span>
            <span class="font-medium text-blue-600">
              {currentDrawer.id === playerId ? 'You' : 
               (currentDrawer.name.length > 10 ? `${currentDrawer.name.slice(0, 10)}...` : currentDrawer.name)}
              {currentDrawer.isHost && ' 👑'}
            </span>
          </div>
        )}

        {/* Current Word (for drawer only) */}
        {localGameState.currentWord && localGameState.currentDrawer === playerId && localGameState.phase === 'drawing' && (
          <div class="mt-2 p-2 bg-blue-100 rounded border-l-4 border-blue-500">
            <div class="text-xs text-blue-700 font-medium">Your word:</div>
            <div class="text-base sm:text-lg font-bold text-blue-800">{localGameState.currentWord}</div>
          </div>
        )}
      </div>

      {/* Player Scores - Mobile responsive */}
      <div class="player-scores">
        <h4 class="text-xs sm:text-sm font-medium text-gray-700 mb-2 sm:mb-3">Players & Scores</h4>
        <div class="space-y-1 sm:space-y-2">
          {sortedPlayers.map((player, index) => (
            <div
              key={player.id}
              class={`flex items-center justify-between p-2 sm:p-3 rounded-lg border transition-colors ${
                player.id === playerId 
                  ? 'bg-blue-50 border-blue-200' 
                  : 'bg-white border-gray-200 hover:bg-gray-50'
              }`}
            >
              <div class="flex items-center space-x-2 sm:space-x-3 min-w-0 flex-1">
                {/* Rank */}
                <div class={`w-5 h-5 sm:w-6 sm:h-6 rounded-full flex items-center justify-center text-xs font-bold flex-shrink-0 ${
                  index === 0 ? 'bg-yellow-400 text-yellow-900' :
                  index === 1 ? 'bg-gray-300 text-gray-700' :
                  index === 2 ? 'bg-orange-300 text-orange-800' :
                  'bg-gray-100 text-gray-600'
                }`}>
                  {index + 1}
                </div>

                {/* Player Info */}
                <div class="flex flex-col min-w-0 flex-1">
                  <div class="flex items-center space-x-1 sm:space-x-2">
                    <span class={`font-medium text-xs sm:text-sm truncate ${
                      player.id === playerId ? 'text-blue-800' : 'text-gray-800'
                    }`}>
                      {player.name.length > 12 ? `${player.name.slice(0, 12)}...` : player.name}
                      {player.id === playerId && (
                        <span class="hidden xs:inline"> (You)</span>
                        <span class="xs:hidden"> (You)</span>
                      )}
                    </span>
                    {player.isHost && <span class="text-xs flex-shrink-0">👑</span>}
                    {player.id === localGameState.currentDrawer && localGameState.phase === 'drawing' && (
                      <span class="text-xs bg-blue-100 text-blue-700 px-1 rounded flex-shrink-0 hidden sm:inline">Drawing</span>
                      <span class="text-xs bg-blue-100 text-blue-700 px-1 rounded flex-shrink-0 sm:hidden">🎨</span>
                    )}
                  </div>
                  
                  {/* Connection Status */}
                  <div class="flex items-center space-x-1 text-xs">
                    <div class={`w-1.5 h-1.5 rounded-full ${
                      player.isConnected ? 'bg-green-500' : 'bg-red-500'
                    }`}></div>
                    <span class={`${player.isConnected ? 'text-green-600' : 'text-red-600'}`}>
                      <span class="hidden sm:inline">{player.isConnected ? 'Online' : 'Offline'}</span>
                      <span class="sm:hidden">{player.isConnected ? '●' : '○'}</span>
                    </span>
                  </div>
                </div>
              </div>

              {/* Score */}
              <div class="text-right flex-shrink-0">
                <div class="text-sm sm:text-lg font-bold text-gray-800">
                  {player.score}
                </div>
                <div class="text-xs text-gray-500 hidden sm:block">points</div>
                <div class="text-xs text-gray-500 sm:hidden">pts</div>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Game Controls (for host) - Mobile responsive */}
      {localGameState.players.find(p => p.id === playerId)?.isHost && (
        <div class="game-controls mt-3 sm:mt-4 p-2 sm:p-3 bg-gray-50 rounded-lg border">
          <h4 class="text-xs sm:text-sm font-medium text-gray-700 mb-2">Host Controls</h4>
          <div class="flex flex-col space-y-2">
            {localGameState.phase === 'waiting' && (
              <button
                class="px-3 py-2 sm:py-3 bg-green-600 text-white rounded hover:bg-green-700 active:bg-green-800 transition-colors text-sm font-medium touch-manipulation no-tap-highlight"
                disabled={connectionStatus.value !== 'connected' || sortedPlayers.length < 2}
              >
                Start Game
              </button>
            )}
            
            {localGameState.phase === 'drawing' && (
              <button
                class="px-3 py-2 sm:py-3 bg-orange-600 text-white rounded hover:bg-orange-700 active:bg-orange-800 transition-colors text-sm font-medium touch-manipulation no-tap-highlight"
                disabled={connectionStatus.value !== 'connected'}
              >
                End Round
              </button>
            )}

            {localGameState.phase === 'results' && (
              <button
                class="px-3 py-2 sm:py-3 bg-blue-600 text-white rounded hover:bg-blue-700 active:bg-blue-800 transition-colors text-sm font-medium touch-manipulation no-tap-highlight"
                disabled={connectionStatus.value !== 'connected'}
              >
                Next Round
              </button>
            )}
          </div>
          
          {sortedPlayers.length < 2 && localGameState.phase === 'waiting' && (
            <div class="text-xs text-gray-500 mt-2 text-center">
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