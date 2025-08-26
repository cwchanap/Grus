// Drawing game specific drawing board component (PixiJS wrapper)
import { useCallback, useEffect, useRef, useState } from "preact/hooks";
import { DrawingCommand } from "../../../types/games/drawing.ts";
import DrawingEngine, { DrawingEngineRef } from "./DrawingEngine.tsx";

interface DrawingBoardProps {
  width: number;
  height: number;
  onDrawCommand?: (command: DrawingCommand) => void;
  drawingData: DrawingCommand[];
  isDrawer: boolean;
  disabled?: boolean;
  playerId: string;
  roomId?: string;
}

export default function DrawingBoard({
  width,
  height,
  onDrawCommand,
  drawingData,
  isDrawer,
  disabled = false,
  playerId,
  roomId,
}: DrawingBoardProps) {
  const engineRef = useRef<DrawingEngineRef | null>(null);
  
  // Create a stable callback that always uses the latest onDrawCommand value.
  // If onDrawCommand is not provided (e.g., cannot be serialized from SSR),
  // fall back to sending over the global WebSocket managed by Scoreboard.
  const stableOnDrawCommand = useCallback((command: DrawingCommand) => {
    if (onDrawCommand) {
      onDrawCommand(command);
      return;
    }

    const send = () => {
      try {
        const ws = (globalThis as any).__gameWebSocket as WebSocket | undefined;
        if (ws && ws.readyState === WebSocket.OPEN) {
          const message = {
            type: "draw",
            roomId,
            playerId,
            data: command,
          };
          ws.send(JSON.stringify(message));
          return true;
        } else {
          return false;
        }
      } catch (err) {
        return false;
      }
    };

    // Immediate attempt + simple retries to handle short races on WS readiness
    if (!send()) {
      setTimeout(() => {
        if (!send()) {
          setTimeout(() => {
            if (!send()) {
              console.error("DrawingBoard: failed to send draw command after retries", command);
            }
          }, 500);
        }
      }, 100);
    }
  }, [onDrawCommand, roomId, playerId]); // Re-create when dependencies change

  // Live state driven by server game-state updates
  const [liveDisabled, setLiveDisabled] = useState<boolean>(disabled);
  const [liveIsDrawer, setLiveIsDrawer] = useState<boolean>(isDrawer);

  // Replay initial drawing data when engine becomes ready or data changes
  useEffect(() => {
    const ref = engineRef.current;
    if (!ref || !drawingData?.length) return;
    // Clear before replay to avoid duplicates
    ref.applyDrawingCommand({ type: "clear", timestamp: Date.now() });
    for (const cmd of drawingData) {
      ref.applyDrawingCommand(cmd);
    }
  }, [engineRef.current, drawingData]);

  // Listen for global game state updates dispatched by Scoreboard
  useEffect(() => {
    const handler = (evt: Event) => {
      const anyEvt = evt as CustomEvent;
      const gs = anyEvt.detail?.gameState as any;
      if (!gs || typeof gs !== "object") return;

      // Enable canvas when game is not in waiting phase
      setLiveDisabled(gs.phase === "waiting");

      // Determine if current player is the drawer (for drawing game)
      const currentDrawer = gs?.gameData?.currentDrawer as string | undefined;
      if (playerId) {
        setLiveIsDrawer(!!currentDrawer && currentDrawer === playerId);
      }
      // Optionally, if full drawing data is sent, replay it
      const data = gs?.gameData?.drawingData as DrawingCommand[] | undefined;
      if (Array.isArray(data) && engineRef.current) {
        // Clear before replay to avoid duplicates
        engineRef.current.applyDrawingCommand({ type: "clear", timestamp: Date.now() });
        for (const cmd of data) engineRef.current.applyDrawingCommand(cmd);
      }
    };

    globalThis.addEventListener("gameStateUpdate", handler as EventListener);
    return () => {
      globalThis.removeEventListener("gameStateUpdate", handler as EventListener);
    };
  }, [playerId]);

  // Listen for draw updates forwarded by Scoreboard via global websocket-message
  useEffect(() => {
    const handler = (evt: Event) => {
      const anyEvt = evt as CustomEvent;
      const msg = anyEvt.detail?.data as any;
      if (!msg || msg.type !== "draw-update") return;

      const payload = msg.data;
      // Support multiple payload shapes:
      // 1) data is a single DrawingCommand (server current behavior)
      // 2) data.command is a single command
      // 3) data.commands is an array of commands
      if (payload) {
        if (Array.isArray((payload as any).commands)) {
          for (const cmd of (payload as any).commands as DrawingCommand[]) {
            engineRef.current?.applyDrawingCommand(cmd);
          }
        } else if ((payload as any).command) {
          const cmd = (payload as any).command as DrawingCommand;
          engineRef.current?.applyDrawingCommand(cmd);
        } else if (typeof payload === "object" && (payload as any).type) {
          const cmd = payload as DrawingCommand;
          engineRef.current?.applyDrawingCommand(cmd);
        }
      }
    };

    globalThis.addEventListener("websocket-message", handler as EventListener);
    return () => {
      globalThis.removeEventListener("websocket-message", handler as EventListener);
    };
  }, []);

  return (
    <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-4">
      <DrawingEngine
        ref={engineRef}
        isDrawer={liveIsDrawer}
        onDrawingCommand={stableOnDrawCommand}
        width={width}
        height={height}
        disabled={liveDisabled}
      />
    </div>
  );
}
