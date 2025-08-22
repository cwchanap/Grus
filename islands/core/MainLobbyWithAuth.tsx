import { useEffect, useState } from "preact/hooks";
import { Button } from "../../components/ui/button.tsx";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "../../components/ui/card.tsx";
import { ScrollArea } from "../../components/ui/scroll-area.tsx";
import { Badge } from "../../components/ui/badge.tsx";
import { Gamepad2, LogIn, LogOut, Plus, User, Users } from "lucide-react";
import CreateRoomModal from "../CreateRoomModal.tsx";
import JoinRoomModal from "../JoinRoomModal.tsx";
import ConnectionStatus from "../../components/ConnectionStatus.tsx";
import type { UserPayload } from "../../lib/auth/auth-utils.ts";
import type { RoomSummary } from "../../lib/core/room-manager.ts";

interface MainLobbyWithAuthProps {
  rooms: RoomSummary[];
}

export default function MainLobbyWithAuth({ rooms: initialRooms }: MainLobbyWithAuthProps) {
  const [rooms, setRooms] = useState<RoomSummary[]>(initialRooms);
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [showJoinModal, setShowJoinModal] = useState(false);
  const [selectedRoomId, setSelectedRoomId] = useState<string | null>(null);
  const [user, setUser] = useState<UserPayload | null>(null);
  const [loading, setLoading] = useState(true);

  // Check authentication status on mount
  useEffect(() => {
    checkAuthStatus();
  }, []);

  const checkAuthStatus = async () => {
    try {
      const response = await fetch("/api/auth/me");
      if (response.ok) {
        const data = await response.json();
        setUser(data.user);
      }
    } catch (error) {
      console.error("Error checking auth status:", error);
    } finally {
      setLoading(false);
    }
  };

  const handleLogout = async () => {
    try {
      await fetch("/api/auth/logout", { method: "POST" });
      setUser(null);
      // Optionally refresh the page
      globalThis.location?.reload?.();
    } catch (error) {
      console.error("Error logging out:", error);
    }
  };

  // Helper: refresh rooms from API
  const refreshRooms = async () => {
    try {
      const response = await fetch("/api/rooms");
      if (response.ok) {
        const data = await response.json();
        setRooms(data.rooms || []);
      } else {
        console.error("Failed to refresh rooms");
      }
    } catch (err) {
      console.error("Error refreshing rooms:", err);
    }
  };

  // WebSocket connection setup
  useEffect(() => {
    try {
      const protocol = globalThis.location?.protocol === "https:" ? "wss:" : "ws:";
      const host = globalThis.location?.host ?? "";
      const wsUrl = `${protocol}//${host}/api/websocket`;

      const ws = new WebSocket(wsUrl);

      ws.onopen = () => {
        console.log("Lobby WebSocket connected");
        try {
          ws.send(JSON.stringify({ type: "subscribe-lobby", data: {} }));
        } catch (err) {
          console.error("Failed to send subscribe-lobby:", err);
        }
      };

      ws.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data);
          if (message.type === "lobby-data") {
            const roomsFromServer = message?.data?.rooms;
            if (Array.isArray(roomsFromServer)) {
              setRooms(roomsFromServer as RoomSummary[]);
            } else {
              // Fallback to REST if payload missing
              refreshRooms();
            }
          } else if (message.type === "lobby-update") {
            // Refresh room list when lobby updates
            refreshRooms();
          }
        } catch (err) {
          console.error("Error parsing WebSocket message:", err);
        }
      };

      ws.onerror = (err) => {
        console.error("WebSocket error:", err);
      };

      ws.onclose = () => {
        console.log("Lobby WebSocket disconnected");
      };

      return () => {
        try {
          ws.close();
        } catch (_) {
          // ignore
        }
      };
    } catch (err) {
      console.error("Failed to init WebSocket:", err);
    }
  }, []);

  const handleRoomClick = (roomId: string) => {
    const room = rooms.find((r) => r.room.id === roomId);
    if (room?.canJoin) {
      setSelectedRoomId(roomId);
      setShowJoinModal(true);
    }
  };

  const handleRoomJoined = (roomId: string, playerId: string) => {
    globalThis.location.href = `/room/${roomId}?playerId=${playerId}`;
  };

  return (
    <div className="min-h-screen bg-gradient-to-br from-purple-600 to-blue-600 p-4">
      <div className="max-w-6xl mx-auto">
        {/* Header with Auth Status */}
        <div className="mb-8 text-center">
          <div className="flex justify-between items-start mb-4">
            <ConnectionStatus />

            {/* Auth Status */}
            <div className="flex items-center gap-2">
              {!loading && (
                user
                  ? (
                    <div className="flex items-center gap-3 bg-white/10 backdrop-blur-sm rounded-lg px-4 py-2">
                      <User className="w-5 h-5 text-white" />
                      <span className="text-white font-medium">
                        {user.name || user.username}
                      </span>
                      <Button
                        onClick={handleLogout}
                        variant="ghost"
                        size="sm"
                        className="text-white hover:bg-white/20"
                      >
                        <LogOut className="w-4 h-4 mr-1" />
                        Logout
                      </Button>
                    </div>
                  )
                  : (
                    <a href="/login">
                      <Button
                        variant="default"
                        className="bg-white text-purple-600 hover:bg-gray-100"
                      >
                        <LogIn className="w-4 h-4 mr-2" />
                        Login
                      </Button>
                    </a>
                  )
              )}
            </div>
          </div>

          <h1 className="text-5xl font-bold text-white mb-2 flex items-center justify-center gap-3">
            <Gamepad2 className="w-12 h-12" />
            Grus Drawing Game
          </h1>
          <p className="text-white/80 text-lg">
            Draw, guess, and have fun with friends!
          </p>
        </div>

        {/* Main Content */}
        <div className="grid md:grid-cols-3 gap-6">
          {/* Active Rooms */}
          <div className="md:col-span-2">
            <Card className="bg-white/95 backdrop-blur-sm">
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Users className="w-5 h-5" />
                  Active Rooms
                </CardTitle>
                <CardDescription>
                  Join a room to start playing or create your own
                </CardDescription>
              </CardHeader>
              <CardContent>
                <ScrollArea className="h-[400px] pr-4">
                  {rooms.length === 0
                    ? (
                      <div className="text-center py-12 text-gray-500">
                        <Gamepad2 className="w-16 h-16 mx-auto mb-4 opacity-50" />
                        <p>No active rooms yet.</p>
                        <p className="text-sm mt-2">Be the first to create one!</p>
                      </div>
                    )
                    : (
                      <div className="space-y-3">
                        {rooms.map((summary) => (
                          <Card
                            key={summary.room.id}
                            className={`transition-all hover:shadow-lg ${
                              summary.canJoin
                                ? "hover:scale-[1.02] hover:bg-purple-50 cursor-pointer"
                                : "opacity-75"
                            }`}
                          >
                            <button
                              type="button"
                              onClick={() => handleRoomClick(summary.room.id)}
                              disabled={!summary.canJoin}
                              className="w-full text-left disabled:cursor-not-allowed"
                            >
                              <CardContent className="p-4">
                                <div className="flex items-center justify-between">
                                  <div>
                                    <div className="flex items-center gap-2 mb-1">
                                      <h3 className="font-semibold">{summary.room.id}</h3>
                                      <Badge
                                        variant={summary.canJoin ? "default" : "secondary"}
                                      >
                                        {summary.canJoin ? "Open" : "Full"}
                                      </Badge>
                                    </div>
                                    <p className="text-sm text-gray-600">
                                      Host: {summary.host?.name || "Unknown"}
                                    </p>
                                  </div>
                                  <div className="text-right">
                                    <div className="flex items-center gap-1 text-sm mb-1">
                                      <Users className="w-4 h-4" />
                                      <span className="font-medium">
                                        {summary.playerCount}/{summary.room.maxPlayers}
                                      </span>
                                    </div>
                                    <Badge variant="outline" className="text-xs">
                                      {summary.room.gameType}
                                    </Badge>
                                  </div>
                                </div>
                              </CardContent>
                            </button>
                          </Card>
                        ))}
                      </div>
                    )}
                </ScrollArea>
              </CardContent>
            </Card>
          </div>

          {/* Quick Actions */}
          <div className="space-y-4">
            <Card className="bg-white/95 backdrop-blur-sm">
              <CardHeader>
                <CardTitle>Quick Actions</CardTitle>
              </CardHeader>
              <CardContent className="space-y-3">
                <Button
                  className="w-full"
                  size="lg"
                  onClick={() => setShowCreateModal(true)}
                  data-testid="create-room-button"
                >
                  <Plus className="w-5 h-5 mr-2" />
                  Create Room
                </Button>
                <Button
                  className="w-full"
                  variant="outline"
                  size="lg"
                  onClick={() => setShowJoinModal(true)}
                >
                  <Users className="w-5 h-5 mr-2" />
                  Join Room
                </Button>
              </CardContent>
            </Card>

            {/* Game Stats */}
            <Card className="bg-white/95 backdrop-blur-sm">
              <CardHeader>
                <CardTitle>Game Stats</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="space-y-2 text-sm">
                  <div className="flex justify-between">
                    <span className="text-gray-600">Active Rooms</span>
                    <span className="font-semibold">{rooms.length}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-gray-600">Players Online</span>
                    <span className="font-semibold">
                      {rooms.reduce((acc, r) => acc + r.playerCount, 0)}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-gray-600">Available Slots</span>
                    <span className="font-semibold">
                      {rooms
                        .filter((r) => r.canJoin)
                        .reduce((acc, s) => acc + (s.room.maxPlayers - s.playerCount), 0)}
                    </span>
                  </div>
                </div>
              </CardContent>
            </Card>

            {/* How to Play */}
            <Card className="bg-white/95 backdrop-blur-sm">
              <CardHeader>
                <CardTitle>How to Play</CardTitle>
              </CardHeader>
              <CardContent>
                <ol className="space-y-2 text-sm text-gray-600">
                  <li>1. Create or join a room</li>
                  <li>2. Take turns drawing and guessing</li>
                  <li>3. Earn points for correct guesses</li>
                  <li>4. Player with most points wins!</li>
                </ol>
              </CardContent>
            </Card>
          </div>
        </div>
      </div>

      {/* Modals */}
      <CreateRoomModal
        show={showCreateModal}
        onClose={() => setShowCreateModal(false)}
        onSuccess={() => {
          setShowCreateModal(false);
          refreshRooms();
        }}
      />
      <JoinRoomModal
        show={showJoinModal}
        room={rooms.find((r) => r.room.id === selectedRoomId) ?? null}
        onClose={() => {
          setShowJoinModal(false);
          setSelectedRoomId(null);
        }}
        onSuccess={handleRoomJoined}
      />
    </div>
  );
}
