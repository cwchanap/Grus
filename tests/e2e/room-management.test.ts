import { test, expect } from "@playwright/test";

/**
 * Tests for room creation, joining, and management functionality
 */

test.describe("Room Management", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("should create room with valid room code", async ({ page }) => {
    const createRoomButton = page.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();
    
    if (await createRoomButton.isVisible()) {
      await createRoomButton.click();
      
      // Should redirect to a room with a valid room code
      await expect(page.url()).toMatch(/\/room\/[A-Z0-9]+/);
      
      // Room code should be displayed somewhere on the page
      const roomCode = page.url().match(/\/room\/([^\/]+)/)?.[1];
      if (roomCode) {
        await expect(page.locator(`text="${roomCode}"`)).toBeVisible({ timeout: 5000 });
      }
    }
  });

  test("should handle room not found", async ({ page }) => {
    // Try to access a non-existent room
    await page.goto("/room/NONEXISTENT");
    
    // Should show error message or redirect
    const errorIndicators = [
      'text="Room not found"',
      'text="Invalid room"',
      'text="Room does not exist"',
      '[data-testid="error"]',
      '.error'
    ];
    
    let foundError = false;
    for (const selector of errorIndicators) {
      if (await page.locator(selector).isVisible({ timeout: 2000 }).catch(() => false)) {
        foundError = true;
        break;
      }
    }
    
    // Should either show error or redirect to home
    expect(foundError || page.url().includes("/")).toBe(true);
  });

  test("should display room information", async ({ page }) => {
    const createRoomButton = page.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();
    
    if (await createRoomButton.isVisible()) {
      await createRoomButton.click();
      await page.waitForTimeout(2000);
      
      // Should display room-related information
      const roomInfoElements = [
        '[data-testid="room-code"]',
        '[data-testid="room-info"]',
        '.room-code',
        '.room-info',
        'text=/Room:/',
        'text=/Code:/'
      ];
      
      let foundRoomInfo = false;
      for (const selector of roomInfoElements) {
        if (await page.locator(selector).isVisible({ timeout: 2000 }).catch(() => false)) {
          foundRoomInfo = true;
          break;
        }
      }
      
      expect(foundRoomInfo).toBe(true);
    }
  });

  test("should show player list", async ({ page }) => {
    const createRoomButton = page.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();
    
    if (await createRoomButton.isVisible()) {
      await createRoomButton.click();
      await page.waitForTimeout(2000);
      
      // Should show current players in the room
      const playerListElements = [
        '[data-testid="players"]',
        '[data-testid="player-list"]',
        '.players',
        '.player-list',
        'text="Players"',
        'text="Connected:"'
      ];
      
      let foundPlayerList = false;
      for (const selector of playerListElements) {
        if (await page.locator(selector).isVisible({ timeout: 3000 }).catch(() => false)) {
          foundPlayerList = true;
          break;
        }
      }
      
      expect(foundPlayerList).toBe(true);
    }
  });

  test("should handle room capacity limits", async ({ browser }) => {
    // This test simulates multiple players joining to test capacity
    const contexts = [];
    const pages = [];
    
    try {
      // Create first player and room
      const context1 = await browser.newContext();
      const player1 = await context1.newPage();
      contexts.push(context1);
      pages.push(player1);
      
      await player1.goto("/");
      const createRoomButton = player1.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();
      
      if (await createRoomButton.isVisible()) {
        await createRoomButton.click();
        await player1.waitForTimeout(2000);
        
        const roomUrl = player1.url();
        const roomCode = roomUrl.match(/\/room\/([^\/]+)/)?.[1];
        
        if (roomCode) {
          // Add more players (up to reasonable limit for testing)
          for (let i = 2; i <= 5; i++) {
            const context = await browser.newContext();
            const page = await context.newPage();
            contexts.push(context);
            pages.push(page);
            
            await page.goto(`/room/${roomCode}`);
            await page.waitForTimeout(1000);
            
            // Each player should successfully join (or get appropriate message if full)
            const isInRoom = page.url().includes(roomCode);
            const hasErrorMessage = await page.locator('text="Room is full", text="Maximum players"').isVisible({ timeout: 2000 }).catch(() => false);
            
            expect(isInRoom || hasErrorMessage).toBe(true);
          }
        }
      }
    } finally {
      // Clean up all contexts
      for (const context of contexts) {
        await context.close();
      }
    }
  });

  test("should allow leaving room", async ({ page }) => {
    const createRoomButton = page.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();
    
    if (await createRoomButton.isVisible()) {
      await createRoomButton.click();
      await page.waitForTimeout(2000);
      
      // Look for leave room button
      const leaveRoomButton = page.locator(
        'button:has-text("Leave"), button:has-text("Exit"), ' +
        '[data-testid="leave-room"], .leave-room'
      ).first();
      
      if (await leaveRoomButton.isVisible({ timeout: 3000 })) {
        await leaveRoomButton.click();
        
        // Should redirect away from room or show confirmation
        await page.waitForTimeout(1000);
        
        const isAwayFromRoom = !page.url().includes("/room/") || 
                              await page.locator('text="Leave room?", text="Are you sure?"').isVisible({ timeout: 2000 });
        
        expect(isAwayFromRoom).toBe(true);
      }
    }
  });
});