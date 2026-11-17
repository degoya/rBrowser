# Changes

## Unreleased

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
