# Changes

## Unreleased

- Added (RB-096-08): Open in API client and Save as request on a request, Save shown as
  requests over the filtered list, Open in API client on an endpoint; the core reads the
  headers and the body and makes every secret a variable (capability `api-client`). MCP
  `networkpanel_api_open`, `networkpanel_api_save`; commands `api_open`
  (`Ctrl+Shift+Space D G`) and `api_save` (`Ctrl+Shift+Space D K`).
- Fixed (RB-094-10): a saved session masks the app's one list of secret URL parameters, now
  with signed-URL credentials (`X-Amz-*`, `X-Goog-*`), `jwt`, `ticket` and SAML messages, in
  the fragment too, and an address's user info.
- Text size (RB-090-39): the panel follows the shell's text size - Windows' *Accessibility >
  Text size* while `ui.followWindowsTextSize` is on -, live; its bundle is rebuilt with
  panel-ui's root font size from `ui.textScale`.
- Changed (RB-080-84): the panel is built on Nuxt UI (Vue, `panel/dist`): the tabs as `UTabs`,
  the requests as a `UTable` that draws only the rows in sight (virtual scrolling, so the list
  stays fluid at the plugin's limit), one tab stop with arrows, Home, End and Enter as before;
  the marks `partial` and `differs` as badges in words, the views as dots shaped by engine; what
  the list cannot show as notes above it, each once; the switches for mocks, cache and
  breakpoints say their state in words and as pressed; the stage and the mock's action as a
  choice of two, a breakpoint's views as a choice of the grid's views; the confirmation of Clear
  site data inline; failures as sentences, counts as plurals; headers and bodies in the one
  highlighter, the text fields coloured as you type.

## 0.3.0

The endpoint analysis (RB-096-06): the tab Endpoints groups the capture by method, origin and path
with ids as placeholders - a segment of digits `{id}`, a UUID `{uuid}`, a hex hash or a
random-looking token `{hash}`, the person's patterns (`endpoints.patterns`, `/api/products/{slug}`)
first - and shows per endpoint the calls, failures, median and p95 of duration and waiting time,
sizes transferred and decoded, statuses, `Cache-Control`, validators and compression, per view
too; Show the requests filters the Requests tab by the endpoint (`list`'s new `endpoint`). Nothing
is sent. The plugin is a `check-plugin` too: `endpoint.duplicate`, `.n-plus-one`,
`.uncompressed`, `.no-cache`, `.slow` and `.breakpoint-calls` over each view's current page, the
person's rule levels applied; command `endpoints_check` (`Ctrl+Shift+Space K 7`) reports them,
command `endpoints` (`Ctrl+Shift+Space P 7`) opens the tab. Tools `endpoints` (masked URLs, the
hand-over `apiClient` for the API client) and `endpoints_check`; settings `endpoints.resourceTypes`,
`.patterns`, `.slowMs`, `.largeBytes`, `.nPlusOne`, `.breakpointDifference` (Network > Endpoints);
capability `findings`.

## 0.2.0

The HTTP debugger (RB-080-37): tabs Breakpoints, Compose and Sessions. Breakpoints hold a
request (method, URL, headers, body) or a response (status, headers, body) of the views named,
editable, with Continue, "continue for all views" and Abort; a held request goes on unchanged
after `breakpoints.timeoutMs`. Compose sends a request of your own from a view's context
(cookies, user agent, headers), Repeat sends a captured one again, up to 10 times in a row; the
results list status, headers, body and time. New rule kinds `mapLocal` (a folder or file below
the project or `mapLocalRoots`) and `mapRemote` (host, port, path prefix), and rewrites with
request and response header operations, query parameters and a body replacement under
`rewrite.maxBodyBytes`. Sessions save the capture as HAR with the view, breakpoint and difference
marks in `.rbrowser/out/network-panel/sessions/` and open for display. Cookies and tokens show
masked. Tools `debug`, `breakpoint_add`, `_remove`, `_enable`, `breakpoints_toggle`, `continue`,
`abort`, `compose`, `repeat`, `session_save`, `session_open`, `sessions_list`; command
`breakpoints_toggle` (`Ctrl+Shift+Space P R`); settings `breakpoints.timeoutMs`,
`allowComposeFromMcp`, `mapLocalRoots`, `rewrite.maxBodyBytes`.

## 0.1.0

The first version (RB-070-02): the requests of every view in one list. The host's capture
(argument `$console`) is grouped by method and URL; a request only some views loaded (`srcset`)
is marked `partial`, one the views saw with another status, a failure or a size more than 5 % and
256 bytes apart `differs`. The panel `network` filters by text, type, status and differences,
shows a request per view with its body and makes a mock from it. Mocks, blocking and rewrites come
from `.rbrowser/mocks/*.json` and the plugin's own `mocks/` (rules added over MCP, HAR imports),
only under workspace trust, and from the setting `block` always; the host asks for them (`@rules`,
capability `network-rules`) and applies them through CDP `Fetch`. HAR 1.2 export (merged, per
view, with the bodies the views hold) and import as replay mocks; the session's mocks and cache
switches and a confirmed "clear site data". MCP: `networkpanel_list`, `_body`, `_mocks`,
`_mock_add`, `_mock_remove`, `_mock_enable`, `_mocks_toggle`, `_cache_toggle`, `_har_export`,
`_har_import`. Commands `panel` (`Ctrl+Shift+Space P N`), `mocks_toggle` (`P M`), `har_export`
(`P H`), `cache_toggle` (`R N`), `site_data_clear` (`R D`).
