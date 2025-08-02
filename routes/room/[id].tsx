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

      // Log warning if playerId is missing (helps with debugging)
      if (!playerId) {
        console.warn(`Room ${roomId} accessed without playerId parameter`);
      }

      // In development, we don't have Cloudflare env, so we skip the DB check
      const env = (ctx.state as any).env as Env;
      const isDevelopment = Deno.env.get("DENO_ENV") !== "production";

      if (!isDevelopment && !env?.DB) {
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
    <div class="h-screen bg-gradient-to-br from-purple-50 to-pink-100 safe-area-inset flex flex-col">
      <div class="container mx-auto px-2 sm:px-4 py-2 sm:py-4 lg:py-8 max-w-7xl flex flex-col flex-1 min-h-0 h-full">
        {/* Player ID missing warning */}
        {(!playerId || playerId.trim() === "") && (
          <div class="bg-yellow-50 border border-yellow-200 rounded-lg p-3 mb-3 sm:mb-4">
            <div class="flex items-center text-yellow-800">
              <span class="text-yellow-600 mr-2">⚠️</span>
              <div class="text-sm">
                <strong>Limited functionality:</strong>{" "}
                Some features may not work properly because player information is missing.
                <a href="/" class="ml-2 underline hover:no-underline">Return to lobby</a>{" "}
                to rejoin properly.
              </div>
            </div>
          </div>
        )}

        {/* Header - Mobile responsive */}
        <div class="bg-white rounded-lg shadow-md p-3 sm:p-4 lg:p-6 mb-3 sm:mb-4 lg:mb-6">
          <div class="flex flex-col xs:flex-row xs:justify-between xs:items-center gap-2 xs:gap-3">
            <div class="min-w-0 flex-1">
              <h1 class="text-lg xs:text-xl sm:text-2xl font-bold text-gray-800 truncate">
                {room.room.name}
              </h1>
              <p class="text-xs sm:text-sm lg:text-base text-gray-600">
                <span class="inline xs:hidden">
                  Host: <span id="host-name-short">{(room.host?.name || "Unknown").slice(0, 10)}</span>
                  <span id="host-name-ellipsis" style="display: none;">...</span>
                </span>
                <span class="hidden xs:inline">Host: <span id="host-name-full">{room.host?.name || "Unknown"}</span></span>
                <span class="mx-1">•</span>
                <span id="player-count-display">
                  {room.playerCount}/{room.room.maxPlayers} players
                </span>
              </p>
            </div>
            <div class="flex gap-2">
              <LeaveRoomButton
                roomId={room.room.id}
                playerId={playerId || ""}
                className="flex-shrink-0 px-3 py-2 sm:px-4 sm:py-2 bg-gray-100 text-gray-700 rounded-lg hover:bg-gray-200 active:bg-gray-300 transition-colors text-sm sm:text-base text-center touch-manipulation no-tap-highlight"
              />
              {/* Fallback link in case the interactive button fails */}
              {(!playerId || playerId.trim() === "") && (
                <a
                  href="/"
                  class="flex-shrink-0 px-3 py-2 sm:px-4 sm:py-2 bg-blue-100 text-blue-700 rounded-lg hover:bg-blue-200 active:bg-blue-300 transition-colors text-sm sm:text-base text-center touch-manipulation no-tap-highlight"
                  title="Simple link back to lobby"
                >
                  <span class="xs:hidden">← Home</span>
                  <span class="hidden xs:inline">← Back to Lobby</span>
                </a>
              )}
            </div>
          </div>
        </div>

        {/* Game area - Drawing board and scoreboard on same row, chat below */}
        <div class="game-container flex flex-col gap-3 sm:gap-4 lg:gap-6 flex-1 min-h-0">
          {/* Top row - Drawing board and scoreboard side by side */}
          <div class="flex flex-col lg:flex-row gap-3 sm:gap-4 lg:gap-6 h-auto">
            {/* Drawing board area - takes 70% on desktop */}
            <div class="lg:w-[70%] bg-white rounded-lg shadow-md p-2 sm:p-3 lg:p-6">
              <div class="drawing-area overflow-hidden">
                <DrawingBoard
                  roomId={room.room.id}
                  playerId={playerId || ""}
                  gameState={gameState}
                  width={960}
                  height={600}
                  className="w-full drawing-area"
                  responsive
                />
              </div>
            </div>

            {/* Scoreboard - takes 30% on desktop, full width on mobile */}
            <div class="lg:w-[30%] bg-white rounded-lg shadow-md p-2 sm:p-3 lg:p-6 flex flex-col">
              <div class="flex-1 min-h-0">
                <Scoreboard
                  roomId={room.room.id}
                  playerId={playerId || ""}
                  gameState={gameState}
                  className="h-full"
                />
              </div>
            </div>
          </div>

          {/* Bottom row - Chat stretches full width and remaining height */}
          <div class="bg-white rounded-lg shadow-md p-2 sm:p-3 lg:p-6 flex-1 min-h-0 flex flex-col">
            <div class="flex-1 min-h-0">
              <ChatRoom
                roomId={room.room.id}
                playerId={playerId || ""}
                playerName={room.players.find((p: any) => p.id === playerId)?.name || "Unknown"}
                currentWord={gameState.currentWord}
                isCurrentDrawer={gameState.currentDrawer === playerId}
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
