// Core WebSocket handler using the new game engine architecture
import { GameRegistry } from "./game-registry.ts";
import { RoomManager } from "./room-manager.ts";
import { BaseClientMessage, BaseServerMessage } from "../../types/core/websocket.ts";
import { BaseGameState } from "../../types/core/game.ts";

export interface WebSocketConnection {
  ws: WebSocket;
  playerId: string;
  roomId: string;
  lastActivity: number;
}

export class CoreWebSocketHandler {
  private connections: Map<string, WebSocketConnection> = new Map();
  private roomConnections: Map<string, Set<string>> = new Map();
  private roomManager: RoomManager;
  private gameRegistry: GameRegistry;
  private gameStates: Map<string, BaseGameState> = new Map();

  constructor() {
    this.roomManager = new RoomManager();
    this.gameRegistry = GameRegistry.getInstance();
  }

  handleWebSocketUpgrade(request: Request): Response {
    const upgradeHeader = request.headers.get("Upgrade");
    if (upgradeHeader !== "websocket") {
      return new Response("Expected Upgrade: websocket", { status: 426 });
    }

    // Check if we're in Cloudflare Workers environment
    if (typeof (globalThis as any).WebSocketPair !== "undefined") {
      return this.handleCloudflareWebSocket(request);
    } else {
      return this.handleDenoWebSocket(request);
    }
  }

  private handleCloudflareWebSocket(_request: Request): Response {
    const webSocketPair = new (globalThis as any).WebSocketPair();
    const [client, server] = Object.values(webSocketPair) as [WebSocket, WebSocket];

    (server as any).accept();

    const connection: WebSocketConnection = {
      ws: server,
      playerId: "",
      roomId: "",
      lastActivity: Date.now(),
    };

    this.setupWebSocketEvents(connection);

    return new Response(
      null,
      {
        status: 101,
        webSocket: client,
      } as ResponseInit & { webSocket: WebSocket },
    );
  }

  private handleDenoWebSocket(request: Request): Response {
    try {
      const { socket, response } = Deno.upgradeWebSocket(request);

      const connection: WebSocketConnection = {
        ws: socket,
        playerId: "",
        roomId: "",
        lastActivity: Date.now(),
      };

      this.setupWebSocketEvents(connection);

      return response;
    } catch (error) {
      console.error("Failed to upgrade WebSocket:", error);
      return new Response("WebSocket upgrade failed", { status: 500 });
    }
  }

  private setupWebSocketEvents(connection: WebSocketConnection): void {
    connection.ws.addEventListener("message", (event: MessageEvent) => {
      this.handleMessage(connection, event.data);
    });

    connection.ws.addEventListener("close", () => {
      this.handleDisconnection(connection);
    });

    connection.ws.addEventListener("error", (error: Event) => {
      console.error("WebSocket error:", error);
      this.handleDisconnection(connection);
    });
  }

  private async handleMessage(connection: WebSocketConnection, data: string): Promise<void> {
    try {
      connection.lastActivity = Date.now();

      if (typeof data !== "string" || data.length === 0) {
        console.error("Invalid message data received");
        return;
      }

      const message: BaseClientMessage = JSON.parse(data);

      // Validate message structure
      if (!message.type || !message.roomId) {
        console.error("Invalid message structure:", message);
        return;
      }

      // Handle core messages
      switch (message.type) {
        case "join-room":
          await this.handleJoinRoom(connection, message);
          break;
        case "leave-room":
          await this.handleLeaveRoom(connection, message);
          break;
        case "chat":
          await this.handleChatMessage(connection, message);
          break;
        case "start-game":
          await this.handleStartGame(connection, message);
          break;
        case "end-game":
          await this.handleEndGame(connection, message);
          break;
        case "update-settings":
          await this.handleUpdateSettings(connection, message);
          break;
        case "ping":
          await this.handlePing(connection, message);
          break;
        default:
          // Delegate to game-specific handler
          await this.handleGameSpecificMessage(connection, message);
          break;
      }
    } catch (error) {
      console.error("Error handling WebSocket message:", error);
      this.sendError(connection, "Invalid message format");
    }
  }

