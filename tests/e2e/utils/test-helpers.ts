import { Page, expect } from "@playwright/test";

/**
 * Utility functions for E2E tests
 */

export class GameTestHelpers {
  constructor(private page: Page) {}

  /**
   * Navigate to a room (create new or join existing)
   */
  async navigateToRoom(roomCode?: string): Promise<string | null> {
    if (roomCode) {
      await this.page.goto(`/room/${roomCode}`);
      await this.page.waitForTimeout(2000);
      return roomCode;
    }

    // Create new room
    await this.page.goto("/");
    const createRoomButton = this.page.locator(
      'button:has-text("Create Room"), a:has-text("Create Room")'
    ).first();
    
    if (await createRoomButton.isVisible()) {
      await createRoomButton.click();
      await this.page.waitForTimeout(2000);
      
      const roomUrl = this.page.url();
      const extractedRoomCode = roomUrl.match(/\/room\/([^\/]+)/)?.[1];
      return extractedRoomCode || null;
    }
    
    return null;
  }

  /**
   * Wait for game elements to load
   */
  async waitForGameElements(): Promise<boolean> {
    const gameElements = [
      'canvas',
      '[data-testid="drawing-board"]',
      '.drawing-canvas',
      '[data-testid="chat"]',
      '.chat'
    ];

    for (const selector of gameElements) {
      if (await this.page.locator(selector).isVisible({ timeout: 5000 }).catch(() => false)) {
        return true;
      }
    }
    
    return false;
  }

  /**
   * Send a chat message
   */
  async sendChatMessage(message: string): Promise<boolean> {
    const chatInput = this.page.locator(
      'input[placeholder*="guess"], input[placeholder*="chat"], input[placeholder*="message"], ' +
      '[data-testid="chat-input"] input, .chat input[type="text"]'
    ).first();

    if (await chatInput.isVisible({ timeout: 3000 })) {
      await chatInput.fill(message);
      await chatInput.press("Enter");
      return true;
    }
    
    return false;
  }

  /**
   * Draw on canvas
   */
  async drawOnCanvas(startX: number, startY: number, endX: number, endY: number): Promise<boolean> {
    const canvas = this.page.locator('canvas').first();
    
    if (await canvas.isVisible({ timeout: 5000 })) {
      const canvasBox = await canvas.boundingBox();
      
      if (canvasBox) {
        const actualStartX = canvasBox.x + startX;
        const actualStartY = canvasBox.y + startY;
        const actualEndX = canvasBox.x + endX;
        const actualEndY = canvasBox.y + endY;
        
        await this.page.mouse.move(actualStartX, actualStartY);
        await this.page.mouse.down();
        await this.page.mouse.move(actualEndX, actualEndY);
        await this.page.mouse.up();
        
        return true;
      }
    }
    
    return false;
  }

  /**
   * Check if player is in a room
   */
  async isInRoom(): Promise<boolean> {
    return this.page.url().includes("/room/");
  }

  /**
   * Get current room code from URL
   */
  getRoomCodeFromUrl(): string | null {
    const roomUrl = this.page.url();
    return roomUrl.match(/\/room\/([^\/]+)/)?.[1] || null;
  }

  /**
   * Wait for specific text to appear
   */
  async waitForText(text: string, timeout = 5000): Promise<boolean> {
    try {
      await expect(this.page.locator(`text="${text}"`)).toBeVisible({ timeout });
      return true;
    } catch {
      return false;
    }
  }

  /**
   * Check if WebSocket connection is established
   */
  async hasWebSocketConnection(): Promise<boolean> {
    return new Promise((resolve) => {
      let hasConnection = false;
      
      const timeout = setTimeout(() => {
        resolve(hasConnection);
      }, 5000);
      
      this.page.on("websocket", () => {
        hasConnection = true;
        clearTimeout(timeout);
        resolve(true);
      });
    });
  }

  /**
   * Get player count from UI
   */
  async getPlayerCount(): Promise<number> {
    const playerElements = this.page.locator(
      '[data-testid="players"] > *, .players > *, .player-list > *'
    );
    
    try {
      return await playerElements.count();
    } catch {
      return 0;
    }
  }

  /**
   * Check if drawing tools are available
   */
  async hasDrawingTools(): Promise<boolean> {
    const toolElements = [
      '[data-testid="drawing-tools"]',
      '.drawing-tools',
      'button:has-text("Brush")',
      'input[type="color"]',
      '[data-testid="color-picker"]'
    ];

    for (const selector of toolElements) {
      if (await this.page.locator(selector).isVisible({ timeout: 2000 }).catch(() => false)) {
        return true;
      }
    }
    
    return false;
  }

  /**
   * Clear canvas if clear button exists
   */
  async clearCanvas(): Promise<boolean> {
    const clearButton = this.page.locator(
      'button:has-text("Clear"), button:has-text("Reset"), ' +
      '[data-testid="clear-canvas"], .clear-canvas'
    ).first();

    if (await clearButton.isVisible({ timeout: 3000 })) {
      await clearButton.click();
      return true;
    }
    
    return false;
  }
}

/**
 * Create multiple browser contexts for multiplayer testing
 */
export async function createMultiplePlayersInRoom(
  browser: any,
  playerCount: number,
  roomCode?: string
): Promise<{ contexts: any[], pages: Page[], roomCode: string | null }> {
  const contexts = [];
  const pages = [];
  let actualRoomCode = roomCode;

  try {
    // Create first player and room if needed
    const context1 = await browser.newContext();
    const player1 = await context1.newPage();
    contexts.push(context1);
    pages.push(player1);

    const helper1 = new GameTestHelpers(player1);
    actualRoomCode = await helper1.navigateToRoom(roomCode);

    if (!actualRoomCode) {
      throw new Error("Failed to create or join room");
    }

    // Add additional players
    for (let i = 1; i < playerCount; i++) {
      const context = await browser.newContext();
      const page = await context.newPage();
      contexts.push(context);
      pages.push(page);

      const helper = new GameTestHelpers(page);
      await helper.navigateToRoom(actualRoomCode);
    }

    return { contexts, pages, roomCode: actualRoomCode };
  } catch (error) {
    // Clean up on error
    for (const context of contexts) {
      await context.close();
    }
    throw error;
  }
}

/**
 * Clean up multiple browser contexts
 */
export async function cleanupMultiplePlayers(contexts: any[]): Promise<void> {
  for (const context of contexts) {
    try {
      await context.close();
    } catch (error) {
      console.warn("Error closing context:", error);
    }
  }
}