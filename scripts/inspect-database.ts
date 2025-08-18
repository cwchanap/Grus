#!/usr/bin/env -S deno run --allow-read --allow-write --allow-env --unstable-kv

/**
 * Database inspection script for viewing database contents and statistics
 * 
 * Usage:
 *   deno run --allow-read --allow-write --allow-env --unstable-kv scripts/inspect-database.ts [options]
 * 
 * Options:
 *   --stats      Show database statistics (default)
 *   --rooms      Show all rooms with player counts
 *   --players    Show all players with room info
 *   --sessions   Show all game sessions
 *   --detailed   Show detailed information for all tables
 */

import { getKVRoomService, KVRoomService } from "../lib/db/index.ts";

interface DatabaseStats {
  totalRooms: number;
  activeRooms: number;
  inactiveRooms: number;
  emptyRooms: number;
  totalPlayers: number;
  totalSessions: number;
  activeSessions: number;
  completedSessions: number;
}

class DatabaseInspector {
  private db: KVRoomService;

  constructor() {
    this.db = getKVRoomService();
  }

  async getStats(): Promise<DatabaseStats> {
    const stats: DatabaseStats = {
      totalRooms: 0,
      activeRooms: 0,
      inactiveRooms: 0,
      emptyRooms: 0,
      totalPlayers: 0,
      totalSessions: 0,
      activeSessions: 0,
      completedSessions: 0,
    };

    // Get room statistics
    const roomsResult = await this.db.getActiveRooms(1000); // Get a large number
    if (roomsResult.success && roomsResult.data) {
      const rooms = roomsResult.data;
      stats.totalRooms = rooms.length;
      stats.activeRooms = rooms.filter(r => r.isActive).length;
      stats.inactiveRooms = rooms.filter(r => !r.isActive).length;

      // Count empty rooms
      for (const room of rooms) {
        const playersResult = await this.db.getPlayersByRoom(room.id);
        if (playersResult.success && playersResult.data) {
          const playerCount = playersResult.data.length;
          if (playerCount === 0) {
            stats.emptyRooms++;
          }
          stats.totalPlayers += playerCount;
        }
      }
    }

    return stats;
  }

  async showStats(): Promise<void> {
    console.log("📊 Database Statistics");
    console.log("=====================");

    const stats = await this.getStats();

    console.log(`\n🏠 Rooms:`);
    console.log(`  Total rooms: ${stats.totalRooms}`);
    console.log(`  Active rooms: ${stats.activeRooms}`);
    console.log(`  Inactive rooms: ${stats.inactiveRooms}`);
    console.log(`  Empty rooms: ${stats.emptyRooms}`);

    console.log(`\n👥 Players:`);
    console.log(`  Total players: ${stats.totalPlayers}`);
    console.log(`  Average players per room: ${stats.totalRooms > 0 ? (stats.totalPlayers / stats.totalRooms).toFixed(1) : 0}`);

    console.log(`\n🎮 Game Sessions:`);
    console.log(`  Total sessions: ${stats.totalSessions}`);
    console.log(`  Active sessions: ${stats.activeSessions}`);
    console.log(`  Completed sessions: ${stats.completedSessions}`);

    // Health indicators
    console.log(`\n🔍 Health Indicators:`);
    if (stats.emptyRooms > 0) {
      console.log(`  ⚠️  ${stats.emptyRooms} empty rooms found (consider cleanup)`);
    } else {
      console.log(`  ✅ No empty rooms found`);
    }

    if (stats.inactiveRooms > 0) {
      console.log(`  ⚠️  ${stats.inactiveRooms} inactive rooms found`);
    } else {
      console.log(`  ✅ All rooms are active`);
    }
  }

