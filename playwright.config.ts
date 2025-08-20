import { defineConfig, devices } from "@playwright/test";
import process from "node:process";

/**
 * Playwright configuration for testing the multiplayer drawing game
 */

// Check if we're in Deno or Node environment
const isCI = (typeof Deno !== "undefined" ? Deno.env.get("CI") : process.env.CI) === "true";

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: true,
  forbidOnly: isCI,
  retries: isCI ? 2 : 0,
  workers: isCI ? 1 : undefined,
  reporter: "html",

  use: {
    baseURL: "http://localhost:3000",
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
    {
      name: "firefox",
      use: { ...devices["Desktop Firefox"] },
    },
    {
      name: "webkit",
      use: { ...devices["Desktop Safari"] },
    },
    {
      name: "Mobile Chrome",
      use: { ...devices["Pixel 5"] },
    },
    {
      name: "Mobile Safari",
      use: { ...devices["iPhone 12"] },
    },
  ],

  webServer: {
    command: "deno task start",
    url: "http://localhost:3000",
    reuseExistingServer: !isCI,
    timeout: 120 * 1000,
  },
});
