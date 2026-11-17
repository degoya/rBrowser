# Changes

## Unreleased

- Text size (RB-090-39): the panel follows the shell's text size - Windows' *Accessibility >
  Text size* while `ui.followWindowsTextSize` is on -, live; its bundle is rebuilt with
  panel-ui's root font size from `ui.textScale`.
- Changed (RB-080-84): the panel is built on Nuxt UI (Vue, `panel/dist`). The messages are a
  `UTable` that draws only the rows in sight (virtualized; 3000 messages stay fluid), each with
  its level as a badge with icon and word, the count, the source and a dot per view; a stack
  opens below its message. Levels and views are toggle buttons that say they are on (pressed,
  check), the filter a search field, the target of the command line a choice of two; the command
  line and the values in the one highlighter's colours. A failure that is a code reads as a
  sentence; no capture and a cut list are said in the status line.
- RB-080-66: what rBrowser's plugins logged in a view (source `rbrowser`, the host's page-script
  log) is its own kind, shown as "rBrowser: <plugin>" instead of "unknown source", hidden unless
  `console.showRbrowserMessages` (off), and never a `console-error` finding of the page. A
  plugin's page script that failed (a stack) shows either way.
- `console.clearOnNavigate` works (Charge 10, core-fixes): the host's `$console` feed now carries
  `navigatedAt`, the newest time of a view's document before its main frame loaded a new one (a
  navigation or a reload, not a same-document one), and the console drops what is not newer.

## 0.1.0

The first version (RB-060-02): one console over every view. The host's capture of each view's
console messages, uncaught exceptions and failed fetch/XHR requests (argument `$console`) is
deduplicated - numbers and ids ignored, the source without its query - into one entry per
message with a dot per reporting view; the panel `console` filters by level, view and text,
shows stack traces and runs commands in every view (or the active one), equal results joined.
MCP: `sharedconsole_grouped` (with `clear`) and `sharedconsole_exec` (off until a person allows
it, `console.mcpExec`). The check reports `js-error` and `console-error` with their views.
Commands `plugin.console.panel` (`Ctrl+Shift+Space P C`), `plugin.console.clear`
(`Ctrl+Shift+Space D X`) and `plugin.console.exec` (`Ctrl+Shift+Space D E`).
