# Changes

## Unreleased

- Fixed (RB-094-13): a view without its source names the browser's reason, not only the
  host's code.
- Added (RB-094-06): the rendered DOM as a second source - `html-validate.source` (`raw`, the
  default, or `rendered`) and the argument `source` of `html_validate`. The DOM is serialized in
  the view's isolated world (doctype and document element, at most 1 MiB); its findings name the
  element by selector, as the lines of a serialization mean nothing.
- Added (RB-094-06): the project's rule file `.rbrowser/htmlvalidate.json`, applied only in a
  trusted workspace: the levels (`off`, `warn`, `error`, 0 to 2) and options of the plugin's
  html-validate rules, over `html-validate.disabledRules`. Only `rules` is read - a file naming
  plugins, presets (`extends`), elements or transformers, which can load code, is refused, as is
  a rule the plugin does not offer; at most 64 KiB, 128 rules, plain options. The answer of
  `html_validate` says whether it was applied (`projectRules`). New capability `project-read`.
- Added (RB-094-06): `no-unknown-attributes`, off by default - html-validate 11.18.0 misses valid
  attributes (`loading` on `<iframe>`, `viewBox` on `<svg>`) - for a project to switch on.
- Added (RB-094-06): every message with a line names its element's selector - a Nu message, a
  stray end tag - read with html-validate's parser, so the findings panel highlights it in the
  view.

## 0.1.0

The first version (RB-090-97): the page source as the view loaded it (`Page.getResourceContent`
of the main frame, at most 1 MiB) validated like the W3C validator does - the markup, not the
rendered DOM. Offline by default: html-validate 11.18.0 (MIT, bundled) runs in the view's
isolated world with the `html-validate:standard` rules plus `missing-doctype` and
`no-missing-references`. With `html-validate.engine` = `nu` the source goes to the Nu HTML
Checker at `html-validate.nuAddress` (a user setting, empty by default) and its errors,
warnings and information messages become the rules `nu-error`, `nu-warning`, `nu-info`; a
failure of that address fails the run with its reason. Each message is a finding with rule,
line, column, the source line and the views it was found in. Settings under Checks > HTML
validation (engine, Nu address, rules off, lowest severity), the MCP tool `html_validate` and
the command `Ctrl+Shift+Space K V`.
