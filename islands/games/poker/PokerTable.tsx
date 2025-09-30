import { h } from "preact";
import { PokerAction, PokerGameState } from "../../../types/games/poker.ts";
import PokerPlayer from "./PokerPlayer.tsx";
import PokerControls from "./PokerControls.tsx";

interface PokerTableProps {
  gameState: PokerGameState;
  playerId: string;
  onAction: (action: PokerAction, amount?: number) => void;
}

export default function PokerTable({ gameState, playerId, onAction }: PokerTableProps) {
  const players = gameState.players || [];
  const localPlayer = players.find((p) => p.id === playerId);

  return (
    <div class="flex flex-col items-center justify-center w-full h-full text-white">
      <div class="relative w-full max-w-4xl aspect-[4/3] bg-green-700 rounded-full border-8 border-yellow-800 shadow-lg">
        <div class="absolute inset-0 flex items-center justify-center">
          <div class="text-center bg-green-600/80 rounded-lg p-4 max-w-xs">
            <h2 class="text-xl font-bold mb-2">Pot: ${gameState.pot || 0}</h2>
            <div class="flex space-x-1 my-2 justify-center">
              {(gameState.communityCards || []).map((card, i) => (
                <div
                  key={i}
                  class="w-12 h-16 bg-white rounded text-black flex items-center justify-center text-sm font-bold"
                >
                  <div class="text-center">
                    <div>{card.rank}</div>
                    <div>{card.suit}</div>
                  </div>
                </div>
              ))}
            </div>
            <p class="text-sm mb-1">Bet: ${gameState.currentBet || 0}</p>
            <p class="text-xs font-semibold">{gameState.bettingRound || "Pre-flop"}</p>
          </div>
        </div>

        {players.map((player, index) => (
          <PokerPlayer
            key={player.id}
            player={player}
            isCurrentPlayer={gameState.currentPlayerId === player.id ||
              (gameState.currentPlayerIndex === index && !gameState.currentPlayerId)}
            isLocalPlayer={player.id === playerId}
            position={index}
          />
        ))}
      </div>

      {localPlayer && (
        <PokerControls
          gameState={gameState}
          playerId={playerId}
          onAction={onAction}
        />
      )}
    </div>
  );
}
