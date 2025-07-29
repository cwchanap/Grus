#!/usr/bin/env -S deno run --allow-run --allow-read

/**
 * Pre-commit hook script for Deno projects
 * Uses lint-staged approach to only process staged files
 */

import * as colors from "$std/fmt/colors.ts";

async function runCommand(command: string[]): Promise<{ success: boolean; output: string }> {
  try {
    const process = new Deno.Command(command[0], {
      args: command.slice(1),
      stdout: "piped",
      stderr: "piped",
    });

    const { code, stdout, stderr } = await process.output();
    const output = new TextDecoder().decode(stdout) + new TextDecoder().decode(stderr);
    
    return { success: code === 0, output };
  } catch (error) {
    return { success: false, output: error.message };
  }
}

async function main() {
  console.log(colors.blue("🔍 Running pre-commit checks...\n"));

  // Run lint-staged to process only staged files
  const { success, output } = await runCommand(["deno", "task", "lint-staged"]);

  if (success) {
    console.log(colors.green("✅ All checks passed! Commit can proceed."));
    Deno.exit(0);
  } else {
    console.log(colors.red("💥 Some checks failed. Please fix the issues before committing."));
    console.log(output);
    console.log(colors.yellow("Tip: Run 'deno task lint-staged' to fix issues manually."));
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}