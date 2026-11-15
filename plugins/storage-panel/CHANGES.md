# Changes

## Unreleased

- Fixed (RB-090-23): "Delete in all views" asks with the focus on Cancel and hands it back to
  the button after either answer.
- Changed (RB-080-84): the panel is built on Nuxt UI (Vue, `panel/dist`): the kinds as tabs, the
  filters as search fields, the grid a UTable with the views grouped under their session and the
  name column kept in place when the views scroll sideways; differing, missing and unread cells
  tinted and named in words; the editor a card with Nuxt UI fields (SameSite a choice, HttpOnly
  and Secure checkboxes) and the coloured value field; "Delete in all views" asks first, inline,
  with the number of views; pins with a switch each. Same calls, keyboard and texts; "{n} of {m}
  views written" with a plural form.

## 0.1.0

The first version (RB-070-01): the cookies, localStorage and sessionStorage of every view side by
side. The panel `storage` has a tab per kind, a row per cookie (domain, path, name, partition) or
key (origin, key) and a column per view, grouped by session (the breakpoints' `session` in
`.rbrowser/config.json`); cells that differ from the views' majority and entries missing in a view
they apply to are marked in colour and text, Firefox and WebKit views are greyed columns. A cell
opens an editor (Enter): save in the view, apply to all views, delete in one or all, pin. Cookies
come over CDP `Network` with every attribute, HttpOnly ones included; storage through a script in
the plugin's isolated world. Pins live in `.rbrowser/out/storage-panel/pins.json`: cookies are set
again over CDP at every document start, storage values by the page script `page/pins.js` before
the page's own scripts (from a record kept in the tab's sessionStorage); they are read and
applied only in a trusted workspace (a repository could commit the file). MCP: `storage_get` (names,
attributes, lengths and differences; values only with the protected `storage-panel.mcpValues`)
and `storage_set` (refused without it). Command `plugin.storage_panel.panel`
(`Ctrl+Shift+Space P A`); settings `pollMs`, `showHttpOnly`, `pinsEnabled`, `mcpValues`.
