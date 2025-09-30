import { Handlers, PageProps } from "$fresh/server.ts";
import { signal } from "@preact/signals";
import { RoomManager } from "../../lib/core/room-manager.ts";
import { GameRegistry } from "../../lib/core/game-registry.ts";
import type { RoomSummary } from "../../lib/core/room-manager.ts";
import type { BaseGameState } from "../../types/core/game.ts";
import "../../lib/games/index.ts"; // Ensure games are registered
import ChatRoom from "../../islands/core/ChatRoom.tsx";
import DrawingBoard from "../../islands/games/drawing/DrawingBoard.tsx";
import PokerRoom from "../../islands/PokerRoom.tsx";
import Scoreboard from "../../islands/Scoreboard.tsx";
import _LeaveRoomButton from "../../islands/LeaveRoomButton.tsx";
import RoomHeader from "../../islands/RoomHeader.tsx";
import GameSettingsWrapper from "../../islands/GameSettingsWrapper.tsx";
import type { DrawingCommand } from "../../types/games/drawing.ts";
import type { PokerAction } from "../../types/games/poker.ts";

interface GameRoomData {
  room: RoomSummary | null;
  playerId: string | null;
  error?: string;
}

// Helper function to create initial game state from room data
function createInitialGameState(room: RoomSummary, playerId?: string | null): BaseGameState {
  const gameRegistry = GameRegistry.getInstance();
  const gameType = room.room.gameType || "drawing";

  // Get default settings for the game type
  const defaultSettings = gameRegistry.getDefaultSettings(gameType) || {
    maxRounds: 5,
    roundTimeSeconds: 75,
  };

  // Ensure the current player is included in the players list
  const players = room.players.map((player: any) => ({
    id: player.id,
    name: player.name,
    isHost: player.isHost,
    isConnected: true, // Assume all players in room are connected
    lastActivity: Date.now(),
  }));

  // If playerId is provided but not found in players, this might be a timing issue
  // Log a warning but don't add a placeholder player
  if (playerId && !players.find((p) => p.id === playerId)) {
    console.warn(
      `Player ${playerId} not found in room players list. This might be a timing issue.`,
    );
  }

  // Initialize game-specific data so clients can render correctly before real-time updates
  const gameData: any = {};
  if (gameType === "drawing") {
    // Make the current player the initial drawer and start with empty drawing history
    gameData.currentDrawer = playerId || null;
    gameData.drawingData = [] as any[];
  } else if (gameType === "poker") {
    // Initialize poker-specific data
    gameData.deck = [];
    gameData.communityCards = [];
    gameData.pot = 0;
    gameData.currentBet = 0;
    gameData.bettingRound = "Pre-flop";
    gameData.currentPlayerIndex = 0;
    gameData.smallBlindIndex = 0;
    gameData.bigBlindIndex = 1;
    // Initialize poker players with empty hands and chips
    gameData.players = players.map((player: any, index: number) => ({
      ...player,
      hand: [],
      chips: (defaultSettings as any).buyIn || 1000,
      currentBet: 0,
      hasActed: false,
      isAllIn: false,
      isFolded: false,
      position: index,
    }));
  }

  const baseState = {
    roomId: room.room.id,
    gameType,
    roundNumber: 0, // Game hasn't started
    timeRemaining: defaultSettings.roundTimeSeconds * 1000, // Convert to milliseconds
    phase: "waiting", // Waiting for game to start
    players: gameType === "poker" ? gameData.players : players,
    scores: players.reduce((acc: any, player: any) => {
      acc[player.id] = 0; // Initialize all scores to 0
      return acc;
    }, {} as Record<string, number>),
    gameData,
    chatMessages: [], // No chat messages initially
    settings: defaultSettings,
  };

  // For poker, add poker-specific fields to the top level
  if (gameType === "poker") {
    return {
      ...baseState,
      deck: gameData.deck,
      communityCards: gameData.communityCards,
      pot: gameData.pot,
      currentBet: gameData.currentBet,
      bettingRound: gameData.bettingRound,
      currentPlayerIndex: gameData.currentPlayerIndex,
      smallBlindIndex: gameData.smallBlindIndex,
      bigBlindIndex: gameData.bigBlindIndex,
    };
  }

  return baseState;
}

