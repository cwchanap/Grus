// WebSocket handler for Cloudflare Workers
import "../../types/websocket.ts";
import { Env } from "../../types/cloudflare.ts";
import {
  ClientMessage,
  GameState,
  PlayerState,
  ServerMessage,
} from "../../types/game.ts";
import {
  getConfig,
  validateChatMessage,
  validatePlayerName,
} from "../config.ts";

export class WebSocketHandler {
  private env: Env;
  private connections: Map<string, WebSocket> = new Map();
  private playerRooms: Map<string, string> = new Map(); // playerId -> roomId
  private roomConnections: Map<string, Set<string>> = new Map(); // roomId -> Set<playerId>
  private rateLimits: Map<
    string,
    { messages: number; drawing: number; lastReset: number }
  > = new Map();
  private roundTimers: Map<string, number> = new Map(); // roomId -> timer

  // Development mode in-memory storage
  private devGameStates: Map<string, GameState> = new Map();
  private devPlayerStates: Map<string, PlayerState> = new Map();

  constructor(env: Env) {
    this.env = env;
  }

  handleWebSocketUpgrade(request: Request): Response {
    const upgradeHeader = request.headers.get("Upgrade");
    if (upgradeHeader !== "websocket") {
      return new Response("Expected Upgrade: websocket", { status: 426 });
    }

    // Check if we're in Cloudflare Workers environment
    if (typeof WebSocketPair !== "undefined") {
      // Cloudflare Workers environment
      const webSocketPair = new WebSocketPair();
      const [client, server] = Object.values(webSocketPair) as [
        WebSocket,
        WebSocket
      ];

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
      // Deno environment - Use Deno's native WebSocket upgrade
      try {
        const { socket, response } = Deno.upgradeWebSocket(request);

        // Handle WebSocket events
        socket.addEventListener("open", () => {
          // Connection opened successfully
        });

        socket.addEventListener("message", (event: MessageEvent) => {
          this.handleMessage(socket, event.data);
        });

        socket.addEventListener("close", () => {
          this.handleDisconnection(socket);
        });

        socket.addEventListener("error", (error: Event) => {
          console.error("WebSocket error:", error);
          this.handleDisconnection(socket);
        });

        return response;
      } catch (error) {
        console.error(
          "Failed to upgrade WebSocket in Deno environment:",
          error
        );
        return new Response("WebSocket upgrade failed", {
          status: 500,
          headers: { "Content-Type": "text/plain" },
        });
      }
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
        case "next-round":
          await this.handleNextRound(ws, message);
          break;
        case "end-game":
          await this.handleEndGame(ws, message);
          break;
        case "ping":
          await this.handlePing(ws, message);
          break;
        default:
          this.sendError(ws, "Unknown message type");
      }
    } catch (error) {
      console.error("WebSocket message error:", error instanceof Error ? error.message : error);
      this.sendError(ws, "Internal server error");
    }
  }

  private validateClientMessage(message: unknown): message is ClientMessage {
    return (
      message !== null &&
      typeof message === "object" &&
      "type" in message &&
      "roomId" in message &&
      "playerId" in message &&
      "data" in message &&
      typeof (message as any).type === "string" &&
      typeof (message as any).roomId === "string" &&
      typeof (message as any).playerId === "string" &&
      (message as any).data !== undefined
    );
  }

  private checkRateLimit(playerId: string, messageType: string): boolean {
    const config = getConfig();
    const now = Date.now();
    const limit = this.rateLimits.get(playerId) || {
      messages: 0,
      drawing: 0,
      lastReset: now,
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

    // Check if player is already in this room to prevent duplicate processing
    if (this.playerRooms.get(playerId) === roomId && this.connections.has(playerId)) {
      return;
    }

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
    try {
      await this.updatePlayerState(playerId, {
        id: playerId,
        name: playerName,
        isHost: false, // Will be determined by room logic
        isConnected: true,
        lastActivity: Date.now(),
      });
    } catch (error) {
      console.error("Error updating player state:", error);
    }

    // Broadcast room update to all players in room
    try {
      await this.broadcastToRoom(roomId, {
        type: "room-update",
        roomId,
        data: {
          type: "player-joined",
          playerId,
          playerName,
        },
      });
    } catch (error) {
      console.error("Error broadcasting room update:", error);
    }

    // Update and send current game state
    try {
      let gameState = await this.getGameState(roomId);
      
      // Small delay to ensure database operations are completed
      await new Promise(resolve => setTimeout(resolve, 50));
      
      // Get all current players in the room (including the new player)
      const allPlayers = await this.getPlayersFromDatabase(roomId);
      
      // If no game state exists, create initial waiting state
      if (!gameState) {
        
        gameState = {
          roomId,
          phase: "waiting",
          roundNumber: 0,
          currentDrawer: "",
          currentWord: "",
          timeRemaining: 0,
          players: allPlayers.map((p) => ({
            id: p.id,
            name: p.name,
            isHost: p.isHost,
            isConnected: true,
            lastActivity: Date.now(),
          })),
          scores: allPlayers.reduce((acc, p) => {
            acc[p.id] = 0;
            return acc;
          }, {} as Record<string, number>),
          drawingData: [],
          correctGuesses: [],
          chatMessages: [],
        };
      } else {
        // Game state exists, update it with current players
        
        // Update players list with all current players
        gameState.players = allPlayers.map((p) => ({
          id: p.id,
          name: p.name,
          isHost: p.isHost,
          isConnected: true,
          lastActivity: Date.now(),
        }));
        
        // Update scores to include new players (preserve existing scores)
        const newScores: Record<string, number> = {};
        allPlayers.forEach((p) => {
          newScores[p.id] = gameState.scores[p.id] || 0;
        });
        gameState.scores = newScores;
      }
      
      // Save the updated game state
      await this.updateGameState(roomId, gameState);
      
      // Broadcast updated game state to ALL players in the room
      await this.broadcastToRoom(roomId, {
        type: "game-state",
        roomId,
        data: gameState,
      });
      
      // Also broadcast a room update to ensure UI updates
      await this.broadcastToRoom(roomId, {
        type: "room-update",
        roomId,
        data: {
          type: "player-joined",
          playerId,
          playerName,
          gameState: gameState,
        },
      });
    } catch (error) {
      console.error("Error getting/sending game state:", error);
    }
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

    // Get current game state
    const gameState = await this.getGameState(roomId);
    if (!gameState) {
      this.sendError(ws, "Game not found");
      return;
    }

    // Create chat message
    const chatMessage = {
      id: crypto.randomUUID(),
      playerId,
      playerName: playerState.name,
      message: text,
      timestamp: Date.now(),
      isGuess: gameState.phase === "drawing", // Only guesses during drawing phase
      isCorrect: false,
    };

    // Check if it's a correct guess
    if (
      gameState.phase === "drawing" &&
      gameState.currentWord &&
      playerId !== gameState.currentDrawer &&
      text.toLowerCase().trim() === gameState.currentWord.toLowerCase()
    ) {
      chatMessage.isCorrect = true;

      try {
        // Update game state for correct guess
        gameState.phase = "results";
        if (!gameState.correctGuesses) {
          gameState.correctGuesses = [];
        }
        gameState.correctGuesses.push({
          playerId,
          timestamp: Date.now(),
        });

        // Update scores
        const basePoints = 100;
        const timeBonus = Math.floor((gameState.timeRemaining / 1000) * 2);
        const totalPoints = basePoints + timeBonus;

        if (!gameState.scores[playerId]) {
          gameState.scores[playerId] = 0;
        }
        gameState.scores[playerId] += totalPoints;

        // Award points to drawer too
        if (
          gameState.currentDrawer &&
          !gameState.scores[gameState.currentDrawer]
        ) {
          gameState.scores[gameState.currentDrawer] = 0;
        }
        if (gameState.currentDrawer) {
          gameState.scores[gameState.currentDrawer] += 50; // Drawer bonus
        }

        await this.updateGameState(roomId, gameState);

        // Clear round timer since round is ending
        this.clearRoundTimer(roomId);

        // Broadcast correct guess and score update
        await this.broadcastToRoom(roomId, {
          type: "game-state",
          roomId,
          data: {
            type: "correct-guess",
            playerId,
            playerName: playerState.name,
            word: gameState.currentWord,
            scores: gameState.scores,
            gameState: gameState,
          },
        });

        // Broadcast chat message with correct flag
        await this.broadcastToRoom(roomId, {
          type: "chat-message",
          roomId,
          data: chatMessage,
        });

        // Auto-advance to results phase after a short delay
        setTimeout(async () => {
          try {
            const currentGameState = await this.getGameState(roomId);
            if (!currentGameState) return;

            currentGameState.phase = "results";
            await this.updateGameState(roomId, currentGameState);

            await this.broadcastToRoom(roomId, {
              type: "game-state",
              roomId,
              data: {
                type: "round-ended",
                reason: "correct-guess",
                gameState: currentGameState,
                scores: currentGameState.scores,
              },
            });

            // Check if game is complete
            if (currentGameState.roundNumber >= 5) {
              // Assuming 5 rounds max
              await this.broadcastToRoom(roomId, {
                type: "game-state",
                roomId,
                data: {
                  type: "game-completed",
                  finalScores: currentGameState.scores,
                },
              });
            }
          } catch (error) {
            console.error("Error ending round after correct guess:", error);
          }
        }, 2000); // 2 second delay to show the correct answer

        return; // Don't send regular chat message since we handled it above
      } catch (error) {
        console.error("Error processing correct guess:", error);
        // Continue to send as regular chat message
      }
    }

    // Broadcast regular chat message to room
    await this.broadcastToRoom(roomId, {
      type: "chat-message",
      roomId,
      data: chatMessage,
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
      timestamp: Date.now(),
    };

    // Update game state with drawing data
    if (gameState.drawingData) {
      gameState.drawingData.push(drawingCommand);
    } else {
      gameState.drawingData = [drawingCommand];
    }

    await this.updateGameState(roomId, gameState);

    // Broadcast drawing update to all players except sender
    await this.broadcastToRoom(
      roomId,
      {
        type: "draw-update",
        roomId,
        data: drawingCommand,
      },
      playerId
    );
  }

  private async handleGuessMessage(_ws: WebSocket, message: ClientMessage) {
    // This is handled in chat message for now
    await this.handleChatMessage(_ws, message);
  }

  private async handleStartGame(ws: WebSocket, message: ClientMessage) {
    const { roomId, playerId } = message;

    try {
      // Verify player is host using database
      const isHost = await this.verifyPlayerIsHost(playerId, roomId);
      if (!isHost) {
        this.sendError(ws, "Only host can start game");
        return;
      }

      // Get players from database
      const players = await this.getPlayersFromDatabase(roomId);
      if (!players || players.length < 2) {
        this.sendError(ws, "Need at least 2 players to start game");
        return;
      }

      // Check if game is already started
      let gameState = await this.getGameState(roomId);
      if (
        gameState &&
        gameState.phase !== "waiting" &&
        gameState.phase !== "finished"
      ) {
        this.sendError(ws, "Game is already in progress");
        return;
      }

      // Initialize new game state
      const firstDrawer = players[0].id;
      gameState = {
        roomId,
        phase: "drawing",
        roundNumber: 1,
        currentDrawer: firstDrawer,
        currentWord: this.getRandomWord(),
        timeRemaining: 90000, // 90 seconds
        players: players.map((p) => ({
          id: p.id,
          name: p.name,
          isHost: p.isHost,
          isConnected: true,
          lastActivity: Date.now(),
        })),
        scores: players.reduce((acc, p) => {
          acc[p.id] = 0;
          return acc;
        }, {} as Record<string, number>),
        drawingData: [],
        correctGuesses: [],
        chatMessages: [],
      };

      // Save game state
      await this.updateGameState(roomId, gameState);

      // Start round timer
      this.startRoundTimer(roomId);

      // Game started successfully
      console.log(`Game started in room ${roomId} by host ${playerId}`);

      // Broadcast game state to all players
      await this.broadcastToRoom(roomId, {
        type: "game-state",
        roomId,
        data: {
          type: "game-started",
          gameState: gameState,
          currentDrawer: gameState.currentDrawer,
          roundNumber: gameState.roundNumber,
          timeRemaining: gameState.timeRemaining,
        },
      });

      // Send word to drawer only
      const drawerWs = this.connections.get(gameState.currentDrawer);
      if (drawerWs) {
        this.sendMessage(drawerWs, {
          type: "game-state",
          roomId,
          data: {
            type: "drawing-word",
            word: gameState.currentWord,
          },
        });
      }

      // Send success response to host
      this.sendMessage(ws, {
        type: "game-state",
        roomId,
        data: {
          type: "game-start-success",
          message: "Game started successfully",
          gameState: gameState,
        },
      });
    } catch (error) {
      console.error("Error starting game:", error);
      this.sendError(ws, "Failed to start game");
    }
  }

  private async handleNextRound(ws: WebSocket, message: ClientMessage) {
    const { roomId, playerId } = message;

    // Verify player is host
    const playerState = await this.getPlayerState(playerId);
    if (!playerState || !playerState.isHost) {
      this.sendError(ws, "Only host can start next round");
      return;
    }

    try {
      const gameState = await this.getGameState(roomId);
      if (!gameState) {
        this.sendError(ws, "Game not found");
        return;
      }

      // Check if we can start next round
      if (gameState.phase !== "results" && gameState.phase !== "waiting") {
        this.sendError(ws, "Cannot start next round during current phase");
        return;
      }

      // Start new round
      gameState.roundNumber += 1;
      gameState.phase = "drawing";
      gameState.timeRemaining = 90000;
      gameState.drawingData = [];
      gameState.correctGuesses = [];

      // Get next drawer
      const activePlayers = gameState.players.filter(
        (p: PlayerState) => p.isConnected
      );
      const currentDrawerIndex = activePlayers.findIndex(
        (p: PlayerState) => p.id === gameState.currentDrawer
      );
      const nextDrawerIndex = (currentDrawerIndex + 1) % activePlayers.length;
      gameState.currentDrawer = activePlayers[nextDrawerIndex].id;
      gameState.currentWord = this.getRandomWord();

      await this.updateGameState(roomId, gameState);

      // Clear existing timer and start new one
      this.clearRoundTimer(roomId);
      this.startRoundTimer(roomId);

      // Broadcast new round state
      await this.broadcastToRoom(roomId, {
        type: "game-state",
        roomId,
        data: {
          type: "round-started",
          gameState: gameState,
          currentDrawer: gameState.currentDrawer,
          roundNumber: gameState.roundNumber,
          timeRemaining: gameState.timeRemaining,
        },
      });

      // Send word to new drawer
      const drawerWs = this.connections.get(gameState.currentDrawer);
      if (drawerWs) {
        this.sendMessage(drawerWs, {
          type: "game-state",
          roomId,
          data: {
            type: "drawing-word",
            word: gameState.currentWord,
          },
        });
      }
    } catch (error) {
      console.error("Error starting next round:", error);
      this.sendError(ws, "Failed to start next round");
    }
  }

  private async handleEndGame(ws: WebSocket, message: ClientMessage) {
    const { roomId, playerId } = message;

    // Verify player is host
    const playerState = await this.getPlayerState(playerId);
    if (!playerState || !playerState.isHost) {
      this.sendError(ws, "Only host can end game");
      return;
    }

    try {
      const gameState = await this.getGameState(roomId);
      if (!gameState) {
        this.sendError(ws, "Game not found");
        return;
      }

      // End the game
      gameState.phase = "finished";
      await this.updateGameState(roomId, gameState);

      // Clear round timer
      this.clearRoundTimer(roomId);

      // Broadcast final results
      await this.broadcastToRoom(roomId, {
        type: "game-state",
        roomId,
        data: {
          type: "game-ended",
          gameState: gameState,
          finalScores: gameState.scores,
        },
      });
    } catch (error) {
      console.error("Error ending game:", error);
      this.sendError(ws, "Failed to end game");
    }
  }

  private async handlePing(ws: WebSocket, message: ClientMessage) {
    const { roomId, playerId, data } = message;
    
    // Send pong response
    this.sendMessage(ws, {
      type: "game-state",
      roomId,
      data: {
        type: "pong",
        playerId,
        timestamp: Date.now(),
        originalTimestamp: data?.timestamp,
      },
    });
  }

  private getRandomWord(): string {
    const words = [
      // Animals
      "cat",
      "dog",
      "bird",
      "fish",
      "elephant",
      "lion",
      "tiger",
      "bear",
      "rabbit",
      "horse",
      "cow",
      "pig",
      "sheep",
      "chicken",
      "duck",
      "frog",
      "snake",
      "turtle",
      "butterfly",
      "bee",

      // Objects
      "house",
      "car",
      "tree",
      "flower",
      "book",
      "chair",
      "table",
      "phone",
      "computer",
      "clock",
      "lamp",
      "door",
      "window",
      "key",
      "bag",
      "hat",
      "shoe",
      "cup",
      "plate",
      "spoon",

      // Food
      "pizza",
      "apple",
      "banana",
      "orange",
      "cake",
      "bread",
      "cheese",
      "ice cream",
      "cookie",
      "sandwich",
      "hamburger",
      "hot dog",
      "pasta",
      "rice",
      "soup",
      "salad",
      "chicken",
      "fish",
      "egg",
      "milk",

      // Nature
      "sun",
      "moon",
      "star",
      "cloud",
      "rain",
      "snow",
      "mountain",
      "ocean",
      "river",
      "forest",
      "beach",
      "island",
      "desert",
      "volcano",
      "rainbow",
      "lightning",
      "wind",
      "fire",
      "earth",
      "sky",

      // Activities
      "running",
      "swimming",
      "dancing",
      "singing",
      "reading",
      "writing",
      "cooking",
      "painting",
      "sleeping",
      "jumping",
      "flying",
      "driving",
      "walking",
      "climbing",
      "fishing",
      "camping",
      "shopping",
      "playing",
      "working",
      "studying",

      // Easy to draw concepts
      "smile",
      "heart",
      "diamond",
      "circle",
      "square",
      "triangle",
      "arrow",
      "cross",
      "checkmark",
      "question mark",
    ];
    return words[Math.floor(Math.random() * words.length)];
  }

  private validateDrawingCommand(data: unknown): boolean {
    return (
      data !== null &&
      typeof data === "object" &&
      "type" in data &&
      typeof (data as any).type === "string" &&
      ["start", "move", "end", "clear"].includes((data as any).type) &&
      ((data as any).type === "clear" ||
        ("x" in data &&
          "y" in data &&
          typeof (data as any).x === "number" &&
          typeof (data as any).y === "number" &&
          (data as any).x >= 0 &&
          (data as any).x <= 1920 && // Max canvas width
          (data as any).y >= 0 &&
          (data as any).y <= 1080)) // Max canvas height
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
        playerId,
      },
    });
  }

  private async broadcastToRoom(
    roomId: string,
    message: ServerMessage,
    excludePlayerId?: string
  ) {
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
      try {
        ws.send(JSON.stringify(message));
      } catch (error) {
        console.error("Error sending message:", error);
      }
    }
  }

  private sendError(ws: WebSocket, error: string) {
    this.sendMessage(ws, {
      type: "room-update",
      roomId: "",
      data: {
        type: "error",
        message: error,
      },
    });
  }

  // Helper methods for data access
  private async checkRoomExists(roomId: string): Promise<boolean> {
    try {
      // In development, we don't have access to Cloudflare DB
      // Use the database service instead
      if (!this.env?.DB) {
        const { getDatabaseService } = await import("../database-factory.ts");
        const db = getDatabaseService();
        const result = await db.getRoomById(roomId);
        return result.success && result.data !== null;
      }

      const stmt = this.env.DB.prepare(
        "SELECT id FROM rooms WHERE id = ? AND is_active = 1"
      );
      const result = await stmt.bind(roomId).first();
      return result !== null;
    } catch (error) {
      console.error("Error checking room existence:", error);
      return false;
    }
  }

  private async verifyPlayerIsHost(
    playerId: string,
    roomId: string
  ): Promise<boolean> {
    try {
      // In development, we don't have access to Cloudflare DB
      // Use the database service instead
      if (!this.env?.DB) {
        const { getDatabaseService } = await import("../database-factory.ts");
        const db = getDatabaseService();
        const result = await db.getPlayerById(playerId);
        return result.success && result.data?.isHost === true;
      }

      const stmt = this.env.DB.prepare(
        "SELECT is_host FROM players WHERE id = ? AND room_id = ?"
      );
      const result = await stmt
        .bind(playerId, roomId)
        .first<{ is_host: number }>();
      return result?.is_host === 1;
    } catch (error) {
      console.error("Error verifying host status:", error);
      return false;
    }
  }

  private async getPlayersFromDatabase(roomId: string): Promise<Array<{
    id: string;
    name: string;
    isHost: boolean;
  }> | null> {
    try {
      // In development, we don't have access to Cloudflare DB
      // Use the database service instead
      if (!this.env?.DB) {
        const { getDatabaseService } = await import("../database-factory.ts");
        const db = getDatabaseService();
        const result = await db.getPlayersByRoom(roomId);
        if (result.success && result.data) {
          return result.data.map((player: any) => ({
            id: player.id,
            name: player.name,
            isHost: player.isHost,
          }));
        }
        return null;
      }

      const stmt = this.env.DB.prepare(
        "SELECT id, name, is_host FROM players WHERE room_id = ? ORDER BY joined_at ASC"
      );
      const results = await stmt.bind(roomId).all<{
        id: string;
        name: string;
        is_host: number;
      }>();

      if (!results.success) {
        console.error("Failed to get players from database:", results.error);
        return null;
      }

      return (results.results || []).map((row) => ({
        id: row.id,
        name: row.name,
        isHost: row.is_host === 1,
      }));
    } catch (error) {
      console.error("Error getting players from database:", error);
      return null;
    }
  }

  private async checkRoomCapacity(roomId: string): Promise<boolean> {
    try {
      // In development, we don't have access to Cloudflare DB
      // Use the database service instead
      if (!this.env?.DB) {
        const { getDatabaseService } = await import("../database-factory.ts");
        const db = getDatabaseService();
        const roomResult = await db.getRoomById(roomId);
        const playersResult = await db.getPlayersByRoom(roomId);

        if (!roomResult.success || !roomResult.data) return false;
        if (!playersResult.success) return false;

        const playerCount = playersResult.data?.length || 0;
        return playerCount < roomResult.data.maxPlayers;
      }

      const roomStmt = this.env.DB.prepare(
        "SELECT max_players FROM rooms WHERE id = ?"
      );
      const room = await roomStmt.bind(roomId).first<{ max_players: number }>();

      if (!room) return false;

      const playerStmt = this.env.DB.prepare(
        "SELECT COUNT(*) as count FROM players WHERE room_id = ?"
      );
      const playerCount = await playerStmt
        .bind(roomId)
        .first<{ count: number }>();

      return (playerCount?.count || 0) < room.max_players;
    } catch (error) {
      console.error("Error checking room capacity:", error);
      return false;
    }
  }

  private async getGameState(roomId: string): Promise<GameState | null> {
    try {
      // In development, use in-memory storage
      if (!this.env?.GAME_STATE) {
        console.log("Development mode: Using in-memory game state storage");
        return this.devGameStates.get(`game:${roomId}`) || null;
      }

      const gameStateStr = await this.env.GAME_STATE.get(`game:${roomId}`);
      return gameStateStr ? JSON.parse(gameStateStr) : null;
    } catch (error) {
      console.error("Error getting game state:", error);
      return null;
    }
  }

  private async updateGameState(
    roomId: string,
    gameState: GameState
  ): Promise<void> {
    try {
      // In development, use in-memory storage
      if (!this.env?.GAME_STATE) {
        console.log("Development mode: Storing game state in memory");
        this.devGameStates.set(`game:${roomId}`, gameState);
        return;
      }

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
      // In development, use in-memory storage
      if (!this.env?.GAME_STATE) {
        console.log("Development mode: Using in-memory player state storage");
        return this.devPlayerStates.get(`player:${playerId}`) || null;
      }

      const playerStateStr = await this.env.GAME_STATE.get(
        `player:${playerId}`
      );
      return playerStateStr ? JSON.parse(playerStateStr) : null;
    } catch (error) {
      console.error("Error getting player state:", error);
      return null;
    }
  }

  private async updatePlayerState(
    playerId: string,
    playerState: PlayerState
  ): Promise<void> {
    try {
      // In development, use in-memory storage
      if (!this.env?.GAME_STATE) {
        console.log("Development mode: Storing player state in memory");
        this.devPlayerStates.set(`player:${playerId}`, playerState);
        return;
      }

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
  async broadcastToRoomPublic(
    _roomId: string,
    message: ServerMessage
  ): Promise<void> {
    await this.broadcastToRoom(_roomId, message);
  }

  private startRoundTimer(roomId: string) {
    // Clear existing timer if any
    this.clearRoundTimer(roomId);

    const timer = setInterval(async () => {
      try {
        const gameState = await this.getGameState(roomId);
        if (!gameState || gameState.phase !== "drawing") {
          this.clearRoundTimer(roomId);
          return;
        }

        // Update time remaining
        const newTimeRemaining = Math.max(0, gameState.timeRemaining - 1000);
        gameState.timeRemaining = newTimeRemaining;

        await this.updateGameState(roomId, gameState);

        // Broadcast time update
        await this.broadcastToRoom(roomId, {
          type: "game-state",
          roomId,
          data: {
            type: "time-update",
            timeRemaining: newTimeRemaining,
          },
        });

        // End round if time is up
        if (newTimeRemaining <= 0) {
          await this.handleRoundTimeout(roomId);
        }
      } catch (error) {
        console.error("Error in round timer:", error);
        this.clearRoundTimer(roomId);
      }
    }, 1000); // Update every second

    this.roundTimers.set(roomId, timer);
  }

  private clearRoundTimer(roomId: string) {
    const timer = this.roundTimers.get(roomId);
    if (timer) {
      clearInterval(timer);
      this.roundTimers.delete(roomId);
    }
  }

  private async handleRoundTimeout(roomId: string) {
    try {
      this.clearRoundTimer(roomId);

      const gameState = await this.getGameState(roomId);
      if (!gameState) return;

      gameState.phase = "results";
      await this.updateGameState(roomId, gameState);

      // Broadcast round ended
      await this.broadcastToRoom(roomId, {
        type: "game-state",
        roomId,
        data: {
          type: "round-ended",
          reason: "timeout",
          gameState: gameState,
          scores: gameState.scores,
        },
      });

      // If game is complete, broadcast final results
      if (gameState.phase === "results" && gameState.roundNumber >= 5) {
        await this.broadcastToRoom(roomId, {
          type: "game-state",
          roomId,
          data: {
            type: "game-completed",
            finalScores: gameState.scores,
          },
        });
      }
    } catch (error) {
      console.error("Error handling round timeout:", error);
    }
  }

  // Cleanup method for graceful shutdown
  cleanup(): void {
    // Clear all round timers
    for (const [roomId, timer] of this.roundTimers.entries()) {
      clearInterval(timer);
    }
    this.roundTimers.clear();

    // Close all connections
    for (const [playerId, ws] of this.connections.entries()) {
      try {
        ws.close(1000, "Server shutdown");
      } catch (error) {
        console.error(
          `Error closing connection for player ${playerId}:`,
          error
        );
      }
    }

    // Clear all maps
    this.connections.clear();
    this.playerRooms.clear();
    this.roomConnections.clear();
    this.rateLimits.clear();
  }
}
