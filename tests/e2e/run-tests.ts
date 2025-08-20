#!/usr/bin/env -S deno run -A --unstable-kv

/**
 * Test runner for Playwright E2E tests
 * This script handles running Playwright tests with proper Deno integration
 */

import { parseArgs } from "$std/cli/parse_args.ts";

const args = parseArgs(Deno.args, {
  boolean: ["headed", "debug", "help"],
  string: ["project", "grep", "reporter"],
  alias: {
    h: "help",
    p: "project",
    g: "grep",
    r: "reporter",
  },
});

if (args.help) {
  console.log(`
Usage: deno task test:e2e [options]

Options:
  --headed        Run tests in headed mode (show browser)
  --debug         Run tests in debug mode
  --project       Run tests for specific project (chromium, firefox, webkit)
  --grep          Run tests matching pattern
  --help, -h      Show this help message

Examples:
  deno task test:e2e                    # Run all tests headless
  deno task test:e2e --headed           # Run tests with browser visible
  deno task test:e2e --project chromium # Run only Chrome tests
  deno task test:e2e --grep "room"      # Run tests matching "room"
  `);
  Deno.exit(0);
}

// Build the Playwright command
const playwrightArgs = ["test"];

if (args.headed) {
  playwrightArgs.push("--headed");
}

if (args.debug) {
  playwrightArgs.push("--debug");
}

if (args.project) {
  playwrightArgs.push("--project", args.project);
}

if (args.grep) {
  playwrightArgs.push("--grep", args.grep);
}

// Ensure non-interactive execution and auto-quit behavior:
// - Use `npx --yes` to avoid any install prompts
// - Reporter handling: allow override via --reporter, otherwise default to 'line'
if (args.reporter) {
  playwrightArgs.push("--reporter", String(args.reporter));
} else if (!playwrightArgs.some((a) => a.startsWith("--reporter"))) {
  playwrightArgs.push("--reporter", "line");
}

// Run Playwright tests
const command = new Deno.Command("npx", {
  args: ["--yes", "playwright", ...playwrightArgs],
  cwd: Deno.cwd(),
  stdout: "inherit",
  stderr: "inherit",
});

const { code } = await command.output();
Deno.exit(code);