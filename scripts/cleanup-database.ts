#!/usr/bin/env -S deno run --allow-read --allow-write --allow-env --unstable-kv

/**
 * Database cleanup script for removing dangling rooms and orphaned data
 * 
 * Usage:
 *   deno run --allow-read --allow-write --allow-env --unstable-kv scripts/cleanup-database.ts [options]
 * 
 * Options:
 *   --dry-run    Show what would be cleaned up without making changes
 *   --all        Clean up all types of dangling data (rooms, players, sessions)
 *   --rooms      Clean up empty rooms only (default)
 *   --players    Clean up orphaned players only
 *   --sessions   Clean up orphaned game sessions only
 *   --limit=N    Limit number of records to process (default: 100)
 */

import { getKVRoomService, KVRoomService } from "../lib/db/index.ts";

interface CleanupOptions {
  dryRun: boolean;
  cleanRooms: boolean;
  cleanPlayers: boolean;
  cleanSessions: boolean;
  cleanOldRooms: boolean;
  limit: number;
  maxAgeMinutes: number;
}

interface CleanupStats {
  emptyRoomsRemoved: number;
  oldRoomsRemoved: number;
  orphanedPlayersRemoved: number;
  orphanedSessionsRemoved: number;
  inactiveRoomsDeactivated: number;
  kvGameStatesRemoved: number;
  kvPlayerSessionsRemoved: number;
  kvDrawingDataRemoved: number;
  kvCacheDataRemoved: number;
  totalProcessed: number;
}

class DatabaseCleanup {
  private db: KVRoomService;

  constructor() {
    this.db = getKVRoomService();
  }

  async cleanup(options: CleanupOptions): Promise<CleanupStats> {
    const stats: CleanupStats = {
      emptyRoomsRemoved: 0,
      oldRoomsRemoved: 0,
      orphanedPlayersRemoved: 0,
      orphanedSessionsRemoved: 0,
      inactiveRoomsDeactivated: 0,
      kvGameStatesRemoved: 0,
      kvPlayerSessionsRemoved: 0,
      kvDrawingDataRemoved: 0,
      kvCacheDataRemoved: 0,
      totalProcessed: 0,
    };

    console.log("🧹 Starting database cleanup...");
    console.log(`Options: ${JSON.stringify(options, null, 2)}`);

    if (options.cleanRooms) {
      console.log("\n📦 Cleaning up empty rooms...");
      const roomStats = await this.cleanupEmptyRooms(options);
      stats.emptyRoomsRemoved = roomStats.emptyRoomsRemoved;
      stats.inactiveRoomsDeactivated = roomStats.inactiveRoomsDeactivated;
      stats.totalProcessed += roomStats.totalProcessed;
    }

    if (options.cleanOldRooms) {
      console.log(`\n⏰ Cleaning up rooms older than ${options.maxAgeMinutes} minutes...`);
      const oldRoomStats = await this.cleanupOldRooms(options);
      stats.oldRoomsRemoved = oldRoomStats.oldRoomsRemoved;
      stats.totalProcessed += oldRoomStats.totalProcessed;
    }

    if (options.cleanPlayers) {
      console.log("\n👥 Cleaning up orphaned players...");
      const playerStats = await this.cleanupOrphanedPlayers(options);
      stats.orphanedPlayersRemoved = playerStats.orphanedPlayersRemoved;
      stats.totalProcessed += playerStats.totalProcessed;
    }

    if (options.cleanSessions) {
      console.log("\n🎮 Cleaning up orphaned game sessions...");
      const sessionStats = await this.cleanupOrphanedSessions(options);
      stats.orphanedSessionsRemoved = sessionStats.orphanedSessionsRemoved;
      stats.totalProcessed += sessionStats.totalProcessed;
    }

    // Always clean up KV data for removed rooms
    console.log("\n🗄️  Cleaning up KV storage...");
    const kvStats = await this.cleanupKVStorage(options);
    stats.kvGameStatesRemoved = kvStats.kvGameStatesRemoved;
    stats.kvPlayerSessionsRemoved = kvStats.kvPlayerSessionsRemoved;
    stats.kvDrawingDataRemoved = kvStats.kvDrawingDataRemoved;
    stats.kvCacheDataRemoved = kvStats.kvCacheDataRemoved;
    stats.totalProcessed += kvStats.totalProcessed;

    return stats;
  }

