import { useState } from "preact/hooks";

interface CreateRoomModalProps {
  show: boolean;
  onClose: () => void;
  onSuccess: () => void;
}

export default function CreateRoomModal(
  { show, onClose, onSuccess: _onSuccess }: CreateRoomModalProps,
) {
  const [formData, setFormData] = useState({
    roomName: "",
    hostName: "",
    gameType: "drawing",
    maxPlayers: 8,
    isPrivate: false,
  });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const handleSubmit = async (e: Event) => {
    e.preventDefault();

    if (!formData.roomName.trim() || !formData.hostName.trim()) {
      setError("Room name and host name are required");
      return;
    }

    if (formData.maxPlayers < 2 || formData.maxPlayers > 16) {
      setError("Max players must be between 2 and 16");
      return;
    }

    try {
      setLoading(true);
      setError("");

      const response = await fetch("/api/rooms", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          name: formData.roomName.trim(),
          hostName: formData.hostName.trim(),
          gameType: formData.gameType,
          maxPlayers: formData.maxPlayers,
          isPrivate: formData.isPrivate,
        }),
      });

      const data = await response.json();

      if (response.ok) {
        // Navigate to the created room
        globalThis.location.href = `/room/${data.roomId}?playerId=${data.playerId}`;
      } else {
        setError(data.error || "Failed to create room");
      }
    } catch (error) {
      console.error("Error creating room:", error);
      setError("Failed to create room. Please try again.");
    } finally {
      setLoading(false);
    }
  };

  const handleClose = () => {
    if (!loading) {
      setFormData({ roomName: "", hostName: "", gameType: "drawing", maxPlayers: 8, isPrivate: false });
      setError("");
      onClose();
    }
  };

  if (!show) return null;

  return (
    <div
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
      role="dialog"
      aria-modal="true"
      data-testid="create-room-modal"
    >
      <div class="bg-white rounded-lg shadow-xl max-w-md w-full">
        <div class="flex justify-between items-center p-6 border-b border-gray-200">
          <h2 class="text-xl font-semibold text-gray-800">Create New Room</h2>
          <button
            type="button"
            onClick={handleClose}
            disabled={loading}
            class="text-gray-400 hover:text-gray-600 text-2xl leading-none disabled:opacity-50"
          >
            ×
          </button>
        </div>

        <form onSubmit={handleSubmit} class="p-6">
          {error && (
            <div class="mb-4 p-3 bg-red-50 border border-red-200 rounded-lg text-red-700 text-sm">
              {error}
            </div>
          )}

          <div class="mb-4">
            <label class="block text-sm font-medium text-gray-700 mb-2">
              Room Name *
            </label>
            <input
              type="text"
              value={formData.roomName}
              onInput={(e) =>
                setFormData((prev) => ({
                  ...prev,
                  roomName: (e.target as HTMLInputElement).value,
                }))}
              placeholder="Enter room name"
              maxLength={50}
              disabled={loading}
              class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:bg-gray-100"
              data-testid="room-name-input"
              required
            />
            <div class="text-xs text-gray-500 mt-1">
              {formData.roomName.length}/50 characters
            </div>
          </div>

          <div class="mb-4">
            <label class="block text-sm font-medium text-gray-700 mb-2">
              Your Name *
            </label>
            <input
              type="text"
              value={formData.hostName}
              onInput={(e) =>
                setFormData((prev) => ({
                  ...prev,
                  hostName: (e.target as HTMLInputElement).value,
                }))}
              placeholder="Enter your name"
              maxLength={30}
              disabled={loading}
              class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:bg-gray-100"
              data-testid="host-name-input"
              required
            />
            <div class="text-xs text-gray-500 mt-1">
              {formData.hostName.length}/30 characters
            </div>
          </div>

          <div class="mb-4">
            <label class="block text-sm font-medium text-gray-700 mb-2">
              Game Type
            </label>
            <select
              value={formData.gameType}
              onChange={(e) =>
                setFormData((prev) => ({
                  ...prev,
                  gameType: (e.target as HTMLSelectElement).value,
                }))}
              disabled={loading}
              class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:bg-gray-100"
            >
              <option value="drawing">🎨 Drawing & Guessing</option>
              {/* Future game types can be added here */}
            </select>
          </div>

          <div class="mb-6">
            <label class="block text-sm font-medium text-gray-700 mb-2">
              Max Players
            </label>
            <select
              value={formData.maxPlayers}
              onChange={(e) =>
                setFormData((prev) => ({
                  ...prev,
                  maxPlayers: parseInt((e.target as HTMLSelectElement).value),
                }))}
              disabled={loading}
              class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:bg-gray-100"
            >
              {Array.from({ length: 15 }, (_, i) => i + 2).map((num) => (
                <option key={num} value={num}>
                  {num} players
                </option>
              ))}
            </select>
          </div>

          <div class="mb-6">
            <label class="flex items-center space-x-3">
              <input
                type="checkbox"
                checked={formData.isPrivate}
                onChange={(e) =>
                  setFormData((prev) => ({
                    ...prev,
                    isPrivate: (e.target as HTMLInputElement).checked,
                  }))}
                disabled={loading}
                class="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 disabled:opacity-50"
              />
              <div>
                <span class="text-sm font-medium text-gray-700">Private Room</span>
                <p class="text-xs text-gray-500 mt-1">
                  Private rooms won't appear on the public dashboard and can only be joined via direct link
                </p>
              </div>
            </label>
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
              disabled={loading || !formData.roomName.trim() || !formData.hostName.trim()}
              class="flex-1 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              data-testid="create-room-submit"
            >
              {loading
                ? (
                  <span class="flex items-center justify-center">
                    <span class="animate-spin mr-2">⟳</span>
                    Creating...
                  </span>
                )
                : (
                  "Create Room"
                )}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
