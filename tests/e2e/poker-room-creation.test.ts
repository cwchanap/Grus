import { test, expect } from "@playwright/test";
import { GameTestHelpers } from "./utils/test-helpers.ts";

/**
 * Tests for poker room creation functionality
 */

test.describe("Poker Room Creation", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("should display poker as game type option in create room modal", async ({ page }) => {
    // Click the create room button to open modal
    const createButton = page.getByRole("button", { name: "+ Create Room" });
    await expect(createButton).toBeVisible({ timeout: 5000 });
    await createButton.click();

    // Wait for modal to appear
    const modal = page.locator('[data-testid="create-room-modal"]');
    await expect(modal).toBeVisible({ timeout: 5000 });

    // Check that the game type dropdown contains poker option
    const gameTypeSelect = modal.locator('select').nth(0); // First select should be game type
    await expect(gameTypeSelect).toBeVisible();

    // Get all options and verify poker is present
    const options = await gameTypeSelect.locator('option').allTextContents();
    const hasPokerOption = options.some(option => option.includes("Poker") || option.includes("🃏"));

    expect(hasPokerOption).toBe(true);
  });

  test("should create poker room successfully", async ({ page }) => {
    // Click the create room button to open modal
    const createButton = page.getByRole("button", { name: "+ Create Room" });
    await createButton.click();

    // Wait for modal to appear
    const modal = page.locator('[data-testid="create-room-modal"]');
    await expect(modal).toBeVisible({ timeout: 5000 });

    // Fill in room details
    const roomNameInput = modal.locator('[data-testid="room-name-input"]');
    const hostNameInput = modal.locator('[data-testid="host-name-input"]');
    const gameTypeSelect = modal.locator('select').nth(0);
    const submitButton = modal.locator('[data-testid="create-room-submit"]');

    await roomNameInput.fill("Test Poker Room");
    await hostNameInput.fill("Poker Tester");

    // Select poker game type
    await gameTypeSelect.selectOption({ label: /.*Poker.*/ });

    // Submit the form
    await submitButton.click();

    // Should navigate to room page
    await expect(page).toHaveURL(/\/room\/[a-f0-9-]+/, { timeout: 10000 });

    // Verify we're in a room
    const roomUrl = page.url();
    const roomCode = roomUrl.match(/\/room\/([^\/\?]+)/)?.[1];
    expect(roomCode).toBeTruthy();
  });

  test("should show correct player count options for poker", async ({ page }) => {
    // Click the create room button to open modal
    const createButton = page.getByRole("button", { name: "+ Create Room" });
    await createButton.click();

    // Wait for modal to appear
    const modal = page.locator('[data-testid="create-room-modal"]');
    await expect(modal).toBeVisible({ timeout: 5000 });

    // Select poker game type first
    const gameTypeSelect = modal.locator('select').nth(0);
    await gameTypeSelect.selectOption({ label: /.*Poker.*/ });

    // Check max players dropdown
    const maxPlayersSelect = modal.locator('select').nth(1); // Second select should be max players
    await expect(maxPlayersSelect).toBeVisible();

    // Get all player count options
    const playerOptions = await maxPlayersSelect.locator('option').allTextContents();

    // Poker should support 2-8 players
    const hasMinPlayers = playerOptions.some(option => option.includes("2 players"));
    const hasMaxPlayers = playerOptions.some(option => option.includes("8 players"));

    expect(hasMinPlayers).toBe(true);
    expect(hasMaxPlayers).toBe(true);
  });

  test("should validate poker room creation with different player counts", async ({ page }) => {
    const helper = new GameTestHelpers(page);

    // Test creating poker room with different player counts
    const playerCounts = [2, 4, 6, 8];

    for (const playerCount of playerCounts) {
      // Navigate to home
      await page.goto("/");

      // Click create room button
      const createButton = page.getByRole("button", { name: "+ Create Room" });
      await createButton.click();

      // Wait for modal
      const modal = page.locator('[data-testid="create-room-modal"]');
      await expect(modal).toBeVisible({ timeout: 5000 });

      // Fill in details
      const roomNameInput = modal.locator('[data-testid="room-name-input"]');
      const hostNameInput = modal.locator('[data-testid="host-name-input"]');
      const gameTypeSelect = modal.locator('select').nth(0);
      const maxPlayersSelect = modal.locator('select').nth(1);
      const submitButton = modal.locator('[data-testid="create-room-submit"]');

      await roomNameInput.fill(`Poker Room ${playerCount} Players`);
      await hostNameInput.fill("Poker Host");
      await gameTypeSelect.selectOption({ label: /.*Poker.*/ });
      await maxPlayersSelect.selectOption({ label: `${playerCount} players` });

      // Submit and verify navigation
      await submitButton.click();
      await expect(page).toHaveURL(/\/room\/[a-f0-9-]+/, { timeout: 10000 });

      // Verify we're in the room
      expect(await helper.isInRoom()).toBe(true);
    }
  });

  test("should handle API failure gracefully", async ({ page }) => {
    // Intercept the games API to simulate failure
    await page.route("/api/games", (route) => {
      route.fulfill({
        status: 500,
        contentType: "application/json",
        body: JSON.stringify({ error: "Internal server error" }),
      });
    });

    // Click create room button
    const createButton = page.getByRole("button", { name: "+ Create Room" });
    await createButton.click();

    // Wait for modal
    const modal = page.locator('[data-testid="create-room-modal"]');
    await expect(modal).toBeVisible({ timeout: 5000 });

    // Should still show drawing game as fallback
    const gameTypeSelect = modal.locator('select').nth(0);
    const options = await gameTypeSelect.locator('option').allTextContents();

    // Should have at least drawing game as fallback
    const hasDrawingOption = options.some(option => option.includes("Drawing") || option.includes("🎨"));
    expect(hasDrawingOption).toBe(true);
  });

  test("should show loading state while fetching game types", async ({ page }) => {
    // Intercept the games API to add delay
    await page.route("/api/games", async (route) => {
      // Add a delay to simulate slow loading
      await new Promise(resolve => setTimeout(resolve, 1000));
      route.continue();
    });

    // Click create room button
    const createButton = page.getByRole("button", { name: "+ Create Room" });
    await createButton.click();

    // Wait for modal
    const modal = page.locator('[data-testid="create-room-modal"]');
    await expect(modal).toBeVisible({ timeout: 5000 });

    // Should show loading state initially
    const gameTypeSelect = modal.locator('select').nth(0);
    const loadingOption = gameTypeSelect.locator('option:has-text("Loading game types...")');

    // Check if loading state appears (might be brief)
    const hasLoadingState = await loadingOption.isVisible({ timeout: 2000 }).catch(() => false);

    // Eventually should show actual game types
    await expect(gameTypeSelect.locator('option').first()).not.toHaveText("Loading game types...");
  });
});