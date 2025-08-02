/// <reference lib="deno.unstable" />

// KV service for game state management using Deno KV
export interface KVResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

export class KVService {
  private kv: Deno.Kv | null = null;

  private async getKv(): Promise<Deno.Kv> {
    if (!this.kv) {
      this.kv = await Deno.openKv();
    }
    return this.kv;
  }

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
    return await this.executeOperation(async () => {
      const kv = await this.getKv();
      const key = ["game_state", roomId];
      const options = ttl ? { expireIn: ttl * 1000 } : {};
      await kv.set(key, state, options);
    }, `Failed to set game state for room: ${roomId}`);
  }

  async getGameState(roomId: string): Promise<KVResult<any | null>> {
    return await this.executeOperation(async () => {
      const kv = await this.getKv();
      const key = ["game_state", roomId];
      const result = await kv.get(key);
      return result.value;
    }, `Failed to get game state for room: ${roomId}`);
  }

  async deleteGameState(roomId: string): Promise<KVResult<void>> {
    return await this.executeOperation(async () => {
      const kv = await this.getKv();
      const key = ["game_state", roomId];
      await kv.delete(key);
    }, `Failed to delete game state for room: ${roomId}`);
  }

  // Player session operations
  async setPlayerSession(playerId: string, session: any, ttl = 3600): Promise<KVResult<void>> {
    return await this.executeOperation(async () => {
      const kv = await this.getKv();
      const key = ["player_session", playerId];
      await kv.set(key, session, { expireIn: ttl * 1000 });
    }, `Failed to set player session for: ${playerId}`);
  }

  async getPlayerSession(playerId: string): Promise<KVResult<any | null>> {
    return await this.executeOperation(async () => {
      const kv = await this.getKv();
      const key = ["player_session", playerId];
      const result = await kv.get(key);
      return result.value;
    }, `Failed to get player session for: ${playerId}`);
  }

  async deletePlayerSession(playerId: string): Promise<KVResult<void>> {
    return await this.executeOperation(async () => {
      const kv = await this.getKv();
      const key = ["player_session", playerId];
      await kv.delete(key);
    }, `Failed to delete player session for: ${playerId}`);
  }

  // Room cache operations
  async cacheRoomData(roomId: string, data: any, ttl = 300): Promise<KVResult<void>> {
    return await this.executeOperation(async () => {
      const kv = await this.getKv();
      const key = ["room_cache", roomId];
      await kv.set(key, data, { expireIn: ttl * 1000 });
    }, `Failed to cache room data for: ${roomId}`);
  }

  async getCachedRoomData(roomId: string): Promise<KVResult<any | null>> {
    return await this.executeOperation(async () => {
      const kv = await this.getKv();
      const key = ["room_cache", roomId];
      const result = await kv.get(key);
      return result.value;
    }, `Failed to get cached room data for: ${roomId}`);
  }

  // Drawing data operations
  async saveDrawingData(roomId: string, drawingData: any, ttl = 1800): Promise<KVResult<void>> {
    return await this.executeOperation(async () => {
      const kv = await this.getKv();
      const timestamp = Date.now();
      const key = ["drawing", roomId, timestamp];
      await kv.set(key, { ...drawingData, timestamp }, { expireIn: ttl * 1000 });
    }, `Failed to save drawing data for room: ${roomId}`);
  }

  async getDrawingHistory(roomId: string): Promise<KVResult<any[]>> {
    return await this.executeOperation(async () => {
      const kv = await this.getKv();
      const prefix = ["drawing", roomId];
      const entries = kv.list({ prefix });

      const drawings: any[] = [];
      for await (const entry of entries) {
        if (entry.value) {
          drawings.push(entry.value);
        }
      }

      return drawings.sort((a: any, b: any) => (b.timestamp || 0) - (a.timestamp || 0));
    }, `Failed to get drawing history for room: ${roomId}`);
  }

  // Chat message cache
  async cacheMessage(roomId: string, message: any, ttl = 3600): Promise<KVResult<void>> {
    return await this.executeOperation(async () => {
      const kv = await this.getKv();
      const timestamp = Date.now();
      const key = ["message", roomId, timestamp];
      await kv.set(key, { ...message, timestamp }, { expireIn: ttl * 1000 });
    }, `Failed to cache message for room: ${roomId}`);
  }

  async getRecentMessages(roomId: string, limit = 50): Promise<KVResult<any[]>> {
    return await this.executeOperation(async () => {
      const kv = await this.getKv();
      const prefix = ["message", roomId];
      const entries = kv.list({ prefix });

      const messages: any[] = [];
      for await (const entry of entries) {
        if (entry.value) {
          messages.push(entry.value);
        }
      }

      return messages
        .sort((a: any, b: any) => (b.timestamp || 0) - (a.timestamp || 0))
        .slice(0, limit);
    }, `Failed to get recent messages for room: ${roomId}`);
  }

  // Generic operations
  async set(key: string, value: any, ttl?: number): Promise<KVResult<void>> {
    return await this.executeOperation(async () => {
      const kv = await this.getKv();
      const kvKey = ["generic", key];
      const options = ttl ? { expireIn: ttl * 1000 } : {};
      await kv.set(kvKey, value, options);
    }, `Failed to set key: ${key}`);
  }

  async get(key: string): Promise<KVResult<any | null>> {
    return await this.executeOperation(async () => {
      const kv = await this.getKv();
      const kvKey = ["generic", key];
      const result = await kv.get(kvKey);
      return result.value;
    }, `Failed to get key: ${key}`);
  }

  async delete(key: string): Promise<KVResult<void>> {
    return await this.executeOperation(async () => {
      const kv = await this.getKv();
      const kvKey = ["generic", key];
      await kv.delete(kvKey);
    }, `Failed to delete key: ${key}`);
  }

  // Health check
  async healthCheck(): Promise<KVResult<boolean>> {
    return await this.executeOperation(async () => {
      const kv = await this.getKv();
      const testKey = ["health_check"];
      const testValue = { timestamp: Date.now() };

      await kv.set(testKey, testValue, { expireIn: 60000 });
      const result = await kv.get(testKey);
      await kv.delete(testKey);

      return result.value !== null;
    }, "KV health check failed");
  }

  // Cleanup method for graceful shutdown
  close(): void {
    if (this.kv) {
      this.kv.close();
      this.kv = null;
    }
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
