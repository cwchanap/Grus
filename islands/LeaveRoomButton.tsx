import { useState } from "preact/hooks";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from "../components/ui/dialog.tsx";

interface LeaveRoomButtonProps {
  roomId: string;
  playerId: string;
  className?: string;
  children?: any;
}

export default function LeaveRoomButton({
  roomId,
  playerId,
  className = "",
  children,
}: LeaveRoomButtonProps) {
  const [isLeaving, setIsLeaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showConfirmDialog, setShowConfirmDialog] = useState(false);

  const handleLeaveRoom = async () => {
    if (isLeaving) return;

    setIsLeaving(true);
    setError(null);

    // If no playerId, just redirect to lobby (fallback behavior)
    if (!playerId || playerId.trim() === "") {
      console.warn("No playerId provided, redirecting to lobby without API call");
      globalThis.location.href = "/";
      return;
    }

    try {
      const response = await fetch(`/api/rooms/${roomId}/leave`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ playerId }),
      });

      if (!response.ok) {
        // Try to parse error response
        let errorMessage = "Failed to leave room";
        try {
          const data = await response.json();
          errorMessage = data.error || errorMessage;
        } catch {
          // If JSON parsing fails, use status text
          errorMessage = response.statusText || errorMessage;
        }
        throw new Error(errorMessage);
      }

      // Successfully left room - redirect to lobby
      globalThis.location.href = "/";
    } catch (error) {
      console.error("Error leaving room:", error);
      setError(error instanceof Error ? error.message : "Failed to leave room");
      setIsLeaving(false);

      // After 3 seconds, offer fallback option to go to lobby anyway
      setTimeout(() => {
        if (confirm("Unable to properly leave the room. Go to lobby anyway?")) {
          globalThis.location.href = "/";
        }
      }, 3000);
    }
  };

  const handleClick = (e: Event) => {
    e.preventDefault();

    // Show confirmation dialog for better UX
    setShowConfirmDialog(true);
  };

  const handleConfirmLeave = () => {
    setShowConfirmDialog(false);
    handleLeaveRoom();
  };

  const handleCancelLeave = () => {
    setShowConfirmDialog(false);
  };

  return (
    <>
      <button
        type="button"
        onClick={handleClick}
        disabled={isLeaving}
        class={`${className} ${isLeaving ? "opacity-50 cursor-not-allowed" : ""}`}
        title={error
          ? error
          : isLeaving
          ? "Leaving room..."
          : !playerId || playerId.trim() === ""
          ? "Return to lobby (player info missing)"
          : "Leave room"}
      >
        {isLeaving
          ? (
            <>
              <span class="xs:hidden">Leaving...</span>
              <span class="hidden xs:inline">← Leaving...</span>
            </>
          )
          : (
            children || (
              <>
                <span class="xs:hidden">← Lobby</span>
                <span class="hidden xs:inline">← Back to Lobby</span>
              </>
            )
          )}
      </button>

      {/* Confirmation Dialog */}
      <Dialog open={showConfirmDialog} onOpenChange={setShowConfirmDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Leave Room</DialogTitle>
            <DialogDescription>
              {!playerId || playerId.trim() === ""
                ? "Return to lobby? (Note: Unable to properly leave room due to missing player information)"
                : "Are you sure you want to leave this room?"}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <button
              type="button"
              onClick={handleCancelLeave}
              class="px-4 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={handleConfirmLeave}
              class="px-4 py-2 text-sm font-medium text-white bg-red-600 border border-transparent rounded-md hover:bg-red-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-red-500"
            >
              Leave Room
            </button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Error message */}
      {error && (
        <div class="fixed top-4 right-4 bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded z-50 max-w-sm">
          <div class="flex items-center">
            <span class="text-red-500 mr-2">⚠️</span>
            <div>
              <strong class="font-bold">Error:</strong>
              <span class="block sm:inline">{error}</span>
            </div>
            <button
              type="button"
              onClick={() => setError(null)}
              class="ml-2 text-red-500 hover:text-red-700"
            >
              ×
            </button>
          </div>
        </div>
      )}
    </>
  );
}
