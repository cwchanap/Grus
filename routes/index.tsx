import { Handlers, PageProps } from "$fresh/server.ts";
import MainLobby from "../islands/core/MainLobby.tsx";
import { RoomManager, RoomSummary } from "../lib/core/room-manager.ts";
import { getSession } from "../lib/auth/auth-utils.ts";

interface LobbyData {
  rooms: RoomSummary[];
  error?: string;
  isDev: boolean;
  user?: {
    id: string;
    email: string;
    username: string;
    name: string | null;
  };
}

export const handler: Handlers<LobbyData> = {
  async GET(req, ctx) {
    try {
      const roomManager = new RoomManager();
      const isDev = Deno.env.get("DENO_ENV") === "development";

      // Check for user session
      let user = undefined;
      const cookie = req.headers.get("Cookie");
      const cookieName = Deno.env.get("SESSION_COOKIE_NAME") || "grus_session";
      const token = cookie?.split(";")
        .find((c) => c.trim().startsWith(`${cookieName}=`))
        ?.split("=")[1];

      if (token) {
        try {
          const session = await getSession(token);
          if (session) {
            user = session.user;
          }
        } catch (error) {
          console.error("Error getting session:", error);
        }
      }

      // Get active rooms with automatic cleanup
      const result = await roomManager.getActiveRoomsWithCleanup(20);

      if (!result.success) {
        return ctx.render({
          rooms: [],
          error: result.error || "Failed to load rooms",
          isDev,
          user,
        });
      }

      return ctx.render({
        rooms: result.data || [],
        isDev,
        user,
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
          user={data.user}
        />
      </div>
    </div>
  );
}
