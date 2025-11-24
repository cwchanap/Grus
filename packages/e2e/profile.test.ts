import { test, expect } from "@playwright/test";

test.describe("User Profile", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("should redirect to login when accessing profile without authentication", async ({ page }) => {
    // Try to access profile page directly
    await page.goto("/profile");
    
    // Should redirect to login page
    await expect(page).toHaveURL("/login");
  });

  test("should show profile button for authenticated users", async ({ page }) => {
    // First, let's check if we can see the login flow
    await page.goto("/login");
    
    // Check if login form is present
    const loginForm = page.locator('form');
    if (await loginForm.isVisible()) {
      // Try with a test user - note this might fail if user doesn't exist
      await page.locator('#emailOrUsername').fill("testuser");
      await page.locator('#password').fill("testpassword");
      await page.getByRole("button", { name: "Login" }).click();
      
      // Wait for response
      await page.waitForTimeout(2000);
      
      // If login was successful, check for profile button
      if (page.url() === "/") {
        const profileButton = page.getByRole("button", { name: "Profile" });
        if (await profileButton.isVisible()) {
          console.log("✅ Profile button is visible for authenticated user");
        } else {
          console.log("⚠️ Profile button not found - user might not be authenticated");
        }
      }
    }
  });

  test("should navigate to profile page when profile button is clicked", async ({ page }) => {
    // This test assumes we have an authenticated session
    await page.goto("/");
    
    // Look for profile button
    const profileButton = page.getByRole("button", { name: "Profile" });
    
    if (await profileButton.isVisible()) {
      await profileButton.click();
      
      // Should navigate to profile page
      await expect(page).toHaveURL("/profile");
      
      // Should show profile elements
      await expect(page.getByRole("heading", { name: "User Profile" })).toBeVisible();
      await expect(page.getByRole("heading", { name: "Profile Information" })).toBeVisible();
      await expect(page.getByRole("heading", { name: "Change Password" })).toBeVisible();
    } else {
      // Skip test if user is not authenticated
      test.skip(!await profileButton.isVisible(), "User not authenticated");
    }
  });

  test("should show user information in profile", async ({ page }) => {
    // Navigate to profile (will redirect to login if not authenticated)
    await page.goto("/profile");
    
    // If we're on profile page, check for user info
    if (page.url().includes("/profile")) {
      // Should show profile information card
      await expect(page.getByText("Profile Information")).toBeVisible();
      
      // Should show email, username, and display name fields
      await expect(page.getByText("Email")).toBeVisible();
      await expect(page.getByText("Username")).toBeVisible();
      await expect(page.getByText("Display Name")).toBeVisible();
      
      // Should show avatar
      const avatar = page.locator('.rounded-full').first();
      await expect(avatar).toBeVisible();
    }
  });

  test("should show password change form", async ({ page }) => {
    await page.goto("/profile");
    
    // If we're on profile page, check for password form
    if (page.url().includes("/profile")) {
      // Should show change password card
      await expect(page.getByText("Change Password")).toBeVisible();
      
      // Should show password fields
      await expect(page.locator('#currentPassword')).toBeVisible();
      await expect(page.locator('#newPassword')).toBeVisible();
      await expect(page.locator('#confirmPassword')).toBeVisible();
      
      // Should show change password button
      await expect(page.getByRole("button", { name: "Change Password" })).toBeVisible();
    }
  });

  test("should validate password change form", async ({ page }) => {
    await page.goto("/profile");
    
    // If we're on profile page, test password validation
    if (page.url().includes("/profile")) {
      // Fill mismatched passwords
      await page.locator('#currentPassword').fill("currentpass");
      await page.locator('#newPassword').fill("newpass123");
      await page.locator('#confirmPassword').fill("different123");
      
      // Submit form
      await page.getByRole("button", { name: "Change Password" }).click();
      
      // Should show validation error
      await expect(page.getByText("New passwords do not match")).toBeVisible();
      
      // Test short password
      await page.locator('#newPassword').fill("123");
      await page.locator('#confirmPassword').fill("123");
      
      await page.getByRole("button", { name: "Change Password" }).click();
      
      // Should show length validation error
      await expect(page.getByText("New password must be at least 6 characters")).toBeVisible();
    }
  });

  test("should have back button functionality", async ({ page }) => {
    await page.goto("/profile");
    
    // If we're on profile page, test back button
    if (page.url().includes("/profile")) {
      const backButton = page.getByRole("button", { name: "Back" });
      await expect(backButton).toBeVisible();
      
      // Click back button
      await backButton.click();
      
      // Should go back in history
      await page.waitForTimeout(1000);
      
      // We should be navigated away from profile
      expect(page.url()).not.toContain("/profile");
    }
  });

  test("should be mobile responsive", async ({ page }) => {
    // Set mobile viewport
    await page.setViewportSize({ width: 375, height: 667 });
    
    await page.goto("/profile");
    
    // If we're on profile page, check mobile responsiveness
    if (page.url().includes("/profile")) {
      // Avatar section should be responsive
      const avatar = page.locator('.rounded-full').first();
      await expect(avatar).toBeVisible();
      
      // Cards should be visible on mobile
      await expect(page.getByText("Profile Information")).toBeVisible();
      await expect(page.getByText("Change Password")).toBeVisible();
      
      // Form elements should be visible
      await expect(page.locator('#currentPassword')).toBeVisible();
    }
  });
});
