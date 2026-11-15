# PWA check fixtures (RB-080-08)

`pwa-ok/` and `pwa-broken/` are served by any static server (the fixture server serves
`tests/fixtures/site/` at `http://localhost:8080/`; `.webmanifest` must go out as
`application/manifest+json`). `plugins/pwa-check/tests/fixtures.rs` serves the same files through
a fake host at `http://localhost:8080/<folder>/`, with what the collector reports for each page in
`plugins/pwa-check/tests/fixtures/<folder>.json`, and holds the findings to the snapshots
`plugins/pwa-check/tests/snapshots/<folder>.txt`.

- `pwa-ok/`: a complete manifest, icons of 192 and 512 px (one maskable), a service worker that
  caches the shell and serves it cache first. No error, no warning.
- `pwa-broken/`: no 512 px icon, `icons/icon-96.png` answers 404, `start_url` `/elsewhere/`
  outside the scope `/pwa-broken/` (and answering 404), no service worker, no maskable icon, no
  `theme-color` meta tag and no `apple-touch-icon`.
