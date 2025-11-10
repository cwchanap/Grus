import { h } from "preact";
import { useState } from "preact/hooks";
import { PokerAction, PokerGameState } from "../../../types/games/poker.ts";
import { Button } from "../../../components/ui/button.tsx";
import { Input } from "../../../components/ui/input.tsx";

interface PokerControlsProps {
  gameState: PokerGameState;
  playerId: string;
  onAction: (action: PokerAction, amount?: number) => void;
}

export default function PokerControls({ gameState, playerId, onAction }: PokerControlsProps) {
  const player = gameState.players.find((p) => p.id === playerId);
  const [raiseAmount, setRaiseAmount] = useState(gameState.minRaise);

  // Only show controls if it's this player's turn
  const currentPlayer = gameState.players[gameState.currentPlayerIndex];
  if (!player || currentPlayer?.id !== playerId) {
    return null;
  }

  const canCheck = player.bet === gameState.currentBet;
  const callAmount = Math.min(player.chips, gameState.currentBet - player.bet);

  const handleRaiseChange = (e: h.JSX.TargetedEvent<HTMLInputElement>) => {
    const amount = parseInt(e.currentTarget.value, 10);
    setRaiseAmount(Math.max(gameState.minRaise, Math.min(player.chips, amount)));
  };

  const handleAllIn = () => {
    onAction(PokerAction.ALL_IN);
  };

  return (
    <div class="fixed bottom-0 left-0 right-0 bg-gray-900 p-4 shadow-lg md:relative md:bg-transparent md:shadow-none">
      <div class="flex flex-col md:flex-row items-center justify-center space-y-4 md:space-y-0 md:space-x-4">
        <div class="flex space-x-2">
          <Button onClick={() => onAction(PokerAction.FOLD)} variant="destructive">Fold</Button>
          {canCheck
            ? <Button onClick={() => onAction(PokerAction.CHECK)} variant="secondary">Check</Button>
            : (
              <Button onClick={() => onAction(PokerAction.CALL, callAmount)} variant="secondary">
                Call ${callAmount}
              </Button>
            )}
        </div>

        <div class="flex items-center space-x-2">
          <Button
            onClick={() => onAction(PokerAction.RAISE, raiseAmount)}
            disabled={raiseAmount > player.chips}
          >
            Raise to ${raiseAmount}
          </Button>
          <Input
            type="range"
            min={gameState.minRaise}
            max={player.chips}
            value={raiseAmount}
            onInput={handleRaiseChange}
            class="w-32 md:w-48"
          />
          <Input
            type="number"
            value={raiseAmount}
            onChange={handleRaiseChange}
            class="w-24 bg-gray-800 text-white"
          />
        </div>

        <Button onClick={handleAllIn} variant="destructive">All-in (${player.chips})</Button>
      </div>
      <p class="text-center text-sm mt-2 md:hidden">Your Chips: ${player.chips}</p>
    </div>
  );
}
