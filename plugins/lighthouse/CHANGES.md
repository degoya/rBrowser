# Changes

## Unreleased

- Added (RB-091-02): the tool `reports` (MCP `lighthouse_reports`: `list`, `open`, `compare`),
  the commands `open_report` (`Ctrl+Shift+Space K R`) and `delete_run`
  (`Ctrl+Shift+Space K D`), the settings `reportWindow` and `keepRuns` (protected, 0 = keep every run, the default); each report opens in
  rBrowser's report window, the panel lists the runs with a delete that asks first and compares
  two runs of a breakpoint by scores and metrics; a run's summary carries its metrics.
- Fixed (RB-090-23): a score tile is a group named by its category, score, rating and change.
- Changed (RB-080-98): a check run over one Firefox or WebKit view answers that the view's engine
  cannot be checked (`plugins.error.not_available` with `engine`), so the findings panel shows
  the view skipped for its engine instead of "Nothing found"; a run over every view leaves such
  views out as before.
- Changed (RB-080-83): the panel is built on Nuxt UI - the cards, tiles, badges and notes are
  Nuxt UI's; the measured page opens in the browser from the panel (`open_link`); the findings
  count is plural-aware.
- Changed (RB-080-62): the panel is laid out with the panel kit - a card per breakpoint with
  its scores as tiles coloured by their rating, the change since the last run as an arrow, Run
  and Cancel in the header, the reports with a file icon.
- Fixed (RB-080-58): an installed Google Chrome or Microsoft Edge in the machine-wide program
  folders is found again; the browser can be named in Settings > Engines > Node runtime >
  Lighthouse browser (`engines.lighthouseBrowserPath`), and `errors.no_browser` says so.

## 0.1.0

The first version (RB-060-07): Lighthouse (13.6.0, in the Node sidecar) for every Chromium
breakpoint with its own emulation - size, device pixel ratio, mobile from the breakpoint's
`mobile` or `touch` (else a width below 768 px), its user agent - one breakpoint after another in
a headless browser of the sidecar's own, so the views never navigate. The HTML and JSON reports
and a summary go to `.rbrowser/out/lighthouse/<run>/`, `latest.json` keeps the newest summary
with the previous run's scores. Failed audits become findings (one rule per audit,
`lighthouse.<audit-id>`), merged across the breakpoints by element. Firefox and WebKit
breakpoints are skipped with the reason. Categories from the project's
`checks.lighthouse.categories`, throttling from `lighthouse.throttling`; the MCP tools
`lighthouse_run`, `lighthouse_status`, `lighthouse_cancel`, the command `Ctrl+Shift+Space K L`
and a panel with the score table.
