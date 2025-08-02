// Database service factory - now uses SQLite for both development and production
import { getDatabaseService as getService } from "./db/index.ts";

// Re-export the database service
export function getDatabaseService() {
  return getService();
}

// For development: clear all data (SQLite version)
export function clearDevelopmentData(): void {
  const isDevelopment = Deno.env.get("DENO_ENV") !== "production";
  if (isDevelopment) {
    // For SQLite, we could truncate tables or delete the database file
    // For now, just log that this would clear data
    console.log("Development database clear requested (SQLite)");
  }
}

// For development: get stats (SQLite version)
export function getDevelopmentStats(): any {
  const isDevelopment = Deno.env.get("DENO_ENV") !== "production";
  if (isDevelopment) {
    // Could implement table row counts for SQLite
    console.log("Development stats requested (SQLite)");
    return { message: "SQLite stats not implemented yet" };
  }
  return null;
}
