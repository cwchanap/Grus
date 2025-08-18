// Core game lobby component - game agnostic
import { useEffect, useState } from "preact/hooks";
import { Player } from "../../types/core/room.ts";
import { BaseGameSettings } from "../../types/core/game.ts";

interface GameLobbyProps {
  roomId: string;
  players: Player[];
  currentPlayerId: string;
  isHost: boolean;
  gameType: string;
  settings: BaseGameSettings;
  onStartGame: () => void;
  onUpdateSettings: (settings: BaseGameSettings) => void;
  onLeaveRoom: () => void;
  canStartGame: boolean;
  minPlayers: number;
}

export default function GameLobby({
  roomId,
  players,
  currentPlayerId,
  isHost,
  gameType,
  settings,
  onStartGame,
  onUpdateSettings,
  onLeaveRoom,
  canStartGame,
  minPlayers,
}: GameLobbyProps) {
  const [showSettings, setShowSettings] = useState(false);
  const [localSettings, setLocalSettings] = useState(settings);

  useEffect(() => {
    setLocalSettings(settings);
  }, [settings]);

  const handleSettingsSubmit = (e: Event) => {
    e.preventDefault();
    onUpdateSettings(localSettings);
    setShowSettings(false);
  };

  const host = players?.find((p) => p.isHost) || null;

  return (
    <div class="max-w-4xl mx-auto p-6 space-y-6">
      {/* Room Header */}
      <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
        <div class="flex items-center justify-between mb-4">
          <div>
            <h1 class="text-2xl font-bold text-gray-900">Room {roomId}</h1>
            <p class="text-gray-600 capitalize">{gameType.replace("-", " ")} Game</p>
          </div>
          <button
            type="button"
            onClick={onLeaveRoom}
            class="px-4 py-2 text-red-600 hover:text-red-700 hover:bg-red-50 rounded-md transition-colors"
          >
            Leave Room
          </button>
        </div>

        {host && (
          <div class="text-sm text-gray-600">
            Host: <span class="font-medium">{host.name}</span>
          </div>
        )}
      </div>

      {/* Players List */}
      <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
        <h2 class="text-lg font-semibold text-gray-900 mb-4">
          Players ({players?.length || 0}/8)
        </h2>
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
          {players?.map((player) => (
            <div
              key={player.id}
              class={`flex items-center space-x-3 p-3 rounded-lg border ${
                player.id === currentPlayerId
                  ? "bg-blue-50 border-blue-200"
                  : "bg-gray-50 border-gray-200"
              }`}
            >
              <div class="flex-shrink-0">
                <div class="w-8 h-8 bg-gradient-to-br from-blue-400 to-purple-500 rounded-full flex items-center justify-center text-white text-sm font-medium">
                  {player.name.charAt(0).toUpperCase()}
                </div>
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-sm font-medium text-gray-900 truncate">
                  {player.name}
                  {player.id === currentPlayerId && " (You)"}
                </p>
                {player.isHost && <p class="text-xs text-blue-600">Host</p>}
              </div>
            </div>
          )) || (
            <div class="col-span-full text-center py-4 text-gray-500">
              Loading players...
            </div>
          )}
        </div>
      </div>

      {/* Game Settings */}
      <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-semibold text-gray-900">Game Settings</h2>
          {isHost && (
            <button
              type="button"
              onClick={() => setShowSettings(!showSettings)}
              class="px-3 py-1 text-sm text-blue-600 hover:text-blue-700 hover:bg-blue-50 rounded-md transition-colors"
            >
              {showSettings ? "Cancel" : "Edit"}
            </button>
          )}
        </div>

        {showSettings && isHost
          ? (
            <form onSubmit={handleSettingsSubmit} class="space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">
                  Max Rounds
                </label>
                <input
                  type="number"
                  min="1"
                  max="10"
                  value={localSettings.maxRounds}
                  onInput={(e) =>
                    setLocalSettings({
                      ...localSettings,
                      maxRounds: parseInt((e.target as HTMLInputElement).value),
                    })}
                  class="w-full px-3 py-2 border border-gray-300 rounded-md text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                />
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">
                  Round Time (seconds)
                </label>
                <input
                  type="number"
                  min="30"
                  max="180"
                  value={localSettings.roundTimeSeconds}
                  onInput={(e) =>
                    setLocalSettings({
                      ...localSettings,
                      roundTimeSeconds: parseInt((e.target as HTMLInputElement).value),
                    })}
                  class="w-full px-3 py-2 border border-gray-300 rounded-md text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                />
              </div>
              <div class="flex space-x-2">
                <button
                  type="submit"
                  class="px-4 py-2 bg-blue-500 text-white text-sm font-medium rounded-md hover:bg-blue-600 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 transition-colors"
                >
                  Save Settings
                </button>
                <button
                  type="button"
                  onClick={() => setShowSettings(false)}
                  class="px-4 py-2 text-gray-600 text-sm font-medium rounded-md hover:bg-gray-100 focus:outline-none focus:ring-2 focus:ring-gray-500 focus:ring-offset-2 transition-colors"
                >
                  Cancel
                </button>
              </div>
            </form>
          )
          : (
            <div class="space-y-2 text-sm text-gray-600">
              <div>
                Max Rounds: <span class="font-medium">{settings.maxRounds}</span>
              </div>
              <div>
                Round Time: <span class="font-medium">{settings.roundTimeSeconds}s</span>
              </div>
            </div>
          )}
      </div>

      {/* Start Game Button */}
      <div class="bg-white rounded-lg shadow-sm border border-gray-200 p-6">
        {isHost
          ? (
            <div class="space-y-3">
              <button
                type="button"
                onClick={onStartGame}
                disabled={!canStartGame}
                class="w-full px-6 py-3 bg-green-500 text-white font-medium rounded-lg hover:bg-green-600 focus:outline-none focus:ring-2 focus:ring-green-500 focus:ring-offset-2 disabled:bg-gray-300 disabled:cursor-not-allowed transition-colors"
              >
                Start Game
              </button>
              {!canStartGame && (
                <p class="text-sm text-gray-600 text-center">
                  Need at least {minPlayers} players to start the game
                </p>
              )}
            </div>
          )
          : (
            <div class="text-center text-gray-600">
              <p>Waiting for host to start the game...</p>
              <p class="text-sm mt-1">
                {(players?.length || 0) < minPlayers &&
                  `Need ${minPlayers - (players?.length || 0)} more player(s)`}
              </p>
            </div>
          )}
      </div>
    </div>
  );
}
