#!/usr/bin/env -S deno run --allow-read --allow-write

/**
 * Setup script to install Git hooks for the Deno project
 */

import * as colors from "https://deno.land/std@0.216.0/fmt/colors.ts";
import { ensureDir } from "https://deno.land/std@0.216.0/fs/ensure_dir.ts";

async function main() {
  console.log(colors.blue("🔧 Setting up Git hooks...\n"));

  try {
    // Ensure .git/hooks directory exists
    await ensureDir(".git/hooks");

    // Create pre-commit hook
    const preCommitHook = `#!/bin/sh
# Pre-commit hook for Deno project
exec deno run --allow-run --allow-read scripts/pre-commit.ts
`;

    await Deno.writeTextFile(".git/hooks/pre-commit", preCommitHook);
    
    // Make it executable
    await Deno.chmod(".git/hooks/pre-commit", 0o755);

    console.log(colors.green("✅ Pre-commit hook installed successfully!"));
    console.log(colors.gray("   Location: .git/hooks/pre-commit"));
    
    console.log(colors.blue("\n📋 Available tasks:"));
    console.log(colors.gray("   deno task pre-commit  - Run all pre-commit checks"));
    console.log(colors.gray("   deno task format      - Format code"));
    console.log(colors.gray("   deno task lint        - Run linter"));
    console.log(colors.gray("   deno task type-check  - Run type checking"));
    console.log(colors.gray("   deno task ci          - Run all CI checks"));

  } catch (error) {
    console.error(colors.red("❌ Failed to setup Git hooks:"), error instanceof Error ? error.message : String(error));
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}