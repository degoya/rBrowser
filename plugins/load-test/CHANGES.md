# Changes

## Unreleased

- Fixed (RB-096-09): without a saved request the scenario editor says where requests are made
  and opens the API client panel; the saved requests are read again when the panel gets the
  focus back or a request list opens.
- Added (RB-096-07): the load test - your own server put under load on purpose by many virtual
  users sending the API client's saved requests: scenarios in `.rbrowser/api/load/*.json`
  (order or weights, think times, values per user, constant users or a ramp up, hold and down,
  a target rate, abort rules on the failure rate and p95), live numbers in the panel
  (requests per second, p50/p95/p99, failures by kind, users, a time chart), a report per run
  (`.rbrowser/out/load-test/`, JSON and HTML) and two reports compared. Only the project's own
  sites and this machine without asking, any other host after your confirmation; every run
  starts by your click on its summary; Stop (also in the activity bar) and closing rBrowser end
  every connection at once. MCP `load_reports`, `load_report`, `load_compare` (read only);
  commands `panel` (`Ctrl+Shift+Space P 5`) and `cancel` (`Ctrl+Shift+Space K 2`); settings
  `maxUsers` (50), `maxRps` (200), `maxDurationS` (300), `allowedHosts`.
