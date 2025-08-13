import { Handlers, PageProps } from "$fresh/server.ts";
import MainLobby from "../islands/core/MainLobby.tsx";
// import { getDatabaseService } from "../lib/database-factory.ts";
import { RoomManager, RoomSummary } from "../lib/core/room-manager.ts";

interface LobbyData {
  rooms: RoomSummary[];
  error?: string;
  isDev: boolean;
}

export const handler: Handlers<LobbyData> = {
  async GET(_req, ctx) {
    try {
      const roomManager = new RoomManager();
      const isDev = Deno.env.get("DENO_ENV") !== "production";

      // Get active rooms with automatic cleanup
      const result = await roomManager.getActiveRoomsWithCleanup(20);

      if (!result.success) {
        return ctx.render({
          rooms: [],
          error: result.error || "Failed to load rooms",
          isDev,
        });
      }

      return ctx.render({
        rooms: result.data || [],
        isDev,
      });
    } catch (error) {
      console.error("Error loading lobby:", error);
      return ctx.render({
        rooms: [],
        error: "Failed to load lobby",
        isDev: Deno.env.get("DENO_ENV") !== "production",
      });
    }
  },
};

export default function Home({ data }: PageProps<LobbyData>) {
  return (
    <div class="min-h-screen-safe bg-gradient-to-br from-blue-50 to-indigo-100 safe-area-inset">
      <div class="container mx-auto px-3 sm:px-4 lg:px-8 py-4 sm:py-6 lg:py-8 max-w-7xl">
        <div class="text-center mb-6 sm:mb-8">
          <h1 class="text-2xl xs:text-3xl sm:text-4xl lg:text-5xl font-bold text-gray-800 mb-2 sm:mb-4">
            🎨 Drawing Game
          </h1>
          <p class="text-sm sm:text-base lg:text-lg text-gray-600 max-w-2xl mx-auto px-4">
            Join a room or create your own to start playing with friends!
          </p>
        </div>

        <MainLobby
          initialRooms={data.rooms}
          error={data.error}
          isDev={data.isDev}
        />
      </div>
    </div>
  );
}
