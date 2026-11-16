# Changes

## Unreleased

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
