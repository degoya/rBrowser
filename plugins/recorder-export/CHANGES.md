# Changes

## Unreleased

- Fixed (RB-094-10): the config's `baseURL` from the start URL carries no user info
  (`user:password@`), and the scheme counts in any case (`rb_util::url::origin_of`).
- The recording editor's steps (RB-092-01): a step switched off (`disabled`) is left out of every
  format, a pause set in the editor (`pauseMs`) takes the measured one's place (capped at 10 s
  like it), and the fixed wait `wait { ms }` is a `page.waitForTimeout` at every pace, at most
  10 s.
- The pointer's way of a step (RB-090-87): a step the recorder kept with its way (`path`, CSS px
  of the recorded view's viewport with the time of each point) moves the pointer along it with
  `page.mouse.move` before its action - at the `recorded` pace with the recorded times, at
  `fast` through the points without waits -, only in a view of the recording's size
  (`view.width`/`view.height` in the flow, `page.viewportSize()` in the spec). The way's time is
  taken from the step's pause, so a run takes no longer and the `pauses` stay the one rule. A
  recording without ways exports as before.
- A hover rests before the step after it at the `recorded` pace (RB-090-84): the recorder
  reports a menu's hover at the pointer-down of the click into its submenu, so the two carried
  one time and played back to back. A hover followed sooner than 500 ms now waits that long
  before the next step, the time taken from the pause before the hover - the run takes no
  longer than the recording did. Each step but the first waits at least `options.minPauseMs`
  (`automation.recorder.minPauseMs`, 0 by default), and in the flow the pause waits first inside
  the step's `step(...)`, so the step a run reports is the one waiting. The internal format
  `pauses` (not offered as an export) answers the pause before each written step, so the host's
  run budget and the Recorder panel read the one rule from here.
- Locators that hold in every breakpoint (RB-090-81): a role or text locator of the element ends
  in `.filter({ visible: true })` and `.nth(visibleNth)` - the match's index among the visible
  ones, recorded by the agent - or `.first()` for the first, a unique one too, so each view
  takes the visible match the person picked and a name two elements share is no strict-mode
  error (Playwright 1.51 or later). A later match recorded without `visibleNth` (a recording made
  before, a hidden element) keeps `.nth(n)`. Test ids, shadow hosts and the CSS path stay as
  they were.
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
