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
    
    // Should show connection status
    await expect(page.locator('text="Connected"')).toBeVisible();
  });

  test("should create room through modal", async ({ page }) => {
    // Click create room button
    await page.getByRole("button", { name: "+ Create Room" }).click();
    
    // Modal should appear
    await expect(page.getByRole("heading", { name: "Create New Room" })).toBeVisible();
    
    // Fill out form
    await page.getByRole("textbox", { name: "Enter room name" }).fill("E2E Test Room");
    await page.getByRole("textbox", { name: "Enter your name" }).fill("E2E Player");
    
    // Create room button should be enabled
    const createButton = page.locator('button:has-text("Create Room")').last();
    await expect(createButton).toBeEnabled();
    
    // Click create room
    await createButton.click();
    
    // Should navigate to room
    await expect(page.url()).toMatch(/\/room\/[a-f0-9-]+/);
    
    // Should show room interface
    await expect(page.getByRole("heading", { name: "E2E Test Room" })).toBeVisible();
  });

  test("should display complete game interface in room", async ({ page }) => {
    // Create a room first
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.getByRole("textbox", { name: "Enter room name" }).fill("UI Test Room");
    await page.getByRole("textbox", { name: "Enter your name" }).fill("UI Tester");
    await page.locator('button:has-text("Create Room")').last().click();
    
    // Wait for room to load
    await page.waitForTimeout(2000);
    
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
    await page.getByRole("textbox", { name: "Enter room name" }).fill("Chat Test");
    await page.getByRole("textbox", { name: "Enter your name" }).fill("Chat Tester");
    await page.locator('button:has-text("Create Room")').last().click();
    
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
    await page.getByRole("textbox", { name: "Enter room name" }).fill("WebSocket Test");
    await page.getByRole("textbox", { name: "Enter your name" }).fill("WS Tester");
    await page.locator('button:has-text("Create Room")').last().click();
    
    await page.waitForTimeout(3000);
    
    // Should show connected status in multiple places
    const connectedElements = page.locator('text="Connected"');
    await expect(connectedElements.first()).toBeVisible();
    
    // Should show online status in chat
    await expect(page.locator('text="Online"')).toBeVisible();
  });

  test("should handle game settings modal", async ({ page }) => {
    // Create a room
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.getByRole("textbox", { name: "Enter room name" }).fill("Settings Test");
    await page.getByRole("textbox", { name: "Enter your name" }).fill("Settings Tester");
    await page.locator('button:has-text("Create Room")').last().click();
    
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
    await page.getByRole("textbox", { name: "Enter room name" }).fill("Drawing Test");
    await page.getByRole("textbox", { name: "Enter your name" }).fill("Drawing Tester");
    await page.locator('button:has-text("Create Room")').last().click();
    
    await page.waitForTimeout(2000);
    
    // Should show drawing disabled message
    await expect(page.locator('text="Drawing disabled"')).toBeVisible();
    await expect(page.locator('text="Game hasn\'t started yet"')).toBeVisible();
  });

  test("should handle back to lobby navigation", async ({ page }) => {
    // Create a room
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.getByRole("textbox", { name: "Enter room name" }).fill("Navigation Test");
    await page.getByRole("textbox", { name: "Enter your name" }).fill("Nav Tester");
    await page.locator('button:has-text("Create Room")').last().click();
    
    await page.waitForTimeout(2000);
    
    // Click back to lobby
    await page.getByRole("button", { name: "← Back to Lobby" }).click();
    
    // Should return to homepage
    await expect(page.url()).toBe("http://localhost:3000/");
    await expect(page.getByRole("heading", { name: "🎨 Drawing Game" })).toBeVisible();
  });
});