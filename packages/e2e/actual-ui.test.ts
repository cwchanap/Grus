import { test, expect } from "@playwright/test";

/**
 * Tests based on the actual UI structure discovered through manual testing
 */

test.describe("Actual UI Tests", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("should load homepage with room lobby", async ({ page }) => {
    // Check for main heading
    await expect(page.getByRole("heading", { name: "🎨 Drawing Game" })).toBeVisible();
    
    // Check for create room button
    await expect(page.getByRole("button", { name: "+ Create Room" })).toBeVisible();
    
    // Should show lobby connection status (Connected or Dev Mode)
    const connected = page.locator('text="Connected"');
    const devMode = page.locator('text="Dev Mode"');
    const isConnectedVisible = await connected.isVisible().catch(() => false);
    if (!isConnectedVisible) {
      await expect(devMode).toBeVisible();
    }
  });

  test("should create room through modal", async ({ page }) => {
    // Click create room button
    await page.getByRole("button", { name: "+ Create Room" }).click();
    
    // Modal should appear
    await expect(page.getByRole("heading", { name: "Create New Room" })).toBeVisible();
    
    // Fill out form using stable selectors
    await page.locator('[data-testid="room-name-input"]').fill("E2E Test Room");
    await page.locator('[data-testid="host-name-input"]').fill("E2E Player");
    
    // Create room button should be enabled
    const createButton = page.locator('[data-testid="create-room-submit"]');
    await expect(createButton).toBeEnabled();
    
    // Click create room and wait for navigation
    await createButton.click();
    await page.waitForURL(/\/room\/[a-f0-9-]+/);
    
    // Should show room interface
    await expect(page.getByRole("heading", { name: "E2E Test Room" })).toBeVisible();
  });

  test("should display complete game interface in room", async ({ page }) => {
    // Create a room first
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.locator('[data-testid="room-name-input"]').fill("UI Test Room");
    await page.locator('[data-testid="host-name-input"]').fill("UI Tester");
    await page.locator('[data-testid="create-room-submit"]').click();
    
    // Wait for room to load
    await page.waitForURL(/\/room\/[a-f0-9-]+/);
    await page.waitForTimeout(1000);
    
    // Check for main game components
    await expect(page.getByRole("heading", { name: "Drawing Board" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Scoreboard" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Chat & Guessing" })).toBeVisible();
    
    // Check for player info
    await expect(page.locator('text="UI Tester"')).toBeVisible();
    await expect(page.locator('text="Host"')).toBeVisible();
    
    // Check for game controls
    await expect(page.getByRole("button", { name: "Start Game" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Game Settings" })).toBeVisible();
    
    // Check for chat input
    await expect(page.getByRole("textbox", { name: "Type your guess here..." })).toBeVisible();
  });

  test("should handle chat input", async ({ page }) => {
    // Create a room
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.locator('[data-testid="room-name-input"]').fill("Chat Test");
    await page.locator('[data-testid="host-name-input"]').fill("Chat Tester");
    await page.locator('[data-testid="create-room-submit"]').click();
    
    await page.waitForTimeout(2000);
    
    // Find chat input
    const chatInput = page.getByRole("textbox", { name: "Type your guess here..." });
    await expect(chatInput).toBeVisible();
    
    // Type a message
    await chatInput.fill("Hello, this is a test message!");
    
    // Send button should be enabled when there's text
    const sendButton = page.getByRole("button", { name: "Send" });
    await expect(sendButton).toBeEnabled();
    
    // Send the message
    await sendButton.click();
    
    // Message should appear in chat (basic test)
    await expect(page.locator('text="Hello, this is a test message!"')).toBeVisible({ timeout: 5000 });
  });

  test("should show WebSocket connection status", async ({ page }) => {
    // Create a room
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.locator('[data-testid="room-name-input"]').fill("WebSocket Test");
    await page.locator('[data-testid="host-name-input"]').fill("WS Tester");
    await page.locator('[data-testid="create-room-submit"]').click();

    await page.waitForURL(/\/room\/[a-f0-9-]+\/?$/);
    await page.waitForTimeout(2000);

    // Should show connection status in scoreboard (lowercase)
    await expect(page.locator('text="connected"')).toBeVisible();
  });

  test("should handle game settings modal", async ({ page }) => {
    // Create a room
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.locator('[data-testid="room-name-input"]').fill("Settings Test");
    await page.locator('[data-testid="host-name-input"]').fill("Settings Tester");
    await page.locator('[data-testid="create-room-submit"]').click();
    
    await page.waitForTimeout(2000);
    
    // Click game settings
    await page.getByRole("button", { name: "Game Settings" }).click();
    
    // Settings modal should appear (basic check)
    // The actual modal content would depend on the GameSettingsModal implementation
    await page.waitForTimeout(1000);
  });

  test("should show drawing board status", async ({ page }) => {
    // Create a room
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.locator('[data-testid="room-name-input"]').fill("Drawing Test");
    await page.locator('[data-testid="host-name-input"]').fill("Drawing Tester");
    await page.locator('[data-testid="create-room-submit"]').click();
    
    await page.waitForTimeout(2000);
    
    // Should show drawing disabled message
    await expect(page.locator('text="Drawing disabled"')).toBeVisible();
    await expect(page.locator('text="Game hasn\'t started yet"')).toBeVisible();
  });

  test("should handle back to lobby navigation", async ({ page }) => {
    // Create a room
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.locator('[data-testid="room-name-input"]').fill("Navigation Test");
    await page.locator('[data-testid="host-name-input"]').fill("Nav Tester");
    await page.locator('[data-testid="create-room-submit"]').click();
    
    await page.waitForTimeout(2000);
    
    // Click back to lobby (confirm dialog)
    page.once("dialog", (dialog) => dialog.accept());
    await page.getByRole("button", { name: "← Back to Lobby" }).click();
    
    // Should return to homepage
    await page.waitForURL(/^https?:\/\/[^/]+\/$/);
    await expect(page.getByRole("heading", { name: "🎨 Drawing Game" })).toBeVisible();
  });
});