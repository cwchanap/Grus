import { Handlers, PageProps } from "$fresh/server.ts";
import { RoomManager } from "../../lib/room-manager.ts";
import { Env } from "../../types/cloudflare.ts";
import type { RoomSummary } from "../../lib/room-manager.ts";
import type { GameState } from "../../types/game.ts";
import ChatRoom from "../../islands/ChatRoom.tsx";
import DrawingBoard from "../../islands/DrawingBoard.tsx";
import Scoreboard from "../../islands/Scoreboard.tsx";
import LeaveRoomButton from "../../islands/LeaveRoomButton.tsx";

interface GameRoomData {
  room: RoomSummary | null;
  playerId: string | null;
  error?: string;
}

// Helper function to create initial game state from room data
function createInitialGameState(room: RoomSummary): GameState {
  return {
    roomId: room.room.id,
    currentDrawer: "", // No drawer initially
    currentWord: "", // No word initially
    roundNumber: 0, // Game hasn't started
    timeRemaining: 120000, // 2 minutes default
    phase: "waiting", // Waiting for game to start
    players: room.players.map((player: any) => ({
      id: player.id,
      name: player.name,
      isHost: player.isHost,
      isConnected: true, // Assume all players in room are connected
      lastActivity: Date.now(),
    })),
    scores: room.players.reduce((acc: any, player: any) => {
      acc[player.id] = 0; // Initialize all scores to 0
      return acc;
    }, {} as Record<string, number>),
    drawingData: [], // No drawing data initially
    correctGuesses: [], // No correct guesses initially
    chatMessages: [], // No chat messages initially
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
          error: "Database not available",
        });
      }

      const roomManager = new RoomManager();
      const result = await roomManager.getRoomSummary(roomId);

      if (!result.success || !result.data) {
        return ctx.render({
          room: null,
          playerId,
          error: result.error || "Room not found",
        });
      }

      return ctx.render({
        room: result.data,
        playerId,
      });
    } catch (error) {
      console.error("Error loading game room:", error);
      return ctx.render({
        room: null,
        playerId: null,
        error: "Failed to load game room",
      });
    }
  },
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
    <div class="min-h-screen-safe bg-gradient-to-br from-purple-50 to-pink-100 safe-area-inset">
      <div class="container mx-auto px-2 sm:px-4 py-2 sm:py-4 lg:py-8 max-w-7xl">
        {/* Header - Mobile responsive */}
        <div class="bg-white rounded-lg shadow-md p-3 sm:p-4 lg:p-6 mb-3 sm:mb-4 lg:mb-6">
          <div class="flex flex-col xs:flex-row xs:justify-between xs:items-center gap-2 xs:gap-3">
            <div class="min-w-0 flex-1">
              <h1 class="text-lg xs:text-xl sm:text-2xl font-bold text-gray-800 truncate">
                {room.room.name}
              </h1>
              <p class="text-xs sm:text-sm lg:text-base text-gray-600">
                <span class="inline xs:hidden">
                  Host: {(room.host?.name || "Unknown").slice(0, 10)}
                  {(room.host?.name || "").length > 10 ? "..." : ""}
                </span>
                <span class="hidden xs:inline">Host: {room.host?.name || "Unknown"}</span>
                <span class="mx-1">•</span>
                <span>{room.playerCount}/{room.room.maxPlayers} players</span>
              </p>
            </div>
            <LeaveRoomButton
              roomId={room.room.id}
              playerId={playerId || ""}
              className="flex-shrink-0 px-3 py-2 sm:px-4 sm:py-2 bg-gray-100 text-gray-700 rounded-lg hover:bg-gray-200 active:bg-gray-300 transition-colors text-sm sm:text-base text-center touch-manipulation no-tap-highlight"
            >
              <span class="xs:hidden">← Lobby</span>
              <span class="hidden xs:inline">← Back to Lobby</span>
            </LeaveRoomButton>
          </div>
        </div>

        {/* Game area - Enhanced mobile layout */}
        <div class="flex flex-col lg:grid lg:grid-cols-4 gap-3 sm:gap-4 lg:gap-6">
          {/* Drawing board area */}
          <div class="order-2 lg:order-1 lg:col-span-3">
            <div class="bg-white rounded-lg shadow-md p-2 sm:p-3 lg:p-6">
              <DrawingBoard
                roomId={room.room.id}
                playerId={playerId || ""}
                gameState={gameState}
                width={800}
                height={500}
                className="w-full"
                responsive
              />
            </div>
          </div>

          {/* Sidebar - Mobile optimized */}
          <div class="order-1 lg:order-2 space-y-3 sm:space-y-4 lg:space-y-6">
            {/* Mobile: Stacked layout, tablet: side-by-side, desktop: stacked */}
            <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-1 gap-3 sm:gap-4 lg:gap-6">
              {/* Chat area - Mobile optimized height */}
              <div class="bg-white rounded-lg shadow-md p-2 sm:p-3 lg:p-6">
                <div class="h-48 xs:h-56 sm:h-64 lg:h-80">
                  <ChatRoom
                    roomId={room.room.id}
                    playerId={playerId || ""}
                    playerName={room.players.find((p: any) => p.id === playerId)?.name || "Unknown"}
                    currentWord={gameState.currentWord}
                    isCurrentDrawer={gameState.currentDrawer === playerId}
                  />
                </div>
              </div>

              {/* Scoreboard - Mobile optimized */}
              <div class="bg-white rounded-lg shadow-md p-2 sm:p-3 lg:p-6">
                <div class="h-48 xs:h-56 sm:h-64 lg:h-auto">
                  <Scoreboard
                    roomId={room.room.id}
                    playerId={playerId || ""}
                    gameState={gameState}
                    className="h-full"
                  />
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
