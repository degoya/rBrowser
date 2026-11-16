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
await page.goto('http://127.0.0.1:8000/recorder.html');
// Step 2: click - Open the mobile menu (only in mobile)
if (['mobile'].includes(view.breakpoint)) {
  await page.getByRole('button', { name: 'Menu', exact: true }).click();
}
// Step 3: assert visible (only in mobile)
if (['mobile'].includes(view.breakpoint)) {
  await expect(page.getByRole('link', { name: 'About', exact: true })).toBeVisible();
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
await page.getByRole('checkbox', { name: 'Remember me', exact: true }).check();
// Step 10: uncheck
await page.getByRole('checkbox', { name: 'Remember me', exact: true }).uncheck();
// Step 11: hover
await page.getByTestId('help').hover();
// Step 12: assert visible
await expect(page.getByText('We never share your email.', { exact: true })).toBeVisible();
// Step 13: dblclick
await page.getByTestId('counter').dblclick();
// Step 14: assert text
await expect(page.getByTestId('counter')).toHaveText('Double-clicked');
// Step 15: scroll
await page.locator('#terms').evaluate((element) => element.scrollTo(0, 120));
// Step 16: click - Sign in
await page.getByTestId('sign-in').click();
// Step 17: waitForUrl
await expect(page).toHaveURL('http://127.0.0.1:8000/recorder-welcome.html?plan=pro');
// Step 18: assert text
await expect(page.locator('#who')).toHaveText('Signed in as alice@example.org');
// Step 19: assert url
await expect(page).toHaveURL('http://127.0.0.1:8000/recorder-welcome.html?plan=pro');
