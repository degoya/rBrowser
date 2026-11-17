rBrowser shows one site at every breakpoint at once: a grid of views, each a real Chromium
page at its own size and emulation. You control the views the person sees.

Views and targets:
- `views_list` lists the views: their `id`, breakpoint, size, URL and loading state.
- Every view tool takes `target`: `"all"` (the default for navigation), a view id from
  `views_list`, or a breakpoint id such as `"mobile"` (every view showing it). An unknown
  target answers with the known views.
- `navigate`, `reload`, `back` and `forward` wait for the loads and answer each view's final
  URL; `settled: false` means a view was still loading at the timeout.

Project and settings:
- `project_info` says which project is open and whether its workspace is trusted. Trust is a
  person's decision in rBrowser; no tool grants it.
- `config_get` reads `.rbrowser/config.json`; `settings_get` and `settings_set` read and change
  settings. `settings_get` answers the settings window's categories, sections and keys (filter
  with `key`, `category`, `section` or `query`); every key has a deep link
  `rbrowser://settings/<key>` to show the person. Protected settings (the MCP server itself,
  trust, plugins, the sandbox) are refused. Shortcuts: `settings_set` with
  `shortcuts.bindings.<commandId>` and a list of bindings; reserved or taken ones are refused.
  The person sees every change you make.
- `breakpoints_detect` proposes breakpoints from the site's CSS; `breakpoints_apply` writes the
  chosen ones. `view_set_breakpoint` sets one view to a breakpoint or a free size,
  `emulation_set` changes a view's emulation, `layout_set` and `environment_set` switch the
  layout (a preset, focus, or the modes horizontal, split and canvas) and the environment;
  `canvas_place` moves a view on the canvas, `canvas_zoom` zooms it (1 = the active page at
  100 %, or `"fit"`). `view_pop_out` shows one view in a window of its own at its real size
  (zoom 1, still synced, same view id); `view_pop_in` puts it back.

Reading the pages:
- `snapshot` answers the accessibility tree of each view - the cheapest way to see a page.
- `console_logs`, `network_requests` and `errors` answer what each view reported since the
  server started (requests without headers or bodies).
- `evaluate` runs JavaScript in the page; it is off unless a person switched it on, and then
  answers an error that names the setting.
- `view_metrics` answers each view's Web Vitals (LCP, CLS, INP, FCP, TTFB) with their rating
  and the element behind them, its CPU share and memory, and whether it is throttled; a
  missing value says why (`na`).
- `status_messages` answers the status bar's last messages - what rBrowser told the person,
  errors first to look at after a failed action.
- `bookmarks_list` answers the project's bookmarks: `relative: true` is a path that follows the
  environment, `url` where it leads now (open it with `navigate`); `bookmarks_make_relative`
  turns the absolute ones that lie under an environment into paths.

Interacting and sync:
- `click`, `type`, `hover`, `press_key` and `scroll` act as real input in the views of `target`
  (the active view when absent) and answer per view; `sync_set` switches the synchronisation
  between the views and answers its state; `bus_send` sends a message on the page API's bus.
- `screenshot` captures views as PNG, `checks_run` runs the check plugins and answers their
  findings, `session_assign` and `session_clear` move views between sessions and clear one.

Changing the config: `config_set` writes one key of `.rbrowser/config.json`;
`breakpoint_add`, `breakpoint_update` and `breakpoint_remove` edit the breakpoints. Keys that
workspace trust guards (headers, network, plugins, extensions) are refused.

Flows: `flow_list` lists the project's Playwright flows (`.rbrowser/flows/*.ts`) and whether
the workspace is trusted; `flow_run` runs one in every chosen view at once and answers each
view's status, error line and screenshot. Flows are project code: an untrusted workspace is
refused (`flows.untrusted`) until a person trusts it in rBrowser.

Recording: `recorder_start` records one view's interactions (secrets as placeholders),
`recorder_stop` writes the recording (or pauses it), `recorder_export` turns it into a flow or a
Playwright test.

Visual regression: `visual_compare` diffs the views against their baselines and answers each
view's status; `visual_baseline_update` stores the views as the new baselines.

Firefox and WebKit views run in windows of their own; `engine_restart` restarts them.

Other tools: `devtools_open`, `devtools_inspect_at` (DevTools with the element at a point
selected; it opens them whatever `devtools.openOnInspect` says), `app_info`, `wsl_info`, `diagnostics`, `plugins_list`,
`project_open`.

Playwright and other CDP clients: `cdp_endpoint` answers the URL for
`connectOverCDP(url, { headers })` with its token (and the URL from WSL); a person switches the
proxy on, and `evaluate` too (`mcp.allowEvaluate`): the token is full control of the page, so
without it the answer is the error evaluate_disabled. Only the grid's views are targets. Never
call `setViewportSize` on rBrowser's pages:
the size of each view is its breakpoint and the call has no effect. Use `view_set_breakpoint`
instead.
