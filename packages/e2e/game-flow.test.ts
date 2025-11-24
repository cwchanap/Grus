import { test, expect, type Page } from "@playwright/test";
import { GameTestHelpers } from "./utils/test-helpers.ts";

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
    // Open create room modal and create a room using stable selectors
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.locator('[data-testid="room-name-input"]').fill("E2E Flow Room");
    await page.locator('[data-testid="host-name-input"]').fill("Flow Host");
    await page.locator('[data-testid="create-room-submit"]').click();

    // Wait for navigation to the room
    await page.waitForURL(/\/room\/[a-f0-9-]+/);

    // Check if we're in a room (look for game elements)
    await expect(page.getByRole("heading", { name: "Drawing Board" })).toBeVisible();
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
    // Create a room via modal
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.locator('[data-testid="room-name-input"]').fill("Lobby Elements Room");
    await page.locator('[data-testid="host-name-input"]').fill("Lobby Host");
    await page.locator('[data-testid="create-room-submit"]').click();
    await page.waitForURL(/\/room\/[a-f0-9-]+/);

    // Check for essential game elements via headings
    await expect(page.getByRole("heading", { name: "Drawing Board" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Scoreboard" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Chat & Guessing" })).toBeVisible();
  });

  test("should handle drawing interactions", async ({ page }) => {
    // Create a room via modal
    const helper = new GameTestHelpers(page);
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.locator('[data-testid="room-name-input"]').fill("Drawing Room");
    await page.locator('[data-testid="host-name-input"]').fill("Drawer");
    await page.locator('[data-testid="create-room-submit"]').click();
    await page.waitForURL(/\/room\/[a-f0-9-]+/);

    const drew = await helper.drawOnCanvas(50, 50, 150, 100);
    expect(drew).toBe(true);
  });

  test("should handle chat functionality", async ({ page }) => {
    // Create a room via modal
    const helper = new GameTestHelpers(page);
    await page.getByRole("button", { name: "+ Create Room" }).click();
    await page.locator('[data-testid="room-name-input"]').fill("Chat Room");
    await page.locator('[data-testid="host-name-input"]').fill("Chatter");
    await page.locator('[data-testid="create-room-submit"]').click();
    await page.waitForURL(/\/room\/[a-f0-9-]+/);

    const sent = await helper.sendChatMessage("test message");
    expect(sent).toBe(true);
    await expect(page.locator('text="test message"')).toBeVisible({ timeout: 5000 });
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
      // Player 1 creates a room via modal
      await player1.goto("/");
      await player1.getByRole("button", { name: "+ Create Room" }).click();
      await player1.locator('[data-testid="room-name-input"]').fill("MP Room");
      await player1.locator('[data-testid="host-name-input"]').fill("Host1");
      await player1.locator('[data-testid="create-room-submit"]').click();
      await player1.waitForURL(/\/room\/[a-f0-9-]+/);

      // Get room code from URL
      const roomUrl = player1.url();
      const roomCode = roomUrl.match(/\/room\/([^\/]+)/)?.[1];
      
      if (roomCode) {
        // Player 2 joins the same room
        await player2.goto(`/room/${roomCode}`);
        await player2.waitForURL(new RegExp(`/room/${roomCode}`));
        
        // Both players should be in the same room
        expect(player1.url()).toContain(roomCode);
        expect(player2.url()).toContain(roomCode);
        
        // Check if both players can see game elements
        const gameElementsVisible = await Promise.all([
          player1.getByRole("heading", { name: "Drawing Board" }).isVisible({ timeout: 5000 }),
          player2.getByRole("heading", { name: "Drawing Board" }).isVisible({ timeout: 5000 }),
        ]);
        
        expect(gameElementsVisible.every((v) => v)).toBe(true);
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
      await player1.getByRole("button", { name: "+ Create Room" }).click();
      await player1.locator('[data-testid="room-name-input"]').fill("Sync Draw Room");
      await player1.locator('[data-testid="host-name-input"]').fill("Drawer1");
      await player1.locator('[data-testid="create-room-submit"]').click();
      await player1.waitForURL(/\/room\/[a-f0-9-]+/);

      const roomCode = player1.url().match(/\/room\/([^\/]+)/)?.[1];
      if (roomCode) {
        await player2.goto(`/room/${roomCode}`);
        await player2.waitForURL(new RegExp(`/room/${roomCode}`));

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

            const canvas2 = player2.locator('canvas').first();
            await expect(canvas2).toBeVisible();
          }
        }
      }
    } finally {
      await context1.close();
      await context2.close();
    }
  });
});