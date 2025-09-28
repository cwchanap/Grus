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
  const localPlayer = gameState.players.find((p) => p.id === playerId);

  return (
    <div class="flex flex-col items-center justify-center bg-green-800 min-h-screen text-white">
      <div class="relative w-[90vw] h-[90vw] md:w-[600px] md:h-[600px] bg-green-700 rounded-full border-8 border-yellow-800 shadow-lg">
        <div class="absolute inset-0 flex items-center justify-center">
          <div class="text-center">
            <h2 class="text-2xl font-bold">Pot: ${gameState.pot}</h2>
            <div class="flex space-x-2 my-2">
              {gameState.communityCards.map((card, i) => (
                <div
                  key={i}
                  class="w-16 h-24 bg-white rounded-lg text-black flex items-center justify-center text-2xl font-bold"
                >
                  {card.rank}
                  {card.suit}
                </div>
              ))}
            </div>
            <p class="text-lg">Current Bet: ${gameState.currentBet}</p>
            <p class="text-md font-semibold">{gameState.bettingRound}</p>
          </div>
        </div>

        {gameState.players.map((player, index) => (
          <PokerPlayer
            key={player.id}
            player={player}
            isCurrentPlayer={gameState.currentPlayerId === player.id}
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
