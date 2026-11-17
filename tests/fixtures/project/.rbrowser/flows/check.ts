// @description Fails in the tablet view only (the flow runner's fixture, RB-040-07).
await step('check the breakpoint', async () => {
  expect(view.breakpoint).not.toBe('tablet');
});
