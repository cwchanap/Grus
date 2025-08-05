import { Handlers } from "$fresh/server.ts";
import { RoomManager } from "../../../lib/room-manager.ts";
import { GameState } from "../../../types/game.ts";
import { getConfig } from "../../../lib/config.ts";
import { Env } from "../../../types/cloudflare.ts";

interface StartGameRequest {
  roomId: string;
  playerId: string;
}

interface StartGameResponse {
  success: boolean;
  error?: string;
  gameState?: GameState;
}

export const handler: Handlers = {
  async POST(req, ctx) {
    try {
      const body: StartGameRequest = await req.json();
      const { roomId, playerId } = body;

      if (!roomId || !playerId) {
        return new Response(
          JSON.stringify({
            success: false,
            error: "Room ID and Player ID are required",
          }),
          {
            status: 400,
            headers: { "Content-Type": "application/json" },
          },
        );
      }

      const roomManager = new RoomManager();

      // Get room summary to verify room exists and get players
      const roomResult = await roomManager.getRoomSummary(roomId);
      if (!roomResult.success || !roomResult.data) {
        return new Response(
          JSON.stringify({
            success: false,
            error: "Room not found",
          }),
          {
            status: 404,
            headers: { "Content-Type": "application/json" },
          },
        );
      }

      const { room: _room, players, host } = roomResult.data;

      // Verify player is host
      if (!host || host.id !== playerId) {
        return new Response(
          JSON.stringify({
            success: false,
            error: "Only host can start game",
          }),
          {
            status: 403,
            headers: { "Content-Type": "application/json" },
          },
        );
      }

      // Check minimum players
      if (players.length < 2) {
        return new Response(
          JSON.stringify({
            success: false,
            error: "Need at least 2 players to start game",
          }),
          {
            status: 400,
            headers: { "Content-Type": "application/json" },
          },
        );
      }

      // Initialize game state
      const firstDrawer = players[0].id;
      const gameState: GameState = {
        roomId,
        phase: "drawing",
        roundNumber: 1,
        currentDrawer: firstDrawer,
        currentWord: getRandomWord(),
        timeRemaining: 90000, // 90 seconds
        players: players.map((p: any) => ({
          id: p.id,
          name: p.name,
          isHost: p.isHost,
          isConnected: true,
          lastActivity: Date.now(),
        })),
        scores: players.reduce((acc: any, p: any) => {
          acc[p.id] = 0;
          return acc;
        }, {} as Record<string, number>),
        drawingData: [],
        correctGuesses: [],
        chatMessages: [],
        settings: { maxRounds: 5, roundTimeSeconds: 75 },
      };

      // In development, we can't use KV store, so we'll just return the game state
      // In production, this would be stored in Cloudflare KV
      const env = (ctx.state as any).env as Env;
      if (env?.GAME_STATE) {
        try {
          const config = getConfig();
          await env.GAME_STATE.put(
            `game:${roomId}`,
            JSON.stringify(gameState),
            { expirationTtl: config.kv.defaultTtl },
          );
        } catch (error) {
          console.error("Error storing game state in KV:", error);
          // Continue anyway for development
        }
      }

      console.log(`Game started in room ${roomId} by host ${playerId} via REST API`);

      return new Response(
        JSON.stringify({
          success: true,
          gameState,
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      );
    } catch (error) {
      console.error("Error starting game via REST API:", error);
      return new Response(
        JSON.stringify({
          success: false,
          error: "Internal server error",
        }),
        {
          status: 500,
          headers: { "Content-Type": "application/json" },
        },
      );
    }
  },
};

function getRandomWord(): string {
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
