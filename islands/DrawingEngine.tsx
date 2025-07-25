import { useEffect, useRef, useState, useImperativeHandle } from "preact/hooks";
import { forwardRef } from "preact/compat";
import * as PIXI from "pixi.js";
import { DrawingCommand } from "../types/game.ts";
import { 
  DrawingCommandThrottler, 
  DrawingCommandBuffer,
  validateDrawingCommand 
} from "../lib/drawing-utils.ts";
import MobileDrawingTools from "../components/MobileDrawingTools.tsx";

export interface DrawingEngineProps {
  isDrawer: boolean;
  onDrawingCommand: (command: DrawingCommand) => void;
  onDrawingCommands?: (commands: DrawingCommand[]) => void;
  width?: number;
  height?: number;
  disabled?: boolean;
}

export interface DrawingTool {
  color: string;
  size: number;
  type: 'brush';
}

export interface DrawingEngineRef {
  applyDrawingCommand: (command: DrawingCommand) => void;
  clearCanvas: () => void;
  undo: () => void;
  getDrawingHistory: () => DrawingCommand[];
}

const DrawingEngine = forwardRef<DrawingEngineRef, DrawingEngineProps>(({
  isDrawer,
  onDrawingCommand,
  onDrawingCommands,
  width = 800,
  height = 600,
  disabled = false,
}: DrawingEngineProps, ref) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const pixiAppRef = useRef<PIXI.Application | null>(null);
  const drawingContainerRef = useRef<PIXI.Container | null>(null);
  const currentPathRef = useRef<PIXI.Graphics | null>(null);
  const isDrawingRef = useRef(false);
  const lastPointRef = useRef<{ x: number; y: number } | null>(null);
  
  const [currentTool, setCurrentTool] = useState<DrawingTool>({
    color: '#000000',
    size: 5,
    type: 'brush'
  });
  
  const [drawingHistory, setDrawingHistory] = useState<DrawingCommand[]>([]);
  const [undoStack, setUndoStack] = useState<DrawingCommand[][]>([]);
  
  // Network optimization
  const throttlerRef = useRef<DrawingCommandThrottler | null>(null);
  const bufferRef = useRef<DrawingCommandBuffer | null>(null);

  // Initialize network optimization tools
  useEffect(() => {
    throttlerRef.current = new DrawingCommandThrottler(16); // ~60fps
    bufferRef.current = new DrawingCommandBuffer(
      (commands) => {
        if (onDrawingCommands) {
          onDrawingCommands(commands);
        }
      },
      10, // buffer size
      50  // flush interval
    );

    return () => {
      throttlerRef.current?.destroy();
      bufferRef.current?.destroy();
    };
  }, [onDrawingCommands]);

  // Initialize Pixi.js application
  useEffect(() => {
    if (!canvasRef.current) return;

    const app = new PIXI.Application();
    
    const initPixi = async () => {
      await app.init({
        canvas: canvasRef.current!,
        width,
        height,
        backgroundColor: 0xffffff,
        antialias: true,
        resolution: window.devicePixelRatio || 1,
        autoDensity: true,
      });

      // Create drawing container
      const drawingContainer = new PIXI.Container();
      app.stage.addChild(drawingContainer);
      
      pixiAppRef.current = app;
      drawingContainerRef.current = drawingContainer;

      // Set up interaction
      app.stage.eventMode = 'static';
      app.stage.hitArea = new PIXI.Rectangle(0, 0, width, height);
      
      if (isDrawer && !disabled) {
        setupDrawingEvents(app, drawingContainer);
      }
    };

    initPixi();

    return () => {
      if (pixiAppRef.current) {
        pixiAppRef.current.destroy(true);
        pixiAppRef.current = null;
      }
    };
  }, [width, height]);

  // Update drawing permissions when isDrawer changes
  useEffect(() => {
    if (!pixiAppRef.current) return;
    
    if (isDrawer && !disabled) {
      setupDrawingEvents(pixiAppRef.current, drawingContainerRef.current!);
    } else {
      removeDrawingEvents(pixiAppRef.current);
    }
  }, [isDrawer, disabled]);

  const setupDrawingEvents = (app: PIXI.Application, container: PIXI.Container) => {
    // Enhanced mobile touch support
    let lastTouchTime = 0;
    let touchStartPoint: { x: number; y: number } | null = null;
    
    // Prevent default touch behaviors to avoid scrolling/zooming while drawing
    const preventDefaultTouch = (e: TouchEvent) => {
      e.preventDefault();
      e.stopPropagation();
    };

    // Enhanced pointer down with mobile optimizations
    const onPointerDown = (event: PIXI.FederatedPointerEvent) => {
      if (!isDrawer || disabled) return;
      
      // Prevent default touch behavior
      if (event.nativeEvent) {
        event.nativeEvent.preventDefault();
        event.nativeEvent.stopPropagation();
      }
      
      const point = event.global;
      touchStartPoint = { x: point.x, y: point.y };
      lastTouchTime = Date.now();
      
      startDrawing(point.x, point.y);
    };

    // Enhanced pointer move with mobile optimizations
    const onPointerMove = (event: PIXI.FederatedPointerEvent) => {
      if (!isDrawer || disabled || !isDrawingRef.current) return;
      
      // Prevent default touch behavior
      if (event.nativeEvent) {
        event.nativeEvent.preventDefault();
        event.nativeEvent.stopPropagation();
      }
      
      const point = event.global;
      
      // Mobile optimization: throttle move events for better performance
      const now = Date.now();
      if (now - lastTouchTime < 16) return; // ~60fps throttling
      lastTouchTime = now;
      
      continueDrawing(point.x, point.y);
    };

    // Enhanced pointer up with mobile optimizations
    const onPointerUp = (event?: PIXI.FederatedPointerEvent) => {
      if (!isDrawer || disabled) return;
      
      // Prevent default touch behavior
      if (event?.nativeEvent) {
        event.nativeEvent.preventDefault();
        event.nativeEvent.stopPropagation();
      }
      
      // Mobile optimization: detect tap vs draw
      if (touchStartPoint && event) {
        const point = event.global;
        const distance = Math.sqrt(
          Math.pow(point.x - touchStartPoint.x, 2) + 
          Math.pow(point.y - touchStartPoint.y, 2)
        );
        
        // If it's a very short tap (< 5px movement), treat as a dot
        if (distance < 5) {
          // Create a small dot at the touch point
          const dotCommand: DrawingCommand = {
            type: 'start',
            x: touchStartPoint.x,
            y: touchStartPoint.y,
            color: currentTool.color,
            size: Math.max(currentTool.size, 3), // Minimum size for visibility
            timestamp: Date.now(),
          };
          
          if (validateDrawingCommand(dotCommand)) {
            setDrawingHistory(prev => [...prev, dotCommand]);
            if (throttlerRef.current) {
              throttlerRef.current.throttle(dotCommand, onDrawingCommand);
            } else {
              onDrawingCommand(dotCommand);
            }
          }
        }
      }
      
      touchStartPoint = null;
      endDrawing();
    };

    // Mobile-specific touch event handlers
    const onTouchStart = (e: TouchEvent) => {
      if (!isDrawer || disabled) return;
      
      e.preventDefault();
      e.stopPropagation();
      
      // Handle multi-touch gestures
      if (e.touches.length > 1) {
        // Multi-touch detected - could implement zoom/pan here
        return;
      }
      
      const touch = e.touches[0];
      const rect = (e.target as HTMLCanvasElement).getBoundingClientRect();
      const x = (touch.clientX - rect.left) * (app.canvas.width / rect.width);
      const y = (touch.clientY - rect.top) * (app.canvas.height / rect.height);
      
      touchStartPoint = { x, y };
      lastTouchTime = Date.now();
      startDrawing(x, y);
    };

    const onTouchMove = (e: TouchEvent) => {
      if (!isDrawer || disabled || !isDrawingRef.current) return;
      
      e.preventDefault();
      e.stopPropagation();
      
      if (e.touches.length > 1) return; // Ignore multi-touch
      
      const now = Date.now();
      if (now - lastTouchTime < 16) return; // Throttle for performance
      lastTouchTime = now;
      
      const touch = e.touches[0];
      const rect = (e.target as HTMLCanvasElement).getBoundingClientRect();
      const x = (touch.clientX - rect.left) * (app.canvas.width / rect.width);
      const y = (touch.clientY - rect.top) * (app.canvas.height / rect.height);
      
      continueDrawing(x, y);
    };

    const onTouchEnd = (e: TouchEvent) => {
      if (!isDrawer || disabled) return;
      
      e.preventDefault();
      e.stopPropagation();
      
      // Handle tap vs draw detection
      if (touchStartPoint && e.changedTouches.length > 0) {
        const touch = e.changedTouches[0];
        const rect = (e.target as HTMLCanvasElement).getBoundingClientRect();
        const x = (touch.clientX - rect.left) * (app.canvas.width / rect.width);
        const y = (touch.clientY - rect.top) * (app.canvas.height / rect.height);
        
        const distance = Math.sqrt(
          Math.pow(x - touchStartPoint.x, 2) + 
          Math.pow(y - touchStartPoint.y, 2)
        );
        
        // Create dot for short taps
        if (distance < 5) {
          const dotCommand: DrawingCommand = {
            type: 'start',
            x: touchStartPoint.x,
            y: touchStartPoint.y,
            color: currentTool.color,
            size: Math.max(currentTool.size, 3),
            timestamp: Date.now(),
          };
          
          if (validateDrawingCommand(dotCommand)) {
            setDrawingHistory(prev => [...prev, dotCommand]);
            if (throttlerRef.current) {
              throttlerRef.current.throttle(dotCommand, onDrawingCommand);
            } else {
              onDrawingCommand(dotCommand);
            }
          }
        }
      }
      
      touchStartPoint = null;
      endDrawing();
    };

    // Add enhanced touch event prevention to canvas
    const canvas = app.canvas;
    if (canvas) {
      // Standard touch events with enhanced mobile support
      canvas.addEventListener('touchstart', onTouchStart, { passive: false });
      canvas.addEventListener('touchmove', onTouchMove, { passive: false });
      canvas.addEventListener('touchend', onTouchEnd, { passive: false });
      canvas.addEventListener('touchcancel', onTouchEnd, { passive: false });
      
      // Prevent context menu on long press
      canvas.addEventListener('contextmenu', preventDefaultTouch, { passive: false });
      
      // Prevent selection
      canvas.style.userSelect = 'none';
      canvas.style.webkitUserSelect = 'none';
      canvas.style.webkitTouchCallout = 'none';
      
      // Optimize for touch
      canvas.style.touchAction = 'none';
    }

    // Standard pointer events (fallback for non-touch devices)
    app.stage.on('pointerdown', onPointerDown);
    app.stage.on('pointermove', onPointerMove);
    app.stage.on('pointerup', onPointerUp);
    app.stage.on('pointerupoutside', onPointerUp);
  };

  const removeDrawingEvents = (app: PIXI.Application) => {
    // Remove touch event prevention from canvas
    const canvas = app.canvas;
    if (canvas) {
      canvas.removeEventListener('touchstart', () => {});
      canvas.removeEventListener('touchmove', () => {});
      canvas.removeEventListener('touchend', () => {});
    }

    app.stage.off('pointerdown');
    app.stage.off('pointermove');
    app.stage.off('pointerup');
    app.stage.off('pointerupoutside');
  };

  const startDrawing = (x: number, y: number) => {
    if (!drawingContainerRef.current) return;

    isDrawingRef.current = true;
    lastPointRef.current = { x, y };

    // Create new graphics object for this stroke
    const graphics = new PIXI.Graphics();
    graphics.moveTo(x, y);
    drawingContainerRef.current.addChild(graphics);
    currentPathRef.current = graphics;

    // Create drawing command
    const command: DrawingCommand = {
      type: 'start',
      x,
      y,
      color: currentTool.color,
      size: currentTool.size,
      timestamp: Date.now(),
    };

    // Validate and add to history
    if (validateDrawingCommand(command)) {
      setDrawingHistory(prev => [...prev, command]);
      
      // Send via throttler for network optimization
      if (throttlerRef.current) {
        throttlerRef.current.throttle(command, onDrawingCommand);
      } else {
        onDrawingCommand(command);
      }
    }
  };

  const continueDrawing = (x: number, y: number) => {
    if (!isDrawingRef.current || !currentPathRef.current || !lastPointRef.current) return;

    // Draw line from last point to current point
    currentPathRef.current.stroke({
      width: currentTool.size,
      color: currentTool.color,
      cap: 'round',
      join: 'round',
    });
    currentPathRef.current.lineTo(x, y);

    // Create drawing command
    const command: DrawingCommand = {
      type: 'move',
      x,
      y,
      color: currentTool.color,
      size: currentTool.size,
      timestamp: Date.now(),
    };

    // Validate and add to history
    if (validateDrawingCommand(command)) {
      setDrawingHistory(prev => [...prev, command]);
      
      // Send via throttler for network optimization
      if (throttlerRef.current) {
        throttlerRef.current.throttle(command, onDrawingCommand);
      } else {
        onDrawingCommand(command);
      }
    }

    lastPointRef.current = { x, y };
  };

  const endDrawing = () => {
    if (!isDrawingRef.current) return;

    isDrawingRef.current = false;
    currentPathRef.current = null;
    lastPointRef.current = null;

    // Create end command
    const command: DrawingCommand = {
      type: 'end',
      timestamp: Date.now(),
    };

    // Save current state for undo
    setUndoStack(prev => [...prev, [...drawingHistory]]);

    // Validate and add to history
    if (validateDrawingCommand(command)) {
      setDrawingHistory(prev => [...prev, command]);
      
      // Send via throttler for network optimization
      if (throttlerRef.current) {
        throttlerRef.current.throttle(command, onDrawingCommand);
      } else {
        onDrawingCommand(command);
      }
    }
  };

  const clearCanvas = () => {
    if (!drawingContainerRef.current) return;

    // Save current state for undo
    setUndoStack(prev => [...prev, [...drawingHistory]]);

    // Clear all graphics
    drawingContainerRef.current.removeChildren();

    // Create clear command
    const command: DrawingCommand = {
      type: 'clear',
      timestamp: Date.now(),
    };

    // Validate and reset history
    if (validateDrawingCommand(command)) {
      setDrawingHistory([command]);
      
      // Send via throttler for network optimization
      if (throttlerRef.current) {
        throttlerRef.current.throttle(command, onDrawingCommand);
      } else {
        onDrawingCommand(command);
      }
    }
  };

  const undo = () => {
    if (undoStack.length === 0) return;

    const previousState = undoStack[undoStack.length - 1];
    setUndoStack(prev => prev.slice(0, -1));
    
    // Restore previous state
    setDrawingHistory(previousState);
    redrawFromHistory(previousState);
    
    // Notify about the undo operation
    if (onDrawingCommands) {
      onDrawingCommands(previousState);
    }
  };

  const redrawFromHistory = (commands: DrawingCommand[]) => {
    if (!drawingContainerRef.current) return;

    // Clear current drawing
    drawingContainerRef.current.removeChildren();

    let currentGraphics: PIXI.Graphics | null = null;

    commands.forEach(command => {
      switch (command.type) {
        case 'start':
          currentGraphics = new PIXI.Graphics();
          currentGraphics.moveTo(command.x!, command.y!);
          drawingContainerRef.current!.addChild(currentGraphics);
          break;
          
        case 'move':
          if (currentGraphics) {
            currentGraphics.stroke({
              width: command.size!,
              color: command.color!,
              cap: 'round',
              join: 'round',
            });
            currentGraphics.lineTo(command.x!, command.y!);
          }
          break;
          
        case 'end':
          currentGraphics = null;
          break;
          
        case 'clear':
          drawingContainerRef.current!.removeChildren();
          currentGraphics = null;
          break;
      }
    });
  };

  // Apply external drawing commands (from other players)
  // Expose method to apply external drawing commands
  const applyDrawingCommand = (command: DrawingCommand) => {
    if (!drawingContainerRef.current || !validateDrawingCommand(command)) return;

    switch (command.type) {
      case 'start':
        const graphics = new PIXI.Graphics();
        graphics.moveTo(command.x!, command.y!);
        drawingContainerRef.current.addChild(graphics);
        // Store reference for subsequent moves
        (graphics as any).commandId = command.timestamp;
        break;
        
      case 'move':
        // Find the graphics object for this stroke
        const targetGraphics = drawingContainerRef.current.children.find(
          child => (child as any).commandId && 
          Math.abs((child as any).commandId - command.timestamp) < 1000
        ) as PIXI.Graphics;
        
        if (targetGraphics) {
          targetGraphics.stroke({
            width: command.size!,
            color: command.color!,
            cap: 'round',
            join: 'round',
          });
          targetGraphics.lineTo(command.x!, command.y!);
        }
        break;
        
      case 'clear':
        drawingContainerRef.current.removeChildren();
        break;
    }
  };

  // Expose methods via ref
  useImperativeHandle(ref, () => ({
    applyDrawingCommand,
    clearCanvas,
    undo,
    getDrawingHistory: () => drawingHistory,
  }));

  return (
    <div class="drawing-engine">
      {/* Drawing Tools */}
      {isDrawer && !disabled && (
        <div class="drawing-tools mb-4">
          <MobileDrawingTools
            currentTool={currentTool}
            onToolChange={setCurrentTool}
            onClear={clearCanvas}
            onUndo={undo}
            canUndo={undoStack.length > 0}
            disabled={disabled}
          />
        </div>
      )}

      {/* Canvas */}
      <div class="drawing-canvas border-2 border-gray-300 rounded-lg overflow-hidden bg-white">
        <canvas
          ref={canvasRef}
          class={`block w-full h-auto max-w-full ${
            !isDrawer || disabled ? 'cursor-not-allowed' : 'cursor-crosshair'
          }`}
          style={{ 
            touchAction: 'none',
            maxWidth: '100%',
            height: 'auto'
          }}
        />
      </div>

      {/* Status */}
      <div class="mt-2 text-sm text-gray-600">
        {disabled ? 'Drawing disabled' : 
         isDrawer ? 'You are drawing' : 'Watching...'}
      </div>
    </div>
  );
});

export default DrawingEngine;