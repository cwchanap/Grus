import { Handlers, PageProps } from "$fresh/server.ts";
import GameLobby from "../islands/GameLobby.tsx";
import { RoomManager } from "../lib/room-manager.ts";
import { Env } from "../types/cloudflare.ts";
import type { RoomSummary } from "../lib/room-manager.ts";

interface LobbyData {
  rooms: RoomSummary[];
  error?: string;
}

export const handler: Handlers<LobbyData> = {
  async GET(req, ctx) {
    try {
      const env = (ctx.state as any).env as Env;
      
      if (!env?.DB) {
        return ctx.render({ 
          rooms: [], 
          error: "Database not available" 
        });
      }

      const roomManager = new RoomManager(env.DB);
      const result = await roomManager.listRooms({ limit: 20 });

      if (!result.success) {
        return ctx.render({ 
          rooms: [], 
          error: result.error || "Failed to load rooms" 
        });
      }

      return ctx.render({ 
        rooms: result.data || [] 
      });
    } catch (error) {
      console.error("Error loading lobby:", error);
      return ctx.render({ 
        rooms: [], 
        error: "Failed to load lobby" 
      });
    }
  }
};

export default function Home({ data }: PageProps<LobbyData>) {
  return (
    <div class="min-h-screen bg-gradient-to-br from-blue-50 to-indigo-100">
      <div class="container mx-auto px-4 py-8">
        <div class="text-center mb-8">
          <h1 class="text-4xl font-bold text-gray-800 mb-2">
            🎨 Drawing Game Lobby
          </h1>
          <p class="text-lg text-gray-600">
            Join a room or create your own to start playing!
          </p>
        </div>
        
        <GameLobby 
          initialRooms={data.rooms} 
          error={data.error}
        />
      </div>
    </div>
  );
}
