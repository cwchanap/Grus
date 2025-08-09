// Drawing game specific drawing board component
import { useEffect, useRef, useState } from "preact/hooks";
import { DrawingCommand } from "../../../types/games/drawing.ts";
import { DrawingCommandThrottler } from "../../../lib/games/drawing/drawing-utils.ts";

interface DrawingBoardProps {
  width: number;
  height: number;
  onDrawCommand: (command: DrawingCommand) => void;
  drawingData: DrawingCommand[];
  isDrawer: boolean;
  disabled?: boolean;
  playerId: string;
}

export default function DrawingBoard({
  width,
  height,
  onDrawCommand,
  drawingData,
  isDrawer,
  disabled = false,
  playerId,
}: DrawingBoardProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [isDrawing, setIsDrawing] = useState(false);
  const [currentColor, setCurrentColor] = useState("#000000");
  const [currentSize, setCurrentSize] = useState(5);
  const throttlerRef = useRef<DrawingCommandThrottler | null>(null);

  // Live state driven by server game-state updates
  const [liveDisabled, setLiveDisabled] = useState<boolean>(disabled);
  const [liveIsDrawer, setLiveIsDrawer] = useState<boolean>(isDrawer);
  const [liveDrawingData, setLiveDrawingData] = useState<DrawingCommand[]>(drawingData);

  const colors = [
    "#000000",
    "#FF0000",
    "#00FF00",
    "#0000FF",
    "#FFFF00",
    "#FF00FF",
    "#00FFFF",
    "#FFA500",
  ];

  const sizes = [2, 5, 10];

  useEffect(() => {
    if (liveIsDrawer && !liveDisabled) {
      throttlerRef.current = new DrawingCommandThrottler(16); // ~60fps
    }

    return () => {
      throttlerRef.current?.destroy();
    };
  }, [liveIsDrawer, liveDisabled]);

  useEffect(() => {
    redrawCanvas();
  }, [liveDrawingData]);

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

      // Sync drawing data when provided in full game-state
      const data = gs?.gameData?.drawingData;
      if (Array.isArray(data)) {
        setLiveDrawingData(data as DrawingCommand[]);
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

      const payload = msg.data || {};
      const commands: DrawingCommand[] | undefined = payload.commands;
      const command: DrawingCommand | undefined = payload.command;

      if (Array.isArray(commands)) {
        setLiveDrawingData((prev) => [...prev, ...commands]);
      } else if (command && typeof command === "object") {
        setLiveDrawingData((prev) => [...prev, command]);
      }
    };

    globalThis.addEventListener("websocket-message", handler as EventListener);
    return () => {
      globalThis.removeEventListener("websocket-message", handler as EventListener);
    };
  }, []);

  const redrawCanvas = () => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    // Clear canvas
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    // Redraw all commands
    let currentPath: Path2D | null = null;

    for (const command of liveDrawingData) {
      switch (command.type) {
        case "start":
          currentPath = new Path2D();
          ctx.strokeStyle = command.color || "#000000";
          ctx.lineWidth = command.size || 5;
          ctx.lineCap = "round";
          ctx.lineJoin = "round";
          if (command.x !== undefined && command.y !== undefined) {
            currentPath.moveTo(command.x, command.y);
          }
          break;

        case "move":
          if (currentPath && command.x !== undefined && command.y !== undefined) {
            currentPath.lineTo(command.x, command.y);
            ctx.stroke(currentPath);
          }
          break;

        case "end":
          currentPath = null;
          break;

        case "clear":
          ctx.clearRect(0, 0, canvas.width, canvas.height);
          break;
      }
    }
  };

  const getCanvasCoordinates = (e: MouseEvent | TouchEvent): { x: number; y: number } => {
    const canvas = canvasRef.current;
    if (!canvas) return { x: 0, y: 0 };

    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;

    let clientX: number, clientY: number;

    if (e instanceof MouseEvent) {
      clientX = e.clientX;
      clientY = e.clientY;
    } else {
      const touch = e.touches[0] || e.changedTouches[0];
      clientX = touch.clientX;
      clientY = touch.clientY;
    }

    return {
      x: (clientX - rect.left) * scaleX,
      y: (clientY - rect.top) * scaleY,
    };
  };

  const startDrawing = (e: MouseEvent | TouchEvent) => {
    if (!liveIsDrawer || liveDisabled) return;

    e.preventDefault();
    setIsDrawing(true);

    const { x, y } = getCanvasCoordinates(e);
    const command: DrawingCommand = {
      type: "start",
      x,
      y,
      color: currentColor,
      size: currentSize,
      timestamp: Date.now(),
    };

    if (throttlerRef.current) {
      throttlerRef.current.throttle(command, onDrawCommand);
    } else {
      onDrawCommand(command);
    }
  };

  const draw = (e: MouseEvent | TouchEvent) => {
    if (!isDrawing || !liveIsDrawer || liveDisabled) return;

    e.preventDefault();
    const { x, y } = getCanvasCoordinates(e);
    const command: DrawingCommand = {
      type: "move",
      x,
      y,
      timestamp: Date.now(),
    };

    if (throttlerRef.current) {
      throttlerRef.current.throttle(command, onDrawCommand);
    } else {
      onDrawCommand(command);
    }
  };

  const stopDrawing = (e: MouseEvent | TouchEvent) => {
    if (!isDrawing || !liveIsDrawer || liveDisabled) return;

    e.preventDefault();
    setIsDrawing(false);

    const command: DrawingCommand = {
      type: "end",
      timestamp: Date.now(),
    };

    if (throttlerRef.current) {
      throttlerRef.current.throttle(command, onDrawCommand);
    } else {
      onDrawCommand(command);
    }
  };

  const clearCanvas = () => {
    if (!liveIsDrawer || liveDisabled) return;

    const command: DrawingCommand = {
      type: "clear",
      timestamp: Date.now(),
    };

    onDrawCommand(command);
  };

  return (
    <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-4">
      {/* Drawing Tools */}
      {isDrawer && !disabled && (
        <div class="mb-4 space-y-3">
          {/* Colors */}
          <div class="flex items-center space-x-2">
            <span class="text-sm font-medium text-gray-700">Color:</span>
            <div class="flex space-x-1">
              {colors.map((color) => (
                <button
                  type="button"
                  key={color}
                  onClick={() => setCurrentColor(color)}
                  class={`w-8 h-8 rounded-full border-2 ${
                    currentColor === color ? "border-gray-800" : "border-gray-300"
                  }`}
                  style={{ backgroundColor: color }}
                />
              ))}
            </div>
          </div>

          {/* Brush Sizes */}
          <div class="flex items-center space-x-2">
            <span class="text-sm font-medium text-gray-700">Size:</span>
            <div class="flex space-x-2">
              {sizes.map((size) => (
                <button
                  type="button"
                  key={size}
                  onClick={() => setCurrentSize(size)}
                  class={`px-3 py-1 text-sm rounded-md border ${
                    currentSize === size
                      ? "bg-blue-500 text-white border-blue-500"
                      : "bg-white text-gray-700 border-gray-300 hover:bg-gray-50"
                  }`}
                >
                  {size}px
                </button>
              ))}
            </div>
          </div>

          {/* Clear Button */}
          <div>
            <button
              type="button"
              onClick={clearCanvas}
              class="px-4 py-2 bg-red-500 text-white text-sm font-medium rounded-md hover:bg-red-600 focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-2 transition-colors"
            >
              Clear Canvas
            </button>
          </div>
        </div>
      )}

      {/* Canvas */}
      <div class="relative">
        <canvas
          ref={canvasRef}
          width={width}
          height={height}
          class={`border border-gray-300 rounded-md ${
            liveIsDrawer && !liveDisabled ? "cursor-crosshair" : "cursor-default"
          }`}
          style={{ maxWidth: "100%", height: "auto" }}
          onMouseDown={startDrawing}
          onMouseMove={draw}
          onMouseUp={stopDrawing}
          onMouseLeave={stopDrawing}
          onTouchStart={startDrawing}
          onTouchMove={draw}
          onTouchEnd={stopDrawing}
        />

        {(!liveIsDrawer || liveDisabled) && (
          <div class="absolute inset-0 bg-gray-100 bg-opacity-50 flex items-center justify-center rounded-md">
            <p class="text-gray-600 text-sm">
              {liveDisabled ? "Game not started" : "Watch others draw"}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
