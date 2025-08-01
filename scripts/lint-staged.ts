#!/usr/bin/env -S deno run --allow-run --allow-read

/**
 * Lint-staged equivalent for Deno
 * Runs linting and formatting only on staged files
 */

import { globToRegExp } from "$std/path/mod.ts";
import { config, excludePatterns } from "./lint-staged.config.ts";

interface StagedFile {
  path: string;
  extension: string;
}

async function getStagedFiles(): Promise<StagedFile[]> {
  const cmd = new Deno.Command("git", {
    args: ["diff", "--cached", "--name-only", "--diff-filter=ACM"],
    stdout: "piped",
    stderr: "piped",
  });

  const { code, stdout } = await cmd.output();
  
  if (code !== 0) {
    console.error("❌ Failed to get staged files");
    Deno.exit(1);
  }

  const files = new TextDecoder().decode(stdout)
    .split("\n")
    .filter(Boolean)
    .filter(path => {
      // Exclude files matching exclude patterns
      return !excludePatterns.some(pattern => {
        const regex = globToRegExp(pattern);
        return regex.test(path);
      });
    })
    .map(path => ({
      path,
      extension: path.split(".").pop() || "",
    }));

  return files;
}

function matchesPattern(filePath: string, pattern: string): boolean {
  const regex = globToRegExp(pattern);
  return regex.test(filePath);
}

async function runCommand(
  command: string, 
  args: string[], 
  description: string,
  files?: string[]
): Promise<boolean> {
  if (files && files.length === 0) return true;
  
  console.log(`🔄 ${description}...`);
  
  const finalArgs = files ? [...args, ...files] : args;
  
  const cmd = new Deno.Command(command, {
    args: finalArgs,
    stdout: "piped",
    stderr: "piped",
  });

  const { code, stdout, stderr } = await cmd.output();
  
  if (code !== 0) {
    console.error(`❌ ${description} failed:`);
    const errorOutput = new TextDecoder().decode(stderr);
    if (errorOutput.trim()) {
      console.error(errorOutput);
    }
    return false;
  }

  const output = new TextDecoder().decode(stdout);
  if (output.trim()) {
    console.log(output);
  }

  return true;
}

async function executeCommandsForFiles(
  files: string[], 
  commands: string | string[]
): Promise<boolean> {
  const commandList = Array.isArray(commands) ? commands : [commands];
  
  for (const command of commandList) {
    const [cmd, ...args] = command.split(" ");
    
    let success = false;
    
    if (cmd === "deno" && args[0] === "fmt") {
      success = await runCommand("deno", ["fmt", ...files], `Formatting ${files.length} file(s)`);
      if (success && files.length > 0) {
        // Stage the formatted files
        await runCommand("git", ["add", ...files], "Staging formatted files");
      }
    } else if (cmd === "deno" && args[0] === "lint") {
      success = await runCommand("deno", ["lint", ...files], `Linting ${files.length} file(s)`);
    } else if (cmd === "deno" && args[0] === "check") {
      success = await runCommand("deno", args, `Type checking ${files.length} file(s)`, files);
    } else {
      // Generic command execution
      success = await runCommand(cmd, [...args, ...files], `Running ${command} on ${files.length} file(s)`);
    }
    
    if (!success) {
      return false;
    }
  }
  
  return true;
}

async function main() {
  console.log("🚀 Running lint-staged checks...\n");

  const stagedFiles = await getStagedFiles();
  
  if (stagedFiles.length === 0) {
    console.log("📝 No staged files to process");
    return;
  }

  console.log(`📁 Processing ${stagedFiles.length} staged file(s):`);
  stagedFiles.forEach(f => console.log(`   • ${f.path}`));
  console.log();

  let allPassed = true;

  // Process files according to configuration
  for (const [pattern, commands] of Object.entries(config)) {
    const matchingFiles = stagedFiles
      .filter(f => matchesPattern(f.path, pattern))
      .map(f => f.path);

    if (matchingFiles.length > 0) {
      console.log(`\n🎯 Processing ${matchingFiles.length} file(s) matching "${pattern}"`);
      
      const success = await executeCommandsForFiles(matchingFiles, commands);
      if (!success) {
        allPassed = false;
        break; // Stop on first failure
      }
    }
  }

  if (allPassed) {
    console.log("\n✅ All lint-staged checks passed!");
  } else {
    console.log("\n❌ Some lint-staged checks failed!");
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}