  async showRooms(): Promise<void> {
    console.log("🏠 Rooms Overview");
    console.log("================");

    const roomsResult = await this.db.getActiveRooms(100);
    if (!roomsResult.success || !roomsResult.data) {
      console.error("❌ Failed to get rooms:", roomsResult.error);
      return;
    }

    const rooms = roomsResult.data;
    console.log(`Found ${rooms.length} rooms:\n`);

    for (const room of rooms) {
      const playersResult = await this.db.getPlayersByRoom(room.id);
      const playerCount = playersResult.success ? (playersResult.data?.length || 0) : 0;
      const players = playersResult.success ? (playersResult.data || []) : [];
      
      const status = room.isActive ? "🟢 Active" : "🔴 Inactive";
      const occupancy = playerCount === 0 ? "🚫 Empty" : 
                       playerCount >= room.maxPlayers ? "🔴 Full" : 
                       `👥 ${playerCount}/${room.maxPlayers}`;

      console.log(`${status} | ${occupancy} | ${room.id.substring(0, 8)}... | ${room.name}`);
      
      if (players.length > 0) {
        const hostPlayer = players.find(p => p.isHost);
        const otherPlayers = players.filter(p => !p.isHost);
        
        if (hostPlayer) {
          console.log(`    👑 Host: ${hostPlayer.name}`);
        }
        if (otherPlayers.length > 0) {
          console.log(`    👤 Players: ${otherPlayers.map(p => p.name).join(", ")}`);
        }
      }
      console.log(`    📅 Created: ${room.createdAt}`);
      console.log("");
    }
  }

  async showPlayers(): Promise<void> {
    console.log("👥 Players Overview");
    console.log("==================");

    const roomsResult = await this.db.getActiveRooms(100);
    if (!roomsResult.success || !roomsResult.data) {
      console.error("❌ Failed to get rooms:", roomsResult.error);
      return;
    }

    const rooms = roomsResult.data;
    let totalPlayers = 0;

    for (const room of rooms) {
      const playersResult = await this.db.getPlayersByRoom(room.id);
      if (playersResult.success && playersResult.data) {
        const players = playersResult.data;
        totalPlayers += players.length;

        if (players.length > 0) {
          console.log(`\n🏠 Room: ${room.name} (${room.id.substring(0, 8)}...)`);
          
          for (const player of players) {
            const role = player.isHost ? "👑 Host" : "👤 Player";
            console.log(`  ${role}: ${player.name} (${player.id.substring(0, 8)}...)`);
            console.log(`    📅 Joined: ${player.joinedAt}`);
          }
        }
      }
    }

    console.log(`\n📊 Total players across all rooms: ${totalPlayers}`);
  }

  async showDetailed(): Promise<void> {
    console.log("🔍 Detailed Database Inspection");
    console.log("===============================");

    await this.showStats();
    console.log("\n" + "=".repeat(50) + "\n");
    await this.showRooms();
    console.log("\n" + "=".repeat(50) + "\n");
    await this.showPlayers();
  }
}

function parseArgs(args: string[]): {
  showStats: boolean;
  showRooms: boolean;
  showPlayers: boolean;
  showSessions: boolean;
  showDetailed: boolean;
} {
  const options = {
    showStats: true, // Default
    showRooms: false,
    showPlayers: false,
    showSessions: false,
    showDetailed: false,
  };

  if (args.length === 0) {
    return options; // Default to stats
  }

  // Reset default if specific options are provided
  if (args.some(arg => ["--rooms", "--players", "--sessions", "--detailed"].includes(arg))) {
    options.showStats = false;
  }

  for (const arg of args) {
    switch (arg) {
      case "--stats":
        options.showStats = true;
        break;
      case "--rooms":
        options.showRooms = true;
        break;
      case "--players":
        options.showPlayers = true;
        break;
      case "--sessions":
        options.showSessions = true;
        break;
      case "--detailed":
        options.showDetailed = true;
        break;
    }
  }

  return options;
}

function printUsage() {
  console.log(`
🔍 Database Inspection Script

Usage:
  deno run --allow-read --allow-write --allow-env --unstable-kv scripts/inspect-database.ts [options]

Options:
  --stats      Show database statistics (default)
  --rooms      Show all rooms with player counts
  --players    Show all players with room info
  --sessions   Show all game sessions
  --detailed   Show detailed information for all tables

Examples:
  # Show basic statistics (default)
  deno task db:inspect

  # Show all rooms
  deno task db:inspect --rooms

  # Show detailed information
  deno task db:inspect --detailed
`);
}

async function main() {
  const args = Deno.args;

  if (args.includes("--help") || args.includes("-h")) {
    printUsage();
    return;
  }

  const options = parseArgs(args);
  const inspector = new DatabaseInspector();

  try {
    if (options.showDetailed) {
      await inspector.showDetailed();
    } else {
      if (options.showStats) {
        await inspector.showStats();
      }
      if (options.showRooms) {
        await inspector.showRooms();
      }
      if (options.showPlayers) {
        await inspector.showPlayers();
      }
      if (options.showSessions) {
        console.log("🎮 Game sessions inspection not implemented yet");
      }
    }
  } catch (error) {
    console.error("❌ Inspection failed:", error);
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}