# Changes

## Unreleased

- A recorded flow runs through in every breakpoint (RB-090-76): a `waitForUrl` after a click
  (or key) of only some breakpoints first takes the others there -
  `if (![<clicked>].includes(<breakpoint>)) { await page.goto(<url>); }`, the clicked
  breakpoints being every click's and key's since the navigation before - so its expectation
  holds everywhere; a `fill` on a checkbox, radio or switch (by its role) is left out before a
  `check`/`uncheck` of the same control and written as `check()` otherwise; in the flow every step runs inside `await step('Step <id>: <kind>', async () => {
  ... });`, so the run's result names it. The spec keeps its form.
- The pace of an export (RB-090-66): with `options.pace: "recorded"` (the setting
  `automation.recorder.playbackPace`) a step whose recorded pause - its `atMs` minus the step
  before's, at most 10 s - is above 0 gets `await page.waitForTimeout(<ms>);` below its comment,
  in the flow and in the test; `"fast"`, the default, writes none.
- A `waitFor` step (the page menu's "Wait for element", RB-090-60) is
  `await <locator>.waitFor();` in the test and in the flow.
- Only a `fill` the recorder marked `secret` reads the environment; a placeholder anywhere
  else - typed, from a page, in a URL, an option or an assertion - stays text (RB-050-02
  review).
- A secret is read through `rbSecret('RB_SECRET_<NAME>')`, which stops the step with the
  variable's name when it is not set, instead of filling `''`; the flow's comment says that
  rBrowser's flow runner passes no environment variables to a flow yet.

## 0.1.0

The first version (RB-050-02): a recording as an rBrowser flow (`page`, `view` and `expect`
injected, no imports) and as a Playwright test with a `playwright.config.ts` holding one project
per breakpoint (viewport, device scale factor, mobile, touch, user agent, locale, colour scheme,
reduced motion, browser). Locators from the best selector candidate (`getByTestId`, `getByRole`
with its name, `getByText`, CSS), `onlyIn` steps guarded by the project's name or the view's
breakpoint, secrets as `process.env.RB_SECRET_<NAME>`.
