import { useCallback, useEffect, useRef, useState } from "preact/hooks";
import { ClientMessage, DrawingCommand, GameState, ServerMessage } from "../types/game.ts";
import DrawingEngine, { DrawingEngineRef, DrawingTool } from "./DrawingEngine.tsx";

export interface DrawingBoardProps {
  roomId: string;
  playerId: string;
  gameState: GameState;
  onGameStateUpdate?: (gameState: GameState) => void;
  width?: number;
  height?: number;
  className?: string;
  responsive?: boolean;
}

export default function DrawingBoard({
  roomId,
  playerId,
  gameState,
  onGameStateUpdate,
  width = 800,
  height = 600,
  className = "",
  responsive = true,
}: DrawingBoardProps) {
  const drawingEngineRef = useRef<DrawingEngineRef>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const [connectionStatus, setConnectionStatus] = useState<
    "connecting" | "connected" | "disconnected"
  >("disconnected");
  const [drawingHistory, setDrawingHistory] = useState<DrawingCommand[]>([]);
  const [isDrawer, setIsDrawer] = useState(false);
  const [currentTool, setCurrentTool] = useState<DrawingTool>({
    color: "#000000",
    size: 5,
    type: "brush",
  });

  // Determine if current player is the drawer
  useEffect(() => {
    const isCurrentDrawer = gameState.currentDrawer === playerId;
    setIsDrawer(isCurrentDrawer);
  }, [gameState.currentDrawer, playerId]);

  // Initialize WebSocket connection
  useEffect(() => {
    if (!roomId || !playerId) return;

    const connectWebSocket = () => {
      try {
        setConnectionStatus("connecting");

        // Construct WebSocket URL
        const protocol = globalThis.location.protocol === "https:" ? "wss:" : "ws:";
        const wsUrl =
          `${protocol}//${globalThis.location.host}/ws?roomId=${roomId}&playerId=${playerId}`;

        const ws = new WebSocket(wsUrl);
        wsRef.current = ws;

        ws.onopen = () => {
          console.log("DrawingBoard WebSocket connected");
          setConnectionStatus("connected");

          // Subscribe to drawing updates
          const subscribeMessage: ClientMessage = {
            type: "join-room",
            roomId,
            playerId,
            data: { subscribeToDrawing: true },
          };
          ws.send(JSON.stringify(subscribeMessage));
        };

        ws.onmessage = (event) => {
          try {
            const message: ServerMessage = JSON.parse(event.data);
            handleWebSocketMessage(message);
          } catch (error) {
            console.error("Error parsing WebSocket message:", error);
          }
        };

        ws.onclose = (event) => {
          console.log("DrawingBoard WebSocket disconnected:", event.code, event.reason);
          setConnectionStatus("disconnected");
          wsRef.current = null;

          // Attempt to reconnect after a delay
          if (!event.wasClean) {
            setTimeout(connectWebSocket, 2000);
          }
        };

        ws.onerror = (error) => {
          console.error("DrawingBoard WebSocket error:", error);
          setConnectionStatus("disconnected");
        };
      } catch (error) {
        console.error("Error creating WebSocket connection:", error);
        setConnectionStatus("disconnected");
      }
    };

    connectWebSocket();

    return () => {
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [roomId, playerId]);

  // Handle incoming WebSocket messages
  const handleWebSocketMessage = useCallback((message: ServerMessage) => {
    switch (message.type) {
      case "draw-update":
        handleDrawingUpdate(message.data);
        break;
      case "game-state":
        if (onGameStateUpdate) {
          onGameStateUpdate(message.data);
        }
        break;
      case "room-update":
        // Handle room updates if needed
        break;
      default:
        console.log("Unhandled WebSocket message type:", message.type);
    }
  }, [onGameStateUpdate]);

  // Handle drawing updates from other players
  const handleDrawingUpdate = useCallback(
    (data: { command?: DrawingCommand; commands?: DrawingCommand[] }) => {
      if (data.command) {
        // Single command update
        const command = data.command;

        // Apply command to drawing engine
        if (drawingEngineRef.current) {
          drawingEngineRef.current.applyDrawingCommand(command);
        }

        // Update local drawing history
        setDrawingHistory((prev) => [...prev, command]);
      } else if (data.commands) {
        // Batch command update
        data.commands.forEach((command) => {
          if (drawingEngineRef.current) {
            drawingEngineRef.current.applyDrawingCommand(command);
          }
        });

        // Update local drawing history
        setDrawingHistory((prev) => [...prev, ...(data.commands || [])]);
      }
    },
    [],
  );

  // Send drawing command to other players
  const handleDrawingCommand = useCallback((command: DrawingCommand) => {
    if (!wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) {
      console.warn("WebSocket not connected, cannot send drawing command");
      return;
    }

    // Add to local history
    setDrawingHistory((prev) => [...prev, command]);

    // Send to server
    const message: ClientMessage = {
      type: "draw",
      roomId,
      playerId,
      data: { command },
    };

    try {
      wsRef.current.send(JSON.stringify(message));
    } catch (error) {
      console.error("Error sending drawing command:", error);
    }
  }, [roomId, playerId]);

  // Send batch of drawing commands (for optimization)
  const handleDrawingCommands = useCallback((commands: DrawingCommand[]) => {
    if (!wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) {
      console.warn("WebSocket not connected, cannot send drawing commands");
      return;
    }

    // Add to local history
    setDrawingHistory((prev) => [...prev, ...commands]);

    // Send batch to server
    const message: ClientMessage = {
      type: "draw",
      roomId,
      playerId,
      data: { commands },
    };

    try {
      wsRef.current.send(JSON.stringify(message));
    } catch (error) {
      console.error("Error sending drawing commands:", error);
    }
  }, [roomId, playerId]);

  // Clear canvas for all players
  const handleClearCanvas = useCallback(() => {
    if (!isDrawer) return;

    const clearCommand: DrawingCommand = {
      type: "clear",
      timestamp: Date.now(),
    };

    handleDrawingCommand(clearCommand);

    // Clear local canvas
    if (drawingEngineRef.current) {
      drawingEngineRef.current.clearCanvas();
    }

    // Reset drawing history
    setDrawingHistory([clearCommand]);
  }, [isDrawer, handleDrawingCommand]);

  // Undo last drawing action
  const handleUndo = useCallback(() => {
    if (!isDrawer) return;

    if (drawingEngineRef.current) {
      drawingEngineRef.current.undo();
    }
  }, [isDrawer]);

  // Connection status indicator
  const getConnectionStatusColor = () => {
    switch (connectionStatus) {
      case "connected":
        return "text-green-600";
      case "connecting":
        return "text-yellow-600";
      case "disconnected":
        return "text-red-600";
      default:
        return "text-gray-600";
    }
  };

  const getConnectionStatusText = () => {
    switch (connectionStatus) {
      case "connected":
        return "Connected";
      case "connecting":
        return "Connecting...";
      case "disconnected":
        return "Disconnected";
      default:
        return "Unknown";
    }
  };

  // Determine if drawing should be disabled
  const isDrawingDisabled = !isDrawer || gameState.phase !== "drawing" ||
    connectionStatus !== "connected";

  return (
    <div class={`drawing-board ${className}`}>
      {/* Header with status and controls - Mobile responsive */}
      <div class="drawing-board-header mb-4 p-3 bg-gray-50 rounded-lg border">
        <div class="flex flex-col sm:flex-row sm:justify-between sm:items-center gap-2 sm:gap-4">
          <div class="flex items-center gap-2 sm:gap-4">
            <h3 class="text-lg font-semibold">Drawing Board</h3>
            <div class={`text-sm ${getConnectionStatusColor()}`}>
              <span class="inline-block w-2 h-2 rounded-full bg-current mr-2"></span>
              {getConnectionStatusText()}
            </div>
          </div>

          <div class="flex items-center gap-2 text-sm">
            {gameState.phase === "drawing" && (
              <>
                <span class="text-gray-600 hidden sm:inline">Current drawer:</span>
                <span class="text-gray-600 sm:hidden">Drawer:</span>
                <span class="font-medium">
                  {gameState.currentDrawer === playerId
                    ? "You"
                    : gameState.players.find((p) => p.id === gameState.currentDrawer)?.name ||
                      "Unknown"}
                </span>
              </>
            )}
          </div>
        </div>
      </div>

      {/* Drawing Engine Component */}
      <DrawingEngine
        ref={drawingEngineRef}
        isDrawer={isDrawer}
        onDrawingCommand={handleDrawingCommand}
        onDrawingCommands={handleDrawingCommands}
        width={responsive
          ? Math.min(width, (typeof window !== "undefined" ? globalThis.innerWidth : 1280) - 40)
          : width}
        height={responsive
          ? Math.min(height, (typeof window !== "undefined" ? globalThis.innerHeight : 720) * 0.6)
          : height}
        disabled={isDrawingDisabled}
      />

      {/* Additional Controls - Mobile responsive */}
      {isDrawer && gameState.phase === "drawing" && (
        <div class="drawing-board-controls mt-4 p-3 bg-blue-50 rounded-lg border">
          <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 sm:gap-0">
            <div class="text-sm text-blue-800 text-center sm:text-left">
              <strong>Your turn to draw!</strong>
              <span class="hidden sm:inline">Others are trying to guess your drawing.</span>
              <div class="sm:hidden text-xs mt-1">Others are guessing your drawing.</div>
            </div>
            <div class="flex gap-2 justify-center sm:justify-end">
              <button
                onClick={handleClearCanvas}
                class="flex-1 sm:flex-none px-4 py-2 sm:px-3 sm:py-1 bg-red-500 text-white rounded hover:bg-red-600 text-sm font-medium touch-manipulation"
                disabled={connectionStatus !== "connected"}
              >
                Clear All
              </button>
              <button
                onClick={handleUndo}
                class="flex-1 sm:flex-none px-4 py-2 sm:px-3 sm:py-1 bg-gray-500 text-white rounded hover:bg-gray-600 text-sm font-medium touch-manipulation"
                disabled={connectionStatus !== "connected"}
              >
                Undo
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Status Messages */}
      {!isDrawer && gameState.phase === "drawing" && (
        <div class="drawing-board-status mt-4 p-3 bg-gray-50 rounded-lg border">
          <div class="text-sm text-gray-700">
            <strong>Watching:</strong> {gameState.players.find((p) =>
              p.id === gameState.currentDrawer
            )?.name || "Someone"} is drawing. Try to guess what they're drawing in the chat!
          </div>
        </div>
      )}

      {gameState.phase === "waiting" && (
        <div class="drawing-board-status mt-4 p-3 bg-yellow-50 rounded-lg border">
          <div class="text-sm text-yellow-800">
            <strong>Waiting:</strong> Game hasn't started yet or waiting for next round.
          </div>
        </div>
      )}

      {gameState.phase === "results" && (
        <div class="drawing-board-status mt-4 p-3 bg-green-50 rounded-lg border">
          <div class="text-sm text-green-800">
            <strong>Round Complete:</strong>{" "}
            The word was "{gameState.currentWord}". Check the scoreboard for results!
          </div>
        </div>
      )}

      {/* Debug Info (only in development) */}
      {false && ( // Debug info disabled in production
        <div class="drawing-board-debug mt-4 p-2 bg-gray-100 rounded text-xs text-gray-600">
          <div>Room: {roomId}</div>
          <div>Player: {playerId}</div>
          <div>Is Drawer: {isDrawer ? "Yes" : "No"}</div>
          <div>Phase: {gameState.phase}</div>
          <div>Drawing History: {drawingHistory.length} commands</div>
          <div>Connection: {connectionStatus}</div>
        </div>
      )}
    </div>
  );
}
