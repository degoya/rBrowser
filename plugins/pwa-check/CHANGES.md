# Changes

## Unreleased

- Changed (RB-080-85): the panel is built on Nuxt UI - the five sections as tabs (findings with
  their count first), the findings as a table with the severity as a badge and a click that opens
  the finding's tab at its place, the manifest and the service worker as tables, the icons as
  cards, the page as a select, skipped views and a shortened answer as one note.
- Changed (RB-080-98): a check run over one Firefox or WebKit view answers that the view's engine
  cannot be checked (`plugins.error.not_available` with `engine`), so the findings panel shows
  the view skipped for its engine instead of "Nothing found"; a run over every view leaves such
  views out as before.
- Changed (RB-080-62): the panel is laid out with the panel kit - Check again as the main
  action, sections with icons, kit tables, a finding's severity and an icon's state as badges.

## 0.1.0

The first version (RB-080-08): the PWA check of the page each Chromium view shows, each URL once
and its findings copied to every view showing it. The collector (`src/collect.js`, through
`evaluate` in the plugin's isolated world, read only) gives the manifest link, `theme-color`,
`apple-touch-icon`, the secure context and the service worker registration; the manifest, its
icons, the start URL and the service worker's script come over `http` from the page's own origin
only (another origin is named, not requested; redirects followed within the origin; at most 20
icons; `pwa-check.timeoutMs`); Chromium's verdict is CDP `Page.getInstallabilityErrors`. Rules:
`manifest.*` (linked, loads, JSON content type, valid, name, start_url, app display, 192 and 512
px icons, the recommended members), `fetch.other_origin`, `icons.*` (loads, is an image, its
declared size and type, sniffed from the header), `start_url.*` (within scope, answers 200),
`sw.*` (secure context, registered, active, controls the page, a fetch handler),
`offline.not_cached` (the Cache Storage holds the start URL; `pwa-check.offline.enabled`),
`install.error` (each of Chromium's errors) and `meta.*` (theme-color, apple-touch-icon). The panel
shows the manifest as a table, every icon with its preview (the maskable safe zone drawn), the
service worker, the installability errors and the findings. MCP `pwa_check` and `pwa_icon`;
commands `plugin.pwa_check.check` (`Ctrl+Shift+Space K M`) and `plugin.pwa_check.panel`
(`Ctrl+Shift+Space P W`). Settings in Checks > PWA.
