#!/usr/bin/env -S deno run -A

// Deployment script for Cloudflare Workers

import { getConfig } from "../lib/config.ts";
import { existsSync } from "$std/fs/mod.ts";

interface DeploymentConfig {
  environment: 'development' | 'production';
  skipTests: boolean;
  skipBuild: boolean;
  dryRun: boolean;
}

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

async function validateEnvironment(): Promise<void> {
  console.log("🔍 Validating environment...");
  
  // Check if wrangler is installed
  try {
    await runCommand(["wrangler", "--version"]);
  } catch {
    throw new Error("Wrangler CLI is not installed. Run: npm install -g wrangler");
  }
  
  // Check if user is logged in
  try {
    await runCommand(["wrangler", "whoami"]);
  } catch {
    throw new Error("Not logged in to Cloudflare. Run: wrangler login");
  }
  
  // Check if required files exist
  const requiredFiles = ["wrangler.toml", "main.ts", "deno.json"];
  for (const file of requiredFiles) {
    if (!existsSync(file)) {
      throw new Error(`Required file missing: ${file}`);
    }
  }
  
  console.log("✅ Environment validation passed");
}

async function runTests(): Promise<void> {
  console.log("🧪 Running tests...");
  await runCommand(["deno", "task", "test"]);
  console.log("✅ All tests passed");
}

async function buildApplication(): Promise<void> {
  console.log("📦 Building application...");
  await runCommand(["deno", "task", "build"]);
  console.log("✅ Build completed");
}

async function deployToCloudflare(config: DeploymentConfig): Promise<void> {
  console.log(`☁️ Deploying to Cloudflare Workers (${config.environment})...`);
  
  const deployCmd = ["wrangler", "deploy"];
  
  if (config.environment !== 'production') {
    deployCmd.push("--env", config.environment);
  }
  
  if (config.dryRun) {
    deployCmd.push("--dry-run");
    console.log("🔍 Running dry-run deployment...");
  }
  
  await runCommand(deployCmd);
  
  if (!config.dryRun) {
    console.log("✅ Deployment completed successfully!");
  } else {
    console.log("✅ Dry-run completed successfully!");
  }
}

async function runSmokeTests(environment: string): Promise<void> {
  console.log("🧪 Running smoke tests...");
  await runCommand([
    "deno", "run", "--allow-net", 
    "scripts/smoke-tests.ts", 
    `--env=${environment}`
  ]);
  console.log("✅ Smoke tests passed");
}

async function notifyDeployment(config: DeploymentConfig, success: boolean): Promise<void> {
  const webhookUrl = Deno.env.get("SLACK_WEBHOOK_URL");
  if (!webhookUrl) {
    console.log("No Slack webhook configured, skipping notification");
    return;
  }
  
  const message = success 
    ? `🚀 Deployment to ${config.environment} completed successfully!`
    : `❌ Deployment to ${config.environment} failed!`;
    
  try {
    await fetch(webhookUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ text: message })
    });
    console.log("📢 Notification sent");
  } catch (error) {
    console.log(`Failed to send notification: ${error.message}`);
  }
}

async function deploy(config: DeploymentConfig): Promise<void> {
  console.log(`🚀 Starting deployment to ${config.environment}...`);
  console.log(`Configuration: ${JSON.stringify(config, null, 2)}`);
  
  let success = false;
  let deploymentId: string | null = null;
  
  try {
    // Run pre-deployment checks
    console.log("🔍 Running pre-deployment checks...");
    await runCommand(["deno", "run", "-A", "scripts/pre-deploy-checks.ts"]);
    
    // Validate environment
    await validateEnvironment();
    
    // Run tests (unless skipped)
    if (!config.skipTests) {
      await runTests();
    } else {
      console.log("⚠️ Skipping tests");
    }
    
    // Build application (unless skipped)
    if (!config.skipBuild) {
      await buildApplication();
    } else {
      console.log("⚠️ Skipping build");
    }
    
    // Get current deployment ID for potential rollback
    if (!config.dryRun) {
      try {
        const currentDeployment = await runCommand(["wrangler", "deployments", "list", "--limit", "1"]);
        const deploymentMatch = currentDeployment.match(/([a-f0-9-]{36})/);
        if (deploymentMatch) {
          deploymentId = deploymentMatch[1];
          console.log(`📋 Current deployment ID: ${deploymentId}`);
        }
      } catch {
        console.log("⚠️ Could not retrieve current deployment ID");
      }
    }
    
    // Deploy to Cloudflare Workers
    await deployToCloudflare(config);
    
    // Run smoke tests (only for actual deployments, not dry runs)
    if (!config.dryRun) {
      try {
        await runSmokeTests(config.environment);
      } catch (error) {
        console.error(`❌ Smoke tests failed: ${error.message}`);
        
        // Offer rollback option
        if (deploymentId && config.environment === 'production') {
          console.log("🔄 Attempting automatic rollback...");
          try {
            await runCommand(["wrangler", "rollback", "--version-id", deploymentId]);
            console.log("✅ Rollback completed successfully");
            throw new Error("Deployment rolled back due to smoke test failures");
          } catch (rollbackError) {
            console.error(`❌ Rollback failed: ${rollbackError.message}`);
            throw new Error(`Smoke tests failed and rollback failed: ${error.message}`);
          }
        } else {
          throw error;
        }
      }
    }
    
    success = true;
    console.log(`🎉 Deployment to ${config.environment} completed successfully!`);
    
  } catch (error) {
    console.error(`❌ Deployment failed: ${error.message}`);
    success = false;
  } finally {
    // Send notification
    await notifyDeployment(config, success);
    
    if (!success) {
      Deno.exit(1);
    }
  }
}

function parseArgs(): DeploymentConfig {
  const args = Deno.args;
  
  return {
    environment: args.includes('--dev') ? 'development' : 'production',
    skipTests: args.includes('--skip-tests'),
    skipBuild: args.includes('--skip-build'),
    dryRun: args.includes('--dry-run')
  };
}

if (import.meta.main) {
  const config = parseArgs();
  await deploy(config);
}