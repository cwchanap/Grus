#!/usr/bin/env -S deno run -A

// Environment setup script for new deployments

import { existsSync } from "$std/fs/mod.ts";

interface EnvironmentConfig {
  environment: 'development' | 'production';
  skipDatabase: boolean;
  skipKV: boolean;
  skipMonitoring: boolean;
}

async function runCommand(cmd: string[]): Promise<string> {
  console.log(`Running: ${cmd.join(' ')}`);
  const process = new Deno.Command(cmd[0], {
    args: cmd.slice(1),
    stdout: "piped",
    stderr: "piped",
  });
  
  const { success, stdout, stderr } = await process.output();
  const output = new TextDecoder().decode(stdout);
  const error = new TextDecoder().decode(stderr);
  
  if (!success) {
    console.error(`Command failed: ${error}`);
    throw new Error(`Command failed: ${cmd.join(' ')}`);
  }
  
  return output;
}

async function checkPrerequisites(): Promise<void> {
  console.log("🔍 Checking prerequisites...");
  
  // Check Deno
  try {
    const denoVersion = await runCommand(["deno", "--version"]);
    console.log(`✅ Deno: ${denoVersion.split('\n')[0]}`);
  } catch {
    throw new Error("Deno is not installed. Please install from https://deno.land/");
  }
  
  // Check Wrangler
  try {
    const wranglerVersion = await runCommand(["wrangler", "--version"]);
    console.log(`✅ Wrangler: ${wranglerVersion.trim()}`);
  } catch {
    throw new Error("Wrangler is not installed. Run: npm install -g wrangler");
  }
  
  // Check authentication
  try {
    const whoami = await runCommand(["wrangler", "whoami"]);
    console.log(`✅ Authenticated as: ${whoami.trim()}`);
  } catch {
    throw new Error("Not authenticated with Cloudflare. Run: wrangler login");
  }
  
  // Check required files
  const requiredFiles = ["wrangler.toml", "deno.json", "main.ts"];
  for (const file of requiredFiles) {
    if (!existsSync(file)) {
      throw new Error(`Required file missing: ${file}`);
    }
  }
  console.log("✅ Required files present");
}

async function setupDatabase(config: EnvironmentConfig): Promise<void> {
  if (config.skipDatabase) {
    console.log("⚠️ Skipping database setup");
    return;
  }
  
  console.log(`🗄️ Setting up database for ${config.environment}...`);
  
  const dbName = config.environment === 'production' 
    ? 'drawing-game-db-prod' 
    : 'drawing-game-db-dev';
  
  try {
    // Try to create database
    const output = await runCommand(["wrangler", "d1", "create", dbName]);
    console.log(`✅ Database created: ${dbName}`);
    
    // Extract database ID from output
    const dbIdMatch = output.match(/database_id = "([^"]+)"/);
    if (dbIdMatch) {
      console.log(`📋 Database ID: ${dbIdMatch[1]}`);
      console.log(`⚠️ Please update wrangler.toml with this database ID`);
    }
  } catch (error) {
    if (error.message.includes("already exists")) {
      console.log(`✅ Database already exists: ${dbName}`);
    } else {
      throw error;
    }
  }
  
  // Run database setup script
  console.log("🔄 Running database migrations and seeding...");
  await runCommand([
    "deno", "run", "-A", "scripts/setup-db.ts",
    config.environment === 'production' ? '--prod' : ''
  ].filter(Boolean));
}

