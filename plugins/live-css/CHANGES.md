# Changes

## 0.1.0

The first version (RB-094-04): a CSS draft that applies at once in every view, as one constructed
stylesheet per view that the page script adopts in the plugin's isolated world
(`document.adoptedStyleSheets`, filled with `replaceSync`: CSS only, no markup), at the start of
every document (`live-css.applyOnNavigate`, on by default) and at every change. The draft is rule
blocks, each for every view or for chosen breakpoints of the grid - a scoped block applies inside
the media query of their widths (from a breakpoint's width to one below the next one's), so what
the views show is what *Copy as media query* gives. On and off without losing the draft; it is
kept in `.rbrowser/out/live-css/draft.json`, the whole draft as one stylesheet in `draft.css`,
both read and written only in a trusted workspace. A block's braces must balance, so it cannot
leave its media query; at most 32 blocks and 30 000 bytes of CSS. The panel `editor` edits the
blocks with the CSS coloured as it is typed and the breakpoints as a checkbox group, and hands
each edit to the views a moment after the last keystroke. MCP: `livecss_set` (only while
`mcp.allowEvaluate` is on: CSS can read page data out) and `livecss_clear`. Commands `plugin.live-css.panel` (`Ctrl+Shift+Space P Q`) and
`plugin.live-css.toggle` (`Ctrl+Shift+Space D L`, said in the status bar).
