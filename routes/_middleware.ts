// Middleware to provide mock Cloudflare environment for development
import { MiddlewareHandlerContext } from "$fresh/server.ts";
import { Env } from "../types/cloudflare.ts";

// Shared data store for mock database (singleton pattern)
const mockDatabaseData = new Map<string, any[]>();
mockDatabaseData.set('rooms', []);
mockDatabaseData.set('players', []);
mockDatabaseData.set('game_sessions', []);
mockDatabaseData.set('scores', []);

// Mock D1 Database for development
class MockD1Database {
  private data: Map<string, any[]>;

  constructor() {
    // Use shared data store to persist between requests
    this.data = mockDatabaseData;
  }

  prepare(query: string) {
    return new MockD1PreparedStatement(query, this.data);
  }

  async batch(statements: any[]) {
    const results = [];
    for (const stmt of statements) {
      results.push(await stmt.run());
    }
    return results;
  }

  async exec(query: string) {
    // Simple exec implementation
    return { success: true, meta: { duration: 0 } };
  }
}

class MockD1PreparedStatement {
  private query: string;
  private data: Map<string, any[]>;
  private params: any[] = [];

  constructor(query: string, data: Map<string, any[]>) {
    this.query = query;
    this.data = data;
  }

  bind(...params: any[]) {
    this.params = params;
    return this;
  }

