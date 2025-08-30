#!/usr/bin/env -S deno run -A

/**
 * Script to set up Prisma and initialize the database
 * Run with: deno run -A scripts/setup-prisma.ts
 */

import "$std/dotenv/load.ts";
import { getPrismaClient, closePrismaClient } from "../lib/auth/prisma-client.ts";

async function setupPrisma() {
  console.log("🔧 Setting up Prisma and database...\n");

  // Check for required environment variables
  const requiredVars = ["DATABASE_URL", "JWT_SECRET"];
  const missing = requiredVars.filter(v => !Deno.env.get(v));
  
  if (missing.length > 0) {
    console.error("❌ Missing required environment variables:");
    missing.forEach(v => console.error(`   - ${v}`));
    console.error("\n📝 Please create a .env file based on .env.example");
    Deno.exit(1);
  }

  try {
    // Generate Prisma client
    console.log("📦 Generating Prisma client...");
    const generateCommand = new Deno.Command("npx", {
      args: ["prisma", "generate"],
      stdout: "piped",
      stderr: "piped",
    });
    
    const generateResult = await generateCommand.output();
    if (!generateResult.success) {
      throw new Error(`Failed to generate Prisma client: ${new TextDecoder().decode(generateResult.stderr)}`);
    }
    console.log("✅ Prisma client generated");

    // Push schema to database
    console.log("\n🗄️  Pushing schema to database...");
    const pushCommand = new Deno.Command("npx", {
      args: ["prisma", "db", "push"],
      stdout: "piped",
      stderr: "piped",
    });
    
    const pushResult = await pushCommand.output();
    if (!pushResult.success) {
      throw new Error(`Failed to push schema: ${new TextDecoder().decode(pushResult.stderr)}`);
    }
    console.log("✅ Database schema updated");

    // Test connection
    console.log("\n🔌 Testing database connection...");
    const prisma = await getPrismaClient();
    await prisma.$connect();
    console.log("✅ Successfully connected to database");

    // Clean up expired sessions (if any exist)
    console.log("\n🧹 Cleaning up expired sessions...");
    const deleted = await prisma.session.deleteMany({
      where: {
        expiresAt: {
          lt: new Date(),
        },
      },
    });
    if (deleted.count > 0) {
      console.log(`✅ Removed ${deleted.count} expired sessions`);
    } else {
      console.log("✅ No expired sessions to clean");
    }

    await closePrismaClient();
    
    console.log("\n✨ Prisma setup complete!");
    console.log("\n📝 Next steps:");
    console.log("   1. Start the development server: deno task dev");
    console.log("   2. Visit http://localhost:8000 to see the app");
    console.log("   3. Click 'Login' to access the authentication pages");
    
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    console.error("\n❌ Setup failed:", message);
    Deno.exit(1);
  }
}

// Run setup
if (import.meta.main) {
  await setupPrisma();
}
