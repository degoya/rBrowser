# Changes

## Unreleased

- Added (RB-094-02): the outline across the breakpoints. The rule `interactive-differs` finds an
  interactive element (the focus list) some views of a page show and others lack or hide; the
  rules over the breakpoints name every view missing the element (`missing`), the same in each
  view's run, so the findings panel merges them into one finding with its breakpoints;
  `outline.diff.elements` chooses the kinds compared (interactive elements, headings,
  landmarks). The rule `reading-order` (WCAG 1.3.2) finds, among the headings and interactive
  elements in source order, one shown before its predecessor - left of it on the line, or above
  it and not in a column to its right; right to left where the content reads so;
  `outline.readingOrder.tolerancePx` - both elements named, the other's selector in `data.other`;
  `focus-order-jump` names WCAG 2.4.3. The collector reads each heading's and focusable element's
  place in the document (`order`) and the writing direction (`dir`). MCP
  `outline_breakpoint_diff` (the table element x breakpoint) and `outline_reading_order`;
  the panel's tabs Breakpoints and Reading order; the commands `breakpoint_diff`
  (`Ctrl+Shift+Space B D`) and `reading_order` (`Ctrl+Shift+Space B R`) show those tabs.
  Acceptance page `tests/fixtures/site/outline/breakpoints.html`.
- Text size (RB-090-39): the panel follows the shell's text size - Windows' *Accessibility >
  Text size* while `ui.followWindowsTextSize` is on -, live; its bundle is rebuilt with
  panel-ui's root font size from `ui.textScale`.
- Changed (RB-091-06): the panel and `outline_get` show each finding at the level the person's
  `checks.rules` gives its rule (`checks.rules.outline.<rule>`), as the findings panel does; a
  rule turned off shows nowhere. The host answers the levels through `settings.get`
  (`checks.rules`), read only.
- Added (RB-090-63): "Check the outline of every view" in the page's context menu under Checks
  (`context_menu`).
- Added (RB-091-03): the panel analyses the structure - each tab's count of findings in the
  colour of the worst (from one `get` with `findingsOnly` until the tab is read), the findings
  at their entries with severity colour and icon, the findings at no entry above the tree
  (clickable to mark), and "Only problems". New rules `h1-missing`, `heading-first-not-h1`,
  `heading-empty`, `heading-image-no-alt`, `heading-too-long` (`outline.headingMaxLength`),
  `landmark-main-multiple`, `landmark-banner-multiple`, `landmark-contentinfo-multiple`,
  `landmark-banner-nested`, `landmark-contentinfo-nested`, `landmark-unique`,
  `content-outside-landmarks`, `focus-order-jump`, and over the breakpoints `heading-differs`
  and `landmark-differs`. Shared ids with `seo` and `a11y-axe` for the headings and landmarks
  (`heading-skip` now `heading-order`, `multiple-h1` `h1-multiple`). The collector reads a
  heading's name, full length and image without alt, the content outside every landmark and
  the fixed focus stops.
- Changed (RB-080-85): the panel is built on Nuxt UI - the sections as tabs (with their findings
  as a count) instead of a row of buttons, the entries as a tree with a dot per view and badges,
  Refresh and Clear marks as header actions set apart (Clear in red, only while something is
  marked), Check links in the Links tab, one status line with an icon, an empty section as such.
- Changed (RB-080-98): a check run over one Firefox or WebKit view answers that the view's engine
  cannot be checked (`plugins.error.not_available` with `engine`), so the findings panel shows
  the view skipped for its engine instead of "Nothing found"; a run over every view leaves such
  views out as before.

## 0.1.0

The first version (RB-070-03): the structure of the page in every view side by side - headings
(nested by level), landmarks, links, images (alt, rendered and natural size, lazy), form fields
(label and its source, type, autocomplete, required), the focus order, the accessibility tree
(CDP `Accessibility.getFullAXTree`, Chromium) and the custom selectors of
`outline.customSelectors` - with a dot per view and "only <views>" where an entry shows in some
views only (the navigation in a closed mobile menu). A click marks the entry in every view
(`DOM`/`Overlay`, as the findings panel does). The broken-link check requests the current
page's links (HEAD, GET fallback, redirects followed, never into the local network from a public
site) at `outline.linkCheck.rps` (at most four a second) within `outline.linkCheck.timeoutMs`.
Findings for `checks_run`: `heading-skip`, `multiple-h1`, `landmark-main-missing`, `link-broken`,
`link-empty`, `img-alt-missing`, `form-label-missing`; the last three share their ids with `seo`
and `a11y-axe`. MCP `outline_get`, `outline_check_links`, `outline_mark`; commands
`plugin.outline.panel` (`Ctrl+Shift+Space P O`) and `plugin.outline.check`
(`Ctrl+Shift+Space K O`). Settings in DevTools & Debugging > Outline.
