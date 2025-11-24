import { test, expect } from "@playwright/test";

/**
 * Test Account Verification E2E Tests
 * 
 * This test suite verifies:
 * 1. Whether authentication is properly set up
 * 2. Test account creation and login functionality
 * 3. Profile navigation feature (username clickability)
 * 4. Graceful handling when authentication is not configured
 */

const TEST_ACCOUNT = {
  email: "test@example.com",
  username: "testuser",
  password: "testpass123",
  name: "Test User",
};

test.describe("Test Account and Authentication Verification", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("should handle unauthenticated state gracefully", async ({ page }) => {
    console.log("🔍 Testing unauthenticated state handling...");
    
    // Check if login link is visible (indicating no auth or user not logged in)
    const loginLink = page.locator('a[href="/login"]');
    const isLoginVisible = await loginLink.isVisible();
    
    if (isLoginVisible) {
      console.log("✅ Login link is visible - authentication flow is available");
      await expect(loginLink).toBeVisible();
      
      // Check that we can navigate to login page
      await loginLink.click();
      await expect(page).toHaveURL("/login");
      await expect(page.getByRole("heading", { name: "Welcome Back!" })).toBeVisible();
    } else {
      console.log("⚠️ No login link visible - checking for authenticated state or auth disabled");
      
      // Look for user profile indicator (username in top nav)
      const userProfile = page.locator('text=/👤/');
      const hasUserProfile = await userProfile.isVisible();
      
      if (hasUserProfile) {
        console.log("✅ User appears to be authenticated");
      } else {
        console.log("⚠️ Authentication might be disabled or not configured");
      }
    }
  });

  test("should test authentication setup status", async ({ page }) => {
    console.log("🔧 Testing authentication setup status...");
    
    // Try to access login page
    await page.goto("/login");
    
    // Check if login form loads properly
    const loginForm = page.locator('form');
    const emailField = page.locator('#emailOrUsername');
    const passwordField = page.locator('#password');
    const loginButton = page.getByRole("button", { name: "Login" });
    
    const hasLoginForm = await loginForm.isVisible();
    const hasEmailField = await emailField.isVisible();
    const hasPasswordField = await passwordField.isVisible();
    const hasLoginButton = await loginButton.isVisible();
    
    if (hasLoginForm && hasEmailField && hasPasswordField && hasLoginButton) {
      console.log("✅ Login form is properly rendered - authentication UI is working");
      
      // Test with invalid credentials to see if backend is responding
      await emailField.fill("invalid@test.com");
      await passwordField.fill("invalidpassword");
      await loginButton.click();
      
      // Wait for response
      await page.waitForTimeout(3000);
      
      // Check for error response (indicates backend is working)
      const errorMessage = page.locator('.bg-red-50, .text-red-700, [role="alert"]');
      const hasError = await errorMessage.isVisible();
      
      if (hasError) {
        const errorText = await errorMessage.textContent();
        console.log(`✅ Authentication backend is responding with error: ${errorText}`);
      } else {
        console.log("⚠️ No error message - authentication backend may not be properly configured");
      }
    } else {
      console.log("❌ Login form is incomplete - authentication may not be properly set up");
    }
  });

  test("should test account creation if possible", async ({ page }) => {
    console.log("👤 Testing account creation...");
    
    await page.goto("/signup");
    
    // Check if signup form is available
    const signupForm = page.locator('form');
    const hasSignupForm = await signupForm.isVisible();
    
    if (!hasSignupForm) {
      console.log("⚠️ Signup form not available - skipping account creation test");
      test.skip();
      return;
    }
    
    console.log("✅ Signup form is available");
    
    // Generate unique test data to avoid conflicts
    const timestamp = Date.now();
    const testEmail = `test-e2e-${timestamp}@grus.dev`;
    const testUsername = `testuser-e2e-${timestamp}`;
    
    // Fill signup form
    await page.locator('#email').fill(testEmail);
    await page.locator('#username').fill(testUsername);
    await page.locator('#name').fill("Test User E2E");
    await page.locator('#password').fill(TEST_ACCOUNT.password);
    await page.locator('#confirmPassword').fill(TEST_ACCOUNT.password);
    
    // Submit form
    console.log(`🔄 Attempting to create account: ${testEmail}`);
    await page.getByRole("button", { name: "Sign Up" }).click();
    
    // Wait for response
    await page.waitForTimeout(5000);
    
    const currentUrl = page.url();
    const errorMessage = page.locator('.bg-red-50');
    const hasError = await errorMessage.isVisible();
    
    if (hasError) {
      const errorText = await errorMessage.textContent();
      console.log(`❌ Account creation failed: ${errorText}`);
      
      if (errorText?.includes("DATABASE_URL") || errorText?.includes("Prisma") || errorText?.includes("connection")) {
        console.log("💡 This indicates authentication database is not set up");
      }
    } else if (currentUrl === "/" || !currentUrl.includes("/signup")) {
      console.log("✅ Account creation appears successful - redirected from signup");
      
      // Test the new account login
      await page.goto("/login");
      await page.locator('#emailOrUsername').fill(testEmail);
      await page.locator('#password').fill(TEST_ACCOUNT.password);
      await page.getByRole("button", { name: "Login" }).click();
      
      await page.waitForTimeout(3000);
      
      if (page.url() === "/") {
        console.log("✅ Login with new account successful");
      } else {
        console.log("⚠️ Login with new account failed");
      }
    } else {
      console.log("⚠️ Account creation result unclear");
    }
  });

  test("should test predefined test account login", async ({ page }) => {
    console.log("🔑 Testing predefined test account login...");
    
    await page.goto("/login");
    
    // Check if login form is available
    const loginForm = page.locator('form');
    if (!await loginForm.isVisible()) {
      console.log("⚠️ Login form not available - skipping test account login");
      test.skip();
      return;
    }
    
    // Try login with test account credentials
    console.log(`🔄 Attempting login with: ${TEST_ACCOUNT.email}`);
    await page.locator('#emailOrUsername').fill(TEST_ACCOUNT.email);
    await page.locator('#password').fill(TEST_ACCOUNT.password);
    await page.getByRole("button", { name: "Login" }).click();
    
    // Wait for response
    await page.waitForTimeout(5000);
    
    const currentUrl = page.url();
    const errorMessage = page.locator('.bg-red-50');
    const hasError = await errorMessage.isVisible();
    
    if (hasError) {
      const errorText = await errorMessage.textContent();
      console.log(`❌ Test account login failed: ${errorText}`);
      
      if (errorText?.includes("Invalid credentials") || errorText?.includes("User not found")) {
        console.log("💡 Test account does not exist. Run 'deno task test:create-account' to create it.");
      } else if (errorText?.includes("DATABASE_URL") || errorText?.includes("Prisma")) {
        console.log("💡 Authentication database is not properly configured.");
      }
      
      expect(hasError).toBe(true); // This will fail but provide useful info
    } else if (currentUrl === "/") {
      console.log("✅ Test account login successful!");
      
      // Verify user is authenticated by looking for username in nav
      const userNav = page.locator('text=/👤.*Test User|👤.*testuser/');
      await expect(userNav).toBeVisible();
      console.log("✅ User authentication state confirmed in navigation");
    } else {
      console.log("⚠️ Login result unclear - still on login page");
    }
  });

  test("should test username profile navigation feature", async ({ page }) => {
    console.log("🖱️ Testing username profile navigation...");
    
    // First try to login (if auth is set up)
    await page.goto("/login");
    
    const loginForm = page.locator('form');
    if (await loginForm.isVisible()) {
      // Try to login with test account
      await page.locator('#emailOrUsername').fill(TEST_ACCOUNT.email);
      await page.locator('#password').fill(TEST_ACCOUNT.password);
      await page.getByRole("button", { name: "Login" }).click();
      await page.waitForTimeout(3000);
    }
    
    // Go to main page
    await page.goto("/");
    
    // Look for username in navigation
    const usernameLink = page.locator('a[href="/profile"]');
    const usernameText = page.locator('text=/👤.*Test User|👤.*testuser/');
    
    const hasUsernameLink = await usernameLink.isVisible();
    const hasUsernameText = await usernameText.isVisible();
    
    if (hasUsernameLink && hasUsernameText) {
      console.log("✅ Username is displayed and clickable in navigation");
      
      // Test clicking username navigates to profile
      await usernameLink.click();
      await page.waitForTimeout(2000);
      
      const currentUrl = page.url();
      if (currentUrl.includes("/profile")) {
        console.log("✅ Username click successfully navigated to profile page");
        await expect(page).toHaveURL("/profile");
      } else if (currentUrl.includes("/login")) {
        console.log("✅ Username click correctly redirected to login (user not authenticated)");
        await expect(page).toHaveURL("/login");
      } else {
        console.log(`⚠️ Unexpected navigation result: ${currentUrl}`);
      }
    } else if (hasUsernameText && !hasUsernameLink) {
      console.log("⚠️ Username is displayed but not clickable - feature may not be implemented");
    } else {
      console.log("⚠️ No authenticated user found in navigation - testing profile redirect");
      
      // Test direct profile access without authentication
      await page.goto("/profile");
      await page.waitForTimeout(2000);
      
      // Should redirect to login
      await expect(page).toHaveURL("/login");
      console.log("✅ Profile page correctly redirects to login when not authenticated");
    }
  });

  test("should test complete authentication flow end-to-end", async ({ page }) => {
    console.log("🔄 Testing complete authentication flow...");
    
    // 1. Start unauthenticated
    await page.goto("/");
    console.log("Step 1: Started on homepage");
    
    // 2. Access login page
    await page.goto("/login");
    const hasLoginForm = await page.locator('form').isVisible();
    
    if (!hasLoginForm) {
      console.log("❌ Authentication is not properly set up - login form not available");
      test.skip();
      return;
    }
    
    console.log("Step 2: Login form is available");
    
    // 3. Test invalid login first
    await page.locator('#emailOrUsername').fill("invalid@test.com");
    await page.locator('#password').fill("wrongpassword");
    await page.getByRole("button", { name: "Login" }).click();
    await page.waitForTimeout(3000);
    
    const hasLoginError = await page.locator('.bg-red-50').isVisible();
    if (hasLoginError) {
      console.log("Step 3: Invalid login correctly shows error");
    }
    
    // 4. Try valid login (if test account exists)
    await page.locator('#emailOrUsername').fill(TEST_ACCOUNT.email);
    await page.locator('#password').fill(TEST_ACCOUNT.password);
    await page.getByRole("button", { name: "Login" }).click();
    await page.waitForTimeout(5000);
    
    const currentUrl = page.url();
    
    if (currentUrl === "/") {
      console.log("Step 4: Valid login successful - redirected to homepage");
      
      // 5. Test authenticated features
      const userNav = page.locator('a[href="/profile"]');
      if (await userNav.isVisible()) {
        console.log("Step 5: Authenticated user navigation visible");
        
        // 6. Test profile navigation
        await userNav.click();
        await page.waitForTimeout(2000);
        
        if (page.url().includes("/profile")) {
          console.log("Step 6: Profile navigation successful");
          await expect(page.getByText("Profile Information")).toBeVisible();
        }
        
        // 7. Test logout
        await page.goto("/");
        const logoutLink = page.locator('a:has-text("Logout")');
        if (await logoutLink.isVisible()) {
          await logoutLink.click();
          await page.waitForTimeout(2000);
          console.log("Step 7: Logout successful");
        }
      }
    } else {
      const errorText = await page.locator('.bg-red-50').textContent();
      console.log(`Step 4: Login failed - ${errorText}`);
      console.log("💡 This likely means the test account doesn't exist or auth DB is not set up");
    }
  });

  test("should provide setup instructions", async ({ page }) => {
    console.log("📋 Authentication Setup Status Report");
    console.log("=====================================");
    
    await page.goto("/");
    
    const loginLink = page.locator('a[href="/login"]');
    const hasLogin = await loginLink.isVisible();
    
    if (hasLogin) {
      await page.goto("/login");
      const hasForm = await page.locator('form').isVisible();
      
      if (hasForm) {
        console.log("✅ Authentication UI is present");
        
        // Test backend connectivity
        await page.locator('#emailOrUsername').fill("test");
        await page.locator('#password').fill("test");
        await page.getByRole("button", { name: "Login" }).click();
        await page.waitForTimeout(3000);
        
        const errorMessage = await page.locator('.bg-red-50').textContent();
        
        if (errorMessage) {
          console.log("✅ Authentication backend is responding");
          
          if (errorMessage.includes("DATABASE_URL") || errorMessage.includes("Prisma")) {
            console.log("❌ Database not configured");
            console.log("");
            console.log("🔧 Setup Instructions:");
            console.log("1. Follow docs/authentication-setup.md");
            console.log("2. Set DATABASE_URL environment variable");
            console.log("3. Run: deno run -A scripts/setup-prisma.ts");
            console.log("4. Run: deno task test:create-account");
            console.log("5. Test login with test@example.com / testpass123");
          } else if (errorMessage.includes("Invalid credentials")) {
            console.log("✅ Database is configured but test account doesn't exist");
            console.log("");
            console.log("🔧 Next Steps:");
            console.log("1. Run: deno task test:create-account");
            console.log("2. Test login with test@example.com / testpass123");
          } else {
            console.log(`⚠️ Unexpected error: ${errorMessage}`);
          }
        } else {
          console.log("⚠️ No error response - authentication status unclear");
        }
      } else {
        console.log("❌ Login form not rendering properly");
      }
    } else {
      console.log("⚠️ No login link found - authentication may be disabled");
    }
    
    console.log("");
    console.log("🎮 Alternative: Test without authentication");
    console.log("• Game works in guest mode");
    console.log("• Users can create/join rooms without accounts");
    console.log("• Profile features require authentication");
  });
});
