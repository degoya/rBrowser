// @description Every view opens its own page (the flow runner's fixture, RB-040-07).
const target: string = `${env}/${view.breakpoint}`;
await step('open', async () => {
  await page.goto(target);
});
expect(page.url()).toBe(target);
