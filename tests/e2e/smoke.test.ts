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
    
    // Look for any way to create or join a room
    const navigationElements = [
      'button:has-text("Create Room")',
      'a:has-text("Create Room")',
      'button:has-text("Join Room")',
      'a:has-text("Join Room")',
      'button:has-text("Play")',
      'a:has-text("Play")'
    ];
    
    let foundNavigation = false;
    for (const selector of navigationElements) {
      const element = page.locator(selector).first();
      if (await element.isVisible({ timeout: 2000 })) {
        await element.click();
        foundNavigation = true;
        break;
      }
    }
    
    expect(foundNavigation).toBe(true);
    
    // Should navigate somewhere (not stay on homepage)
    await page.waitForTimeout(1000);
    const currentUrl = page.url();
    expect(currentUrl).not.toBe("http://localhost:8000/");
  });

  test("room page loads basic elements", async ({ page }) => {
    await page.goto("/");
    
    // Navigate to a room
    const createRoomButton = page.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();
    if (await createRoomButton.isVisible({ timeout: 3000 })) {
      await createRoomButton.click();
      await page.waitForTimeout(3000);
      
      // Should be in a room
      expect(page.url()).toMatch(/\/room\/|\/game\/|\/play\//);
      
      // Should have at least one of the essential game elements
      const essentialElements = [
        'canvas',
        '[data-testid="drawing-board"]',
        '[data-testid="chat"]',
        '[data-testid="game"]',
        '.game-container',
        '.drawing-area'
      ];
      
      let foundEssential = false;
      for (const selector of essentialElements) {
        if (await page.locator(selector).isVisible({ timeout: 5000 }).catch(() => false)) {
          foundEssential = true;
          break;
        }
      }
      
      expect(foundEssential).toBe(true);
    }
  });

  test("no critical JavaScript errors", async ({ page }) => {
    const jsErrors: string[] = [];
    
    page.on("pageerror", (error) => {
      jsErrors.push(error.message);
    });
    
    await page.goto("/");
    
    // Navigate through the app
    const createRoomButton = page.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();
    if (await createRoomButton.isVisible({ timeout: 3000 })) {
      await createRoomButton.click();
      await page.waitForTimeout(2000);
    }
    
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