  private async handleJoinRoom(
    connection: WebSocketConnection,
    message: BaseClientMessage,
  ): Promise<void> {
    const { roomId, playerId } = message;
    const { playerName } = message.data;

    try {
      // Join room via room manager
      const result = await this.roomManager.joinRoom({ roomId, playerName });

      if (!result.success || !result.data) {
        this.sendError(connection, result.error || "Failed to join room");
        return;
      }

      // Update connection
      connection.playerId = result.data.playerId;
      connection.roomId = roomId;

      // Add to connection pools
      this.connections.set(result.data.playerId, connection);
      if (!this.roomConnections.has(roomId)) {
        this.roomConnections.set(roomId, new Set());
      }
      this.roomConnections.get(roomId)!.add(result.data.playerId);

      // Send room update to all players
      await this.broadcastToRoom(roomId, {
        type: "room-update",
        roomId,
        data: result.data.room,
      });
    } catch (error) {
      console.error("Error joining room:", error);
      this.sendError(connection, "Failed to join room");
    }
  }

  private async handleLeaveRoom(
    connection: WebSocketConnection,
    message: BaseClientMessage,
  ): Promise<void> {
    const { roomId, playerId } = message;

    try {
      const result = await this.roomManager.leaveRoom(roomId, playerId);

      if (result.success && result.data) {
        // Remove from connection pools
        this.connections.delete(playerId);
        this.roomConnections.get(roomId)?.delete(playerId);

        // Broadcast room update if room still exists
        if (!result.data.roomDeleted) {
          await this.broadcastToRoom(roomId, {
            type: "room-update",
            roomId,
            data: {
              type: "player-left",
              playerId,
              newHostId: result.data.newHostId,
              remainingPlayers: result.data.remainingPlayers,
            },
          });
        }
      }

      // Close connection
      connection.ws.close();
    } catch (error) {
      console.error("Error leaving room:", error);
    }
  }