// Create a stable poker action handler function
function createPokerActionHandler(roomId: string, playerId: string) {
  return (action: PokerAction, amount?: number) => {
    const sendPokerAction = () => {
      try {
        const ws = (globalThis as any).__gameWebSocket as WebSocket | undefined;
        console.log("Attempting to send poker action", {
          action,
          amount,
          wsExists: !!ws,
          readyState: ws?.readyState,
        });
        if (ws && ws.readyState === WebSocket.OPEN) {
          const message = {
            type: "poker-action",
            roomId: roomId,
            playerId: playerId,
            data: { action, amount },
          };
          console.log("Sending poker action message:", message);
          ws.send(JSON.stringify(message));
          return true;
        } else {
          console.warn("WS not ready for poker action", { action, readyState: ws?.readyState });
          return false;
        }
      } catch (err) {
        console.error("Failed to send poker action", err);
        return false;
      }
    };

    // Attempt to send immediately, with retry if needed
    if (!sendPokerAction()) {
      setTimeout(() => {
        sendPokerAction();
      }, 100);
    }
  };
}

// Create a stable drawing command handler function
function createDrawingCommandHandler(roomId: string, playerId: string) {
  return (command: DrawingCommand) => {
    // Forward drawing command via WebSocket with retry logic
    const sendDrawCommand = () => {
      try {
        const ws = (globalThis as any).__gameWebSocket as WebSocket | undefined;
        console.log("Attempting to send draw command", {
          command,
          wsExists: !!ws,
          readyState: ws?.readyState,
        });
        if (ws && ws.readyState === WebSocket.OPEN) {
          // Server expects message.data to be a single DrawingCommand
          const message = {
            type: "draw",
            roomId: roomId,
            playerId: playerId,
            data: command,
          };
          console.log("Sending draw message:", message);
          ws.send(JSON.stringify(message));
          return true;
        } else {
          console.warn("WS not ready for draw command", { command, readyState: ws?.readyState });
          return false;
        }
      } catch (err) {
        console.error("Failed to send draw command", err);
        return false;
      }
    };

    // Try to send immediately
    if (!sendDrawCommand()) {
      // If WebSocket not ready, wait a bit and retry
      console.log("WebSocket not ready, retrying in 100ms...");
      setTimeout(() => {
        if (!sendDrawCommand()) {
          console.log("WebSocket still not ready, retrying in 500ms...");
          setTimeout(() => {
            if (!sendDrawCommand()) {
              console.error("Failed to send draw command after retries");
            }
          }, 500);
        }
      }, 100);
    }
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

      const _isDevelopment = Deno.env.get("DENO_ENV") === "development";

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
      <div class="min-h-screen bg-gradient-to-br from-purple-600 to-blue-600 flex items-center justify-center p-4">
        <div class="max-w-md mx-auto text-center">
          <div class="bg-white/10 backdrop-blur-sm rounded-lg shadow-lg border border-white/20 p-8">
            <div class="text-white text-6xl mb-4">⚠️</div>
            <h1 class="text-2xl font-bold text-white mb-2">Room Not Found</h1>
            <p class="text-white/80 mb-6">
              {data.error || "The room you're looking for doesn't exist or is no longer available."}
            </p>
            <a
              href="/"
              class="inline-block px-6 py-3 bg-white text-purple-600 rounded-lg hover:bg-white/90 transition-colors font-semibold"
            >
              Back to Lobby
            </a>
          </div>
        </div>
      </div>
    );
  }

  const { room, playerId } = data;
  const gameState = createInitialGameState(room, playerId);
  const currentPlayerName = room.players.find((p) => p.id === playerId)?.name || "Unknown";

  // Game settings modal state using signals
  const showSettingsModal = signal(false);

  // Create a stable drawing command handler
  const drawingCommandHandler = createDrawingCommandHandler(room.room.id, playerId || "");

  // Create a stable poker action handler
  const pokerActionHandler = createPokerActionHandler(room.room.id, playerId || "");

  // Use dedicated layout based on game type
  if (room.room.gameType === "poker") {
    return (
      <PokerRoom
        room={room}
        playerId={playerId || ""}
        gameState={gameState}
        pokerActionHandler={pokerActionHandler}
      />
    );
  }

  // Default drawing game layout
  return (
    <div class="min-h-screen bg-gradient-to-br from-purple-600 to-blue-600 safe-area-inset">
      {/* Main Content */}
      <div class="pt-4 px-4 max-w-7xl mx-auto h-[calc(100vh-5rem)] flex flex-col overflow-hidden">
        {/* Player ID missing warning */}
        {(!playerId || playerId.trim() === "") && (
          <div class="bg-yellow-400/20 border border-yellow-400/30 rounded-lg p-3 mb-4 backdrop-blur-sm">
            <div class="flex items-center text-yellow-100">
              <span class="text-yellow-200 mr-2">⚠️</span>
              <div class="text-sm">
                <strong>Limited functionality:</strong>{" "}
                Some features may not work properly because player information is missing.
                <a href="/" class="ml-2 underline hover:no-underline">Return to lobby</a>{" "}
                to rejoin properly.
              </div>
            </div>
          </div>
        )}

        {/* Header - Mobile responsive with real-time updates */}
        <RoomHeader
          room={room}
          playerId={playerId || ""}
          gameState={gameState}
        />

        {/* Game area - Drawing board and scoreboard on same row, chat below */}
        <div class="flex flex-col gap-4 lg:gap-6 flex-1 min-h-0 overflow-hidden">
          {/* Top row - Drawing board and scoreboard side by side */}
          <div class="flex-1 flex flex-col lg:flex-row gap-4 lg:gap-6 min-h-0">
            {/* Drawing board area - takes 70% on desktop */}
            <div class="flex-1 lg:flex-[0_0_70%] bg-white/10 backdrop-blur-sm border border-white/20 rounded-lg shadow-lg p-4 lg:p-6 min-h-0">
              <div class="game-area overflow-hidden">
                {room.room.gameType === "drawing"
                  ? (
                    <DrawingBoard
                      width={960}
                      height={600}
                      onDrawCommand={drawingCommandHandler}
                      drawingData={(gameState as any)?.gameData?.drawingData || []}
                      isDrawer={(gameState as any)?.gameData?.currentDrawer === (playerId || "")}
                      playerId={playerId || ""}
                      roomId={room.room.id}
                    />
                  )
                  : (
                    <div class="flex items-center justify-center h-64 text-white/70">
                      Game type "{room.room.gameType}" not yet implemented
                    </div>
                  )}
              </div>
            </div>

            {/* Scoreboard - takes 30% on desktop, full width on mobile */}
            <div class="lg:w-[30%] bg-white/10 backdrop-blur-sm border border-white/20 rounded-lg shadow-lg p-4 lg:p-6 flex flex-col min-h-0">
              <div class="flex-1 min-h-0">
                <Scoreboard
                  roomId={room.room.id}
                  playerId={playerId || ""}
                  gameState={gameState}
                  onShowSettingsModal={() => {
                    console.log("Setting showSettingsModal to true");
                    console.log("Before setting:", showSettingsModal.value);
                    showSettingsModal.value = true;
                    console.log("After setting:", showSettingsModal.value);
                  }}
                  className="h-full"
                />
              </div>
            </div>
          </div>

          {/* Bottom row - Chat stretches full width and remaining height */}
          <div class="bg-white/10 backdrop-blur-sm border border-white/20 rounded-lg shadow-lg overflow-hidden flex flex-col flex-1 min-h-0">
            <ChatRoom
              roomId={room.room.id}
              messages={gameState.chatMessages}
              currentPlayerId={playerId || ""}
              currentPlayerName={currentPlayerName}
              disabled={false}
              placeholder="Type a message..."
            />
          </div>
        </div>
      </div>

      {/* Game Settings Modal - centered on screen */}
      <GameSettingsWrapper
        roomId={room.room.id}
        playerId={playerId || ""}
        isOpen={showSettingsModal.value}
        onModalClose={() => showSettingsModal.value = false}
        currentSettings={gameState.settings}
      />
    </div>
  );
}
