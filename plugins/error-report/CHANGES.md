# Changes

## Unreleased

- Fixed (RB-090-23): each entry is a tree item of its category's tree, so its open or closed
  state belongs to a role a screen reader names.
- Changed (RB-080-84): the panel is built on Nuxt UI (Vue, `panel/dist`): Report now and Clear
  in the header, the views legend and "Reload the views first" in a row of their own below it,
  the categories with a count badge, each entry with its severity as a badge in words and with
  its icon, a dot per view (shape by engine), its count and "only in" as badges, and its details
  - source, request, the stack coloured by the one highlighter - under the row. Failures are one
  sentence, counts ICU plurals ("1 time", "Reloaded 1 view").
- Changed (RB-080-62): the panel is laid out with the panel kit - Report now and Clear in the
  header, the severity, count and views as badges, the empty state with an icon.

## 0.1.0

The first version (RB-080-09): one error report over every view. The host's capture of each
Chromium view (argument `$console`: exceptions, requests, console messages and the `Audits` issues
of CSP violations, mixed content and deprecations) is classified into seven categories -
`js-exception`, `http-not-found` (404, 410), `http-server-error` (5xx), `mixed-content-blocked`,
`mixed-content-passive`, `csp-violation`, `deprecation` - and deduplicated over the views: texts
without numbers and ids, URLs without cache busters, stacks without positions. An entry not every
view reported names its views ("only in views: mobile"). MCP `errorreport_get` (`reload`, `limit`)
and `errorreport_clear`; the check reports one rule per category for `checks_run`. Commands
`plugin.error_report.run` (`Ctrl+Shift+Space K E`: report.json, report.html and the findings,
optionally after a reload), `plugin.error_report.clear` (`Ctrl+Shift+Space K Z`) and
`plugin.error_report.panel` (`Ctrl+Shift+Space P E`). Settings in Checks > Error Report.
