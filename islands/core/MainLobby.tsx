// Main lobby component for showing available rooms
import { useEffect, useState } from "preact/hooks";
import { signal } from "@preact/signals";
import type { RoomSummary } from "../../lib/core/room-manager.ts";
import CreateRoomModal from "../CreateRoomModal.tsx";
import JoinRoomModal from "../JoinRoomModal.tsx";

interface MainLobbyProps {
  initialRooms: RoomSummary[];
  error?: string;
  isDev: boolean;
  user?: {
    id: string;
    email: string;
    username: string;
    name: string | null;
  };
}

// Global signals for modals
const showCreateModal = signal(false);
const showJoinModal = signal(false);
const selectedRoom = signal<RoomSummary | null>(null);

export default function MainLobby({ initialRooms, error, isDev, user }: MainLobbyProps) {
  const [rooms, setRooms] = useState<RoomSummary[]>(initialRooms);
  const [loading, setLoading] = useState(false);
  const [wsConnected, setWsConnected] = useState(false);
  const [cleanupLoading, setCleanupLoading] = useState(false);

  // WebSocket connection for real-time updates
  useEffect(() => {
    // Enable WebSocket in development environment
    console.log("Enabling WebSocket connection for lobby");

    let ws: WebSocket | null = null;
    let reconnectTimeout: number | null = null;

    const connectWebSocket = () => {
      try {
        const protocol = globalThis.location.protocol === "https:" ? "wss:" : "ws:";
        const wsUrl = `${protocol}//${globalThis.location.host}/api/websocket`;

        ws = new WebSocket(wsUrl);

        ws.onopen = () => {
          console.log("Lobby WebSocket connected");
          setWsConnected(true);

          // Subscribe to lobby updates
          ws?.send(JSON.stringify({
            type: "subscribe-lobby",
            data: {},
          }));
        };

        ws.onmessage = (event) => {
          try {
            const message = JSON.parse(event.data);

            if (message.type === "lobby-update") {
              // Refresh room list when lobby updates
              refreshRooms();
            }
          } catch (error) {
            console.error("Error parsing WebSocket message:", error);
          }
        };

        ws.onclose = () => {
          console.log("Lobby WebSocket disconnected");
          setWsConnected(false);

          // Attempt to reconnect after 3 seconds
          reconnectTimeout = setTimeout(connectWebSocket, 3000);
        };

        ws.onerror = (error) => {
          console.error("WebSocket error:", error);
          setWsConnected(false);
        };
      } catch (error) {
        console.error("Failed to connect WebSocket:", error);
        setWsConnected(false);

        // Retry connection after 5 seconds
        reconnectTimeout = setTimeout(connectWebSocket, 5000);
      }
    };

    connectWebSocket();

    return () => {
      if (reconnectTimeout) {
        clearTimeout(reconnectTimeout);
      }
      if (ws) {
        ws.close();
      }
    };
  }, []);

  const refreshRooms = async () => {
    try {
      setLoading(true);
      const response = await fetch("/api/rooms");

      if (response.ok) {
        const data = await response.json();
        setRooms(data.rooms || []);
      } else {
        console.error("Failed to refresh rooms");
      }
    } catch (error) {
      console.error("Error refreshing rooms:", error);
    } finally {
      setLoading(false);
    }
  };

  const handleCreateRoom = () => {
    showCreateModal.value = true;
  };

  const handleJoinRoom = (room: RoomSummary) => {
    selectedRoom.value = room;
    showJoinModal.value = true;
  };

  const handleRoomCreated = () => {
    refreshRooms();
    showCreateModal.value = false;
  };

  const handleRoomJoined = (roomId: string, playerId: string) => {
    // Navigate to game room
    globalThis.location.href = `/room/${roomId}?playerId=${playerId}`;
  };

  const handleCleanupDanglingRooms = async () => {
    try {
      setCleanupLoading(true);
      const response = await fetch("/api/admin/cleanup-rooms", { method: "POST" });
      if (response.ok) {
        const data = await response.json();
        console.log(`Cleaned dangling rooms: ${data.cleanedCount}`);
        await refreshRooms();
      } else {
        console.error("Failed to clean dangling rooms");
      }
    } catch (err) {
      console.error("Cleanup error:", err);
    } finally {
      setCleanupLoading(false);
    }
  };

  if (error) {
    return (
      <div class="max-w-4xl mx-auto">
        <div class="bg-red-100/90 border border-red-300 rounded-lg p-6 text-center backdrop-blur-sm">
          <div class="text-red-700 text-lg font-semibold mb-2">
            ⚠️ Error Loading Lobby
          </div>
          <p class="text-red-800">{error}</p>
          <button
            type="button"
            onClick={refreshRooms}
            class="mt-4 px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors"
          >
            Try Again
          </button>
        </div>
      </div>
    );
  }

  return (
    <div class="min-h-screen bg-gradient-to-br from-purple-600 to-blue-600 p-4">
      {/* Fixed Top Navigation Bar */}
      <div class="fixed top-0 left-0 right-0 z-50 bg-white/5 backdrop-blur-md border-b border-white/10">
        <div class="max-w-6xl mx-auto px-4 py-3">
          <div class="flex justify-between items-center">
            {/* Left side - App Title and Connection Status */}
            <div class="flex items-center space-x-3">
              <h1 class="text-lg sm:text-xl font-bold text-white">
                🎨 Drawing Game
              </h1>
              <div class="flex items-center space-x-1">
                <div class={`w-2 h-2 rounded-full ${wsConnected ? "bg-green-400" : "bg-yellow-400"}`}>
                </div>
                <span class="text-xs text-white/60">
                  {wsConnected ? "Connected" : (isDev ? "Dev" : "")}
                </span>
              </div>
            </div>

            {/* Right side - User Auth */}
            <div class="flex items-center gap-2">
              {user
                ? (
                  <div class="flex items-center gap-2 bg-white/15 backdrop-blur-sm rounded-full px-4 py-2 border border-white/20 shadow-lg">
                    <span class="text-white font-semibold text-sm">
                      👤 {user.name || user.username}
                    </span>
                    <a
                      href="/api/auth/logout"
                      onClick={async (e) => {
                        e.preventDefault();
                        await fetch("/api/auth/logout", { method: "POST" });
                        globalThis.location.reload();
                      }}
                      class="text-xs text-white/80 hover:text-white transition-colors"
                    >
                      Logout
                    </a>
                  </div>
                )
                : (
                  <a
                    href="/login"
                    class="px-4 py-2 bg-white/90 text-purple-600 rounded-lg hover:bg-white transition-colors font-medium text-sm shadow-lg backdrop-blur-sm"
                  >
                    Login
                  </a>
                )}
            </div>
          </div>
        </div>
      </div>

      {/* Main Content with top padding to account for fixed nav */}
      <div class="max-w-6xl mx-auto pt-20 px-4 sm:px-6 lg:px-8">
        {/* App description */}
        <div class="text-center mb-6">
          <p class="text-sm sm:text-base text-white/90 max-w-2xl mx-auto">
            Join a room or create your own to start playing with friends!
          </p>
        </div>

        {/* Header with refresh button and create button - Mobile responsive */}
        <div class="flex flex-col sm:flex-row sm:justify-between sm:items-center gap-4 mb-6">
          <div class="flex items-center justify-between sm:justify-start sm:space-x-4">
            <button
              type="button"
              onClick={refreshRooms}
              disabled={loading}
              class="flex items-center space-x-2 px-3 py-2 sm:px-4 sm:py-2 bg-white/20 text-white rounded-lg hover:bg-white/30 transition-colors disabled:opacity-50 touch-manipulation no-tap-highlight"
            >
              <span class={`text-sm ${loading ? "animate-spin" : ""}`}>
                {loading ? "⟳" : "↻"}
              </span>
              <span class="hidden xs:inline">Refresh</span>
            </button>
          </div>

          <div class="flex flex-col sm:flex-row gap-2 sm:gap-3 w-full sm:w-auto items-center">
            {isDev && (
              <button
                type="button"
                onClick={handleCleanupDanglingRooms}
                disabled={cleanupLoading}
                class="w-full sm:w-auto px-4 py-3 bg-amber-600 text-white rounded-lg hover:bg-amber-700 active:bg-amber-800 transition-colors font-semibold touch-manipulation no-tap-highlight text-touch disabled:opacity-60"
                title="Delete rooms with no players"
              >
                {cleanupLoading ? "Cleaning…" : "Clear Dangling Rooms"}
              </button>
            )}
            <button
              type="button"
              onClick={handleCreateRoom}
              class="w-full sm:w-auto px-6 py-3 bg-white text-purple-600 rounded-lg hover:bg-white/90 active:bg-white/80 transition-colors font-semibold touch-manipulation no-tap-highlight text-touch"
            >
              <span class="sm:hidden">+ New Room</span>
              <span class="hidden sm:inline">+ Create Room</span>
            </button>
          </div>
        </div>

        {isDev && <div class="text-yellow-300 font-bold">Dev Mode Enabled</div>}

        {/* Room list - Enhanced mobile grid */}
        <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
          {rooms.length === 0
            ? (
              <div class="col-span-full text-center py-12 px-4">
                <div class="text-white/60 text-4xl sm:text-6xl mb-4">🎨</div>
                <h3 class="text-lg sm:text-xl font-semibold text-white mb-2">
                  No active rooms
                </h3>
                <p class="text-sm sm:text-base text-white/80 mb-6 max-w-md mx-auto">
                  Be the first to create a room and start playing!
                </p>
                <button
                  type="button"
                  onClick={handleCreateRoom}
                  class="px-6 py-3 bg-white text-purple-600 rounded-lg hover:bg-white/90 active:bg-white/80 transition-colors touch-manipulation no-tap-highlight"
                >
                  Create First Room
                </button>
              </div>
            )
            : (
              rooms.map((roomSummary) => (
                <RoomCard
                  key={roomSummary.room.id}
                  roomSummary={roomSummary}
                  onJoin={() => handleJoinRoom(roomSummary)}
                />
              ))
            )}
        </div>

        {/* Modals */}
        <CreateRoomModal
          show={showCreateModal.value}
          onClose={() => showCreateModal.value = false}
          onSuccess={handleRoomCreated}
        />

        <JoinRoomModal
          show={showJoinModal.value}
          room={selectedRoom.value}
          onClose={() => {
            showJoinModal.value = false;
            selectedRoom.value = null;
          }}
          onSuccess={handleRoomJoined}
        />
      </div>
    </div>
  );
}

