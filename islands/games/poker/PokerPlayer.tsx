import { PokerPlayer as PokerPlayerType } from "../../../types/games/poker.ts";

interface PokerPlayerProps {
  player: PokerPlayerType;
  isCurrentPlayer: boolean;
  isLocalPlayer: boolean;
  position: number;
}

export default function PokerPlayer(
  { player, isCurrentPlayer, isLocalPlayer, position }: PokerPlayerProps,
) {
  const totalPlayers = 8;
  const angle = (360 / totalPlayers) * position;
  const transform = `rotate(${angle}deg) translate(200px) rotate(-${angle}deg)`;

  return (
    <div
      class={`absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-20 h-28 flex flex-col items-center justify-center p-1 rounded-lg shadow-md transition-all duration-300 ${
        isCurrentPlayer ? "ring-2 ring-blue-500" : ""
      }`}
      style={{ transform }}
    >
      <div
        class={`text-center p-1 rounded ${
          player.status === "folded" ? "bg-gray-600" : "bg-gray-800"
        }`}
      >
        <p class="font-bold text-xs truncate">{player.name}</p>
        <p class="text-yellow-400 text-xs">${player.chips}</p>
        {player.isDealer && (
          <div class="absolute -top-2 -right-2 w-6 h-6 bg-white text-black rounded-full flex items-center justify-center font-bold text-xs">
            D
          </div>
        )}
      </div>

      <div class="flex space-x-1 mt-1">
        {player.hand.map((card, i) => (
          <div
            key={i}
            class={`w-6 h-8 rounded text-xs ${
              isLocalPlayer ? "bg-white text-black" : "bg-red-800 border border-red-900"
            }`}
          >
            {isLocalPlayer && (
              <div class="flex flex-col items-center justify-center h-full">
                <span class="text-xs font-bold">{card.rank}</span>
                <span class="text-xs">{card.suit}</span>
              </div>
            )}
          </div>
        ))}
      </div>

      {player.bet > 0 && (
        <div class="absolute -bottom-4 bg-gray-900 text-yellow-300 px-2 py-1 rounded-full text-xs">
          ${player.bet}
        </div>
      )}

      {isCurrentPlayer && (
        <div class="absolute inset-0 bg-blue-500 bg-opacity-20 rounded-lg animate-pulse"></div>
      )}
    </div>
  );
}
