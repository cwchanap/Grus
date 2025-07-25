#!/usr/bin/env -S deno run -A

// Pre-deployment validation script

import { existsSync } from "$std/fs/mod.ts";

interface CheckResult {
  name: string;
  success: boolean;
  message: string;
  critical: boolean;
}

class PreDeployChecker {
  private results: CheckResult[] = [];

  async runCheck(
    name: string, 
    checkFn: () => Promise<void>, 
    critical = true
  ): Promise<void> {
    console.log(`🔍 Checking: ${name}`);
    
    try {
      await checkFn();
      this.results.push({
        name,
        success: true,
        message: "✅ Passed",
        critical
      });
      console.log(`✅ ${name}: Passed`);
    } catch (error) {
      this.results.push({
        name,
        success: false,
        message: error.message,
        critical
      });
      console.log(`${critical ? '❌' : '⚠️'} ${name}: ${error.message}`);
    }
  }

  async checkRequiredFiles(): Promise<void> {
    const requiredFiles = [
      'wrangler.toml',
      'deno.json',
      'main.ts',
      'fresh.config.ts',
      'db/schema.sql',
      'db/migrations/001_initial_schema.sql'
    ];

    for (const file of requiredFiles) {
      if (!existsSync(file)) {
        throw new Error(`Required file missing: ${file}`);
      }
    }
  }

  async checkWranglerConfig(): Promise<void> {
    const config = await Deno.readTextFile('wrangler.toml');
    
    // Check for placeholder values
    if (config.includes('your-database-id-here')) {
      throw new Error('Database ID not configured in wrangler.toml');
    }
    
    if (config.includes('your-kv-namespace-id-here')) {
      throw new Error('KV namespace ID not configured in wrangler.toml');
    }
    
    // Check for required bindings
    if (!config.includes('[[d1_databases]]')) {
      throw new Error('D1 database binding not configured');
    }
    
    if (!config.includes('[[kv_namespaces]]')) {
      throw new Error('KV namespace binding not configured');
    }
  }

  async checkEnvironmentVariables(): Promise<void> {
    const requiredEnvVars = ['CLOUDFLARE_API_TOKEN'];
    const optionalEnvVars = ['SLACK_WEBHOOK_URL', 'STAGING_URL', 'PRODUCTION_URL'];
    
    for (const envVar of requiredEnvVars) {
      if (!Deno.env.get(envVar)) {
        throw new Error(`Required environment variable missing: ${envVar}`);
      }
    }
    
    const missingOptional = optionalEnvVars.filter(env => !Deno.env.get(env));
    if (missingOptional.length > 0) {
      console.log(`⚠️ Optional environment variables missing: ${missingOptional.join(', ')}`);
    }
  }

  async checkDatabaseMigrations(): Promise<void> {
    const migrationsDir = 'db/migrations';
    if (!existsSync(migrationsDir)) {
      throw new Error('Migrations directory not found');
    }

    const migrations = [];
    for await (const entry of Deno.readDir(migrationsDir)) {
      if (entry.isFile && entry.name.endsWith('.sql')) {
        migrations.push(entry.name);
      }
    }

    if (migrations.length === 0) {
      throw new Error('No migration files found');
    }

    // Check migration naming convention
    for (const migration of migrations) {
      if (!/^\d{3}_\w+\.sql$/.test(migration)) {
        throw new Error(`Invalid migration filename: ${migration}. Use format: 001_description.sql`);
      }
    }
  }

  async checkTypeScript(): Promise<void> {
    const process = new Deno.Command("deno", {
      args: ["check", "**/*.ts", "**/*.tsx"],
      stdout: "piped",
      stderr: "piped",
    });

    const { success, stderr } = await process.output();
    if (!success) {
      const error = new TextDecoder().decode(stderr);
      throw new Error(`TypeScript check failed: ${error}`);
    }
  }

  async checkLinting(): Promise<void> {
    const process = new Deno.Command("deno", {
      args: ["lint"],
      stdout: "piped",
      stderr: "piped",
    });

    const { success, stderr } = await process.output();
    if (!success) {
      const error = new TextDecoder().decode(stderr);
      throw new Error(`Linting failed: ${error}`);
    }
  }

