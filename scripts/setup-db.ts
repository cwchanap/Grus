#!/usr/bin/env -S deno run -A

// Database setup script for Cloudflare D1

import { existsSync } from "$std/fs/mod.ts";

async function runCommand(cmd: string[]): Promise<void> {
  console.log(`Running: ${cmd.join(' ')}`);
  const process = new Deno.Command(cmd[0], {
    args: cmd.slice(1),
    stdout: "inherit",
    stderr: "inherit",
  });
  
  const { success } = await process.output();
  if (!success) {
    throw new Error(`Command failed: ${cmd.join(' ')}`);
  }
}

async function runMigrations(dbName: string): Promise<void> {
  const migrationsDir = './db/migrations';
  
  if (!existsSync(migrationsDir)) {
    console.log("No migrations directory found, skipping migrations");
    return;
  }
  
  const migrations = [];
  for await (const entry of Deno.readDir(migrationsDir)) {
    if (entry.isFile && entry.name.endsWith('.sql')) {
      migrations.push(entry.name);
    }
  }
  
  migrations.sort(); // Run migrations in order
  
  for (const migration of migrations) {
    console.log(`🔄 Running migration: ${migration}`);
    await runCommand([
      "wrangler", "d1", "execute", dbName,
      "--file", `${migrationsDir}/${migration}`
    ]);
  }
}

async function seedDatabase(dbName: string): Promise<void> {
  const seedFile = './db/seeds.sql';
  
  if (existsSync(seedFile)) {
    console.log("🌱 Seeding database...");
    await runCommand([
      "wrangler", "d1", "execute", dbName,
      "--file", seedFile
    ]);
  } else {
    console.log("No seed file found, skipping seeding");
  }
}

async function setupDatabase(environment: 'development' | 'production' = 'development') {
  console.log(`🗄️ Setting up database for ${environment}...`);
  
  try {
    const dbName = environment === 'production' ? 'drawing-game-db' : 'drawing-game-db-dev';
    
    // Create database (if it doesn't exist)
    console.log("📊 Creating database...");
    try {
      await runCommand(["wrangler", "d1", "create", dbName]);
    } catch (error) {
      console.log("Database might already exist, continuing...");
    }
    
    // Run migrations
    console.log("🔄 Running database migrations...");
    await runMigrations(dbName);
    
    // Seed database
    await seedDatabase(dbName);
    
    console.log("✅ Database setup completed successfully!");
    console.log(`📋 Next steps:`);
    console.log(`1. Update wrangler.toml with your database ID`);
    console.log(`2. Run: wrangler d1 list to see your databases`);
    
  } catch (error) {
    console.error("❌ Database setup failed:", error.message);
    console.log("💡 Make sure you have wrangler installed and are logged in:");
    console.log("   npm install -g wrangler");
    console.log("   wrangler login");
    Deno.exit(1);
  }
}

// Parse command line arguments
const args = Deno.args;
const environment = args.includes('--prod') ? 'production' : 'development';

if (import.meta.main) {
  await setupDatabase(environment);
}