  private async cleanupEmptyRooms(options: CleanupOptions): Promise<{
    emptyRoomsRemoved: number;
    inactiveRoomsDeactivated: number;
    totalProcessed: number;
  }> {
    let emptyRoomsRemoved = 0;
    let inactiveRoomsDeactivated = 0;
    let totalProcessed = 0;

    // Get all active rooms
    const roomsResult = await this.db.getActiveRooms(options.limit);
    if (!roomsResult.success) {
      console.error("❌ Failed to get active rooms:", roomsResult.error);
      return { emptyRoomsRemoved, inactiveRoomsDeactivated, totalProcessed };
    }

    const rooms = roomsResult.data || [];
    console.log(`Found ${rooms.length} active rooms to check`);

    for (const room of rooms) {
      totalProcessed++;
      
      // Get players for this room
      const playersResult = await this.db.getPlayersByRoom(room.id);
      if (!playersResult.success) {
        console.error(`❌ Failed to get players for room ${room.id}:`, playersResult.error);
        continue;
      }

      const players = playersResult.data || [];
      
      if (players.length === 0) {
        console.log(`🗑️  Empty room found: ${room.id} (${room.name})`);
        
        if (!options.dryRun) {
          const deleteResult = await this.db.deleteRoom(room.id);
          if (deleteResult.success) {
            emptyRoomsRemoved++;
            console.log(`✅ Deleted empty room: ${room.id}`);
          } else {
            console.error(`❌ Failed to delete room ${room.id}:`, deleteResult.error);
            
            // Fallback: deactivate the room
            const deactivateResult = await this.db.updateRoom(room.id, { isActive: false });
            if (deactivateResult.success) {
              inactiveRoomsDeactivated++;
              console.log(`⚠️  Deactivated room instead: ${room.id}`);
            }
          }
        } else {
          console.log(`🔍 [DRY RUN] Would delete empty room: ${room.id}`);
          emptyRoomsRemoved++;
        }
      } else {
        console.log(`✅ Room ${room.id} has ${players.length} players - keeping`);
      }
    }

    return { emptyRoomsRemoved, inactiveRoomsDeactivated, totalProcessed };
  }

  private async cleanupOldRooms(options: CleanupOptions): Promise<{
    oldRoomsRemoved: number;
    totalProcessed: number;
  }> {
    let oldRoomsRemoved = 0;
    let totalProcessed = 0;

    // Calculate cutoff time
    const cutoffTime = new Date(Date.now() - (options.maxAgeMinutes * 60 * 1000));
    console.log(`Cutoff time: ${cutoffTime.toISOString()}`);

    // Get all active rooms
    const roomsResult = await this.db.getActiveRooms(options.limit);
    if (!roomsResult.success) {
      console.error("❌ Failed to get active rooms:", roomsResult.error);
      return { oldRoomsRemoved, totalProcessed };
    }

    const rooms = roomsResult.data || [];
    console.log(`Found ${rooms.length} active rooms to check for age`);

    for (const room of rooms) {
      totalProcessed++;
      
      // Parse room creation time
      const roomCreatedAt = new Date(room.createdAt);
      
      if (roomCreatedAt < cutoffTime) {
        console.log(`🗑️  Old room found: ${room.id} (${room.name}) - created ${roomCreatedAt.toISOString()}`);
        
        if (!options.dryRun) {
          // First remove all players from the room
          const playersResult = await this.db.getPlayersByRoom(room.id);
          if (playersResult.success && playersResult.data) {
            for (const player of playersResult.data) {
              await this.db.removePlayer(player.id);
            }
          }

          // Then delete the room
          const deleteResult = await this.db.deleteRoom(room.id);
          if (deleteResult.success) {
            oldRoomsRemoved++;
            console.log(`✅ Deleted old room: ${room.id} (${room.name})`);
          } else {
            console.error(`❌ Failed to delete old room ${room.id}:`, deleteResult.error);
          }
        } else {
          console.log(`🔍 [DRY RUN] Would delete old room: ${room.id} (${room.name})`);
          oldRoomsRemoved++;
        }
      } else {
        const ageMinutes = Math.floor((Date.now() - roomCreatedAt.getTime()) / (60 * 1000));
        console.log(`✅ Room ${room.id} is ${ageMinutes} minutes old - keeping`);
      }
    }

    return { oldRoomsRemoved, totalProcessed };
  }