interface RoomCardProps {
  roomSummary: RoomSummary;
  onJoin: () => void;
}

function RoomCard({ roomSummary, onJoin }: RoomCardProps) {
  const { room, playerCount, canJoin, host } = roomSummary;

  return (
    <div class="bg-white/10 backdrop-blur-sm rounded-lg shadow-lg border border-white/20 p-4 sm:p-6 hover:bg-white/15 active:bg-white/20 transition-all duration-200 touch-manipulation">
      <div class="flex justify-between items-start mb-3 sm:mb-4">
        <div class="min-w-0 flex-1 mr-3">
          <h3 class="text-base sm:text-lg font-semibold text-white mb-1 truncate">
            {room.name}
          </h3>
          <p class="text-xs sm:text-sm text-white/80 truncate">
            Host: {host?.name || "Unknown"}
          </p>
        </div>
        <div class="text-right flex-shrink-0">
          <div class="text-sm text-white/70 mb-1">
            {playerCount}/{room.maxPlayers}
          </div>
          <div
            class={`text-xs px-2 py-1 rounded-full font-medium ${
              canJoin ? "bg-green-400/20 text-green-200" : "bg-red-400/20 text-red-200"
            }`}
          >
            {canJoin ? "Open" : "Full"}
          </div>
        </div>
      </div>

      <div class="mb-4">
        <div class="text-xs sm:text-sm text-white/70 mb-2">Players:</div>
        <div class="flex flex-wrap gap-1">
          {roomSummary.players.slice(0, 4).map((player: any) => (
            <span
              key={player.id}
              class={`text-xs px-2 py-1 rounded-full truncate max-w-20 sm:max-w-none ${
                player.isHost ? "bg-yellow-400/20 text-yellow-200" : "bg-white/15 text-white/90"
              }`}
              title={`${player.name}${player.isHost ? " (Host)" : ""}`}
            >
              <span class="truncate">
                {player.name.length > 8 ? `${player.name.slice(0, 8)}...` : player.name}
              </span>
              {player.isHost && " 👑"}
            </span>
          ))}
          {roomSummary.players.length > 4 && (
            <span class="text-xs px-2 py-1 rounded-full bg-white/15 text-white/90">
              +{roomSummary.players.length - 4}
            </span>
          )}
        </div>
      </div>

      <button
        type="button"
        onClick={onJoin}
        disabled={!canJoin}
        class={`w-full py-3 px-4 rounded-lg font-medium transition-all duration-200 touch-manipulation no-tap-highlight text-touch ${
          canJoin
            ? "bg-white text-purple-600 hover:bg-white/90 active:bg-white/80 active:scale-95"
            : "bg-white/20 text-white/50 cursor-not-allowed"
        }`}
      >
        {canJoin ? "Join Room" : "Room Full"}
      </button>
    </div>
  );
}
