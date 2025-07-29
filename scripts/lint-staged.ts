#!/usr/bin/env -S deno run --allow-run --allow-read
/**
 * Deno equivalent of lint-staged
 * Runs linting and formatting only on staged files
 */

import * as colors from "$std/fmt/colors.ts";

interface StagedFile {
  path: string;
  status: string;
}

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
    return { success: false, output: error instanceof Error ? error.message : String(error) };
  }
}

async function getStagedFiles(): Promise<StagedFile[]> {
  const { success, output } = await runCommand(["git", "diff", "--cached", "--name-status"]);

  if (!success) {
    console.error(colors.red("❌ Failed to get staged files"));
    return [];
  }

  return output
    .trim()
    .split("\n")
    .filter((line) => line.trim())
    .map((line) => {
      const [status, path] = line.split("\t");
      return { status, path };
    })
    .filter((file) =>
      // Only process TypeScript/JavaScript files that exist (not deleted)
      !file.status.startsWith("D") &&
      (file.path.endsWith(".ts") || file.path.endsWith(".tsx") || file.path.endsWith(".js") ||
        file.path.endsWith(".jsx"))
    );
}

async function formatFiles(files: string[]): Promise<boolean> {
  if (files.length === 0) return true;

  console.log(colors.blue(`📝 Formatting ${files.length} staged files...`));

  const { success, output } = await runCommand(["deno", "fmt", ...files]);

  if (!success) {
    console.error(colors.red("❌ Formatting failed:"));
    console.error(output);
    return false;
  }

  console.log(colors.green("✅ Files formatted successfully"));
  return true;
}

async function lintFiles(files: string[]): Promise<boolean> {
  if (files.length === 0) return true;

  console.log(colors.blue(`🔍 Linting ${files.length} staged files...`));

  const { success, output } = await runCommand(["deno", "lint", ...files]);

  if (!success) {
    console.error(colors.yellow("⚠️ Linting issues found:"));
    console.error(output);
    // Don't fail on linting issues, just warn
    return true;
  }

  console.log(colors.green("✅ No linting issues found"));
  return true;
}

async function typeCheckFiles(files: string[]): Promise<boolean> {
  if (files.length === 0) return true;

  console.log(colors.blue(`🔧 Type checking ${files.length} staged files...`));

  const { success, output } = await runCommand(["deno", "check", ...files]);

  if (!success) {
    console.error(colors.red("❌ Type checking failed:"));
    console.error(output);
    return false;
  }

  console.log(colors.green("✅ Type checking passed"));
  return true;
}

async function restageFiles(files: string[]): Promise<void> {
  if (files.length === 0) return;

  console.log(colors.blue("📦 Re-staging formatted files..."));

  const { success } = await runCommand(["git", "add", ...files]);

  if (!success) {
    console.error(colors.yellow("⚠️ Failed to re-stage some files"));
  }
}

async function main() {
  console.log(colors.bold("🚀 Running lint-staged for Deno..."));

  const stagedFiles = await getStagedFiles();

  if (stagedFiles.length === 0) {
    console.log(colors.green("✅ No staged TypeScript/JavaScript files to process"));
    return;
  }

  const filePaths = stagedFiles.map((f) => f.path);
  console.log(colors.cyan(`Found ${filePaths.length} staged files:`));
  filePaths.forEach((path) => console.log(colors.gray(`  - ${path}`)));

  let allPassed = true;

  // 1. Format files first
  const formatSuccess = await formatFiles(filePaths);
  if (!formatSuccess) {
    allPassed = false;
  }

  // 2. Re-stage formatted files
  if (formatSuccess) {
    await restageFiles(filePaths);
  }

  // 3. Run linting (non-blocking)
  await lintFiles(filePaths);

  // 4. Type check files
  const typeCheckSuccess = await typeCheckFiles(filePaths);
  if (!typeCheckSuccess) {
    allPassed = false;
  }

  if (allPassed) {
    console.log(colors.green("✅ All checks passed! Commit can proceed."));
    Deno.exit(0);
  } else {
    console.log(colors.red("💥 Some checks failed. Please fix the issues before committing."));
    console.log(colors.yellow("Tip: Run 'deno task lint-staged' to fix issues manually."));
    Deno.exit(1);
  }
}

if (import.meta.main) {
  main();
}
