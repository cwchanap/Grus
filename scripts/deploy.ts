#!/usr/bin/env -S deno run -A

// Deployment script for Cloudflare Workers

import { getConfig } from "../lib/config.ts";

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

async function deploy(environment: 'development' | 'production' = 'production') {
  console.log(`🚀 Deploying to ${environment}...`);
  
  const config = getConfig(environment);
  
  try {
    // Build the application
    console.log("📦 Building application...");
    await runCommand(["deno", "task", "build"]);
    
    // Deploy to Cloudflare Workers
    console.log("☁️ Deploying to Cloudflare Workers...");
    if (environment === 'production') {
      await runCommand(["wrangler", "deploy", "--env", "production"]);
    } else {
      await runCommand(["wrangler", "deploy", "--env", "development"]);
    }
    
    console.log("✅ Deployment completed successfully!");
    console.log(`🌐 Application deployed to ${environment} environment`);
    
  } catch (error) {
    console.error("❌ Deployment failed:", error.message);
    Deno.exit(1);
  }
}

// Parse command line arguments
const args = Deno.args;
const environment = args.includes('--dev') ? 'development' : 'production';

if (import.meta.main) {
  await deploy(environment);
}