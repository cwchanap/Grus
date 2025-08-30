import { test, expect } from "@playwright/test";

test.describe("Authentication Flow", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("should show login and signup links", async ({ page }) => {
    // Check for auth-related navigation
    const loginLink = page.locator('a[href="/login"]');
    const signupLink = page.locator('a[href="/signup"]');
    
    // Either link should be visible (depending on auth state)
    const hasLoginLink = await loginLink.isVisible();
    const hasSignupLink = await signupLink.isVisible();
    
    expect(hasLoginLink || hasSignupLink).toBe(true);
  });

  test("should navigate to signup page", async ({ page }) => {
    await page.goto("/signup");
    
    // Should show signup form
    await expect(page.getByRole("heading", { name: "Create an Account" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Sign Up" })).toBeVisible();
    
    // Check form fields
    await expect(page.locator('#email')).toBeVisible();
    await expect(page.locator('#username')).toBeVisible();
    await expect(page.locator('#password')).toBeVisible();
    await expect(page.locator('#confirmPassword')).toBeVisible();
  });

  test("should navigate to login page", async ({ page }) => {
    await page.goto("/login");
    
    // Should show login form
    await expect(page.getByRole("heading", { name: "Welcome Back!" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Login" })).toBeVisible();
    
    // Check form fields
    await expect(page.locator('#emailOrUsername')).toBeVisible();
    await expect(page.locator('#password')).toBeVisible();
  });

  test("should show validation errors for invalid signup", async ({ page }) => {
    await page.goto("/signup");
    
    // Try to submit with empty fields
    await page.getByRole("button", { name: "Sign Up" }).click();
    
    // Should still be on signup page (form validation prevents submission)
    await expect(page).toHaveURL("/signup");
  });

  test("should show validation errors for password mismatch", async ({ page }) => {
    await page.goto("/signup");
    
    // Fill form with mismatched passwords
    await page.locator('#email').fill("test@example.com");
    await page.locator('#username').fill("testuser");
    await page.locator('#password').fill("password123");
    await page.locator('#confirmPassword').fill("different");
    
    // Submit form
    await page.getByRole("button", { name: "Sign Up" }).click();
    
    // Should show error message
    await expect(page.locator('text="Passwords do not match"')).toBeVisible();
  });

  test("should show validation errors for short password", async ({ page }) => {
    await page.goto("/signup");
    
    // Fill form with short password
    await page.locator('#email').fill("test@example.com");
    await page.locator('#username').fill("testuser");
    await page.locator('#password').fill("123");
    await page.locator('#confirmPassword').fill("123");
    
    // Submit form
    await page.getByRole("button", { name: "Sign Up" }).click();
    
    // Should show error message
    await expect(page.locator('text="Password must be at least 6 characters"')).toBeVisible();
  });

  test("should attempt signup with valid data", async ({ page }) => {
    await page.goto("/signup");
    
    // Generate unique test data
    const timestamp = Date.now();
    const email = `test${timestamp}@example.com`;
    const username = `testuser${timestamp}`;
    
    // Fill form with valid data
    await page.locator('#email').fill(email);
    await page.locator('#username').fill(username);
    await page.locator('#name').fill("Test User");
    await page.locator('#password').fill("testpassword123");
    await page.locator('#confirmPassword').fill("testpassword123");
    
    // Submit form
    await page.getByRole("button", { name: "Sign Up" }).click();
    
    // Wait for response (either success redirect or error message)
    await page.waitForTimeout(2000);
    
    // Check if we were redirected to home (success) or still on signup (error)
    const currentUrl = page.url();
    const isOnHomepage = currentUrl.endsWith("/") && !currentUrl.includes("/signup");
    const hasErrorMessage = await page.locator('.bg-red-50').isVisible();
    
    if (isOnHomepage) {
      console.log("✅ Signup successful - redirected to homepage");
    } else if (hasErrorMessage) {
      const errorText = await page.locator('.bg-red-50').textContent();
      console.log(`⚠️ Signup failed with error: ${errorText}`);
    } else {
      console.log("⚠️ Signup result unclear");
    }
    
    // At minimum, we should not be stuck on the signup form
    expect(isOnHomepage || hasErrorMessage).toBe(true);
  });

  test("should show validation errors for invalid login", async ({ page }) => {
    await page.goto("/login");
    
    // Try login with invalid credentials
    await page.locator('#emailOrUsername').fill("nonexistent@example.com");
    await page.locator('#password').fill("wrongpassword");
    
    // Submit form
    await page.getByRole("button", { name: "Login" }).click();
    
    // Wait for response
    await page.waitForTimeout(2000);
    
    // Should show error message or still be on login page
    const hasErrorMessage = await page.locator('.bg-red-50').isVisible();
    const stillOnLogin = page.url().includes("/login");
    
    expect(hasErrorMessage || stillOnLogin).toBe(true);
  });

  test("should handle continue without login", async ({ page }) => {
    await page.goto("/login");
    
    // Click "Continue without login" link
    const continueLink = page.locator('a[href="/"]');
    if (await continueLink.isVisible()) {
      await continueLink.click();
      
      // Should redirect to homepage
      await expect(page).toHaveURL("/");
    }
  });

  test("should handle continue without signup", async ({ page }) => {
    await page.goto("/signup");
    
    // Click "Continue without signup" link
    const continueLink = page.locator('a[href="/"]');
    if (await continueLink.isVisible()) {
      await continueLink.click();
      
      // Should redirect to homepage
      await expect(page).toHaveURL("/");
    }
  });

  test("should navigate between login and signup", async ({ page }) => {
    // Start on login page
    await page.goto("/login");
    
    // Click "Sign up" link
    await page.locator('a[href="/signup"]').click();
    await expect(page).toHaveURL("/signup");
    
    // Click "Login" link
    await page.locator('a[href="/login"]').click();
    await expect(page).toHaveURL("/login");
  });
});
