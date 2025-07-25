// API route for leaving a specific room
import { Handlers } from "$fresh/server.ts";
import { RoomManager } from "../../../../lib/room-manager.ts";
import { WebSocketManager } from "../../../../lib/websocket/websocket-manager.ts";
import { Env } from "../../../../types/cloudflare.ts";

// Global WebSocket manager instance
let wsManager: WebSocketManager | null = null;

function getWebSocketManager(env: Env): WebSocketManager {
  if (!wsManager) {
    wsManager = new WebSocketManager(env);
  }
  return wsManager;
}

export const handler: Handlers = {
  // POST /api/rooms/[id]/leave - Leave a room
  async POST(req, ctx) {
    try {
      const env = (ctx.state as any).env as Env;
      if (!env?.DB) {
        return new Response(JSON.stringify({ error: "Database not available" }), {
          status: 500,
          headers: { "Content-Type": "application/json" }
        });
      }

      const roomId = ctx.params.id;
      const body = await req.json();
      const { playerId } = body;

      if (!playerId) {
        return new Response(JSON.stringify({ error: "Player ID is required" }), {
          status: 400,
          headers: { "Content-Type": "application/json" }
        });
      }

      // Validate that the room exists
      const roomManager = new RoomManager(env.DB);
      const roomSummary = await roomManager.getRoomSummary(roomId);
      
      if (!roomSummary.success || !roomSummary.data) {
        return new Response(JSON.stringify({ error: "Room not found" }), {
          status: 404,
          headers: { "Content-Type": "application/json" }
        });
      }

      // Validate that the player is in the room
      const playerInRoom = roomSummary.data.players.find(p => p.id === playerId);
      if (!playerInRoom) {
        return new Response(JSON.stringify({ error: "Player not found in room" }), {
          status: 404,
          headers: { "Content-Type": "application/json" }
        });
      }

      // Remove player from room
      const result = await roomManager.leaveRoom(playerId);

      if (!result.success) {
        return new Response(JSON.stringify({ error: result.error }), {
          status: 400,
          headers: { "Content-Type": "application/json" }
        });
      }

      // Get WebSocket manager and broadcast updates
      const wsManager = getWebSocketManager(env);
      
      // Broadcast room update to remaining players in the room
      try {
        await wsManager.broadcastToRoomPublic(roomId, {
          type: "room-update",
          roomId,
          data: {
            type: "player-left",
            playerId,
            playerName: playerInRoom.name,
            wasHost: result.data?.wasHost || false,
            newHostId: result.data?.newHostId
          }
        });
      } catch (error) {
        console.error("Error broadcasting player left:", error);
        // Don't fail the request if broadcast fails
      }

      // Broadcast lobby update for room list changes
      try {
        await wsManager.broadcastLobbyUpdate();
      } catch (error) {
        console.error("Error broadcasting lobby update:", error);
        // Don't fail the request if broadcast fails
      }

      return new Response(JSON.stringify({ 
        success: true,
        wasHost: result.data?.wasHost || false,
        newHostId: result.data?.newHostId
      }), {
        headers: { "Content-Type": "application/json" }
      });
    } catch (error) {
      console.error("Error leaving room:", error);
      return new Response(JSON.stringify({ error: "Internal server error" }), {
        status: 500,
        headers: { "Content-Type": "application/json" }
      });
    }
  }
};