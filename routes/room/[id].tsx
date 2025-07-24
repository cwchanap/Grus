import { Handlers, PageProps } from "$fresh/server.ts";
import { RoomManager } from "../../lib/room-manager.ts";
import { Env } from "../../types/cloudflare.ts";
import type { RoomSummary } from "../../lib/room-manager.ts";
import type { GameState } from "../../types/game.ts";
import ChatRoom from "../../islands/ChatRoom.tsx";
import DrawingBoard from "../../islands/DrawingBoard.tsx";
import Scoreboard from "../../islands/Scoreboard.tsx";

interface GameRoomData {
  room: RoomSummary | null;
  playerId: string | null;
  error?: string;
}

// Helper function to create initial game state from room data
function createInitialGameState(room: RoomSummary): GameState {
  return {
    roomId: room.room.id,
    currentDrawer: '', // No drawer initially
    currentWord: '', // No word initially
    roundNumber: 0, // Game hasn't started
    timeRemaining: 120000, // 2 minutes default
    phase: 'waiting', // Waiting for game to start
    players: room.players.map(player => ({
      id: player.id,
      name: player.name,
      isHost: player.isHost,
      isConnected: true, // Assume all players in room are connected
      lastActivity: Date.now()
    })),
    scores: room.players.reduce((acc, player) => {
      acc[player.id] = 0; // Initialize all scores to 0
      return acc;
    }, {} as Record<string, number>),
    drawingData: [] // No drawing data initially
  };
}

export const handler: Handlers<GameRoomData> = {
  async GET(req, ctx) {
    try {
      const roomId = ctx.params.id;
      const url = new URL(req.url);
      const playerId = url.searchParams.get("playerId");
      
      const env = (ctx.state as any).env as Env;
      
      if (!env?.DB) {
        return ctx.render({ 
          room: null, 
          playerId,
          error: "Database not available" 
        });
      }

      const roomManager = new RoomManager(env.DB);
      const result = await roomManager.getRoomSummary(roomId);

      if (!result.success || !result.data) {
        return ctx.render({ 
          room: null, 
          playerId,
          error: result.error || "Room not found" 
        });
      }

      return ctx.render({ 
        room: result.data,
        playerId
      });
    } catch (error) {
      console.error("Error loading game room:", error);
      return ctx.render({ 
        room: null, 
        playerId: null,
        error: "Failed to load game room" 
      });
    }
  }
};

export default function GameRoom({ data }: PageProps<GameRoomData>) {
  if (data.error || !data.room) {
    return (
      <div class="min-h-screen bg-gradient-to-br from-red-50 to-red-100 flex items-center justify-center">
        <div class="max-w-md mx-auto text-center">
          <div class="bg-white rounded-lg shadow-lg p-8">
            <div class="text-red-600 text-6xl mb-4">⚠️</div>
            <h1 class="text-2xl font-bold text-gray-800 mb-2">Room Not Found</h1>
            <p class="text-gray-600 mb-6">
              {data.error || "The room you're looking for doesn't exist or is no longer available."}
            </p>
            <a
              href="/"
              class="inline-block px-6 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
            >
              Back to Lobby
            </a>
          </div>
        </div>
      </div>
    );
  }

  const { room, playerId } = data;
  const gameState = createInitialGameState(room);

  return (
    <div class="min-h-screen bg-gradient-to-br from-purple-50 to-pink-100">
      <div class="container mx-auto px-4 py-8">
        {/* Header */}
        <div class="bg-white rounded-lg shadow-md p-6 mb-6">
          <div class="flex justify-between items-center">
            <div>
              <h1 class="text-2xl font-bold text-gray-800">{room.room.name}</h1>
              <p class="text-gray-600">
                Host: {room.host?.name || 'Unknown'} • 
                Players: {room.playerCount}/{room.room.maxPlayers}
              </p>
            </div>
            <a
              href="/"
              class="px-4 py-2 bg-gray-100 text-gray-700 rounded-lg hover:bg-gray-200 transition-colors"
            >
              ← Back to Lobby
            </a>
          </div>
        </div>

        {/* Game area */}
        <div class="grid grid-cols-1 lg:grid-cols-4 gap-6">
          {/* Drawing board area */}
          <div class="lg:col-span-3">
            <div class="bg-white rounded-lg shadow-md p-6">
              <DrawingBoard
                roomId={room.room.id}
                playerId={playerId || ''}
                gameState={gameState}
                width={800}
                height={500}
                className="w-full"
              />
            </div>
          </div>

          {/* Sidebar */}
          <div class="space-y-6">
            {/* Chat area */}
            <div class="bg-white rounded-lg shadow-md p-6 h-80">
              <ChatRoom
                roomId={room.room.id}
                playerId={playerId || ''}
                playerName={room.players.find(p => p.id === playerId)?.name || 'Unknown'}
                currentWord={gameState.currentWord}
                isCurrentDrawer={gameState.currentDrawer === playerId}
              />
            </div>

            {/* Scoreboard */}
            <div class="bg-white rounded-lg shadow-md p-6">
              <Scoreboard
                roomId={room.room.id}
                playerId={playerId || ''}
                gameState={gameState}
                className="h-full"
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}