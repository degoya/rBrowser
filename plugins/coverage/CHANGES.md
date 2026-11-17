# Changes

## Unreleased

- Text size (RB-090-39): the panel follows the shell's text size - Windows' *Accessibility >
  Text size* while `ui.followWindowsTextSize` is on -, live; its bundle is rebuilt with
  panel-ui's root font size from `ui.textScale`.
- Changed (RB-080-84): the panel is built on Nuxt UI (Vue, `panel/dist`) and reads at a glance:
  one row per file with one value - the bytes and share unused in every breakpoint that loads it,
  with a bar -, the largest lever first, a badge with the span when the breakpoints differ; the
  values per breakpoint open with the row; the views that could not be measured named once above
  the table; the type by content (a style sheet a dev server delivers as a script module is CSS);
  filter CSS/JS, search by address, the total below; the source in the detail coloured by the one
  highlighter with the unused parts marked.
- Changed (RB-080-98): a check run over one Firefox or WebKit view answers that the view's engine
  cannot be checked (`plugins.error.not_available` with `engine`), so the findings panel shows
  the view skipped for its engine instead of "Nothing found"; a run over every view leaves such
  views out as before.
- Changed (RB-080-62): the panel is laid out with the panel kit - icon buttons with tooltips
  (Start and Stop now say "measuring"), the status line with an icon, the files as a kit table,
  the file's detail as a card.

## 0.1.0

The first version (RB-080-07): unused CSS and JavaScript per breakpoint. CSS rule usage tracking
and V8's precise coverage run in every Chromium view at once - after a reload of every view
(page load: precise coverage before the reload, the rules the loaded page matches after it) or
from `start` to `stop` while the person interacts - and each file's used and unused units
(UTF-16, as DevTools counts them) are compared across the views. Findings for `checks_run` (from
the last measurement): `unused-file` (a file of at least `minBytes` with more than
`unusedPercent` unused in every view), `rule-partial` (a CSS rule only some breakpoints use, with
their names) and `function-unused` (a function no view ran, interaction runs only). A panel with
the table file x breakpoint, a detail with the rules, the marked source and the functions, and
the export into `.rbrowser/out/coverage/coverage.json` and `coverage.csv`. MCP `coverage_get`
(`mode: load|delta|last`), `coverage_start`, `coverage_stop`, `coverage_file`; commands
`plugin.coverage.run_load` (`Ctrl+Shift+Space K C`), `start` (`K I`), `stop` (`K X`), `export`
(`K J`). Settings in Checks > Coverage. Firefox and WebKit views are marked unavailable.
