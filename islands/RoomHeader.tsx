import { useEffect, useState } from "preact/hooks";
import type { RoomSummary } from "../lib/room-manager.ts";
import type { GameSettings, GameState } from "../types/game.ts";
import LeaveRoomButton from "./LeaveRoomButton.tsx";
import GameSettingsModal from "../components/GameSettingsModal.tsx";

interface RoomHeaderProps {
  room: RoomSummary;
  playerId: string;
  gameState: GameState;
}

export default function RoomHeader(
  { room: initialRoom, playerId, gameState: initialGameState }: RoomHeaderProps,
) {
  const [room, setRoom] = useState(initialRoom);
  const [gameState, setGameState] = useState(initialGameState);
  const [showSettingsModal, setShowSettingsModal] = useState(false);

  // Always sync with the latest game state from parent
  useEffect(() => {
    setGameState(initialGameState);
  }, [initialGameState]);

  // Debug logging for host changes
  useEffect(() => {
    console.log(
      `RoomHeader: Current host is ${room.host?.name} (${room.host?.id}), current player is ${playerId}, isHost: ${
        playerId === room.host?.id
      }`,
    );
  }, [room.host, playerId]);

  // Handle settings save
  const handleSettingsSave = (settings: GameSettings) => {
    console.log("RoomHeader: Saving game settings:", settings);

    // Send settings update via WebSocket
    const ws = (globalThis as any).__gameWebSocket as WebSocket;
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({
        type: "update-settings",
        roomId: room.room.id,
        playerId,
        data: settings,
      }));
    }

    // Update local game state
    setGameState((prev) => ({
      ...prev,
      settings,
    }));
  };

  // Sync with game state changes (in case parent component updates)
  useEffect(() => {
    const hostPlayer = gameState.players?.find((p) => p.isHost);
    const currentPlayerCount = gameState.players?.length || 0;

    // Always update player count from game state
    if (currentPlayerCount !== room.playerCount) {
      console.log(
        `RoomHeader: Updating player count from ${room.playerCount} to ${currentPlayerCount}`,
      );
      setRoom((prevRoom) => ({
        ...prevRoom,
        playerCount: currentPlayerCount,
      }));
    }

    // Update host information if changed
    if (hostPlayer && hostPlayer.id !== room.host?.id) {
      console.log(
        `RoomHeader: Syncing host from game state - ${hostPlayer.name} (${hostPlayer.id})`,
      );
      setRoom((prevRoom) => ({
        ...prevRoom,
        host: {
          id: hostPlayer.id,
          name: hostPlayer.name,
          isHost: true,
          isConnected: hostPlayer.isConnected || true,
          lastActivity: hostPlayer.lastActivity || Date.now(),
          joinedAt: new Date().toISOString(),
        },
        playerCount: currentPlayerCount,
      }));
    }
  }, [gameState.players, room.host?.id, room.playerCount]);

  useEffect(() => {
    // Listen for WebSocket updates from the global game WebSocket connection
    // The Scoreboard component manages the main WebSocket connection
    const handleGameStateUpdate = (event: Event) => {
      const customEvent = event as CustomEvent;
      const { gameState: updatedGameState } = customEvent.detail;
      if (updatedGameState && updatedGameState.roomId === room.room.id) {
        console.log("RoomHeader: Received game state update from global WebSocket");
        setGameState(updatedGameState);

        // Update room host information from game state
        const hostPlayer = updatedGameState.players?.find((p: any) => p.isHost);
        if (hostPlayer) {
          setRoom((prevRoom) => ({
            ...prevRoom,
            host: {
              id: hostPlayer.id,
              name: hostPlayer.name,
              isHost: true,
              isConnected: hostPlayer.isConnected,
              lastActivity: hostPlayer.lastActivity,
              joinedAt: new Date().toISOString(),
            },
            playerCount: updatedGameState.players?.length || prevRoom.playerCount,
          }));
        }
      }
    };

    const handleRoomUpdate = (event: Event) => {
      const customEvent = event as CustomEvent;
      const { updateData } = customEvent.detail;
      if (updateData && updateData.roomId === room.room.id) {
        console.log("RoomHeader: Received room update from global WebSocket", updateData);

        // Handle host migration
        if (updateData.type === "host-changed") {
          setRoom((prevRoom) => ({
            ...prevRoom,
            host: {
              id: updateData.newHostId,
              name: updateData.newHostName,
              isHost: true,
              isConnected: true,
              lastActivity: Date.now(),
              joinedAt: new Date().toISOString(),
            },
          }));

          setGameState((prevState) => ({
            ...prevState,
            players: prevState.players.map((player) => ({
              ...player,
              isHost: player.id === updateData.newHostId,
            })),
          }));
        } else if (updateData.type === "player-left") {
          console.log("RoomHeader: Player left, updating player count");

          // Handle host migration if the leaving player was host
          if (updateData.wasHost && updateData.hostMigration) {
            setRoom((prevRoom) => ({
              ...prevRoom,
              host: {
                id: updateData.hostMigration.newHostId,
                name: updateData.hostMigration.newHostName,
                isHost: true,
                isConnected: true,
                lastActivity: Date.now(),
                joinedAt: new Date().toISOString(),
              },
              playerCount: Math.max(0, prevRoom.playerCount - 1),
            }));

            setGameState((prevState) => ({
              ...prevState,
              players: prevState.players
                .filter((player) => player.id !== updateData.playerId)
                .map((player) => ({
                  ...player,
                  isHost: player.id === updateData.hostMigration.newHostId,
                })),
            }));
          } else {
            // Regular player leaving (not host)
            setRoom((prevRoom) => ({
              ...prevRoom,
              playerCount: Math.max(0, prevRoom.playerCount - 1),
            }));

            setGameState((prevState) => ({
              ...prevState,
              players: prevState.players.filter((player) => player.id !== updateData.playerId),
            }));
          }
        } else if (updateData.type === "player-joined") {
          console.log("RoomHeader: Player joined, updating player count");

          setRoom((prevRoom) => ({
            ...prevRoom,
            playerCount: prevRoom.playerCount + 1,
          }));

          // If the update contains complete game state, use it for accurate player list
          if (updateData.gameState && updateData.gameState.players) {
            setGameState(updateData.gameState);

            // Update room info from complete game state
            const hostPlayer = updateData.gameState.players.find((p: any) => p.isHost);
            if (hostPlayer) {
              setRoom((prevRoom) => ({
                ...prevRoom,
                host: {
                  id: hostPlayer.id,
                  name: hostPlayer.name,
                  isHost: true,
                  isConnected: hostPlayer.isConnected,
                  lastActivity: hostPlayer.lastActivity,
                  joinedAt: new Date().toISOString(),
                },
                playerCount: updateData.gameState.players.length,
              }));
            }
          }
        }

        // If the update contains a complete game state, use it for most accurate data
        if (updateData.gameState && updateData.gameState.players) {
          console.log("RoomHeader: Updating from complete game state in room update");
          setGameState(updateData.gameState);

          const hostPlayer = updateData.gameState.players.find((p: any) => p.isHost);
          if (hostPlayer) {
            setRoom((prevRoom) => ({
              ...prevRoom,
              host: {
                id: hostPlayer.id,
                name: hostPlayer.name,
                isHost: true,
                isConnected: hostPlayer.isConnected,
                lastActivity: hostPlayer.lastActivity,
                joinedAt: new Date().toISOString(),
              },
              playerCount: updateData.gameState.players.length,
            }));
          }
        }
      }
    };

    // Listen for header update events from Scoreboard
    const handleHeaderUpdate = (event: Event) => {
      const customEvent = event as CustomEvent;
      const { playerCount, hostName } = customEvent.detail;
      
      setRoom((prevRoom) => ({
        ...prevRoom,
        playerCount,
        host: prevRoom.host ? {
          ...prevRoom.host,
          name: hostName
        } : prevRoom.host
      }));
    };

    // Listen for custom events from the global WebSocket
    globalThis.addEventListener("gameStateUpdate", handleGameStateUpdate);
    globalThis.addEventListener("roomUpdate", handleRoomUpdate);
    globalThis.addEventListener("headerUpdate", handleHeaderUpdate);

    return () => {
      globalThis.removeEventListener("gameStateUpdate", handleGameStateUpdate);
      globalThis.removeEventListener("roomUpdate", handleRoomUpdate);
      globalThis.removeEventListener("headerUpdate", handleHeaderUpdate);
    };
  }, [room.room.id]);

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
            <span class="hidden xs:inline">
              Host: <span>{room.host?.name || "Unknown"}</span>
            </span>
            {playerId === room.host?.id && (
              <span class="ml-1 text-yellow-600 font-semibold">(You)</span>
            )}
            <span class="mx-1">•</span>
            <span id="player-count-display">
              {gameState.players?.length || room.playerCount || 0}/{room.room.maxPlayers} players
            </span>
          </p>
        </div>
        <div class="flex gap-2">
          {/* Settings button - only show for host */}
          {playerId === room.host?.id && (
            <button
              type="button"
              onClick={() => setShowSettingsModal(true)}
              class="flex-shrink-0 px-3 py-2 sm:px-4 sm:py-2 bg-blue-100 text-blue-700 rounded-lg hover:bg-blue-200 active:bg-blue-300 transition-colors text-sm sm:text-base text-center touch-manipulation no-tap-highlight"
              title="Game Settings"
            >
              <span class="xs:hidden">⚙️</span>
              <span class="hidden xs:inline">⚙️ Settings</span>
            </button>
          )}

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

      {/* Game Settings Modal */}
      <GameSettingsModal
        isOpen={showSettingsModal}
        onClose={() => setShowSettingsModal(false)}
        onSave={handleSettingsSave}
        currentSettings={gameState.settings || { maxRounds: 5, roundTimeSeconds: 75 }}
      />
    </div>
  );
}
