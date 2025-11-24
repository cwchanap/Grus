import { test, expect } from "@playwright/test";

test.describe("Leave Room Dialog Positioning", () => {
  test("dialog should overlay on top of screen, not squeezed to nav bar", async ({ page }) => {
    // Navigate to lobby
    await page.goto("/");
    await page.waitForLoadState("networkidle");

    // Create a room
    await page.fill('input[placeholder="Your Name"]', "TestUser");
    await page.click('button:has-text("Create Room")');
    await page.waitForURL(/\/room\/.+/, { timeout: 10000 });
    await page.waitForTimeout(2000);

    // Click leave room button
    await page.click('button:has-text("Lobby")');
    await page.waitForTimeout(500);

    // Find the dialog overlay
    const dialogOverlay = page.locator('.fixed.inset-0').first();
    await expect(dialogOverlay).toBeVisible();

    // Get the dialog overlay bounding box
    const overlayBox = await dialogOverlay.boundingBox();
    console.log("Dialog overlay bounding box:", overlayBox);

    // Get viewport size
    const viewport = page.viewportSize();
    console.log("Viewport size:", viewport);

    // Check that dialog overlay covers the full viewport
    if (overlayBox && viewport) {
      expect(overlayBox.x).toBe(0);
      expect(overlayBox.y).toBe(0);
      expect(overlayBox.width).toBe(viewport.width);
      expect(overlayBox.height).toBe(viewport.height);
    }

    // Get computed styles and verify portal rendering
    const dialogInfo = await dialogOverlay.evaluate((el) => {
      const computed = window.getComputedStyle(el);
      return {
        position: computed.position,
        top: computed.top,
        left: computed.left,
        right: computed.right,
        bottom: computed.bottom,
        zIndex: computed.zIndex,
        isDirectChildOfBody: el.parentElement?.tagName === 'BODY',
      };
    });
    console.log("Dialog overlay info:", dialogInfo);

    // Verify fixed positioning
    expect(dialogInfo.position).toBe("fixed");
    expect(dialogInfo.top).toBe("0px");
    expect(dialogInfo.left).toBe("0px");
    expect(dialogInfo.right).toBe("0px");
    expect(dialogInfo.bottom).toBe("0px");

    // Verify dialog is rendered via portal as direct child of body
    expect(dialogInfo.isDirectChildOfBody).toBe(true);

    // Take screenshot for visual verification
    await page.screenshot({ path: "/tmp/dialog_position_test.png", fullPage: true });
    console.log("Screenshot saved to /tmp/dialog_position_test.png");
  });
});
