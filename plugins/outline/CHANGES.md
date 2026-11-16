# Changes

## Unreleased

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
