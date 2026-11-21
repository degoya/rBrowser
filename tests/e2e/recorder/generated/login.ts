// @description Exported by rBrowser (recorder-export) from the recording "login".
// Secrets are never written here: the steps read RB_SECRET_PASSWORD from the environment and stop with an error when one is not set. rBrowser's flow runner passes no environment variables to a flow yet.
function rbSecret(name: string): string {
  const value = process.env[name];
  if (value === undefined || value === '') {
    throw new Error(`The secret ${name} is not set in the environment.`);
  }
  return value;
}
// Step 1: navigate
await step('Step 1: navigate', async () => {
  await page.goto('http://127.0.0.1:8000/recorder.html');
});
// Step 2: click - Open the mobile menu (only in mobile)
await step('Step 2: click - Open the mobile menu', async () => {
  if (['mobile'].includes(view.breakpoint)) {
    await page.getByRole('button', { name: 'Menu', exact: true }).filter({ visible: true }).first().click();
  }
});
// Step 3: assert visible (only in mobile)
await step('Step 3: assert visible', async () => {
  if (['mobile'].includes(view.breakpoint)) {
    await expect(page.getByRole('link', { name: 'About', exact: true }).filter({ visible: true }).first()).toBeVisible();
  }
});
// Step 4: press
await step('Step 4: press', async () => {
  await page.keyboard.press('Escape');
});
// Step 5: fill
await step('Step 5: fill', async () => {
  await page.getByTestId('email').fill('alice@example.org');
});
// Step 6: assert value
await step('Step 6: assert value', async () => {
  await expect(page.getByTestId('email')).toHaveValue('alice@example.org');
});
// Step 7: fill
await step('Step 7: fill', async () => {
  await page.locator('#password').fill(rbSecret('RB_SECRET_PASSWORD'));
});
// Step 8: select
await step('Step 8: select', async () => {
  await page.locator('#plan').selectOption('pro');
});
// Step 9: check
await step('Step 9: check', async () => {
  await page.getByRole('checkbox', { name: 'Remember me', exact: true }).filter({ visible: true }).first().check();
});
// Step 10: uncheck
await step('Step 10: uncheck', async () => {
  await page.getByRole('checkbox', { name: 'Remember me', exact: true }).filter({ visible: true }).first().uncheck();
});
// Step 11: hover
await step('Step 11: hover', async () => {
  await page.getByTestId('help').hover();
});
// Step 12: assert visible
await step('Step 12: assert visible', async () => {
  await expect(page.locator('span').filter({ hasText: /^\s*We\s+never\s+share\s+your\s+email\.\s*$/ }).filter({ visible: true }).first()).toBeVisible();
});
// Step 13: dblclick
await step('Step 13: dblclick', async () => {
  await page.getByTestId('counter').dblclick();
});
// Step 14: assert text
await step('Step 14: assert text', async () => {
  await expect(page.getByTestId('counter')).toHaveText('Double-clicked');
});
// Step 15: scroll
await step('Step 15: scroll', async () => {
  await page.locator('#terms').evaluate((element) => element.scrollTo(0, 120));
});
// Step 16: click - Sign in
await step('Step 16: click - Sign in', async () => {
  await page.getByTestId('sign-in').click();
});
// Step 17: waitForUrl
await step('Step 17: waitForUrl', async () => {
  await expect(page).toHaveURL('http://127.0.0.1:8000/recorder-welcome.html?plan=pro');
});
// Step 18: assert text
await step('Step 18: assert text', async () => {
  await expect(page.locator('#who')).toHaveText('Signed in as alice@example.org');
});
// Step 19: assert url
await step('Step 19: assert url', async () => {
  await expect(page).toHaveURL('http://127.0.0.1:8000/recorder-welcome.html?plan=pro');
});