async function setupKVStorage(config: EnvironmentConfig): Promise<void> {
  if (config.skipKV) {
    console.log("⚠️ Skipping KV storage setup");
    return;
  }
  
  console.log(`🗂️ Setting up KV storage for ${config.environment}...`);
  
  const kvName = "GAME_STATE";
  const isPreview = config.environment !== 'production';
  
  try {
    const cmd = ["wrangler", "kv:namespace", "create", kvName];
    if (isPreview) {
      cmd.push("--preview");
    }
    
    const output = await runCommand(cmd);
    console.log(`✅ KV namespace created: ${kvName}`);
    
    // Extract KV ID from output
    const kvIdMatch = output.match(/id = "([^"]+)"/);
    if (kvIdMatch) {
      console.log(`📋 KV Namespace ID: ${kvIdMatch[1]}`);
      console.log(`⚠️ Please update wrangler.toml with this KV namespace ID`);
    }
  } catch (error) {
    if (error.message.includes("already exists")) {
      console.log(`✅ KV namespace already exists: ${kvName}`);
    } else {
      throw error;
    }
  }
}

async function setupMonitoring(config: EnvironmentConfig): Promise<void> {
  if (config.skipMonitoring) {
    console.log("⚠️ Skipping monitoring setup");
    return;
  }
  
  console.log(`📊 Setting up monitoring for ${config.environment}...`);
  
  await runCommand([
    "deno", "run", "-A", "scripts/monitoring-setup.ts",
    config.environment === 'production' ? '--prod' : ''
  ].filter(Boolean));
}

async function generateConfigurationSummary(config: EnvironmentConfig): Promise<void> {
  console.log("\n" + "=".repeat(60));
  console.log("📋 CONFIGURATION SUMMARY");
  console.log("=".repeat(60));
  
  console.log(`Environment: ${config.environment}`);
  console.log(`Database setup: ${config.skipDatabase ? 'Skipped' : 'Completed'}`);
  console.log(`KV storage setup: ${config.skipKV ? 'Skipped' : 'Completed'}`);
  console.log(`Monitoring setup: ${config.skipMonitoring ? 'Skipped' : 'Completed'}`);
  
  console.log("\n📋 NEXT STEPS:");
  console.log("1. Update wrangler.toml with the database and KV namespace IDs shown above");
  console.log("2. Review and apply monitoring configuration files");
  console.log("3. Set up environment variables in your CI/CD system:");
  console.log("   - CLOUDFLARE_API_TOKEN");
  console.log("   - STAGING_URL / PRODUCTION_URL");
  console.log("   - SLACK_WEBHOOK_URL (optional)");
  console.log("4. Test the deployment:");
  console.log(`   deno task deploy${config.environment === 'development' ? ':dev' : ''}`);
  console.log("5. Run smoke tests:");
  console.log(`   deno task smoke-tests${config.environment === 'production' ? ':prod' : ':staging'}`);
  
  console.log("\n🔗 USEFUL LINKS:");
  console.log("- Cloudflare Dashboard: https://dash.cloudflare.com");
  console.log("- Deployment Guide: ./docs/deployment.md");
  console.log("- Operations Runbook: ./docs/runbook.md");
  
  console.log("\n✅ Environment setup completed!");
}

function parseArgs(): EnvironmentConfig {
  const args = Deno.args;
  
  return {
    environment: args.includes('--prod') ? 'production' : 'development',
    skipDatabase: args.includes('--skip-database'),
    skipKV: args.includes('--skip-kv'),
    skipMonitoring: args.includes('--skip-monitoring'),
  };
}

async function main(): Promise<void> {
  const config = parseArgs();
  
  console.log(`🚀 Setting up ${config.environment} environment...`);
  console.log(`Configuration: ${JSON.stringify(config, null, 2)}`);
  
  try {
    await checkPrerequisites();
    await setupDatabase(config);
    await setupKVStorage(config);
    await setupMonitoring(config);
    await generateConfigurationSummary(config);
    
  } catch (error) {
    console.error(`❌ Environment setup failed: ${error.message}`);
    console.log("\n💡 Troubleshooting tips:");
    console.log("- Ensure you have the required permissions in Cloudflare");
    console.log("- Check that wrangler is properly authenticated");
    console.log("- Verify your Cloudflare account has the necessary quotas");
    console.log("- Review the deployment guide: ./docs/deployment.md");
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}