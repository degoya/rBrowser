// Exported by rBrowser (recorder-export) from the recording "login".
// Run it with `npx playwright test`: playwright.config.ts makes every breakpoint a project.
// Secrets are never written here: set RB_SECRET_PASSWORD in the environment; a step stops with an error when one is not set.
import { test, expect } from '@playwright/test';

function rbSecret(name: string): string {
  const value = process.env[name];
  if (value === undefined || value === '') {
    throw new Error(`The secret ${name} is not set in the environment.`);
  }
  return value;
}

test('login', async ({ page }) => {
  // Step 1: navigate
  await page.goto('/recorder.html');
  // Step 2: click - Open the mobile menu (only in mobile)
  if (['mobile'].includes(test.info().project.name)) {
    await page.getByRole('button', { name: 'Menu', exact: true }).filter({ visible: true }).first().click();
  }
  // Step 3: assert visible (only in mobile)
  if (['mobile'].includes(test.info().project.name)) {
    await expect(page.getByRole('link', { name: 'About', exact: true }).filter({ visible: true }).first()).toBeVisible();
  }
  // Step 4: press
  await page.keyboard.press('Escape');
  // Step 5: fill
  await page.getByTestId('email').fill('alice@example.org');
  // Step 6: assert value
  await expect(page.getByTestId('email')).toHaveValue('alice@example.org');
  // Step 7: fill
  await page.locator('#password').fill(rbSecret('RB_SECRET_PASSWORD'));
  // Step 8: select
  await page.locator('#plan').selectOption('pro');
  // Step 9: check
  await page.getByRole('checkbox', { name: 'Remember me', exact: true }).filter({ visible: true }).first().check();
  // Step 10: uncheck
  await page.getByRole('checkbox', { name: 'Remember me', exact: true }).filter({ visible: true }).first().uncheck();
  // Step 11: hover
  await page.getByTestId('help').hover();
  // Step 12: assert visible
  await expect(page.locator('span').filter({ hasText: /^\s*We\s+never\s+share\s+your\s+email\.\s*$/ }).filter({ visible: true }).first()).toBeVisible();
  // Step 13: dblclick
  await page.getByTestId('counter').dblclick();
  // Step 14: assert text
  await expect(page.getByTestId('counter')).toHaveText('Double-clicked');
  // Step 15: scroll
  await page.locator('#terms').evaluate((element) => element.scrollTo(0, 120));
  // Step 16: click - Sign in
  await page.getByTestId('sign-in').click();
  // Step 17: waitForUrl
  await expect(page).toHaveURL('/recorder-welcome.html?plan=pro');
  // Step 18: assert text
  await expect(page.locator('#who')).toHaveText('Signed in as alice@example.org');
  // Step 19: assert url
  await expect(page).toHaveURL('/recorder-welcome.html?plan=pro');
});
