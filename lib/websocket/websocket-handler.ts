// WebSocket handler for Cloudflare Workers
import "../../types/websocket.ts";
import { Env } from "../../types/cloudflare.ts";
import { ClientMessage, ServerMessage, GameState, PlayerState } from "../../types/game.ts";
import { getConfig, validateChatMessage, validatePlayerName } from "../config.ts";

export class WebSocketHandler {
  private env: Env;
  private connections: Map<string, WebSocket> = new Map();
  private playerRooms: Map<string, string> = new Map(); // playerId -> roomId
  private roomConnections: Map<string, Set<string>> = new Map(); // roomId -> Set<playerId>
  private rateLimits: Map<string, { messages: number; drawing: number; lastReset: number }> = new Map();

  constructor(env: Env) {
    this.env = env;
  }

  async handleWebSocketUpgrade(request: Request): Promise<Response> {
    const upgradeHeader = request.headers.get("Upgrade");
    if (upgradeHeader !== "websocket") {
      return new Response("Expected Upgrade: websocket", { status: 426 });
    }

    // Check if we're in Cloudflare Workers environment
    if (typeof WebSocketPair !== 'undefined') {
      // Cloudflare Workers environment
      const webSocketPair = new WebSocketPair();
      const [client, server] = Object.values(webSocketPair) as [WebSocket, WebSocket];

      // Accept the WebSocket connection
      server.accept();

      // Handle WebSocket events
      server.addEventListener("message", (event: MessageEvent) => {
        this.handleMessage(server, event.data);
      });

      server.addEventListener("close", () => {
        this.handleDisconnection(server);
      });

      server.addEventListener("error", (error: Event) => {
        console.error("WebSocket error:", error);
        this.handleDisconnection(server);
      });

      return new Response(null, {
        status: 101,
        webSocket: client,
      } as ResponseInit & { webSocket: WebSocket });
    } else {
      // Deno environment - WebSocket upgrade is handled differently
      // For development, we'll return a placeholder response
      console.log("WebSocket connection attempted in Deno environment - not fully supported in development");
      return new Response("WebSocket not supported in development environment", { 
        status: 501,
        headers: { "Content-Type": "text/plain" }
      });
    }
  }

  private async handleMessage(ws: WebSocket, data: string) {
    try {
      const message: ClientMessage = JSON.parse(data);
      
      // Validate message structure
      if (!this.validateClientMessage(message)) {
        this.sendError(ws, "Invalid message format");
        return;
      }

      // Check rate limits
      if (!this.checkRateLimit(message.playerId, message.type)) {
        this.sendError(ws, "Rate limit exceeded");
        return;
      }

      // Handle different message types
      switch (message.type) {
        case "join-room":
          await this.handleJoinRoom(ws, message);
          break;
        case "leave-room":
          await this.handleLeaveRoom(ws, message);
          break;
        case "chat":
          await this.handleChatMessage(ws, message);
          break;
        case "draw":
          await this.handleDrawMessage(ws, message);
          break;
        case "guess":
          await this.handleGuessMessage(ws, message);
          break;
        case "start-game":
          await this.handleStartGame(ws, message);
          break;
        default:
          this.sendError(ws, "Unknown message type");
      }
    } catch (error) {
      console.error("Error handling message:", error);
      this.sendError(ws, "Internal server error");
    }
  }

  private validateClientMessage(message: any): message is ClientMessage {
    return (
      typeof message === "object" &&
      typeof message.type === "string" &&
      typeof message.roomId === "string" &&
      typeof message.playerId === "string" &&
      message.data !== undefined
    );
  }

  private checkRateLimit(playerId: string, messageType: string): boolean {
    const config = getConfig();
    const now = Date.now();
    const limit = this.rateLimits.get(playerId) || { 
      messages: 0, 
      drawing: 0, 
      lastReset: now 
    };

    // Reset counters every minute
    if (now - limit.lastReset > 60000) {
      limit.messages = 0;
      limit.drawing = 0;
      limit.lastReset = now;
    }

    // Check limits based on message type
    if (messageType === "draw") {
      if (limit.drawing >= config.security.rateLimitDrawing) {
        return false;
      }
      limit.drawing++;
    } else {
      if (limit.messages >= config.security.rateLimitMessages) {
        return false;
      }
      limit.messages++;
    }

    this.rateLimits.set(playerId, limit);
    return true;
  }

