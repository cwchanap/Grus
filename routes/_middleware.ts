// Middleware to provide mock Cloudflare environment for development
import { MiddlewareHandlerContext } from "$fresh/server.ts";
import { Env } from "../types/cloudflare.ts";

// Mock D1 Database for development
class MockD1Database {
  private data: Map<string, any[]> = new Map();

  constructor() {
    // Initialize with empty tables
    this.data.set('rooms', []);
    this.data.set('players', []);
    this.data.set('game_sessions', []);
    this.data.set('scores', []);
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
    // Mock implementation - in a real app you'd parse SQL and execute
    console.log(`Mock DB Query: ${this.query}`, this.params);
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
  }

  async first<T = any>(): Promise<T | null> {
    // Mock implementation
    console.log(`Mock DB Query (first): ${this.query}`, this.params);
    
    // Return mock data based on query type
    if (this.query.includes('SELECT') && this.query.includes('rooms')) {
      if (this.query.includes('COUNT')) {
        return { count: 0 } as T;
      }
      return null; // No rooms in development
    }
    
    return null;
  }

  async all<T = any>() {
    // Mock implementation
    console.log(`Mock DB Query (all): ${this.query}`, this.params);
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