  private async cleanupOrphanedPlayers(options: CleanupOptions): Promise<{
    orphanedPlayersRemoved: number;
    totalProcessed: number;
  }> {
    let orphanedPlayersRemoved = 0;
    let totalProcessed = 0;

    // This would require a custom query to find players with non-existent room_ids
    // For now, we'll use the existing room-based approach
    console.log("ℹ️  Orphaned player cleanup not implemented yet");
    console.log("   (Players are automatically cleaned up when rooms are deleted due to foreign key constraints)");

    return { orphanedPlayersRemoved, totalProcessed };
  }

  private async cleanupOrphanedSessions(options: CleanupOptions): Promise<{
    orphanedSessionsRemoved: number;
    totalProcessed: number;
  }> {
    let orphanedSessionsRemoved = 0;
    let totalProcessed = 0;

    // This would require a custom query to find sessions with non-existent room_ids
    console.log("ℹ️  Orphaned session cleanup not implemented yet");
    console.log("   (Sessions are automatically cleaned up when rooms are deleted due to foreign key constraints)");

    return { orphanedSessionsRemoved, totalProcessed };
  }

  private async cleanupKVStorage(options: CleanupOptions): Promise<{
    kvGameStatesRemoved: number;
    kvPlayerSessionsRemoved: number;
    kvDrawingDataRemoved: number;
    kvCacheDataRemoved: number;
    totalProcessed: number;
  }> {
    let kvGameStatesRemoved = 0;
    let kvPlayerSessionsRemoved = 0;
    let kvDrawingDataRemoved = 0;
    let kvCacheDataRemoved = 0;
    let totalProcessed = 0;

    try {
      // Get all active room IDs from KV room store
      const roomsResult = await this.db.getActiveRooms(1000);
      const activeRoomIds = new Set<string>();
      
      if (roomsResult.success && roomsResult.data) {
        roomsResult.data.forEach(room => activeRoomIds.add(room.id));
      }

      console.log(`Found ${activeRoomIds.size} active rooms in KV store`);

      // Access the KV store directly to list all keys
      const kv = await Deno.openKv();

      // Clean up game states
      console.log("🎮 Checking game states...");
      const gameStateEntries = kv.list({ prefix: ["game_state"] });
      for await (const entry of gameStateEntries) {
        totalProcessed++;
        const roomId = entry.key[1] as string;
        
        if (!activeRoomIds.has(roomId)) {
          console.log(`🗑️  Orphaned game state found: ${roomId}`);
          
          if (!options.dryRun) {
            await kv.delete(entry.key);
            kvGameStatesRemoved++;
            console.log(`✅ Deleted game state for room: ${roomId}`);
          } else {
            console.log(`🔍 [DRY RUN] Would delete game state for room: ${roomId}`);
            kvGameStatesRemoved++;
          }
        }
      }

      // Clean up room cache
      console.log("💾 Checking room cache...");
      const roomCacheEntries = kv.list({ prefix: ["room_cache"] });
      for await (const entry of roomCacheEntries) {
        totalProcessed++;
        const roomId = entry.key[1] as string;
        
        if (!activeRoomIds.has(roomId)) {
          console.log(`🗑️  Orphaned room cache found: ${roomId}`);
          
          if (!options.dryRun) {
            await kv.delete(entry.key);
            kvCacheDataRemoved++;
            console.log(`✅ Deleted room cache for room: ${roomId}`);
          } else {
            console.log(`🔍 [DRY RUN] Would delete room cache for room: ${roomId}`);
            kvCacheDataRemoved++;
          }
        }
      }

      // Clean up drawing data
      console.log("🎨 Checking drawing data...");
      const drawingEntries = kv.list({ prefix: ["drawing"] });
      for await (const entry of drawingEntries) {
        totalProcessed++;
        const roomId = entry.key[1] as string;
        
        if (!activeRoomIds.has(roomId)) {
          console.log(`🗑️  Orphaned drawing data found: ${roomId}`);
          
          if (!options.dryRun) {
            await kv.delete(entry.key);
            kvDrawingDataRemoved++;
            console.log(`✅ Deleted drawing data for room: ${roomId}`);
          } else {
            console.log(`🔍 [DRY RUN] Would delete drawing data for room: ${roomId}`);
            kvDrawingDataRemoved++;
          }
        }
      }

      // Clean up message cache
      console.log("💬 Checking message cache...");
      const messageEntries = kv.list({ prefix: ["message"] });
      for await (const entry of messageEntries) {
        totalProcessed++;
        const roomId = entry.key[1] as string;
        
        if (!activeRoomIds.has(roomId)) {
          console.log(`🗑️  Orphaned message cache found: ${roomId}`);
          
          if (!options.dryRun) {
            await kv.delete(entry.key);
            kvCacheDataRemoved++;
            console.log(`✅ Deleted message cache for room: ${roomId}`);
          } else {
            console.log(`🔍 [DRY RUN] Would delete message cache for room: ${roomId}`);
            kvCacheDataRemoved++;
          }
        }
      }

      // Clean up player sessions (these might be orphaned if players disconnected)
      console.log("👤 Checking player sessions...");
      const playerSessionEntries = kv.list({ prefix: ["player_session"] });
      for await (const entry of playerSessionEntries) {
        totalProcessed++;
        const playerId = entry.key[1] as string;
        
        // Check if player exists in any active room
        const playerResult = await this.db.getPlayerById(playerId);
        if (!playerResult.success || !playerResult.data) {
          console.log(`🗑️  Orphaned player session found: ${playerId}`);
          
          if (!options.dryRun) {
            await kv.delete(entry.key);
            kvPlayerSessionsRemoved++;
            console.log(`✅ Deleted player session: ${playerId}`);
          } else {
            console.log(`🔍 [DRY RUN] Would delete player session: ${playerId}`);
            kvPlayerSessionsRemoved++;
          }
        }
      }

      kv.close();

    } catch (error) {
      console.error("❌ Error cleaning up KV storage:", error);
    }

    return {
      kvGameStatesRemoved,
      kvPlayerSessionsRemoved,
      kvDrawingDataRemoved,
      kvCacheDataRemoved,
      totalProcessed,
    };
  }
}

