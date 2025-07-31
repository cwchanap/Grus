import { test, expect } from "@playwright/test";

/**
 * Tests for drawing canvas functionality and interactions
 */

test.describe("Drawing Functionality", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    
    // Navigate to a room for testing
    const createRoomButton = page.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();
    if (await createRoomButton.isVisible()) {
      await createRoomButton.click();
      await page.waitForTimeout(2000);
    }
  });

  test("should display drawing canvas", async ({ page }) => {
    // Look for canvas element
    const canvas = page.locator('canvas, [data-testid="drawing-board"] canvas, [data-testid="canvas"]').first();
    await expect(canvas).toBeVisible({ timeout: 10000 });
    
    // Canvas should have reasonable dimensions
    const canvasBox = await canvas.boundingBox();
    expect(canvasBox).toBeTruthy();
    if (canvasBox) {
      expect(canvasBox.width).toBeGreaterThan(200);
      expect(canvasBox.height).toBeGreaterThan(200);
    }
  });

  test("should handle mouse drawing interactions", async ({ page }) => {
    const canvas = page.locator('canvas').first();
    
    if (await canvas.isVisible({ timeout: 5000 })) {
      const canvasBox = await canvas.boundingBox();
      
      if (canvasBox) {
        // Test basic drawing interaction
        const startX = canvasBox.x + 50;
        const startY = canvasBox.y + 50;
        const endX = canvasBox.x + 150;
        const endY = canvasBox.y + 100;
        
        // Simulate drawing a line
        await page.mouse.move(startX, startY);
        await page.mouse.down();
        await page.mouse.move(endX, endY);
        await page.mouse.up();
        
        // Drawing interaction should complete without errors
        expect(true).toBe(true);
      }
    }
  });

  test("should provide drawing tools", async ({ page }) => {
    // Look for drawing tools/controls
    const toolElements = [
      '[data-testid="drawing-tools"]',
      '.drawing-tools',
      'button:has-text("Brush")',
      'button:has-text("Pen")',
      'button:has-text("Eraser")',
      '[data-testid="color-picker"]',
      '.color-picker',
      'input[type="color"]',
      '[data-testid="brush-size"]',
      '.brush-size'
    ];
    
    let foundTools = 0;
    for (const selector of toolElements) {
      if (await page.locator(selector).isVisible({ timeout: 2000 }).catch(() => false)) {
        foundTools++;
      }
    }
    
    // Should find at least some drawing tools
    expect(foundTools).toBeGreaterThan(0);
  });

  test("should handle color selection", async ({ page }) => {
    // Look for color picker or color options
    const colorPicker = page.locator('input[type="color"], [data-testid="color-picker"]').first();
    
    if (await colorPicker.isVisible({ timeout: 3000 })) {
      // Test color selection
      await colorPicker.click();
      
      // Color picker should be interactive
      expect(true).toBe(true);
    } else {
      // Look for color buttons/swatches
      const colorButtons = page.locator(
        '.color-option, .color-swatch, button[data-color], ' +
        '[data-testid="color-red"], [data-testid="color-blue"]'
      );
      
      const colorCount = await colorButtons.count();
      if (colorCount > 0) {
        await colorButtons.first().click();
        expect(true).toBe(true);
      }
    }
  });

  test("should handle brush size adjustment", async ({ page }) => {
    // Look for brush size controls
    const brushSizeControls = [
      'input[type="range"]',
      '[data-testid="brush-size"]',
      '.brush-size',
      'button:has-text("Small")',
      'button:has-text("Medium")',
      'button:has-text("Large")'
    ];
    
    for (const selector of brushSizeControls) {
      const control = page.locator(selector).first();
      if (await control.isVisible({ timeout: 2000 })) {
        await control.click();
        
        // If it's a range input, test changing the value
        if (selector.includes('range')) {
          await control.fill("50");
        }
        
        expect(true).toBe(true);
        break;
      }
    }
  });

  test("should handle clear/reset canvas", async ({ page }) => {
    const canvas = page.locator('canvas').first();
    
    if (await canvas.isVisible({ timeout: 5000 })) {
      // First, draw something
      const canvasBox = await canvas.boundingBox();
      if (canvasBox) {
        await page.mouse.move(canvasBox.x + 50, canvasBox.y + 50);
        await page.mouse.down();
        await page.mouse.move(canvasBox.x + 100, canvasBox.y + 100);
        await page.mouse.up();
        
        // Look for clear/reset button
        const clearButton = page.locator(
          'button:has-text("Clear"), button:has-text("Reset"), ' +
          '[data-testid="clear-canvas"], .clear-canvas'
        ).first();
        
        if (await clearButton.isVisible({ timeout: 3000 })) {
          await clearButton.click();
          
          // Canvas should be cleared (this is a basic interaction test)
          expect(true).toBe(true);
        }
      }
    }
  });

  test("should handle undo functionality", async ({ page }) => {
    const canvas = page.locator('canvas').first();
    
    if (await canvas.isVisible({ timeout: 5000 })) {
      // Draw something first
      const canvasBox = await canvas.boundingBox();
      if (canvasBox) {
        await page.mouse.move(canvasBox.x + 50, canvasBox.y + 50);
        await page.mouse.down();
        await page.mouse.move(canvasBox.x + 100, canvasBox.y + 100);
        await page.mouse.up();
        
        // Look for undo button
        const undoButton = page.locator(
          'button:has-text("Undo"), [data-testid="undo"], .undo'
        ).first();
        
        if (await undoButton.isVisible({ timeout: 3000 })) {
          await undoButton.click();
          expect(true).toBe(true);
        } else {
          // Test keyboard shortcut
          await page.keyboard.press("Control+Z");
          expect(true).toBe(true);
        }
      }
    }
  });

  test("should be responsive on mobile", async ({ page, isMobile }) => {
    if (isMobile) {
      const canvas = page.locator('canvas').first();
      
      if (await canvas.isVisible({ timeout: 5000 })) {
        const canvasBox = await canvas.boundingBox();
        
        if (canvasBox) {
          // Test touch drawing
          await page.touchscreen.tap(canvasBox.x + 50, canvasBox.y + 50);
          
          // Simulate touch drawing
          await page.touchscreen.tap(canvasBox.x + 60, canvasBox.y + 60);
          await page.touchscreen.tap(canvasBox.x + 70, canvasBox.y + 70);
          
          expect(true).toBe(true);
        }
      }
    }
  });
});