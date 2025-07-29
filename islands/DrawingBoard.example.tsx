/**
 * Example usage of the DrawingBoard component in a game room
 * This demonstrates how to integrate the DrawingBoard with game state management
 */

import { useEffect, useState } from "preact/hooks";
import DrawingBoard from "./DrawingBoard.tsx";
import { GameState } from "../types/game.ts";

interface GameRoomExampleProps {
  roomId: string;
  playerId: string;
  playerName: string;
}

export default function GameRoomExample({ roomId, playerId, playerName }: GameRoomExampleProps) {
  const [gameState, setGameState] = useState<GameState>({
    roomId,
    currentDrawer: "player1",
    currentWord: "house",
    roundNumber: 1,
    timeRemaining: 60,
    phase: "drawing",
    players: [
      { id: "player1", name: "Alice", isHost: true, isConnected: true, lastActivity: Date.now() },
      { id: "player2", name: "Bob", isHost: false, isConnected: true, lastActivity: Date.now() },
      {
        id: playerId,
        name: playerName,
        isHost: false,
        isConnected: true,
        lastActivity: Date.now(),
      },
    ],
    scores: { player1: 0, player2: 0, [playerId]: 0 },
    drawingData: [],
    correctGuesses: [],
    chatMessages: [],
  });

  const [timeRemaining, setTimeRemaining] = useState(gameState.timeRemaining);

  // Simulate game timer
  useEffect(() => {
    if (gameState.phase === "drawing" && timeRemaining > 0) {
      const timer = setTimeout(() => {
        setTimeRemaining((prev) => prev - 1);
        setGameState((prev) => ({ ...prev, timeRemaining: timeRemaining - 1 }));
      }, 1000);

      return () => clearTimeout(timer);
    }
  }, [gameState.phase, timeRemaining]);

  // Handle game state updates from WebSocket
  const handleGameStateUpdate = (newGameState: GameState) => {
    setGameState(newGameState);
    setTimeRemaining(newGameState.timeRemaining);
  };

  // Simulate phase transitions
  const handlePhaseTransition = (newPhase: GameState["phase"]) => {
    setGameState((prev) => ({ ...prev, phase: newPhase }));
  };

  // Simulate turn rotation
  const handleNextTurn = () => {
    const currentIndex = gameState.players.findIndex((p) => p.id === gameState.currentDrawer);
    const nextIndex = (currentIndex + 1) % gameState.players.length;
    const nextDrawer = gameState.players[nextIndex].id;

    setGameState((prev) => ({
      ...prev,
      currentDrawer: nextDrawer,
      phase: "drawing",
      timeRemaining: 60,
    }));
    setTimeRemaining(60);
  };

  return (
    <div class="game-room-example max-w-6xl mx-auto p-4">
      <div class="grid grid-cols-1 lg:grid-cols-4 gap-6">
        {/* Game Info Panel */}
        <div class="lg:col-span-1">
          <div class="bg-white rounded-lg shadow-md p-4 mb-4">
            <h2 class="text-xl font-bold mb-4">Game Info</h2>
            <div class="space-y-2 text-sm">
              <div>
                <strong>Room:</strong> {roomId}
              </div>
              <div>
                <strong>Round:</strong> {gameState.roundNumber}
              </div>
              <div>
                <strong>Phase:</strong> {gameState.phase}
              </div>
              <div>
                <strong>Time:</strong> {timeRemaining}s
              </div>
              {gameState.phase === "drawing" && (
                <div>
                  <strong>Word:</strong>{" "}
                  {gameState.currentDrawer === playerId ? gameState.currentWord : "???"}
                </div>
              )}
            </div>
          </div>

          {/* Players List */}
          <div class="bg-white rounded-lg shadow-md p-4 mb-4">
            <h3 class="text-lg font-semibold mb-3">Players</h3>
            <div class="space-y-2">
              {gameState.players.map((player) => (
                <div
                  key={player.id}
                  class={`flex items-center justify-between p-2 rounded ${
                    player.id === gameState.currentDrawer ? "bg-blue-100" : "bg-gray-50"
                  }`}
                >
                  <div class="flex items-center gap-2">
                    <div
                      class={`w-2 h-2 rounded-full ${
                        player.isConnected ? "bg-green-500" : "bg-red-500"
                      }`}
                    >
                    </div>
                    <span
                      class={`text-sm ${
                        player.id === gameState.currentDrawer ? "font-semibold" : ""
                      }`}
                    >
                      {player.name}
                      {player.id === playerId && " (You)"}
                      {player.isHost && " 👑"}
                    </span>
                  </div>
                  <span class="text-sm text-gray-600">
                    {gameState.scores[player.id] || 0} pts
                  </span>
                </div>
              ))}
            </div>
          </div>

          {/* Game Controls */}
          <div class="bg-white rounded-lg shadow-md p-4">
            <h3 class="text-lg font-semibold mb-3">Controls</h3>
            <div class="space-y-2">
              <button
                onClick={() => handlePhaseTransition("waiting")}
                class="w-full px-3 py-2 bg-yellow-500 text-white rounded hover:bg-yellow-600 text-sm"
              >
                Pause Game
              </button>
              <button
                onClick={() => handlePhaseTransition("drawing")}
                class="w-full px-3 py-2 bg-green-500 text-white rounded hover:bg-green-600 text-sm"
              >
                Resume Game
              </button>
              <button
                onClick={handleNextTurn}
                class="w-full px-3 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 text-sm"
              >
                Next Turn
              </button>
              <button
                onClick={() => handlePhaseTransition("results")}
                class="w-full px-3 py-2 bg-purple-500 text-white rounded hover:bg-purple-600 text-sm"
              >
                Show Results
              </button>
            </div>
          </div>
        </div>

        {/* Drawing Board */}
        <div class="lg:col-span-3">
          <div class="bg-white rounded-lg shadow-md p-4">
            <DrawingBoard
              roomId={roomId}
              playerId={playerId}
              gameState={gameState}
              onGameStateUpdate={handleGameStateUpdate}
              width={800}
              height={600}
              className="w-full"
            />
          </div>
        </div>
      </div>

      {/* Chat Area (Placeholder) */}
      <div class="mt-6 bg-white rounded-lg shadow-md p-4">
        <h3 class="text-lg font-semibold mb-3">Chat & Guesses</h3>
        <div class="bg-gray-50 rounded p-4 h-32 overflow-y-auto mb-3">
          <div class="text-sm text-gray-600 italic">
            Chat messages and guesses would appear here...
          </div>
        </div>
        <div class="flex gap-2">
          <input
            type="text"
            placeholder={gameState.currentDrawer === playerId
              ? "You're drawing!"
              : "Type your guess..."}
            disabled={gameState.currentDrawer === playerId}
            class="flex-1 px-3 py-2 border rounded focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
          <button
            disabled={gameState.currentDrawer === playerId}
            class="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 disabled:bg-gray-400"
          >
            Send
          </button>
        </div>
      </div>

      {/* Instructions */}
      <div class="mt-6 bg-blue-50 rounded-lg p-4">
        <h3 class="text-lg font-semibold mb-2">How to Play</h3>
        <div class="text-sm text-gray-700 space-y-1">
          <p>
            <strong>When it's your turn to draw:</strong>{" "}
            Use the drawing tools to illustrate the given word. Others will try to guess!
          </p>
          <p>
            <strong>When others are drawing:</strong>{" "}
            Watch the drawing and type your guesses in the chat. Faster correct guesses earn more
            points!
          </p>
          <p>
            <strong>Game phases:</strong> Drawing → Guessing → Results → Next Turn
          </p>
        </div>
      </div>
    </div>
  );
}
