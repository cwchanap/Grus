#!/usr/bin/env -S deno run --allow-run --allow-read

/**
 * Pre-commit hook script for Deno projects
 * This script runs formatting, linting, and type checking before commits
 */

import * as colors from "https://deno.land/std@0.216.0/fmt/colors.ts";

interface CheckResult {
  name: string;
  success: boolean;
  output?: string;
}

async function runCommand(cmd: string[]): Promise<CheckResult> {
  const process = new Deno.Command(cmd[0], {
    args: cmd.slice(1),
    stdout: "piped",
    stderr: "piped",
  });

  const { code, stdout, stderr } = await process.output();
  const output = new TextDecoder().decode(stdout) + new TextDecoder().decode(stderr);

  return {
    name: cmd.join(" "),
    success: code === 0,
    output: output.trim(),
  };
}

async function main() {
  console.log(colors.blue("🔍 Running pre-commit checks...\n"));

  const checks: CheckResult[] = [];

  // Format check
  console.log("📝 Checking formatting...");
  checks.push(await runCommand(["deno", "fmt", "--check"]));

  // Linting
  console.log("🔍 Running linter...");
  checks.push(await runCommand(["deno", "lint"]));

  // Type checking
  console.log("🔧 Type checking...");
  checks.push(await runCommand(["deno", "check", "**/*.ts", "**/*.tsx"]));

  // Print results
  console.log("\n" + colors.bold("Results:"));
  let allPassed = true;

  for (const check of checks) {
    if (check.success) {
      console.log(colors.green(`✅ ${check.name}`));
    } else {
      console.log(colors.red(`❌ ${check.name}`));
      if (check.output) {
        console.log(colors.gray(check.output));
      }
      allPassed = false;
    }
  }

  if (allPassed) {
    console.log(colors.green("\n🎉 All checks passed! Commit can proceed."));
    Deno.exit(0);
  } else {
    console.log(colors.red("\n💥 Some checks failed. Please fix the issues before committing."));
    console.log(colors.yellow("\nTip: Run 'deno task pre-commit' to fix formatting and linting issues."));
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}