  private async handleJoinRoom(ws: WebSocket, message: ClientMessage) {
    const { roomId, playerId, data } = message;
    const { playerName } = data;

    // Validate player name
    if (!validatePlayerName(playerName)) {
      this.sendError(ws, "Invalid player name");
      return;
    }

    // Check if room exists and has capacity
    const roomExists = await this.checkRoomExists(roomId);
    if (!roomExists) {
      this.sendError(ws, "Room not found");
      return;
    }

    const roomCapacity = await this.checkRoomCapacity(roomId);
    if (!roomCapacity) {
      this.sendError(ws, "Room is full");
      return;
    }

    // Add connection to room
    this.connections.set(playerId, ws);
    this.playerRooms.set(playerId, roomId);
    
    if (!this.roomConnections.has(roomId)) {
      this.roomConnections.set(roomId, new Set());
    }
    this.roomConnections.get(roomId)!.add(playerId);

    // Update player state in KV
    await this.updatePlayerState(playerId, {
      id: playerId,
      name: playerName,
      isHost: false, // Will be determined by room logic
      isConnected: true,
      lastActivity: Date.now()
    });

    // Broadcast room update to all players in room
    await this.broadcastToRoom(roomId, {
      type: "room-update",
      roomId,
      data: {
        type: "player-joined",
        playerId,
        playerName
      }
    });

    // Send current game state to new player
    const gameState = await this.getGameState(roomId);
    this.sendMessage(ws, {
      type: "game-state",
      roomId,
      data: gameState
    });
  }

  private async handleLeaveRoom(ws: WebSocket, message: ClientMessage) {
    const { roomId, playerId } = message;
    await this.removePlayerFromRoom(playerId, roomId);
  }

  private async handleChatMessage(ws: WebSocket, message: ClientMessage) {
    const { roomId, playerId, data } = message;
    const { text } = data;

    if (!validateChatMessage(text)) {
      this.sendError(ws, "Invalid chat message");
      return;
    }

    // Get player info
    const playerState = await this.getPlayerState(playerId);
    if (!playerState) {
      this.sendError(ws, "Player not found");
      return;
    }

    // Create chat message
    const chatMessage = {
      id: crypto.randomUUID(),
      playerId,
      playerName: playerState.name,
      message: text,
      timestamp: Date.now(),
      isGuess: true, // All chat messages are potential guesses
      isCorrect: false
    };

    // Check if it's a correct guess (this would integrate with game logic)
    const gameState = await this.getGameState(roomId);
    if (gameState && gameState.currentWord && 
        text.toLowerCase().trim() === gameState.currentWord.toLowerCase()) {
      chatMessage.isCorrect = true;
      // Handle scoring logic here
    }

    // Broadcast chat message to room
    await this.broadcastToRoom(roomId, {
      type: "chat-message",
      roomId,
      data: chatMessage
    });
  }

  private async handleDrawMessage(ws: WebSocket, message: ClientMessage) {
    const { roomId, playerId, data } = message;
    
    // Verify player is current drawer
    const gameState = await this.getGameState(roomId);
    if (!gameState || gameState.currentDrawer !== playerId) {
      this.sendError(ws, "Not your turn to draw");
      return;
    }

    // Validate drawing command
    if (!this.validateDrawingCommand(data)) {
      this.sendError(ws, "Invalid drawing command");
      return;
    }

    // Add timestamp to drawing command
    const drawingCommand = {
      ...data,
      timestamp: Date.now()
    };

    // Update game state with drawing data
    if (gameState.drawingData) {
      gameState.drawingData.push(drawingCommand);
    } else {
      gameState.drawingData = [drawingCommand];
    }
    
    await this.updateGameState(roomId, gameState);

    // Broadcast drawing update to all players except sender
    await this.broadcastToRoom(roomId, {
      type: "draw-update",
      roomId,
      data: drawingCommand
    }, playerId);
  }

  private async handleGuessMessage(ws: WebSocket, message: ClientMessage) {
    // This is handled in chat message for now
    await this.handleChatMessage(ws, message);
  }

  private async handleStartGame(ws: WebSocket, message: ClientMessage) {
    const { roomId, playerId } = message;
    
    // Verify player is host
    const playerState = await this.getPlayerState(playerId);
    if (!playerState || !playerState.isHost) {
      this.sendError(ws, "Only host can start game");
      return;
    }

    // Start game logic would go here
    // For now, just broadcast game start
    await this.broadcastToRoom(roomId, {
      type: "game-state",
      roomId,
      data: {
        type: "game-started",
        startedBy: playerId
      }
    });
  }

  private validateDrawingCommand(data: any): boolean {
    return (
      typeof data === "object" &&
      typeof data.type === "string" &&
      ["start", "move", "end", "clear"].includes(data.type) &&
      (data.type === "clear" || (
        typeof data.x === "number" &&
        typeof data.y === "number" &&
        data.x >= 0 && data.x <= 1920 && // Max canvas width
        data.y >= 0 && data.y <= 1080   // Max canvas height
      ))
    );
  }

  private async handleDisconnection(ws: WebSocket) {
    // Find player by WebSocket connection
    let disconnectedPlayerId: string | null = null;
    for (const [playerId, connection] of this.connections.entries()) {
      if (connection === ws) {
        disconnectedPlayerId = playerId;
        break;
      }
    }

    if (disconnectedPlayerId) {
      const roomId = this.playerRooms.get(disconnectedPlayerId);
      if (roomId) {
        await this.removePlayerFromRoom(disconnectedPlayerId, roomId);
      }
    }
  }

