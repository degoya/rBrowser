# Changes

## Unreleased

- Faster, and sections as filters (RB-110-16): the search for todo files skips hidden folders
  below the root and the folders the project's `.gitignore` files ignore (the root's and nested
  ones; a rule it does not understand never leaves out more than git), without listing them -
  a folder a `todo.files` glob names outright is still searched, a todo file git ignores is
  still found; the manifest reads `**/.gitignore` for it, and the log names the folders listed
  and skipped. The panel asks the window to keep it (`keep`), so a switch back shows the list
  at once and only Reload searches again. The "Add to" choice cuts the list to its section (a
  heading with everything below it up to the next heading of its level, a project with its
  block), shown as a filter that clears with one click. H1 to H6 get six distinct sizes
  (`text-2xl` to `text-xs`), weights and spacing. `add` with a `text` that is a Markdown heading
  (`## Sprint`) adds that heading line without a box - after a heading's whole section, at the
  end of a project's block in its children's indent, or at the top level before `Archive:` - and
  the panel chooses it next.
- Markdown headings (RB-110-15): a line of one to six `#`, white space and a text (up to three
  columns of indent, a closing run of `#` dropped, as CommonMark reads it) is a heading of its
  level, not a note - `list` answers `kind: heading` with `level`, the text without the `#`s and
  `raw` the whole line. `#tag`, `#####` alone and seven `#` stay notes. The panel shows headings
  bold and sized by level and projects as headings; a heading is edited as its whole line, moved
  and deleted as its own line, and `add` with `under` a heading puts the todo after the
  heading's own lines (before the next heading, a less indented line or `Archive:`).

## 0.1.0

The first version (RB-110-13): the project's `.todo` files in the Todo+ format
(`fabiospampinato.vscode-todo-plus`) as checklists. Projects (`Name:`), tasks (open `☐`, done
`✔`, cancelled `✘` and the other symbols Todo+ reads), tags and notes are read and written line
by line, so a file nothing changed in stays byte for byte the same - line endings, byte-order
mark, tabs and formatting included. Checking writes `@done(<time>)`, cancelling
`@cancelled(<time>)` (setting `todo.timestampFormat`, Todo+' `YY-MM-DD HH:mm` by default);
Archive moves the finished tasks into `Archive:` with `@project(...)`. The panel checks, adds,
edits, deletes and moves todos, filters by tag and reads the files again every
`todo.refreshSeconds`; MCP `todo_list`, `todo_add`, `todo_toggle`, `todo_edit`, `todo_archive`.
`list` answers in pages that fit a panel's 64 KiB message - the files' heads, then one file's
items from `from` with `next` and `rev` - and a line too long for one answer is shown cut.
Files are written with the capability `project-write`: only `.todo` files, only in a trusted
workspace, atomically and only over the bytes read - a file changed elsewhere is read again,
never overwritten.
