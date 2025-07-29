// KV service for game state management using Cloudflare REST API
import { getCloudflareAPI } from "./cloudflare-api.ts";

export interface KVResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

export class KVService {
  private api = getCloudflareAPI();

  private async executeOperation<T>(
    operation: () => Promise<T>,
    errorMessage: string,
  ): Promise<KVResult<T>> {
    try {
      const data = await operation();
      return { success: true, data };
    } catch (error) {
      console.error(`KV error: ${errorMessage}`, error);
      return {
        success: false,
        error: error instanceof Error ? error.message : errorMessage,
      };
    }
  }

  // Game state operations
  async setGameState(roomId: string, state: any, ttl?: number): Promise<KVResult<void>> {
    return this.executeOperation(async () => {
      const key = `game_state:${roomId}`;
      const value = JSON.stringify(state);
      const options = ttl ? { expirationTtl: ttl } : {};
      await this.api.kvPut(key, value, options);
    }, `Failed to set game state for room: ${roomId}`);
  }

  async getGameState(roomId: string): Promise<KVResult<any | null>> {
    return this.executeOperation(async () => {
      const key = `game_state:${roomId}`;
      const value = await this.api.kvGet(key);
      return value ? JSON.parse(value) : null;
    }, `Failed to get game state for room: ${roomId}`);
  }

  async deleteGameState(roomId: string): Promise<KVResult<void>> {
    return this.executeOperation(async () => {
      const key = `game_state:${roomId}`;
      await this.api.kvDelete(key);
    }, `Failed to delete game state for room: ${roomId}`);
  }

  // Player session operations
  async setPlayerSession(playerId: string, session: any, ttl = 3600): Promise<KVResult<void>> {
    return this.executeOperation(async () => {
      const key = `player_session:${playerId}`;
      const value = JSON.stringify(session);
      await this.api.kvPut(key, value, { expirationTtl: ttl });
    }, `Failed to set player session for: ${playerId}`);
  }

  async getPlayerSession(playerId: string): Promise<KVResult<any | null>> {
    return this.executeOperation(async () => {
      const key = `player_session:${playerId}`;
      const value = await this.api.kvGet(key);
      return value ? JSON.parse(value) : null;
    }, `Failed to get player session for: ${playerId}`);
  }

  async deletePlayerSession(playerId: string): Promise<KVResult<void>> {
    return this.executeOperation(async () => {
      const key = `player_session:${playerId}`;
      await this.api.kvDelete(key);
    }, `Failed to delete player session for: ${playerId}`);
  }

  // Room cache operations
  async cacheRoomData(roomId: string, data: any, ttl = 300): Promise<KVResult<void>> {
    return this.executeOperation(async () => {
      const key = `room_cache:${roomId}`;
      const value = JSON.stringify(data);
      await this.api.kvPut(key, value, { expirationTtl: ttl });
    }, `Failed to cache room data for: ${roomId}`);
  }

  async getCachedRoomData(roomId: string): Promise<KVResult<any | null>> {
    return this.executeOperation(async () => {
      const key = `room_cache:${roomId}`;
      const value = await this.api.kvGet(key);
      return value ? JSON.parse(value) : null;
    }, `Failed to get cached room data for: ${roomId}`);
  }

  // Drawing data operations
  async saveDrawingData(roomId: string, drawingData: any, ttl = 1800): Promise<KVResult<void>> {
    return this.executeOperation(async () => {
      const key = `drawing:${roomId}:${Date.now()}`;
      const value = JSON.stringify(drawingData);
      await this.api.kvPut(key, value, { expirationTtl: ttl });
    }, `Failed to save drawing data for room: ${roomId}`);
  }

  async getDrawingHistory(roomId: string): Promise<KVResult<any[]>> {
    return this.executeOperation(async () => {
      const prefix = `drawing:${roomId}:`;
      const result = await this.api.kvList({ prefix, limit: 100 });

      const drawings = [];
      for (const key of result.keys || []) {
        const value = await this.api.kvGet(key.name);
        if (value) {
          drawings.push(JSON.parse(value));
        }
      }

      return drawings.sort((a, b) => b.timestamp - a.timestamp);
    }, `Failed to get drawing history for room: ${roomId}`);
  }

  // Chat message cache
  async cacheMessage(roomId: string, message: any, ttl = 3600): Promise<KVResult<void>> {
    return this.executeOperation(async () => {
      const key = `message:${roomId}:${Date.now()}`;
      const value = JSON.stringify(message);
      await this.api.kvPut(key, value, { expirationTtl: ttl });
    }, `Failed to cache message for room: ${roomId}`);
  }

  async getRecentMessages(roomId: string, limit = 50): Promise<KVResult<any[]>> {
    return this.executeOperation(async () => {
      const prefix = `message:${roomId}:`;
      const result = await this.api.kvList({ prefix, limit });

      const messages = [];
      for (const key of result.keys || []) {
        const value = await this.api.kvGet(key.name);
        if (value) {
          messages.push(JSON.parse(value));
        }
      }

      return messages.sort((a, b) => b.timestamp - a.timestamp).slice(0, limit);
    }, `Failed to get recent messages for room: ${roomId}`);
  }

  // Generic operations
  async set(key: string, value: any, ttl?: number): Promise<KVResult<void>> {
    return this.executeOperation(async () => {
      const jsonValue = JSON.stringify(value);
      const options = ttl ? { expirationTtl: ttl } : {};
      await this.api.kvPut(key, jsonValue, options);
    }, `Failed to set key: ${key}`);
  }

  async get(key: string): Promise<KVResult<any | null>> {
    return this.executeOperation(async () => {
      const value = await this.api.kvGet(key);
      return value ? JSON.parse(value) : null;
    }, `Failed to get key: ${key}`);
  }

  async delete(key: string): Promise<KVResult<void>> {
    return this.executeOperation(async () => {
      await this.api.kvDelete(key);
    }, `Failed to delete key: ${key}`);
  }

  // Health check
  async healthCheck(): Promise<KVResult<boolean>> {
    return this.executeOperation(async () => {
      const testKey = "health_check";
      const testValue = { timestamp: Date.now() };

      await this.api.kvPut(testKey, JSON.stringify(testValue), { expirationTtl: 60 });
      const retrieved = await this.api.kvGet(testKey);
      await this.api.kvDelete(testKey);

      return retrieved !== null;
    }, "KV health check failed");
  }
}

// Singleton instance
let kvService: KVService | null = null;

export function getKVService(): KVService {
  if (!kvService) {
    kvService = new KVService();
  }
  return kvService;
}
