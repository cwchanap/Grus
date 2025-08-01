// Core WebSocket server implementation
import type { Env } from "../../../types/cloudflare.ts";
import type { WebSocketConnection } from "../types/websocket-internal.ts";
import { ConnectionPool } from "./connection-pool.ts";
import { MessageRouter } from "./message-router.ts";
import { MessageValidator } from "../utils/message-validator.ts";
import { PlayerService } from "../services/player-service.ts";
import { GameStateService } from "../services/game-state-service.ts";
import { TimerService } from "../services/timer-service.ts";
import { WordGenerator } from "../utils/word-generator.ts";
import { RoomHandler } from "../handlers/room-handler.ts";
import { ChatHandler } from "../handlers/chat-handler.ts";
import { DrawingHandler } from "../handlers/drawing-handler.ts";
import { GameHandler } from "../handlers/game-handler.ts";

export class WebSocketServer {
  private env: Env;
  private connectionPool: ConnectionPool;
  private messageRouter: MessageRouter;
  private timerService: TimerService;

  // Services
  private validator: MessageValidator;
  private playerService: PlayerService;
  private gameStateService: GameStateService;
  private wordGenerator: WordGenerator;

  // Handlers
  private roomHandler: RoomHandler;
  private chatHandler: ChatHandler;
  private drawingHandler: DrawingHandler;
  private gameHandler: GameHandler;

  constructor(env: Env) {
    this.env = env;
    this.connectionPool = new ConnectionPool();
    this.messageRouter = new MessageRouter(this.connectionPool);
    this.timerService = new TimerService();

    // Initialize services
    this.validator = new MessageValidator();
    this.playerService = new PlayerService(env);
    this.gameStateService = new GameStateService(env);
    this.wordGenerator = new WordGenerator();

    // Initialize handlers
    this.roomHandler = new RoomHandler(
      this.connectionPool,
      this.validator,
      this.playerService,
      this.gameStateService,
    );

    this.chatHandler = new ChatHandler(
      this.connectionPool,
      this.validator,
      this.gameStateService,
    );

    this.drawingHandler = new DrawingHandler(
      this.connectionPool,
      this.validator,
      this.gameStateService,
    );

    this.gameHandler = new GameHandler(
      this.connectionPool,
      this.playerService,
      this.gameStateService,
      this.timerService,
      this.wordGenerator,
    );

    // Register message handlers
    this.registerHandlers();
  }

  private registerHandlers(): void {
    // Room operations
    this.messageRouter.registerHandler("join-room", this.roomHandler);
    this.messageRouter.registerHandler("leave-room", this.roomHandler);

    // Chat operations
    this.messageRouter.registerHandler("chat", this.chatHandler);
    this.messageRouter.registerHandler("guess", this.chatHandler);

    // Drawing operations
    this.messageRouter.registerHandler("draw", this.drawingHandler);

    // Game operations
    this.messageRouter.registerHandler("start-game", this.gameHandler);
    this.messageRouter.registerHandler("next-round", this.gameHandler);
    this.messageRouter.registerHandler("end-game", this.gameHandler);
    this.messageRouter.registerHandler("ping", this.gameHandler);
  }

  handleWebSocketUpgrade(request: Request): Response {
    const upgradeHeader = request.headers.get("Upgrade");
    if (upgradeHeader !== "websocket") {
      return new Response("Expected Upgrade: websocket", { status: 426 });
    }

    // Check if we're in Cloudflare Workers environment
    if (typeof WebSocketPair !== "undefined") {
      return this.handleCloudflareWebSocket(request);
    } else {
      return this.handleDenoWebSocket(request);
    }
  }

  private handleCloudflareWebSocket(request: Request): Response {
    // Cloudflare Workers environment
    const webSocketPair = new WebSocketPair();
    const [client, server] = Object.values(webSocketPair) as [WebSocket, WebSocket];

    // Accept the WebSocket connection
    server.accept();

    // Create connection object (will be updated when player joins)
    let connection: WebSocketConnection = {
      ws: server,
      playerId: "",
      roomId: "",
      lastActivity: Date.now(),
    };

    // Handle WebSocket events
    server.addEventListener("message", (event: MessageEvent) => {
      this.handleMessage(connection, event.data);
    });

    server.addEventListener("close", () => {
      this.handleDisconnection(connection);
    });

    server.addEventListener("error", (error: Event) => {
      console.error("WebSocket error:", error);
      this.handleDisconnection(connection);
    });

    return new Response(null, {
      status: 101,
      webSocket: client,
    } as ResponseInit & { webSocket: WebSocket });
  }

  private handleDenoWebSocket(request: Request): Response {
    // Deno environment - Use Deno's native WebSocket upgrade
    try {
      const { socket, response } = Deno.upgradeWebSocket(request);

      // Create connection object (will be updated when player joins)
      let connection: WebSocketConnection = {
        ws: socket,
        playerId: "",
        roomId: "",
        lastActivity: Date.now(),
      };

      // Handle WebSocket events
      socket.addEventListener("open", () => {
        // Connection opened successfully
      });

      socket.addEventListener("message", (event: MessageEvent) => {
        this.handleMessage(connection, event.data);
      });

      socket.addEventListener("close", () => {
        this.handleDisconnection(connection);
      });

      socket.addEventListener("error", (error: Event) => {
        console.error("WebSocket error:", error);
        this.handleDisconnection(connection);
      });

      return response;
    } catch (error) {
      console.error("Failed to upgrade WebSocket in Deno environment:", error);
      return new Response("WebSocket upgrade failed", {
        status: 500,
        headers: { "Content-Type": "text/plain" },
      });
    }
  }

  private async handleMessage(connection: WebSocketConnection, data: string): Promise<void> {
    // Update connection activity
    connection.lastActivity = Date.now();

    // Route message through the message router
    await this.messageRouter.routeMessage(connection, data);
  }

  private async handleDisconnection(connection: WebSocketConnection): Promise<void> {
    if (connection.playerId && connection.roomId) {
      // Remove from connection pool (this will handle cleanup)
      const roomId = this.connectionPool.removeConnection(connection.playerId);
      
      if (roomId) {
        // Update player state
        const playerState = await this.gameStateService.getPlayerState(connection.playerId);
        if (playerState) {
          playerState.isConnected = false;
          await this.gameStateService.updatePlayerState(connection.playerId, playerState);
        }

        // Broadcast player left to room
        await this.connectionPool.broadcastToRoom(roomId, {
          type: "room-update",
          roomId,
          data: {
            type: "player-left",
            playerId: connection.playerId,
          },
        });
      }
    }
  }

  // Public utility methods
  getConnectionCount(): number {
    return this.connectionPool.getConnectionCount();
  }

  getRoomConnectionCount(roomId: string): number {
    return this.connectionPool.getRoomConnectionCount(roomId);
  }

  getActiveRoomIds(): string[] {
    return this.connectionPool.getActiveRoomIds();
  }

  async broadcastToRoom(roomId: string, message: any): Promise<void> {
    await this.connectionPool.broadcastToRoom(roomId, message);
  }

  cleanup(): void {
    this.timerService.cleanup();
    this.messageRouter.cleanup();
    this.connectionPool.cleanup();
  }
}