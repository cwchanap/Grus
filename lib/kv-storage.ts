import type { KVNamespace } from "../types/cloudflare.ts";
import type { GameState, ChatMessage } from "../types/game.ts";

export class KVStorageService {
  constructor(private kv: KVNamespace) {}

  // Game state operations
  async getGameState(roomId: string): Promise<GameState | null> {
    const data = await this.kv.get(`game:${roomId}`, { type: "json" });
    return data as GameState | null;
  }

  async setGameState(roomId: string, gameState: GameState, ttlSeconds = 3600): Promise<void> {
    await this.kv.put(`game:${roomId}`, JSON.stringify(gameState), {
      expirationTtl: ttlSeconds
    });
  }

  async deleteGameState(roomId: string): Promise<void> {
    await this.kv.delete(`game:${roomId}`);
  }

  // Chat message operations
  async addChatMessage(roomId: string, message: ChatMessage): Promise<void> {
    const key = `chat:${roomId}:${message.timestamp}`;
    await this.kv.put(key, JSON.stringify(message), {
      expirationTtl: 86400 // 24 hours
    });
  }

  async getChatMessages(roomId: string, limit = 50): Promise<ChatMessage[]> {
    const listResult = await this.kv.list({
      prefix: `chat:${roomId}:`,
      limit
    });

    const messages: ChatMessage[] = [];
    for (const key of listResult.keys) {
      const data = await this.kv.get(key.name, { type: "json" });
      if (data) {
        messages.push(data as ChatMessage);
      }
    }

    return messages.sort((a, b) => a.timestamp - b.timestamp);
  }

  // Player session operations
  async setPlayerSession(playerId: string, sessionData: any, ttlSeconds = 7200): Promise<void> {
    await this.kv.put(`session:${playerId}`, JSON.stringify(sessionData), {
      expirationTtl: ttlSeconds
    });
  }

  async getPlayerSession(playerId: string): Promise<any | null> {
    const data = await this.kv.get(`session:${playerId}`, { type: "json" });
    return data;
  }

  async deletePlayerSession(playerId: string): Promise<void> {
    await this.kv.delete(`session:${playerId}`);
  }

  // Room connection tracking
  async addPlayerToRoom(roomId: string, playerId: string): Promise<void> {
    const key = `room:${roomId}:players`;
    const existingData = await this.kv.get(key, { type: "json" }) as string[] || [];
    
    if (!existingData.includes(playerId)) {
      existingData.push(playerId);
      await this.kv.put(key, JSON.stringify(existingData), {
        expirationTtl: 3600
      });
    }
  }

  async removePlayerFromRoom(roomId: string, playerId: string): Promise<void> {
    const key = `room:${roomId}:players`;
    const existingData = await this.kv.get(key, { type: "json" }) as string[] || [];
    
    const updatedData = existingData.filter(id => id !== playerId);
    if (updatedData.length > 0) {
      await this.kv.put(key, JSON.stringify(updatedData), {
        expirationTtl: 3600
      });
    } else {
      await this.kv.delete(key);
    }
  }

  async getRoomPlayers(roomId: string): Promise<string[]> {
    const data = await this.kv.get(`room:${roomId}:players`, { type: "json" });
    return (data as string[]) || [];
  }

  // Drawing data operations
  async setDrawingData(roomId: string, drawingCommands: any[], ttlSeconds = 3600): Promise<void> {
    await this.kv.put(`drawing:${roomId}`, JSON.stringify(drawingCommands), {
      expirationTtl: ttlSeconds
    });
  }

  async getDrawingData(roomId: string): Promise<any[]> {
    const data = await this.kv.get(`drawing:${roomId}`, { type: "json" });
    return (data as any[]) || [];
  }

  async clearDrawingData(roomId: string): Promise<void> {
    await this.kv.delete(`drawing:${roomId}`);
  }
}