  private async handleChatMessage(
    connection: WebSocketConnection,
    message: BaseClientMessage,
  ): Promise<void> {
    const { roomId, playerId } = message;
    const { text } = message.data;

    // Create chat message
    const chatMessage = {
      id: `msg_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
      playerId,
      playerName: message.data.playerName || "Unknown",
      message: text,
      timestamp: Date.now(),
      isSystemMessage: false,
    };

    // Broadcast to room
    await this.broadcastToRoom(roomId, {
      type: "chat-message",
      roomId,
      data: chatMessage,
    });
  }

  private async handleStartGame(
    connection: WebSocketConnection,
    message: BaseClientMessage,
  ): Promise<void> {
    const { roomId, playerId } = message;

    try {
      // Get room info
      const room = await this.roomManager.getRoom(roomId);
      if (!room) {
        this.sendError(connection, "Room not found");
        return;
      }

      // Get game engine for this room's game type
      const gameEngine = this.gameRegistry.getGameEngine(room.gameType);
      if (!gameEngine) {
        this.sendError(connection, "Game type not supported");
        return;
      }

      // Get room summary for players
      const roomSummary = await this.roomManager.getRoomSummary(roomId);
      if (!roomSummary.success || !roomSummary.data) {
        this.sendError(connection, "Failed to get room info");
        return;
      }

      // Check if player is host
      const isHost = roomSummary.data.players.find((p) => p.id === playerId)?.isHost;
      if (!isHost) {
        this.sendError(connection, "Only host can start the game");
        return;
      }

      // Get default settings for game type
      const defaultSettings = this.gameRegistry.getDefaultSettings(room.gameType);
      if (!defaultSettings) {
        this.sendError(connection, "Failed to get game settings");
        return;
      }

      // Initialize and start game
      const playerStates = roomSummary.data.players.map((p) => ({
        id: p.id,
        name: p.name,
        isHost: p.isHost,
        isConnected: true,
        lastActivity: Date.now(),
      }));

      let gameState = gameEngine.initializeGame(roomId, playerStates, defaultSettings);
      gameState = gameEngine.startGame(gameState);

      // Store game state
      this.gameStates.set(roomId, gameState);

      // Broadcast game started
      await this.broadcastToRoom(roomId, {
        type: "game-state",
        roomId,
        data: gameState,
      });
    } catch (error) {
      console.error("Error starting game:", error);
      this.sendError(connection, "Failed to start game");
    }
  }

  private async handleEndGame(
    connection: WebSocketConnection,
    message: BaseClientMessage,
  ): Promise<void> {
    const { roomId, playerId } = message;

    try {
      const gameState = this.gameStates.get(roomId);
      if (!gameState) {
        this.sendError(connection, "No active game");
        return;
      }

      // Check if player is host
      const isHost = gameState.players.find((p) => p.id === playerId)?.isHost;
      if (!isHost) {
        this.sendError(connection, "Only host can end the game");
        return;
      }

      // Get game engine and end game
      const gameEngine = this.gameRegistry.getGameEngine(gameState.gameType);
      if (gameEngine) {
        const endedGameState = gameEngine.endGame(gameState);
        this.gameStates.set(roomId, endedGameState);

        await this.broadcastToRoom(roomId, {
          type: "game-state",
          roomId,
          data: endedGameState,
        });
      }
    } catch (error) {
      console.error("Error ending game:", error);
      this.sendError(connection, "Failed to end game");
    }
  }

  private async handleUpdateSettings(
    connection: WebSocketConnection,
    message: BaseClientMessage,
  ): Promise<void> {
    const { roomId, playerId } = message;
    const { settings } = message.data;

    try {
      // Get room and verify host
      const roomSummary = await this.roomManager.getRoomSummary(roomId);
      if (!roomSummary.success || !roomSummary.data) {
        this.sendError(connection, "Room not found");
        return;
      }

      const isHost = roomSummary.data.players.find((p) => p.id === playerId)?.isHost;
      if (!isHost) {
        this.sendError(connection, "Only host can update settings");
        return;
      }

      // Update game state if game is active
      const gameState = this.gameStates.get(roomId);
      if (gameState) {
        gameState.settings = { ...gameState.settings, ...settings };
        this.gameStates.set(roomId, gameState);
      }

      // Broadcast settings update
      await this.broadcastToRoom(roomId, {
        type: "settings-updated",
        roomId,
        data: settings,
      });
    } catch (error) {
      console.error("Error updating settings:", error);
      this.sendError(connection, "Failed to update settings");
    }
  }

  private async handlePing(
    connection: WebSocketConnection,
    message: BaseClientMessage,
  ): Promise<void> {
    // Send pong response
    this.sendMessage(connection, {
      type: "pong",
      roomId: message.roomId,
      data: { timestamp: Date.now() },
    });
  }

  private async handleGameSpecificMessage(
    connection: WebSocketConnection,
    message: BaseClientMessage,
  ): Promise<void> {
    const { roomId } = message;
    const gameState = this.gameStates.get(roomId);

    if (!gameState) {
      this.sendError(connection, "No active game");
      return;
    }

    // Get game engine for this game type
    const gameEngine = this.gameRegistry.getGameEngine(gameState.gameType);
    if (!gameEngine) {
      this.sendError(connection, "Game engine not found");
      return;
    }

    try {
      // Handle message with game engine
      const result = gameEngine.handleClientMessage(gameState, message as any);

      // Update game state
      this.gameStates.set(roomId, result.updatedState);

      // Send server messages
      for (const serverMessage of result.serverMessages) {
        await this.broadcastToRoom(roomId, serverMessage);
      }
    } catch (error) {
      console.error("Error handling game-specific message:", error);
      this.sendError(connection, "Failed to process game action");
    }
  }

  private async handleDisconnection(connection: WebSocketConnection): Promise<void> {
    const { playerId, roomId } = connection;

    if (playerId && roomId) {
      try {
        await this.handleLeaveRoom(connection, {
          type: "leave-room",
          roomId,
          playerId,
          data: {},
        });
      } catch (error) {
        console.error("Error handling disconnection:", error);
      }
    }

    // Clean up connection
    if (playerId) {
      this.connections.delete(playerId);
      this.roomConnections.get(roomId)?.delete(playerId);
    }
  }

  private sendMessage(connection: WebSocketConnection, message: BaseServerMessage): void {
    try {
      if (connection.ws.readyState === WebSocket.OPEN) {
        connection.ws.send(JSON.stringify(message));
      }
    } catch (error) {
      console.error("Error sending message:", error);
    }
  }

  private sendError(connection: WebSocketConnection, error: string): void {
    this.sendMessage(connection, {
      type: "error",
      roomId: connection.roomId || "",
      data: { error },
    });
  }

  private async broadcastToRoom(roomId: string, message: BaseServerMessage): Promise<void> {
    const playerIds = this.roomConnections.get(roomId);
    if (!playerIds) return;

    const promises = Array.from(playerIds).map((playerId) => {
      const connection = this.connections.get(playerId);
      if (connection) {
        this.sendMessage(connection, message);
      }
    });

    await Promise.all(promises);
  }

  // Public utility methods
  getConnectionCount(): number {
    return this.connections.size;
  }

  getRoomConnectionCount(roomId: string): number {
    return this.roomConnections.get(roomId)?.size || 0;
  }

  getActiveRoomIds(): string[] {
    return Array.from(this.roomConnections.keys());
  }

  cleanup(): void {
    // Close all connections
    for (const connection of this.connections.values()) {
      try {
        connection.ws.close();
      } catch (error) {
        console.error("Error closing connection:", error);
      }
    }

    this.connections.clear();
    this.roomConnections.clear();
    this.gameStates.clear();
  }
}
