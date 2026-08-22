import { expect, test } from '@playwright/test';

test('opens the desktop application UI', async ({ page }) => {
  await page.goto('/');
});
