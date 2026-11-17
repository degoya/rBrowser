// hello-panel's panel in the e2e of the Nuxt UI panels (RB-080-87): the SDK's worked example. A
// click on Greet calls the tool `greet`; the stand-in answers it as the component does.
export default {
  title: 'Hello',
  tooltip: '#greet',
  answer: (call) => (call.tool === 'greet' ? { greeting: 'Hello, rBrowser!' } : {}),
  /** The table stays empty until a page says hello: the empty state shows. */
  check: async (frame, expect) => {
    await frame.locator('#greet').click();
    const empty = await frame.locator('#empty').isVisible();
    expect('hello-panel: shows its empty state and calls greet without an error', empty, empty);
  },
};
