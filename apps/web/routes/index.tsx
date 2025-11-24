import { Handlers, PageProps } from "$fresh/server.ts";
import MainLobby from "../islands/core/MainLobby.tsx";
import { RoomManager, RoomSummary } from "../lib/core/room-manager.ts";
import { getSession } from "../lib/auth/auth-utils.ts";
import { getConfig } from "../lib/config.ts";

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
      const config = getConfig();

      // Check for user session
      let user = undefined;
      const cookie = req.headers.get("Cookie");
      const cookieName = config.auth.sessionCookie.name;
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
    <MainLobby
      initialRooms={data.rooms}
      error={data.error}
      isDev={data.isDev}
      user={data.user}
    />
  );
}
