import { useEffect, useState } from "preact/hooks";
import { signal } from "@preact/signals";
import type { GameState, PlayerState } from "../types/game.ts";
import type { JSX } from "preact";
import GameSettingsModal, { type GameSettings } from "../components/GameSettingsModal.tsx";

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
  const [showSettingsModal, setShowSettingsModal] = useState(false);
  const [gameSettings, setGameSettings] = useState<GameSettings>({
    maxRounds: 5,
    roundTimeMinutes: 1,
    roundTimeSeconds: 30
  });
  const [phaseTransition, setPhaseTransition] = useState<string | null>(null);
  const [previousRound, setPreviousRound] = useState<number>(gameState.roundNumber);
  const [clientTimeRemaining, setClientTimeRemaining] = useState<number>(gameState.timeRemaining);
  const [lastServerUpdate, setLastServerUpdate] = useState<number>(Date.now());

  // Update local state when props change and detect transitions
  useEffect(() => {
    // Detect round transitions
    if (gameState.roundNumber !== previousRound) {
      setPhaseTransition(`Round ${gameState.roundNumber} starting!`);
      setPreviousRound(gameState.roundNumber);
      
      // Clear transition message after 3 seconds
      setTimeout(() => setPhaseTransition(null), 3000);
    }
    
    // Detect phase transitions
    if (gameState.phase !== localGameState.phase) {
      let transitionMessage = '';
      switch (gameState.phase) {
        case 'drawing':
          transitionMessage = 'Drawing phase started!';
          break;
        case 'guessing':
          transitionMessage = 'Guessing time!';
          break;
        case 'results':
          transitionMessage = 'Round complete!';
          break;
        case 'waiting':
          transitionMessage = 'Waiting for next round...';
          break;
      }
      
      if (transitionMessage && localGameState.phase !== 'waiting') {
        setPhaseTransition(transitionMessage);
        setTimeout(() => setPhaseTransition(null), 2500);
      }
    }
    
    setLocalGameState(gameState);
    setClientTimeRemaining(gameState.timeRemaining);
    setLastServerUpdate(Date.now());
  }, [gameState, localGameState.phase, previousRound]);

  // Client-side timer countdown for smooth updates
  useEffect(() => {
    if (localGameState.phase !== 'drawing' && localGameState.phase !== 'guessing') {
      return;
    }

    let hasPlayedWarning = false;
    let hasPlayedCritical = false;

    const interval = setInterval(() => {
      const now = Date.now();
      const elapsed = now - lastServerUpdate;
      const newTime = Math.max(0, localGameState.timeRemaining - elapsed);
      const seconds = Math.floor(newTime / 1000);
      
      // Play notification sounds at specific thresholds
      if (seconds === 30 && !hasPlayedWarning) {
        playNotificationSound('warning');
        hasPlayedWarning = true;
      } else if (seconds === 10 && !hasPlayedCritical) {
        playNotificationSound('critical');
        hasPlayedCritical = true;
      }
      
      setClientTimeRemaining(newTime);
      
      // Stop countdown when time reaches 0
      if (newTime <= 0) {
        clearInterval(interval);
      }
    }, 100); // Update every 100ms for smooth countdown

    return () => clearInterval(interval);
  }, [localGameState.phase, localGameState.timeRemaining, lastServerUpdate]);

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
            } else if (message.type === 'timer-sync') {
              // Handle timer synchronization
              const { timeRemaining, serverTimestamp } = message.data;
              const clientTimestamp = Date.now();
              const latency = clientTimestamp - serverTimestamp;
              const adjustedTime = Math.max(0, timeRemaining - latency);
              
              setLocalGameState(prev => ({
                ...prev,
                timeRemaining: adjustedTime
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

  // Get timer warning state
  const getTimerWarningState = (timeRemaining: number): 'normal' | 'warning' | 'critical' => {
    const seconds = Math.floor(timeRemaining / 1000);
    if (seconds <= 10) return 'critical';
    if (seconds <= 30) return 'warning';
    return 'normal';
  };

  // Get timer color classes
  const getTimerColorClasses = (warningState: 'normal' | 'warning' | 'critical'): string => {
    switch (warningState) {
      case 'critical': return 'text-red-600 bg-red-100 border-red-300 animate-pulse';
      case 'warning': return 'text-orange-600 bg-orange-100 border-orange-300';
      case 'normal': return 'text-blue-600 bg-blue-100 border-blue-300';
    }
  };

  // Calculate round progress percentage
  const getRoundProgress = (): number => {
    if (!gameSettings.maxRounds) return 0;
    return Math.min((localGameState.roundNumber / gameSettings.maxRounds) * 100, 100);
  };

  // Play notification sound (optional)
  const playNotificationSound = (type: 'warning' | 'critical') => {
    try {
      // Create a simple beep sound using Web Audio API
      const audioContext = new (window.AudioContext || (window as any).webkitAudioContext)();
      const oscillator = audioContext.createOscillator();
      const gainNode = audioContext.createGain();
      
      oscillator.connect(gainNode);
      gainNode.connect(audioContext.destination);
      
      oscillator.frequency.value = type === 'critical' ? 800 : 600;
      oscillator.type = 'sine';
      
      gainNode.gain.setValueAtTime(0.1, audioContext.currentTime);
      gainNode.gain.exponentialRampToValueAtTime(0.01, audioContext.currentTime + 0.2);
      
      oscillator.start(audioContext.currentTime);
      oscillator.stop(audioContext.currentTime + 0.2);
    } catch (error) {
      // Silently fail if audio is not supported
      console.log('Audio notification not supported');
    }
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

  // Check if current player is host
  const isHost = localGameState.players.find(p => p.id === playerId)?.isHost || false;

  // Send WebSocket message
  const sendGameControlMessage = (type: 'start-game' | 'next-round' | 'end-game', data: any = {}) => {
    const ws = wsConnection.value;
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({
        type,
        roomId,
        playerId,
        data
      }));
    }
  };

  // Handle start game
  const handleStartGame = () => {
    sendGameControlMessage('start-game');
  };

  // Handle next round
  const handleNextRound = () => {
    sendGameControlMessage('next-round');
  };

  // Handle end game
  const handleEndGame = () => {
    sendGameControlMessage('end-game');
  };

  // Handle settings save
  const handleSettingsSave = (newSettings: GameSettings) => {
    setGameSettings(newSettings);
    // In a real implementation, you might want to send these settings to the server
    // For now, they're just stored locally for display purposes
  };

  return (
    <div className={`bg-white rounded-lg shadow-md p-4 ${className}`}>
      {/* Phase Transition Notification */}
      {phaseTransition && (
        <div className="mb-4 p-3 bg-gradient-to-r from-blue-500 to-purple-600 text-white rounded-lg text-center font-medium phase-transition">
          {phaseTransition}
        </div>
      )}

      {/* Connection Status */}
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold text-gray-800">Scoreboard</h2>
        <div className="flex items-center space-x-2">
          <div className={`w-2 h-2 rounded-full ${
            connectionStatus.value === 'connected' ? 'bg-green-500' : 
            connectionStatus.value === 'connecting' ? 'bg-yellow-500' : 'bg-red-500'
          }`}></div>
          <span className="text-xs text-gray-600 capitalize">
            {connectionStatus.value}
          </span>
        </div>
      </div>

      {/* Game Status */}
      <div className="mb-4">
        <div className={`inline-block px-3 py-1 rounded-full text-sm font-medium ${getPhaseColor()}`}>
          {getPhaseText()}
        </div>
        
        {/* Enhanced Timer Display */}
        {(localGameState.phase === 'drawing' || localGameState.phase === 'guessing') && (
          <div className="mt-3">
            <div className={`inline-flex items-center px-4 py-2 rounded-lg border-2 font-mono text-lg font-bold ${
              getTimerColorClasses(getTimerWarningState(clientTimeRemaining))
            } ${getTimerWarningState(clientTimeRemaining) === 'critical' ? 'timer-pulse' : ''}`}>
              <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              {formatTime(clientTimeRemaining)}
            </div>
            {getTimerWarningState(clientTimeRemaining) === 'critical' && (
              <div className="text-xs text-red-600 mt-1 font-medium animate-pulse">
                Time's almost up!
              </div>
            )}
            {getTimerWarningState(clientTimeRemaining) === 'warning' && (
              <div className="text-xs text-orange-600 mt-1 font-medium">
                Hurry up!
              </div>
            )}
            
            {/* Timer Progress Bar */}
            <div className="mt-2 w-full bg-gray-200 rounded-full h-1">
              <div 
                className={`h-1 rounded-full transition-all duration-100 ${
                  getTimerWarningState(clientTimeRemaining) === 'critical' ? 'bg-red-500' :
                  getTimerWarningState(clientTimeRemaining) === 'warning' ? 'bg-orange-500' : 'bg-blue-500'
                }`}
                style={{ 
                  width: `${Math.max(0, (clientTimeRemaining / (gameSettings.roundTimeMinutes * 60 * 1000 + gameSettings.roundTimeSeconds * 1000)) * 100)}%` 
                }}
              ></div>
            </div>
          </div>
        )}
        
        {/* Round Progress */}
        <div className="mt-3">
          <div className="flex items-center justify-between text-sm text-gray-600 mb-1">
            <span>Round Progress</span>
            <span>{localGameState.roundNumber} / {gameSettings.maxRounds}</span>
          </div>
          <div className="w-full bg-gray-200 rounded-full h-2">
            <div 
              className="bg-blue-600 h-2 rounded-full round-progress-fill"
              style={{ width: `${getRoundProgress()}%` }}
            ></div>
          </div>
          {localGameState.roundNumber === gameSettings.maxRounds && (
            <div className="text-xs text-orange-600 mt-1 font-medium">
              Final round!
            </div>
          )}
        </div>
      </div>

      {/* Current Drawer */}
      {currentDrawer && localGameState.phase === 'drawing' && (
        <div className="mb-4 p-2 bg-blue-50 rounded-lg">
          <div className="text-sm font-medium text-blue-800">
            {currentDrawer.name} is drawing
          </div>
        </div>
      )}

      {/* Player Scores */}
      <div className="space-y-2 mb-4">
        <h3 className="text-sm font-medium text-gray-700">Players</h3>
        {sortedPlayers.map((player, index) => (
          <div key={player.id} className="flex items-center justify-between p-2 bg-gray-50 rounded">
            <div className="flex items-center space-x-2">
              <span className="text-sm font-medium text-gray-600">#{index + 1}</span>
              <span className="text-sm font-medium text-gray-800">{player.name}</span>
              {player.isHost && (
                <span className="text-xs bg-yellow-100 text-yellow-800 px-2 py-1 rounded">Host</span>
              )}
              {player.id === localGameState.currentDrawer && (
                <span className="text-xs bg-blue-100 text-blue-800 px-2 py-1 rounded">Drawing</span>
              )}
              <div className={`w-2 h-2 rounded-full ${
                player.isConnected ? 'bg-green-500' : 'bg-gray-400'
              }`}></div>
            </div>
            <span className="text-sm font-semibold text-gray-800">{player.score} pts</span>
          </div>
        ))}
      </div>

      {/* Host Controls */}
      {isHost && (
        <div className="border-t pt-4">
          <h3 className="text-sm font-medium text-gray-700 mb-3">Host Controls</h3>
          
          {/* Start Game Button */}
          {localGameState.phase === 'waiting' && (
            <button
              onClick={handleStartGame}
              className="w-full bg-green-600 hover:bg-green-700 text-white font-medium py-2 px-4 rounded-lg transition-colors duration-200 mb-2"
              disabled={sortedPlayers.filter(p => p.isConnected).length < 2}
            >
              Start Game
            </button>
          )}

          {/* End Round Button (during drawing phase) */}
          {localGameState.phase === 'drawing' && (
            <button
              onClick={handleNextRound}
              className="w-full bg-orange-600 hover:bg-orange-700 text-white font-medium py-2 px-4 rounded-lg transition-colors duration-200 mb-2"
            >
              End Round Early
            </button>
          )}

          {/* Next Round Button (during results phase) */}
          {localGameState.phase === 'results' && (
            <button
              onClick={handleNextRound}
              className="w-full bg-blue-600 hover:bg-blue-700 text-white font-medium py-2 px-4 rounded-lg transition-colors duration-200 mb-2"
            >
              Start Next Round
            </button>
          )}

          {/* End Game Button */}
          {(localGameState.phase === 'drawing' || localGameState.phase === 'results') && (
            <button
              onClick={handleEndGame}
              className="w-full bg-red-600 hover:bg-red-700 text-white font-medium py-2 px-4 rounded-lg transition-colors duration-200"
            >
              End Game
            </button>
          )}

          {/* Game Settings Button */}
          {localGameState.phase === 'waiting' && (
            <button
              onClick={() => setShowSettingsModal(true)}
              className="w-full bg-gray-600 hover:bg-gray-700 text-white font-medium py-2 px-4 rounded-lg transition-colors duration-200 mb-2"
            >
              Game Settings
            </button>
          )}

          {/* Game Settings Info */}
          <div className="mt-3 text-xs text-gray-500">
            <div>Max Rounds: {gameSettings.maxRounds}</div>
            <div>Round Time: {gameSettings.roundTimeMinutes}:{gameSettings.roundTimeSeconds.toString().padStart(2, '0')}</div>
          </div>
        </div>
      )}

      {/* Game Settings Modal */}
      <GameSettingsModal
        isOpen={showSettingsModal}
        onClose={() => setShowSettingsModal(false)}
        onSave={handleSettingsSave}
        currentSettings={gameSettings}
      />
    </div>
  );
}