import { signal } from "@preact/signals";
import PokerTable from "./games/poker/PokerTable.tsx";
import ChatRoom from "./core/ChatRoom.tsx";
import Scoreboard from "./Scoreboard.tsx";
import RoomHeader from "./RoomHeader.tsx";
import GameSettingsWrapper from "./GameSettingsWrapper.tsx";
import type { RoomSummary } from "../lib/core/room-manager.ts";
import type { BaseGameState } from "../types/core/game.ts";
import type { PokerAction } from "../types/games/poker.ts";

interface PokerRoomProps {
  room: RoomSummary;
  playerId: string;
  gameState: BaseGameState;
  pokerActionHandler: (action: PokerAction, amount?: number) => void;
}

export default function PokerRoom(
  { room, playerId, gameState, pokerActionHandler }: PokerRoomProps,
) {
  const showSettingsModal = signal(false);

  return (
    <div class="min-h-screen bg-gradient-to-br from-green-900 to-green-700 safe-area-inset">
      {/* Header */}
      <div class="pt-4 px-4 max-w-full mx-auto">
        <RoomHeader
          room={room}
          playerId={playerId}
          gameState={gameState}
        />
      </div>

      {/* Main Poker Layout - CSS Grid */}
      <div class="grid grid-rows-[3fr_1fr] h-[calc(100vh-8rem)] gap-4 p-4">
        {/* Poker Table - Takes 75% (3fr out of 4fr total) */}
        <div class="w-full h-full flex items-center justify-center bg-green-800 rounded-lg shadow-lg border-4 border-yellow-600">
          <PokerTable
            gameState={gameState as any}
            playerId={playerId}
            onAction={pokerActionHandler}
          />
        </div>

        {/* Bottom Panel - Chat and Scoreboard side by side - Takes 25% (1fr out of 4fr total) */}
        <div class="grid grid-cols-[3fr_2fr] gap-4 min-h-0">
          {/* Chat - Takes 60% of bottom width */}
          <div class="bg-white/10 backdrop-blur-sm border border-white/20 rounded-lg shadow-lg">
            <ChatRoom
              roomId={room.room.id}
              currentPlayerId={playerId}
              currentPlayerName={room.players.find((p) => p.id === playerId)?.name || "Unknown"}
            />
          </div>

          {/* Scoreboard - Takes 40% of bottom width */}
          <div class="bg-white/10 backdrop-blur-sm border border-white/20 rounded-lg shadow-lg p-4 flex flex-col min-h-0">
            <div class="flex-1 min-h-0">
              <Scoreboard
                roomId={room.room.id}
                playerId={playerId}
                gameState={gameState}
                onShowSettingsModal={() => {
                  console.log("Poker Settings button clicked");
                  showSettingsModal.value = true;
                }}
              />
            </div>
          </div>
        </div>
      </div>

      {/* Game Settings Modal */}
      <GameSettingsWrapper
        isOpen={showSettingsModal.value}
        onModalClose={() => {
          showSettingsModal.value = false;
        }}
        roomId={room.room.id}
        playerId={playerId}
        currentSettings={(gameState as any).settings}
      />

      {/* Player ID missing warning */}
      {(!playerId || playerId.trim() === "") && (
        <div class="fixed top-20 left-4 right-4 bg-yellow-400/20 border border-yellow-400/30 rounded-lg p-3 backdrop-blur-sm z-50">
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
    </div>
  );
}
