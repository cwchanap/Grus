import { defineConfig, devices } from "@playwright/test";
import process from "node:process";

/**
 * Playwright configuration for testing the multiplayer drawing game
 */

// Check if we're in Deno or Node environment
const isCI = (typeof Deno !== "undefined" ? Deno.env.get("CI") : process.env.CI) === "true";

export default defineConfig({
  testDir: "./",
  fullyParallel: true,
  forbidOnly: isCI,
  retries: isCI ? 2 : 0,
  workers: isCI ? 5 : undefined,
  reporter: [
    ["list"],
    ["html", { open: "never" }],
  ],

  use: {
    baseURL: "http://localhost:3000",
    // Run headed locally to avoid macOS headless SIGTRAP crashes; keep headless in CI
    headless: isCI,
    trace: "on-first-retry",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
    // Stabilize Chromium on macOS (avoid SIGTRAP crashes) by disabling GPU
    // and forcing ANGLE software rendering. Safe no-ops for Firefox/WebKit.
    launchOptions: {
      args: [
        "--disable-gpu",
        "--use-gl=angle",
        "--use-angle=swiftshader",
      ],
    },
  },

  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],

  webServer: {
    // Use non-watching server to avoid restarts during tests
    command: "deno task --config ../../deno.json preview",
    url: "http://localhost:3000",
    reuseExistingServer: !isCI,
    timeout: 120 * 1000,
  },
});
