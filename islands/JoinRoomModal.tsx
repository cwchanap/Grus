import { useState } from "preact/hooks";
import type { RoomSummary } from "../lib/room-manager.ts";

interface JoinRoomModalProps {
  show: boolean;
  room: RoomSummary | null;
  onClose: () => void;
  onSuccess: (roomId: string, playerId: string) => void;
}

export default function JoinRoomModal({ show, room, onClose, onSuccess }: JoinRoomModalProps) {
  const [playerName, setPlayerName] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    
    if (!playerName.trim()) {
      setError('Player name is required');
      return;
    }

    if (!room) {
      setError('No room selected');
      return;
    }

    // Check if name already exists in room
    const existingPlayer = room.players.find(
      player => player.name.toLowerCase() === playerName.trim().toLowerCase()
    );
    
    if (existingPlayer) {
      setError('A player with this name already exists in the room');
      return;
    }

    try {
      setLoading(true);
      setError('');

      const response = await fetch(`/api/rooms/${room.room.id}/join`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({
          playerName: playerName.trim()
        })
      });

      const data = await response.json();

      if (response.ok) {
        onSuccess(data.roomId, data.playerId);
      } else {
        setError(data.error || 'Failed to join room');
      }
    } catch (error) {
      console.error('Error joining room:', error);
      setError('Failed to join room. Please try again.');
    } finally {
      setLoading(false);
    }
  };

  const handleClose = () => {
    if (!loading) {
      setPlayerName('');
      setError('');
      onClose();
    }
  };

  if (!show || !room) return null;

  return (
    <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
      <div class="bg-white rounded-lg shadow-xl max-w-md w-full">
        <div class="flex justify-between items-center p-6 border-b border-gray-200">
          <h2 class="text-xl font-semibold text-gray-800">Join Room</h2>
          <button
            onClick={handleClose}
            disabled={loading}
            class="text-gray-400 hover:text-gray-600 text-2xl leading-none disabled:opacity-50"
          >
            ×
          </button>
        </div>

        <div class="p-6">
          {/* Room info */}
          <div class="mb-6 p-4 bg-gray-50 rounded-lg">
            <h3 class="font-semibold text-gray-800 mb-2">{room.room.name}</h3>
            <div class="text-sm text-gray-600 space-y-1">
              <div>Host: {room.host?.name || 'Unknown'}</div>
              <div>Players: {room.playerCount}/{room.room.maxPlayers}</div>
            </div>
            
            {room.players.length > 0 && (
              <div class="mt-3">
                <div class="text-sm text-gray-600 mb-2">Current players:</div>
                <div class="flex flex-wrap gap-1">
                  {room.players.map((player) => (
                    <span
                      key={player.id}
                      class={`text-xs px-2 py-1 rounded-full ${
                        player.isHost 
                          ? 'bg-yellow-100 text-yellow-800' 
                          : 'bg-gray-100 text-gray-700'
                      }`}
                    >
                      {player.name}
                      {player.isHost && ' 👑'}
                    </span>
                  ))}
                </div>
              </div>
            )}
          </div>

          <form onSubmit={handleSubmit}>
            {error && (
              <div class="mb-4 p-3 bg-red-50 border border-red-200 rounded-lg text-red-700 text-sm">
                {error}
              </div>
            )}

            <div class="mb-6">
              <label class="block text-sm font-medium text-gray-700 mb-2">
                Your Name *
              </label>
              <input
                type="text"
                value={playerName}
                onInput={(e) => setPlayerName((e.target as HTMLInputElement).value)}
                placeholder="Enter your name"
                maxLength={30}
                disabled={loading}
                class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:bg-gray-100"
                required
                autoFocus
              />
              <div class="text-xs text-gray-500 mt-1">
                {playerName.length}/30 characters
              </div>
            </div>

            <div class="flex space-x-3">
              <button
                type="button"
                onClick={handleClose}
                disabled={loading}
                class="flex-1 px-4 py-2 border border-gray-300 text-gray-700 rounded-lg hover:bg-gray-50 transition-colors disabled:opacity-50"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={loading || !playerName.trim() || !room.canJoin}
                class="flex-1 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {loading ? (
                  <span class="flex items-center justify-center">
                    <span class="animate-spin mr-2">⟳</span>
                    Joining...
                  </span>
                ) : (
                  'Join Room'
                )}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  );
}