  private async removePlayerFromRoom(playerId: string, roomId: string) {
    // Remove from connection maps
    this.connections.delete(playerId);
    this.playerRooms.delete(playerId);
    
    const roomConnections = this.roomConnections.get(roomId);
    if (roomConnections) {
      roomConnections.delete(playerId);
      if (roomConnections.size === 0) {
        this.roomConnections.delete(roomId);
      }
    }

    // Update player state
    const playerState = await this.getPlayerState(playerId);
    if (playerState) {
      playerState.isConnected = false;
      await this.updatePlayerState(playerId, playerState);
    }

    // Broadcast player left to room
    await this.broadcastToRoom(roomId, {
      type: "room-update",
      roomId,
      data: {
        type: "player-left",
        playerId
      }
    });
  }

  private async broadcastToRoom(roomId: string, message: ServerMessage, excludePlayerId?: string) {
    const roomConnections = this.roomConnections.get(roomId);
    if (!roomConnections) return;

    const messageStr = JSON.stringify(message);
    
    for (const playerId of roomConnections) {
      if (excludePlayerId && playerId === excludePlayerId) continue;
      
      const ws = this.connections.get(playerId);
      if (ws && ws.readyState === WebSocket.OPEN) {
        try {
          ws.send(messageStr);
        } catch (error) {
          console.error(`Error sending message to player ${playerId}:`, error);
          // Remove failed connection
          await this.removePlayerFromRoom(playerId, roomId);
        }
      }
    }
  }

  private sendMessage(ws: WebSocket, message: ServerMessage) {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(message));
    }
  }

  private sendError(ws: WebSocket, error: string) {
    this.sendMessage(ws, {
      type: "room-update",
      roomId: "",
      data: {
        type: "error",
        message: error
      }
    });
  }

  // Helper methods for data access
  private async checkRoomExists(roomId: string): Promise<boolean> {
    try {
      const stmt = this.env.DB.prepare("SELECT id FROM rooms WHERE id = ? AND is_active = 1");
      const result = await stmt.bind(roomId).first();
      return result !== null;
    } catch (error) {
      console.error("Error checking room existence:", error);
      return false;
    }
  }

  private async checkRoomCapacity(roomId: string): Promise<boolean> {
    try {
      const roomStmt = this.env.DB.prepare("SELECT max_players FROM rooms WHERE id = ?");
      const room = await roomStmt.bind(roomId).first<{ max_players: number }>();
      
      if (!room) return false;

      const playerStmt = this.env.DB.prepare("SELECT COUNT(*) as count FROM players WHERE room_id = ?");
      const playerCount = await playerStmt.bind(roomId).first<{ count: number }>();
      
      return (playerCount?.count || 0) < room.max_players;
    } catch (error) {
      console.error("Error checking room capacity:", error);
      return false;
    }
  }

  private async getGameState(roomId: string): Promise<GameState | null> {
    try {
      const gameStateStr = await this.env.GAME_STATE.get(`game:${roomId}`);
      return gameStateStr ? JSON.parse(gameStateStr) : null;
    } catch (error) {
      console.error("Error getting game state:", error);
      return null;
    }
  }

  private async updateGameState(roomId: string, gameState: GameState): Promise<void> {
    try {
      const config = getConfig();
      await this.env.GAME_STATE.put(
        `game:${roomId}`, 
        JSON.stringify(gameState),
        { expirationTtl: config.kv.defaultTtl }
      );
    } catch (error) {
      console.error("Error updating game state:", error);
    }
  }

  private async getPlayerState(playerId: string): Promise<PlayerState | null> {
    try {
      const playerStateStr = await this.env.GAME_STATE.get(`player:${playerId}`);
      return playerStateStr ? JSON.parse(playerStateStr) : null;
    } catch (error) {
      console.error("Error getting player state:", error);
      return null;
    }
  }

  private async updatePlayerState(playerId: string, playerState: PlayerState): Promise<void> {
    try {
      const config = getConfig();
      await this.env.GAME_STATE.put(
        `player:${playerId}`, 
        JSON.stringify(playerState),
        { expirationTtl: config.kv.defaultTtl }
      );
    } catch (error) {
      console.error("Error updating player state:", error);
    }
  }

  // Public utility methods for WebSocketManager
  getConnectionCount(): number {
    return this.connections.size;
  }

  getRoomConnectionCount(roomId: string): number {
    const roomConnections = this.roomConnections.get(roomId);
    return roomConnections ? roomConnections.size : 0;
  }

  getActiveRoomIds(): string[] {
    return Array.from(this.roomConnections.keys());
  }

  // Public method to allow manager to broadcast to specific room
  async broadcastToRoomPublic(roomId: string, message: any): Promise<void> {
    await this.broadcastToRoom(roomId, message);
  }

  // Cleanup method for graceful shutdown
  async cleanup(): Promise<void> {
    // Close all connections
    for (const [playerId, ws] of this.connections.entries()) {
      try {
        ws.close(1000, "Server shutdown");
      } catch (error) {
        console.error(`Error closing connection for player ${playerId}:`, error);
      }
    }

    // Clear all maps
    this.connections.clear();
    this.playerRooms.clear();
    this.roomConnections.clear();
    this.rateLimits.clear();
  }
}