function parseArgs(args: string[]): CleanupOptions {
  const options: CleanupOptions = {
    dryRun: false,
    cleanRooms: true, // Default to cleaning rooms
    cleanPlayers: false,
    cleanSessions: false,
    cleanOldRooms: false,
    limit: 100,
    maxAgeMinutes: 30, // Default to 30 minutes as per product rules
  };

  for (const arg of args) {
    switch (arg) {
      case "--dry-run":
        options.dryRun = true;
        break;
      case "--all":
        options.cleanRooms = true;
        options.cleanPlayers = true;
        options.cleanSessions = true;
        options.cleanOldRooms = true;
        break;
      case "--rooms":
        options.cleanRooms = true;
        options.cleanPlayers = false;
        options.cleanSessions = false;
        break;
      case "--players":
        options.cleanRooms = false;
        options.cleanPlayers = true;
        options.cleanSessions = false;
        break;
      case "--sessions":
        options.cleanRooms = false;
        options.cleanPlayers = false;
        options.cleanSessions = true;
        break;
      case "--old-rooms":
        options.cleanRooms = false;
        options.cleanPlayers = false;
        options.cleanSessions = false;
        options.cleanOldRooms = true;
        break;
      default:
        if (arg.startsWith("--limit=")) {
          const limit = parseInt(arg.split("=")[1]);
          if (!isNaN(limit) && limit > 0) {
            options.limit = limit;
          }
        } else if (arg.startsWith("--max-age=")) {
          const maxAge = parseInt(arg.split("=")[1]);
          if (!isNaN(maxAge) && maxAge > 0) {
            options.maxAgeMinutes = maxAge;
          }
        }
        break;
    }
  }

  return options;
}

