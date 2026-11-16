# Changes

## Unreleased

- Fixed (RB-090-23): Delete asks with the focus on Keep; kept, Delete has it back, deleted, the
  name field.
- Changed (RB-080-84): the panel is built on Nuxt UI (Vue, `panel/dist`): the snippets as a
  keyboard list beside the editor card (name, code coloured as it is typed, where a run goes as a
  choice), Run as the card's main action, Delete asks inline first; each run a card with a line per
  outcome (a badge for done or failed, the views, the value coloured, a failure in words, never a
  bare code), the stack on request. The panel no longer takes the focus by itself (the sandboxed
  frame refuses focus without a user's action).

## 0.1.0

The first version (RB-070-04): JavaScript snippets run in every view at once or in the active
one, equal results joined with their views. Three sources: the person's `snippets.library` (user
settings, protected from MCP), the project's `.rbrowser/snippets/*.js` and the panel's saved
snippets (`.rbrowser/out/snippets/library.json`) - the latter two only under workspace trust. A
header (`// @name`, `@description`, `@world main|isolated`, `@target all|active`, `@timeoutMs`)
sets each snippet's world, target and timeout (100-9000 ms, default 5 s; an endless loop is
cut). The panel `snippets` lists, edits, saves and runs them. MCP: `snippets_list`,
`snippets_run` and `snippets_save` (the latter two only while `mcp.allowEvaluate` is on).
Commands `plugin.snippets.panel` (`Ctrl+Shift+Space P J`) and `plugin.snippets.run_last`
(`Ctrl+Shift+Space D J`).
