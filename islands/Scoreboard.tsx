import { useEffect, useMemo, useState } from "preact/hooks";
import { signal } from "@preact/signals";
import type { BaseGameState } from "../types/core/game.ts";
import type { PlayerState } from "../types/core/room.ts";
import type { DrawingGameState } from "../types/games/drawing.ts";
// import type { JSX } from "preact";

interface ScoreboardProps {
  roomId: string;
  playerId: string;
  gameState: BaseGameState;
  onGameStateUpdate?: (gameState: BaseGameState) => void;
  onShowSettingsModal?: () => void;
  className?: string;
}

// Global signal for WebSocket connection
const _wsConnection = signal<WebSocket | null>(null);
const connectionStatus = signal<"connecting" | "connected" | "disconnected">("disconnected");

export default function Scoreboard({
  roomId,
  playerId,
  gameState,
  onGameStateUpdate,
  onShowSettingsModal,
  className = "",
}: ScoreboardProps) {
  const [localGameState, setLocalGameState] = useState<BaseGameState>(gameState);

  // Sync local state with gameState prop changes - simplified dependencies
  useEffect(() => {
    if (!gameState.players || !Array.isArray(gameState.players)) {
      console.error("Scoreboard: Invalid gameState prop - players is not an array");
      return;
    }

    // Always update local state when gameState prop changes
    setLocalGameState(gameState);
  }, [gameState]);

  const [phaseTransition, setPhaseTransition] = useState<string | null>(null);
  const [previousRound, setPreviousRound] = useState<number>(gameState.roundNumber);

  const [clientTimeRemaining, setClientTimeRemaining] = useState<number>(gameState.timeRemaining);
  const [lastServerUpdate, setLastServerUpdate] = useState<number>(Date.now());
  const [isStartingGame, setIsStartingGame] = useState(false);

  // Helper function to emit header update events (instead of direct DOM manipulation)
  const emitHeaderUpdate = (updatedGameState: BaseGameState) => {
    try {
      // Emit custom event for header components to listen to
      globalThis.dispatchEvent(
        new CustomEvent("headerUpdate", {
          detail: {
            playerCount: updatedGameState.players.length,
            hostName: updatedGameState.players.find((p) => p.isHost)?.name || "Unknown",
          },
        }),
      );
    } catch (error) {
      console.error("Error emitting header update:", error);
    }
  };

  // Update local state when localGameState changes and detect transitions
  useEffect(() => {
    // Detect round transitions
    if (localGameState.roundNumber !== previousRound) {
      setPhaseTransition(`Round ${localGameState.roundNumber} starting!`);
      setPreviousRound(localGameState.roundNumber);

      // Clear transition message after 3 seconds
      setTimeout(() => setPhaseTransition(null), 3000);
    }

    // Update client time remaining when game state changes
    setClientTimeRemaining(localGameState.timeRemaining);
    setLastServerUpdate(Date.now());

    // Emit header update event instead of direct DOM manipulation
    emitHeaderUpdate(localGameState);
  }, [localGameState, previousRound]);

  // Detect phase transitions separately to avoid dependency issues
  useEffect(() => {
    const currentPhase = localGameState.phase;
    let transitionMessage = "";

    switch (currentPhase) {
      case "playing":
        transitionMessage = "Game started!";
        break;
      case "results":
        transitionMessage = "Round complete!";
        break;
      case "waiting":
        transitionMessage = "Waiting for next round...";
        break;
    }

    if (transitionMessage && currentPhase !== "waiting") {
      setPhaseTransition(transitionMessage);
      setTimeout(() => setPhaseTransition(null), 2500);
    }
  }, [localGameState.phase]);

  // Client-side timer countdown for smooth updates
  useEffect(() => {
    if (localGameState.phase !== "playing") {
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
        playNotificationSound("warning");
        hasPlayedWarning = true;
      } else if (seconds === 10 && !hasPlayedCritical) {
        playNotificationSound("critical");
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
    let ws: WebSocket | null = null;

    const connectWebSocket = () => {
      try {
        connectionStatus.value = "connecting";
        const protocol = globalThis.location.protocol === "https:" ? "wss:" : "ws:";
        const wsUrl = `${protocol}//${globalThis.location.host}/api/websocket?roomId=${roomId}`;

        ws = new WebSocket(wsUrl);

        ws.onopen = () => {
          connectionStatus.value = "connected";

          // Store WebSocket globally for game control messages
          (globalThis as any).__gameWebSocket = ws;

          // Get player name from game state
          const currentPlayer = localGameState.players.find((p) => p.id === playerId);
          let playerName = currentPlayer?.name;

          // If player not found in initial game state, try to get from session storage or URL
          if (!playerName && playerId) {
            const urlParams = new URLSearchParams(globalThis.location.search);
            playerName = urlParams.get("playerName") ||
              (globalThis.sessionStorage?.getItem(`playerName_${playerId}`)) || undefined;
          }

          // Only send join-room message if we have a valid player name and playerId
          if (playerName && playerName !== "Unknown" && playerId) {
            ws?.send(JSON.stringify({
              type: "join-room",
              roomId,
              playerId,
              data: { playerName },
            }));
          } else {
            console.warn(
              `Scoreboard: Cannot join room ${roomId} - playerId: ${playerId}, playerName: ${playerName}`,
            );
          }
        };

        ws.onmessage = (event) => {
          try {
            const message = JSON.parse(event.data);

            if (message.type === "game-state") {
              const updatedGameState = message.data;

              // Handle different game state message types
              if (updatedGameState && typeof updatedGameState === "object") {
                if (updatedGameState.type === "game-started" && updatedGameState.gameState) {
                  // Game started successfully
                  setLocalGameState(updatedGameState.gameState);
                  setIsStartingGame(false);
                  setPhaseTransition("Game started!");
                  setTimeout(() => setPhaseTransition(null), 2000);

                  if (onGameStateUpdate) {
                    onGameStateUpdate(updatedGameState.gameState);
                  }

                  // Emit custom event for other components
                  globalThis.dispatchEvent(
                    new CustomEvent("gameStateUpdate", {
                      detail: { gameState: updatedGameState.gameState },
                    }),
                  );
                } else if (updatedGameState.type === "game-start-success") {
                  // Host received confirmation
                  setIsStartingGame(false);
                  setPhaseTransition("Game started successfully!");
                  setTimeout(() => setPhaseTransition(null), 2000);

                  // Update game state if provided
                  if (updatedGameState.gameState) {
                    setLocalGameState(updatedGameState.gameState);
                    if (onGameStateUpdate) {
                      onGameStateUpdate(updatedGameState.gameState);
                    }

                    // Emit custom event for other components
                    globalThis.dispatchEvent(
                      new CustomEvent("gameStateUpdate", {
                        detail: { gameState: updatedGameState.gameState },
                      }),
                    );
                  }
                } else if (updatedGameState.players) {
                  // Regular game state update - this handles all player changes

                  // Show transition messages for player changes
                  if (
                    updatedGameState.updateType === "player-joined" && updatedGameState.joinedPlayer
                  ) {
                    setPhaseTransition(
                      `${updatedGameState.joinedPlayer.playerName} joined the room!`,
                    );
                    setTimeout(() => setPhaseTransition(null), 2000);
                  } else if (
                    updatedGameState.updateType === "player-left" && updatedGameState.leftPlayer
                  ) {
                    const leftPlayer = updatedGameState.leftPlayer;
                    if (leftPlayer.wasHost && updatedGameState.hostMigration) {
                      setPhaseTransition(
                        `${leftPlayer.playerName} left. ${updatedGameState.hostMigration.newHostName} is now the host.`,
                      );
                    } else {
                      setPhaseTransition(`${leftPlayer.playerName} left the room`);
                    }
                    setTimeout(() => setPhaseTransition(null), 2000);
                  }

                  // Validate the game state before updating
                  if (!updatedGameState.players || !Array.isArray(updatedGameState.players)) {
                    console.error(
                      "Scoreboard: Invalid game state received - players is not an array",
                    );
                    return;
                  }

                  // Check if current player is missing from the game state
                  const currentPlayerExists = updatedGameState.players.some((p: any) =>
                    p.id === playerId
                  );
                  if (!currentPlayerExists && playerId) {
                    console.warn(
                      `Scoreboard: Current player ${playerId} missing from game state, requesting refresh`,
                    );
                    // Send another join-room message to ensure we're properly synced
                    const currentPlayer = localGameState.players.find((p) => p.id === playerId);
                    const playerName = currentPlayer?.name;
                    const globalWs = (globalThis as any).__gameWebSocket;
                    if (playerName && globalWs?.readyState === WebSocket.OPEN) {
                      setTimeout(() => {
                        globalWs.send(JSON.stringify({
                          type: "join-room",
                          roomId,
                          playerId,
                          data: { playerName },
                        }));
                      }, 1000);
                    }
                  }

                  // Create a new state object to ensure React detects the change
                  const newGameState = {
                    ...updatedGameState,
                    players: [...updatedGameState.players],
                    scores: { ...updatedGameState.scores },
                  };

                  setLocalGameState(newGameState);

                  if (onGameStateUpdate) {
                    onGameStateUpdate(newGameState);
                  }

                  // Emit header update event
                  emitHeaderUpdate(newGameState);

                  // Emit custom event for other components
                  globalThis.dispatchEvent(
                    new CustomEvent("gameStateUpdate", {
                      detail: { gameState: newGameState },
                    }),
                  );
                }
              }
            } else if (message.type === "room-update") {
              // Handle room updates (pre-game join/leave, host change)
              if (message.data) {
                const updateData = message.data;

                // Emit custom event for other components
                globalThis.dispatchEvent(
                  new CustomEvent("roomUpdate", {
                    detail: { updateData: { ...updateData, roomId } },
                  }),
                );

                // Update local game state when server sends room membership data
                // Two possible shapes:
                // 1) Room summary (from join): { room, players, playerCount, canJoin, host }
                // 2) Player left (from leave): { type: 'player-left', playerId, newHostId, remainingPlayers }
                const summaryPlayers = Array.isArray(updateData?.players)
                  ? updateData.players
                  : null;
                const remainingPlayers = Array.isArray(updateData?.remainingPlayers)
                  ? updateData.remainingPlayers
                  : null;

                let updatedPlayers: PlayerState[] | null = null;
                if (summaryPlayers) {
                  updatedPlayers = summaryPlayers.map((p: any) => ({
                    id: p.id,
                    name: p.name,
                    isHost: !!p.isHost,
                    isConnected: true,
                    lastActivity: Date.now(),
                  }));
                } else if (remainingPlayers) {
                  updatedPlayers = remainingPlayers.map((p: any) => ({
                    id: p.id,
                    name: p.name,
                    isHost: !!p.isHost,
                    isConnected: true,
                    lastActivity: Date.now(),
                  }));
                }

                // Apply host change if provided only as IDs
                const newHostId: string | undefined = updateData?.host?.id || updateData?.newHostId;
                if (updatedPlayers && newHostId) {
                  updatedPlayers = updatedPlayers.map((p) => ({
                    ...p,
                    isHost: p.id === newHostId,
                  }));
                }

                if (updatedPlayers) {
                  // Merge scores: keep existing where possible, default to 0 for new players
                  const newScores: Record<string, number> = { ...localGameState.scores };
                  const validIds = new Set(updatedPlayers.map((p) => p.id));
                  for (const id of Object.keys(newScores)) {
                    if (!validIds.has(id)) delete newScores[id];
                  }
                  for (const p of updatedPlayers) {
                    if (newScores[p.id] === undefined) newScores[p.id] = 0;
                  }

                  const newGameState = {
                    ...localGameState,
                    players: updatedPlayers,
                    scores: newScores,
                  } as BaseGameState;

                  setLocalGameState(newGameState);
                  onGameStateUpdate?.(newGameState);
                  emitHeaderUpdate(newGameState);
                  globalThis.dispatchEvent(
                    new CustomEvent("gameStateUpdate", { detail: { gameState: newGameState } }),
                  );
                }
              }
            } else if (message.type === "chat-message") {
              // Handle chat messages and forward to ChatRoom component
              globalThis.dispatchEvent(
                new CustomEvent("websocket-message", {
                  detail: { data: message },
                }),
              );
            } else if (message.type === "draw-update") {
              // Forward drawing updates for DrawingBoard to consume
              globalThis.dispatchEvent(
                new CustomEvent("websocket-message", {
                  detail: { data: message },
                }),
              );
            }
          } catch (error) {
            console.error("Scoreboard: Error parsing message:", error);
          }
        };

        ws.onclose = () => {
          connectionStatus.value = "disconnected";
          // Clear global WebSocket reference
          (globalThis as any).__gameWebSocket = null;
        };

        ws.onerror = (error) => {
          console.error("Scoreboard: WebSocket error:", error);
          connectionStatus.value = "disconnected";
          // Clear global WebSocket reference
          (globalThis as any).__gameWebSocket = null;
        };
      } catch (error) {
        console.error("Scoreboard: Failed to create WebSocket:", error);
        connectionStatus.value = "disconnected";
      }
    };

    connectWebSocket();

    return () => {
      if (ws) {
        ws.close();
      }
      // Clear global WebSocket reference
      (globalThis as any).__gameWebSocket = null;
    };
  }, [roomId, playerId, onGameStateUpdate]);

  // Format time remaining as MM:SS
  const formatTime = (milliseconds: number): string => {
    const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000));
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds.toString().padStart(2, "0")}`;
  };

  // Get timer warning state
  const getTimerWarningState = (timeRemaining: number): "normal" | "warning" | "critical" => {
    const seconds = Math.floor(timeRemaining / 1000);
    if (seconds <= 10) return "critical";
    if (seconds <= 30) return "warning";
    return "normal";
  };

  // Get timer color classes
  const _getTimerColorClasses = (warningState: "normal" | "warning" | "critical"): string => {
    switch (warningState) {
      case "critical":
        return "text-red-600 bg-red-100 border-red-300 animate-pulse";
      case "warning":
        return "text-orange-600 bg-orange-100 border-orange-300";
      case "normal":
        return "text-blue-600 bg-blue-100 border-blue-300";
    }
  };

  // Calculate round progress percentage
  const getRoundProgress = (): number => {
    const settings = localGameState.settings || { maxRounds: 5, roundTimeSeconds: 75 };
    if (!settings.maxRounds) return 0;
    return Math.min((localGameState.roundNumber / settings.maxRounds) * 100, 100);
  };

  // Play notification sound (optional)
  const playNotificationSound = (type: "warning" | "critical") => {
    try {
      // Create a simple beep sound using Web Audio API
      const audioContext = new (globalThis.AudioContext || (window as any).webkitAudioContext)();
      const oscillator = audioContext.createOscillator();
      const gainNode = audioContext.createGain();

      oscillator.connect(gainNode);
      gainNode.connect(audioContext.destination);

      oscillator.frequency.value = type === "critical" ? 800 : 600;
      oscillator.type = "sine";

      gainNode.gain.setValueAtTime(0.1, audioContext.currentTime);
      gainNode.gain.exponentialRampToValueAtTime(0.01, audioContext.currentTime + 0.2);

      oscillator.start(audioContext.currentTime);
      oscillator.stop(audioContext.currentTime + 0.2);
    } catch (_error) {
      // Silently fail if audio is not supported
    }
  };

  // Get sorted players by score - memoized for performance
  const sortedPlayers = useMemo((): (PlayerState & { score: number })[] => {
    return localGameState.players
      .map((player) => ({
        ...player,
        score: localGameState.scores[player.id] || 0,
      }))
      .sort((a, b) => b.score - a.score);
  }, [localGameState.players, localGameState.scores]);

  // Get current drawer info (for drawing game)
  const currentDrawer = useMemo((): PlayerState | null => {
    const drawingState = localGameState as DrawingGameState;
    if (drawingState.gameData?.currentDrawer) {
      return localGameState.players.find((p) => p.id === drawingState.gameData.currentDrawer) ||
        null;
    }
    return null;
  }, [localGameState.players, localGameState]);

  // Get phase display text
  const getPhaseText = (): string => {
    switch (localGameState.phase) {
      case "waiting":
        return "Waiting for game to start";
      case "playing":
        return "Game in progress";
      case "results":
        return "Round complete";
      default:
        return "Unknown phase";
    }
  };

  // Get phase color
  const getPhaseColor = (): string => {
    switch (localGameState.phase) {
      case "waiting":
        return "text-yellow-200 bg-yellow-400/20 border border-yellow-400/30";
      case "playing":
        return "text-blue-200 bg-blue-400/20 border border-blue-400/30";
      case "results":
        return "text-purple-200 bg-purple-400/20 border border-purple-400/30";
      default:
        return "text-white/70 bg-white/20 border border-white/30";
    }
  };

  // Check if current player is host
  const isHost = localGameState.players.find((p) => p.id === playerId)?.isHost || false;

  // Send WebSocket message
  const sendGameControlMessage = (
    type: "start-game" | "next-round" | "end-game",
    data: any = {},
  ) => {
    // Find the current WebSocket connection from the useEffect
    const _connections = document.querySelectorAll("script[data-ws-connection]");
    let ws: WebSocket | null = null;

    // Try to get the WebSocket from the connection status
    if (connectionStatus.value === "connected") {
      // Use a more direct approach - store the WebSocket in a global variable
      ws = (globalThis as any).__gameWebSocket;
    }

    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({
        type,
        roomId,
        playerId,
        data,
      }));
      return true;
    } else {
      console.warn(`WebSocket not available for ${type} message, readyState:`, ws?.readyState);
      return false;
    }
  };

  // Handle start game
  const handleStartGame = async () => {
    if (isStartingGame) return; // Prevent double-clicks

    setIsStartingGame(true);
    setPhaseTransition("Starting game...");

    try {
      // Try WebSocket first
      if (connectionStatus.value === "connected") {
        const success = sendGameControlMessage("start-game");
        if (success) {
          // WebSocket response will be handled in the message handler
          // Set a timeout to reset the starting state if no response comes
          setTimeout(() => {
            if (isStartingGame) {
              console.warn("No WebSocket response received, falling back to REST API");
              setIsStartingGame(false);
              setPhaseTransition("WebSocket timeout, trying REST API...");
              // Try REST API as fallback
              handleStartGameREST();
            }
          }, 5000);
          return;
        }
      }

      // Fallback to REST API
      await handleStartGameREST();
    } catch (error) {
      console.error("Error starting game:", error);
      setPhaseTransition("Error: Failed to start game");
      setTimeout(() => setPhaseTransition(null), 5000);
      setIsStartingGame(false);
    }
  };

  // REST API fallback for starting game
  const handleStartGameREST = async () => {
    try {
      const response = await fetch("/api/game/start", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          roomId,
          playerId,
        }),
      });

      const result = await response.json();

      if (result.success && result.gameState) {
        // Update local game state
        setLocalGameState(result.gameState);

        if (onGameStateUpdate) {
          onGameStateUpdate(result.gameState);
        }

        // Show success message
        setPhaseTransition("Game started successfully!");
        setTimeout(() => setPhaseTransition(null), 3000);
      } else {
        // Show error message
        setPhaseTransition(`Error: ${result.error || "Failed to start game"}`);
        setTimeout(() => setPhaseTransition(null), 5000);
      }
    } catch (error) {
      console.error("Error starting game via REST:", error);
      setPhaseTransition("Error: Failed to start game");
      setTimeout(() => setPhaseTransition(null), 5000);
    } finally {
      setIsStartingGame(false);
    }
  };

  // Handle next round
  const handleNextRound = () => {
    sendGameControlMessage("next-round");
  };

  // Handle end game
  const handleEndGame = () => {
    sendGameControlMessage("end-game");
  };


  return (
    <div className={`${className} flex flex-col h-full`}>
      {/* Main content area that grows to fill available space */}
      <div className="flex-1 min-h-0">
        {/* Phase Transition Notification */}
        {phaseTransition && (
          <div className="mb-4 p-3 bg-gradient-to-r from-blue-500 to-purple-600 text-white rounded-lg text-center font-medium phase-transition backdrop-blur-sm">
            {phaseTransition}
          </div>
        )}

        {/* Connection Status */}
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold text-white">Scoreboard</h2>
          <div className="flex items-center space-x-2">
            <div
              className={`w-2 h-2 rounded-full ${
                connectionStatus.value === "connected"
                  ? "bg-green-400"
                  : connectionStatus.value === "connecting"
                  ? "bg-yellow-400"
                  : "bg-red-400"
              }`}
            >
            </div>
            <span className="text-xs text-white/70 capitalize">
              {connectionStatus.value}
            </span>
          </div>
        </div>

        {/* Game Status */}
        <div className="mb-4">
          <div
            className={`inline-block px-3 py-1 rounded-full text-sm font-medium ${getPhaseColor()}`}
          >
            {getPhaseText()}
          </div>

          {/* Enhanced Timer Display */}
          {localGameState.phase === "playing" && (
            <div className="mt-3">
              <div
                className={`inline-flex items-center px-4 py-2 rounded-lg border-2 border-white/30 font-mono text-lg font-bold text-white bg-white/10 backdrop-blur-sm ${
                  getTimerWarningState(clientTimeRemaining) === "critical" ? "timer-pulse" : ""
                }`}
              >
                <svg className="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
                  />
                </svg>
                {formatTime(clientTimeRemaining)}
              </div>
              {getTimerWarningState(clientTimeRemaining) === "critical" && (
                <div className="text-xs text-red-300 mt-1 font-medium animate-pulse">
                  Time's almost up!
                </div>
              )}
              {getTimerWarningState(clientTimeRemaining) === "warning" && (
                <div className="text-xs text-orange-300 mt-1 font-medium">
                  Hurry up!
                </div>
              )}

              {/* Timer Progress Bar */}
              <div className="mt-2 w-full bg-white/20 rounded-full h-1">
                <div
                  className={`h-1 rounded-full transition-all duration-100 ${
                    getTimerWarningState(clientTimeRemaining) === "critical"
                      ? "bg-red-400"
                      : getTimerWarningState(clientTimeRemaining) === "warning"
                      ? "bg-orange-400"
                      : "bg-blue-400"
                  }`}
                  style={{
                    width: `${
                      Math.max(
                        0,
                        (clientTimeRemaining /
                          ((localGameState.settings?.roundTimeSeconds || 75) * 1000)) * 100,
                      )
                    }%`,
                  }}
                >
                </div>
              </div>
            </div>
          )}

          {/* Round Progress */}
          <div className="mt-3">
            <div className="flex items-center justify-between text-sm text-white/70 mb-1">
              <span>Round Progress</span>
              <span>{localGameState.roundNumber} / {(localGameState.settings?.maxRounds || 5)}</span>
            </div>
            <div className="w-full bg-white/20 rounded-full h-2">
              <div
                className="bg-blue-400 h-2 rounded-full round-progress-fill"
                style={{ width: `${getRoundProgress()}%` }}
              >
              </div>
            </div>
            {localGameState.roundNumber === (localGameState.settings?.maxRounds || 5) && (
              <div className="text-xs text-orange-300 mt-1 font-medium">
                Final round!
              </div>
            )}
          </div>
        </div>

        {/* Current Drawer */}
        {currentDrawer && localGameState.phase === "playing" && (
          <div className="mb-4 p-2 bg-blue-400/20 rounded-lg border border-blue-400/30 backdrop-blur-sm">
            <div className="text-sm font-medium text-blue-200">
              {currentDrawer.name} is drawing
            </div>
          </div>
        )}

        {/* Player Scores */}
        <div key={`players-container-${sortedPlayers.length}`} className="space-y-2 mb-4">
          <h3 className="text-sm font-medium text-white/90">Players ({sortedPlayers.length})</h3>

          {sortedPlayers.map((player, index) => (
            <div
              key={`player-${player.id}-${player.name}`}
              className="flex items-center justify-between p-2 bg-white/10 backdrop-blur-sm rounded border border-white/20"
            >
              <div className="flex items-center space-x-2">
                <span className="text-sm font-medium text-white/70">#{index + 1}</span>
                <span className="text-sm font-medium text-white">{player.name}</span>
                {player.isHost && (
                  <span className="text-xs bg-yellow-400/20 text-yellow-200 px-2 py-1 rounded border border-yellow-400/30">
                    Host
                  </span>
                )}
                {player.id === (localGameState as DrawingGameState).gameData?.currentDrawer && (
                  <span className="text-xs bg-blue-400/20 text-blue-200 px-2 py-1 rounded border border-blue-400/30">
                    Drawing
                  </span>
                )}
                <div
                  className={`w-2 h-2 rounded-full bg-green-400`}
                >
                </div>
              </div>
              <span className="text-sm font-semibold text-white">{player.score} pts</span>
            </div>
          ))}
        </div>
      </div>

      {/* Host Controls - positioned at bottom */}
      {isHost && (
        <div className="border-t border-white/20 pt-4 mt-4">
          <h3 className="text-sm font-medium text-white/90 mb-3">Host Controls</h3>

          {/* Start Game Button */}
          {localGameState.phase === "waiting" && (
            <button
              type="button"
              onClick={handleStartGame}
              className="w-full bg-green-600 hover:bg-green-700 disabled:bg-gray-400 disabled:cursor-not-allowed text-white font-medium py-2 px-4 rounded-lg transition-colors duration-200 mb-2 flex items-center justify-center"
              disabled={sortedPlayers.length < 2 || isStartingGame}
            >
              {isStartingGame
                ? (
                  <>
                    <svg
                      className="animate-spin -ml-1 mr-2 h-4 w-4 text-white"
                      xmlns="http://www.w3.org/2000/svg"
                      fill="none"
                      viewBox="0 0 24 24"
                    >
                      <circle
                        className="opacity-25"
                        cx="12"
                        cy="12"
                        r="10"
                        stroke="currentColor"
                        strokeWidth="4"
                      >
                      </circle>
                      <path
                        className="opacity-75"
                        fill="currentColor"
                        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                      >
                      </path>
                    </svg>
                    Starting...
                  </>
                )
                : (
                  "Start Game"
                )}
            </button>
          )}

          {/* End Round Button (during drawing phase) */}
          {localGameState.phase === "playing" && (
            <button
              type="button"
              onClick={handleNextRound}
              className="w-full bg-orange-600 hover:bg-orange-700 text-white font-medium py-2 px-4 rounded-lg transition-colors duration-200 mb-2"
            >
              End Round Early
            </button>
          )}

          {/* Next Round Button (during results phase) */}
          {localGameState.phase === "results" && (
            <button
              type="button"
              onClick={handleNextRound}
              className="w-full bg-blue-600 hover:bg-blue-700 text-white font-medium py-2 px-4 rounded-lg transition-colors duration-200 mb-2"
            >
              Start Next Round
            </button>
          )}

          {/* End Game Button */}
          {(localGameState.phase === "playing" || localGameState.phase === "results") && (
            <button
              type="button"
              onClick={handleEndGame}
              className="w-full bg-red-600 hover:bg-red-700 text-white font-medium py-2 px-4 rounded-lg transition-colors duration-200"
            >
              End Game
            </button>
          )}

          {/* Game Settings Button */}
          {localGameState.phase === "waiting" && (
            <button
              type="button"
              onClick={() => onShowSettingsModal?.()}
              className="w-full bg-gray-600 hover:bg-gray-700 text-white font-medium py-2 px-4 rounded-lg transition-colors duration-200 mb-2"
            >
              Game Settings
            </button>
          )}

          {/* Game Settings Info */}
          <div className="mt-3 text-xs text-white/60">
            <div>Max Rounds: {(localGameState.settings?.maxRounds || 5)}</div>
            <div>
              Round Time:{" "}
              {Math.floor((localGameState.settings?.roundTimeSeconds || 75) / 60)}:{((localGameState.settings?.roundTimeSeconds || 75) % 60)
                .toString().padStart(
                  2,
                  "0",
                )}
            </div>
          </div>
        </div>
      )}

    </div>
  );
}