function printUsage() {
  console.log(`
🧹 Database Cleanup Script

Usage:
  deno run --allow-read --allow-write --allow-env --unstable-kv scripts/cleanup-database.ts [options]

Options:
  --dry-run      Show what would be cleaned up without making changes
  --all          Clean up all types of dangling data (rooms, players, sessions, old rooms)
  --rooms        Clean up empty rooms only (default)
  --players      Clean up orphaned players only
  --sessions     Clean up orphaned game sessions only
  --old-rooms    Clean up rooms older than specified age
  --limit=N      Limit number of records to process (default: 100)
  --max-age=N    Maximum age in minutes for rooms (default: 30, used with --old-rooms or --all)

Examples:
  # Dry run to see what would be cleaned up
  deno run --allow-read --allow-write --allow-env --unstable-kv scripts/cleanup-database.ts --dry-run

  # Clean up empty rooms (default behavior)
  deno run --allow-read --allow-write --allow-env --unstable-kv scripts/cleanup-database.ts

  # Clean up rooms older than 30 minutes (following product rules)
  deno run --allow-read --allow-write --allow-env --unstable-kv scripts/cleanup-database.ts --old-rooms

  # Clean up rooms older than 60 minutes
  deno run --allow-read --allow-write --allow-env --unstable-kv scripts/cleanup-database.ts --old-rooms --max-age=60

  # Clean up everything with a limit of 50 records
  deno run --allow-read --allow-write --allow-env --unstable-kv scripts/cleanup-database.ts --all --limit=50
`);
}

async function main() {
  const args = Deno.args;

  if (args.includes("--help") || args.includes("-h")) {
    printUsage();
    return;
  }

  const options = parseArgs(args);
  const cleanup = new DatabaseCleanup();

  try {
    const stats = await cleanup.cleanup(options);

    console.log("\n📊 Cleanup Summary:");
    console.log("==================");
    console.log(`🏠 KV Room Store:`);
    console.log(`  Empty rooms removed: ${stats.emptyRoomsRemoved}`);
    console.log(`  Old rooms removed: ${stats.oldRoomsRemoved}`);
    console.log(`  Inactive rooms deactivated: ${stats.inactiveRoomsDeactivated}`);
    console.log(`  Orphaned players removed: ${stats.orphanedPlayersRemoved}`);
    console.log(`  Orphaned sessions removed: ${stats.orphanedSessionsRemoved}`);
    console.log(`\n🗄️  KV Storage:`);
    console.log(`  Game states removed: ${stats.kvGameStatesRemoved}`);
    console.log(`  Player sessions removed: ${stats.kvPlayerSessionsRemoved}`);
    console.log(`  Drawing data removed: ${stats.kvDrawingDataRemoved}`);
    console.log(`  Cache data removed: ${stats.kvCacheDataRemoved}`);
    console.log(`\n📊 Total records processed: ${stats.totalProcessed}`);

    if (options.dryRun) {
      console.log("\n🔍 This was a dry run - no changes were made");
    } else {
      console.log("\n✅ Cleanup completed successfully");
    }

  } catch (error) {
    console.error("❌ Cleanup failed:", error);
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}