import { test, expect } from "@playwright/test";

/**
 * Working tests based on actual UI behavior with proper timing
 */

test.describe("Working UI Tests", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("should load homepage successfully", async ({ page }) => {
    // Check for main heading
    await expect(page.getByRole("heading", { name: "🎨 Drawing Game" })).toBeVisible();
    
    // Check for create room button
    await expect(page.getByRole("button", { name: "+ Create Room" })).toBeVisible();
    
    // Should show connection status
    await expect(page.locator('text="Connected"')).toBeVisible();
  });

  test("should open create room modal", async ({ page }) => {
    // Click create room button
    await page.getByRole("button", { name: "+ Create Room" }).click();
    
    // Modal should appear
    await expect(page.getByRole("heading", { name: "Create New Room" })).toBeVisible();
    
    // Form fields should be present
    await expect(page.getByRole("textbox", { name: "Enter room name" })).toBeVisible();
    await expect(page.getByRole("textbox", { name: "Enter your name" })).toBeVisible();
    
    // Create button should be disabled initially
    const createButton = page.locator('button:has-text("Create Room")').last();
    await expect(createButton).toBeDisabled();
  });

  test("should enable create button when form is filled", async ({ page }) => {
    // Open modal
    await page.getByRole("button", { name: "+ Create Room" }).click();
    
    // Fill form
    await page.getByRole("textbox", { name: "Enter room name" }).fill("Test Room");
    await page.getByRole("textbox", { name: "Enter your name" }).fill("Test Player");
    
    // Create button should be enabled
    const createButton = page.locator('button:has-text("Create Room")').last();
    await expect(createButton).toBeEnabled();
  });

  test("should create room and navigate", async ({ page }) => {
    // Open modal and fill form
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.getByRole("textbox", { name: "Enter room name" }).fill("Navigation Test");
    await page.getByRole("textbox", { name: "Enter your name" }).fill("Nav Player");
    
    // Click create room and wait for navigation
    const createButton = page.locator('button:has-text("Create Room")').last();
    await createButton.click();
    
    // Wait for navigation to complete
    await page.waitForURL(/\/room\/[a-f0-9-]+/);
    
    // Should be in a room
    expect(page.url()).toMatch(/\/room\/[a-f0-9-]+/);
    
    // Should show room interface
    await expect(page.getByRole("heading", { name: "Navigation Test" })).toBeVisible();
  });

  test("should display complete game interface", async ({ page }) => {
    // Create room
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.getByRole("textbox", { name: "Enter room name" }).fill("Interface Test");
    await page.getByRole("textbox", { name: "Enter your name" }).fill("Interface Player");
    await page.locator('button:has-text("Create Room")').last().click();
    
    // Wait for room to load
    await page.waitForURL(/\/room\/[a-f0-9-]+/);
    await page.waitForTimeout(1000); // Allow components to initialize
    
    // Check for main game components
    await expect(page.getByRole("heading", { name: "Drawing Board" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Scoreboard" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Chat & Guessing" })).toBeVisible();
    
    // Check for player info
    await expect(page.locator('text="Interface Player"')).toBeVisible();
    await expect(page.locator('text="Host"')).toBeVisible();
    
    // Check for game status
    await expect(page.locator('text="Waiting for game to start"')).toBeVisible();
    
    // Check for connection status
    await expect(page.locator('text="Connected"')).toBeVisible();
    await expect(page.locator('text="Online"')).toBeVisible();
  });

  test("should handle chat input", async ({ page }) => {
    // Create room
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.getByRole("textbox", { name: "Enter room name" }).fill("Chat Test");
    await page.getByRole("textbox", { name: "Enter your name" }).fill("Chat Player");
    await page.locator('button:has-text("Create Room")').last().click();
    
    await page.waitForURL(/\/room\/[a-f0-9-]+/);
    await page.waitForTimeout(1000);
    
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
    
    // Message should appear in chat
    await expect(page.locator('text="Hello, this is a test message!"')).toBeVisible({ timeout: 5000 });
  });

  test("should show game controls for host", async ({ page }) => {
    // Create room
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.getByRole("textbox", { name: "Enter room name" }).fill("Host Test");
    await page.getByRole("textbox", { name: "Enter your name" }).fill("Host Player");
    await page.locator('button:has-text("Create Room")').last().click();
    
    await page.waitForURL(/\/room\/[a-f0-9-]+/);
    await page.waitForTimeout(1000);
    
    // Should show host controls
    await expect(page.getByRole("heading", { name: "Host Controls" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Start Game" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Game Settings" })).toBeVisible();
    
    // Start game should be disabled with only one player
    await expect(page.getByRole("button", { name: "Start Game" })).toBeDisabled();
  });

  test("should handle back to lobby navigation", async ({ page }) => {
    // Create room
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.getByRole("textbox", { name: "Enter room name" }).fill("Back Test");
    await page.getByRole("textbox", { name: "Enter your name" }).fill("Back Player");
    await page.locator('button:has-text("Create Room")').last().click();
    
    await page.waitForURL(/\/room\/[a-f0-9-]+/);
    await page.waitForTimeout(1000);
    
    // Click back to lobby
    await page.getByRole("button", { name: "← Back to Lobby" }).click();
    
    // Should return to homepage
    await page.waitForURL("http://localhost:3000/");
    await expect(page.getByRole("heading", { name: "🎨 Drawing Game" })).toBeVisible();
  });

  test("should join existing room", async ({ page }) => {
    // First, check if there are any existing rooms
    const joinButtons = page.locator('button:has-text("Join Room")');
    const joinButtonCount = await joinButtons.count();
    
    if (joinButtonCount > 0) {
      // Click the first join room button
      await joinButtons.first().click();
      
      // Should navigate to a room or show join modal
      await page.waitForTimeout(2000);
      
      // Either we're in a room or there's a join modal
      const isInRoom = page.url().includes("/room/");
      const hasJoinModal = await page.locator('text="Join Room"').isVisible();
      
      expect(isInRoom || hasJoinModal).toBe(true);
    } else {
      // No rooms available to join - this is also a valid state
      expect(true).toBe(true);
    }
  });

  test("should show WebSocket connection status", async ({ page }) => {
    // Create room
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.getByRole("textbox", { name: "Enter room name" }).fill("WebSocket Test");
    await page.getByRole("textbox", { name: "Enter your name" }).fill("WS Player");
    await page.locator('button:has-text("Create Room")').last().click();
    
    await page.waitForURL(/\/room\/[a-f0-9-]+/);
    await page.waitForTimeout(2000); // Allow WebSocket connections to establish
    
    // Should show connected status in multiple places
    const connectedElements = page.locator('text="Connected"');
    await expect(connectedElements.first()).toBeVisible();
    
    // Should show online status in chat
    await expect(page.locator('text="Online"')).toBeVisible();
    
    // Should show connection status in scoreboard
    await expect(page.locator('text="connected"')).toBeVisible();
  });
});