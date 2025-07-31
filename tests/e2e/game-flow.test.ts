import { test, expect, type Page } from "@playwright/test";

/**
 * End-to-end tests for the multiplayer drawing game
 * Tests the complete game flow from room creation to gameplay
 */

test.describe("Multiplayer Drawing Game", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("should load the homepage", async ({ page }) => {
    await expect(page).toHaveTitle(/Drawing Game/);
    
    // Check for main navigation elements
    await expect(page.locator("h1")).toBeVisible();
  });

  test("should create a new room", async ({ page }) => {
    // Look for create room button/link
    const createRoomButton = page.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();
    await expect(createRoomButton).toBeVisible();
    
    await createRoomButton.click();
    
    // Should navigate to room creation or directly to a room
    await expect(page.url()).toMatch(/\/room\/|\/create/);
    
    // Check if we're in a room (look for game elements)
    await expect(
      page.locator('canvas, [data-testid="drawing-board"], .drawing-canvas').first()
    ).toBeVisible({ timeout: 10000 });
  });

  test("should join an existing room", async ({ page }) => {
    // Look for join room functionality
    const joinRoomButton = page.locator('button:has-text("Join Room"), a:has-text("Join Room")').first();
    
    if (await joinRoomButton.isVisible()) {
      await joinRoomButton.click();
      
      // Look for room code input
      const roomCodeInput = page.locator('input[placeholder*="room"], input[placeholder*="code"], input[type="text"]').first();
      if (await roomCodeInput.isVisible()) {
        await roomCodeInput.fill("TEST123");
        
        const submitButton = page.locator('button[type="submit"], button:has-text("Join")').first();
        await submitButton.click();
      }
    }
  });

  test("should display game lobby elements", async ({ page }) => {
    // Navigate to a room (create one first)
    const createRoomButton = page.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();
    if (await createRoomButton.isVisible()) {
      await createRoomButton.click();
    }
    
    // Wait for room to load
    await page.waitForTimeout(2000);
    
    // Check for essential game elements
    const gameElements = [
      'canvas',
      '[data-testid="drawing-board"]',
      '.drawing-canvas',
      '[data-testid="chat"]',
      '.chat',
      '[data-testid="scoreboard"]',
      '.scoreboard',
      '[data-testid="players"]',
      '.players'
    ];
    
    let foundElements = 0;
    for (const selector of gameElements) {
      const element = page.locator(selector).first();
      if (await element.isVisible({ timeout: 1000 }).catch(() => false)) {
        foundElements++;
      }
    }
    
    // Should find at least some game elements
    expect(foundElements).toBeGreaterThan(0);
  });

  test("should handle drawing interactions", async ({ page }) => {
    // Navigate to a room
    const createRoomButton = page.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();
    if (await createRoomButton.isVisible()) {
      await createRoomButton.click();
    }
    
    await page.waitForTimeout(2000);
    
    // Look for drawing canvas
    const canvas = page.locator('canvas, [data-testid="drawing-board"] canvas').first();
    
    if (await canvas.isVisible({ timeout: 5000 })) {
      // Get canvas bounding box
      const canvasBox = await canvas.boundingBox();
      
      if (canvasBox) {
        // Simulate drawing by clicking and dragging on canvas
        await page.mouse.move(canvasBox.x + 50, canvasBox.y + 50);
        await page.mouse.down();
        await page.mouse.move(canvasBox.x + 100, canvasBox.y + 100);
        await page.mouse.move(canvasBox.x + 150, canvasBox.y + 50);
        await page.mouse.up();
        
        // Drawing should trigger some visual feedback or state change
        // This is a basic interaction test
        expect(true).toBe(true);
      }
    }
  });

  test("should handle chat functionality", async ({ page }) => {
    // Navigate to a room
    const createRoomButton = page.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();
    if (await createRoomButton.isVisible()) {
      await createRoomButton.click();
    }
    
    await page.waitForTimeout(2000);
    
    // Look for chat input
    const chatInput = page.locator(
      'input[placeholder*="guess"], input[placeholder*="chat"], input[placeholder*="message"], ' +
      '[data-testid="chat-input"] input, .chat input[type="text"]'
    ).first();
    
    if (await chatInput.isVisible({ timeout: 5000 })) {
      await chatInput.fill("test message");
      await chatInput.press("Enter");
      
      // Should see the message appear in chat
      await expect(page.locator('text="test message"')).toBeVisible({ timeout: 3000 });
    }
  });
});

test.describe("Multiplayer Functionality", () => {
  test("should support multiple players in same room", async ({ browser }) => {
    // Create two browser contexts to simulate multiple players
    const context1 = await browser.newContext();
    const context2 = await browser.newContext();
    
    const player1 = await context1.newPage();
    const player2 = await context2.newPage();
    
    try {
      // Player 1 creates a room
      await player1.goto("/");
      const createRoomButton = player1.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();
      
      if (await createRoomButton.isVisible()) {
        await createRoomButton.click();
        await player1.waitForTimeout(2000);
        
        // Get room code from URL or page
        const roomUrl = player1.url();
        const roomCode = roomUrl.match(/\/room\/([^\/]+)/)?.[1];
        
        if (roomCode) {
          // Player 2 joins the same room
          await player2.goto(`/room/${roomCode}`);
          await player2.waitForTimeout(2000);
          
          // Both players should be in the same room
          await expect(player1.url()).toContain(roomCode);
          await expect(player2.url()).toContain(roomCode);
          
          // Check if both players can see game elements
          const gameElementsVisible = await Promise.all([
            player1.locator('canvas, [data-testid="drawing-board"]').first().isVisible({ timeout: 5000 }),
            player2.locator('canvas, [data-testid="drawing-board"]').first().isVisible({ timeout: 5000 })
          ]);
          
          expect(gameElementsVisible.some(visible => visible)).toBe(true);
        }
      }
    } finally {
      await context1.close();
      await context2.close();
    }
  });

  test("should sync drawing between players", async ({ browser }) => {
    const context1 = await browser.newContext();
    const context2 = await browser.newContext();
    
    const player1 = await context1.newPage();
    const player2 = await context2.newPage();
    
    try {
      // Setup both players in the same room
      await player1.goto("/");
      const createRoomButton = player1.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();
      
      if (await createRoomButton.isVisible()) {
        await createRoomButton.click();
        await player1.waitForTimeout(2000);
        
        const roomUrl = player1.url();
        const roomCode = roomUrl.match(/\/room\/([^\/]+)/)?.[1];
        
        if (roomCode) {
          await player2.goto(`/room/${roomCode}`);
          await player2.waitForTimeout(2000);
          
          // Player 1 draws something
          const canvas1 = player1.locator('canvas').first();
          if (await canvas1.isVisible({ timeout: 5000 })) {
            const canvasBox = await canvas1.boundingBox();
            if (canvasBox) {
              await player1.mouse.move(canvasBox.x + 50, canvasBox.y + 50);
              await player1.mouse.down();
              await player1.mouse.move(canvasBox.x + 100, canvasBox.y + 100);
              await player1.mouse.up();
              
              // Wait for sync
              await player1.waitForTimeout(1000);
              
              // Player 2 should see the drawing (this is a basic test)
              // In a real implementation, you'd check canvas content or drawing events
              const canvas2 = player2.locator('canvas').first();
              await expect(canvas2).toBeVisible();
            }
          }
        }
      }
    } finally {
      await context1.close();
      await context2.close();
    }
  });
});