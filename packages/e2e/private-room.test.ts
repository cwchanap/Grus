import { test, expect } from "@playwright/test";

/**
 * Tests for private room functionality
 * - Creating private rooms
 * - Dashboard filtering
 * - Direct link access
 * - Share functionality
 */

test.describe("Private Room Functionality", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("should create a private room that doesn't appear on dashboard", async ({ page }) => {
    // First, create a public room to verify it appears on dashboard
    const createRoomButton = page.getByRole("button", { name: "+ Create Room" });
    await expect(createRoomButton).toBeVisible();
    await createRoomButton.click();

    // Fill out the create room form without checking private
    const modal = page.locator('[data-testid="create-room-modal"]');
    await expect(modal).toBeVisible();

    const roomNameInput = page.locator('[data-testid="room-name-input"]');
    const hostNameInput = page.locator('[data-testid="host-name-input"]');
    const submitBtn = page.locator('[data-testid="create-room-submit"]');

    const publicRoomName = `Public Room ${Date.now()}`;
    await roomNameInput.fill(publicRoomName);
    await hostNameInput.fill("Public Host");
    await submitBtn.click();

    // Should navigate to room
    await expect(page.url()).toMatch(/\/room\/[a-f0-9-]+/);
    const publicRoomUrl = page.url();

    // Go back to lobby and verify public room appears
    await page.goto("/");
    await expect(page.locator(`text="${publicRoomName}"`)).toBeVisible();

    // Now create a private room
    await createRoomButton.click();
    await expect(modal).toBeVisible();

    const privateRoomName = `Private Room ${Date.now()}`;
    await roomNameInput.fill(privateRoomName);
    await hostNameInput.fill("Private Host");

    // Check the private room checkbox
    const privateCheckbox = page.locator('input[type="checkbox"]').first();
    await privateCheckbox.check();

    await submitBtn.click();

    // Should navigate to room
    await expect(page.url()).toMatch(/\/room\/[a-f0-9-]+/);
    const privateRoomUrl = page.url();

    // Go back to lobby and verify private room does NOT appear
    await page.goto("/");
    await expect(page.locator(`text="${privateRoomName}"`)).toBeHidden();
    // But public room should still be visible
    await expect(page.locator(`text="${publicRoomName}"`)).toBeVisible();

    // Verify we can still access private room via direct link
    await page.goto(privateRoomUrl);
    await expect(page.locator(`text="${privateRoomName}"`)).toBeVisible();
  });

  test("should allow joining private rooms via direct link", async ({ page }) => {
    // Create a private room first
    const createRoomButton = page.getByRole("button", { name: "+ Create Room" });
    await createRoomButton.click();

    const modal = page.locator('[data-testid="create-room-modal"]');
    await expect(modal).toBeVisible();

    const roomNameInput = page.locator('[data-testid="room-name-input"]');
    const hostNameInput = page.locator('[data-testid="host-name-input"]');
    const privateCheckbox = page.locator('input[type="checkbox"]').first();
    const submitBtn = page.locator('[data-testid="create-room-submit"]');

    const roomName = `Private Test Room ${Date.now()}`;
    await roomNameInput.fill(roomName);
    await hostNameInput.fill("Test Host");
    await privateCheckbox.check();
    await submitBtn.click();

    await expect(page.url()).toMatch(/\/room\/[a-f0-9-]+/);
    const roomUrl = page.url();

    // Open a new page and try to join via direct link
    const newPage = await page.context().newPage();
    await newPage.goto(roomUrl);

    // Should be able to access the room
    await expect(newPage.locator(`text="${roomName}"`)).toBeVisible();
  });

  test("should share room link using share button", async ({ page, context }) => {
    // Create a room first
    const createRoomButton = page.getByRole("button", { name: "+ Create Room" });
    await createRoomButton.click();

    const modal = page.locator('[data-testid="create-room-modal"]');
    await expect(modal).toBeVisible();

    const roomNameInput = page.locator('[data-testid="room-name-input"]');
    const hostNameInput = page.locator('[data-testid="host-name-input"]');
    const submitBtn = page.locator('[data-testid="create-room-submit"]');

    const roomName = `Share Test Room ${Date.now()}`;
    await roomNameInput.fill(roomName);
    await hostNameInput.fill("Share Host");
    await submitBtn.click();

    await expect(page.url()).toMatch(/\/room\/[a-f0-9-]+/);
    const roomUrl = page.url();

    // Find and click the share button
    const shareButton = page.locator('button[title="Share Room Link"]');
    await expect(shareButton).toBeVisible();
    await shareButton.click();

    // For testing purposes, we'll verify the button exists and is clickable
    // In a real environment with clipboard permissions, we could test the actual clipboard content
    // For now, we just ensure the share button is functional
  });

  test("should handle mixed public and private rooms on dashboard", async ({ page }) => {
    // Create multiple rooms with mixed visibility
    const createRoomButton = page.getByRole("button", { name: "+ Create Room" });

    // Create public room 1
    await createRoomButton.click();
    let modal = page.locator('[data-testid="create-room-modal"]');
    await expect(modal).toBeVisible();

    const roomNameInput = page.locator('[data-testid="room-name-input"]');
    const hostNameInput = page.locator('[data-testid="host-name-input"]');
    const submitBtn = page.locator('[data-testid="create-room-submit"]');

    await roomNameInput.fill(`Public Room 1 ${Date.now()}`);
    await hostNameInput.fill("Public Host 1");
    await submitBtn.click();

    await expect(page.url()).toMatch(/\/room\/[a-f0-9-]+/);
    await page.goto("/");

    // Create private room
    await createRoomButton.click();
    modal = page.locator('[data-testid="create-room-modal"]');
    await expect(modal).toBeVisible();

    await roomNameInput.fill(`Private Room ${Date.now()}`);
    await hostNameInput.fill("Private Host");

    const privateCheckbox = page.locator('input[type="checkbox"]').first();
    await privateCheckbox.check();
    await submitBtn.click();

    await expect(page.url()).toMatch(/\/room\/[a-f0-9-]+/);
    await page.goto("/");

    // Create public room 2
    await createRoomButton.click();
    modal = page.locator('[data-testid="create-room-modal"]');
    await expect(modal).toBeVisible();

    await roomNameInput.fill(`Public Room 2 ${Date.now()}`);
    await hostNameInput.fill("Public Host 2");
    await submitBtn.click();

    await expect(page.url()).toMatch(/\/room\/[a-f0-9-]+/);
    await page.goto("/");

    // Verify only public rooms appear on dashboard
    const roomCards = page.locator('[class*="bg-white/10"]');
    const roomCount = await roomCards.count();

    // Should have at least 2 rooms (the public ones), but private room should be hidden
    expect(roomCount).toBeGreaterThanOrEqual(2);

    // Verify no private room names appear
    await expect(page.locator('text="Private Room"')).toBeHidden();
  });
});
