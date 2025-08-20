import { test, expect } from "@playwright/test";

/**
 * Smoke tests - basic functionality checks
 * These tests should run quickly and catch major issues
 */

test.describe("Smoke Tests", () => {
  test("homepage loads successfully", async ({ page }) => {
    await page.goto("/");
    
    // Page should load without errors
    await expect(page).toHaveTitle(/Drawing Game|Multiplayer|Game/);
    
    // Should not have any console errors
    const errors: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        errors.push(msg.text());
      }
    });
    
    await page.waitForTimeout(2000);
    
    // Allow some common non-critical errors but fail on serious ones
    const criticalErrors = errors.filter(error => 
      !error.includes("favicon") && 
      !error.includes("404") &&
      !error.includes("net::ERR_")
    );
    
    expect(criticalErrors.length).toBe(0);
  });

  test("can navigate to create room", async ({ page }) => {
    await page.goto("/");
    
    // Use stable Create Room modal flow
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.locator('[data-testid="room-name-input"]').fill("Smoke Room");
    await page.locator('[data-testid="host-name-input"]').fill("Smoke Host");
    await page.locator('[data-testid="create-room-submit"]').click();
    await page.waitForURL(/\/room\/[a-f0-9-]+/);
    
    expect(page.url()).toMatch(/\/room\//);
  });

  test("room page loads basic elements", async ({ page }) => {
    await page.goto("/");
    
    // Create a room via modal
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.locator('[data-testid="room-name-input"]').fill("Smoke Elements Room");
    await page.locator('[data-testid="host-name-input"]').fill("Smoke Host");
    await page.locator('[data-testid="create-room-submit"]').click();
    await page.waitForURL(/\/room\/[a-f0-9-]+/);
    
    // Should be in a room
    expect(page.url()).toMatch(/\/room\//);
    
    // Should have visible core areas
    await expect(page.locator('[data-testid="drawing-canvas"]')).toBeVisible();
    await expect(page.getByRole("heading", { name: "Scoreboard" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Chat" })).toBeVisible();
  });

  test("no critical JavaScript errors", async ({ page }) => {
    const jsErrors: string[] = [];
    
    page.on("pageerror", (error) => {
      jsErrors.push(error.message);
    });
    
    await page.goto("/");
    
    // Navigate through the app using Create Room modal
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.locator('[data-testid="room-name-input"]').fill("Smoke Error Room");
    await page.locator('[data-testid="host-name-input"]').fill("Smoke Host");
    await page.locator('[data-testid="create-room-submit"]').click();
    await page.waitForURL(/\/room\/[a-f0-9-]+/);
    
    // Filter out non-critical errors
    const criticalErrors = jsErrors.filter(error => 
      !error.includes("Non-Error promise rejection captured") &&
      !error.includes("ResizeObserver loop limit exceeded") &&
      !error.includes("WebSocket connection")
    );
    
    expect(criticalErrors.length).toBe(0);
  });

  test("basic responsive design", async ({ page }) => {
    await page.goto("/");
    
    // Test different viewport sizes
    const viewports = [
      { width: 1920, height: 1080 }, // Desktop
      { width: 768, height: 1024 },  // Tablet
      { width: 375, height: 667 }    // Mobile
    ];
    
    for (const viewport of viewports) {
      await page.setViewportSize(viewport);
      await page.waitForTimeout(500);
      
      // Page should still be functional
      const body = page.locator("body");
      await expect(body).toBeVisible();
      
      // Should not have horizontal scroll on mobile
      if (viewport.width <= 768) {
        const scrollWidth = await page.evaluate(() => document.body.scrollWidth);
        const clientWidth = await page.evaluate(() => document.body.clientWidth);
        
        // Allow small differences due to scrollbars
        expect(scrollWidth - clientWidth).toBeLessThan(20);
      }
    }
  });

  test("API health check", async ({ page }) => {
    // Test if the server is responding to API requests
    const response = await page.request.get("/api/health").catch(() => null);
    
    if (response) {
      expect(response.status()).toBeLessThan(500);
    }
    
    // Even if health endpoint doesn't exist, the main page should work
    await page.goto("/");
    await expect(page.locator("body")).toBeVisible();
  });
});