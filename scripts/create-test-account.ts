#!/usr/bin/env -S deno run --allow-read --allow-write --allow-env --unstable-kv

/**
 * Create Test Account Script
 * 
 * Creates a test account for development and testing purposes.
 * The test account can be used to test authentication, profile features,
 * and multiplayer functionality.
 * 
 * NOTE: This script requires authentication to be set up first.
 * See docs/authentication-setup.md for setup instructions.
 */

import { createUser, createSession } from "../lib/auth/auth-utils.ts";
import { getPrismaClient } from "../lib/auth/prisma-client.ts";

const TEST_ACCOUNT = {
  email: "test@grus.dev",
  username: "testuser",
  name: "Test User",
  password: "testpass123",
};

async function checkAuthenticationSetup(): Promise<boolean> {
  try {
    const databaseUrl = Deno.env.get("DATABASE_URL");
    const jwtSecret = Deno.env.get("JWT_SECRET");
    
    if (!databaseUrl) {
      console.log("❌ DATABASE_URL environment variable not set");
      return false;
    }
    
    if (!jwtSecret) {
      console.log("⚠️  JWT_SECRET environment variable not set (using default)");
    }
    
    // Try to connect to database
    await getPrismaClient();
    return true;
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    console.log("❌ Authentication not set up:", errorMessage);
    return false;
  }
}

async function createTestAccount() {
  console.log("🎮 Creating test account for Grus Drawing Game...\n");

  try {
    const prisma = await getPrismaClient();

    // Check if test user already exists
    const existingUser = await prisma.user.findFirst({
      where: {
        OR: [
          { email: TEST_ACCOUNT.email },
          { username: TEST_ACCOUNT.username },
        ],
      },
    });

    if (existingUser) {
      console.log("✅ Test account already exists:");
      console.log(`   Email: ${existingUser.email}`);
      console.log(`   Username: ${existingUser.username}`);
      console.log(`   Name: ${existingUser.name}`);
      console.log(`   ID: ${existingUser.id}\n`);
      
      // Create a fresh session for testing
      const session = await createSession(existingUser);
      console.log("🔑 Fresh session created for testing");
      console.log(`   Token: ${session.token.substring(0, 20)}...`);
      console.log(`   Expires: ${session.expiresAt.toISOString()}\n`);
      
      return existingUser;
    }

    // Create new test user
    console.log("👤 Creating new test account...");
    const newUser = await createUser(
      TEST_ACCOUNT.email,
      TEST_ACCOUNT.username,
      TEST_ACCOUNT.password,
      TEST_ACCOUNT.name,
    );

    console.log("✅ Test account created successfully:");
    console.log(`   Email: ${newUser.email}`);
    console.log(`   Username: ${newUser.username}`);
    console.log(`   Name: ${newUser.name}`);
    console.log(`   ID: ${newUser.id}\n`);

    // Create initial session
    const session = await createSession(newUser);
    console.log("🔑 Initial session created:");
    console.log(`   Token: ${session.token.substring(0, 20)}...`);
    console.log(`   Expires: ${session.expiresAt.toISOString()}\n`);

    return newUser;
  } catch (error) {
    console.error("❌ Error creating test account:", error);
    throw error;
  }
}

async function main() {
  console.log("🔧 Checking authentication setup...\n");
  
  const isSetup = await checkAuthenticationSetup();
  
  if (!isSetup) {
    console.log("\n📚 Authentication Setup Required:");
    console.log("1. Follow the setup guide: docs/authentication-setup.md");
    console.log("2. Set up DATABASE_URL environment variable");
    console.log("3. Run: deno run -A scripts/setup-prisma.ts");
    console.log("4. Then run this script again\n");
    
    console.log("🎮 Alternative: Play Without Authentication");
    console.log("• The game works without authentication");
    console.log("• Users can play as guests");
    console.log("• Profile features require authentication");
    console.log("• Start dev server: deno task start\n");
    
    return;
  }
  
  try {
    const user = await createTestAccount();
    
    console.log("🎯 Test Account Details:");
    console.log("┌─────────────────────────────────────────┐");
    console.log(`│ Email:    ${TEST_ACCOUNT.email.padEnd(26)} │`);
    console.log(`│ Username: ${TEST_ACCOUNT.username.padEnd(26)} │`);
    console.log(`│ Password: ${TEST_ACCOUNT.password.padEnd(26)} │`);
    console.log(`│ Name:     ${TEST_ACCOUNT.name.padEnd(26)} │`);
    console.log("└─────────────────────────────────────────┘");
    
    console.log("\n📝 Usage Instructions:");
    console.log("1. Start the development server: deno task start");
    console.log("2. Navigate to http://localhost:3000/login");
    console.log("3. Use the credentials above to log in");
    console.log("4. Test profile navigation by clicking the username");
    console.log("5. Create/join rooms to test multiplayer features\n");
    
    console.log("🔧 Development Notes:");
    console.log("• This account is for testing only");
    console.log("• Password is intentionally simple for dev convenience");
    console.log("• Account will persist until database is reset");
    console.log("• Use 'deno task db:cleanup:all' to remove all data\n");
    
  } catch (error) {
    console.error("Failed to create test account:", error);
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}
