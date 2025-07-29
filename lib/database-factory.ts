// Database service factory that chooses between production and development services
import { DatabaseService } from "./database-service.ts";
import { DevDatabaseService } from "./dev-database-service.ts";

// Common interface for both database services
export interface IDatabaseService {
  createRoom(name: string, hostId: string, maxPlayers?: number): Promise<any>;
  getRoomById(id: string): Promise<any>;
  getActiveRooms(limit?: number): Promise<any>;
  updateRoom(id: string, updates: any): Promise<any>;
  deleteRoom(id: string): Promise<any>;
  createPlayer(name: string, roomId: string, isHost?: boolean): Promise<any>;
  getPlayerById(id: string): Promise<any>;
  getPlayersByRoom(roomId: string): Promise<any>;
  removePlayer(id: string): Promise<any>;
  createGameSession(roomId: string, totalRounds?: number): Promise<any>;
  endGameSession(id: string, winnerId?: string): Promise<any>;
  createScore(sessionId: string, playerId: string, points?: number): Promise<any>;
  updateScore(id: string, points: number, correctGuesses: number): Promise<any>;
  getScoresBySession(sessionId: string): Promise<any>;
  healthCheck(): Promise<any>;
}

// Singleton instance
let databaseService: IDatabaseService | null = null;

export function getDatabaseService(): IDatabaseService {
  if (!databaseService) {
    const isDevelopment = Deno.env.get("DENO_ENV") !== "production";
    
    if (isDevelopment) {
      console.log("Using development database service (in-memory)");
      databaseService = new DevDatabaseService();
    } else {
      console.log("Using production database service (Cloudflare D1)");
      databaseService = new DatabaseService();
    }
  }
  return databaseService;
}

// For development: clear all data
export function clearDevelopmentData(): void {
  const isDevelopment = Deno.env.get("DENO_ENV") !== "production";
  if (isDevelopment && databaseService instanceof DevDatabaseService) {
    databaseService.clearAll();
    console.log("Development database cleared");
  }
}

// For development: get stats
export function getDevelopmentStats(): any {
  const isDevelopment = Deno.env.get("DENO_ENV") !== "production";
  if (isDevelopment && databaseService instanceof DevDatabaseService) {
    return databaseService.getStats();
  }
  return null;
}