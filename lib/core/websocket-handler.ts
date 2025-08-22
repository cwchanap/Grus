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

    // Use Deno's native WebSocket upgrade exclusively
    return this.handleDenoWebSocket(request);
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

      // Validate message structure - some messages like subscribe-lobby don't need roomId
      if (!message.type) {
        console.error("Invalid message structure:", message);
        return;
      }

      // Messages that don't require roomId
      const noRoomIdRequired = ["subscribe-lobby", "ping"];
      if (!noRoomIdRequired.includes(message.type) && !message.roomId) {
        console.error("Invalid message structure:", message);
        return;
      }

      // Handle core messages
      switch (message.type) {
        case "subscribe-lobby":
          await this.handleSubscribeLobby(connection, message);
          break;
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
    const { playerName } = message.data || {};

    try {
      // Get current room summary to detect existing players
      const roomSummary = await this.roomManager.getRoomSummary(roomId);
      if (!roomSummary.success || !roomSummary.data) {
        this.sendError(connection, roomSummary.error || "Room not found");
        return;
      }

      const players = roomSummary.data.players || [];
      const byId = playerId ? players.find((p: any) => p.id === playerId) : undefined;
      const byName = !byId && playerName
        ? players.find((p: any) => p.name?.trim().toLowerCase() === playerName.trim().toLowerCase())
        : undefined;

      if (byId || byName) {
        // Existing player connecting (or reconnecting) — register this socket without creating a new player
        const effectiveId = (byId || byName)!.id as string;
        connection.playerId = effectiveId;
        connection.roomId = roomId;

        // Register in connection pools
        this.connections.set(effectiveId, connection);
        if (!this.roomConnections.has(roomId)) {
          this.roomConnections.set(roomId, new Set());
        }
        this.roomConnections.get(roomId)!.add(effectiveId);

        // Send current room summary directly to this client
        this.sendMessage(connection, {
          type: "room-update",
          roomId,
          data: roomSummary.data,
        });
        // Also broadcast to ensure all clients have a consistent, up-to-date player list
        await this.broadcastToRoom(roomId, {
          type: "room-update",
          roomId,
          data: roomSummary.data,
        });
        return;
      }

      // No existing player matched — create a new player entry via RoomManager
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

      // Send room update to the joining player immediately
      this.sendMessage(connection, {
        type: "room-update",
        roomId,
        data: result.data.room,
      });
      // Broadcast room update to all players as well
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
          // Send a full room summary to keep clients in sync with a consistent payload shape
          const updatedRoom = await this.roomManager.getRoomSummary(roomId);
          if (updatedRoom.success && updatedRoom.data) {
            await this.broadcastToRoom(roomId, {
              type: "room-update",
              roomId,
              data: updatedRoom.data,
            });
          } else {
            console.error("Failed to get updated room after leave:", updatedRoom.error);
          }
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

    // Ensure the sender receives the message even if join hasn't fully completed yet
    // (race between client sending chat and server adding the connection to room membership)
    const roomSet = this.roomConnections.get(roomId);
    const senderInRoom = !!(connection.playerId && roomSet && roomSet.has(connection.playerId));
    if (!senderInRoom) {
      this.sendMessage(connection, {
        type: "chat-message",
        roomId,
        data: chatMessage,
      });
    }
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

  private async handleSubscribeLobby(
    connection: WebSocketConnection,
    _message: BaseClientMessage,
  ): Promise<void> {
    try {
      // Get active rooms from room manager
      const result = await this.roomManager.getActiveRoomsWithCleanup(20);

      if (!result.success) {
        this.sendError(connection, result.error || "Failed to get lobby data");
        return;
      }

      // Send lobby data to client
      this.sendMessage(connection, {
        type: "lobby-data",
        roomId: "", // No specific room for lobby data
        data: {
          rooms: result.data || [],
          timestamp: Date.now(),
        },
      });
    } catch (error) {
      console.error("Error handling subscribe-lobby:", error);
      this.sendError(connection, "Failed to subscribe to lobby");
    }
  }

  private handlePing(
    connection: WebSocketConnection,
    message: BaseClientMessage,
  ): void {
    // Send pong response
    this.sendMessage(connection, {
      type: "pong",
      roomId: message.roomId || "",
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

    try {
      console.log(
        `[WS] Broadcast ${message.type} to room ${roomId} -> recipients: ${playerIds.size}`,
        Array.from(playerIds),
      );

      const promises = Array.from(playerIds).map((pid) => {
        const conn = this.connections.get(pid);
        if (!conn) {
          console.warn(`[WS] No active connection found for player ${pid} in room ${roomId}`);
          return Promise.resolve();
        }
        try {
          this.sendMessage(conn, message);
        } catch (err) {
          console.error(`[WS] Failed to send to ${pid} in room ${roomId}:`, err);
        }
        return Promise.resolve();
      });

      await Promise.all(promises);
    } catch (e) {
      console.error(`[WS] Broadcast error for room ${roomId}:`, e);
    }
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