  async run() {
    console.log(`Mock DB Query (run): ${this.query}`, this.params);
    
    try {
      // Handle INSERT operations
      if (this.query.includes('INSERT INTO')) {
        if (this.query.includes('rooms')) {
          const [id, name, hostId, maxPlayers, isActive] = this.params;
          const room = {
            id,
            name,
            host_id: hostId,
            max_players: maxPlayers,
            is_active: isActive,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString()
          };
          this.data.get('rooms')?.push(room);
          console.log('Mock DB: Created room:', room);
          console.log('Mock DB: Total rooms:', this.data.get('rooms')?.length);
        } else if (this.query.includes('players')) {
          const [id, name, roomId, isHost] = this.params;
          const player = {
            id,
            name,
            room_id: roomId,
            is_host: isHost,
            joined_at: new Date().toISOString()
          };
          this.data.get('players')?.push(player);
        }
      }
      
      // Handle UPDATE operations
      if (this.query.includes('UPDATE')) {
        if (this.query.includes('players') && this.query.includes('room_id')) {
          const playerId = this.params[this.params.length - 1]; // Last param is usually the ID
          const roomId = this.params[0]; // First param is usually the new room_id
          const players = this.data.get('players') || [];
          const player = players.find(p => p.id === playerId);
          if (player) {
            player.room_id = roomId;
          }
        }
      }

      return {
        success: true,
        meta: {
          duration: 1,
          last_row_id: Math.floor(Math.random() * 1000),
          changes: 1,
          served_by: "mock",
          internal_stats: null
        }
      };
    } catch (error) {
      console.error('Mock DB Error:', error);
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error',
        meta: {
          duration: 1,
          served_by: "mock",
          internal_stats: null
        }
      };
    }
  }

  async first<T = any>(): Promise<T | null> {
    console.log(`Mock DB Query (first): ${this.query}`, this.params);
    
    try {
      // Handle SELECT operations
      if (this.query.includes('SELECT')) {
        if (this.query.includes('rooms')) {
          const rooms = this.data.get('rooms') || [];
          console.log('Mock DB: Looking for rooms, total available:', rooms.length);
          console.log('Mock DB: Available room IDs:', rooms.map(r => r.id));
          
          if (this.query.includes('COUNT(p.id) as player_count')) {
            // Room with player count query
            const roomId = this.params[0];
            console.log('Mock DB: Looking for room with ID:', roomId);
            const room = rooms.find(r => r.id === roomId);
            if (room) {
              const players = this.data.get('players') || [];
              const playerCount = players.filter(p => p.room_id === roomId).length;
              console.log('Mock DB: Found room:', room, 'with player count:', playerCount);
              return {
                id: room.id,
                name: room.name,
                hostId: room.host_id,
                host_id: room.host_id,
                maxPlayers: room.max_players,
                max_players: room.max_players,
                isActive: room.is_active,
                is_active: room.is_active,
                createdAt: room.created_at,
                created_at: room.created_at,
                updatedAt: room.updated_at,
                updated_at: room.updated_at,
                player_count: playerCount,
                playerCount: playerCount
              } as T;
            } else {
              console.log('Mock DB: Room not found with ID:', roomId);
            }
          } else if (this.query.includes('WHERE id = ?')) {
            // Single room by ID
            const roomId = this.params[0];
            const room = rooms.find(r => r.id === roomId);
            if (room) {
              return {
                id: room.id,
                name: room.name,
                hostId: room.host_id,
                host_id: room.host_id,
                maxPlayers: room.max_players,
                max_players: room.max_players,
                isActive: room.is_active,
                is_active: room.is_active,
                createdAt: room.created_at,
                created_at: room.created_at,
                updatedAt: room.updated_at,
                updated_at: room.updated_at
              } as T;
            }
            return null;
          } else if (this.query.includes('COUNT(*)')) {
            // Count query
            return { count: rooms.length } as T;
          }
        } else if (this.query.includes('players')) {
          const players = this.data.get('players') || [];
          
          if (this.query.includes('WHERE id = ?')) {
            // Single player by ID
            const playerId = this.params[0];
            const player = players.find(p => p.id === playerId);
            if (player) {
              return {
                id: player.id,
                name: player.name,
                roomId: player.room_id,
                room_id: player.room_id,
                isHost: player.is_host,
                is_host: player.is_host,
                joinedAt: player.joined_at,
                joined_at: player.joined_at
              } as T;
            }
            return null;
          } else if (this.query.includes('COUNT(*)')) {
            // Count query
            const roomId = this.params[0];
            const count = players.filter(p => p.room_id === roomId).length;
            return { count } as T;
          }
        }
      }
      
      return null;
    } catch (error) {
      console.error('Mock DB First Error:', error);
      return null;
    }
  }

  async all<T = any>() {
    console.log(`Mock DB Query (all): ${this.query}`, this.params);
    
    try {
      let results: T[] = [];
      
      if (this.query.includes('SELECT')) {
        if (this.query.includes('rooms')) {
          const rooms = this.data.get('rooms') || [];
          
          if (this.query.includes('WHERE is_active = true')) {
            // Active rooms query
            const activeRooms = rooms.filter(r => r.is_active);
            results = activeRooms.map(room => ({
              id: room.id,
              name: room.name,
              hostId: room.host_id,
              host_id: room.host_id,
              maxPlayers: room.max_players,
              max_players: room.max_players,
              isActive: room.is_active,
              is_active: room.is_active,
              createdAt: room.created_at,
              created_at: room.created_at,
              updatedAt: room.updated_at,
              updated_at: room.updated_at
            })) as T[];
          } else {
            results = rooms as T[];
          }
        } else if (this.query.includes('players')) {
          const players = this.data.get('players') || [];
          
          if (this.query.includes('WHERE room_id = ?')) {
            // Players by room
            const roomId = this.params[0];
            const filteredPlayers = players.filter(p => p.room_id === roomId);
            results = filteredPlayers.map(player => ({
              id: player.id,
              name: player.name,
              roomId: player.room_id,
              room_id: player.room_id,
              isHost: player.is_host,
              is_host: player.is_host,
              joinedAt: player.joined_at,
              joined_at: player.joined_at
            })) as T[];
          } else {
            results = players as T[];
          }
        }
      }

      return {
        results,
        success: true,
        meta: {
          duration: 1,
          served_by: "mock"
        }
      };
    } catch (error) {
      console.error('Mock DB All Error:', error);
      return {
        results: [] as T[],
        success: true,
        meta: {
          duration: 1,
          served_by: "mock"
        }
      };
    }
  }
}

// Mock KV Storage for development
class MockKVNamespace {
  private data: Map<string, string> = new Map();

  async get(key: string): Promise<string | null> {
    console.log(`Mock KV Get: ${key}`);
    return this.data.get(key) || null;
  }

  async put(key: string, value: string, options?: any): Promise<void> {
    console.log(`Mock KV Put: ${key}`, options);
    this.data.set(key, value);
  }

  async delete(key: string): Promise<void> {
    console.log(`Mock KV Delete: ${key}`);
    this.data.delete(key);
  }

  async list(options?: any) {
    console.log(`Mock KV List:`, options);
    return {
      keys: Array.from(this.data.keys()).map(name => ({ name })),
      list_complete: true,
      cursor: null
    };
  }
}

export async function handler(
  req: Request,
  ctx: MiddlewareHandlerContext,
) {
  // Only add mock environment in development
  const isDevelopment = Deno.env.get('ENVIRONMENT') !== 'production';
  
  if (isDevelopment) {
    // Create mock Cloudflare environment
    const mockEnv: Env = {
      DB: new MockD1Database() as any,
      GAME_STATE: new MockKVNamespace() as any,
      // Add other Cloudflare bindings as needed
    };

    // Add mock environment to context state
    ctx.state.env = mockEnv;
  }

  return await ctx.next();
}