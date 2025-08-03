import { useEffect, useState } from "preact/hooks";
import type { RoomSummary } from "../lib/room-manager.ts";
import type { GameState } from "../types/game.ts";
import LeaveRoomButton from "./LeaveRoomButton.tsx";

interface RoomHeaderProps {
  room: RoomSummary;
  playerId: string;
  gameState: GameState;
}

export default function RoomHeader({ room: initialRoom, playerId, gameState: initialGameState }: RoomHeaderProps) {
  const [room, setRoom] = useState(initialRoom);
  const [gameState, setGameState] = useState(initialGameState);

  // Always sync with the latest game state from parent
  useEffect(() => {
    setGameState(initialGameState);
  }, [initialGameState]);

  // Debug logging for host changes
  useEffect(() => {
    console.log(`RoomHeader: Current host is ${room.host?.name} (${room.host?.id}), current player is ${playerId}, isHost: ${playerId === room.host?.id}`);
  }, [room.host, playerId]);

  // Sync with game state changes (in case parent component updates)
  useEffect(() => {
    const hostPlayer = gameState.players?.find(p => p.isHost);
    if (hostPlayer && hostPlayer.id !== room.host?.id) {
      console.log(`RoomHeader: Syncing host from game state - ${hostPlayer.name} (${hostPlayer.id})`);
      setRoom(prevRoom => ({
        ...prevRoom,
        host: {
          id: hostPlayer.id,
          name: hostPlayer.name,
          isHost: true,
          isConnected: hostPlayer.isConnected || true,
          lastActivity: hostPlayer.lastActivity || Date.now()
        },
        playerCount: gameState.players?.length || prevRoom.playerCount
      }));
    }
  }, [gameState.players, room.host?.id]);

  useEffect(() => {
    // Set up WebSocket connection for real-time updates
    let ws: WebSocket | null = null;
    let reconnectTimeout: number | null = null;

    const connectWebSocket = () => {
      try {
        const protocol = globalThis.location.protocol === "https:" ? "wss:" : "ws:";
        const wsUrl = `${protocol}//${globalThis.location.host}/api/websocket`;

        ws = new WebSocket(wsUrl);

        ws.onopen = () => {
          console.log("RoomHeader WebSocket connected");

          // Join the room to receive updates
          ws?.send(JSON.stringify({
            type: "join-room",
            roomId: room.room.id,
            playerId: playerId,
            data: {
              playerName: room.players.find(p => p.id === playerId)?.name || "Unknown"
            }
          }));
        };

        ws.onmessage = (event) => {
          try {
            const message = JSON.parse(event.data);

            if (message.type === "game-state" && message.data) {
              // Update game state when received
              console.log("RoomHeader: Received game-state update", message.data);
              setGameState(message.data);
              
              // Also update room host information from game state
              const hostPlayer = message.data.players?.find(p => p.isHost);
              if (hostPlayer) {
                setRoom(prevRoom => ({
                  ...prevRoom,
                  host: {
                    id: hostPlayer.id,
                    name: hostPlayer.name,
                    isHost: true,
                    isConnected: hostPlayer.isConnected,
                    lastActivity: hostPlayer.lastActivity
                  },
                  playerCount: message.data.players?.length || prevRoom.playerCount
                }));
              }
            } else if (message.type === "room-update" && message.data) {
              const updateData = message.data;

              // Handle host migration
              if (updateData.type === "host-changed") {
                console.log("RoomHeader: Host changed detected", updateData);
                
                // Update the room state with new host information
                setRoom(prevRoom => ({
                  ...prevRoom,
                  host: {
                    id: updateData.newHostId,
                    name: updateData.newHostName,
                    isHost: true,
                    isConnected: true,
                    lastActivity: Date.now()
                  }
                }));

                // Update game state to reflect host change
                setGameState(prevState => ({
                  ...prevState,
                  players: prevState.players.map(player => ({
                    ...player,
                    isHost: player.id === updateData.newHostId
                  }))
                }));
              } else if (updateData.type === "player-left" && updateData.wasHost && updateData.hostMigration) {
                console.log("RoomHeader: Host migration detected", updateData);
                
                // Update the room state with new host information
                setRoom(prevRoom => ({
                  ...prevRoom,
                  host: {
                    id: updateData.hostMigration.newHostId,
                    name: updateData.hostMigration.newHostName,
                    isHost: true,
                    isConnected: true,
                    lastActivity: Date.now()
                  },
                  playerCount: prevRoom.playerCount - 1
                }));

                // Update game state to reflect host change and player removal
                setGameState(prevState => ({
                  ...prevState,
                  players: prevState.players
                    .filter(player => player.id !== updateData.playerId)
                    .map(player => ({
                      ...player,
                      isHost: player.id === updateData.hostMigration.newHostId
                    }))
                }));
                
                console.log(`RoomHeader: Host migrated from ${updateData.playerName} to ${updateData.hostMigration.newHostName}`);
              } else if (updateData.type === "player-left") {
                // Handle regular player leaving (not host)
                setRoom(prevRoom => ({
                  ...prevRoom,
                  playerCount: prevRoom.playerCount - 1
                }));

                setGameState(prevState => ({
                  ...prevState,
                  players: prevState.players.filter(player => player.id !== updateData.playerId)
                }));
              } else if (updateData.type === "player-joined") {
                // Handle new player joining
                setRoom(prevRoom => ({
                  ...prevRoom,
                  playerCount: prevRoom.playerCount + 1
                }));
              }

              // If the update contains a complete game state, use it
              if (updateData.gameState) {
                setGameState(updateData.gameState);
                
                // Also update room host information from the complete game state
                const hostPlayer = updateData.gameState.players?.find(p => p.isHost);
                if (hostPlayer) {
                  setRoom(prevRoom => ({
                    ...prevRoom,
                    host: {
                      id: hostPlayer.id,
                      name: hostPlayer.name,
                      isHost: true,
                      isConnected: hostPlayer.isConnected,
                      lastActivity: hostPlayer.lastActivity
                    },
                    playerCount: updateData.gameState.players?.length || prevRoom.playerCount
                  }));
                }
              }
            }
          } catch (error) {
            console.error("RoomHeader: Error parsing WebSocket message:", error);
          }
        };

        ws.onclose = () => {
          console.log("RoomHeader WebSocket disconnected");
          // Attempt to reconnect after 3 seconds
          reconnectTimeout = setTimeout(connectWebSocket, 3000);
        };

        ws.onerror = (error) => {
          console.error("RoomHeader WebSocket error:", error);
        };
      } catch (error) {
        console.error("RoomHeader: Failed to connect WebSocket:", error);
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
  }, [room.room.id, playerId]);

  return (
    <div class="bg-white rounded-lg shadow-md p-3 sm:p-4 lg:p-6 mb-3 sm:mb-4 lg:mb-6">
      <div class="flex flex-col xs:flex-row xs:justify-between xs:items-center gap-2 xs:gap-3">
        <div class="min-w-0 flex-1">
          <h1 class="text-lg xs:text-xl sm:text-2xl font-bold text-gray-800 truncate">
            {room.room.name}
          </h1>
          <p class="text-xs sm:text-sm lg:text-base text-gray-600">
            <span class="inline xs:hidden">
              Host: <span>{(room.host?.name || "Unknown").slice(0, 10)}</span>
              {(room.host?.name || "").length > 10 && <span>...</span>}
            </span>
            <span class="hidden xs:inline">Host: <span>{room.host?.name || "Unknown"}</span></span>
            {playerId === room.host?.id && (
              <span class="ml-1 text-yellow-600 font-semibold">(You)</span>
            )}
            <span class="mx-1">•</span>
            <span>
              {gameState.players.length}/{room.room.maxPlayers} players
            </span>
          </p>
        </div>
        <div class="flex gap-2">
          <LeaveRoomButton
            roomId={room.room.id}
            playerId={playerId}
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
  );
}