  async checkFormatting(): Promise<void> {
    const process = new Deno.Command("deno", {
      args: ["fmt", "--check"],
      stdout: "piped",
      stderr: "piped",
    });

    const { success, stderr } = await process.output();
    if (!success) {
      const error = new TextDecoder().decode(stderr);
      throw new Error(`Formatting check failed: ${error}`);
    }
  }

  async checkTests(): Promise<void> {
    const process = new Deno.Command("deno", {
      args: ["test", "--allow-all"],
      stdout: "piped",
      stderr: "piped",
    });

    const { success, stderr } = await process.output();
    if (!success) {
      const error = new TextDecoder().decode(stderr);
      throw new Error(`Tests failed: ${error}`);
    }
  }

  async checkWranglerAuth(): Promise<void> {
    const process = new Deno.Command("wrangler", {
      args: ["whoami"],
      stdout: "piped",
      stderr: "piped",
    });

    const { success, stderr } = await process.output();
    if (!success) {
      const error = new TextDecoder().decode(stderr);
      throw new Error(`Wrangler authentication failed: ${error}`);
    }
  }

  async checkSecrets(): Promise<void> {
    // Check for common secrets in code
    const secretPatterns = [
      /api[_-]?key\s*[:=]\s*["'][^"']+["']/i,
      /secret[_-]?key\s*[:=]\s*["'][^"']+["']/i,
      /password\s*[:=]\s*["'][^"']+["']/i,
      /token\s*[:=]\s*["'][^"']+["']/i,
    ];

    const filesToCheck = [];
    for await (const entry of Deno.readDir('.')) {
      if (entry.isFile && (entry.name.endsWith('.ts') || entry.name.endsWith('.tsx'))) {
        filesToCheck.push(entry.name);
      }
    }

    for (const file of filesToCheck) {
      const content = await Deno.readTextFile(file);
      for (const pattern of secretPatterns) {
        if (pattern.test(content)) {
          throw new Error(`Potential secret found in ${file}. Please use environment variables.`);
        }
      }
    }
  }

  async runAllChecks(): Promise<boolean> {
    console.log("🚀 Running pre-deployment checks...");
    console.log("=" .repeat(50));

    // Critical checks (deployment will fail if these fail)
    await this.runCheck("Required Files", () => this.checkRequiredFiles(), true);
    await this.runCheck("Wrangler Configuration", () => this.checkWranglerConfig(), true);
    await this.runCheck("Environment Variables", () => this.checkEnvironmentVariables(), true);
    await this.runCheck("Database Migrations", () => this.checkDatabaseMigrations(), true);
    await this.runCheck("TypeScript Check", () => this.checkTypeScript(), true);
    await this.runCheck("Wrangler Authentication", () => this.checkWranglerAuth(), true);

    // Non-critical checks (warnings only)
    await this.runCheck("Code Linting", () => this.checkLinting(), false);
    await this.runCheck("Code Formatting", () => this.checkFormatting(), false);
    await this.runCheck("Unit Tests", () => this.checkTests(), false);
    await this.runCheck("Secret Detection", () => this.checkSecrets(), false);

    console.log("=" .repeat(50));
    console.log("📊 Check Results:");

    const criticalFailures = this.results.filter(r => !r.success && r.critical);
    const warnings = this.results.filter(r => !r.success && !r.critical);
    const passed = this.results.filter(r => r.success);

    console.log(`✅ Passed: ${passed.length}`);
    console.log(`⚠️ Warnings: ${warnings.length}`);
    console.log(`❌ Critical Failures: ${criticalFailures.length}`);

    if (criticalFailures.length > 0) {
      console.log("\n❌ Critical Issues (must be fixed before deployment):");
      criticalFailures.forEach(r => console.log(`  - ${r.name}: ${r.message}`));
    }

    if (warnings.length > 0) {
      console.log("\n⚠️ Warnings (recommended to fix):");
      warnings.forEach(r => console.log(`  - ${r.name}: ${r.message}`));
    }

    const canDeploy = criticalFailures.length === 0;
    
    if (canDeploy) {
      console.log("\n✅ All critical checks passed! Ready for deployment.");
    } else {
      console.log("\n❌ Critical issues found. Please fix before deploying.");
    }

    return canDeploy;
  }
}

async function main(): Promise<void> {
  const checker = new PreDeployChecker();
  const canDeploy = await checker.runAllChecks();
  
  if (!canDeploy) {
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}