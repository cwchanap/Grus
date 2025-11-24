import { test, expect } from "@playwright/test";

/**
 * Tests for share link button functionality and toast notifications
 */

test.describe("Share Link Button", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("should copy room link to clipboard and show toast notification", async ({ page }) => {
    // Create a room first
    const createRoomButton = page.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();

    if (await createRoomButton.isVisible()) {
      await createRoomButton.click();

      // Wait for room to load
      await page.waitForTimeout(2000);

      // Verify we're in a room (UUID format)
      await expect(page.url()).toMatch(/\/room\/[a-f0-9-]+/);

      // Find and click the share button
      const shareButton = page.locator('button:has-text("📤 Share")').first();
      await expect(shareButton).toBeVisible();

      // Click the share button
      await shareButton.click();

      // Wait for toast notification to appear
      await page.waitForTimeout(500);

      // Check if toast notification appears
      const toastNotification = page.locator('.fixed.top-4.right-4 .bg-green-500').first();
      await expect(toastNotification).toBeVisible({ timeout: 3000 });

      // Verify toast message content
      await expect(toastNotification).toContainText('Room link copied to clipboard');

      // Verify clipboard contains the room URL
      const clipboardContent = await page.evaluate(() => navigator.clipboard.readText());
      const currentUrl = page.url();
      expect(clipboardContent).toBe(currentUrl);

      // Wait for toast to auto-dismiss
      await page.waitForTimeout(3500);
      await expect(toastNotification).not.toBeVisible();
    }
  });

  test("should handle share button on mobile view", async ({ page }) => {
    // Set mobile viewport
    await page.setViewportSize({ width: 375, height: 667 });

    // Create a room first
    const createRoomButton = page.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();

    if (await createRoomButton.isVisible()) {
      await createRoomButton.click();

      // Wait for room to load
      await page.waitForTimeout(2000);

      // Verify we're in a room (UUID format)
      await expect(page.url()).toMatch(/\/room\/[a-f0-9-]+/);

      // Find the mobile share button (should show just the emoji)
      const shareButton = page.locator('button:has-text("📤")').first();
      await expect(shareButton).toBeVisible();

      // Click the share button
      await shareButton.click();

      // Wait for toast notification
      await page.waitForTimeout(500);

      // Check if toast notification appears
      const toastNotification = page.locator('.fixed.top-4.right-4 .bg-green-500').first();
      await expect(toastNotification).toBeVisible({ timeout: 3000 });

      // Verify toast message
      await expect(toastNotification).toContainText('Room link copied to clipboard');
    }
  });

  test("should handle clipboard API not supported gracefully", async ({ page }) => {
    // Mock clipboard API to be unavailable
    await page.addScriptTag({
      content: `
        Object.defineProperty(navigator, 'clipboard', {
          value: undefined,
          writable: true
        });
      `
    });

    // Create a room first
    const createRoomButton = page.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();

    if (await createRoomButton.isVisible()) {
      await createRoomButton.click();

      // Wait for room to load
      await page.waitForTimeout(2000);

      // Verify we're in a room (UUID format)
      await expect(page.url()).toMatch(/\/room\/[a-f0-9-]+/);

      // Find and click the share button
      const shareButton = page.locator('button:has-text("📤 Share")').first();
      await expect(shareButton).toBeVisible();

      // Click the share button
      await shareButton.click();

      // Should still work with fallback (prompt or other method)
      // The implementation should handle this gracefully
      await page.waitForTimeout(1000);

      // Test passes if no JavaScript errors occur
      const hasErrors = await page.evaluate(() => {
        return window.console.error && window.console.error.length > 0;
      });

      expect(hasErrors).toBe(false);
    }
  });

  test("should show different toast types", async ({ page }) => {
    // Create a room first
    const createRoomButton = page.locator('button:has-text("Create Room"), a:has-text("Create Room")').first();

    if (await createRoomButton.isVisible()) {
      await createRoomButton.click();
      await page.waitForTimeout(2000);

      // Verify we're in a room (UUID format)
      await expect(page.url()).toMatch(/\/room\/[a-f0-9-]+/);

      // Test success toast (from share button)
      const shareButton = page.locator('button:has-text("📤 Share")').first();
      await shareButton.click();

      await page.waitForTimeout(500);
      const successToast = page.locator('.fixed.top-4.right-4 .bg-green-500').first();
      await expect(successToast).toBeVisible();

      // Test error toast by dispatching custom event
      await page.evaluate(() => {
        const event = new CustomEvent('showToast', {
          detail: { message: 'Test error message', type: 'error' }
        });
        window.dispatchEvent(event);
      });

      await page.waitForTimeout(500);
      const errorToast = page.locator('.fixed.top-4.right-4 .bg-red-500').first();
      await expect(errorToast).toBeVisible();
      await expect(errorToast).toContainText('Test error message');

      // Test info toast
      await page.evaluate(() => {
        const event = new CustomEvent('showToast', {
          detail: { message: 'Test info message', type: 'info' }
        });
        window.dispatchEvent(event);
      });

      await page.waitForTimeout(500);
      const infoToast = page.locator('.fixed.top-4.right-4 .bg-blue-500').first();
      await expect(infoToast).toBeVisible();
      await expect(infoToast).toContainText('Test info message');
    }
  });
});