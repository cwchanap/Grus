import { useEffect, useImperativeHandle, useRef, useState } from "preact/hooks";
import { forwardRef } from "preact/compat";
import * as PIXI from "pixi.js";
import { DrawingCommand } from "../../../types/games/drawing.ts";
import {
  DrawingCommandBuffer,
  DrawingCommandThrottler,
  validateDrawingCommand,
} from "../../../lib/games/drawing/drawing-utils.ts";
import MobileDrawingTools from "../../../components/MobileDrawingTools.tsx";

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
  type: "brush";
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
  // For applying external (remote) drawing commands from other players
  const externalPathRef = useRef<PIXI.Graphics | null>(null);
  // Track latest permission flags to avoid stale closures during async Pixi init
  const isDrawerRef = useRef<boolean>(isDrawer);
  const disabledRef = useRef<boolean>(disabled);
  useEffect(() => {
    isDrawerRef.current = isDrawer;
    disabledRef.current = disabled;
  }, [isDrawer, disabled]);
  // 2D canvas fallback when Pixi is not ready
  const twoDRef = useRef<CanvasRenderingContext2D | null>(null);
  // 2D fallback for remote (external) strokes
  const external2DRef = useRef<CanvasRenderingContext2D | null>(null);
  const lastExternalPointRef = useRef<{ x: number; y: number } | null>(null);
  // When true, DOM-level pointer handlers are active; used to prevent double-handling with Pixi stage events
  const domPointerActiveRef = useRef<boolean>(false);

  // Normalize color to Pixi's expected numeric format (0xRRGGBB)
  const toPixiColor = (c: string | number | undefined): number => {
    if (typeof c === "number") return c;
    if (!c) return 0x000000;
    try {
      // Prefer Pixi util when available
      const anyPIXI: any = PIXI as any;
      if (anyPIXI?.utils?.string2hex) return anyPIXI.utils.string2hex(c);
      // Fallback: parse #RRGGBB
      const hex = c.startsWith("#") ? c.slice(1) : c;
      return Number.parseInt(hex, 16) >>> 0;
    } catch {
      return 0x000000;
    }
  };

  // Pixi v7/v8 compatibility helpers for stroking lines
  const supportsStroke = (g: PIXI.Graphics) => typeof (g as any).stroke === "function";
  const supportsLineStyle = (g: PIXI.Graphics) => typeof (g as any).lineStyle === "function";
  const ensureLineStyleIfNeeded = (g: PIXI.Graphics, width: number, color: number) => {
    if (!supportsStroke(g) && supportsLineStyle(g)) {
      // Pixi v7 style; alpha 1 for full opacity
      (g as any).lineStyle(width, color, 1);
    }
  };
  const applyStroke = (g: PIXI.Graphics, width: number, color: number) => {
    if (supportsStroke(g)) {
      (g as any).stroke({ width, color, cap: "round", join: "round" });
    } else if (supportsLineStyle(g)) {
      // In v7, lineStyle persists; set it to be safe
      (g as any).lineStyle(width, color, 1);
      // No explicit stroke call in v7; drawing happens with path commands
    }
  };

  const [currentTool, setCurrentTool] = useState<DrawingTool>({
    color: "#000000",
    size: 5,
    type: "brush",
  });

  const [drawingHistory, setDrawingHistory] = useState<DrawingCommand[]>([]);
  const [undoStack, setUndoStack] = useState<DrawingCommand[][]>([]);

  // Network optimization
  const throttlerRef = useRef<DrawingCommandThrottler | null>(null);
  const bufferRef = useRef<DrawingCommandBuffer | null>(null);

  // Force render the stage immediately (useful for tests/headless envs)
  const renderNow = () => {
    const app = pixiAppRef.current;
    if (app) {
      try {
        app.renderer.render(app.stage);
      } catch (_e) {
        // ignore render errors
      }
    }
  };

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
      50, // flush interval
    );

    return () => {
      throttlerRef.current?.destroy();
      bufferRef.current?.destroy();
    };
  }, [onDrawingCommands]);

  // Initialize Pixi.js application
  useEffect(() => {
    if (!canvasRef.current) return;

    // Pre-size canvas to intended logical size so PNG captures are meaningful even if Pixi init fails
    try {
      const dpr = globalThis.devicePixelRatio || 1;
      const cvs = canvasRef.current;
      cvs.width = Math.floor(width * dpr);
      cvs.height = Math.floor(height * dpr);
      cvs.style.width = `${width}px`;
      cvs.style.height = `${height}px`;
    } catch (_e) {
      // ignore
    }

    const app = new PIXI.Application();

    const initPixi = async () => {
      // Mark canvas for E2E tests to target reliably
      canvasRef.current!.setAttribute("data-testid", "drawing-canvas");

      await app.init({
        canvas: canvasRef.current!,
        width,
        height,
        backgroundColor: 0xffffff,
        backgroundAlpha: 1,
        antialias: true,
        resolution: globalThis.devicePixelRatio || 1,
        autoDensity: true,
        // Enable reliable image capture for tests (WebGL context)
        preserveDrawingBuffer: true,
      });

      // Ensure the canvas backing store size matches our logical size for reliable toDataURL
      try {
        const dpr = globalThis.devicePixelRatio || 1;
        const cvs = app.canvas ?? canvasRef.current!;
        // Set pixel buffer size explicitly
        cvs.width = Math.floor(width * dpr);
        cvs.height = Math.floor(height * dpr);
        // Ensure CSS size matches logical size
        cvs.style.width = `${width}px`;
        cvs.style.height = `${height}px`;
      } catch (_e) {
        // ignore sizing issues
      }

      // Ensure ticker is running so frames render even in headless
      try {
        app.ticker.start();
      } catch (_e) {
        // noop
      }

      // Create drawing container
      const drawingContainer = new PIXI.Container();
      app.stage.addChild(drawingContainer);

      pixiAppRef.current = app;
      drawingContainerRef.current = drawingContainer;

      // Expose a small test helper to fetch PNG data for hashing in Playwright
      (globalThis as any).__drawing = {
        // Force a render on demand
        renderNow: () => {
          try {
            renderNow();
            return true;
          } catch {
            return false;
          }
        },
        // Readiness helpers for tests
        isReady: () =>
          !!(pixiAppRef.current && (drawingContainerRef.current || pixiAppRef.current!.stage)),
        waitUntilReady: async (timeoutMs = 3000) => {
          const start = globalThis.performance?.now?.() ?? Date.now();
          while (((globalThis.performance?.now?.() ?? Date.now()) - start) < timeoutMs) {
            if ((globalThis as any).__drawing.isReady()) return true;
            await new Promise((r) => setTimeout(r, 25));
          }
          return false;
        },
        png: () => {
          const app = pixiAppRef.current;
          if (!app) {
            try {
              return canvasRef.current?.toDataURL("image/png") ?? null;
            } catch (_e) {
              return null;
            }
          }
          renderNow();
          // Try Extract first
          try {
            const r: any = app.renderer as any;
            const extract: any = r.extract ?? r.plugins?.extract;
            if (extract && typeof extract.canvas === "function") {
              const c = extract.canvas(app.stage);
              if (c) return c.toDataURL("image/png");
            }
          } catch (_e) {
            // ignore and fall back
          }
          try {
            const c = (app.renderer as any).view ?? app.canvas ?? canvasRef.current;
            return c?.toDataURL("image/png") ?? null;
          } catch (_e) {
            return null;
          }
        },
        drawLine: (x1: number, y1: number, x2: number, y2: number, steps = 12) => {
          try {
            if (!isDrawer || disabled) return false;
            // Use internal drawing helpers for deterministic strokes in tests
            startDrawing(x1, y1);
            for (let i = 1; i <= steps; i++) {
              const t = i / steps;
              const x = x1 + (x2 - x1) * t;
              const y = y1 + (y2 - y1) * t;
              continueDrawing(x, y);
            }
            endDrawing();
            return true;
          } catch (_e) {
            return false;
          }
        },
        // Test helper: bypass permission checks to draw deterministically
        forceDrawLine: (x1: number, y1: number, x2: number, y2: number, steps = 12) => {
          try {
            const app = pixiAppRef.current;
            const container = drawingContainerRef.current ?? app?.stage ?? null;
            if (!app || !container) {
              const cvs = canvasRef.current;
              const ctx = cvs?.getContext("2d");
              if (!ctx) return false;
              ctx.strokeStyle = "#000000";
              ctx.lineWidth = currentTool.size;
              ctx.lineCap = "round";
              ctx.beginPath();
              ctx.moveTo(x1, y1);
              ctx.lineTo(x2, y2);
              ctx.stroke();
              return true;
            }
            startDrawing(x1, y1);
            for (let i = 1; i <= steps; i++) {
              const t = i / steps;
              const x = x1 + (x2 - x1) * t;
              const y = y1 + (y2 - y1) * t;
              continueDrawing(x, y);
            }
            endDrawing();
            renderNow();
            return true;
          } catch (_e) {
            return false;
          }
        },
        // Test helper: draw a filled rectangle to validate rendering pipeline
        debugRect: (
          x: number,
          y: number,
          w: number,
          h: number,
          color: string | number = 0xff0000,
        ) => {
          try {
            const app = pixiAppRef.current;
            // Fallback to 2D canvas when Pixi isn't ready
            if (!app) {
              const cvs = canvasRef.current;
              const ctx = cvs?.getContext("2d");
              if (!ctx) return false;
              ctx.fillStyle = typeof color === "string"
                ? color
                : `#${(color as number).toString(16).padStart(6, "0")}`;
              ctx.fillRect(x, y, w, h);
              return true;
            }
            const container = drawingContainerRef.current ?? app.stage ?? null;
            if (!container) {
              const cvs = canvasRef.current;
              const ctx = cvs?.getContext("2d");
              if (!ctx) return false;
              const chex = typeof color === "string"
                ? color
                : `#${(color as number).toString(16).padStart(6, "0")}`;
              ctx.fillStyle = chex;
              ctx.fillRect(x, y, w, h);
              return true;
            }
            const g = new PIXI.Graphics();
            const c = toPixiColor(color as any);
            if (typeof (g as any).fill === "function") {
              (g as any).rect(x, y, w, h).fill(c);
            } else {
              // Pixi v7 fallback
              (g as any).beginFill(c, 1);
              (g as any).drawRect(x, y, w, h);
              (g as any).endFill();
            }
            container.addChild(g);
            renderNow();
            return true;
          } catch (_e) {
            return false;
          }
        },
        canDraw: () => !!(isDrawer && !disabled),
        status: () => ({ isDrawer: !!isDrawer, disabled: !!disabled }),
        waitUntilCanDraw: async (timeoutMs = 5000) => {
          const start = globalThis.performance?.now?.() ?? Date.now();
          while (((globalThis.performance?.now?.() ?? Date.now()) - start) < timeoutMs) {
            if (isDrawer && !disabled) return true;
            await new Promise((r) => setTimeout(r, 50));
          }
          return false;
        },
      };

      // Set up interaction
      // Support Pixi v8 (eventMode) and v7 (interactive) for reliable pointer events
      if ("eventMode" in (app.stage as any)) {
        (app.stage as any).eventMode = "static";
      } else {
        // Pixi v7 fallback
        (app.stage as any).interactive = true;
        (app.stage as any).interactiveChildren = true;
      }
      app.stage.hitArea = new PIXI.Rectangle(0, 0, width, height);

      // Attach drawing events using the latest permission flags
      if (isDrawerRef.current && !disabledRef.current) {
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

  // Ensure test helper exists early and updates with permissions
  useEffect(() => {
    (globalThis as any).__drawing = {
      renderNow: () => {
        try {
          renderNow();
          return true;
        } catch {
          return false;
        }
      },
      isReady: () =>
        !!(pixiAppRef.current && (drawingContainerRef.current || pixiAppRef.current!.stage)),
      waitUntilReady: async (timeoutMs = 3000) => {
        const start = globalThis.performance?.now?.() ?? Date.now();
        while (((globalThis.performance?.now?.() ?? Date.now()) - start) < timeoutMs) {
          if ((globalThis as any).__drawing.isReady()) return true;
          await new Promise((r) => setTimeout(r, 25));
        }
        return false;
      },
      png: () => {
        const app = pixiAppRef.current;
        if (!app) {
          try {
            return canvasRef.current?.toDataURL("image/png") ?? null;
          } catch (_e) {
            return null;
          }
        }
        renderNow();
        try {
          const r: any = app.renderer as any;
          const extract: any = r.extract ?? r.plugins?.extract;
          if (extract && typeof extract.canvas === "function") {
            const c = extract.canvas(app.stage);
            if (c) return c.toDataURL("image/png");
          }
        } catch (_e) {
          // ignore
        }
        try {
          const c = (app.renderer as any).view ?? app.canvas ?? canvasRef.current;
          return c?.toDataURL("image/png") ?? null;
        } catch (_e) {
          return null;
        }
      },
      drawLine: (x1: number, y1: number, x2: number, y2: number, steps = 12) => {
        try {
          if (!isDrawer || disabled) return false;
          startDrawing(x1, y1);
          for (let i = 1; i <= steps; i++) {
            const t = i / steps;
            const x = x1 + (x2 - x1) * t;
            const y = y1 + (y2 - y1) * t;
            continueDrawing(x, y);
          }
          endDrawing();
          return true;
        } catch (_e) {
          return false;
        }
      },
      // Test helper: bypass permission checks
      forceDrawLine: (x1: number, y1: number, x2: number, y2: number, steps = 12) => {
        try {
          const app = pixiAppRef.current;
          const container = drawingContainerRef.current ?? app?.stage ?? null;
          if (!app || !container) {
            const cvs = canvasRef.current;
            const ctx = cvs?.getContext("2d");
            if (!ctx) return false;
            ctx.strokeStyle = "#000000";
            ctx.lineWidth = currentTool.size;
            ctx.lineCap = "round";
            ctx.beginPath();
            ctx.moveTo(x1, y1);
            ctx.lineTo(x2, y2);
            ctx.stroke();
            return true;
          }
          startDrawing(x1, y1);
          for (let i = 1; i <= steps; i++) {
            const t = i / steps;
            const x = x1 + (x2 - x1) * t;
            const y = y1 + (y2 - y1) * t;
            continueDrawing(x, y);
          }
          endDrawing();
          renderNow();
          return true;
        } catch (_e) {
          return false;
        }
      },
      // Test helper: draw a filled rectangle to validate rendering pipeline
      debugRect: (
        x: number,
        y: number,
        w: number,
        h: number,
        color: string | number = 0xff0000,
      ) => {
        try {
          const app = pixiAppRef.current;
          if (!app) {
            const cvs = canvasRef.current;
            const ctx = cvs?.getContext("2d");
            if (!ctx) return false;
            ctx.fillStyle = typeof color === "string"
              ? color
              : `#${(color >>> 0).toString(16).padStart(6, "0")}`;
            ctx.fillRect(x, y, w, h);
            return true;
          }
          const container = drawingContainerRef.current ?? app.stage ?? null;
          if (!container) {
            const cvs = canvasRef.current;
            const ctx = cvs?.getContext("2d");
            if (!ctx) return false;
            ctx.fillStyle = typeof color === "string"
              ? color
              : `#${(color >>> 0).toString(16).padStart(6, "0")}`;
            ctx.fillRect(x, y, w, h);
            return true;
          }
          const g = new PIXI.Graphics();
          const c = toPixiColor(color as any);
          if (typeof (g as any).fill === "function") {
            (g as any).rect(x, y, w, h).fill(c);
          } else {
            // Pixi v7 fallback
            (g as any).beginFill(c, 1);
            (g as any).drawRect(x, y, w, h);
            (g as any).endFill();
          }
          container.addChild(g);
          renderNow();
          return true;
        } catch (_e) {
          return false;
        }
      },
      canDraw: () => !!(isDrawer && !disabled),
      status: () => ({ isDrawer: !!isDrawer, disabled: !!disabled }),
      waitUntilCanDraw: async (timeoutMs = 5000) => {
        const start = globalThis.performance?.now?.() ?? Date.now();
        while (((globalThis.performance?.now?.() ?? Date.now()) - start) < timeoutMs) {
          if (isDrawer && !disabled) return true;
          await new Promise((r) => setTimeout(r, 50));
        }
        return false;
      },
    };

    return () => {
      try {
        delete (globalThis as any).__drawing;
      } catch (_e) {
        // ignore
      }
    };
  }, [isDrawer, disabled]);

  // Update drawing permissions when isDrawer changes
  useEffect(() => {
    if (!pixiAppRef.current) return;

    if (isDrawer && !disabled) {
      setupDrawingEvents(pixiAppRef.current, drawingContainerRef.current!);
    } else {
      removeDrawingEvents(pixiAppRef.current);
    }
  }, [isDrawer, disabled]);

  const setupDrawingEvents = (app: PIXI.Application, _container: PIXI.Container) => {
    // Enhanced mobile touch support
    let lastTouchTime = 0;
    let touchStartPoint: { x: number; y: number } | null = null;

    // Prevent default touch behaviors to avoid scrolling/zooming while drawing
    const _preventDefaultTouch = (e: TouchEvent) => {
      e.preventDefault();
      e.stopPropagation();
    };

    // Enhanced pointer down with mobile optimizations
    const onPointerDown = (event: PIXI.FederatedPointerEvent) => {
      // Use refs to avoid stale closures when permissions change
      if (!isDrawerRef.current || disabledRef.current) return;
      // If DOM-level pointer handlers are active, skip Pixi handler to avoid double-processing
      if (domPointerActiveRef.current) return;

      // Prevent default touch behavior
      if (event.nativeEvent && "preventDefault" in event.nativeEvent) {
        event.nativeEvent.preventDefault();
      }
      if (event.nativeEvent && "stopPropagation" in event.nativeEvent) {
        event.nativeEvent.stopPropagation();
      }

      try {
        console.debug("[DrawingEngine] stage pointerdown", {
          x: (event as any)?.global?.x,
          y: (event as any)?.global?.y,
          isDrawer: isDrawerRef.current,
          disabled: disabledRef.current,
        });
      } catch (_e) {
        // Intentionally ignore console errors in restricted environments
      }

      const point = event.global;
      touchStartPoint = { x: point.x, y: point.y };
      lastTouchTime = Date.now();

      startDrawing(point.x, point.y);
    };

    // Enhanced pointer move with mobile optimizations
    const onPointerMove = (event: PIXI.FederatedPointerEvent) => {
      if (!isDrawerRef.current || disabledRef.current || !isDrawingRef.current) return;
      if (domPointerActiveRef.current) return; // Avoid double-processing when DOM handlers active

      // Prevent default touch behavior
      if (event.nativeEvent && "preventDefault" in event.nativeEvent) {
        event.nativeEvent.preventDefault();
      }
      if (event.nativeEvent && "stopPropagation" in event.nativeEvent) {
        event.nativeEvent.stopPropagation();
      }

      try {
        console.debug("[DrawingEngine] stage pointermove", {
          x: (event as any)?.global?.x,
          y: (event as any)?.global?.y,
        });
      } catch (_e) {
        // Intentionally ignore console errors in restricted environments
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
      if (!isDrawerRef.current || disabledRef.current) return;
      if (domPointerActiveRef.current) return; // DOM handler will finalize the stroke

      // Prevent default touch behavior
      if (event?.nativeEvent && "preventDefault" in event.nativeEvent) {
        event.nativeEvent.preventDefault();
      }
      if (event?.nativeEvent && "stopPropagation" in event.nativeEvent) {
        event.nativeEvent.stopPropagation();
      }

      try {
        console.debug("[DrawingEngine] stage pointerup");
      } catch (_e) {
        // Intentionally ignore console errors in restricted environments
      }

      // Mobile optimization: detect tap vs draw
      if (touchStartPoint && event) {
        const point = event.global;
        const distance = Math.sqrt(
          Math.pow(point.x - touchStartPoint.x, 2) +
            Math.pow(point.y - touchStartPoint.y, 2),
        );

        // If it's a very short tap (< 5px movement), treat as a dot
        if (distance < 5) {
          // Create a small dot at the touch point
          const dotCommand: DrawingCommand = {
            type: "start",
            x: touchStartPoint.x,
            y: touchStartPoint.y,
            color: currentTool.color,
            size: Math.max(currentTool.size, 3), // Minimum size for visibility
            timestamp: Date.now(),
          };

          if (validateDrawingCommand(dotCommand)) {
            setDrawingHistory((prev) => [...prev, dotCommand]);
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
      if (!isDrawerRef.current || disabledRef.current) return;

      e.preventDefault();
      e.stopPropagation();

      try {
        console.debug("[DrawingEngine] dom touchstart", { touches: e.touches.length });
      } catch (_e) {
        // Intentionally ignore console errors in restricted environments
      }

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
      if (!isDrawerRef.current || disabledRef.current || !isDrawingRef.current) return;

      e.preventDefault();
      e.stopPropagation();

      try {
        console.debug("[DrawingEngine] dom touchmove");
      } catch (_e) {
        // Intentionally ignore console errors in restricted environments
      }

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
      if (!isDrawerRef.current || disabledRef.current) return;

      e.preventDefault();
      e.stopPropagation();

      try {
        console.debug("[DrawingEngine] dom touchend");
      } catch (_e) {
        // Intentionally ignore console errors in restricted environments
      }

      // Handle tap vs draw detection
      if (touchStartPoint && e.changedTouches.length > 0) {
        const touch = e.changedTouches[0];
        const rect = (e.target as HTMLCanvasElement).getBoundingClientRect();
        const x = (touch.clientX - rect.left) * (app.canvas.width / rect.width);
        const y = (touch.clientY - rect.top) * (app.canvas.height / rect.height);

        const distance = Math.sqrt(
          Math.pow(x - touchStartPoint.x, 2) +
            Math.pow(y - touchStartPoint.y, 2),
        );

        // Create dot for short taps
        if (distance < 5) {
          const dotCommand: DrawingCommand = {
            type: "start",
            x: touchStartPoint.x,
            y: touchStartPoint.y,
            color: currentTool.color,
            size: Math.max(currentTool.size, 3),
            timestamp: Date.now(),
          };

          if (validateDrawingCommand(dotCommand)) {
            setDrawingHistory((prev) => [...prev, dotCommand]);
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
      canvas.addEventListener("touchstart", onTouchStart, { passive: false });
      canvas.addEventListener("touchmove", onTouchMove, { passive: false });
      canvas.addEventListener("touchend", onTouchEnd, { passive: false });
      canvas.addEventListener("touchcancel", onTouchEnd, { passive: false });

      // Prevent context menu on long press
      canvas.addEventListener("contextmenu", (e: Event) => e.preventDefault(), { passive: false });

      // Prevent selection
      canvas.style.userSelect = "none";
      (canvas.style as any).webkitUserSelect = "none";
      (canvas.style as any).webkitTouchCallout = "none";

      // Optimize for touch
      canvas.style.touchAction = "none";

      // DOM-level pointer fallback (capture phase) to ensure input works even if Pixi events don't fire
      const onDomPointerDown = (e: PointerEvent) => {
        if (!isDrawerRef.current || disabledRef.current) return;
        try {
          console.debug("[DrawingEngine] dom pointerdown", {
            type: e.pointerType,
            x: e.clientX,
            y: e.clientY,
          });
        } catch (_e) {
          // Intentionally ignore console errors in restricted environments
        }
        e.preventDefault();
        e.stopPropagation();
        domPointerActiveRef.current = true;
        const rect = (e.target as HTMLCanvasElement).getBoundingClientRect();
        const x = (e.clientX - rect.left) * (app.canvas.width / rect.width);
        const y = (e.clientY - rect.top) * (app.canvas.height / rect.height);
        touchStartPoint = { x, y };
        lastTouchTime = Date.now();
        startDrawing(x, y);
      };
      const onDomPointerMove = (e: PointerEvent) => {
        if (!isDrawerRef.current || disabledRef.current || !isDrawingRef.current) return;
        try {
          console.debug("[DrawingEngine] dom pointermove");
        } catch (_e) {
          // Intentionally ignore console errors in restricted environments
        }
        e.preventDefault();
        e.stopPropagation();
        const now = Date.now();
        if (now - lastTouchTime < 16) return;
        lastTouchTime = now;
        const rect = (e.target as HTMLCanvasElement).getBoundingClientRect();
        const x = (e.clientX - rect.left) * (app.canvas.width / rect.width);
        const y = (e.clientY - rect.top) * (app.canvas.height / rect.height);
        continueDrawing(x, y);
      };
      const onDomPointerUp = (e: PointerEvent) => {
        if (!isDrawerRef.current || disabledRef.current) return;
        try {
          console.debug("[DrawingEngine] dom pointerup");
        } catch (_e) {
          // Intentionally ignore console errors in restricted environments
        }
        e.preventDefault();
        e.stopPropagation();
        touchStartPoint = null;
        endDrawing();
        domPointerActiveRef.current = false;
      };

      canvas.addEventListener("pointerdown", onDomPointerDown, { passive: false, capture: true });
      canvas.addEventListener("pointermove", onDomPointerMove, { passive: false, capture: true });
      canvas.addEventListener("pointerup", onDomPointerUp, { passive: false, capture: true });
      canvas.addEventListener("pointercancel", onDomPointerUp, { passive: false, capture: true });

      // Store DOM handlers for precise removal later
      (setupDrawingEvents as any)._domHandlers = {
        onDomPointerDown,
        onDomPointerMove,
        onDomPointerUp,
      };
    }

    // Standard pointer events (fallback for non-touch devices)
    app.stage.on("pointerdown", onPointerDown);
    app.stage.on("pointermove", onPointerMove);
    app.stage.on("pointerup", onPointerUp);
    app.stage.on("pointerupoutside", onPointerUp);

    // Save handlers so we can remove them precisely later
    (setupDrawingEvents as any)._handlers = {
      onPointerDown,
      onPointerMove,
      onPointerUp,
      onTouchStart,
      onTouchMove,
      onTouchEnd,
    };
  };

  const removeDrawingEvents = (app: PIXI.Application) => {
    // Remove touch event prevention from canvas using stored handlers
    const canvas = app.canvas;
    const h = (setupDrawingEvents as any)._handlers || {};
    const dh = (setupDrawingEvents as any)._domHandlers || {};
    if (canvas) {
      try {
        canvas.removeEventListener("touchstart", h.onTouchStart);
      } catch (_e) { /* ignore */ }
      try {
        canvas.removeEventListener("touchmove", h.onTouchMove);
      } catch (_e) { /* ignore */ }
      try {
        canvas.removeEventListener("touchend", h.onTouchEnd);
      } catch (_e) { /* ignore */ }
      try {
        canvas.removeEventListener("touchcancel", h.onTouchEnd);
      } catch (_e) { /* ignore */ }
      try {
        canvas.removeEventListener("pointerdown", dh.onDomPointerDown, { capture: true } as any);
      } catch (_e) { /* ignore */ }
      try {
        canvas.removeEventListener("pointermove", dh.onDomPointerMove, { capture: true } as any);
      } catch (_e) { /* ignore */ }
      try {
        canvas.removeEventListener("pointerup", dh.onDomPointerUp, { capture: true } as any);
      } catch (_e) { /* ignore */ }
      try {
        canvas.removeEventListener("pointercancel", dh.onDomPointerUp, { capture: true } as any);
      } catch (_e) { /* ignore */ }
    }

    // Remove stage pointer listeners
    try {
      app.stage.off("pointerdown", h.onPointerDown);
    } catch (_e) { /* ignore */ }
    try {
      app.stage.off("pointermove", h.onPointerMove);
    } catch (_e) { /* ignore */ }
    try {
      app.stage.off("pointerup", h.onPointerUp);
    } catch (_e) { /* ignore */ }
    try {
      app.stage.off("pointerupoutside", h.onPointerUp);
    } catch (_e) { /* ignore */ }
  };

  const startDrawing = (x: number, y: number) => {
    const container = drawingContainerRef.current ?? pixiAppRef.current?.stage ?? null;

    isDrawingRef.current = true;
    lastPointRef.current = { x, y };

    if (container) {
      // Create new graphics object for this stroke
      const graphics = new PIXI.Graphics();
      // Ensure proper style is configured before any path in Pixi v7
      ensureLineStyleIfNeeded(graphics, currentTool.size, toPixiColor(currentTool.color));
      graphics.moveTo(x, y);
      container.addChild(graphics);
      currentPathRef.current = graphics;
    } else {
      // Fallback to 2D canvas drawing path so PNG changes even if Pixi isn't ready
      const ctx = canvasRef.current?.getContext("2d");
      if (ctx) {
        try {
          // Use current tool color; fallback to black on error
          (ctx as any).strokeStyle = currentTool.color as any;
        } catch {
          ctx.strokeStyle = "#000000";
        }
        ctx.lineWidth = currentTool.size;
        ctx.lineCap = "round";
        ctx.beginPath();
        ctx.moveTo(x, y);
        twoDRef.current = ctx;
      }
    }

    // Create drawing command
    const command: DrawingCommand = {
      type: "start",
      x,
      y,
      color: currentTool.color,
      size: currentTool.size,
      timestamp: Date.now(),
    };

    // Validate and add to history
    if (validateDrawingCommand(command)) {
      setDrawingHistory((prev) => [...prev, command]);

      // Send via throttler for network optimization
      if (throttlerRef.current) {
        throttlerRef.current.throttle(command, onDrawingCommand);
      } else {
        onDrawingCommand(command);
      }
    }
  };

  const continueDrawing = (x: number, y: number) => {
    if (!isDrawingRef.current || !lastPointRef.current) return;

    // Draw line from last point to current point with correct API order per version
    const g = currentPathRef.current;
    if (g) {
      if (supportsStroke(g)) {
        // Pixi v8+: add segment then stroke
        g.lineTo(x, y);
        applyStroke(g, currentTool.size, toPixiColor(currentTool.color));
      } else {
        // Pixi v7: set line style before adding segment
        ensureLineStyleIfNeeded(g, currentTool.size, toPixiColor(currentTool.color));
        g.lineTo(x, y);
      }
    } else if (twoDRef.current) {
      // 2D canvas fallback path drawing
      const ctx = twoDRef.current;
      try {
        (ctx as any).strokeStyle = currentTool.color as any;
      } catch {
        ctx.strokeStyle = "#000000";
      }
      ctx.lineWidth = currentTool.size;
      ctx.lineCap = "round";
      ctx.lineTo(x, y);
      ctx.stroke();
    }

    // Create drawing command
    const command: DrawingCommand = {
      type: "move",
      x,
      y,
      color: currentTool.color,
      size: currentTool.size,
      timestamp: Date.now(),
    };

    // Validate and add to history
    if (validateDrawingCommand(command)) {
      setDrawingHistory((prev) => [...prev, command]);

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

    // Close any 2D path if we were using the fallback
    if (twoDRef.current) {
      try {
        (twoDRef.current as any).closePath?.();
      } catch (_e) {
        // Silent close failure is acceptable for cleanup
      }
      twoDRef.current = null;
    }

    isDrawingRef.current = false;
    currentPathRef.current = null;
    lastPointRef.current = null;

    // Create end command
    const command: DrawingCommand = {
      type: "end",
      timestamp: Date.now(),
    };

    // Save current state for undo
    setUndoStack((prev) => [...prev, [...drawingHistory]]);

    // Validate and add to history
    if (validateDrawingCommand(command)) {
      setDrawingHistory((prev) => [...prev, command]);

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
    setUndoStack((prev) => [...prev, [...drawingHistory]]);

    // Clear all graphics
    drawingContainerRef.current.removeChildren();

    // Create clear command
    const command: DrawingCommand = {
      type: "clear",
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
    setUndoStack((prev) => prev.slice(0, -1));

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

    commands.forEach((command) => {
      switch (command.type) {
        case "start":
          currentGraphics = new PIXI.Graphics();
          // Ensure style for Pixi v7 before pathing
          ensureLineStyleIfNeeded(currentGraphics, command.size!, toPixiColor(command.color!));
          currentGraphics.moveTo(command.x!, command.y!);
          drawingContainerRef.current!.addChild(currentGraphics);
          break;

        case "move":
          if (currentGraphics) {
            if (supportsStroke(currentGraphics)) {
              currentGraphics.lineTo(command.x!, command.y!);
              applyStroke(currentGraphics, command.size!, toPixiColor(command.color!));
            } else {
              ensureLineStyleIfNeeded(currentGraphics, command.size!, toPixiColor(command.color!));
              currentGraphics.lineTo(command.x!, command.y!);
            }
          }
          break;

        case "end":
          currentGraphics = null;
          break;

        case "clear":
          drawingContainerRef.current!.removeChildren();
          currentGraphics = null;
          break;
      }
    });
  };

  // Apply external drawing commands (from other players)
  // Uses a persistent path since only one drawer is active at a time
  const applyDrawingCommand = (command: DrawingCommand) => {
    if (!validateDrawingCommand(command)) return;
    const container = drawingContainerRef.current ?? pixiAppRef.current?.stage ?? null;

    switch (command.type) {
      case "start": {
        if (container) {
          // Begin a new external stroke using Pixi
          const g = new PIXI.Graphics();
          ensureLineStyleIfNeeded(g, command.size!, toPixiColor(command.color!));
          g.moveTo(command.x!, command.y!);
          container.addChild(g);
          externalPathRef.current = g;
          // Ensure a render so initial moveTo is visible when capturing PNG
          try {
            renderNow();
          } catch (_e) {
            // Render failures are non-critical for external apply
          }
        } else {
          // 2D fallback
          const ctx = canvasRef.current?.getContext("2d");
          if (ctx) {
            const chex = typeof command.color === "string"
              ? (command.color as string)
              : `#${(toPixiColor(command.color as any) >>> 0).toString(16).padStart(6, "0")}`;
            try {
              (ctx as any).strokeStyle = chex ?? "#000000";
            } catch {
              ctx.strokeStyle = "#000000";
            }
            ctx.lineWidth = command.size ?? 5;
            ctx.lineCap = "round";
            ctx.beginPath();
            ctx.moveTo(command.x!, command.y!);
            external2DRef.current = ctx;
            lastExternalPointRef.current = { x: command.x!, y: command.y! };
          }
        }
        break;
      }

      case "move": {
        // Continue the current external stroke
        const g = externalPathRef.current;
        if (g) {
          if (supportsStroke(g)) {
            g.lineTo(command.x!, command.y!);
            applyStroke(g, command.size!, toPixiColor(command.color!));
          } else {
            ensureLineStyleIfNeeded(g as any, command.size!, toPixiColor(command.color!));
            g.lineTo(command.x!, command.y!);
          }
          // Force render so PNG reflects changes immediately
          try {
            renderNow();
          } catch (_e) {
            // Render failures are non-critical for external apply
          }
        } else if (external2DRef.current) {
          const ctx = external2DRef.current;
          const chex = typeof command.color === "string"
            ? (command.color as string)
            : `#${(toPixiColor(command.color as any) >>> 0).toString(16).padStart(6, "0")}`;
          try {
            (ctx as any).strokeStyle = chex ?? "#000000";
          } catch {
            ctx.strokeStyle = "#000000";
          }
          ctx.lineWidth = command.size ?? 5;
          ctx.lineCap = "round";
          ctx.lineTo(command.x!, command.y!);
          ctx.stroke();
          lastExternalPointRef.current = { x: command.x!, y: command.y! };
        }
        break;
      }

      case "end": {
        // Finish external stroke
        externalPathRef.current = null;
        if (external2DRef.current) {
          try {
            (external2DRef.current as any).closePath?.();
          } catch (_e) {
            // Silent close failure is acceptable for cleanup
          }
          external2DRef.current = null;
          lastExternalPointRef.current = null;
        }
        // Ensure final state is rendered
        try {
          renderNow();
        } catch (_e) {
          // Render failures are non-critical for external apply
        }
        break;
      }

      case "clear": {
        if (container) {
          container.removeChildren();
        }
        externalPathRef.current = null;
        // Clear 2D canvas as well
        const cvs = canvasRef.current;
        const ctx = cvs?.getContext("2d");
        if (ctx && cvs) {
          ctx.clearRect(0, 0, cvs.width, cvs.height);
          // Fill white to match Pixi background
          ctx.fillStyle = "#ffffff";
          ctx.fillRect(0, 0, cvs.width, cvs.height);
        }
        external2DRef.current = null;
        lastExternalPointRef.current = null;
        try {
          renderNow();
        } catch (_e) {
          // Render failures are non-critical for external apply
        }
        break;
      }
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
      {/* Drawing Tools (always rendered; disabled when not drawer or when globally disabled) */}
      <div class="drawing-tools mb-4" data-testid="drawing-tools">
        <MobileDrawingTools
          currentTool={currentTool}
          onToolChange={setCurrentTool}
          onClear={clearCanvas}
          onUndo={undo}
          canUndo={undoStack.length > 0}
          disabled={!isDrawer || disabled}
        />
      </div>

      {/* Canvas */}
      <div class="drawing-canvas border-2 border-gray-300 rounded-lg overflow-hidden bg-white">
        <canvas
          ref={canvasRef}
          class={`block w-full h-auto max-w-full ${
            !isDrawer || disabled ? "cursor-not-allowed" : "cursor-crosshair"
          }`}
          style={{
            touchAction: "none",
            maxWidth: "100%",
            height: "auto",
          }}
        />
      </div>

      {/* Status */}
      <div class="mt-2 text-sm text-gray-600">
        {disabled ? "Drawing disabled" : isDrawer ? "You are drawing" : "Watching..."}
      </div>
    </div>
  );
});

export default DrawingEngine;
