# Changes

## Unreleased

- Fixed (RB-090-86): the gallery's breakpoint and status filters apply. Their rules were
  `#bp-N:checked~.sheet ...`, but the radios sit in their fieldsets and the sheet is no sibling;
  they start at `<main>` now (`main:has(#bp-N:checked) .sheet ...`), still without a script.
- Changed (RB-090-86): the gallery's look follows the app's (teal and slate, light and dark by
  the system's choice); the app's own gallery window shows a Nuxt UI page instead, and the
  `index.html` is the export for the system browser.
- Added (RB-090-86): the setting `thumbnailWidth` (Settings > Capture > Sitemap; 160, 240, 320,
  480 or 640 CSS px, 320 by default) - how wide the contact sheet shows a shot; a gallery takes
  it when its capture ends. A shot still opens its large view on a click.

## 0.1.0

The first version (RB-050-05): finds a site's sitemaps (`robots.txt`, then `/sitemap.xml` and
`/sitemap_index.xml`) or reads a given sitemap URL, file or URL list; follows indexes and gzip
under limits for size, depth, sitemaps and pages, and redirects within their origin only; keeps
the site's own pages (another origin is left out and counted) and reads no sitemap on this machine
or the local network unless the site is local; picks
the pages by include and exclude patterns (glob or `re:`), `hreflang`, `lastmod`, one per template
path, every n-th and at most N, honours `robots.txt` when asked, and rewrites the production
origin to the environment's, leaving out the pages of other origins. Renders the gallery of a run - contact sheet, filters, large view,
error list - without a script and with every text escaped.

## Charge 5 (RB-050-05)

- The MCP prefix is `sitemapcapture` (tools `sitemapcapture_discover`, `sitemapcapture_gallery`):
  the core tools `sitemap_capture`, `sitemap_status` and `sitemap_cancel` hold `sitemap`.
- The core now runs the capture: the plugin finds the pages and renders the gallery, the bundled
  loader gives it the HTTP transport.
