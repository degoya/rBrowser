# Changelog

All notable changes to rBrowser are documented in this file. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). `scripts/set-version.sh` cuts
`[Unreleased]` into a version section at every version bump.

## [Unreleased]

## [0.9.9-beta.1] - 2026-11-26

### Added

- The Windows MSI is built in the public CI and the release (RB-099-27): WiX 5 pinned
  (`RB_WIX_VERSION`, installed by `scripts/ci-tools.sh wix`), `scripts/package-msi.sh` builds the
  MSI from the package ZIP on `windows-2025`, `ci.yml` installs it per user, runs
  `rbrowser.com --version` and uninstalls it, and `release.yml` publishes
  `rbrowser-<version>-windows-x64.msi` beside the ZIP, listed in `SHA256SUMS`.
- A video or audio a view cannot play names the reason (RB-099-29): when a `<video>`, `<audio>`
  or `<source>` fails on a format this build has no codec for (MP4 with H.264 or AAC, HEVC,
  QuickTime), the status bar says which format is missing and that WebM (VP9, AV1) and Opus
  play, once per page and format, with a button to the handbook's *Media formats* page; an Info
  finding `media.unsupported-format` names the source and the element for the findings panel
  and MCP. The sync agent reports the failure (message `media`); no setting.

### Changed

- A check a person runs shows its result (RB-099-25): a plugin check from the palette, a menu or
  its shortcut ("Validate the HTML of every view") and "Run all checks" open the findings, a dock
  folded to its rail unfolded, and a plugin check's end in the status bar names how many it
  found ("nothing found" for none). MCP's `checks_run` keeps the dock as it is.
- The canvas is the built-in layout mode (RB-099-26): a project without a start preset - a new
  one from `rbrowser init`, one without any config - opens in the canvas, which places the views
  itself until they are moved; a preset without `mode` is a canvas preset unless it has a grid
  of its own (`layout`), which stays a grid, and a config that names a mode keeps it. The setup
  wizard preselects the canvas; its grid choice starts the project in a grid preset over every
  view (`layouts.default`).
- The screenshot editor opens, annotates, redacts, crops, saves, copies and exports every image
  the capture writes - up to 65535 pixels a side and 256 megapixels instead of 16384 and 25 -
  rendered in bands at full resolution; the panel shows an image too large for a canvas as a
  preview at 1:2, 1:3 or 1:4 (RB-099-24). The plugin contract gains `project-range` (a file
  larger than one `project.read`, read in ranges) and `out-stream` (a file of `out.write` in
  parts, renamed into place at the commit); a plugin that imports neither links as before.

### Fixed

- Full-page screenshots show lazily loaded images (RB-099-28): before every full-page capture
  (context menu, commands, MCP `screenshot`, visual regression, sitemap capture) the page is
  scrolled to its end one viewport at a time, so `loading="lazy"` images and
  IntersectionObserver loaders below the fold load, then back to where it was; the images get up
  to 4 s, then the page is measured again and captured. Images still loading at the deadline do
  not stop the screenshot - its answer says `imagesPending: true`. The other views do not follow
  the scrolling, and the captured view keeps its scroll position.

## [0.9.9-pre.3] - 2026-11-26

### Added

- The final Windows package (RB-099-01): `scripts/package-windows.sh --msi` also builds an
  optional per-user MSI (`scripts/package-msi.sh`, WiX 5 on Windows; installs to
  `%LOCALAPPDATA%\Programs\rBrowser` without admin rights, with a Start menu entry, and keeps the
  user data on uninstall). The ZIP is reproducible - two runs on one commit write the same
  SHA-256, listed in `artifacts/windows/SHA256SUMS` - ships only the CEF locales en-US, de, es
  and fr (about 48 MB less unpacked), and is held to a content list before it is written.
- `scripts/e2e-windows.sh` checks the acceptance criteria K1 to K12, the scroll sync fixture and
  the milestone smokes against the Windows package, staged portable in its own folder; the cases
  are TypeScript in `tests/e2e/` (MCP through `rbrowser.com mcp`, CDP through the token proxy, a
  fixture server that counts POSTs and stamps reloads), K1, K3 and K11 through `scripts/perf.sh`;
  `report.json` and `report.md` hold every value with its threshold; `--only`, `--native`,
  `--list`, `--keep`, `--allow-missing`. The first green run on the reference machine is pending
  (RB-099-02).

## [0.9.9-pre.2] - 2026-11-26

### Added

- Licence texts of every shipped component (RB-099-12): each package (Windows, Linux, macOS)
  carries `THIRD-PARTY-LICENSES.txt` beside `NOTICE`, the full licence texts with the copyright
  lines of every Rust crate and npm package it ships (`scripts/licenses.sh`; a package without a
  licence file gets its licence's standard text, one without any fails the preflight); the three
  `NOTICE` files come from one source, `packaging/NOTICE.in` (macOS gains the missing entries);
  one copyright holder everywhere; the About tab shows the GPL's copyright and no-warranty notice,
  and its licence list no longer names `sentry`/`@sentry/*`, which the packages leave out; the
  framework-devtools plugin carries `panel/dist/THIRD-PARTY-LICENSES.txt`.
- The LGPL source offer and Google's adb terms (RB-099-13): every `NOTICE` names the exact CEF
  version, commit and Chromium version of the package (filled from the CEF pin, so a CEF update
  cannot leave them stale), their source addresses and a written offer, valid three years, of
  the complete corresponding source of the CEF runtime's LGPL components; the Windows `NOTICE`
  lists adb among the downloads on request. "Tools: download adb" now shows Google's Android SDK
  terms in the toolbar strip, with a link to them, and downloads only after "Accept and
  download"; Cancel downloads nothing.
- Native Windows tests for the security code (RB-099-17): the private folder's and the instance
  pipe's DACL read back, the user SID, the job object, the pipe's first instance and the
  Credential Manager store.
- Privacy and network transparency (RB-099-14): the handbook's new pages *Network connections*
  (every connection rBrowser makes, when, what goes out and the setting that switches it off) and
  *Data protection notes* (what is kept where and how to delete it, HAR content, the portal over
  plain HTTP, the presentation through Cloudflare, what an MCP agent passes to its model
  provider); the public README and the package repository's page link the imprint and privacy
  policy, the README adds a privacy section and a trademark notice; the DPIA threshold
  assessment is `docs/privacy/dpia-threshold.md`. The presentation's consent names Cloudflare,
  Inc. (USA) and asks to present no real personal data, and the README asks for a GPL-compatible
  plugin licence instead of promising "any licence".

### Changed

- The MCP interface is smaller and says more (RB-099-16): every core tool carries the MCP
  annotations `readOnlyHint`, `destructiveHint`, `idempotentHint` and `openWorldHint`; the
  descriptions are a third shorter, so `tools/list` shrinks although the annotations join it;
  the 15 tools without arguments refuse any; every `target` says its default by one rule (`"all"`
  for reading, navigating and runs over the views, the active view for tools that act in a page
  as a person would); every failure code is dotted (`area.reason`: `evaluate.disabled`,
  `args.invalid`, `config.key_protected`, `script.disabled`, ...) and, until 1.0, an answer whose
  code replaced a flat one carries it as `legacyCode` (in code mode too: a caught `ToolError` has
  `error.legacyCode`). `rb_ipc::PROTOCOL_VERSION`, never checked,
  is gone from Rust and TypeScript.
- The shell pages load less (RB-099-20): the settings keys' labels, descriptions, help texts and
  option names - 41 % of `locales/en.json` - leave the chunk every page parses and load with the
  pages window only, so the toolbar, the status bar and the canvas board each load about 163 kB
  less script (the shared chunk 521 to 357 kB); `scripts/check.sh --preflight` holds every page
  to its bundle budget (`ui/bundle-budget.json`, `scripts/bundle-budget.sh`).
- Tests wait on what they check instead of a fixed sleep (RB-099-18): positive checks poll
  their condition, absences wait behind a barrier or on virtual time (agent, sidecar, ui and 32
  Rust sites), the shell's query allowlist is built by a tested pure function and the CDP
  proxy's order promise names the target check, which a test holds.
- The texts say one thing one way (RB-099-22): every command title follows "Show or hide ..." or
  "Switch ... on or off" in sentence case, core and plugins, and the group D is "DevTools &
  Debugging" like its settings category; the catalogues write the ellipsis `…`, settings paths
  with ` › `, real plurals instead of "(s)" ("1 plugin update was installed"), "When off" and
  "If empty" in help texts and British "analyse"; the theme's system option and the first run's
  folder hint fit every platform; the status statements of the README and the roadmap name the
  released beta. The i18n lint holds the title pattern and the typography; the generated wiki
  references and the public wiki export carry no job id (the export refuses one).

- The loop warning of RB-099-07 names its cause (RB-099-07): every time the window re-attaches
  its views - a grid sent again, the DevTools dock or the plugin panels' dock laid out anew - is
  counted with what caused it (the place that rebuilt the grid, the footer that changed its
  height with the room it was fitted to, a view that left or entered the grid, the text size),
  and more than ten in a second are logged once a minute with those causes counted.
- Less work on the UI thread and in step (RB-099-19): the shortcut editor's changes write the
  user settings off the UI thread, on the settings' lane and under their one-change lock, so a
  shortcut and a setting changed at the same moment both survive; each view applies its size
  and emulation under a lock of its own instead of one for all views (only the geolocation
  grants of a browser context still take turns), so a resize no longer sends eight views'
  metrics one after the other; the DevTools window's bounds are saved off the UI thread;
  DevTools and the time control catch up with closed views and the active view after missing
  events; CDP answers are no longer copied, and the CDP writer counts its waiting bytes and
  warns once past 32 MiB.

### Fixed

- A presentation's hidden paths that name a marker - a rule beginning with `.` or `__`, such as
  `/.git`, `/.env` and the editor launches - now match at every path segment, not only from the
  root (RB-099-15): `/sub/.git/config`, `/sub/.env` and `/app/__open-in-editor` answer 404 like
  their root forms, also percent-encoded; other rules such as `/admin` and `/@fs/` stay prefixes
  from the root, so a Nuxt app's modules under `/_nuxt/@fs/` still load. The unused sitemap HTTP transport, which skipped the
  plugins' public-address check, is gone, and the threat model names the three credential stores
  and an agent's reach into the LAN through `navigate` as a residual risk.

- Screen readers hear the sign-in bar and the sitemap start page (RB-099-21): the sign-in bar is a
  named form instead of a non-modal alert dialog, says a problem at once and marks the field it
  is about, and lets a password manager fill user name and password; the sitemap start page says
  a failure and a refused start at once and moves the focus to "Find the pages again" instead of
  dropping it; its page tree says a row's choice once, not as selected and checked. The bug
  report preview, the sitemap gallery and the plugin catalogue's changed-key warning say their
  errors when they appear. The sign-in bar and the sitemap start page are in the accessibility
  tests and inventory, the native strips' target-size exception is written down, and the
  Definition of Done asks every new surface into both.

## [0.9.9-pre.1] - 2026-11-23

### Added

- Sitemap capture of only the visible views (RB-099-08): `sitemap.capture_visible`
  (`Leader C J`, Capture > Sitemap), `sitemap_capture` with `views: "visible"` and `rbrowser
  capture sitemap --visible` take only the breakpoints the grid shows when the run starts (the
  hidden selection, a chosen preset's included, and only the cells of a grid with cells); the
  list is kept in `run.json`. `views` together with `breakpoints` is refused, and so is a start
  with no visible Chromium breakpoint. The shared `rb_config::resolve_scope` (all, visible or
  named breakpoints) is the scope for the other runs too.
- Sitemap capture in two steps (RB-099-09): `sitemap.capture` (`Leader C M`, the menu, the
  sitemap bar's Start) opens a start page that finds the pages first (progress, Cancel), shows
  them as a tree by path segments with a count per node, and captures only the pages and areas
  chosen - checkboxes per subtree with a partial state, All and None following a search over the
  URLs, the environment the pages move to, all or only the visible views, and "N pages x M views
  = K shots" before Start. The tree draws only the rows on screen, so 10,000 pages stay fluid.
  The core takes only URLs of the discovery the start names (the last four are kept), at most
  100,000 pages. MCP `sitemap_discover` (pages and tree, nothing captured) and `sitemap_capture`
  with `discovery` and `urls`; `rbrowser capture sitemap --list` (the discovery is written to
  `.rbrowser/out/sitemap-discover/<id>.json`) and `--urls <file>` (a `.txt` file inside the
  project, read by the running rBrowser). A refused start shows on the start page; a discovery
  serves only the project it was made for.
- Tests over the visible views or chosen breakpoints (RB-099-10): checks, visual regression, flows
  and Lighthouse take a scope - every view, the views the grid shows or chosen breakpoints - from
  one pick list at each start point (findings panel, its visual regression, Flows and Lighthouse
  panels), the last choice kept per function; `checks.run_visible` (`Leader K 3`),
  `visual.compare_visible` (`Leader C C`), `flows.run_all_visible` (`Leader F G`) and
  `plugin.lighthouse.run_visible` (`Leader K 4`); MCP `checks_run`, `visual_compare`,
  `visual_baseline_update`, `flow_run` and `lighthouse_run` take `target` `"visible"` or a list
  of view and breakpoint ids. Baselines of other breakpoints stay; the results name the views
  covered and a notice the chosen breakpoints no view shows. The Flows panel offers a single
  view beside the scopes, and "Run again" in the findings panel keeps the views of the run shown.

### Fixed

- Full-page screenshots of long pages (RB-099-11): a breakpoint without `dpr` renders at the
  screen's pixel ratio, which the cap of one shot counted as 1, so at 150 % scaling the image
  was 24576 px tall whatever the page and repeated its top from row 16384 on. The ratio is now
  measured from the page's layout metrics, and a page longer than one GPU texture is captured in
  sections of at most 16000 device px under one viewport and stacked into one PNG - context
  menu, `capture.*` commands, MCP `screenshot` (`fullPage`), sitemap capture and visual
  regression alike. The image holds at most 65535 px and 256 megapixels; a longer page is cut
  there, its entry says `truncated` with `pageHeight`, and the commands show a warning. The
  overview image takes each view's top 16384 rows.
- Basic auth credentials on macOS and Linux (RB-099-05): the macOS Keychain (a generic password,
  service `rbrowser/basic-auth`) and on Linux the Secret Service (GNOME Keyring, KWallet) keep
  them, as the Credential Manager does on Windows; without a running Secret Service storing fails
  with "no secret service is running", never a file. The texts name the store of the platform.
- HTML validation says why a view has no source as served (RB-099-06): a page that asks for a
  sign-in (HTTP 401 or 407, pointing to the environment's Basic auth), a page still loading, or a
  document the browser did not keep (with what helps: reload, the setting Collect network requests
  from the start, or the rendered DOM) instead of CDP's words, which stay the detail of any other
  reason. A document body the view's own network log still holds is validated without a second
  request. The checks panel ends a failure's reason with a period before "Its findings are
  missing." when the reason has none.
- CEF's log no longer grows without bound (RB-099-07): `cef.log` keeps to the size and number of
  files of `logs.maxFileSizeMb` and `logs.maxFiles` - trimmed before CEF starts and once a minute
  while the app runs, its end kept in `cef.1.log` - and the diagnostics ZIP takes only the last
  5 MiB of each CEF log. CEF writes only its errors up to `logs.level` `info`: Chromium warns for
  every stacking move of a browser view's window (`StackChildLayerRelativeTo`), which flooded the
  file; `debug` brings CEF's warnings back. A grid built again more than ten times a second is
  logged as a loop.
- Basic auth per environment answers again (RB-099-04): CEF handed every view's challenge to
  Chromium's own "Sign in" dialog and never asked rBrowser, so a stored credential was never
  sent. Chromium's prompt is now off (`disable-chrome-login-prompt`): the stored credential of an
  environment answers its challenge in every view, read off CEF's network thread. Any other
  challenge - another site's, a proxy's, a refused password's - is asked in rBrowser's own sign-in
  bar in the toolbar strip: origin, realm, proxy and the views waiting, user and password, Sign in
  or Cancel (the server's 401 page). One question per origin, realm and proxy answers every view
  waiting, a popup under the view that opened it; a sitemap capture's render views use the store
  only. A refused answer asks again, saying so, up to three times in a row per view; plain http to
  another machine says it is sent unencrypted; "Save for environment" stores it for an
  environment's own address (only while that address is still the one asked for), else it is
  used once for the session. The credential store is never read on the UI or network thread.
  Every decision is logged at INFO with the environment, origin, realm and reason, never
  the user, the password or the request URL. Settings > Project > Environments says which
  environments share an address (and so one sign-in), and the answer of Save stands on its own
  row.

## [0.9.8] - 2026-11-22

### Added

- MCP code mode (RB-098-01): the tool `script_run` runs a short JavaScript whose
  `await rb.<tool>(...)` calls rBrowser's own MCP tools - core and plugin - and answers only what
  the script returns, one call instead of a round trip per tool and view. Every inner call goes
  through the same router and guards as a direct one (`evaluate` behind `mcp.allowEvaluate`,
  flows behind workspace trust, protected keys refused), `script_run` inside a script is refused,
  and the script runs in a wasmtime sandbox (the Boa engine as a component with no WASI: no file
  system, network or process) with a memory limit. Settings `mcp.scripts.enabled` (off),
  `mcp.scripts.timeoutMs`, `mcp.scripts.maxCalls`, `mcp.scripts.maxResultBytes`; the tool's
  description lists every `rb.*` signature; `Leader X J` opens the setting. `rbrowser mcp tools
  --format anthropic [--allowed-caller <type>]` prints the core tools as Messages API custom
  tools, for programs using programmatic tool calling.

### Changed

- CEF 156.0.31 (Chromium 156.0.8078.100, the `cef` crate 156.7.0) on every platform
  (RB-098-02): the Windows x64, macOS arm64 and x64 and Linux x64 archives pinned to new
  SHA-256s, each checked against the SHA-1 of the CDN index; the CEF age notice counts from
  the build's release on 2026-11-18. CEF's own change is the Chromium update alone; no API moved.

## [0.9.7] - 2026-11-22

### Added

- The macOS app bundle ships the Node runtime and the sidecar (RB-097-01): `rBrowser.app`
  carries the Node of `.nvmrc` (the official darwin-arm64 or darwin-x64 build, held to its
  SHA-256 pin) and the sidecar with its production packages in `Contents/Resources`, signed ad hoc
  with the app and started once while packaging; rBrowser finds both there, and on macOS the
  Playwright browsers go to `~/.local/state/rBrowser/ms-playwright`. WebKit views, flows and
  Lighthouse on macOS follow (RB-097-02 to RB-097-04).
- Flows on macOS (RB-097-03): the flow worker is granted each folder also under its real path
  (macOS's `/tmp` and `/var` are links into `/private`), and `rbrowser flow run` names
  `XDG_STATE_HOME` and `HOME` when it finds no local folder off Windows.
- Lighthouse on macOS (RB-097-04): it runs in Playwright's Chrome for Testing build, else an
  installed Google Chrome, Microsoft Edge or Chromium from `/Applications` or `~/Applications`;
  `engines.lighthouseBrowserPath` takes an app bundle (`/Applications/Google Chrome.app`).
- Basic auth at the environment (RB-094-24): Settings > Project > Environments shows each
  environment's Basic auth switch, user and password with Save and Remove, stored where the
  shell's bar stores them (the Windows Credential Manager, never a project file); the password
  field never shows a value. Renaming an environment takes its Basic auth along, and its
  Environment compare entry and the API client's environment of the same name.

### Changed

- Dependencies: hashbrown 0.17.2.
- The log rotation applies at once (RB-094-28): a change of `logs.maxFileSizeMb` or
  `logs.maxFiles` takes effect on the running log without a restart; fewer files kept clears the
  older ones at the next rotation, never the file being written.
- The API client's private values are no setting (RB-094-27): Settings > Network > API client no
  longer shows a "Private values" card without an option; a secret key (`x-secret`) is in no
  category, search, "only changed" or JSON view of the settings window. Every link to them - a
  missing `$secret`, an import's placeholders, the network panel's capture, the status bar's
  "do not read" - opens the API client at its Variables tab, and the texts say
  "API client › Variables". The new schema annotation `x-panel` names the panel that edits a
  secret key; the settings lint refuses a link to a secret key without one.
- Dependencies (Dependabot, 2026-11-22): axe-core 4.14.1 and html-validate 11.21.1 in their
  plugins' bundles, vitest 5.1.1, Playwright 1.65.1 in the examples and the recorder tests, the
  SDK template's `@nuxt/ui` 4.13.0, `taiki-e/install-action` 2.90.6 in the workflows.
- Settings apply without a restart (RB-094-23): a project setting written in the settings window
  or over MCP applies at once, also for a project on a WSL share (an environment created there
  appeared in the main menu only after a restart); a plugin switched on or off, a trusted key, the
  development mode and the crash limit apply at once, as do the Recorder's and Flows' settings
  in an open panel and the layout-mode settings of the mode shown. The keys that need a restart
  for a technical reason say "Applies after a restart of rBrowser" with "Restart now", the new
  command `app.restart` (`Ctrl+Shift+Space , S`); the settings lint holds every key to one of
  the two.

### Fixed

- MCP `config_set` and protected keys (security, RB-094-31): a write at a parent key that would
  remove or change a protected key - `null` or an object without it, such as `api-client` or
  `capture` - is refused, for the core's protected keys (`capture.dir`, the recorder's folders
  and secret list, the report folder) and the plugins' (allowed hosts, secrets). A plugin's
  protected keys are refused whether it is loaded or not, read from every installed package
  (bundled, user, project); a package whose settings do not read is guarded whole.
- MCP `config_set` keeps persisted request headers (security, RB-094-31): a write at a parent
  key that would remove or change them - `emulation {}` or `null`, `breakpoints []` - is
  refused with `config_key_protected`, and a protected key is refused so also with no project
  open.
- Linux: a start that cannot go on shows its reason in the desktop's dialog (`zenity`, else
  `kdialog`) when started from the menu, besides stderr (RB-097-05).
- Basic auth shows what is stored (RB-094-29): Settings > Project > Environments and the
  shell's Basic auth bar say per environment "Credentials stored for <user>" or "No credentials
  stored", current after Save and Remove; Remove is off without stored credentials. The password
  is never shown or sent to the shell.
- `plugins.compileCache` takes effect (RB-094-30): on, the compiled plugin code is kept in
  `plugin-cache` of the local folder, so a start with unchanged plugins does not compile them
  again; off, no cache is read or written - from the next install, update or load on. A new
  preflight rule fails on a settings key no code reads (`scripts/check-settings-read.sh`).
- WebKit views on macOS (RB-097-02): Update engines (Ctrl+Shift+Space, V, U) downloads
  Playwright's WebKit build of the Mac - macOS 14 to 26, Intel or Apple silicon - held to its own
  SHA-256 pin, and unpacks it with its executables and framework links; the views follow, sync and
  take screenshots as on Windows. On macOS each WebKit view stays a window of its own, and its cell
  says why (macOS lets no app show another app's window in its cell). The engine menu names the
  row "WebKit (Playwright)" beside "Safari"; the badge's tooltip keeps "WebKit (not Safari)".
- Linux x64 (RB-097-05): rBrowser builds and runs natively on Linux with CEF for Linux (the
  archive held to its SHA-256 pin), the Chromium sandbox on - the kernel's user namespaces or the
  setuid `chrome-sandbox`; where neither works, the start says how to fix it instead of failing
  without a word - and the Linux desktops' own shortcuts reserved (Super, `Alt+F1` to `Alt+F10`,
  `Ctrl+Alt+T`, the workspaces). `scripts/package-linux.sh` writes
  `rbrowser-<version>-linux-x64.tar.gz` and, with `--deb --rpm`, the deb and rpm packages
  (installed in `/usr/lib/rbrowser` with a menu entry); the CI Linux job builds and smoke-tests
  them.
- Security: MCP's `config_set` refuses the plugin settings protected from MCP (RB-094-26): an MCP
  client can no longer allow itself a host (`api-client.allowedHosts`, `load-test.allowedHosts`), nor
  overwrite the API client's secrets, at the key or through a parent key that sets it.
- Settings > Breakpoints: Delete removes a breakpoint while `config.local.json` holds breakpoints
  too (a view added at a size): its entry leaves both project files, which the merge joins by id,
  without copying the other file's breakpoints; MCP `breakpoint_remove` the same, whichever file
  defines it. A write the core refuses is named at the top of the settings window with its
  reasons and a link to the field (RB-094-25).
- API client: the controls work in the panel's sandboxed frame (RB-096-13). A dialog's OK, a
  private value's Save and adding an environment were form submissions the frame blocks; no
  plugin panel renders a form any more (panel-ui `PanelForm`, a test over every panel). Save,
  Save as new, Copy as code and the Variables' Save carried a value the frame cannot post and
  said only "The plugin could not do that."; calls now post plain data, and a failure of the
  panel itself names its reason. Adding a parameter keeps the new row. A collection run and a
  benchmark to a new host ask "Allow <host> for this project?" like a send, and run again after
  Allow. A refused write of the private values or the allowed hosts says the core's reason. The
  Variables tab has one way to a new environment (its name and Create, or one of the project's
  environments), renaming apart from it; the side column's tabs are icons with tooltips.
- A settings link to a plugin's setting lands on it (RB-094-26): "Open the setting" of the API
  client's benchmark hint (`api-client.benchRepeats`) opens Settings > Network > API client with
  the field highlighted instead of General. The settings window holds the loaded plugins' keys -
  shown in their category, written like a core key, following a plugin installed, removed or
  switched on or off - and a window open before the plugins came reads them then. A link to a key
  no setting has opens Settings with a notice naming the key; while the plugins still load it says
  the key opens once they are in. A key annotated `x-secret` (`api-client.secrets`) shows none of
  its values: its field says they are kept in config.local.json and edited in the plugin, the JSON
  view and MCP `settings_get` mask them.

## [0.9.6] - 2026-11-21

### Added

- Android: a local dev server reachable on the device (RB-096-12). A view that loads
  `localhost`, `127.0.0.1` or `[::1]` gets `adb reverse` for exactly that port (the scheme's
  without one) before the page loads - on opening, navigating, a history step and a reload -
  after `adb reverse --list`, with `--no-rebind`, never touching another tool's reverse. A view
  holds only the port of the address it loads now; rBrowser removes a reverse when no view
  needs it any more, when Android or the new protected setting `engines.android.reverseLocalhost`
  (on) is switched off, and first thing when it quits. One a crash left stays until the device
  reconnects or the person removes it (adb names no owner, so rBrowser never removes a reverse
  it cannot prove its own). Off, the view tells once per port that the device cannot reach the
  address, with the setting linked.
- API client laid out anew (RB-096-11): the request's header - method, URL, Send; title,
  collection, Save, Save as new, Delete, Copy as code with its language, Benchmark - stands fixed
  above the request's tabs, and the side column is tabs Requests, Variables, Import and Export
  under an environment picker that is always there. Collections (folders below `.rbrowser/api/`)
  are made, renamed, duplicated, exported and deleted, and requests opened, duplicated, renamed,
  moved and deleted, from a context menu (right-click, the menu key or Shift+F10; a delete asks
  and names the count). The Variables tab edits `http-client.env.json` - `$shared` and each
  environment as a table, environments added, renamed and removed; a row marked private goes
  to the private values, never into the file; a save over a file changed meanwhile is refused.
  The editor shows each variable of the request with its value and marks one without a value,
  and a `{{$secret name}}` without a value gets "Set value" before the send, kept at once.
  `{{$secret name}}` now reads the private values (the environment's, then `$shared`) before
  `RB_SECRET_<NAME>`. A plugin panel opens the settings at a key (`open_settings`); the API
  client's texts that name a setting link to it. The new ops are the panel's alone: MCP neither
  changes nor deletes a collection.
- Every settings mention is a link (RB-094-21): a text that names a settings path ("Settings >
  Engines > Android") opens Settings at its key. A notice links in the status bar, its history,
  the error toast and MCP `status_messages` also when its code named no setting or the text is a
  failure's reason; the Portal panel's off hint, the setup wizard (off, welcome, plugin step,
  missing analyser, MCP that did not switch on) and the panels of Fonts, Lighthouse, Load test,
  Network, Rulers, Security headers, Snippets and Structured data show *Open the setting* beside
  the text (panel-ui `SettingLink`, a status line's `setting`). The settings lint fails on a
  mention without its row in `crates/rbrowser/data/settings-links.json`.
- Findings panel: run one check or a selection again (RB-094-17) - "Run <check> again" in a
  check's row of the progress details and beside the plugin filter, checkboxes and "Run selected
  again" in the details. The run goes over the last run's views and page and each check's result
  is merged into the last run, the other checks' findings and rows kept; `checks_run` with
  `plugins` over the same views merges the same way.
- Requests of the page into the API client (RB-096-08): the network panel opens a captured
  request in the API client as an unsaved draft (method, URL, headers, body) or saves it as a
  request, and Save shown as requests saves the filtered list at once - one `.http` file per
  method and path below `.rbrowser/api/<host>/`, one saved before skipped, the counts in the
  status line; the endpoint view opens its newest request. Cookie, Authorization, API-key and
  token headers, secret query parameters and body fields become variables named after the host;
  their values go to the private values (`config.local.json`) or nowhere
  (`api-client.captureSecrets`), a value the capture masked stays a variable without one. A body
  is checked for secrets whatever its type (JSON sent as `text/plain`, multipart fields); one
  that cannot be checked or that the view cannot hand over is left out and said, and a draft
  over unsaved edits in the API client waits for Replace. MCP
  `networkpanel_api_open` (read only) and `networkpanel_api_save` never keep a value; commands
  `Leader D G` and `Leader D K`.
- rBrowser's version and the credit line in the main window's title (RB-094-19):
  `<project> - rBrowser <version> - made with Love by Alexander Herling/DEGOYA.digital`, without
  the project part when none is open. `ui.titleBar.showVersion` and `ui.titleBar.showCredit`
  (both on, Settings > General > Appearance) switch each part off at once; the other windows keep
  their titles.
- Devices and engines in the menus, an action's error at the top (RB-094-20, RB-096-10): ≡ ›
  Devices & engines and the end of the toolbar's "+" (now built in the shell) list the Android
  devices adb saw last with their state, "No device - how to connect" and Try again, the setting
  while Android is off, Download adb, Safari and iOS ("macOS only" elsewhere). The error of an
  action the person started shows as a toast in the toolbar's strip with Retry, Settings and
  Download (`ui.notifications.errorToast`, on); a view that failed to start names its reason in
  its label strip and retries on a click. An empty device list names the USB mode "File
  transfer", USB debugging, the maker's Windows driver and Samsung's Auto Blocker; the debug log
  names the adb in use and its answer to `adb devices -l`. No command is reachable only by a
  shortcut any more: `mcp.setup` and `portal.present.start` got main menu rows, and the menu
  coverage test allows no omission beyond the main menu's opener and the numbered families.
- Connect an AI agent over MCP (RB-091-09): the setup wizard's recommended step "AI agent"
  (skippable) and Settings > Automation > MCP under the new `mcp.client` show, for Claude Code,
  VS Code / Copilot, Claude Desktop, Cursor, OpenAI Codex and pi, the client's project file
  (`.mcp.json`, `.cursor/mcp.json`, `.codex/config.toml`, `.pi/mcp.json`) and its connection for
  every project (`claude mcp add --scope user`, `code --add-mcp`, `codex mcp add`, `pi mcp add`,
  or Cursor's and Claude Desktop's user file, written only after the person ticks it and with a
  copy of the old file). Every way is stdio through `rbrowser mcp`, no port and no token in any
  file; an existing file keeps every other server, a file that cannot be merged is never
  written. The wizard's button switches `mcp.enabled` on - the person's decision, never MCP's;
  "Check connection" says when a client last reached rBrowser. `mcp.setup` (`Leader , N`) opens
  the setup.
- Accessibility of the shell UI (RB-090-08): axe-core's WCAG 2.2 A/AA rules and keyboard
  checks (every control reachable, every Tab stop named) run in `npm test` over every page,
  route and bar of the shell; the setup wizard's radio groups are named, the activity bar says
  which activities start and end, scrolling code views are named regions; inventory and the
  Windows/NVDA pass in `docs/accessibility.md`.
- MCP prompts and resources (RB-095-01): an agent's client offers the prompts
  `responsive_review` (every breakpoint looked at, findings per breakpoint), `a11y_triage` (axe's
  findings by severity with fixes) and `bug_repro` (a bug reproduced in every view, then a bug
  report), each with `url` and `breakpoints`; the read-only resources `rbrowser://views`,
  `rbrowser://findings/latest`, `rbrowser://screenshots/latest` (a PNG only when its own URI is
  read) and `rbrowser://config` (without the protected keys) save tool calls. Same token and
  answer limit as the tools; settings `mcp.prompts` and `mcp.resources` (on), Settings >
  Automation > MCP.
- Android devices on USB as views (RB-095-02): with `engines.android.enabled` on, "Add an
  Android device as a view" (`view.add_android`, `Leader V ;`, MCP `view_add_android`) adds
  Chrome on the device over ADB as the breakpoint `android-<serial>` at the device's own size;
  its navigations follow the grid, screenshots and CDP input reach the device, and its DevTools
  port is forwarded to 127.0.0.1 only (a forward that answers beyond the machine is refused).
  adb comes from `engines.android.adbPath`, PATH, `ANDROID_HOME`, `ANDROID_SDK_ROOT` or the
  usual SDK folders; without one, `tools.adb.install` (`Leader X S`) downloads Google's
  Platform Tools after the consent notice - never shipped, never over MCP. MCP
  `android_devices` lists the devices.
- Safari as the engine of a view on macOS (RB-095-03): a view set to `safari` (engine badge, `Leader B E`, MCP `views_set_engine`) opens the installed Safari over `safaridriver` in a window of its own at the breakpoint's viewport; it follows navigation and scroll, replays clicks, keys and text, takes screenshots, and only follows - Safari locks input in automated windows. Remote automation switched off names `safaridriver --enable`; outside macOS the badge menu shows Safari disabled with the reason. Settings `engines.safari.enabled` and `engines.safari.technologyPreview`; the Safari version in Settings > About. The new crate `rb-safari`; written against a fake driver, the spike on a Mac is open.
- iOS as the engine of a view on macOS (RB-095-04): a view set to `ios` runs in Mobile Safari over `safaridriver` - in the iOS Simulator when Xcode is installed (any model, booted on demand), otherwise on an iPhone or iPad connected over USB; without either, the reason names both ways and the setting. The breakpoint's `iosDevice` names the model or the UDID; `Leader B I` (`ios.devices`) and the engine badge's menu list the simulators and devices, MCP `ios_devices` lists them and `views_set_engine` takes `iosDevice`. The size is the device's (marked), a click replays as a touch, the view only follows. Settings `engines.ios.mode`, `engines.ios.defaultSimulator`, `engines.ios.bootTimeoutSec`; device UDIDs are cut short in logs and masked in the diagnostics ZIP. Written against fakes; the spike on a Mac is open.
- API client panel (RB-096-01, plugin `api-client`, `Leader P 4`): build a request - method, URL
  with its query as rows, headers, body (JSON, form, multipart, raw, binary file, GraphQL), auth
  (basic, bearer, API key; digest and OAuth 2 kept, not sent yet), timeout, redirects, TLS check
  - and send it from rBrowser's core, without CORS, through the environment's proxy, with the
  phases timed (DNS, connection, TLS, first byte, total); read the answer as a JSON tree with
  search and a JSONPath filter, as source, as an image, its headers and cookies. Requests are
  kept as one `.http` file each below `.rbrowser/api/` (VS Code REST Client and the JetBrains
  HTTP Client run them as they are); variables `{{name}}` from
  `.rbrowser/api/http-client.env.json`, coupled to the views' environment (`{{baseUrl}}`), the
  private values of `api-client.secrets` (config.local.json) and `RB_SECRET_*`, every secret
  masked in what comes back. The first send to a new host asks; the hosts allowed are kept for
  the project (`api-client.allowedHosts`, Settings > Network > API client) and redirects follow
  only to them. MCP: `api_collections`, `api_run` (saved requests only), `api_history`. New
  capability `api-client`, the model crate `rb-api-model`.
- Load test (RB-096-07, plugin `load-test`, `Leader P 5`, Stop `Leader K 2`): your own server
  put under load on purpose by many virtual users sending the API client's saved requests - a
  scenario in `.rbrowser/api/load/*.json` (order or weights, think times, values per user,
  constant users or a ramp up, hold and down, a target rate, abort rules on the failure rate and
  p95), run in the core on its own connection pool, live in the panel (requests per second,
  p50/p95/p99, failures by kind, users, a time chart) and in the activity bar with Stop, a report
  per run in `.rbrowser/out/load-test/` (JSON and HTML) and two reports compared. Targets only the
  trusted project's own sites and this machine, any other host after the person confirms it is
  theirs or theirs to test (`load-test.allowedHosts`, config.local.json, `x-protected`); every
  run starts by the person's click on its summary; the ceilings are settings only
  (`load-test.maxUsers` 50, `maxRps` 200, `maxDurationS` 300); Stop and closing rBrowser end every
  connection at once; every request says `User-Agent: ... (rBrowser load test)` and goes out
  without a proxy. MCP `load_reports`, `load_report`, `load_compare` read only; capability
  `load-test` (risky in the trust dialog).
- API client checks, collection runs and a benchmark (RB-096-04): checks per request - status
  (`201`, `2xx`), a JSONPath value equals or contains, a header, the time - kept as
  `# @rb-check` lines in the `.http` file, judged on every send; a collection run (`Leader K 1`,
  MCP `api_run_collection`) sends every saved request and judges its checks, keeps the report in
  `.rbrowser/out/api-client/report.json` (MCP `api_report`) and puts its failures into the
  findings panel (rules `api-client.check-failed`, `api-client.request-failed`); the benchmark
  of a saved request (MCP `api_bench`) sends it `api-client.benchRepeats` times (10) in one or
  two environments, at most `api-client.benchConcurrency` (1, at most 4) at once, only to allowed
  hosts, and shows min, median, p95 and max of every phase and the difference between the two.
- API client imports (RB-096-02): the panel's **Import a collection** reads Postman (collection
  v2.0/v2.1, environment, globals), Insomnia (export 4, JSON and YAML), Thunder Client, a Bruno
  folder, HTTPie Desktop, `.http` files of several requests (VS Code REST Client, JetBrains, and
  `http-client.env.json`), OpenAPI 3.x and Swagger 2.0 (one request per operation with an example
  body), HAR and a pasted `curl` command into `.rbrowser/api/<folder>/` - one `.http` file per
  request, no file overwritten, environments added to `http-client.env.json` without changing a
  value (`api-client.importEnvironments`). A preview first; the report names what was taken,
  what rBrowser does not run (scripts, tests, other auth kinds) and what was left out; every
  literal token, password, key or cookie becomes a `{{$secret <name>}}` placeholder. Panel only,
  never MCP. New crate `rb-api-import`.

- API client: export and code (RB-096-03). **Copy as code** turns the request in the editor into
  curl, HTTPie, `fetch` (Node) or a Playwright test - the environment's plain values put in, every
  secret an environment variable the code reads (`api-client.codeVariables`), the command quoted
  for PowerShell, cmd or a POSIX shell (`api-client.codeShell`). **Export** writes a folder, or
  every request, as a Postman v2.1 collection, one `.http` file or a curl script to
  `.rbrowser/out/api-client/exports/`, every variable kept. MCP `api_code` (saved requests,
  read-only). New crate `rb-api-export`.

- The portal as a presentation over the internet (RB-096-05): `Leader S W` (`portal.present.start`,
  a person only - MCP can read its state and end it, `portal_present_status`,
  `portal_present_stop`) opens a Cloudflare Quick Tunnel (no account) after the consent in the
  Portal panel and hands out one-time links (`#t=` token in the fragment, `portal.presentation.
  linkTtlSec`, `maxViewers`, five failures spend a link); the customer's browser follows the
  presenter and drives nothing, paths outside `allowPaths` or in `denyPaths` answer 404 without
  reaching the dev server, and no cookie or server detail passes. cloudflared is a managed tool
  (pinned, SHA-256, Apache-2.0, downloaded only after the person's word) or
  `tools.cloudflaredPath`; `Leader S K` a new link, `Leader S E` ends it; the activity bar shows
  the viewers and the link's time left.
- Endpoint analysis in the network panel (RB-096-06): a tab **Endpoints** groups the requests the
  views already made by method and path, ids as placeholders (`/api/users/{id}`; own patterns in
  `network-panel.endpoints.patterns`), with calls, median and p95 of duration and time to first
  byte, sizes, status mix, caching and compression per endpoint and per view, and **Show the
  requests** to jump to them. Findings for duplicate calls, N+1 patterns, large uncompressed and
  uncacheable answers, slow endpoints and endpoints one breakpoint calls more often (`checks_run`,
  `endpoints_check`, `Leader K 7`); limits in Settings > Network > Endpoints; MCP
  `networkpanel_endpoints`. `network_requests` and the plugins' capture now carry each response's
  waiting time, caching and compression facts and decoded size.
- Pause a video recording (RB-090-68): `capture.video.pause` (`Leader C Z`, the activity bar's
  Pause/Resume, the Video menu's check row, MCP `record_video` `pause`/`resume`) holds the
  recording; the files leave the pause out - no gap, no frozen stretch - and
  `capture.video.maxDurationSec` counts recorded time. Paused, the activity bar and the toolbar's
  indicator say so and the time stands still. A flow run's activity bar entry names the step each
  running view is at, "step n/m" (`m` the `step(` calls of the flow).
- After an update that ships plugins you were not offered before, the status bar names them once
  with an **Install all** button (they load at once); `plugins.catalog.newNotice` switches it off
  (RB-094-13).
- Reading order arrows in the tab order overlay (RB-094-02): `tabOrder.readingOrder` (off,
  beside, instead) draws the reading order - the source order screen readers follow - as dashed
  arrows between the tab stops, beside the tab order lines or instead of them, and frames each
  element shown before the one before it in the source (CSS `order`, grid placement, reversed
  flex, absolute positioning; WCAG 1.3.2), right to left where the content reads so; fixed,
  sticky and unseen stops are left out. The outline plugin's `reading-order` rule, with
  `tabOrder.readingTolerancePx` (4 px) as its tolerance; Settings > DevTools > Overlays.
- An address field of its own for a decoupled view (RB-092-03): a click on the address in the
  label strip of a view whose navigation is decoupled, `view.edit_address` (`Leader S L`,
  palette) or the page menu's Sync › Edit this view's address opens a field in the strip (on the
  canvas in the board's label); Enter loads the address in that view only - http, https and file,
  a path under the environment in effect, the address bar's completion otherwise - and the other
  views keep their pages; Escape cancels. A refused address (`javascript:`, `data:`,
  `chrome:`, ...) names its reason in the status bar. A view that follows the active view gets
  no field (the menu entry is disabled and says why). Setting `ui.cellLabel.addressEditable`
  (on); MCP loads a view's page with `navigate { target: viewId }`.
- `dir`/`lang` per view and pseudo-localisation of the page (RB-092-02): the emulation fields
  `dir` (ltr, rtl, auto) and `lang` set the page's `<html dir lang>` live in Chromium views -
  per breakpoint, as project defaults or with `emulation.set` / MCP `emulation_set`;
  `emulation.toggle_dir` (`Leader V .`) flips right to left. `overlays.pseudo_locale`
  (`Leader O Y`, MCP `pseudo_locale`) makes the page's texts longer, accented and marked with
  `[ ]` (Settings > Emulation > Text direction and pseudo-localisation) and puts them back
  without a reload; the sync keeps finding the elements, as the selectors read the original
  texts. Both in the page's context menu Emulation.
- A bug report bundle (RB-093-04): `report.create` (`Leader C U`, Capture > Bug report in the
  page and main menus, the palette, MCP `bug_report { target, note? }`, which answers the paths
  only) writes `.rbrowser/out/reports/<time>/` with a screenshot of each view (password fields
  blacked out), the overview, the console errors and warnings and the failed requests
  deduplicated over the views, page, environment, breakpoint, size, emulation, engine and version
  - as `report.md` with relative image paths and as `report.zip`. Every text is masked (query
  values, credentials, cookie and header values); headers and bodies are never read. Before
  anything is saved a preview window shows the masked `report.md` with Save and Cancel (Cancel
  leaves nothing behind; `report.preview`, on), and `report.folder` puts reports in a folder of
  the person's choice - both user settings no MCP client or project config can set. Settings
  Capture > Bug report: `report.include`, `report.annotate`; the status bar's Open folder
  (`report.open_folder`, `Leader C Y`) and Copy Markdown.
- Width sweep (RB-093-01): the plugin `width-sweep` drives one view from `width-sweep.from` to
  `width-sweep.to` (320 to 1920 px) in steps of `width-sweep.step` (10 px), then pixel by pixel
  around every change, and reports where the layout breaks between the breakpoints - horizontal
  overflow (without the `100vw` false report), text cut off, overlapping siblings
  (`width-sweep.kinds`) - as width ranges in the findings panel ("700-740 px: horizontal overflow
  caused by `.hero img`"). The view leaves the sync while it runs; the end, Escape in the view,
  `plugin.width_sweep.cancel` and MCP `sweep_cancel` give it its own width and sync back, the other
  views keep URL and size. `plugin.width_sweep.run` (`Leader B W`, the context menu's Checks) and
  MCP `sweep_run`. For it a plugin may send `Emulation.setDeviceMetricsOverride` with only a
  `width` and `Emulation.clearDeviceMetricsOverride`: the host's width lease of the view, which
  ends with the plugin's last running call at the latest.
- HTML validation of the rendered DOM and per project (RB-094-06): `html-validate.source`
  (`raw`, the default, or `rendered`; MCP `html_validate { source }`) validates the DOM after
  rendering, serialized in the view, its findings by element instead of line. A trusted
  project's `.rbrowser/htmlvalidate.json` sets the levels and options of the plugin's rules over
  `html-validate.disabledRules` - only `rules`, never plugins, presets or element tables that load
  code - and can switch on the opt-in `no-unknown-attributes`. Every finding with a line names
  its element, so the findings panel highlights it in the view. html-validate is listed in the
  About dialog's licences.
- Live CSS (RB-094-04): the plugin `live-css` applies a CSS draft in every view at once, block
  by block in every view or in chosen breakpoints (inside the media query of their widths), again
  after a navigation (`live-css.applyOnNavigate`); on and off without losing it, *Copy as media
  query* per block, the draft kept in `.rbrowser/out/live-css/` (`draft.css` as one stylesheet)
  only in a trusted workspace. Panel (`Ctrl+Shift+Space P Q`), toggle (`Ctrl+Shift+Space D L`),
  MCP `livecss_set` and `livecss_clear`.
- Basic auth per environment and a session's login state (RB-094-08): `environment.auth`
  (`Leader E A`) stores the user and password of an environment in the Windows Credential Manager,
  bound to the project and the environment's origin - never in `config.json`; every view and
  session then signs in to the environments listed under `basicAuth.environments` (Settings >
  Project > Environments, protected) without a prompt, and nothing else gets the credential.
  `session.state_export` (`Leader E W`) writes a session's cookies and `localStorage` as Playwright
  `storageState` to `.rbrowser/out/state/<session>.json` (the project's sites only unless
  `sessions.stateExportAllDomains`), `session.state_import` (`Leader E I`)
  loads one into a session, and a flow's `// @storageState <file>` starts it logged in. MCP can
  neither read nor set credentials or login state.
- The outline across the breakpoints (RB-094-02): the Outline plugin finds the interactive
  elements, headings and landmarks some breakpoints show and others lack or hide - one finding per
  element naming the breakpoints missing it (`outline.diff.elements`) - and the reading order
  against the source order (WCAG 1.3.2): a heading or control CSS `order`, grid placement or
  reversed flex shows before its predecessor, both named, right to left where the page reads so
  (`outline.readingOrder.tolerancePx`). The panel's tabs Breakpoints (the table element x
  breakpoint) and Reading order, the commands `outline.breakpoint_diff` (`Leader B D`) and
  `outline.reading_order` (`Leader B R`), MCP `outline_breakpoint_diff` and
  `outline_reading_order`; the findings also through `checks_run`.
- Clock and animations in every view (RB-094-07): `emulation.clock` (`Leader S T`, palette, page
  menu Emulation > Clock, MCP `clock_set`) sets the time the pages read in every Chromium view at
  once - fixed, or running from a start at real or faster speed, `Date` and `Intl.DateTimeFormat`
  alike, a time without a zone each page's local time; the timers stay real. `overlays.animations`
  (`Leader O J`, Emulation > Animations, MCP `animations_set`) stops, slows (10 %, 25 %) or
  steps CSS and Web Animations per view or in all; a capture keeps a stopped view stopped. The
  activity bar shows either while changed, with Step, 100 % and Real time. Settings > Emulation >
  Clock and animations; Firefox and WebKit views are named as unsupported.
- Engine comparison (RB-094-03): `compare.engines` (`Leader C Q`, palette, Capture > Visual and
  the view's Screenshot menu) takes the view's page at its breakpoint in Chromium (a hidden render
  view), Firefox and WebKit (a window of their own) at the same size and DPR, each with its own
  user agent, and diffs every engine pair with the colour tolerance and the anti-aliasing test, so
  font smoothing is no difference; `report.html` (side by side, slider, difference, the differing
  areas) and `report.json` land in `.rbrowser/out/engine-compare/<run>/`. An engine that is not
  installed or switched off is named, not an error. MCP `compare_engines` answers when done with
  every path and per engine pair the share of differing pixels and the areas. Settings
  `engineCompare.threshold`, `engineCompare.maxDiffPercent`, `engineCompare.waitMs` (Capture >
  Engine compare). The environment comparison's report lists the differing areas too.
- Plugin progress in the activity bar and Show at N px (RB-093-01): `host.report-progress` of a
  running plugin call is the activity bar's plugins entry - the plugin, `done` of `total` and
  Cancel, which runs the plugin's own `cancel` command (the width sweep's
  `plugin.width_sweep.cancel`) - throttled to 4 steps a second and gone with the plugin's last
  call; `ui.activityBar.show.plugins`. A finding that names a width (the width sweep's) offers
  Show at N px in the findings panel: the plugin's `show` command (`plugin.width_sweep.show`, MCP
  `sweep_show`) holds the finding's view at that width through the width lease until Escape,
  Cancel, another width or `width-sweep.showSeconds` (120 s).
- The last pages at the start (RB-092-04): with `startup.restore = last` (Settings > General >
  Start behaviour; default `startUrl`) every view opens the page it showed when rBrowser closed -
  a page under an environment in the environment in effect now -, and a decoupled view stays
  decoupled; a URL on the command line wins. The pages are kept per view in the project's browser
  profile only (on close and every 15 s), never in the project files, logs or diagnostics ZIP.
  `views.restore_last` (`Leader R L`, palette) and MCP `views_restore_last` open them at any time.
- Findings export and `rbrowser check` (RB-092-05): the last check run as SARIF 2.1.0 (one run
  per plugin, breakpoints, views, engines and URL in each result), JUnit XML (one suite per
  breakpoint, one case per rule), one offline HTML file or its JSON - from the findings panel's
  Export split button, `checks.export` (`Leader K B`) and MCP `checks_export`, which writes only
  below `.rbrowser/out/` or the protected `checks.export.dir`. `rbrowser check [--plugins]
  [--format] [--out] [--fail-on]` runs the checks in the project's running rBrowser and exits 0
  without a finding at or above the threshold, 1 with one, 2 when a plugin failed or none runs.
  Settings `checks.export.defaultFormat`, `checks.export.failOn` and `checks.export.dir`.
- A visual editor for recordings (RB-092-01): in the Recorder panel a `+` between two steps and
  "Add step" at the end add a step of any kind (Navigation, Interaction, Input, Wait, Assert),
  its element taken by "Pick in view" as a recorded click names it, the locator's candidates
  editable; each step can be opened in the editor (kind, name, element, value, button and
  modifiers, pause before it, breakpoints, on/off), tried alone in the active view, played from,
  duplicated or skipped - a skipped step stays in the recording but is neither played nor
  exported. "Continue recording from here" plays the steps up to a step at the pace Fast and
  records the new steps after it (no recording when a step before fails). A fixed wait
  (`wait`, at most 10 s) waits at every pace; a pause set in the editor replaces the recorded one.
  Commands `recorder.step_add` (`Leader F N`), `recorder.step_try` (`F J`), `recorder.play_from`
  (`F H`), `recorder.continue_from` (`F U`), `recorder.step_toggle` (`F B`),
  `recorder.pick_element` (`F I`); MCP `recorder_step_add`, `recorder_step_update`,
  `recorder_step_remove`, `recorder_step_toggle`, `recorder_step_try`, `recorder_start`
  `continueFrom`, `recorder_play` `fromStep`; `automation.recorder.insertedStepPauseMs` (500 ms).
- Responsive images per breakpoint and pixel density (RB-093-02): the first-party plugin
  `responsive-images` shows, live in every view, which file the browser took from `srcset` or
  `<picture>` and whether it is too large or too small for its rendered size times the view's
  device pixel ratio, a `srcset` with widths but no `sizes`, and an LCP image that loads lazily or
  without `fetchpriority="high"` - without a Lighthouse run. A table per breakpoint whose hover
  marks the image in every view; settings in Checks > Responsive images
  (`responsive-images.oversizeFactor`, `undersizeFactor`, `checkLcp`); `Leader K N`, `Leader P V`,
  MCP `images_check`, `images_mark`, and part of `checks_run`.
- A security header check (RB-093-03): the first-party plugin `security-headers` holds the main
  document's headers and the cookies it sets to the common rules - CSP (missing, `'unsafe-inline'`/
  `'unsafe-eval'`, wildcards, no `object-src`/`base-uri`), HSTS (`security-headers.hstsMinAge`; not
  over `http:` or on a loopback host), frame protection, `nosniff`, `Referrer-Policy`,
  `Permissions-Policy`, CORS `*` with credentials, cookie `Secure`/`HttpOnly`/`SameSite` and the
  `__Host-`/`__Secure-` prefixes, an HTTPS page redirecting to HTTP, cookie values never shown - one finding per URL and environment, and
  compares the page across the project's environments in a panel (header x environment, the
  differences marked); `Ctrl+Shift+Space K U`, `Ctrl+Shift+Space E H`, MCP `headers_check`,
  `headers_compare`.
- Container queries made visible (RB-094-01): the plugin `container-queries` frames every size
  container in every view with its name, its size and the `@container` conditions that match now,
  and drags one container's width on a handle to test its rules without changing the view (a
  double click resets the inline style). Its panel lists every condition with its bounds and each
  container per view with the next bound below and above and the breakpoints that switch it
  (`Leader O U`, `Leader B C`; MCP `containers_overlay`, `containers_analyze`, `containers_resize`;
  Settings > DevTools & Debugging > Container queries).
- Consent pre-check (RB-094-05): the plugin `consent-check` lists which requests to other sites
  (host and kind: script, pixel, font, iframe), cookies (name, domain, lifetime; never a value)
  and localStorage keys a page has before any consent and after "Reject all" of a detected
  cookie banner, with the differences and four rules for `checks_run`; settings
  `consent-check.allowHosts`, `consent-check.rejectStep`, `consent-check.essentialCookies`; a
  panel, `Ctrl+Shift+Space K Y`, MCP `consent_check`. A technical pre-check, no legal advice. The
  tool and the command run in a fresh, discarded session of the core's consent probe (RB-094-09);
  a `checks_run` reads the view as it is, read only.
- HTML validation like the W3C validator (RB-090-97): the plugin `html-validate` checks the page
  source as the view loaded it - not the rendered DOM - offline with html-validate (MIT, bundled),
  or with a Nu HTML Checker at `html-validate.nuAddress` (empty by default; only then does the
  source leave the device). Findings with rule, line and column per breakpoint, rules off and the
  lowest severity in Settings > Checks > HTML validation, MCP `html_validate`, `Leader K V`.
- A structured data panel (RB-090-96): the first-party plugin `structured-data` shows the
  schema.org data of the chosen view's page - JSON-LD, Microdata and RDFa - as a tree and checks
  it against the schema.org 30.1 vocabulary and the needs of the common rich results, each
  finding at its node and in a list (`structured-data.richResults`, `structured-data.minSeverity`;
  `Leader K T`, `Leader P U`, MCP `structureddata_inspect`). The SEO check's JSON-LD, Microdata
  and RDFa readers moved into the shared library `plugins/schema-org`.
- Recordings keep every menu hover and the pointer's way (RB-090-87): with `recordHover` `menu`
  every hover on a menu entry of any level becomes a step once the pointer rests there
  `automation.recorder.hoverDwellMs` (250 ms), with or without a click after it; passing over
  quicker is no step. `automation.recorder.recordPointer` (`menu`, `all`, `off`) keeps the way to a
  menu's hover or click with the step; Play and the exported flow and test move the pointer
  along it with `page.mouse.move` before the step - with the recorded timing at the pace As
  recorded, without waits at Fast - in a view of the recording's size. The Recorder panel shows
  "Pointer path (N points)".
- The Recorder panel shows the steps while recording (RB-090-89): each step as soon as it is
  recorded, read-only, the newest marked and scrolled into sight, hovers with their pause and
  pointer path; once saved, the recording opens for editing
  (`automation.recorder.showLiveSteps`, on by default). Play brings a recording's view into
  sight when the layout hides every view it plays in - the canvas centres on it, the strip
  scrolls to it, focus mode focuses its breakpoint - and says so
  (`automation.flows.revealRecording`, on by default).
- Flows and recordings can be deleted (RB-090-91): "Delete" beside each entry of the Flows and
  the Recorder panel, asked in the panel first (`automation.flows.confirmDelete`,
  `automation.recorder.confirmDelete`, on by default); the commands `flows.delete` (`Leader F D`)
  and `recorder.delete` (`Leader F M`) open their panel without a name. A trusted project stays
  trusted when only the deleted flow left its protected folder; the flow running and the
  recording being recorded or played stay. No MCP tool: an agent never deletes project files.
- The SEO panel previews the page as a Google search result for desktop and mobile (RB-090-95):
  favicon, site name, the URL as a breadcrumb, title and description cut by pixel width with an
  ellipsis at a word end, the characters and width per text and what was cut;
  `seo.serpPreview` (`both`, `desktop`, `mobile`, `off`). MCP `seo_social_preview` answers it as
  `serp` with the limits per device (unverified).
- Consent probe in the core (RB-094-09): a plugin granted the new capability `consent-probe`
  (the consent check, RB-094-05) has rBrowser load an `http(s)` page in a fresh, empty,
  temporary session of its own - never a view's -, capture its requests, cookies (name, domain,
  expiry; never a value) and localStorage keys before consent and, with the plugin's "Reject
  all" step, once more after it, then delete the session; the capture reaches the plugin as
  `$consent`, and a `$consent` a caller sends is dropped. The probe loads only public addresses
  and the trusted project's own sites unless `plugins.consentProbeAnyUrl` (Settings > Plugins,
  off, not over MCP) allows any; a cancelled probe closes its session too. The trust dialog
  marks the capability risky.
- Timing counters and the K1/K11 measurement (RB-090-32): release builds keep counters of the
  sync hub (queue wait, replay per engine, the navigation a view was given), the plugin calls
  (wait for a slot, run), the window's requests and the start-up time to the first view's
  first load - count, p50, p95, max over the last 1024. The MCP tool `app_perf` answers them
  (`reset` measures afresh), "Show the timing counters" (`Leader D B`, the DevTools menu) shows
  the p95s, the diagnostics ZIP carries `perf.json`; `performance.counters` (Settings >
  Advanced > Diagnostics, on) switches them. `scripts/perf.sh` measures K1 (100 clicks on
  `latency.html`, p95 at most 150 ms) and, with `--counters` and the saved `app_perf` answer,
  K11 (Firefox and WebKit, p95 at most 300 ms) and the start-up time; `--runs 0` reports them
  without a scroll run. `docs/dod.md` says what stands in for `scripts/e2e-windows.sh` until
  RB-090-05 brings it.

### Changed

- Form autofill (RB-094-22): a fill takes at most `fillBudgetSeconds` (30 seconds, 5 to 300)
  in all, every view and pass together; past it no further pass or view starts, and the answer says so (`outOfTime`, a
  view's `stopped: deadline`, a view not started `timeout`). Consent checkboxes are ticked only
  while the new setting `tickConsent` (on by default) says so, and a newsletter or marketing box
  is never taken for one, whatever else its name says.
- A captured request of a host whose name has `-` or `_` (RB-096-08) goes on into the untagged
  folder an earlier capture made for it (`api-example-test/`), as long as only that host's
  requests are in it and the tagged folder does not exist; a new host gets the tagged name.

- Forced pseudo-class states stand in the activity bar (RB-094-16): while a view has a state
  forced (page menu Force state, `view.force_state.set`, MCP `views_force_state`), an entry names
  the states and the views - narrow, its icon and the number of views - with Reset for every
  view (`view.force_state.reset`) and the close button; folded or with
  `ui.activityBar.show.forcedStates` off, its compact form stands in the toolbar. The status bar
  no longer says "forced" for as long as they last; views without the element, failures and the
  last state gone stay one-off messages there.
- The first-party plugins share their copied helpers - the crawl's URL and `robots.txt`, base64,
  the rules' levels and the CDP mark of an element - in the library `plugins/plugin-util`, and
  write the guest half of their components with `rb_plugin_api::guest_host!`: about 5,200 lines
  fewer in the plugins, behaviour unchanged (RB-094-11).
- The status bar's notice that pseudo-class states are forced has a **Reset** button that takes
  them back in every view (`view.force_state.reset`, RB-090-56).
- Settings help texts name the choices by the names the settings window shows, not by their
  config ids (panels, breakpoint bar, toolbar, activity bar, links, cookies and eight plugins);
  the desync timeout names the toolbar's Desynced (n) button. The settings lint refuses a text
  that names an option id; the user SID is read once in `rb-process`, and the colour picker
  decodes a window image's base64 once (RB-090-29).
- One log store and one collector for MCP and the plugins (RB-090-34): `console_logs`,
  `network_requests`, `errors` and the plugins' `$console` feed share the window's buffers instead
  of collecting every event twice, and the views' network requests are collected only from the
  first time they are asked for - or from the start with the new setting `mcp.collectNetwork`
  (off), Settings > Automation > MCP.
- Less idle work and fewer copies (RB-090-38): the plugin engine's epoch ticker runs only while a
  plugin call does, the cookie copy's status poll only while Firefox or WebKit views exist; panel
  files are served from the plugin's own files instead of a copy, and the sync paths read the
  config's sync section without copying the whole project config. Switching back to one of the
  plugin panels shown last no longer loads it again (`panels.keepFrames`, 3, Settings > Plugins),
  and the palette's popup and the panels window no longer load the settings page with them.
- The activity bar keeps its room, empty while nothing runs, so the views and panels no longer
  jump when a check, a recording or a video starts and ends: `ui.activityBar.mode` `always`, the
  new default; `bar` shows it only while something runs (RB-094-13).
- Hot paths of the core trimmed (RB-094-10): every CDP event is built once and shared by its
  subscribers instead of copied per subscriber; screencast frames no longer flow through the
  shared event buffer, and the sync agents, the navigation sync and the plugin page scripts
  follow their events on channels of their own, so a video recording no longer makes them miss
  a binding call. A burst of bounds changes applies a view's device metrics latest-wins on one
  worker per view; CDP frames are written to the pipe on a thread of their own; the annotated
  settings schemas are built once per process; the MCP log collector looks a session of no view
  up once; a plugin's progress reaches the shell pages as its own message instead of the whole
  state; the leader's timeout ticker runs only while a chord is open.
- Plugins load and unload live (RB-092-06): an install, update or removal from the catalogue,
  the first-run wizard or the update check, and a project trusted after the start, take effect at
  once - commands and shortcuts, the palette and menus, panels, page scripts in the open views,
  checks, the context menu, settings and MCP tools (`notifications/tools/list_changed`) follow
  without a restart. A call to a plugin removed while it runs ends with
  `plugins.error.not_available`. "At the next start" is said only when no load can run.
- The setup wizard runs in one click per step (RB-091-08): each step's button says what it does
  and does it - "Install 3 plugins & continue" (the grants of the choice stand in the step, the
  click is the consent; "Continue without plugins"), "Write config & continue", "Apply &
  continue" - with progress, each plugin's state and Retry in the step. At the very first start
  the plugins chosen there load at once, without a restart: the one load of the plugins waits
  until the plugin step is answered (or the wizard closes). The project step previews by itself;
  with an address the breakpoint analyser is preselected and says what it found ("2 breakpoints
  in 3 stylesheets") or why it had no proposal, with Try again and Use mobile + desktop.
- The Recorder panel stops a recording where it started it (RB-090-94): while one runs, "Stop
  recording" stands beside its badge in place of "Start recording".
- Dependencies: Nuxt UI 4.13.0 in the shell, the plugin panels and the SDK template.
- The page's context menu is grouped like the main menu `≡` (RB-090-92): its submenus carry the
  main menu's names in its order - View (the bars), Layout (Analyze breakpoints), Sync,
  Emulation, Environments › Sessions, Capture › Screenshot and Video, Checks, Tools (Force state
  and the plugins' tools), Flows › Run flow and Recorder, Panels; Inspect element, DevTools and
  the view's own entries stay flat.
- One element path for every check plugin (RB-094-10): the path a finding's id is made of is one
  function in `plugins/web-common` for every page bundle and one in Rust (`schema-org`, re-exported
  by `seo`), held with the page copy of `seo` to the shared cases
  `tests/fixtures/element-path/cases.json`. Findings of `width-sweep` are re-identified once (a
  step is numbered only among siblings of its tag), those of `overlays` and `seo` at camel-case
  SVG elements and at the top-level children of a shadow root too; container paths of
  `container-queries` drop the id form. One `isObject` and one tab-row arithmetic for the shell
  and the plugins, the protocol names of the agent held to `rb-sync`.
- Core consolidation of the quality audit (RB-094-10): one check whether an address is a web
  page (`rb_util::url::is_web_url`: http(s) with a host, no whitespace) for presets, the
  breakpoint keys, environment compare, bookmarks, the config check, restore-last and the
  Lighthouse plugin; one whole-file write (`rb_util::fs::write_atomic`, synced, a temporary name
  per write) for config files, instance files, the session state, flow results, the video
  registry, the plugin catalogue and compile cache, baselines and recordings; one versioned folder
  (`versioned_dir`) for extensions and managed tools; one text cut and hex (`rb_util::text`,
  `rb_util::hex`); the project key table of the settings annotations in a file of its own.
- The dates and times of the update manifests, the update journal, the plugin index, the
  page clock and the local file names run on `time` instead of the unmaintained `chrono`
  (RUSTSEC-2026-0543); files, MCP answers and messages keep their forms. `chrono` stays in
  `Cargo.lock` through `rmcp` and `schemars`, so the advisory stays ignored in `deny.toml`
  (RB-090-77).

### Fixed

- Windows: stopping a load test ends the requests still waiting for a slow server at once (a closed socket does not wake a blocking read there); two tests held to Windows' paths (RB-096-07).
- Review of w33 (RB-096-12, RB-096-11, RB-094-22): an Android view's earlier loopback port is
  released when it loads another address, so a page that sends it on to another local port
  leaves no reverse behind; a view closed while its reverse was set gives it back; the reverses'
  state locks per device and runs adb outside the shared lock; the reverse ledger is one file
  per process, capped, a broken one moved aside. The Variables tab puts the private values back
  when the environment file cannot be written after them, and lets one save delete an
  environment and rename another onto its name.

- Review of w32 (RB-096-11, RB-094-22, RB-096-08): a captured value past the first 1000 is
  still masked in the log; a later autofill pass tells fields apart by the element, not by its
  position, so a field a choice inserts is filled and the one before it is not reported twice;
  the Variables tab refuses a rename onto an environment that keeps private values (`exists`),
  moves both in a swap, writes the environment file only after the private values were kept,
  keeps a new environment's private values when one of its name was removed in the same save,
  and keeps numbers and booleans as such; a collection is not moved where its subfolders would
  lie deeper than four levels (`too_deep`).

- A request taken from the network panel into the API client (RB-096-08) can no longer carry
  page content that sends more than the page did: a body line that would send a project file
  is left out (and the API client never writes a text part that reads back as a file), and a
  request holding variable placeholders the page wrote is skipped as `template`. Tokens in the
  path (reset and invite links, webhook and bot paths, long random segments) become variables,
  in the title and file name too; `X-Session`, `X-Auth`, `X-Sid` and `X-Session-Id` count as
  secret headers; numbers and lists under a secret JSON key become variables, and a JSON string
  that names a secret leaves the body out. Two hosts never share a folder, and a captured value
  kept in place of another is named `replaced`.
- The debug log of `adb devices -l` (RB-096-10) names each device's state with its serial cut
  to four characters, never the whole serial, model or USB path.
- Findings of test ZIP w30 (RB-096-09): a change of the user settings reaches the engines at
  once - `engines.android.enabled` switched on no longer answers `android.disabled` until a
  restart, and `engines.external`, Firefox, WebKit, Safari and iOS follow too (applied only when
  `engines.*` changed, so no running window restarts for an unrelated setting). The load test's
  editor without a saved request says where requests are made and opens the API client panel
  (refused with a message when that plugin is not loaded),
  and reads the requests again when the panel gets the focus back or a request list opens; the
  Portal panel's "Presenting over the internet is off" hint opens Settings at
  `portal.presentation.enabled`.
- An Android view starts: rBrowser reads Chrome's `/json/version` up to its length (Chrome keeps the connection open) and takes the browser target Chrome on Android names without an id; before, the view waited about a minute and failed (RB-096-10).
- Form autofill fills the fields a choice brings in the same run (RB-094-22): selects, radio
  buttons and checkboxes are set first, after each one that changed the fill waits until the page
  is quiet (`form-autofill.settleMilliseconds`, 300 ms, each wait at most 3 s), then reads the
  form again and fills the fields that appeared or became enabled (also `aria-disabled` ones) in
  up to `form-autofill.maxPasses` passes (3) - only empty ones, none twice. A privacy or terms
  checkbox is checked in a valid fill, so a submit button it enables becomes active; nothing is
  clicked or submitted. Each view's answer names its `passes`.

- Security review of 0.9.6 (RB-096-01, RB-096-02, RB-096-05): the API client masks a secret also
  form-encoded (a query API key) and in a Basic pair, takes no credential (`Authorization`,
  `Proxy-Authorization`, `Cookie`, the API key's header, a header holding a secret) across a
  redirect to another origin or from https to http, and reads no body file under `.git` or
  `.rbrowser` in a Windows spelling (`.RBROWSER`, `.rbrowser.`, `GIT~1`); the importers cap an
  OpenAPI example at 10 000 nodes and 1 MiB and refuse YAML nested deeper than 128 before it is
  built; the presentation drops a start whose stop came during cloudflared's download, denies the
  dev servers' editor launches by default (`/__open-in-editor`, `/__open-stack-frame-in-editor`,
  `/__nextjs_launch-editor`, `/__nextjs_original-stack-frame`), sends a viewer's WebSocket control
  frames only to the dev server (no HMR event such as `vite:invalidate`), refuses 8.3 short-name
  path segments, passes no client-address header and starts cloudflared with a blank `--config`
  of its own, so `~/.cloudflared/config.yml` is never read.
- The storage panel's pins load again in a trusted workspace (RB-094-15): the plugin's component
  refused the folder listing its trust check asks for, so every workspace counted as untrusted
  and no pin was read, applied or saved.
- Follow-ups of the 0.9.5 security review (RB-095-05): a changed Safari edition
  (`engines.safari.technologyPreview`) starts that edition's `safaridriver` and the old driver's
  views restart on it; the Safari driver's answers are parsed as they arrive, chunk sizes
  checked; the adb search skips relative `PATH` entries; the Android exposure check asks every
  own address of the machine and refuses a forward with none to ask at; an Android tab whose
  attach fails is closed; long Android serials get breakpoint ids with a hash suffix; adb,
  `xcrun` and `system_profiler` output is capped at 1 MiB; `rbrowser://screenshots/latest/{n}`
  follows no link and answers only a PNG; a breakpoint's `iosDevice` is always written to
  `config.local.json`.
- A narrow settings window keeps room for its tabs (RB-080-96): below 640 px the fixed 256 px
  category column gave way to a bar above the content - the categories as a select, the search
  and "Only changed" beside it - so at 400 px the tab row has the whole width.
- A plugin's own check run (RB-094-14) no longer replaces the findings panel's last run: over the
  same views only that plugin's findings, failures and skips are replaced (summary recounted, the
  export follows), its progress rows show the new result, and the plugin and view filters fall
  back to all when their choice is gone; a failure under the plugin's own code shows its reason.
- The consent check's probe reads and clears its cookies through its own page; it failed with
  "the probe's browser did not answer" (RB-094-13).
- The HTML validation of the served source failed in every view ("has no served source"):
  rBrowser turns the Page domain on in the view's session before a plugin reads a resource's
  content, which Chromium answers only then (RB-094-13).
- Dragging a view or the canvas board moves the view together with its label and frame; the view
  ran about a frame ahead (RB-094-13).
- Dragging a view by its label or panning the canvas board moves the view, its label, frame and
  footer frame together: the board no longer draws a drag at the pointer, the core sends it the
  frames of each move and moves the views once the board painted them, as for a zoom; one move
  at a time, the newest only, and the window skips a placement a newer one overtook. The pace is
  logged at `debug` (target `rbrowser::canvas_pace`) (RB-094-18).
- Audit fixes for secrets and URLs (RB-094-10): one list of secret URL parameter and header
  names (`rb_util::mask`) for MCP answers, page texts, the log rules, recordings, bug reports and
  the network panel's sessions - now with signed-URL credentials (`X-Amz-*`, `X-Goog-*`), `jwt`,
  `ticket` and SAML messages - masking the fragment's parameters and an address's user info
  too; a recording drops an address's user info and the recorder export's `baseURL` carries
  none (RB-050-01, RB-050-02); MCP's `checks_run` and `breakpoints_detect` take only the URLs
  `navigate` may load - http(s), or a file inside the project without links (RB-094-08); a
  stored Basic auth credential answers a second realm of its environment's origin once as well
  (RB-094-08).
- Hardening from the w27 security review: a bug report masks token-shaped URL path segments and
  blacks out a password field shown in clear and fields named for a password, secret, token or
  PIN (RB-093-04); a plugin updated live has its lasting CDP state reset like a removed one
  (RB-092-06); the canvas board's `view.edit_address` is refused unless its arguments are a view
  id and an optional address of bounded length (RB-092-03).
- The setup wizard no longer installs the recommended plugins when Next is pressed without Apply;
  it installs the ticked ones (RB-091-08). Back to the plugin step shows what is installed. The
  analyser no longer falls back to a mobile and a desktop view without a word: the wizard and
  `rbrowser init` (stderr) name the reason - not reachable, no answer in time, no stylesheet, the
  plugin not loaded or failed, nothing found. A wizard step that fails unexpectedly answers with
  an error instead of a spinner that never ends.
- A recorded click on a label whose text is visually hidden plays back (RB-090-93): the export
  takes the elements of the recorded tag whose whole text it is
  (`locator('label').filter({ hasText: /^\s*Menu\s*$/ })`) instead of clicking the hidden span
  the label covers; the checkbox or radio a label toggles is no `check` step of its own, the
  click stays the step; and a view where the label is not shown drops out of the step's `onlyIn`.
- macOS: every window shows its close, minimise and zoom buttons, and the menu bar has a
  Window menu with Minimize (Cmd+M) and Close Window (Cmd+W) (RB-090-98).
- Sitemap shots of a breakpoint with a pixel ratio above 1 show the page once (RB-090-90): the
  render view paints at the breakpoint's ratio, where an iPad mini at 2 showed the page twice side
  by side and repeated down the image. The main menu opens the gallery of a capture while it
  runs, as the sitemap bar does, from the run's start on: the window learns of a run when it
  starts, not with its first shot, so the menu's Gallery and Cancel no longer act on the run
  before.
- The sitemap gallery's filters work and its shots are readable (RB-090-86): the breakpoint and
  status filters reach the contact sheet (still without a script); the shots are as wide as the
  new setting Capture > Sitemap > Gallery thumbnail width says (160 to 640 px, 320 by default)
  and a click opens the large view. The gallery window shows the gallery as a page of the app
  (Nuxt UI): the run's facts, the filters, the contact sheet, a large view with Previous, Next and
  the arrow keys, and the errors; its images come from a route that serves only that run's shots
  to that window. The `index.html` in the run's folder stays the export for the system browser,
  now in the app's colours, light and dark. The form autofill fills the chosen form, else the
  page's fields but those of its navigation - header, nav, menus and search forms - and never a
  menu toggle outside a form such as a site's mobile menu checkbox; the focus no longer narrows
  a fill. A recording gets only the fields a fill changed, so a second fill of the same form
  adds no steps.
- Sitemap: the gallery opens while a capture runs, and full-page shots show the whole page
  (RB-090-88). The gallery window reads the run as it goes - from the menu, the sitemap bar, the
  command and MCP; with Open the gallery in set to your browser, a run without its `index.html`
  yet says that the file is written when the capture ends instead of failing with
  `sitemap.error.no_gallery`. A full-page shot grows the windowless render view to the page's
  height for the shot and back, so the image is no longer the top of the page repeated down to
  its length, nor striped at a device pixel ratio of 2 or 3. A shot stays within 16384 px of
  height at any pixel ratio and 64 megapixels; a page beyond fails its shot with the reason.
- A recording plays in the view it was recorded in after focus mode (RB-090-89): leaving the
  canvas or the strip for focus mode or the grid kept the views' off-screen marks, so Play took
  the focused view for hidden and failed with "every view of the run is hidden in the layout".

- A breakpoint's DPR is at least 0.25 and its width and height at most 10000 CSS px (security
  review w23): a full-page capture grows a view by them. A size typed into a view, `view_add`
  and the size shortcuts keep to the same limits.
- Audit fixes of the core (RB-094-10): a width sweep's late restore never undoes the width of a
  lease taken after it (RB-093-01); a version of an extension or a managed tool set aside during
  an install that cannot be put back is logged as an error, no longer dropped silently; a flow
  result no longer leaves `result.json.partial` behind; the coverage plugin counts a file at the
  page's origin with `:443` or `:80` written out as first-party; Lighthouse refuses a URL with
  leading whitespace or without a host.

## [0.9.0-beta.2] - 2026-11-19

### Added

- Playing a recording shows where it is (RB-090-84): the Recorder panel marks the number of the
  step each view runs, marks a failed step red with its reason and scrolls the running step into
  sight (`automation.recorder.followPlayback`), and shows each step's pause.
  `automation.recorder.minPauseMs` sets the least pause at the pace As recorded. A flow run skips
  the views a layout hides and names them, or shows a layout of its own
  (`automation.flows.hiddenViews`, `automation.flows.runLayout`). Every run's end and each failed
  view's step and reason go into the log; Run all, recorder checks and exports say when they
  start or end.

- rBrowser for macOS in every release (RB-090-80): `release.yml` builds `rBrowser.app` for Apple
  Silicon and for Intel natively from the signed plugins, signed ad hoc and checked, and
  publishes `rbrowser-X.Y.Z-macos-aarch64` and `-x86_64` each as a DMG and a ZIP beside the
  Windows ZIP, in `SHA256SUMS`; a release without one of them is not published. The release
  text lists the downloads and how macOS lets an app start that is not notarised (Open Anyway, or
  `xattr -dr com.apple.quarantine`); the Homebrew cask follows each release
  (`brew install --cask degoya/rbrowser/rbrowser`), now for macOS 13 or later. On macOS the app
  offers a new version as a download of its release page; it does not replace itself.
- First-party plugins as rDownloader handles them (RB-091-04): the package still carries every
  first-party plugin, but a start installs only the chosen ones - a fresh installation what the
  first-run wizard marks (Skip, or no answer by the next start: the recommended set; with the
  wizard off the recommended set at once), an existing one keeps all it had, a plugin new in a later release waits in
  Settings › Plugins › Catalog and a removed one does not come back (`--install-all-bundled-plugins`
  installs all). A chosen plugin comes from the official catalogue when it offers a newer
  version than the shipped one with the same key and no new permission, otherwise from the package
  (a newer copy that does not verify leaves the shipped one to load)
  (`plugins.catalog.preferRepository`). The catalogue lists the shipped plugins with one-click
  Install and Remove and their updates, which `plugins.catalog.autoUpdate` can install by itself.
  A project that uses a plugin that is not installed says so in the status bar with an Install
  button (`plugins.catalog.missingNotice`); MCP `plugins_list` names `missing` and `available`.

- The setup wizard (RB-091-05): optional, once per person at the very first start, every step
  skippable - welcome, plugins (the first-party plugins by category with search; the wizard only
  marks what gets loaded: at a fresh install no optional plugin loads until it answers, the
  recommended ones ticked, anything can be unticked, an installed one included; one confirmation
  names what is installed with its rights and what is removed, then one install with progress,
  from the package's bundle without a network; skipping the step or the setup installs the
  recommended set, a choice made before stays as it is), project (a folder, WSL included, the dev server's address and the
  analyser's proposal, a mobile and a desktop view or chosen devices; the config `rbrowser init`
  writes, shown before it is written), basics (theme, grid or canvas, sync) and a summary. A
  project folder without a config says "Set up project" once in the status bar. "Set up
  rBrowser..." (`⌘ , W`, the palette, App in the main menu, Settings › General › Start) opens it
  again; `ui.firstRunWizard` switches it off for a team. MCP can neither open nor answer it.
- The plugins as a release of their own (RB-090-79): every tag `vX.Y.Z` also publishes
  `plugins-vX.Y.Z` ("rBrowser plugins X.Y.Z") with the signed first-party `.rbplugin` packages
  and their own `SHA256SUMS` (no offline bundle: the plugins the application's package carries
  are the way without a network, RB-091-04); a pre-release with a beta tag, never `latest`. The newest plain tag also signs the official
  plugin catalogue's index and replaces it in the fixed release `plugin-index`, the address the
  app reads; a pre-release leaves the index alone. `.github/workflows/release-plugins.yml` builds
  and signs, `scripts/release-plugins.sh` stages, gates and publishes.
- Self-update is on (RB-090-14): rBrowser checks for a new version at every start
  (`updates.check`, on by default) and shows it in the status bar with its size and notes;
  "Install and restart" (`⌘ , I`) downloads it, checks the signature of the release key and the
  SHA-256, replaces the program folder through `rbrowser-updater.exe` (shipped beside
  `rbrowser.exe`) and starts the new version, which proves itself at its start or is taken back.
  `updates.autoInstall = onQuit` installs a downloaded update when rBrowser quits, never holding
  the quit up. "Check now" (`⌘ , U`) and the install sit under the switch in Settings › General ›
  Updates. The channel is automatic by default (beta for a pre-release build, stable otherwise);
  the beta channel finds its manifest through the GitHub release list. With other projects'
  windows open the install waits for them; Scoop, winget, Homebrew, deb and rpm installations are shown their
  package manager's command, other installations the release page. MCP `update_status`,
  `update_check` and `update_install`, which only asks the person. The notice about an old CEF
  stays silent while updates are checked. The first release that updates itself is
  0.9.0-beta.2; 0.9.0-beta.1 is updated by unpacking the new ZIP.

### Changed

- Every run a command starts in the background shows itself (RB-090-85): the plugins' checks,
  the environment comparison, the visual compare and baseline update, captures,
  `sidecar.restart` (now also its end), `engines.update`, `wsl.restart_helper` (start and end),
  the update, plugin catalogue and extension checks and the diagnostics ZIP name their start in
  the status bar. A message's Setting opens the field, or the section of an area (`sitemap`,
  `visual`) ringed on its tab, instead of the settings' first page; the settings lint checks
  every notice's target. Notices drop an error's code in front of its text.

- The official plugin index lists every first-party plugin with its group and keeps the last
  version of each plugin per plugin contract, and the app ignores index fields it does not know; the plugin release no longer has an offline ZIP -
  the application's package is the offline way (RB-091-04).

### Fixed

- A sitemap capture takes its pictures and says that it runs (RB-090-83): its render views - and
  the environment comparison's - are windowless browsers that paint, not hidden CDP targets, which
  never answered a screenshot. A shot has its own deadline (`sitemap.shotTimeoutSec`, 15 s); a
  failed one names its reason in the log and in `run.json`. A start says "Sitemap capture started:
  N pages × M breakpoints" and opens the sitemap bar by itself; a second start shows the running
  capture. The gallery opens in a window of its own, which loads only its run's files and runs no
  script, or - your own setting `sitemap.galleryOpensIn` - the system browser; never in the
  views: `sitemap.gallery` (`Leader C I`), MCP `sitemap_gallery` (always the window). Render
  views have unguessable markers and never take a popup or another context's page.
- A played recording clicks the link the person picked when the page shows its name twice - a
  hover menu's link and a content link: the recorder keeps the ancestor that makes it unique and
  the flow looks inside it (RB-090-84). At the pace As recorded a hover rests before the next
  step, and the recorded pauses no longer count against the flow's timeout.
- A recorded flow no longer fails on the pointer's way and on duplicate names (RB-090-81): the
  setting Record hovers (`automation.recorder.recordHover`) is a choice - only menu hovers (the
  default: the hover that opened the submenu the next click goes into), all, or none. An old
  `true`, which kept every hover the recorder guessed, loads as `menu`, `false` as `off`; the
  schema still accepts the switch as its earlier form. The agent records a role or text match's
  index among the visible matches (`visibleNth`), and the export writes
  `.filter({ visible: true }).nth(visibleNth)` (`.first()` for the first) instead of the leader's
  `.nth(n)`: a breakpoint that hides an element finds its visible twin, the fourth of several
  visible cards' links stays the fourth, and a name two elements share is no strict-mode error.
- Main menu rows that did nothing (RB-090-82): ≡ › Plugins › List plugins, Install, Enable,
  Disable, Remove and Trust a key open Settings > Plugins (the list answered only MCP, the
  others were not implemented); a plugin's panel command (Show the colour picker panel and the
  other panel rows) opens the docked panels also when they are folded to their rail, instead of
  only marking the panel there; a plugin's call on the active view (the colour picker's palette) and the engine
  pick take the first view while no view is active. The Split and Canvas rows, the canvas centre
  and placement, focus stepping, a recording's pause, stop and check and the sitemap's cancel
  are choices only where they act; the minimap row is checked by this session's switch; pausing
  hot reload says "Hot reload paused" or "Hot reload resumed".
- The toolbar menus run their rows again: with CEF 156.0.30 the menu tells its close before the
  row chosen, so every choice of ≡ and the split buttons was lost (the theme, Create diagnostics
  ZIP, ...). A submenu closing (the pointer moving on, or its row chosen) no longer counts as the
  menu's close, and the close is told a moment later; a `&` in a row shows as itself, not as `&&`. The
  macOS build compiles again (the Print entry's id is `u32` there).
- The setup wizard scrolls (RB-091-05): in the pages window a long plugin list hid Back and Next;
  the wizard scrolls itself and keeps its footer in sight. Its plugin step no longer says "No
  network" when only the plugin catalog cannot be reached (none is published before the first
  full release); it says the catalog is not reachable and the plugins install from the package.
- Settings › About names how this installation updates itself - not in this build, switched off,
  only when asked, or at every start - instead of always "Updates are disabled in this build"
  (RB-091-05).
- The public CI after the first push (RB-090-12): the E2E workflow ends green with a notice while
  `scripts/e2e-windows.sh` (RB-090-05) does not exist, and the package channels workflow ends green
  with a notice before the first plain release instead of failing on "release not found".
- A recording takes what Form autofill fills (RB-090-78). The autofill sets values by script,
  which the recorder rightly does not capture, so a recording ended without the fill - only the
  checkboxes it clicked showed as steps. Now each field it sets while a recording runs becomes a
  step - `fill` with the value set, `check` or `uncheck`, `select` - named as a typed field is
  named, in the views it filled (a view with other values, such as a German phone number, gets
  its own step); a password or secret field is recorded as its placeholder, and a checkbox the
  agent saw clicked is one step, not two. A flow exported from the recording fills the same
  values. `automation.recorder.recordAutofill` (on by default) turns it off; Settings ›
  Automation › Recorder.

## [0.9.0-beta.1] - 2026-11-17

### Added

- The Lighthouse bar under every view (RB-090-73): below the vitals bar, the four category
  scores of the newest Lighthouse run of the view's page and breakpoint as gauges - the shape of
  the rating, the score and the category's name as the width holds, in Lighthouse's colour (0-49
  red, 50-89 orange, 90-100 green) - in every layout, on the canvas and in a view's own window. A
  page without a run (or with a run of another address only) has no bar; the tooltip names the
  run's local time and every score, a click opens the report. `ui.viewFooter.lighthouse` (on by
  default), `⌘ O Q` / `ui.lighthouse_bar.toggle`, a check entry beside the other bars in the
  page's context menu, the label strip's view menu and ≡ › View.
- The vitals bar under every view (RB-090-71): LCP, CLS and INP with the shapes of their rating
  (a dash for one not measured yet) below the breakpoint bar, in every layout, on the canvas and
  in a view's own window; all five vitals in the tooltip, a click marks the element of the worst.
  A vital the engine never measures is left out, and a view that can measure none (WebKit) has
  no bar. `ui.viewFooter.vitals` (on by default, replaces `ui.cellLabel.vitals`, whose value an
  older settings file carries over), `⌘ O W` / `ui.vitals_bar.toggle`, a check entry beside the breakpoint bar's in the
  page's context menu, the label strip's view menu and ≡ › View. The engine window's state stands
  in the engine badge (`Firefox · window closed`), whose menu offers to reopen, restart or show
  the window; the label strip and the canvas board's labels lose their vitals badge.
- Form autofill: a form filled by hand becomes a preset (RB-090-70). *Take form as preset* - the
  page menu's Tools > Fill forms, the autofill panel's header, `Ctrl+Shift+Space D T`, MCP
  `autofill_capture` - reads the values of the right-clicked or focused field's form (else the
  page's only form) and opens an inline dialog in the panel: the name, a box per field with its
  value, the fields not taken with the reason. Password fields, fields that look like secrets
  (password, token, PIN, one-time code, card data) and file inputs are never taken; hidden inputs
  only with `form-autofill.captureHiddenInputs` (off). Presets live in the plugin's own
  `.rbrowser/out/form-autofill/presets.json` and fill like a settings profile; MCP
  `autofill_save` and `autofill_presets`.
- The activity bar (RB-090-67): a strip of 28 px that appears only while something runs - a
  video (its time, the views, Stop), a recording (its steps, Pause/Resume, Add check, Stop, then
  Play last), a flow run (views through, Results, Cancel), a check run (jobs through, Cancel), the
  portal (devices, QR code, Disconnect), views out of sync (Resync) - one entry each, every
  control a command of the registry. It takes its height from the views below the toolbar or
  above the status bar and gives it back after the last activity; an entry's close button folds
  it into its compact indicator in the toolbar until the activity ends. Settings
  `ui.activityBar.position` (`top` | `bottom`), `ui.activityBar.mode` (`bar` | `indicator`) and
  `ui.activityBar.show.<activity>`; the shell's `activityBar` and `collapseActivity` requests and
  the snapshot's `video` and `activityBar`. Never in a view's screenshot or video.

- The canvas board's labels show a zoomed page's level ("110%") as the grid's label strips do:
  the tooltip says the breakpoint applies differently, a click, Enter or Space sets the page back
  to 100 % (RB-090-28).
- Every function in a menu (RB-090-63): an always-visible main menu `≡` at the toolbar's end
  with the command palette, what left the toolbar row, the bookmarks and a submenu per area
  (View, Layout, Sync, Emulation, Environments, Capture, Checks, Overlays, Tools, Flows, Panels,
  Plugins & extensions, App) - check marks, radio choices and each command's shortcut beside it;
  split buttons DevTools, Screenshot, Video and Checks in the toolbar; Videos in the page menu's
  Panels. The theme, the breakpoint bar switch and the breakpoint analyser moved into the main
  menu; `layout.zoom_to_fit` is the one zoom-to-fit in every layout mode (`canvas.zoom.fit` and
  its `⌘ L A` are gone - a script or agent that still runs `canvas.zoom.fit` gets
  `layout.zoom_to_fit`, and a saved binding of it is dropped without a notice; MCP `canvas_zoom`
  `"fit"` is unchanged). A menu longer than the native menu takes ends in a row that opens the
  command palette. The page menu adds Sync › this
  view and its navigation, Session › the sessions and clearing one, CPU throttling in Emulation ›,
  Tools › Peek, the raw JSON/XML view and Fill forms ›, and the fonts, outline, hreflang, parity
  and well-known checks under Checks ›; Settings › Automation runs the CDP endpoint, Playwright
  script, new token and the MCP Claude Code line from buttons under their switches. The settings
  `ui.toolbar.themeToggle` and `ui.toolbar.footerToggle` are gone (a file that still sets them
  loads).

- Play a recording: "Play" in the Recorder panel, `Ctrl+Shift+Space F Y` (`recorder.play`) and
  MCP `recorder_play` play a stored recording in the views of its breakpoints through the flows
  runner - the flow the export would write, held in memory, nothing in `.rbrowser/flows/` - with
  a flow run's trust gate, progress, notices, Cancel and result. The pace beside the button,
  `automation.recorder.playbackPace` by default, plays fast or with the pauses as recorded (at
  most 10 s each), and the flow and test export follow the setting. The export dialog saves a
  flow into `.rbrowser/flows/<name>.ts`, where the Flows panel lists it - replacing a flow of that
  name only when ticked, keeping the workspace trust for that one file -, closes, and says where
  the files went (RB-090-66).
- A Portal button in the toolbar: off, a click starts the portal and shows the QR code to pair a
  phone or tablet (where Settings allow the portal); on, it shows the number of paired devices
  and opens a menu with the QR code, the devices, disconnecting a device and stopping the portal.
  Its tooltip names the shortcut; in a narrow window it moves into the "More" menu;
  `ui.toolbar.portalButton` hides it (RB-090-62).
- Colours for the rulers and grids: the ruler bar, its ticks and numbers, the guides, the column
  overlay's fill and edge and the baseline grid, each with its opacity (`#rrggbb` or
  `#rrggbbaa`), as settings in Settings > DevTools & Debugging > Rulers (`rulers-guides.colors.*`,
  for you or the project) and as your own colours in the rulers panel; every view follows a
  change at once. The settings window's colour fields open a colour picker in a popover, with an
  opacity slider where the key takes one, beside the hex field. MCP `settings_set` and
  `rulers_set` (`colors`) (RB-090-58).
- Todo search: a search field in the Todo panel (`Ctrl+F` in the panel takes the caret, Esc
  clears it) and an "Only open" switch show the todos whose text or tags hold every term -
  without case and accents, `@high` a tag by the start of its name - with their headings and
  projects as context and the words marked; it applies within the "Add to" section and beside
  the tag filter, each cleared on its own, and stays over a panel switch. MCP `todo_list` takes
  the same `query` (RB-110-18).
- The recorder and flows without keys: the toolbar shows a pause/resume and a labelled Stop
  button beside Recording (n); the page menu has Recorder (start, pause/resume, stop, and
  Check: element visible, Check: text visible, Wait for element for the element under the
  pointer - steps for the views that have it) and Run flow (the project's flows, Run all flows);
  the Recorder panel lists the recordings and edits a recording's steps (rename, delete, move,
  change a value; `onlyIn` shown) and exports it as a flow, a Playwright test or both with a
  highlighted preview, copy and save; the Flows panel runs a flow in all views or one, runs all,
  shows each view's status while it runs, cancels, and shows the last result per view with step,
  line, message and screenshot. New `waitFor` step in `recording.v1` and both exports. Commands
  `recorder.check` (`Ctrl+Shift+Space F K`, a visible check on the element you click),
  `panels.recorder` (`F V`) and `panels.flows` (`F F`, MCP `flow_list`) (RB-090-60).
- Force state in the page's context menu: `:hover`, `:active`, `:focus`, `:focus-visible`,
  `:focus-within`, `:visited` and `:target` on the element under the pointer, in this view or
  in all views (the same element found by the sync's selectors), so a CSS hover menu stays open
  while you drag a ruler into it; `:hover`, `:active` and `:focus-within` reach every ancestor
  as a real hover does. The states stay until Reset states or a navigation, and the status bar
  says where they are forced and which views lack the element. Commands
  `view.force_state.set` (`Ctrl+Shift+Space D H`: hover on the element you click) and
  `view.force_state.reset` (`Ctrl+Shift+Space D Q`), MCP `views_force_state`. Menus that open
  only through JavaScript do not react (RB-090-53).
- The Outline panel analyses the page's structure: each tab counts its findings in the colour of
  the worst, every finding sits at its entry with its severity, the page's findings (no `h1`, no
  `main`, content outside the landmarks) stand above the tree and a click marks them, and "Only
  problems" filters the tree. New rules: no `h1`, a first heading that is no `h1`, an empty
  heading or one of an image without `alt`, a very long heading (`outline.headingMaxLength`),
  more than one `main`, `banner` or `contentinfo`, a nested `banner` or `contentinfo`, landmarks
  not told apart by their names, content outside every landmark, a focus order that jumps against
  the visible order, and over the breakpoints a heading or landmark one view lacks that another
  shows. Shared ids with `seo` and `a11y-axe` merge the same problem into one finding; MCP
  `outline_get` answers the findings with severity, and `findingsOnly` (RB-091-03).
- Lighthouse reports open inside rBrowser: "Open report" per breakpoint in the Lighthouse panel
  and `Ctrl+Shift+Space K R` show the HTML report in a report window of its own (setting
  `lighthouse.reportWindow`: one window, or one per report), with no access to rBrowser's IPC,
  the network or the plugins, served only from `.rbrowser/out/lighthouse/` of a trusted project
  (no `..`, no link). The panel's history lists every run with its scores, deletes a run after
  an in-place question (`Ctrl+Shift+Space K D`) and compares two runs of a breakpoint by scores
  and metrics (FCP, LCP, TBT, CLS, Speed Index); `lighthouse.keepRuns` (protected; 0 keeps
  every run, the default) deletes the oldest runs beyond a number set after each run. MCP
  `lighthouse_reports` lists, opens and compares (RB-091-02).
- A signed plugin catalogue in Settings > Plugins > Catalog: the official catalogue and plugin
  repositories you add with their https address and public key, with search and filters (kind,
  installed, updates). An index is refused with a wrong signature, after it expired or with an
  older sequence than seen before; a package installs only with the SHA-256 and size its index
  names, verified like a `.rbplugin` file and with exactly the capabilities and publisher key the
  install dialog showed. The same plugin from two repositories shows both origins; updates come
  from the repository a plugin came from. `Ctrl+Shift+Space X B` opens it, `X Q` checks for
  updates, `plugins.catalog.checkOnStart` checks at every start; MCP `plugins_catalog_list` only
  reads. `rb-plugin-pack index`, `sdk/ci/repository.yml` and the signing job of
  `sdk/ci/plugin.yml` publish a repository (RB-091-01).
- The todo panel shows Markdown headings (`#` to `######`, as CommonMark reads them) as
  headings instead of notes: without the `#`s, bold and sized by level, the two top levels
  underlined; Todo+ projects stand out as headings too. A heading is edited as its whole line
  (`## ...`), moved and deleted as its own line, and a new todo can go below it; the file stays
  byte for byte the same apart from the line changed. MCP `todo_list` names `kind: heading` with
  its `level`, `todo_add` takes a heading's line as `under` (RB-110-15).
- The todo panel opens fast and filters by section: the search for `.todo` files skips hidden
  folders and the folders the project's `.gitignore` files ignore without listing them, runs
  only at the panel's start and on Reload, and the panels window keeps the panel while another
  is shown (a panel may ask for that with `{ type: 'keep' }`). The "Add to" choice cuts the
  list to that heading's or project's section, shown as a filter that clears with one click.
  Heading levels H1 to H6 get six distinct sizes, weights and spacing. A new todo written as a
  heading (`## Sprint`) adds that heading line without a box - in the panel, where it is then
  chosen, and over MCP `todo_add`, so an agent can add a heading and the tasks `under` it
  (RB-110-16).
- A Videos panel for the recorded videos (`Ctrl+Shift+Space C T`): the project's recordings by
  day, all views of a recording side by side and played together with one Play/Pause, scrubber
  and speed, Show in folder, Copy path, and Delete after an in-place question; MCP `video_list`;
  retention settings `capture.video.keepRecordings` and `capture.video.keepDays`, and
  `capture.video.autoplay`. The videos play from a route of their own that serves only
  recordings rBrowser made in the open project (a registry outside the project), to rBrowser's
  own pages only; a project's retention never deletes more than the user's allows. Plugin ids
  `video`, `env-compare`, `flows`, `recordings` and `screenshots` are reserved (RB-110-12).

- rBrowser follows the Windows text size (Accessibility > Text size, 100 to 225 %) and its
  changes without a restart: toolbar, status bar, label strips, breakpoint bars, settings,
  palette and panel dock grow, the views move to make room and the toolbar's "More" menu takes
  what no longer fits; the pages in the views keep their size. Setting
  `ui.followWindowsTextSize` (on) (RB-090-26).
- The plugin panels follow the same text size, live: Console, Network, Lighthouse, Todo and
  every other panel on Nuxt UI grow with the shell (RB-090-39).
- A todo plugin: the project's `.todo` files in the Todo+ format as checklists in a panel -
  check, cancel, add, edit, move, delete and archive todos, filter by tag; the same file stays
  editable in VS Code's Todo+ (unchanged lines byte for byte), a file changed elsewhere is read
  again, never overwritten; MCP `todo_list` (in pages: `from`, `next`, `rev`, so large files
  fit a panel's 64 KiB message), `todo_add`, `todo_toggle`, `todo_edit`,
  `todo_archive`. Plugins may now replace project files their manifest names (`project-write`,
  trusted workspace only) and list the project root (RB-110-13).
- Page zoom inside a view: Ctrl+0, Ctrl+Plus and Ctrl+Minus as in Chrome, the level as a badge
  in the label strip (a click resets it), `view.page_zoom.*`, MCP `view_page_zoom`, the
  `views.pageZoom.*` settings (RB-090-20).
- A public README for github.com/degoya/rBrowser, put in place of the private one by
  `scripts/export-public.sh`; the release flow in rCommander's order, with a `ci/` test branch
  (RB-090-24).
- A macOS test build: `rBrowser.app` for Apple Silicon and Intel with the CEF framework and its
  five helper apps, ad hoc signed, as a ZIP and a DMG from `scripts/package-macos.sh` on the
  macOS runners of CI; a menu bar (About, Settings, Hide, Quit, Edit) and the macOS shortcut
  reservations (Command keys, Spotlight, input sources, Mission Control). WebKit, external
  windows in the cells and the helpers' sandbox follow (RB-090-15).
- Every first-party plugin panel on Nuxt UI: the inspector panels (Console, Network, Storage,
  Snippets, Error report, Coverage - long lists stay fast, Coverage one row per file), the check
  panels (SEO, PWA check with tabs, Outline with section tabs) and the tool panels (colour picker,
  image overlay, rulers and guides, overlays, simulators, form autofill with a row per field,
  screenshot editor); the hand-made panel kit is gone (RB-080-84, RB-080-85, RB-080-86).
- Social previews pick each card from the platform's published rule, with the reason and a toggle
  to the other variant; iMessage, Telegram, Microsoft Teams, Mastodon, Bluesky, Threads and Signal
  can be selected; new finding `social.card_small` (RB-080-99).
- Plugin authors: the SDK's feature template has a Nuxt UI panel built on a copy of rBrowser's
  panel-ui; hello-panel moved to Nuxt UI as the worked example (RB-080-87).
- The public side on GitHub, after rCommander and rDownloader: `scripts/export-public.sh` builds
  the public mirror `github.com/degoya/rBrowser` (one commit per release, without `docs/`, the
  planning, the agents' files and the maintainer's scripts; private names, dead links and secrets
  refused), `scripts/export-public-wiki.sh` the public wiki from the handbook (private pages and
  sections left behind), both with an `--out` dry run; nothing is pushed without `--push`
  (RB-090-12).
- Package channels: a release moves the Scoop bucket `degoya/scoop-rbrowser` and opens the winget
  pull request (`degoya.rBrowser`), each once its secret is set (RB-090-13); the Homebrew tap
  `degoya/homebrew-rbrowser` and the signed apt and dnf repository `degoya/rbrowser-packages` are
  prepared and follow with the macOS and Linux packages (RB-110-04). `scripts/package-managers.sh`
  renders the files from `packaging/` with the release's `SHA256SUMS`.
- rBrowser's own theme light, dark or system everywhere - shell, panels, every window, native
  strips, menus and the title bar - switched without a restart: `ui.theme.cycle`
  (`Ctrl+Shift+Space V 0`), `ui.theme.set` (`V =`), the palette and a toolbar button
  (`ui.toolbar.themeToggle`); the emulation's sun and moon now say they set the pages' colour
  scheme (RB-080-95).
- Canvas zoom up to 200 % of the page size, the level with a slider, minus/plus and presets in the
  status bar (`ui.statusBar.canvasZoom`), `Ctrl+0` on the canvas board for 100 % (the one
  exception to the reservation list, only with the board focused), `canvas.zoom.reset|in|out|fit`,
  MCP `canvas_zoom` (RB-080-94).
- Run checks shows progress per plugin and view, elapsed time and an estimate from the last run,
  each plugin's findings as soon as it is through, and Cancel (`checks.cancel`,
  `Ctrl+Shift+Space K Q`, MCP `checks_cancel`) (RB-080-89).
- Links from rBrowser open in the system browser or the active view (`app.open_link`, http(s)
  only, setting `ui.openLinksIn`); panels ask for it with the post `open_link` on a click
  (RB-080-92).
- Nuxt UI in the plugin panels: `plugins/web-common/panel-ui`, one Vite build per panel, offline
  in the sandbox; Lighthouse and Fonts moved first, Fonts with a preview per font and face
  (`fonts_preview`, settings for file size, sample text and size) (RB-080-83).
- A toolbar switch at the right end for the plugin panels (`ui.toolbar.panelsToggle`)
  (RB-080-79).
- Settings › Plugins in tabs Installed, Disabled and Settings, with counts (RB-080-80).
- i18n lints: no `{code}` text, no count without a plural form, every plugin command has a title
  (RB-080-90, RB-080-82).
- Engine badge at the right end of every view's label strip and on the canvas board: see the engine,
  click to switch it; `view.set_engine` (`Ctrl+Shift+Space B E`), MCP `views_set_engine`, setting
  `ui.engineBadge` (RB-080-33).
- Firefox detection (RB-080-32): every installed edition (release, ESR, Developer Edition, Nightly)
  is found with its version and offered at `engines.firefox.path` ("Found: ..." placeholder, a list
  when there are several, a gone path marked); MCP `engines_list`; `rbrowser engines firefox --list
  [--json]`.
- `views_list` answers `externalWindow: open|closed` for Firefox and WebKit views; every start of an
  external window is logged with its reason (RB-080-38).
- Firefox and WebKit views sit in their grid cells like Chromium views (RB-080-39): the engine's
  window gets the main window as owner, loses frame, title bar and taskbar entry and follows its
  cell through layout, move, resize, minimize, restore, mode switch and canvas pan; Firefox runs
  without tabs and address bar. Setting `engines.external.embed`, **experimental and off by
  default** (the owner's test of 2026-11-13: the window shows the page at 100 % cut to the cell,
  and WebKit's shell keeps its toolbar - a follow-up job scales the page to the cell); on, `views_list`
  answers `embedded` and the badge says "in its cell".
- Settings window: the JSON view colours keys, strings, numbers, booleans, `null` and punctuation in
  both themes; `settings.jsonHighlight` (General › Appearance) switches it off; Copy still delivers
  plain JSON (RB-080-42).
- Web Store extensions can be removed again (RB-080-30): a Remove button per installed extension in
  Settings › Engines › Chromium/CEF, the command *Extensions: remove a Chrome Web Store extension*
  (`Ctrl+Shift+Space X X`) and MCP `extension_remove`, which asks in the window first; the folder
  with every version, pointer and staging leftover goes, what this start loaded stays until the next
  start.
- Framework DevTools (RB-080-31): the Vue panel shows Pinia stores (state, getters), the router
  (current route, routes) and the kit's timeline as tabs; MCP `views_list` names per view the
  frameworks its page runs (`frameworks`); the Timeline tab hangs on the new setting
  `framework-devtools.vue.timeline` (on).
- The page's context menu offers rBrowser's block below Chromium's entries: Inspect element
  (`devtools.inspect_at`, `Ctrl+Shift+Space D S`, MCP `devtools_inspect_at`), Open/Close DevTools,
  Screenshot (this/all views, element under the cursor, overview), Video (this/all), Checks (run
  all, every check plugin listing `context_menu` in its manifest, Web Vitals, Analyze breakpoints),
  Panels; what cannot run now is disabled; `ui.pageContextMenu` switches it off (RB-080-43).
- Add and remove views in every layout mode (RB-080-41): `layout.hidden` in `config.local.json`
  keeps which breakpoints are open, in presets too (`hidden`); an explicit grid keeps the empty slot
  under a `hidden: <label>` entry, computed grids, strip, split and canvas reflow; `Ctrl+Shift+Space
  V +` / `V -` / `V *` (`view.add`, `view.remove`, `view.add_size`), the active view's label menu
  (Remove, Set as active, Size, engine, Pop out), the canvas board's context menus, the toolbar's
  "+" with the hidden breakpoints, Add all and New size...; MCP `view_add`, `view_remove`,
  `view_add_size`; setting `ui.viewContextMenu`. Only
  breakpoints with a cell in the grid in effect are offered; the last view of that grid stays, in
  focus mode too; Remove view closes an ad-hoc view of the quick pick.
- HTTP debugger in the network panel (RB-080-37): breakpoints on requests and responses with
  continue/abort per view or all, compose and repeat, map local and map remote, extended rewrites,
  sessions as HAR; MCP `network_breakpoints`, `network_continue`, `network_abort`,
  `network_compose`, `network_repeat`; settings `network-panel.breakpoints.timeoutMs`,
  `network-panel.allowComposeFromMcp` (protected), `network-panel.mapLocalRoots`,
  `network-panel.rewrite.maxBodyBytes`; chord `Ctrl+Shift+Space P R`. Compose and repeat follow no redirect; over MCP held bodies leave only with
  `mcp.allowEvaluate`, and `network-panel.mapLocalRoots` is protected.
- Embedded Firefox and WebKit views show the page at the breakpoint's width scaled to the cell;
  setting `engines.external.scaleToCell` (on) (RB-080-40).
- A footer under every view names its breakpoint, the range of widths it stands for, the view's size
  and the page's media queries that hold there (`md | 768-1023px | 768 × 1024 @2x | min-width 768px,
  max-width 1023px`), in the breakpoint's colour, in every layout, on the canvas and in a popped-out
  window; a view with nothing to say has none; `ui.viewFooter.visible`, `ui.viewFooter.fields`,
  `breakpoints[].color`, `Ctrl+Shift+Space O B` (`ui.view_footer.toggle`) (RB-080-44).
- `devtools.openOnInspect` (on): Inspect element opens closed DevTools. Off, it neither opens nor
  moves DevTools and is disabled unless they are open for the view; MCP `devtools_inspect_at` always
  opens (RB-080-45).
- An Emulation submenu in the page's context menu: colour scheme (light, dark, no override), reduced
  motion, offline, location, time zone and reset, for the clicked view and for all views (marked by
  what every view has); Analyze breakpoints as a menu entry; `ui.pageContextMenuEmulation`
  (RB-080-49).
- "New size…" lists sizes, devices and Custom; `ui.newViewSize.devices`; MCP `view_add_size { device
  }` (RB-080-46).
- Ctrl+wheel zooms the canvas with its views around the pointer, over a footer too
  (`layoutModes.wheelZoom`); the middle mouse button pans it like Space+drag, a middle click without
  dragging still reaches the page and Escape cancels the pan (`layoutModes.middleButtonPan`)
  (RB-080-50).
- `ui.viewFooter.analyze` (on): the footer reads the media queries of the page each view shows from
  the loaded page itself after every load (nothing is requested again), kept per document beside the
  Analyze Breakpoints result (RB-080-47).
- Bookmarks follow the environment: a page under the environment in effect, or under another with
  the same base path, is stored as its path and opens on the environment in effect
  (`bookmarks.followEnvironment`, the dialog's "Follows the environment"); `bookmarks.make_relative`
  (`Ctrl+Shift+Space P P`) converts existing ones; MCP `bookmarks_list`, `bookmarks_make_relative`
  (RB-080-48).
- Settings > Plugins shows per plugin the reason it did not load, links to its settings and commands
  and an on/off switch (from the next start); `plugins.showBundled`; `settings.open_plugins`
  (`Ctrl+Shift+Space , X`) (RB-080-51).
- `ui.toolbar.overflow` (`menu` | `wrap`: a second toolbar row, the views move down); the "More"
  menu shows a mark while a hidden control reports something; command `ui.toolbar.overflow.open`
  (`Ctrl+Shift+Space P T`) (RB-080-52).
- `layoutModes.showPartialViews` (on, applies at once): off hides a canvas view as soon as any part
  of it leaves the area (RB-080-55).
- `engines.lighthouseBrowserPath` (Engines > Node runtime > Lighthouse browser): the full path of
  the browser Lighthouse runs in, or empty to search (RB-080-58).
- View footer: settings `ui.viewFooter.wrap` (auto|never) and `ui.viewFooter.align` (center|start)
  (RB-080-53).
- Plugin panels dock left, right or below the views or float in their own window; the views keep a
  quarter of the window even with DevTools docked on the same side; drag the bar or use the header's
  Smaller/Larger to resize, size kept per side; the panel shown goes along when the panels move;
  `panels.dock`, `panels.dockSize`, `Ctrl+Shift+Space P Left/Right/Down/Up/X`, MCP `panels_dock`
  (RB-080-57).
- The footer under every view is switched from a toolbar switch (shortcut in its tooltip), a check
  row of the view menu and of the strip's size list, and a check entry of the page's context menu;
  `ui.toolbar.footerToggle` hides the toolbar switch (RB-080-60).
- Docked plugin panels fold to a strip and open again at their size (`panels.dock.toggle`,
  `Ctrl+Shift+Space P Z`, MCP `panels_dock { collapsed }`, `panels.dockCollapsed`); only the person
  unfolds them, a panel opened by a command or a check while folded is marked in the strip; a Dock
  menu with the current position ticked; panel icons from the manifest (`icon`) and a side bar that
  shows icons only (`panels.sidebar`) (RB-080-61).
- The plugin panel kit (`plugins/web-common/panel/kit.css`, `icons.js`, Lucide icons, ISC); a
  panel's settings carry `ui.theme` (RB-080-62).
- Syntax colours wherever rBrowser shows code: the console's JSON messages, stack traces, command
  line and results (strings stay text); the network panel's bodies by MIME type, headers and
  editable fields; snippets' editor, results and stacks; storage values and the value editor; and
  the settings' JSON view; one offline highlighter, light and dark; page content only ever shown as
  text; repeated texts are not re-coloured, typing colours once per frame; more than 200 KiB stays
  one colour with a note; `ui.syntaxHighlight` (General > Appearance, on) (RB-080-65).
- Settings show a key only on request, via the title tooltip and a copy button; `settings.showKeys`
  (RB-080-70).
- Every settings category with several sections uses tabs, remembered per category;
  `settings.sectionTabs` (RB-080-71).
- Known values are chosen by name in a searchable list (plugins, panels, page scripts, rule
  overrides, environments, breakpoints) that follows the project while the settings are open;
  shortcut bindings are in one tab per group (RB-080-72).
- "Save diagnostics…" with the Save dialog in Settings › Advanced › Diagnostics and on About
  (RB-080-73).
- Image overlay: Add design… (`Ctrl+Shift+Space O N`) copies a PNG/JPEG/WebP into
  `.rbrowser/designs/` in a trusted workspace and shows it for the active breakpoint; a cancelled
  dialog says so; a design already in the project is said to be there; Windows device names get a
  `_`; the panel keeps small previews in `.rbrowser/out/image-overlay/thumbs/`; MCP
  `imageoverlay_designs` (RB-080-74).

### Fixed

- Switching a view's engine on the canvas (RB-090-74): a view switched to Firefox or WebKit on a
  canvas preset without grid cells disappeared - the canvas and the split's sides showed
  Chromium breakpoints only; now the view keeps its board at its place, as it keeps its cell in
  the grid. The status bar no longer warns "keeps its engine: config.local.json overrides ..."
  after a write into `config.local.json`: the notice follows the config as read again, and the
  warning comes only when `config.local.json` shadows a write into `config.json`.
- The portal rewrites a page's origins written as HTML character references (RB-090-75): an
  attribute escaper writes `src` and `srcset` as `http&#x3A;&#x2F;&#x2F;localhost&#x3A;3000...`,
  which reached the device unchanged, so its images stayed on an address the phone cannot reach.
  Every mapped origin is now found in hex (upper or lower case) and decimal references as well,
  read whole or streamed, and written back as the portal's origin in the same form; the log and
  the offer of an unlisted loopback origin see the encoded form too, and a forwarded origin in a
  form the portal does not rewrite (a padded reference such as `&#x003A;`) is logged as
  reaching the device unrewritten.
- A recorded flow runs through in every view (RB-090-76). A navigation after a click of only
  some breakpoints takes the others there with `page.goto` before its URL is expected; a click
  into a submenu that opens only on hover is recorded after a hover on its menu entry (the
  agent reports it at the press, marked `menu` in the sync protocol, inside a navigation, header
  or menu only), and the hover of the pointer resting on the element just clicked is no step; a
  checkbox or radio is checked, never filled, and the fill of an older recording is left out
  before the check or uncheck that follows it. In a flow every exported
  step runs inside `step()`, so `result.json` names the step that failed or ran out of time; one
  action waits at most `automation.flows.actionTimeoutMs` (10 s) and then fails with its
  locator, instead of using up the flow's 30 s.
- Form autofill and the vitals bar after the acceptance of ZIP 84e5481 (RB-090-72): the
  autofill panel fills with the profile or preset chosen from a button beside the choice,
  **Fill with <name>**, and its hint and the tooltips of Fill valid and Fill invalid name the
  profile each uses. The page menu's Checks no longer offer "Web Vitals overlay", which switched
  measuring (`vitals.enabled`) and left the vitals bar empty; measuring stays in ≡ › DevTools
  and the settings. Switching the vitals bar on (`ui.vitals_bar.toggle`) switches measuring on
  too and says so in the status bar.

- Flows and played recordings run in a project in WSL again (RB-090-72): the flow worker died
  on every run when it removed Playwright's temporary folder on the `\\wsl.localhost` share,
  which Node's permission model refused, so nothing played in the views. The worker's temp
  folder and its screenshots are now on a local drive (`flows-tmp\<run>` in rBrowser's local
  folder, `%LOCALAPPDATA%\rBrowser` or the portable `data\local`), the only place it may
  write; the screenshots are copied into `.rbrowser/out/flows/<run>` after the run and the
  folder is removed. A worker that ends before it reports names its exit code and the first
  error line of its output in the views it left.
- Portal: images and other resources a page loads by script, from a JSON answer or from the
  device's cache now lead to the asset-origin offer too (RB-090-72). The portal agent on a
  paired device names each origin its page failed to load from (the origin only, at most 16 per
  page); an unmapped loopback origin is logged once and offered in the status bar as before
  (`portal.offerAssetOrigins`), any other origin is logged with the hint that the portal forwards
  only local `http` origins.
- Portal: images an API answer, a script or a style sheet names by the page's own address now
  load on the device (RB-090-72). The portal rewrites the page's own origin - and every asset
  origin - in every HTML, CSS, JavaScript and JSON answer, also without asset origins and also
  in a streamed answer (a long poll, an event stream, a large bundle), which it rewrites as it
  streams instead of passing it unchanged; a device's cached copy of such an answer is no longer
  revalidated into an unrewritten `304` (images, fonts and media still are). An address that still
  reaches the device unrewritten is logged as such. The asset-origin offer never names one of
  rBrowser's own listeners (MCP, the CDP proxy, the portal).
- *Video: show the recorded videos* (the main menu, the Video split button's menu,
  `Ctrl+Shift+Space C T`, the palette, the page menu's Panels) opens the Videos panel
  (RB-090-71): with the panels folded to their icon rail - `panels.dockState` `rail`, the start
  default - the command only marked the entry in the rail, as a check's open does. Every command
  a person runs to show a panel - Videos, Findings, Recorder, Flows and a plugin's panel
  (`plugins.panels`) - opens the folded dock at it now; a panel opened as a side effect (a
  check's findings, a plugin command's `$open`) still only marks the rail.
- Form autofill: a preset taken from a form is chosen right in the autofill panel (RB-090-71).
  The profile choice was folded under Options; it is always in sight now, grouped into the
  settings' profiles and the presets taken from forms, and a preset just saved is chosen. A
  chosen preset is renamed or deleted there (inline, the delete confirmed), also over MCP
  `autofill_rename` and `autofill_delete` - the plugin's own presets file only. The fill
  shortcuts take a preset that `form-autofill.activeProfile` names.
- A flow run can be followed and stopped (RB-090-71). The Flows panel shows **Stop** while a run
  goes on, every view of the run from its start (waiting, running, passed, failed, skipped), the
  elapsed time and the flows of *Run all* still waiting, and says why *Run* waits; the activity
  bar's flow entry says *Stop*, names the waiting flows, and its close button says it only moves
  the run to the toolbar. *Run all* runs every flow of `.rbrowser/flows/` one after the other -
  several exports of one recording read as one flow looping when the run named no queue. No flow
  runs while a recording takes steps (`flows.recording`): the recorder follows the active view's
  navigations, so the flow's became the recording's own steps.
- Portal images from a dev server the environment does not list (RB-090-71): when a portal page
  names a loopback origin the portal does not forward (`http://localhost:5173`), the status bar
  offers to add it to the shown environment's `assetOrigins` (again after each portal start and
  each change of the shown environment; `portal.offerAssetOrigins`, on) - a button that runs the
  new command `portal.add_asset_origin` (`Ctrl+Shift+Space S O`; without arguments the oldest
  offer), which adds exactly that origin to that one environment's list in the file it comes
  from (`config.local.json` when it sets the list, else `config.json`), nothing else of the file
  changed, and the portal follows at once; only a person's click, MCP cannot. Status messages
  can carry such a button (`StatusMessage.action`).
  several exports of one recording read as one flow looping when the run named no queue; a flow
  that does not start ends the queue with a notice naming why and the flows that did not run. A
  flow run and a recording that takes steps exclude each other (`flows.recording`,
  `recorder.flow_running`), checked and claimed under one gate: the recorder follows the active
  view's navigations, so the flow's became the recording's own steps.
- Ctrl+0, Ctrl+Plus and Ctrl+Minus with a Firefox view leading zoom its page zoom and follow
  `views.pageZoom.sync` as with a Chromium view (RB-090-69): the commands took Chromium views
  only, so the key in a Firefox view's cell failed ("the page zoom works in Chromium views
  only") and the key in Firefox's own window never reached rBrowser. A Firefox view now zooms
  its Chromium stand-in in the grid, and the key its sync agent reports runs the same command;
  Firefox's own window keeps the zoom Firefox gives it (BiDi has no page zoom).
- Acceptance findings of ZIP eb59d46, items 1-4 (RB-090-69): a flow runs on a project in WSL
  (the flow worker is granted its folders also in the `\\?\UNC\...` form Node checks a removal
  against, and every path on Windows has backslashes only); **Open folder** opens the run's
  folder when its `result.json` is missing, and nothing (an error) when the folder is gone too,
  never the desktop; an exported flow or test names the recording it came from, its files the
  export's name; screenshot and video run folders, a recording's default name and the sitemap
  and compare run ids are in local time, or UTC with the new setting `ui.fileNameTime` (stored
  RFC 3339 timestamps stay UTC), and the video list and its retention order the folders by when
  they were made, not by name.

- Canvas mode and the portal (RB-090-69): the canvas board's labels show the label strip's Web
  Vitals with their rating shapes, the external engine's window state and the address (same
  `ui.cellLabel.*` settings, focusable buttons with the details as tooltip; a click marks the
  worst vital or restarts/raises the engine, as on a grid strip). The portal rewrites JSON
  answers of a known length (`application/json`, `+json`) beside CSS and JavaScript while asset origins are mapped,
  and URLs with escaped slashes (`http:\/\/host:port`) in every rewritten body, so images an API
  names reach the device; the "unmapped loopback origin" log covers every body read whole.
- The label strip of every view shows what was built for it but never reached the screen
  (RB-090-27): the Core Web Vitals LCP, CLS and INP with the shape of their rating and a CPU or
  memory warning (all five vitals in words in the tooltip; a click marks the element behind the
  worst), the state of a Firefox or WebKit view's window ("Firefox: engine stopped"; a click
  restarts it, opens it again or brings it to the front), and the view's address while it differs
  from the active view's. New settings in Layout › Grid: `ui.cellLabel.vitals`,
  `ui.cellLabel.engineStatus`, `ui.cellLabel.address` (`differs`, `always`, `off`). The unloaded
  page `cell-label.html` and its components are gone.
- The Outline panel and MCP `outline_get` show a finding at the severity `checks.rules` gives
  its rule, as the Findings panel does, and nothing of a rule turned off: a plugin reads the
  levels of its own rules through `settings.get("checks.rules")`, read only (RB-091-06).
- A Ctrl+wheel page zoom of a view on another site, landing on the very level rBrowser set on
  another view in the same reading, is the user's own zoom again: it reaches
  `views.pageZoom.sync` and the wheel guard, where it was taken for a view that moved along
  with the set one. Only views of the same host follow a set, as Chromium keeps the zoom per
  host (RB-090-28).
- An environment or layout preset whose name is longer than 256 characters can be chosen from
  the toolbar's Environment and Layout preset pulldowns: their rows carry their place in the
  menu, not the name, which the native menu refused as a row id with the whole menu
  (RB-090-57).
- A flow saved from the Recorder panel is never written through a symbolic link or junction:
  a repository that ships `.rbrowser/`, `.rbrowser/flows/` or a flow file as a link out of the
  project gets the save refused; the recordings and exports folders refuse a linked `.rbrowser/`
  too (RB-090-66).
- The portal: a paired phone or tablet gets the page's styles and scripts also when the page
  loads them from a second local origin, such as a Vite dev server beside a PHP or Laravel app
  (`http://localhost:5173`). List that origin in the environment's new `assetOrigins`
  (Settings › Project › Environments); the portal forwards it on a port of its own and points
  the page's, style sheets' and scripts' URLs at it. A page that names a local origin the portal
  does not forward is noted once in the log (RB-090-65).
- Settings › Breakpoints deletes a breakpoint again: the layout cells, hidden views, split and
  canvas fields, a preset's focus and `envCompare.breakpoints` that named it go with it, in
  `config.json` and `config.local.json` alike, so the check no longer refuses the save
  (`/layout/cells/13/breakpoint: No breakpoint has this id.`); MCP `breakpoint_remove` does the
  same and counts the references. Every line of a settings list carries a trash icon, and the
  confirmation in place says what goes along ("Also removed from 2 layout cells."). "Add" in a
  settings list starts an entry its schema accepts - a new device preset no longer fails with
  `/0/kind: Not one of the allowed values.` - and a device preset, breakpoint, environment or
  bookmark starts with useful values (RB-090-61).
- Settings › Plugins: **Commands: n** opens the shortcut editor filtered to the plugin's commands also for a plugin id with a hyphen (`color-picker` → `plugin.color_picker.`); before, the list stayed empty.

- The page's console no longer shows "Blocked aria-hidden on an element because its descendant
  retained focus" when the colour picker's pipette or Peek's locked tooltip takes the focus:
  `aria-hidden` sits on each plugin layer instead of their shared `<rbrowser-layers>` host, and
  a layer that takes focus is added as interactive, with a role and a name (`rbPlugin.layers.add`,
  web-common's `addLayer`; a layer an older plugin build added unmarked is hidden itself when the
  container's attribute goes; RB-090-64).
- Video recordings show the page at its full size: a view shown below 100 % (a zoomed-out canvas
  or a fitted grid cell) was recorded small in the top-left corner on white, and every frame was
  stretched to the breakpoint's height instead of the height the view shows. Recordings, MP4
  exports and WebM conversions are written as a standard stream - limited range, BT.709, square
  pixels - so Windows plays them instead of failing with `PIPELINE_ERROR_DECODE`; the posters
  follow (RB-090-52, RB-110-12).
- One plugin paused after repeated failures no longer pauses every plugin, and a long Lighthouse
  run no longer starves the other plugins: all plugins shared one failure count, one suspension
  and one call gate, so the run's call held the slot that the rulers', overlays' and other
  plugins' page requests waited for until they were given up. Each plugin now has its own; a
  paused plugin's row in Settings > Plugins has Resume, and `plugins.resume` (`Ctrl+Shift+Space
  X U`) resumes every paused plugin (RB-090-59).
- Review follow-ups of the 0.9 waves (RB-090-54): a Firefox path with blanks at its edges
  (`engines.firefox.path`) is taken trimmed by the start as by the About page; a bookmark with a
  very long id no longer breaks the toolbar's bookmarks menu; Peek's rendered font, role and name
  on a scrolled page are those of the element under the pointer (`DOM.getNodeForLocation` takes
  document px); the element pick of `capture.element` and of "Inspect element" passes over
  rBrowser's own layers (rulers, guides, plugin hosts); a toolbar pulldown opened again before its
  menu's close arrived keeps the choice made in the new menu.
- The Videos panel plays a recording it cannot stream: a WebM the player fails to stream from
  `rbrowser://video` is loaded whole once and played from memory (at most 512 MiB per recording,
  released when another recording is picked), and the status bar says so. A video that still
  does not play says Chromium's reason under it (`DEMUXER_ERROR_...`, `PIPELINE_ERROR_...`), and
  the log keeps it with the player's error code and every request the route answered (status,
  range asked and answered, length). Posters fill their box: poster and video share one frame
  of the picture's shape instead of a small picture in a large empty box (RB-110-12).

- Zooming the canvas (Ctrl+wheel, the status bar's zoom menu, the zoom keys) and panning it
  with the middle button move each view's frame, label and footer together with the view: the
  frames no longer jump back during the gesture and catch up only when it ends (RB-090-55).
- "Inspect element" from the page's context menu and MCP's `devtools_inspect_at` select the
  page's element under the pointer: rBrowser's own overlays there - the rulers and guides, the
  design overlay's handle, the peek tip, every plugin host - are passed over, and the point is
  the one clicked on a scrolled page too (RB-090-51).
- Recording several views no longer slows them down: each ffmpeg encoder runs below rBrowser's
  priority (Windows below normal, `nice` elsewhere) with its share of threads - all encoders
  together at most half the logical cores - and a frame the encoder cannot take in time is left
  out instead of holding the page; `record_video stop` names the frames left out per view
  (`dropped`) (RB-090-52).

- The breakpoint bar under a view names the class of wide views too - `2xl`, `3xl` and every
  name of the project's own: the footer's analysis now reads the project's names like Analyze
  Breakpoints does, and both read UnoCSS configs (`uno.config.*`, `unocss.config.*`:
  `theme.breakpoint` and `theme.breakpoints`; a new default of `analyzer.staticGlobs`). A
  project that moves the names off Tailwind's widths gets its own names, no longer Tailwind's
  (RB-090-50).
- The Videos panel plays the recordings: new recordings are WebM (VP9) by default
  (`capture.video.format`), since rBrowser's Chromium has no H.264 decoder and could not play
  the MP4 it recorded; an MP4 says so under its player and in the status bar, with the way to
  the setting. Each video gets a poster - its first frame, written by ffmpeg after the
  recording - shown in the list and on the player, and served only as rBrowser wrote it. Every
  refusal of the video route is logged with its reason (RB-110-12).
- Settings › About and MCP's `app_info` name the installed Firefox - its version and path, or
  the path set in Settings › Engines - instead of always reading "not found" (RB-090-49).
- A canvas view zoomed past 100 % (the status bar's zoom menu, Ctrl+wheel, `canvas_zoom`) no
  longer shows only the top-left part of its page until the canvas is panned: Chromium no
  longer shrinks the view to its unscaled size (RB-090-42).
- The colour picker's loupe no longer shows another place at canvas zoom 75 % (and at 25 % in a
  view cut on the left): a view drawn at 0.75 of its DPR was read by its window image's size,
  and a view drawn smaller and cut by the window's left edge was read from the image's corner -
  the markers that agree with more boxes now win, and two readings as good as each other are
  refused in the status bar rather than guessed. The loupe opens left of (or above) the cursor
  where the view is cut off on its right (or below), not only at the viewport's edge (RB-090-40).
- Plugin panels are no longer held to 64 KiB per message: a call, its answer and an event may
  take up to the new setting `plugins.panelMessageMaxBytes` (Settings > Plugins, 4 MiB by
  default, 64 KiB to 16 MiB), and a message over it names the setting. The SEO panel's social
  previews show share images again (one of 170 KB was "too large to show here"); the Outline,
  Fonts and PWA panels ask for answers as large as the limit, the Storage panel lists 1 MiB of
  rows and shows a value whole. The scripts plugins run in the pages keep their 64 KiB
  (RB-090-44).
- The breakpoint analyser's wizard draws its grid preview as the grid that is written: the
  views at one common scale where the grid puts them (four columns, then the next row; added
  views as one new row), each label centred with its size below, fitted to the window's width.
  Without a project the analysis still runs, but "Write" stays off with its reason and the
  wizard offers the recent projects to open; "Mode" and the pages field are Nuxt UI controls
  (RB-090-43).
- The tab rows of the plugin panels (PWA, Outline, Network with its mock action and breakpoint
  stage, Fonts, Storage, Coverage, Console) no longer show a vertical scroll bar with arrows on
  Windows: they stay in one row that scrolls sideways, with a back and a forward button only
  while tabs are cut off, and the chosen tab scrolled into sight - the shared `PanelTabs` of
  `panel-ui`, also in the SDK's panel runtime (RB-090-48).
- The Lighthouse panel lays a breakpoint's four score tiles out four in a row, two by two or
  one below the other, never three and one (RB-060-07).
- The colour picker reads a view that is only partly in the window - the grid scrolled, the
  canvas panned or zoomed so the view sticks out: the pipette also shows its calibration markers
  beside the pointer, where the view is seen, and maps a view whose top or left the window's edge
  cuts off; a grid view scrolled above the window was read too low. A view it still cannot map is
  named in the status bar, never read at a wrong place (RB-090-40).
- The colour picker's loupe no longer shows another place than the one under the cursor on the
  canvas with page zoom: a view's window image of the viewport's size could hold the page drawn
  at the canvas's scale in its corner, and was read at the page's DPR (white beyond the page);
  the markers now decide, and a view they cannot map is named in the status bar (RB-090-31).
- On Windows a `file:` URL typed with backslashes (`file://C:\site\index.html`) is read as the
  browser reads it: `rbrowser init --url` and the breakpoint analyzer no longer fail with "The
  network path was not found", and MCP `navigate` no longer refuses such a project file as
  outside the project; a `..` step written with backslashes is refused everywhere (RB-090-30).
- On Windows a CSS change swaps the stylesheet again instead of reloading the page: the folder
  Windows reports as modified beside the file is no change of its own (RB-090-30).
- Settings choices show names instead of ids, core and plugin keys (e.g. the SEO preview
  platforms); the id only with `settings.showKeys`; the settings lint requires a name for every
  option (RB-090-16).
- Overlays' stacking and tab order no longer fail with "larger than the plugin may handle": one
  byte budget for every answer, the panel says "N of M shown" (RB-090-17).
- The status bar's canvas zoom presets open as a native menu above the bar, so they stay on the
  screen in fullscreen (RB-090-18).
- The colour picker calibrates reliably: a window image of exactly viewport x pixel ratio is read
  directly, a scaled one through four framed markers with a scale check; large views no longer
  fail, and the status bar says why a view cannot be read (RB-090-19).
- Security review findings: MCP `cdp_endpoint` answers the CDP token only while
  `mcp.allowEvaluate` is on (`evaluate_disabled` otherwise); MCP can no longer switch Chrome
  extensions or the Web Store (`engines.chromium.extensions.enabled`, `allowWebStore` protected);
  the MCP instance folder and the plugin compile cache are the user's alone, on Windows with an
  owner-only DACL; the trust dialog highlights the `sidecar` grant, and a plugin's Lighthouse
  runs stay on the sites the views show unless the protected `plugins.lighthouseAnyUrl` is on
  (RB-090-21).
- Accessibility: the light theme's state colours, muted text and input ring reach WCAG AA in the
  shell and every panel, a solid button keeps AA on hover, a pressed toggle has a ring; the active
  view's label strip carries a sign and stands 3:1 off; risky trust grants say "Risky" and why; the
  check progress, the which-key bar, the settings search and the panel list speak only on a
  change; the OS's reduced motion stills animations, spinners included (RB-090-22).
- Keyboard and screen readers: toolbar selects used as menus choose nothing while the arrows
  browse; the focus returns to its opener or a neighbour when a bar, an inline question, the
  shortcut recorder, the findings run or a self-disabling control (strip arrows, list moves)
  lets go of it; Tab moves on between the toolbar, the active view and the status bar;
  indicators keep their visible label as their name; the canvas label and its engine badge are
  separate buttons (Space activates, the badge is larger and says why an engine cannot run);
  settings are described by their description, each Record button names its command; desynced
  views show in the toolbar again ("Desynced (n)", click resyncs) (RB-090-23).
- The panel dock's icon rail shows the plugin panels once the plugins have loaded; a dock opened
  at start showed only Findings (RB-080-97).
- A view whose engine a check plugin cannot serve is "skipped (engine)" with a sentence and listed
  in the run's `skipped` (`findings.json`, `checks_run`), never failed, "Nothing found" or clean;
  failure sentences get their parameters, no raw `{service}`; a cancelled run says Cancelled
  (RB-080-98).
- Storage, Console, Network, Coverage and Error report no longer drop a tab or filter change made
  while they read; SEO shows only the latest request's previews (RB-080-84, RB-080-85).
- The colour picker no longer flickers on Windows: the window image is located by two markers that
  tolerate the display's colour profile, never by a clip of its own; setting "Pipette reads"
  (`color-picker.capture`); no pipette in Firefox and WebKit views (RB-080-77).
- Crash of the browser process on exit: every UI-thread holder of a CEF object is released before
  `CefShutdown`, no CDP re-attach once the window closes (RB-080-91).
- Plugin panels wait for a plugin busy with a check run instead of failing after 10 s, say a
  failure as a sentence with its reason and log it; counts use plural forms (RB-080-90).
- Simulators: "Apply to every view" acts at once in both directions (RB-080-75).
- Settings and About tabs stay in one row that scrolls in a narrow window, never overlapping
  (RB-080-81).
- Plugin command titles and plugin setting labels show their text instead of the raw key, in
  every window (RB-080-82).
- Findings' Help did nothing; the findings summary reads right before the first run and in the
  plural; a failing check plugin names its reason and is logged (RB-080-92, RB-080-89).
- Colour picker: the views no longer flicker when the loupe's tile is taken; the view is read from
  its window image without a clip, checked by a marker once per pick, with a clip only as fallback
  or above 8 megapixels (RB-080-64).
- Colour picker: "Pick from the page" shows at once that it is on ("Picking… Esc to stop"); a second
  click or Esc stops it in every view, the shortcut looks the same, and the status bar names start,
  the colour taken and the end; the panel's "Measure in every view" works (it used a form, which
  never submits in the sandboxed panel) (RB-080-64).
- Screenshot editor: an image over 47 KiB with saved annotations failed to open with
  `plugins.error.too_large`; host errors now read as sentences; the annotation font was blocked by
  CORS in the sandboxed panel and is now embedded, and the font file no longer ships twice
  (RB-080-63).
- The rulers stop flooding every view's console with busy/timed_out: one request in flight per view,
  none while hidden, and the host gives up requests the page gave up (RB-080-66).
- The rulers panel's column overlay and baseline grid act on every change; they never applied
  because Apply was a form the sandbox blocks (RB-080-66).
- The Vue DevTools panel loads in its sandboxed frame (RB-080-66).
- Lighthouse runs no longer leave browser processes or profiles behind: each run closes its browser,
  kills the whole process tree of one that does not close, and removes its own profile folder; the
  sidecar's end cancels every run within its stop budget (RB-080-67).
- On Windows the sidecar starts inside a job object of its own, so everything it starts ends with it
  on stop, crash, restart or app exit; if a helper process could not be tied to the app this way,
  the app shows a warning once; the memory warning names the most frequent processes, and the
  process count includes the sidecar's orphans (RB-080-67).
- The New size picker is complete on its first open, lists every device and starts with Custom
  size…; typed sizes show as New view: W x H; the palette keeps the selection on its entry while its
  list updates and shows at most 50 rows of a long list, naming how many more match (RB-080-68).
- A list in the settings no longer saves an empty entry; a config value outside a setting's allowed
  values is reported and can be removed (RB-080-72).
- The view footer is no longer cut off in narrow views: it breaks between two fields into a second
  line and the view gives it 36 px; a grid is built for its own widths in one pass, and panning,
  scrolling or zooming refits only footers whose width changed; the text is centred, and what two
  lines cannot hold is in the tooltip (RB-080-53).
- Canvas: changing a view's size, by any path (quick pick, Size…, Ctrl+Shift+1…9, device,
  orientation, MCP), places and redraws the view at once, not only after a drag (RB-080-54).
- Canvas: a view that reaches past the window's edge shows the part of its page inside; a view cut
  at the top keeps its label at the top edge; a view panned into the window runs at once; only a
  view wholly outside is hidden (RB-080-55).
- Colour picker: the views no longer flicker while the pipette is on; every clipped screenshot made
  the view draw another picture for a frame, and the pipette took one per pointer move, plus one in
  every view at start and on each synced scroll; now it takes one tile where the pointer rests, only
  in the view the pointer is in (at the start, the active view's centre), and draws the loupe from
  it (RB-080-56).
- Lighthouse finds an installed Google Chrome or Microsoft Edge in the machine-wide program folders
  again; a browser chosen in Settings that does not exist is reported with its own message instead
  of being replaced silently (RB-080-58).
- A failed plugin command shows the plugin's own message and the command's title in the status bar,
  the log and MCP answers instead of a raw key such as `plugin.lighthouse.errors.no_browser`; a
  plugin can no longer show another plugin's text or a core text (RB-080-58).
- Rulers and guides, the screenshot editor and the simulators open in the plugin panels window's
  content area like every other panel, with the list mark following, instead of as a box over the
  panel chosen before; the window opens one of them only when it is chosen, and switching panels no
  longer loses the screenshot editor's unsaved annotations (RB-080-59).
- A plugin panel no longer shows raw translation keys: a panel opened as a dialog never reached the
  panel bridge; the panels now wait for their texts, ask again, and after ten tries show "The
  panel's texts did not arrive." with a Retry button (RB-080-59).
- In a narrow window the toolbar's address bar shrank to nothing and controls were cut off at the
  right edge; it keeps at least 240 px now, wide lists shrink to icons, the rest moves into a "More"
  menu (RB-080-52).
- The page's context menu runs its rBrowser entries again (Inspect, DevTools, Screenshot, Video,
  Checks, Panels, Pop out, Remove view); the menu's table was lost between opening and choosing; a
  menu entry that can no longer run says so in the status bar (RB-080-45).
- Enter in "New view size" adds the view and a refused input says why; the view menu and the add
  menu open in front of the views; a right-click on any label strip opens the view menu in every
  layout; `view_add_size` answers the existing breakpoint's size and refuses another size under an
  existing name (RB-080-46).
- rBrowser's own UI never zooms: Ctrl+wheel no longer enlarges the toolbar, and a zoom left over is
  reset at the start (RB-080-50).
- The view footer draws its breakpoint colour in every layout, on the canvas and in pop-out windows;
  a colour set in Settings > Breakpoints applies when saved; its text is black or white at a
  contrast of 4.5:1 or better (RB-080-47).
- A bookmark path that names another host (`//host`, `/\host`) is a config problem and never opens
  (RB-080-48).
- Settings > Plugins listed no plugin; "Installed plugins" now lists every bundled, installed and
  project plugin with version, source, signature and state (RB-080-51).
- A restart of a Firefox or WebKit view requested while the view was still starting was dropped and
  left the view "restarting"; it now runs after the start under way (RB-080-40).
- Embedded Firefox and WebKit windows are bound by their own process, not by their size (RB-080-40).
- Removing a Web Store extension no longer deletes another rBrowser window's running install of it;
  the removal is refused while that install runs (RB-080-30).
- External Firefox and WebKit windows (RB-080-38): one per view, kept across mode switches, presets
  and config reloads; a start that fails closes the window it opened (Firefox 160 refused a device
  pixel ratio of 0, so breakpoints without a DPR never started and every rebuild left a blank
  window); a window closed by hand stays closed until Open again (a Firefox quitting with its last window counts as closed, not crashed, and a rebuilt engine leaves it closed); a restart only after a crash.
- An engine switched from the badge, the chord or MCP keeps the layout mode in effect; the
  canvas board no longer falls back to the grid after the config is read again (RB-080-33).

### Changed

- Dependencies to their newest compatible releases before 0.9.0-beta.1: CEF 156.0.30
  (Chromium 156.0.8078.96, the archive pins moved with it), wasmtime 50.0.2, rmcp 3.11.0, clap
  4.6.8, zip 9.6.1, rand 0.10.4, uuid 1.29.3, Playwright 1.65.1 (the WebKit revision stays 2388),
  puppeteer-core 25.18.1, vue-i18n 11.4.22, the Lucide icons 1.2.156. ureq stays at 3.5.1 (its
  10 MiB JSON cap breaks the CEF index download), axe-core at 4.14.0 (its rule table is pinned
  with the plugin).
- The page menu has no Print entry and sets its entries apart in groups: DevTools; capture,
  runs, checks, panels and emulation; the view's analysis, forced state, sync and tools; the
  bars (RB-090-72). DevTools is one check entry like the bars: checked while the clicked view's
  DevTools are open, choosing it opens or closes them.

- Scroll sync does less work per view (RB-090-33): in the default scroll mode `anchor` the sync
  agent no longer listens for the wheel (the mode in effect, `sync.scrollMode` or the one
  `sync_set` set, reaches the agents of every engine), and a view that falls behind during fast scrolling
  replays only the last of the scroll positions waiting in its queue; a click or input between
  them keeps its place.
- The UI thread and the shell's pages do less per change (RB-090-36): the settings, the
  palette's flows and the trust check at the start are read off CEF's UI thread; a state like
  the one sent last is not pushed again, and new Web Vitals or resources alone go out as their
  own `metrics` event; `settingsChanged` carries a user setting's new value, so no page asks
  for the settings again; and the window theme is set again only when `ui.theme` changed.
- The recorder records scrolling by default: `automation.recorder.recordScroll` is on, so a
  recording plays back the way the page was scrolled (RB-090-66).
- The command palette is Nuxt UI's command palette, in the look of the other Nuxt UI windows:
  the row under the pointer becomes the selection, and a step has a back button beside its
  title; keys, steps, shortcuts and the screen-reader announcements work as before (RB-090-46).

- Videos are always recorded as WebM, which plays in the Videos panel and every browser on
  Windows, Linux and macOS; the setting `capture.video.format` is gone (an old config that sets
  it still loads, with a warning). To share a recording, the Videos panel exports it as MP4 in
  the background (Windows `h264_mf`, macOS `h264_videotoolbox`; Linux has no LGPL H.264
  encoder, and the panel says so), with the progress in the status bar; an old MP4 recording is
  converted to WebM there and then plays. An ffmpeg without VP9 records VP8 (RB-090-52).

- rBrowser's overlays on a page - rulers and guides, design overlay, Peek, the overlays,
  simulators, font marks and colour picker - live in one element, `<rbrowser-layers>` at the
  end of `<html>`, so DevTools' Elements panel shows one collapsed line instead of one per
  plugin; the docked DevTools' Console lists only the page's contexts, not rBrowser's own
  isolated worlds. Plugin page scripts add their overlays with `rbPlugin.layers.add(host)`
  (RB-090-51).
- The breakpoint analyser's wizard ticks its proposed views and "replace" with Nuxt UI
  checkboxes and folds the stylesheets and boundaries found into a collapsible, like the rest of
  the shell (RB-080-88).
- The findings panel, the bars over the toolbar (bookmarks, sessions, sitemap, save preset, new
  size, free size, extension prompt), the Browsersync and framework DevTools notices, the split
  divider and the palette's step are Nuxt UI like the rest of the shell: the findings grouped by
  rule in folding sections, "Share with the team" a switch in the bookmark bar as in the
  save-preset bar, and clearing a session chooses it with buttons as assigning does (RB-080-88).
- Roadmap jobs that wait only for their acceptance (`Partial (Abnahme offen ...)`) live in
  `docs/roadmap/jobs/acceptance/`, so `docs/roadmap/jobs/` shows only the work:
  `scripts/archive-jobs.sh` moves a job by its status into `acceptance/`, into `archive/` and
  back, the Job Inventory counts the three places, and a job closed without its acceptance
  archives like any other (RB-090-47).
- The settings window, About and the portal page look like every other Nuxt UI surface, in light
  and dark: numbers with their unit beside the field (plus and minus on small counts), searchable
  proposals with free entry, selects, checkbox and radio groups, a category navigation, warnings
  with their actions and virtualised tables; values still save on Enter or on leaving the field.
  The tab rows of the pages window no longer show a vertical scroll bar on Windows (RB-080-88).
- Dependencies: Nuxt UI 4.12.2, Lucide icons 1.2.155, @vue/test-utils 2.6.0, zip 9.6.0, rand
  0.10.4 and other compatible updates; the plugin panels rebuilt.
- Every toolbar pulldown opens as a native menu below its button, like the theme menu: the
  environment, the layout preset and the bookmarks are buttons with their value and a chevron,
  "More" an icon; the menus follow light and dark, keep inside the window and check the value in
  effect, and Enter, Space, an arrow key or F4 opens them. The bookmarks show at most 100 rows;
  a longer list ends in a row that opens the bookmark search. The canvas zoom slider sends the
  level an arrow key steps to, no longer the one before (RB-090-45).
- The toolbar's theme menu (System/Light/Dark) opens as a native menu below its button, painted
  in rBrowser's theme and kept inside the window, the theme in effect checked - no longer
  Chromium's select list with other colours under the mouse; the shell asks for such menus with
  the new `nativeMenu` message (RB-090-41).
- rBrowser's theme is chosen from a sun-and-moon toolbar menu (System/Light/Dark) with a mark for
  the theme in effect; the plugin panels' toolbar switch is off by default (RB-090-10).
- The strip under every view is called "Breakpoint bar" everywhere (RB-090-11).
- The findings panel shows a check run as one row per plugin, views on demand, running plugins on
  top, and one summary line after the run (RB-080-98).
- The About page's source, handbook and changelog links and the handbook command (`app.open_link`
  without a URL) point at the public repository `github.com/degoya/rBrowser` and its wiki; the
  Playwright example moved from `docs/examples/playwright/` to `examples/playwright/` (RB-090-12).
- The plugin panels start docked right as an icon rail, and Close and fold return to it; hiding
  them is a deliberate choice (`panels.dockAtStart`, `panels.closeTo`; `panels.dockState`
  replaces `panels.dockCollapsed`, migrated); the Dock menu and the side bar are Nuxt UI with an
  icon per place (RB-080-93, RB-080-83).
- The view footer shows the range the page's step holds for and the page's class by default
  (name and size stand in the label strip above); a mobile-first page's upper end is the next
  min-width minus one; the footer never takes more than half of its view (RB-080-78).
- Lighthouse and Fonts panels on Nuxt UI; Fonts with a status line, tabs Fonts | Sizes and
  accordion sections (RB-080-83).
- A plugin's status notices no longer name the plugin themselves - the core does (RB-080-76).
- `engines.external.embed` is off by default: Firefox and WebKit views are windows of their own
  again until the embedding scales the page to the cell and hides WebKit's shell (RB-080-40,
  after the owner's test of RB-080-39).
- The settings schema allows `"x-options": "dynamic"` for choices the core finds at runtime
  (RB-080-32).
- `engines.external.autoRestart` is `engines.external.restartOnCrash` (the old key still reads)
  (RB-080-38).
- `view.external.arrange` (`Ctrl+Shift+Space V D`) and `view.pop_out` show an embedded Firefox or
  WebKit view as a window of its own, `view.pop_in` puts it back into its cell; the sidecar's
  `webkit.launch` answers the browser's `pid` (RB-080-39).
- Dependencies (RB-080-34): birpc 4.2.0 in the Vue DevTools panel (wire format unchanged, panel
  rebuilt); vite 8.3.8, @vitejs/plugin-vue 6.0.11, vue-i18n 11.4.19, lucide 1.2.154 in the shell; cc
  1.11.2, libloading 0.9.1, mio 1.2.5, target-lexicon 0.13.6 (ureq stays 3.5.1). TypeScript 7 in
  `ui/` waits for a vue-tsc that runs on the native compiler (vue-tsc 3.3.14 needs
  `typescript/lib/tsc`).
- The commands' `ref` and the MCP tools' `id` of the Web Store are read by one function (RB-080-30).
- The probe's framework report is validated once, in the sync agent, by agent and panel alike; the
  React DevTools backend's reconnect after closing the panel is covered by a test (RB-080-31).
- The settings' list of Web Store extensions follows the core's `webStoreExtensions` event after
  every install, update or removal; `extensions.remove` is refused while the Web Store is off
  (RB-080-30).
- `capture.element` and MCP `screenshot` take `at: {x, y}` (CSS px of the page); `devtools.inspect_at`
  takes x/y in CSS px, `dip: true` for widget DIP; `capture.video.toggle` takes `target`;
  plugin manifests may list `context_menu` commands (RB-080-43).
- One WebKit process per view (`webkit.view.create` answers the pid; a WebKit that ends takes one
  view); the embedded WebKit shell loses its menu bar and toolbar; screenshots of embedded views
  stay breakpoint-sized (RB-080-40).
- MCP `views_list` names `range { from, to }` per view; each view gives its footer 18 px of height
  (RB-080-44).
- `emulation.set` with `fields.colorScheme: null` drops the live colour scheme override (RB-080-49).
- The view footer always names `min-width` and `max-width`: the ends of the breakpoint's range until
  the page is read; queries are separated by ` | ` (RB-080-47).
- MCP `plugins_list` answers `loading` and, per plugin, `description`, `source`, `signature`,
  `state`, `error`, `commands`, `settings` and `verified`; a disabled plugin is listed, a refused
  package only by its file, state and error code (RB-080-51).
- The panels header loses Smaller/Larger; Close is an icon button (RB-080-61).
- The Fonts, Lighthouse, Coverage, PWA, SEO and Error report panels share one layout with icons and
  follow the app's theme (`ui.theme`), not only the system's: header icon buttons with tooltips, a
  status line with an icon, kit tables, badges, empty and loading states; Fonts drops the "unknown"
  column and shows "not scaled" as a badge; Lighthouse shows a card per breakpoint with score tiles
  and the change as an arrow (RB-080-62).
- Screenshot editor: tools as icons with the chosen tool's options beside them, image actions
  grouped, shortcuts in every tooltip; crop with handles, size, and Apply crop (Enter) / Cancel
  (Esc) at the frame, undoable; applying the unchanged crop again changes nothing; Escape in the
  crop tool drops a changed frame, else discards unsaved changes, else leaves the tool; a chosen
  zoom stays after a crop (RB-080-63).
- Plugin panels get shared files from `plugins/web-common/panel/` listed in their `web-common.txt`
  when the plugin is packed, instead of checked-in copies (RB-080-65).
- What plugins' page scripts log goes to the rBrowser log and the shared console (source "rBrowser:
  <plugin>", setting `console.showRbrowserMessages`), never into the page's console; a page request
  timeout reads `plugins.error.no_answer`; a plugin's page script that fails reports its stack to
  the rBrowser log, and to the console even with the setting off; a page request given up after 30 s
  counts as the plugin's failure, so a hung plugin is suspended (RB-080-66).
- Findings open as the first entry of the plugin panels window, floating or docked, instead of a
  window of their own (RB-080-69).
- Image overlay panel: an empty state explains designs; designs are chosen per breakpoint by name,
  size and preview, with missing ones marked (RB-080-74).
- Plugins post status-bar notices only with the capability `status`, shown as "{plugin}: {text}", at
  most one a second; a view's window image is a permission of its own (`cdp.window_image`), and
  screenshots are limited to ten a second per plugin (RB-080-64).

## [0.7.0] - 2026-11-13

### Added

- Save layout as preset (RB-080-04): `layout.save_preset { id, file? }` (`Leader L N`, the
  toolbar's save button beside the layout modes, the command palette; MCP
  `layout_save_preset`) writes the layout shown now as `layouts.presets.<id>` and selects it -
  the canvas with every view's place (not the camera), the split's ratio and sides, the strip's
  height, the grid in effect or focus mode's breakpoint. Without a name a bar over the toolbar
  asks for one (prefilled with the selected preset, else the mode) with "Share with the team
  (config.json)"; Enter saves, Escape closes. It goes to `config.local.json` unless the call or
  `layoutModes.savePresetTo` (Layout > Further modes, `local`) says `project`; a preset of that
  name in that file is replaced. Names are 1 to 40 letters, digits, spaces, `.`, `-` or `_`.
- The active view's label strip is marked in the accent colour in the grid, focus, horizontal and
  split layouts, as the canvas board marks its label; `ui.highlightActiveView` (Layout › Grid,
  on) switches it off (RB-080-04).
- `sync.activateOnFocus` (Sync › Leader & submit, off by default): on, a click into a view
  makes it the active view, not only a click on its label (RB-020-02).
- The log says when the window's UI thread kept work waiting for 500 ms or more, and while it
  does not answer for 10 s, so a frozen window shows up in `rbrowser diagnostics`; it also says
  when a focused view took the lead and when a drag on the canvas board ended or failed.
- Canvas drag (RB-080-04): in the canvas layout a board behind the views (a browser view of its
  own showing `canvas-board.html`) draws the label strips and takes the drags - drag a view by
  its label (it follows live, snaps to `snap` and the neighbours' edges on release and is written
  into the preset; never past the canvas bounds), drag the empty canvas to pan it, Escape
  cancels; with `layoutModes.spacePan` (Layout > Further modes, on) Space held pans from a label too.
  A label is focusable: the arrow keys move its view by the snap grid, Enter activates it.
- Project bookmarks (RB-070-06): `bookmarks[]` in `.rbrowser/config.json` for the team and in
  `config.local.json` for one's own (merged by `id`), each with a `label`, exactly one of `path`
  (put after the current environment's URL, so it follows an environment switch) or `url`
  (absolute http(s), unchanged), an optional `group` and an optional direct `shortcut` (held to
  the reserved list and every other binding, a conflict is reported with its reason). A bookmark
  with both or neither target, a relative path or a non-http(s) URL is refused by the check and
  dropped with a warning on load; the rest of the config stays in force. The toolbar lists them
  grouped; `Leader P B` (`bookmarks.open`) opens a quick pick with search, `Leader P K`
  (`bookmarks.add`) "Bookmark this page" with a path proposed when the page lies under the
  environment, into either file; the palette lists "Open bookmark: <label>"; Settings > Project
  > Bookmarks edits, reorders and deletes. Opening navigates every coupled view, or only the
  active view while its navigation is decoupled. MCP: `navigate` and `config_set`;
  `rb_config::bookmarks::resolve` gives other features the URLs.
- Scroll sync (RB-020-04): a scroll in the active view brings the same content to the top of
  every other view, whatever its width - the agent sends, once per animation frame and
  scroller, an anchor element at the top edge and its relative offset, the follower puts that
  point on its edge (exactly at the top and bottom, the same share of the range without the
  anchor, instant against smooth scrolling, without an echo). Inner scroll containers
  (sidebars, modals) and horizontal scrolling (tables) included; the end position goes once
  more after `sync.scrollDebounceMs`. `sync.scrollMode: "wheel"` replays real wheel events
  instead, for pages that take over scrolling. `Leader S C` switches scroll sync
  (`sync.toggle_scroll`), MCP `scroll` scrolls views to a top, bottom, element or percentage and
  `sync_set { scrollMode }` switches the mode. Fixture `tests/fixtures/site/scroll.html`.
- Recorder export (RB-050-02, K12): the first-party plugin `recorder-export` turns a recording
  into an rBrowser flow (`<name>.ts`, `page`, `view` and `expect` injected, no imports) and a
  Playwright test (`<name>.spec.ts`) with a `playwright.config.ts` that makes every breakpoint a
  project (viewport, device scale factor, mobile, touch, user agent, locale, colour scheme,
  reduced motion, browser); locators from the best selector candidate or
  `automation.recorderExport.locatorPreference` (`getByTestId`, `getByRole` with its name,
  `getByText`, CSS), `onlyIn` steps guarded by the project's name or the view's breakpoint,
  secrets as `process.env.RB_SECRET_<NAME>`, a stable comment per step. Each export is a new
  folder of `automation.recorderExport.dir` (default `.rbrowser/out/exports/<name>/`), never
  overwritten; `recorder.export_flow` (`Ctrl+Shift+Space F W`), `recorder.export_spec` (`F E`)
  and MCP `recorder_export` (registered by RB-030-01). `tests/e2e/recorder/run.sh` type-checks
  the export and runs it with `npx playwright test` against the fixture site.
- Recorder model (RB-050-01): while a recording runs, the active view's synced events become a
  cleaned, versioned recording (`schema/recording.v1.json`) in
  `.rbrowser/out/recordings/<name>.json` - `navigate`, `click`, `dblclick`, `fill` (typing
  coalesced), `press`, `select`, `check`/`uncheck`, `hover` (only a menu trigger before a click
  into it), `scroll` (with `automation.recorder.recordScroll`), `waitForUrl` after a click or
  Enter, with selector candidates in rank order, the source breakpoint and `onlyIn` from the
  replays (a click that failed in a view runs only where it worked). Password fields and
  `automation.recorder.secretFields` are recorded as `{{secret:<name>}}`, never in clear; a file
  is never overwritten and never written into a trust-protected folder. `recorder.start`
  (`Ctrl+Shift+Space F S`), `recorder.stop` (`F T`), `recorder.pause` (`F P`), a recording
  indicator in the toolbar, MCP `recorder_start`/`recorder_stop` (registered by RB-030-01) and
  the stable `Exporter` boundary for RB-050-02.
- Form sync (RB-020-05): a choice in a select (also `multiple`), radio button, checkbox, slider,
  colour, date or text field, and a contenteditable's text, reaches the other views as the same
  state, set by the sync agent so React and Vue handlers see `change` there too; a checkbox,
  clicked directly or through its label, toggles exactly once in every view. Clicks on controls
  that open a native picker (select, file, colour, date) open it only in the active view; file
  inputs are never synced; password values are synced but never logged and shown masked to MCP
  and the recorder. Optional focus sync (`sync.focus`, off by default, `Ctrl+Shift+Space S F`),
  form sync switchable with `sync.forms`; "Do not sync the next click" in every view's context
  menu besides `S N`, with a hint in the active view's label strip until the click or Escape.
  Fixture `tests/fixtures/site/forms.html`.
- Input replay (RB-020-03): a click, typing, a key or a hover in the active view reaches every
  other Chromium view as real input over CDP (`Input.dispatchMouseEvent`,
  `Input.dispatchKeyEvent`, `Input.insertText`, a tap through `Input.dispatchTouchEvent` in
  touch views), so the target page sees trusted events with `:hover`, focus and default
  actions. Text arrives the same with every keyboard layout (AltGr characters included), the
  point follows each view's emulation scale and pinch zoom, a missing or hidden target desyncs
  the view, and with leader-only submit a replayed click or Enter cannot submit a form outside
  the leader. Right clicks only with the new `sync.contextMenu` (off by default). MCP `click`,
  `type`, `hover` and `press_key` run on the same dispatch (registered by RB-030-01); the
  replay time per view is logged at debug level (`replay latency`, p50/p95). Fixtures for the
  sync end-to-end runs: `latency.html`, `mobile-menu.html`, `form.html` and the POST-counting
  server `tests/fixtures/server/post-counter.py`.
- Navigation sync as a fallback (RB-020-06): when a synced click does not take the other views
  to the active view's page within `sync.navigationSettleMs` (default 1000 ms), they load its
  URL - in their own environment when they are in another one; a SPA route (`pushState`, the
  hash) follows through the history API in the other views' documents, without a reload; hash
  changes count per `sync.navigationIncludeHash`. Back, forward, reload and the address bar
  reach every view whose navigation is coupled; a breakpoint with `syncNavigation: false` (or
  `sync.toggle_view_navigation`, `Ctrl+Shift+Space S U`, MCP `sync_set` with
  `feature: "navigation"`) keeps its own page and gets an address field in its label strip.
  Browsersync's ghost mode in a page switches sync off with a notice and "Sync anyway"
  (`sync.detectBrowsersync`). Back and forward also on `Ctrl+Shift+Space V Left`/`V Right`.
- Firefox engine over WebDriver BiDi (RB-040-02): a breakpoint with `engine: "firefox"` opens
  a real window of the installed Firefox (found by `engines.firefox.path`, the registry or the
  standard folders; no geckodriver), started with rBrowser's own profile in
  `.rbrowser/out/ff-profile` (never a folder with someone else's data) on a random port on
  127.0.0.1, one Firefox per window, quit with the app. Viewport and DPR, user agent, locale,
  color scheme, screen orientation, offline and extra headers are emulated per view; what
  Firefox cannot emulate (touch, reduced motion, forced colors, contrast, CPU and network
  throttling, print, ...) is marked at the view. The sync agent runs in a BiDi sandbox, the
  view follows the other views' navigation, clicks, keys, hover, wheel, scroll and form state
  as real input, and with `engines.firefox.syncSource` a click in the Firefox window makes it
  the leader. Without Firefox the view says "Firefox was not found" with a link to the setting
  and the rest runs on. Settings `engines.firefox.path`, `profileDir`, `syncSource`,
  `extraPrefs` (replacing `engines.firefoxPath`, which is still read); command
  `engine.firefox.toggle` (`Ctrl+Shift+Space R F`, MCP `engine_restart`). New crate `rb-bidi`.
- WebKit through Playwright (RB-040-04): a breakpoint with `engine: "webkit"` opens its own
  headed WebKit window - Playwright's WebKit build, labelled "WebKit (not Safari)" - driven by
  the Node sidecar (`webkit.*` methods, one browser context per view with the breakpoint's
  viewport, DPR, mobile, touch, user agent, locale and media). The window follows its cell's
  navigations and the SyncHub replays clicks, keys, text and scrolling into it as real input
  (`page.mouse` / `page.keyboard`) after the sync agent resolved the target - missing or hidden
  targets desync it; input in a WebKit window feeds the hub while it is the active view
  (`engines.webkit.syncSource`). What WebKit cannot emulate (platform, reduced data and
  transparency, color gamut, network throttling, safe areas, text scale) is marked at the view.
  Without the WebKit build the view says so and names "Update engines" (`Ctrl+Shift+Space V U`);
  a finished update starts it. A sidecar crash shows "engine stopped", and `engine_restart`
  brings the windows back. `engines.update` now downloads the pinned WebKit archive itself and
  checks it against our SHA-256 (`RB_WEBKIT_*` in `scripts/lib/tool-versions.sh`) before it
  unpacks anything; Playwright's unverified installer is no longer run. Settings
  `engines.webkit.enabled`, `engines.webkit.syncSource`; command `engine.webkit.toggle`
  (`Ctrl+Shift+Space V K`, MCP `engine_restart`).
- Engine contract and external windows (RB-040-01): `trait Engine` / `trait ViewHandle` in
  rb-engine (navigation, history, emulation with a per-field report of what the engine cannot
  emulate, input in CSS px, screenshot, window placement, events, close), Chromium behind it
  over the existing view host without a change in behaviour, a fake engine and conformance
  checks every engine must pass, and the manager of the Firefox and WebKit windows: started
  when the layout first needs them, placed `beside` the main window, on `monitor:<n>` or where
  they were (`remember`), cascaded, an ended window shown at once and restarted with its last
  page (at most three times with a growing wait), all closed with the app. The SyncHub routes
  each view to its engine's replay (`rb_sync::EngineRouter`). Until Firefox (RB-040-02) and
  WebKit (RB-040-04) land, a breakpoint with `engine: "firefox"` or `"webkit"` shows "not
  available yet" in its cell's label strip while Chromium stands in. Settings
  `engines.external.placement`, `autoRestart`, `showTiles`; commands `view.external.arrange`
  (`Ctrl+Shift+Space V D`), `view.external.raise` (`V W`) and `engine.restart` (`R E`, MCP
  `engine_restart`).
- SyncHub, leader-only submit and the desync badge (RB-020-02): the active view's captured
  events go to every other view in capture order, one queue per view; a form submit runs only in
  the active view and the others load its URL once its navigation commits
  (`sync.leaderOnlySubmit`); a view whose target is missing, hidden or does not answer within
  `sync.desyncTimeoutMs` (default 750 ms) gets a "desynced" badge in the grid gap above it,
  with the reason in the tooltip and resync on click, while the others carry on. Commands
  `sync.toggle` (`Ctrl+Shift+Space S S`), `sync.skip_next` (`S N`), `sync.resync` (`S R`) and
  `sync.toggle_view` (`S X`), all MCP `sync_set`. The agents follow the active view and the
  switches live, and a re-attached view keeps one agent session (review findings of wave
  0.1-w1c). Replaying the events is RB-020-03.
- Page API and message bus (RB-020-08): every view's page gets a frozen `window.__rbrowser`
  (`id`, `label`, `width`, `height`, `index`, `engine`, `emulation`, `bus`) that follows a new
  breakpoint or emulation with the event `rbrowser:viewchange`; `bus.send(type, payload)`,
  `bus.on` and `bus.off` reach the same page in the other views - never the sender, only pages
  of the same origin, at most `bus.maxPayloadBytes` (64 KiB) per message and `bus.maxPerSecond`
  (100) per view, checked again by the isolated agent and the core. Every view's user agent ends
  in ` rBrowser/<version>`, an emulated one included. Settings `pageApi.enabled`,
  `pageApi.uaSuffix`, `bus.enabled`, `bus.maxPayloadBytes`, `bus.maxPerSecond` (Sync > Page API,
  Message bus); toggles `bus.toggle` (`Ctrl+Shift+Space S B`), `page_api.toggle` (`S A`) and
  `page_api.toggle_ua_suffix` (`S I`); MCP `bus_send`; page types in `agent/types/rbrowser.d.ts`.
- Flows (RB-040-07): `.rbrowser/flows/<name>.ts` - TypeScript with top-level `await`, no imports,
  `page`, `view`, `expect` (`@playwright/test` 1.65.0), `env` and `step(name, fn)` injected, head
  comments `// @description`, `// @views`, `// @timeout` - run in every chosen view at once
  through one internal CDP proxy per view, in a worker process of the sidecar under Node's
  permission model (reads the project and its own code, writes only `.rbrowser/out/flows/<run>/`,
  no child processes, environment reduced to `SystemRoot` and the temp folder). Per view
  `passed`, `failed` (error with step and line, screenshot), `timedOut` or `skipped` (Firefox and
  WebKit views, with the reason) in `result.json`. Only in a trusted workspace (K9): flows are
  listed but refused with the way to the trust dialog. Commands `flows.run` (`Ctrl+Shift+Space
  F R`), `flows.run_all` (`F A`), `flows.rerun_last` (`F L`), `flows.cancel` (`F X`),
  `flows.open_results` (`F O`); MCP tools `flow_list` and `flow_run`; `rbrowser flow list` and
  `rbrowser flow run <name> [--views a,b] [--json]` (exit code 1 on failure); settings
  `automation.flows.dir`, `.parallel`, `.timeoutMs`, `.syncDuringRun`, `.screenshotOnFailure`.
- CDP proxy for outside clients (RB-030-06, K10): new crate `rb-cdp` (token, request routing,
  target filter, per-client sessions, emulation and input rules) and `crates/rbrowser/src/cdp_proxy/`.
  Playwright's `connectOverCDP(url, { headers })` and other CDP clients reach the grid's views on
  `127.0.0.1` (`cdp.enabled`, off by default; `cdp.port`, random when empty): `/json/version`,
  `/json/list`, `/json/protocol` and the WebSockets `/devtools/browser/<id>` and
  `/devtools/page/<id>`, each behind a token from the OS's random source (`Authorization: Bearer`
  or `/t/<token>/`, constant-time, never logged, 401 without), a loopback `Host` and no foreign
  `Origin`. Every client gets a flat session of its own on the pipe; the shell, the settings
  window, DevTools and the analyser's hidden page are no targets (`/json/list`,
  `Target.getTargets`, `Target.*` events, any `targetId` refused, auto-attached ones detached).
  `Emulation.setDeviceMetricsOverride` and the other size commands are answered without effect
  (rBrowser owns each view's size, so `setViewportSize` does nothing); other emulation is
  re-applied when a client leaves; `Input.*` is scaled to the view. `cdp.copy_endpoint`
  (`Ctrl+Shift+Space , E`) shows the endpoint, `cdp.rotate_token` (`, O`) replaces the token and
  ends open connections. MCP tool `cdp_endpoint` (URL, header, page socket, the URL from WSL).
  NAT tunnel: `rbrowser-wsl cdp-proxy` listens in the distribution and multiplexes connections
  over stdio to the proxy, started on demand by `cdp_endpoint` or by hand with
  `rbrowser cdp-tunnel`.

- Playwright `connectOverCDP` through the CDP proxy, proven end to end (RB-040-06): the ignored
  test `cdp_proxy::playwright_tests` (stage `playwright-e2e` of `scripts/check.sh --rust`) runs
  the sidecar's playwright-core 1.65.0 (`sidecar/e2e/connect-over-cdp.ts`) against the proxy in
  front of the scripted browser: no or a wrong token (header or path) is refused, the header,
  `/t/<token>/` and `ws://` forms see one context with exactly the views, `setViewportSize`
  reaches no view, a click lands scaled, `page.goto` navigates only the chosen view. Example
  `docs/examples/playwright/` (connect, one step per view; the endpoint from `RBROWSER_CDP_URL`
  and `RBROWSER_CDP_TOKEN` or a saved `cdp_endpoint` answer, never in the script). The proxy now
  owns `Emulation.setEmulatedMedia` as well (answered, not sent), so a connected Playwright no
  longer turns dark views light. `cdp_endpoint` answers a ready `snippet`; `cdp.copy_snippet`
  (`Ctrl+Shift+Space F C`) shows it.

- MCP server (RB-030-01): crate `rb-mcp` on rmcp 3.9, Streamable HTTP on `127.0.0.1` with a
  random port (or `mcp.port`) behind a bearer token from the OS's random source, compared in
  constant time, `Host` and `Origin` held to the loopback; off by default (`mcp.enabled`,
  command `mcp.toggle`, `Ctrl+Shift+Space , M`). An instance file per project (port, token,
  pid; the user's own) with a nonce probe that never sends the token, so a file left by a crash
  is ignored. `rbrowser mcp` forwards stdio to the project's running instance and ends with
  exit code 3 and a clear message when there is none; `mcp.copy_config` (`, C`) shows the
  `claude mcp add` line. Core tools, each through the one command registry: `views_list`,
  `navigate`, `reload`, `back` and `forward` (the final URL of every view, waiting up to
  `mcp.navigationTimeoutMs`), `config_get`, `settings_get`, `settings_set` (protected keys
  refused), `project_info`, `project_open`, `breakpoints_detect`, `breakpoints_apply`,
  `view_set_breakpoint`, `emulation_set`, `layout_set`, `environment_set`, `devtools_open`,
  `app_info`, `wsl_info`, `diagnostics` and `plugins_list`; reading the pages with `snapshot`
  (the accessibility tree), `console_logs`, `network_requests` (never headers or bodies) and
  `errors` from per-view buffers (`mcp.consoleLogLimit`, `mcp.networkRequestLimit`,
  `mcp.errorLimit`), and `evaluate`, off unless `mcp.allowEvaluate`; answers masked and cut at
  `mcp.maxAnswerBytes`. `config_set` and `breakpoint_add`/`_update`/`_remove` write the project
  config after its check and refuse keys guarded by workspace trust (headers, network, plugins,
  extensions). `target` is `"all"`, a view id or a breakpoint id. The interaction and sync tools
  follow with RB-020-02/03; a coverage test holds every command's MCP entry to a registered or
  pending tool.
- MCP coverage test and tool reference (RB-030-08): every command of the registry and of the
  bundled plugins resolves to a registered tool, a pending tool whose job file exists and is not
  done yet, or an `omitted` reason in plain English (no i18n key, no job placeholder); every tool
  has one name, a description, an object schema and `target: "all" | viewId` when it acts on
  views, and is run by a command or says why not; plugin tools are `<mcp_prefix>_<tool>` with
  one prefix per plugin, never an area of the core tools. `schema/mcp-tools.v1.json` (written by
  `RB_UPDATE_SCHEMA=1 cargo nextest run -p rb-mcp mcp_tools_file`) feeds the wiki's
  `reference/mcp-tools.md`, rendered by `scripts/mcp-reference.sh` (`--check` in `--preflight`)
  with each tool's parameters, target, command and guard (`evaluate` behind
  `mcp.allowEvaluate`).
- Code signing, prepared and off (RB-090-03): `scripts/sign-windows.sh` signs the Windows
  binaries with Authenticode (SHA-256, RFC 3161 timestamp, `RB_SIGN_TIMESTAMP_URL`) and verifies
  every signature - osslsigncode in WSL with a PFX or a PKCS#11 token, signtool on Windows, Azure
  Trusted Signing prepared. A package ZIP or folder gets ours and CEF's unsigned binaries signed;
  `node.exe` and vendor-signed DLLs keep theirs. `scripts/package-windows.sh --sign` (or
  `RB_SIGN=1`) checks the configuration before the build and signs before the ZIP is written;
  `release.yml` signs the ZIP behind `RB_CODESIGN`. The key comes from the environment only and
  neither password nor key path is ever printed. Phase 1 stays unsigned
  (`docs/security/code-signing.md`).
- Self-update prepared and inactive (RB-090-02): the crate `rb-update`, linked only with the
  app's Cargo feature `self-update` (off), implements the Ed25519-signed update manifest per
  channel with a placeholder root that fails closed, the stored replay floor, an https-only
  download with a size cap and a SHA-256 check, and a journalled switch of the program folder
  with roll-back and recovery at start. The portable mode's files are never moved. `rb-pack`
  (keygen, build, verify) and `scripts/sign-update.sh` build the manifest, and `release.yml`
  signs it only with `RB_SELF_UPDATE`. Settings > General > Updates adds `updates.enabled`,
  `updates.check` and `updates.autoInstall` (protected, off) and shows the update keys locked,
  with a hint, while the feature is off. No request is made while the feature or the setting
  is off.
- Sitemap capture, in progress (RB-050-05): the plugin `sitemap-capture` finds a site's
  sitemaps (`robots.txt`, `/sitemap.xml`, `/sitemap_index.xml`, indexes, gzip) under size,
  depth and count limits, selects pages (glob or `re:` patterns, `hreflang`, `lastmod`, one per
  template, every n-th, max N, `robots.txt`), rewrites the origin to an environment and renders
  a script-free, escaped HTML gallery; `rb-engine` gains hidden render views
  (`chromium::hidden`, a pool) and a batch with parallelism, per-host spacing, retries, timeout
  and cancel; `rb-visual` keeps a run's `run.json` below `.rbrowser/out/sitemap-capture/<run>/`
  and resumes it without capturing a page twice. Commands, MCP, CLI and the CEF views follow.
- Visual regression (RB-050-04): `visual.compare` (`Leader C R`, MCP model `visual_compare`)
  captures the views and compares each with its baseline in
  `.rbrowser/baselines/<page>/<breakpoint>.png` - a pixelmatch-style YIQ diff with
  `visual.threshold` and anti-aliasing tolerance, a diff image and bounding boxes, a size
  mismatch instead of a failure - and reports ok, differs (with the share in percent), new, size
  mismatch or failed through the new check plugin `visual-regression` into the findings panel;
  diff images go below the screenshot folder. `visual.baseline_update` (`Leader C B`, MCP model
  `visual_baseline_update`) stores the views as baselines with `meta.json` (machine, OS, pixel
  ratio, engine and CEF version, viewport, date; the URL without query or credentials); a
  baseline from another setup makes the comparison "unreliable" (an error with
  `visual.failOnMetaMismatch`). Saving checks that Git LFS tracks the baselines
  (`visual.lfsCheck`) and names `visual.lfs_setup` (`Leader C G`, no MCP tool), which appends the
  `.gitattributes` line and runs `git lfs install --local` only on the person's command. Baseline
  folders never follow a link. Settings: Capture > Visual regression (`threshold`, `fullPage`,
  `slugStrategy`, `lfsCheck`, `failOnMetaMismatch`).
- Emulation extras (RB-080-10): per breakpoint (and the project defaults) `geolocation`
  (`null` switches it off), `timezone` (IANA names, checked against tzdb), `cpuThrottling`
  (1 to 100), `offline` (through the network conditions) and `paper` (A4 794 px, Letter 816 px
  wide) for a print view with `media.mediaType: "print"`; live per view from the toolbar's
  extras popover (presets Berlin, New York, Tokyo, Sydney and free input; Shift-click for every
  view) and the commands `view.emulate.geolocation` / `timezone` / `cpu` / `offline`
  (`Ctrl+Shift+Space V G` / `V T` / `V H` / `V L`, MCP `emulation_set`) and `view.add_print`
  (`V Y`, MCP `breakpoint_add`). The position is granted to the view's own origin only; every
  value is sent again after a CDP reconnect. Settings > Emulation > Geolocation, time zone and
  CPU holds the shortcuts' defaults and your own location presets.
- About page (RB-010-18): Settings > About shows the version, commit, build time and platform,
  CEF and Chromium, Firefox, WebKit and Node (absent until their jobs detect them; a missing
  Firefox links to the engine settings), the plugin API and config schema versions and the data
  and log folders, with "Copy version info" (a block for bug reports, without the folders), the
  internal source, handbook and changelog addresses, the author and the update status. The
  Licences tab lists rBrowser's GPL-3.0-or-later and the SDK's MIT licence and every shipped
  component with its licence - the bundled binaries, the Rust crates of the app and its plugins
  and the npm packages of the shell - generated by `scripts/licenses.sh` and filterable by name
  or licence. Command `app.about` (`Ctrl+Shift+Space , A`); MCP `app_info` answers the same facts
  from the same function.
- Sessions (RB-020-07): `default` stays the one cookie and storage context every Chromium view
  shares; each named session of `sessions` / `breakpoints[].session` gets a CEF request context
  of its own with cache, cookies and storage in `<profile>\sessions\<id>`, persistent across
  restarts; session cookies (logins without an expiry) only with the user setting
  `sessions.persistSessionCookies` (Security > Sessions, off by default, protected from MCP). A named session shows a badge in its colour
  (`sessionColors`, else an automatic one) beside the cell label. `session.assign` (`Leader E S`,
  MCP `session_assign`, `target: "all" | viewId`) recreates views in another session and reloads
  their URL; `session.clear` (`Leader E K`, asks first, MCP `session_clear`) deletes the
  session's cookies and the storage of the origins its views show. Session ids are checked as
  plain folder names (`config.error.invalid_session_id`); `sync.leaderPerSession` keeps one
  submit leader per session. A login fixture (`tests/fixtures/site/sessions-server.py`) and a
  sessions project serve the acceptance.
- Findings of check plugins (RB-060-01): one findings model in `rb-plugin-types` (stable id from
  the rule or shared id, URL and element path; plugins, views, breakpoints, message key and
  parameters, element, help page, data) with `schema/findings.v1.json`; the `check-plugin` world
  describes its rules (severity, shared id, help page) and runs a context (view, URL, rules
  switched on), and `host.report-progress` reports progress; the host's check runner runs every
  check plugin over the target's views in parallel (`checks.parallel`), applies
  `checks.rules` (`off` or a severity) and `checks.ignore`, merges findings across views and
  plugins and keeps the last `checks.keepRuns` runs in `.rbrowser/out/checks/`; the argument and
  answer model of the core MCP tool `checks_run`; the findings panel in the shell (filters by
  severity, plugin and view, grouped by rule, "only in" breakpoints, click to highlight); the SDK
  template `check` with two rules; in the app `checks.run_all` (`Ctrl+Shift+Space K K`) runs the
  loaded check plugins over the views, `Ctrl+Shift+Space P F` shows the panel, and a finding's
  element is marked in its views (CDP `Overlay.highlightNode`). The plugin id `checks` is
  reserved.
- Screenshots (RB-050-03, ahead of 0.5): viewport at the breakpoint's size and DPR, full page
  and element (CSS selector or picked with a click, with padding) of one view or all, captured
  over each view's CDP session - the breakpoint emulation is restored after every capture, views
  out of sight are captured too. One overview image of all views side by side with label, size,
  engine and environment colour, composed in Rust (new crate `rb-visual`, `png`). Files go to
  `.rbrowser/out/screenshots/<date-time>/<breakpoint>.png` and never overwrite; settings
  Capture > Screenshots (`capture.dir`, `capture.pathPattern`, `capture.elementPadding`,
  `capture.defaultDelay` 0/5/10 s, `capture.overviewLabels`); commands `capture.views`,
  `capture.full_page`, `capture.all_viewport`, `capture.element`, `capture.overview` and
  `capture.delay_cycle` on `Ctrl+Shift+Space C S/F/A/L/O/D`, with the argument and answer
  model of the MCP tool `screenshot`; `rbrowser screenshot` (`--views`, `--full`,
  `--element`, `--overview`, `--delay`, `--out`, `--json`) captures in the project's running
  rBrowser over the instance channel and prints the paths (exit 3 when none runs, 1 when a view
  failed).
- Emulation per view (RB-010-05): every emulation field of a breakpoint is applied over CDP -
  media features (`prefers-color-scheme`, reduced motion, data and transparency, forced colours,
  contrast, colour gamut, print), touch, mobile, user agent and platform with client hints, locale
  (Intl and `Accept-Language`), text scale, safe-area insets, orientation, network throttling
  (offline, slow 3G, fast 3G, 4G or custom) and request headers - resolved from the breakpoint,
  its device preset and the project defaults, live without a reload and again after every
  reconnect. Headers from `config.json` and `config.local.json` apply only in a trusted
  workspace. Commands `emulation.set` (MCP `emulation_set`, `target: "all" |
  viewId`), `emulation.toggle_color_scheme`, `emulation.toggle_reduced_motion` and
  `emulation.reset_view`; toolbar controls where Shift-click applies to every view; a notice when
  a change needs a reload; the fixture `tests/fixtures/site/emulation.html`.
- Workspace trust (RB-010-09): an untrusted project loads its breakpoints, layout, environments
  and emulation, but its flows, snippets, project plugins, mocks, page scripts, request headers,
  extension recommendations and the network overrides `ignoreCertificateErrors`, `proxy` and
  `hostResolverRules` (each from `config.json` and `config.local.json`, which a clone can commit
  too) stay blocked until a person trusts it in the shell's dialog. The decision is stored per
  project path and a SHA-256 of the protected parts in `trust.json` beside the user settings
  (written under a file lock); a change to a protected part asks again, a change to breakpoints
  does not; revoking blocks again at once. A symbolic link or junction in a protected folder
  counts by the content it leads to inside the project; one that leads out of the project or
  nowhere keeps the project untrusted. No MCP tool, command or setting grants trust
  (`trust.grant` only opens the dialog). Network overrides change at the next start.
- CLI and single instance (RB-010-11): `rbrowser [URL|DIR]...`, `rbrowser open`, `rbrowser init
  [--url] [--yes]` (the analyser's proposal or a mobile + desktop config, `.rbrowser/.gitignore`,
  never over an existing config), `--project`, `--reload`, `--background`; a Linux path from WSL
  becomes `\\wsl.localhost\<distro>\...` (`/mnt/c/...` the drive). One instance per project: the
  instance file `%LOCALAPPDATA%\rBrowser\instances\<hash>.json` names a named pipe open to the
  current user only (DACL with the user's SID, no remote clients) and a nonce; a second call
  hands its URL and flags over and ends with 0, every request is validated (known kinds only,
  nonce, `http`/`https`/`file` URLs). Documented exit codes (`--help`); the recent projects and
  `project.open_recent`.
- DevTools docked or in a window (RB-010-06): `devtools.toggle` (`Leader D D`), `devtools.open`
  (`Leader D V`, MCP `devtools_open` with `target: viewId`) and `devtools.dock_cycle`
  (`Leader D W`) open DevTools below or beside the grid (the hosted frontend in a view of its
  own, the screencast switched off) or as the native DevTools window, after the setting
  `devtools.mode`; docked DevTools follow the active view (`devtools.followActiveView`). The dock
  connects through an internal WebSocket endpoint on 127.0.0.1 with a one-time token per
  connection, pinned to the inspected page; DevTools' own targets are hidden from outer CDP
  clients. The view gets its breakpoint emulation back when DevTools close. The dock size and
  the window's bounds are kept in `devtools.json`; the toolbar names the inspected view.
  `shortcuts.disableInDevtools` now works: keys typed in DevTools reach DevTools.
- Environments, layout presets and device presets (RB-010-12): switching the environment
  (toolbar, `environments.select`, `Leader E 1...9`) keeps path, query and hash in every view and
  frames the grid in the environment's colour; layout presets (`Leader L 1...9`, the toolbar)
  rebuild the grid without reloading unchanged views; focus mode (`Leader L F`) shows one view with
  a quick switcher (`Leader L ]`/`[`, the arrow keys while the toolbar has the focus,
  `focusMode.arrowKeys`); zoom to fit (`Leader L Z`); the built-in device list
  `crates/rb-config/data/devices.json` (16 phones, tablets and desktops) merged with the user's
  `devices` (`replaceDefaults`), answered to `getDevices`. A breakpoint that names a device
  preset (`"device": "iphone-15"`) may leave out `width` and `height`: the preset's viewport
  fills them, and whatever the breakpoint sets wins - in the grid and its labels, the emulation,
  the size quick pick, `Leader ]`/`[`, the shell's breakpoint list and the analyser's taken
  widths. A breakpoint with neither a size nor a known preset is refused at its field
  (`config.error.size_required`, `config.error.unknown_device`); user presets count.
- The settings window (RB-010-08): a window of its own (`settings.html`, also the home of the
  trust dialog and the analyser's wizard) generated from the schema - 18 categories, every key
  with a control for its type (lists and object lists with add, remove and reorder, paths with
  the native file dialog), the origin badge *Default*/*User*/*Project*/*Local*, where a change
  is saved (local-only keys never in `config.json`, with the reason), search over label, key and
  description, "only changed", reset per field and section, a read-only JSON view per scope and
  deep links (`settings.open { key }`) that highlight the field. Writes are checked by the core
  (the key's schema, the typed config, `x-protected` keys refused for MCP callers), change only
  the keys that differ and apply at once (`layout.gap` in the grid, `logs.level` in the log).
  Settings > Keyboard Shortcuts records a binding, refuses reserved and conflicting ones with the
  reason, resets and exports/imports the shortcut file; the which-key bar shows the next keys
  after the leader.
- Internal ZIP for team tests (RB-010-15): `scripts/package-windows.sh` builds the front ends it
  ships (`ui/`, `agent/` and `sidecar/`, `npm ci --ignore-scripts` from their lock files), gives
  CEF's bootstrap (`rbrowser.exe`) rBrowser's icon and version resource (rcedit, pinned by
  SHA-256), adds `crash_reporter.cfg` (local dumps, no upload), the Node runtime of `.nvmrc`
  (`node\node.exe`, its ZIP held to a SHA-256 pin in the build cache), the sidecar
  (`sidecar\dist\main.js` with the production `playwright-core` and its notices), the bundled
  plugins (`scripts/build-plugins.sh --list`, each also written as
  `artifacts/plugins/<id>-<version>.rbplugin`), `README.txt` (SmartScreen, portable mode,
  diagnostics), `NOTICE` (CEF, Chromium, Node, Playwright) and Chromium's credits, and prints a
  size report; a dirty tree stops it unless `--allow-dirty`, `--sign` ends with "signing not
  configured". Portable mode: a file `portable` beside `rbrowser.exe` keeps settings and logs in
  `data\` beside it. `scripts/run-windows.sh --package` (`run-windows.ps1 -Package`) starts the
  unpacked ZIP.
- Breakpoint analyser (RB-010-13), the first first-party plugin `plugins/breakpoint-analyzer`
  (`analyzer-plugin` + `feature-plugin`, no capability): the `@media` boundaries of a site's
  stylesheets (`min-width`, `max-width`, range syntax, `em`/`rem`, nested and combined
  conditions; `@container` as a hint), merged within `analyzer.mergeTolerancePx`, labelled after
  Tailwind or Bootstrap, proposed in the modes `all`, `simplified` and `edges` with
  `analyzer.edgeViews`; the page is read in a hidden Chromium target over CDP, `rbrowser init`
  fetches it without a window (`analyzer::init_config`; `--url file:///...` reads a static
  site on disk, percent-escapes decoded, but only when named on the command line). The wizard (`Leader B A`, the toolbar
  button) lists the boundaries with their stylesheets, only the new breakpoints when the project
  has some, a grid preview, and writes `config.json` only on the button; `Leader B N` appends the
  detected breakpoints; MCP `breakpoints_detect` and `breakpoints_apply`; `analyzer.checkOnOpen`
  gives a notice. The app loads the bundled plugins (`<exe>\plugins`, signed with the embedded
  release key `rbrowser-plugin-v1`, unsigned only with `plugins.developmentMode`). K5 fixtures
  for Tailwind v3, v4 and Bootstrap.
- GitHub workflows, prepared and inactive until the move to GitHub (RB-090-04): `ci.yml`,
  `e2e.yml`, `release.yml`, `advisories.yml` and `dependabot.yml` call the local `scripts/`,
  start from `permissions: {}` and pin every action to a commit; code signing and update
  manifests stay behind `RB_CODESIGN` and `RB_SELF_UPDATE` - the signing certificate is written
  only in a step after the build, and a signing request without a signer fails the release. `check.sh --preflight` runs
  `scripts/check-actions-pinned.sh` and actionlint (pinned at 1.7.12); new `scripts/advisories.sh`,
  `scripts/release-assets.sh` and `scripts/ci-tools.sh`; secrets and variables in `docs/release.md`.
- Settings registry (RB-010-02): `rb-settings` checks every setting (section, scopes, i18n keys,
  a unique `x-order` per section), reads the `x-*` annotations of the config, user-settings and
  plugin schemas, resolves values with their origin (local > project > user > default) and refuses
  writes to a scope a key does not allow, and parses `rbrowser://settings/<key>` deep links; the
  settings lint `scripts/check-settings.sh` and the wiki reference generator
  `scripts/settings-reference.sh` run in `scripts/check.sh --preflight`.
- Logging and diagnostics (RB-010-10): the crate `rb-diag` - a size-rotated, masked
  `rbrowser.log` under `%LOCALAPPDATA%\rBrowser\logs` with a runtime log level and a panic hook
  that logs the backtrace; secret redaction (tokens, cookies, `Authorization`, header values);
  the diagnostics ZIP (`rbrowser diagnostics [--out]`, command `diagnostics.create`) with logs,
  crash dumps, versions, the masked effective config and the plugin list; the content of
  `crash_reporter.cfg` for local crash dumps without upload (not in the package yet); the notice
  for a CEF build older than six weeks - its release date (`RB_CEF_RELEASE_DATE` in
  `scripts/lib/tool-versions.sh`) embedded by `build.rs`, shown in the shell's status line at
  startup and switched off with `updates.cefAgeWarning`. The log starts first in the browser
  process, CEF logs into the same folder. No telemetry.
- The plugin contract and host (RB-010-03): the WIT package `rbrowser:plugin@0.1.0` with the
  worlds `feature-plugin`, `check-plugin`, `analyzer-plugin` and `export-plugin` and a native trait
  per world (`rb-plugin-api`, with an in-memory test host); the wasmtime host `rb-plugin-host` -
  compile cache, fuel, memory and time limits, call gate, suspension after repeated failures, log
  budget, signed `.rbplugin` packages covering every file, trust on first use, one import per
  capability (views, evaluate, CDP domains, rate-limited HTTP, project reads, writes only into
  `.rbrowser/out/<plugin>/`, findings, sidecar), unique MCP prefixes, project plugins only under
  workspace trust, conformance checks and the `rb-plugin-pack` tool; shared types in
  `rb-plugin-types`; four MIT SDK templates with `scripts/check-sdk-templates.sh`,
  `scripts/build-plugins.sh` and `scripts/check-plugin-imports.sh`; ADR 0002.
- Shell UI scaffold (RB-010-07): `ui/` with Vite, Vue 3, Nuxt UI v4 as a Vue plugin, strict
  TypeScript and Vitest; the toolbar (back, forward, reload and the address bar, which the core
  carries out in every view; global sync switch, layout preset and environment as native selects,
  analyse breakpoints and settings, which wait for their jobs; light/dark logo), a status line
  for notices, the
  `cefQuery` / `rb-ipc` transport with a timed client and a mock core, the IPC contract test
  (TypeScript and Rust halves), the theme from `ui.theme`, English strings through vue-i18n;
  `npm run build && npm test` runs in `scripts/check.sh --preflight`. A cell label page
  (`ui/cell-label.html`) is kept unused: the labels are native (RB-010-04).
- Repository infrastructure taken over from rCommander (RB-000-01): Cargo workspace (edition
  2024) with the placeholder binary `rbrowser` (version resource, icon and long-path manifest on
  Windows) and `rb-process` (no console window for spawned programs); the repository lints
  `file_length`, `no_german` and `no_console_window`; `scripts/` with `check.sh`
  (`--preflight`, `--rust`, `--windows`, `--full`), `worktree.sh`, `session-state.sh`,
  `set-version.sh`, `package-windows.sh`, `run-windows.sh`, `archive-jobs.sh` and their tests;
  `deny.toml`, `.gitleaks.toml`, nextest profiles; `AGENTS.md`, the Claude Code settings, hooks,
  the `wave-worker` and `auditor` agents and the `wave`, `handoff` and `release` skills.
- Newest versions determined and pinned (RB-000-02): Rust 1.99.1, Node 26.13.0 (`.nvmrc`), the
  shared crates in `[workspace.dependencies]` and the development tools in
  `scripts/lib/tool-versions.sh`; `scripts/tool-versions.sh` lists and checks the pins,
  `scripts/update-deps.sh` moves every dependency to its newest release and runs the gate.
- Documentation scaffold (RB-000-03): `ROADMAP.md`, `docs/roadmap.md`, the job index with all
  109 job files for 0.0 to 1.1, feature list, architecture, plugins, settings structure,
  shortcuts, Definition of Done, threat model, development guide; wiki base pages; a Markdown
  link check in `scripts/check.sh --preflight`.
- The CEF app and its grid (RB-010-04): `rbrowser.dll` in CEF's sandboxed bootstrap layout
  (`rbrowser.exe`, sandbox on; `--no-sandbox` and `rbrowser-nosandbox.exe` as the fallback), one
  window with the shell view (`rbrowser://ui/`, a placeholder until the shell is built) over a grid
  of Chromium views built from the project's `layout` with nested BoxLayouts, a native label
  strip per cell in the grid gap (breakpoint and size; a click makes the view active), each
  breakpoint scaled into its cell over CDP (`setDeviceMetricsOverride`, re-applied
  after resize and reconnect) on the in-process CDP pipe (no port); the grid is rebuilt when
  `.rbrowser/config.json` changes without reloading kept views; a browser profile per project;
  commands `view.zoom_fit_all`, `view.zoom_in`/`zoom_out`, `view.focus_next`/`focus_prev`
  (`⌘ V Z/I/O/]/[`), `app.reload_config` (`⌘ , R`); the setting `engines.chromium.sandbox`; `scripts/lib/cef.sh` assembles the app folder and holds the CEF archive to
  our own SHA-256 pin.
- ADR 0001 (RB-000-07): CEF architecture decided from the spikes RB-000-04 … RB-000-06 — cargo-xwin
  build, Alloy-style grid views with BoxLayouts, emulation scale and the CDP input contract, CDP
  over pipes behind a token proxy, docked DevTools, Firefox over WebDriver BiDi.
- Contracts of wave 0.1 (`docs/contracts-0.1.md`): the crates `rb-config` (project config and
  user settings model), `rb-settings` (categories, sections, scopes, annotations), `rb-commands`
  (command ids, leader groups, bindings, reservation list format), `rb-engine` (`ViewHost`,
  `Emulation`, grid cells), `rb-ipc` (shell <-> core messages, mirrored in
  `ui/src/ipc/protocol.ts`) and `rb-i18n` (`t()`, `locales/en.json`), with stub bodies for
  RB-010-01, -02, -04, -05, -07, -14 and -17.
- Application icon generated from `assets/rBrowser-icon.png`.
- Project config and user settings (RB-010-01): `rb-config` finds the project upwards from a
  path, merges `config.json` <- `config.local.json` (lists by `id`) with the origin of every
  value, ignores the local-only keys in `config.json` with a warning, validates with JSON
  pointers and i18n keys (duplicate ids, unknown cell breakpoints, cells outside the grid, ...),
  migrates older versions in memory (a save backs the old file up), writes canonical JSON (byte-identical round trip,
  `.rbrowser/.gitignore` on the first write) and loads the user settings; the generated,
  settings-annotated schemas `schema/config.v1.json` and `schema/settings.v1.json`; the user
  settings gain `logs` and `updates.cefAgeWarning`; `scripts/export-wiki.sh` generates the
  wiki's `reference/configuration.md`.
- i18n infrastructure (RB-010-14): `rb-i18n` reads the shared catalogue `locales/en.json`
  (compiled in) with `{name}` placeholders and ICU plurals, shows a missing key as itself, loads
  plugin locales below `plugin.<id>.` and has the pseudo-locale `en-XA`; the `t!` macro;
  `scripts/lib/merge_locales.py` merges the wave parts `locales/parts/*.json`;
  `scripts/check-i18n.sh` (in `check.sh --preflight`) fails on unknown and unused keys and raw
  text in `.vue` templates; `docs/i18n.md` with the format the shell implements.
- Command registry and shortcut system (RB-010-17): binding parser, the full Windows reservation
  list (browser, DevTools, text editing, system, `accesskey`, AltGr) with a reason per entry,
  registration with conflict checks and plugin fallback to `⌘ X`, user bindings with export and
  import, the leader state machine with which-key data, the CEF key mapping, the core command
  definitions, and a lint with coverage check over core and bundled plugin defaults; wired into
  the window as the one command registry of the core: `OnPreKeyEvent` of every view and the
  shell feeds the leader, the shell's `runCommand` and the shortcut editor's messages go through
  it, changed bindings are written to `settings.json`. `schema/commands.v1.json` (every command
  with its group, keys and MCP mapping, held current by a test) feeds
  `scripts/keyboard-reference.sh`, which renders the wiki's `reference/keyboard-shortcuts.md`
  (`--check` in `scripts/check.sh --preflight`).
- Breakpoint shortcuts for the active view (RB-010-16): `Ctrl+Shift+1…9` (and `⌘ V 1…9`) set
  breakpoint 1-9 in config order, `⌘ ]` / `⌘ [` the next / previous one by width, `⌘ .` /
  `⌘ -` change the width by 1 px, `⌘ 0` returns to the cell's breakpoint, `⌘ B 1…9` take the CSS
  breakpoints the analyser found and `⌘ V S` opens a free size input (`500x800@2`) over the
  toolbar - live emulation without a reload, sync stays on. A click on the active view's label
  strip opens a native quick pick with the breakpoints, device presets, detected breakpoints and
  the free input; the strip shows the new size for `breakpointShortcuts.overlayMs`. With
  `breakpointShortcuts.newViewModifier` (`Shift` or `Ctrl`) held, a quick pick choice or a
  project shortcut opens a new view instead (`Ctrl+Shift+1…9` hold both already; `Alt` is not
  offered, as with `Ctrl` it is AltGr). A breakpoint's own `"shortcut"` works from the config; a
  reserved (as pressed), malformed or doubly bound one is reported with a link to the shortcut
  settings. MCP coverage `view_set_breakpoint`.
- WSL groundwork (RB-030-04): the crate `rb-wsl` translates paths between Windows and a WSL
  distribution (`\\wsl.localhost\<distro>\...` and `\\wsl$\...` <-> Linux, `C:\` <-> `/mnt/c`),
  lists the distributions from `wsl.exe -l -v` (UTF-16 output, default, running or stopped),
  detects the networking mode (`wslinfo --networking-mode`, else `.wslconfig`, else NAT; the
  setting `wsl.networkingModeOverride` first; cached per distribution), speaks a versioned
  line-JSON stdio protocol with a handshake of version and nonce, and keeps the helper running
  (restart with back-off after a crash, stopped with the app; its error output read in lines of
  at most 64 KiB). The helper `rbrowser-wsl` (`watch`, `cdp-proxy`) is a scaffold for RB-030-05
  and RB-030-06; the app does not start it yet, and the package does not carry it yet. Settings
  `wsl.helperPath` (protected), `wsl.distro`; command `wsl.restart_helper` (leader `R W`, MCP
  `wsl_info`).
- The Node sidecar (RB-040-03): the crate `rb-sidecar` finds Node (`engines.nodePath`, else the
  bundled `node\node.exe`; a wrong major says "Node <x> expected, <y> found" with a link to the
  setting), starts it only on the first request, speaks JSON-RPC 2.0 over stdio (timeouts after
  `engines.nodeRpcTimeoutMs`, `$/progress`, `$/cancel`), pings it after each start, ends running
  requests with an error when it crashes and restarts it with backoff (a crash counts from its
  death, failed starts count toward the crash-loop limit), and logs its stderr masked; Node and
  the Playwright CLI get only an allowlist of environment variables (no `NODE_OPTIONS`,
  `NODE_TLS_REJECT_UNAUTHORIZED`, `PLAYWRIGHT_DOWNLOAD_HOST` or proxy variables); a changed
  setting applies without an app restart (the timeout at once, the paths at the next start);
  quitting never waits for a backoff;
  the TypeScript package `sidecar/` (Node 26, esbuild bundle, Vitest) with `sidecar.ping`,
  `sidecar.versions` and `playwright.install` (WebKit into `%LOCALAPPDATA%\rBrowser\ms-playwright`
  or `engines.playwrightBrowsersPath`, with progress in the status line, an existing build
  recognised); the commands `sidecar.restart` (leader `R S`) and `engines.update` (leader `V U`),
  both without an MCP tool; the sidecar's versions in the diagnostics ZIP when it runs.
- Sync agent (RB-020-01): the TypeScript package `agent/` (one 13 kB IIFE), injected into every
  view and out-of-process iframe as the isolated world `rbrowser` with `Runtime.addBinding`
  (invisible to the page, untouched by its CSP); capture of pointer, click, double click, context
  menu, hover (throttled by the new `sync.hoverThrottleMs`), wheel, scroll, keys, input, change,
  submit and focus in the active view only; robust selectors (`data-testid`, role + name, stable
  id, text, CSS path without generated classes, open shadow roots) with a relative offset;
  resolution to viewport points with `not_found`/`not_visible`; a replay marker against echoes;
  keys and input in a password field marked `sensitive` and masked in every log.
  The protocol is the new crate `rb-sync`, mirrored in `agent/src/protocol.ts` and held to shared
  fixtures (`docs/sync-protocol.md`); `scripts/package-windows.sh` refuses to package without the
  agent.
- Hot reload (RB-030-05): `rbrowser-wsl watch` reports the changes of a WSL project over the
  stdio protocol (inotify folder by folder, ignored folders never watched, debounced, `rescan`
  after an overflow); projects on a Windows drive use ReadDirectoryChangesW, and polling stands in
  when asked for (`reload.pollingFallback`) or when the helper or the native watch cannot start.
  Ignore rules in gitignore syntax: `node_modules`, `.git`, dot folders and editor leftovers by
  default, then `reload.ignore` and `.rbrowser/ignore` (applied at once when it changes). A saved
  stylesheet or image is swapped in place in every view (JS state, scroll position and form
  values stay), anything else reloads every view; pages with their own HMR client are left to it
  (`reload.respectHmr`). Commands `reload.css_only` (`⌘ R C`), `reload.toggle_pause` (`⌘ R P`) and
  `reload.all` with `mode: "full" | "css"` (the MCP tool `reload`); interval reload
  (`reload.intervalSec`, per environment) paused while someone types.

- Command palette (RB-060-11): pressing the leader twice (`Ctrl+Shift+Space Ctrl+Shift+Space`,
  also `Ctrl+Shift+Space V P`) opens a search over the toolbar strip. It covers every registry
  command with its shortcut (plugin commands included), the views (`@`), the breakpoints as
  "Set view to ...", the layout presets, the environments and every setting as a deep link (`#`).
  The search is fuzzy, recent entries come first, an entry can ask for its argument in a second
  step, `Ctrl+Enter` opens the shortcut editor, and closing the palette gives the focus back to
  the view. It is fully keyboard-operable (ARIA combobox). Other features can add entries
  through providers (`ui/src/palette/README.md`). New settings under Keyboard Shortcuts >
  Command palette: `palette.recentCount`, `palette.showDescriptions`, `palette.prefixes`. The
  shortcut editor gains a flat list, search by key press, a reset per group, a
  replace/swap/cancel dialog on a conflict, an import preview that shows changes and conflicts
  before saving, and a mark on plugin defaults that stayed unbound.
- Breakpoint analyser, static source (RB-030-07): with `analyzer.static` on, the analysis also
  reads the breakpoints the project declares - Tailwind v3's `theme.screens` and
  `theme.extend.screens` (the config is read as literals, never run), Tailwind v4's
  `@theme { --breakpoint-*: ... }` over the defaults of the entry that imports `tailwindcss`
  (`initial` included), Bootstrap's `$grid-breakpoints` map or its defaults, and `$breakpoint-*`,
  `$bp-*`, `$screen-*` and Less `@screen-*` variables as a heuristic source. They merge with the
  media query boundaries within the merge tolerance and give them their names; a declared width
  no page uses yet becomes a view of its own. What is not statically readable (a call, a
  variable, a range, a raw query) is a hint, not an error. The host reads the files for the
  plugin: only those both the plugin's `project-read` globs and the new `analyzer.staticGlobs`
  match, inside the project, without links, `node_modules`, dot folders or `.rbrowser/ignore`
  paths, and WSL projects over their UNC path. `rbrowser init`, the wizard and
  `breakpoints_detect` use it: each detected boundary carries its `source` and `heuristic`, the
  proposal its `hints`, and the wizard shows both. Plugin `breakpoint-analyzer` 0.2.0.

- `settings_get` answers the settings window's categories, sections and keys with deep links and
  filters. `settings_set` sets shortcuts (reserved and protected commands refused), and every change
  over MCP is logged and shown (`mcp.notifyOnSettingChange`) (RB-030-03).
- Hidden views are throttled and catch up with the leader when shown; optional CPU budget for
  inactive views; a memory warning; `scripts/perf.sh` measures K3 (RB-020-09).
- Sitemap capture runs from the core: every sitemap page in every breakpoint in hidden views, with
  resume, cancel, progress and gallery; `⌘ C M`/`⌘ C X`/`⌘ P G`, MCP
  `sitemap_capture`/`sitemap_status`/`sitemap_cancel`, `rbrowser capture sitemap`, Capture › Sitemap
  settings (RB-050-05).
- The command palette lists the project's flows ("Run flow ...") and, under "Set view to ...", the
  device presets and detected CSS breakpoints; `shortcuts.edit` is covered by `settings_get`
  (RB-060-11).
- The shortcut editor shows the project's breakpoint shortcuts with a conflict line for each one not
  in force (RB-010-16).
- Notices that name a setting link to it from the status line; Security & Privacy warns while the
  Chromium sandbox is off (RB-010-08).
- A splitter between the grid and docked DevTools resizes the dock, kept as `devtools.dockHeight` /
  `devtools.dockWidth` (RB-010-06).
- Web Vitals and resources per view: the sync agent measures LCP, CLS, INP, FCP and TTFB (soft
  navigations start a new segment); each cell label shows badges with shape and rating word, and a
  click marks the element; CPU share and JS heap per Chromium view warn at `resources.ramWarnMb` and
  `resources.cpuWarnPercent`; thresholds in `vitals.thresholds`; MCP `view_metrics`; `Leader D C`
  and `Leader D R` (RB-060-10).
- Copy cookies (`cookies.copy`, `Leader E J`, MCP `cookies_copy`): a Chromium session's cookies go
  into the Firefox and WebKit views of that session - by hand, on start or after page loads
  (`engines.cookies.*`); the answer counts, never shows a value. Firefox views of a named session
  get their own cookie jar (RB-040-05).
- The plugin runtime (RB-060-12): plugin page scripts in their own isolated world in every view, a
  page-script <-> component bridge with posts to page scripts and panels, a plugin panels window
  (`Ctrl+Shift+Space X P`) with sandboxed, network-free panels, plugin commands in the palette and
  shortcuts, plugin MCP tools `<prefix>_<tool>` with `target`, and the settings
  `plugins.pageScriptsDisabled` / `plugins.panelsHidden`.
- First-party check plugin `a11y-axe`: axe-core 4.14.0 (MPL-2.0, bundled) runs in every view's
  isolated world, for the run only; findings name the breakpoints they occur in and share ids with
  `seo` for missing alt text, nameless links and a missing `html lang`; settings `a11y-axe.tags`
  (WCAG 2.2 AA by default), `.rules`, `.include`, `.exclude`, `.reportIncomplete`; MCP `a11y_check`;
  `Leader K A` (RB-060-06).
- Plugin `overlays`: overflow detector (sideways scroll framed yellow, causes red with overhang and
  logged, `100vw` elements blue), element highlight by CSS selector across all views, layout
  outlines; `Leader D O` / `O F` / `O H` / `O X`, panel "Overlays", MCP `overlay_set`,
  `overlay_overflow`, `overlay_state`, settings under DevTools › Overlays (RB-060-03).
- Plugin `console`: one deduplicated console over every view with a dot per reporting view, failed
  fetch/XHR at the level Network, commands run in all or the active view, findings
  `console.js-error`/`console.console-error`; `Leader P C` / `D X` / `D E`; MCP
  `sharedconsole_grouped`, `sharedconsole_exec` (off until `console.mcpExec`) (RB-060-02).
- Peek: hold Alt (or Ctrl/Shift, `peek.modifier`) over an element for its tag, classes, size,
  colours and WCAG contrast with level, declared and rendered font, box model, role and name; the
  element is outlined in every view; Alt+click locks the tooltip with a copy button; `Leader D P`
  toggles; MCP `peek_inspect` (RB-060-04).
- SEO check plugin `seo`: a page or a small crawl of its site is held to the SEO essentials (status
  and redirects, indexability, canonical, title and description, headings, lang, viewport, charset,
  Open Graph, JSON-LD, sitemap in robots.txt) and to the alt text group `seo.images`, over the raw
  response, the raw HTML and the rendered DOM, with JS-only differences reported; settings in Checks
  › SEO, MCP `seo_check`, `Leader K S`, reports in `.rbrowser/out/seo/` (RB-060-08).
- Rulers and guides plugin `rulers-guides`: rulers in page CSS px, guides dragged out of them (px
  from left or right, % of width; Shift snaps, double-click removes), a column overlay per
  breakpoint with Bootstrap/Tailwind templates and a baseline grid, the same in every view; a
  keyboard dialog; MCP `rulers_set`; `Leader O R` / `O G` / `O C` (RB-060-05).
- Host services for plugins: views, evaluate, CDP, HTTP with an SSRF guard, project and output
  files, settings, findings and sidecar; user and project plugins; the console-logs feed; plugin
  capabilities in the trust dialog (RB-060-12, RB-010-09).
- Plugin `overlays` 0.2.0: stacking contexts, unneeded scrollbars, WCAG 1.4.12 text spacing, target
  size, small text, focus tracking, forced `:hover`/`:focus` (CDP), content chaos, placeholdifier,
  CSS/JS/images off, each per view or for all; `overlay_set` answers hits and reports findings
  (RB-070-07).
- Plugin `outline`: headings, landmarks, links with a broken-link check, images, forms, focus order,
  accessibility tree and custom selectors of every view side by side, marking what only some views
  show; a click marks the entry in all views; findings for `checks_run` (shared ids with
  seo/a11y-axe); MCP `outline_get`, `outline_check_links`, `outline_mark`; `Leader P O` / `K O`;
  settings in DevTools & Debugging › Outline (RB-070-03).
- Plugin `snippets`: JavaScript snippets of the person and the project run in every view or the
  active one, results joined per view; panel `Leader P J`, repeat `Leader D J`; MCP `snippets_list`,
  `snippets_run`, `snippets_save` (the latter two only with `mcp.allowEvaluate`) (RB-070-04).
- SEO plugin: hreflang set check (`seo_check` scope `hreflang`, `Leader K H`) from head, Link header
  and sitemap: codes with corrections, x-default, self reference, return links as a matrix in the
  report, targets and language; parity of the views (`parity: true`, `Leader K P`); settings
  `seo.locales`, `seo.requireXDefault`, `seo.hreflang.checkContentLanguage`, `seo.parity.reference`,
  `seo.parity.textSimilarity` (RB-060-09).
- JSON/XML viewer plugin `json-xml-viewer`: JSON and XML documents (sitemaps, RSS, Atom) as a
  keyboard-operable tree in a closed shadow root with virtual rows, a text/JSONPath/XPath filter,
  copy value and path, raw and formatted text (`Leader V J`), the position of a JSON error, and a
  size limit with a notice; settings under DevTools & Debugging › JSON/XML viewer (RB-070-05).
- Plugin `lighthouse`: Lighthouse 13.6.0 in the Node sidecar for every Chromium breakpoint with its
  emulation, in a headless browser of its own; HTML/JSON reports in `.rbrowser/out/lighthouse/`,
  failed audits as findings per breakpoint, score panel with the change since the last run; MCP
  `lighthouse_run`/`_status`/`_cancel`, `Leader K L` (RB-060-07).
- A plugin command's answer `{"$open": {"panel": "<id>"}}` opens the plugin panels window at one of
  that plugin's own panels; an open panels window now also switches to the panel `plugins.panels`
  names (RB-060-12).
- Storage panel plugin `storage-panel`: cookies (HttpOnly included), localStorage and sessionStorage
  of every view side by side, grouped by session; differing and missing entries marked; edit, add,
  delete in one view or all; pinned values set again after every navigation (storage before the
  page's scripts); MCP `storage_get`/`storage_set` (values only with the protected
  `storage-panel.mcpValues`); panel `Leader P A` (RB-070-01).
- Network panel plugin `network-panel`: the requests of every view in one list with partial and
  differing requests marked, mocks, blocking and rewrites from `.rbrowser/mocks/*.json` (workspace
  trust) and the block list, applied by the app through CDP Fetch (capability `network-rules`), HAR
  1.2 export and import as replay, per-session cache switch and clear site data; MCP
  `networkpanel_*`; `Leader P N` / `P M` / `P H` / `R N` / `R D` (RB-070-02).
- Plugin `simulators`: fifteen simulations of colour vision, vision impairments, sunlight and a
  dimmed screen, chosen per view; Chromium's own emulation for six types, the page script for the
  rest (fixed headers stay put, clicks pass through), kept across reloads; picker `Leader O I`
  (Shift+click for every view); MCP `simulator_set`; settings under DevTools & Debugging ›
  Simulators (RB-070-08).
- Plugin `image-overlay`: each view's design from `.rbrowser/designs/` over the page, mapped per
  breakpoint or by nearest width, `@2x`/`@3x` exports scaled; opacity, blend modes, invert, offset
  (arrow keys), pinned to the page or fixed, and a split view synced across views; sidebar panel,
  MCP `imageoverlay_set`, `Leader O D` (RB-070-09).
- Plugin `color-picker`: a pipette on the rendered pixels of every view (DPR-aware, 11 x 11 loupe,
  keyboard, `Leader D I`), WCAG contrast with AA/AAA, values as HEX/rgb()/hsl()/oklch(), the page's
  palette by use, and characters per line marked above 80 (`Leader D M`); panel `Leader P I`; MCP
  `color_sample`, `color_contrast`, `color_palette`, `color_measure_text` (RB-070-10).
- Plugin `form-autofill`: fills the forms of every view from one seed with valid or deliberately
  invalid values (mixed, empty, format, bounds, length, chars), matched to each view's locale, with
  profiles from the settings; never submits, skips file inputs; `Leader D F` / `Leader D N`, panel
  `autofill`, MCP `autofill_fill` (RB-070-11).
- Tab order overlay in the `overlays` plugin (`Leader O T`): numbered stops joined by lines, jumps
  against the reading order and focusable-but-unseen stops marked, live focus across views, views
  compared in the panel; MCP `overlay_tab_order`, findings
  `tab-positive-tabindex`/`tab-invisible-focusable` (RB-070-12).
- Plugin `fonts`: font inventory per view (source, formats, faces, file sizes, use), declared vs
  rendered fonts with fallback glyphs (Chromium), font sizes across breakpoints,
  privacy/performance/readability findings for `checks_run`, panel "Fonts" with marks in every view,
  MCP `fonts_list`/`fonts_check`/`fonts_mark`/`fonts_export`, JSON/CSV export, `Leader P Y`, `Leader
  K F`; Peek notes a font fallback (RB-070-13).
- Plugin `screenshot-editor`: annotate, redact (pixelate, blur, black) and crop screenshots of
  `.rbrowser/out/` in a dialog panel (`Leader C E`, copy `Leader C K`); results and annotations go
  to `.rbrowser/out/screenshot-editor/` without metadata; MCP `shot_annotate`, `shot_load`,
  `shot_list` (RB-080-01).
- Video recording of one, several or all Chromium views (MP4 with `h264_mf` or WebM VP9) with an
  LGPL ffmpeg downloaded only on request, pinned and SHA-256 checked; `Leader C V` / `Leader C W`,
  ffmpeg install and removal `Leader X F` / `Leader X G`, MCP `record_video`; settings Capture ›
  Video (RB-080-02).
- Environment comparison: the same page in several environments side by side and synced (`Leader E
  C`), a pixel-diff run over environments × breakpoints × paths in hidden views with an offline
  report (`Leader E R`, `Leader E O`); MCP `env_compare`, `env_compare_result`, `env_compare_live`;
  settings Capture › Environment compare (RB-080-03).
- Layout modes horizontal, split and canvas (`Leader L H/S/C`, back to the grid with `Leader L G`):
  switching keeps every page, the split ratio and canvas positions are saved in the preset, the
  toolbar has a split slider and a canvas minimap; MCP `layout_set { mode }` and `canvas_place`;
  settings Layout › Further modes (RB-080-04).
- Plugin `coverage`: unused CSS and JS per breakpoint from CSS rule usage and V8 precise coverage in
  every Chromium view (on page load or between start and stop); findings `unused-file`,
  `rule-partial`, `function-unused`; panel with table, rules and marked source; JSON/CSV export; MCP
  `coverage_get`, `coverage_start`, `coverage_stop`, `coverage_file`; `Leader K C/I/X/J`
  (RB-080-07).
- Plugin `pwa-check`: the web app manifest, its icons, the start URL, the service worker (fetch
  handler, offline cache probe) and Chromium's installability errors as findings; panel "PWA", MCP
  `pwa_check` and `pwa_icon`, `Leader K M` and `Leader P W` (RB-080-08).
- Plugin `error-report`: JS exceptions, 404/410 and 5xx, mixed content, CSP violations and
  deprecations of every Chromium view, deduplicated over the views with view dots and "only in
  views"; panel `Leader P E`, report `Leader K E` (`report.json` and `report.html` in
  `.rbrowser/out/error-report/`, plus findings), clear `Leader K Z`; MCP `errorreport_get`,
  `errorreport_clear` (RB-080-09).
- Portal (RB-080-05): phones and tablets in the LAN join the sync after scanning a one-time QR code;
  off by default, protected from MCP, one LAN address, a device list with disconnect (`Leader S P`,
  `S Q`, `S D`, panel `Leader P D`); MCP `portal_start`, `portal_stop`, `portal_devices`,
  `portal_disconnect`, none of which returns a pairing token.
- Chrome extensions from unpacked Manifest V3 folders in the Chromium views
  (`engines.chromium.extensions.*`, loaded at the next start; the `debugger` permission is refused);
  a trusted project's recommended folders are offered in a notice and added only by the person
  (`Leader X O`, `X A`, `X M`); MCP `extension_list` (read-only); extension targets stay hidden from
  the CDP proxy; no Web Store installs (RB-080-06).
- SEO plugin: social previews for Facebook, X, LinkedIn, Slack, WhatsApp and Discord in a new SEO
  panel (`Leader K G`, MCP `seo_social_preview`); checks for the share image, Twitter card and
  `og:url`; `llms.txt` and `security.txt` (`Leader K W`, `seo_check` with `groups`); Microdata/RDFa;
  head order (RB-080-11).
- Screenshots without noise (RB-080-12): before every capture (screenshots, overview, visual
  regression, sitemap capture, environment comparison) cookie banners and chat widgets are hidden (a
  built-in list plus `capture.hideSelectors`) and animations, transitions, the caret and videos are
  frozen; all of it is undone afterwards; keys `capture.hideBanners`, `useBuiltinList`,
  `freezeAnimations`, `settleMs`; per-call `hideBanners`/`freezeAnimations` on the capture tools;
  `Leader C H`, `C N`, `C P`.
- Settings: every section is a card with one header; Layout and Breakpoints & Devices get sub-tabs
  (the tab in the address); breakpoints, device presets, grid cells, environments and layout presets
  are one-line entries that open to edit, with add, duplicate, delete, move and drag; a grid
  preview; search hits open their tab (RB-080-13).
- Pop a view out of the grid into a window of its own at its real size (zoom 1, still synced, the
  same view id); closing it puts the view back; `Leader V X` / `Leader V V`, the cell's context
  menu, MCP `view_pop_out` / `view_pop_in`, Settings › Layout › Pop-out (RB-080-14).
- The Windows package ships the static WSL helper `rbrowser-wsl` beside `rbrowser.exe`, so hot
  reload of WSL projects uses inotify instead of polling (RB-030-04).
- The wiki's keyboard and MCP references list the bundled plugins' commands and tools.
- A status bar at the bottom of the window takes every message, with level icon and word, time,
  settings link and close; errors and warnings stay until closed, info fades; its history (`Leader P
  M`) filters and copies; `Leader P S` shows or hides the bar; Settings › Layout › Status bar; MCP
  `status_messages`. The toolbar no longer shows messages. (RB-080-15)
- Canvas mode: the board's labels show the view's size as the other modes do, and a click on the
  active view's label opens the size quick pick (breakpoints, device presets, free size)
  (RB-080-22).
- Settings: every path has a file or folder dialog (`x-input`), the Lighthouse categories are a
  checkbox choice, platforms, locales, time zones and axe tags are suggested (`x-options`), and
  every setting has a help text with its default (tooltip and search); the settings lint requires it
  (RB-080-20).
- `devtools.screencast` (DevTools & Debugging > DevTools mode, off): shows Chrome's screencast
  beside the panels in docked DevTools; changing it reopens the dock (RB-080-21).
- Chrome Web Store extensions (RB-080-23): an id or store URL in Settings > Engines > Chromium/CEF >
  From the Chrome Web Store (or `Leader X W`) downloads the extension from Google's update server,
  checks its CRX3 signatures (developer key = id, Web Store publisher key), unpacks it under
  `%LOCALAPPDATA%\rBrowser\extensions` and loads it at the next start; a trusted project's
  recommended ids install with `Leader X A`; `Leader X C` and
  `engines.chromium.extensions.checkUpdatesAtStart` (off) look for updates, each installed only
  after confirmation in the window; MCP `extension_install` installs only after that confirmation,
  `extension_list` names what is installed.
- Vue and React DevTools as panels for the active view, offered when a page runs the framework;
  backends (react-devtools-inline 8.0.0, @vue/devtools-kit 8.2.1) ride in the page without a port;
  Settings > DevTools & Debugging > Framework DevTools `frameworkDevtools.vue`/`.react`
  (auto|on|off); `Leader D U`/`Leader D A`; plugin `framework-devtools` (RB-080-24).

### Changed

- Dependencies current for 0.7.0: Rust 1.100.0 (`wasm32-unknown-unknown` now in
  `rust-toolchain.toml`), Node 26.13.1 (the packaged runtime too), rmcp 3.10, zip 9.5.1 (ureq held
  at 3.5.1: 3.5.2 caps a JSON body at 10 MiB, which breaks cef-dll-sys's build script), wat/wit-component/wit-parser 0.263, the npm minors; typescript 7 and
  birpc 4 (majors) wait for a job of their own.
- ffmpeg for video recording is downloaded straight from BtbN's month-end build of October 2026
  (kept for two years, as rDownloader does) instead of a package-registry mirror that was never
  filled; a test holds every BtbN pin to a month-end release (RB-080-02).
- MCP tools of the sync chain registered (RB-030-01 with RB-020-02/03/04/06/07/08, RB-050-03,
  RB-060-01): `sync_set`, `click`, `type`, `hover`, `press_key`, `scroll`, `bus_send`,
  `screenshot`, `checks_run`, `session_assign` and `session_clear`; `views_list` adds each
  view's `desync` and `syncNavigation`, `project_info` the hot reload `watcher`, and `reload`
  takes `mode: "css"`. A breakpoint id shown by several views runs the tool once per view.
  Explicit navigation and a full `reload.all` reach the views whose navigation is coupled;
  synced submits follow `sync.leaderPerSession` through the SyncHub's submit policy, and
  captured input holds back the interval reload.
- The cell label shows the size with a multiplication sign and the pixel ratio when it is not 1,
  as in `Mobile  393 × 852 @3x` (RB-010-07).
- Panning the grid with Space + drag moves to the canvas layout (RB-080-04): the command
  `view.pan` and the setting `grid.spacePan` are gone from 0.1; Layout › Zoom gains
  `grid.defaultZoom`, the zoom of the cells rBrowser lays out itself (RB-010-12).

- The `cdp_endpoint` snippet and the example connect with `noDefaults: true`, and the CDP proxy
  keeps `Browser.setDownloadBehavior` for rBrowser (RB-040-06).
- `rbrowser mcp` keeps running without an instance - `initialize` succeeds and every call says how
  to start rBrowser - and finds a restarted rBrowser at the next call. It picks the project by
  `--project`, then the working folder, then the only running instance, and lists several instances;
  `mcp.stdioAutoStart` starts rBrowser in the background (RB-030-02).
- Plugins read `.rbrowser/{flows,snippets,plugins,mocks,scripts}/` only in a trusted workspace
  (`plugins.error.untrusted_workspace`) (RB-070-04, RB-010-09).
- The trust dialog flags the `network-rules` and `console-logs` capabilities as risky (Charge 7
  review L3).
- The Windows package ships the Lighthouse sidecar's dependencies (Lighthouse 13.6.0, puppeteer-core
  25.17.0), without Sentry (RB-060-07).
- The plugins' `$console` feed and MCP's log collector carry the views' Audits issues (CSP, mixed
  content, deprecations; URLs masked) (RB-080-09).
- The plugin CDP allowlist gains the CSS coverage methods, `Profiler.*PreciseCoverage`,
  `Page.getResourceContent` (frame id and http(s) URL only) and `Page.getInstallabilityErrors`
  (RB-080-07, RB-080-08).
- `sitemap.hideSelectors` is deprecated: it defaults to empty and its entries join the capture-wide
  list (RB-080-12).
- The dead settings `checks.axe` (its tags still count as `a11y-axe.tags`) and
  `json-xml-viewer.inFirefox` are gone.
- The Which-Key overlay stays until Esc or the next key; `shortcuts.leaderTimeoutMs` 0 (new default)
  means no timeout (RB-080-18).
- Build workflow (RB-080-25): the plugin crates build and test in one target lane
  `target/lanes/plugins`; the plugins' page scripts, panels and runners are one npm workspace
  `plugins/` (one lock file, one `npm ci`); `check.sh` runs `npm ci` when a lock file is newer than
  the install; the package's `sidecar/node_modules` ships without source maps, type declarations,
  `*.md` and `*.txt` (licences and notices stay); the release profile stays on thin LTO (fat LTO alone
  measured no smaller; the 30 MB came with `panic = "abort"`, which stays off); three unused
  dependencies removed.
- Tests assert no upper bound on the wall clock any more (poll deadlines, paused tokio clock, call
  order instead); a repo lint `no_wall_clock_upper_bound` holds the convention, `// wall-clock:
  allow (<reason>)` marks a deliberate measurement (RB-080-26).
- Sync hub: a routed event is cloned once for all target views (`Arc<SyncEvent>` in
  `ReplayRequest`), bus payloads are serialised once and passed on as bytes, the agent reads an
  event's composed path once and skips the ancestor walk for a pointermove over the same element,
  and a setting written from the settings window no longer touches the file on the UI thread
  (RB-080-27).
- One home for the duplicated helpers (RB-080-28): the new crate `rb-util` (`hex`, poison-tolerant
  `lock`, `url::{authority_of, host_of, origin_of}`, `json_pointer::escape`), one HTML escape
  `rb_plugin_types::html::escape` used by every report, gallery and placeholder page (the portal
  page and the shell placeholder now escape `'` too), `rb-mcp` shares `rb-cdp`'s endpoint secrets,
  and `plugins/web-common` holds the plugins' web-test loader, fake bridge and the one `isObject`.

### Fixed

- Portal: a page that writes its own origin into links and asset URLs (`APP_URL`,
  `http://localhost:3000/...`) now reaches the device with those URLs pointed at the portal, so
  styles, scripts and links work on the phone and a synced click no longer sends it to
  `localhost` (RB-080-05).
- The settings window no longer scrolls its root out of view when a field is brought into view
  (a blank band below the content with nothing to scroll back) (RB-080-20).
- A list of checkboxes names a value the schema does not know - an empty string included - as
  such instead of an unlabelled box, so it can be unticked (RB-080-20).
- A plugin call from a thread without the core's runtime no longer builds a runtime and blocks (a
  deadlock risk on the UI thread); it fails with `plugins.error.host_failed`, and the check,
  analyser, plugin-load and export threads run on the core's runtime (RB-080-28).
- The Framework DevTools panel bounds a probe report's version string like the sync agent
  (RB-080-24).
- The wall-clock lint also finds inline spans (`Instant::now() - start`, `.as_millis() <`) and
  wrapped `let` bindings (RB-080-26).
- Docked DevTools: a setting toggled while the dock's marker page still loads no longer adopts the
  stale session - the new dock is the one registered as hidden and scripted (RB-080-21).
- `rbrowser init` enters a runtime of its own for the analyser's plugins (a plugin call on a thread
  without one is an error since RB-080-28); the portal's allowlist leaves out an environment URL
  written with credentials (RB-080-28).
- Settings: one change at a time for every writer of the user file, and the shell's `setSetting`
  answers an internal failure when its write panics instead of waiting for the IPC timeout
  (RB-080-27).
- A plugin call from a thread without the core's runtime no longer builds a runtime and blocks (a
  deadlock risk on the UI thread); it fails with `plugins.error.host_failed`, and the check,
  analyser, plugin-load and export threads run on the core's runtime (RB-080-28).
- Docked DevTools (RB-080-21): the hosted frontend's screencast is off by default (the dock's
  session gets `Page.enable` before the setting script, so it takes), the panels use the whole dock
  and inspect mode highlights and picks in the view itself; the DevTools Lighthouse panel runs - the
  relay admits its parallel session on the frontend's own target (`Target.attachToTarget`,
  `Target.setDiscoverTargets`) and nothing beyond it.
- Settings: the Browse button no longer crashes (CEF's file dialog got a null filter list) and the
  chosen path is taken over; the dialog answers exactly once (RB-080-19).
- Toolbar lists (RB-080-16): a bookmark just set is in the Bookmarks menu at once - the config is
  read again right after the write (the watcher hears nothing over a WSL share); the layout-preset
  and environment lists name the entry in effect in a placeholder ("No preset" / "No environment"
  without one) and apply any entry chosen, the shown one, the first or the only one included.
- Focus mode (RB-080-17): after a click into the toolbar every arrow key steps the breakpoint - the
  page loading in the new view no longer takes the keyboard focus; `focusMode.keepFocus` (Layout >
  Focus mode, on by default) switches that off.
- `Leader D C`/`D R` name their new state in the status bar and keep the last readings; a plugin
  command without a panel reports its end, a check command opens the findings window, so `Leader K
  S` shows its result (RB-080-18).
- Portal pairing from a phone's QR scanner app (RB-080-05): `/__rbrowser/pair` now answers `200`
  with a page that goes on to `/` itself (`location.replace`, a refresh without script) instead of a
  `303` - the scanner's navigation is cross-site, so the `SameSite=Strict` device cookie did not
  follow the redirect and the phone saw "not paired" although rBrowser listed it. A device that
  never came back with its cookie is listed as "pairing not completed" and drops out of the list
  after the pairing code lifetime.
- Dragging a view on the canvas board moved it a few pixels and put it back: each live move
  showed the view's overlay again, the window activated it and took the focus from the board,
  and the board cancelled the drag on losing the focus. An overlay is shown only when it was
  hidden, losing the focus cancels no drag, and a move with no button held ends it (RB-080-04).
- The message history's shortcut is `Leader P L` (was `P M`, which took the network panel's
  mocks switch and pushed it to `Leader X B` with a warning at every start); the shortcut lint
  now checks every core command of the app, not only the built-in ones, against the bundled
  plugins (RB-080-15).
- Status bar: `status_messages` masks the registered secrets (CDP, MCP, portal tokens) in each text,
  the core's own messages are cut like the shell's reports, status events reach only the history
  window, and the history size is held to 10..1000 in the UI too (Charge 10 review S1-S4; RB-080-15).
- The which-key bar after `Ctrl+Shift+Space` stays 4 seconds by default (was 1.5 s, too short to read);
  `shortcuts.leaderTimeoutMs` still sets it (RB-010-17).
- Portal: disconnecting a device or stopping the portal now also ends its live-reload (Vite HMR),
  event-stream and long-poll connections; at most 16 connections per address and 8 agent
  sockets per device; a slow HTML page times out (Charge 10 review M1, L2; RB-080-05).
- Portal: with an empty `portal.interface` it starts only on a private LAN address (10.x,
  172.16-31.x, 192.168.x, 169.254.x, 100.64/10); a public or VPN address asks for the setting
  (Charge 10 review M2; RB-080-05).
- MCP can no longer start the portal (`portal_start` removed; a LAN listener is a person's
  decision); stop, devices and disconnect remain (Charge 10 review L1; RB-080-05).
- Portal: any `portal.*` change stops a running portal, so a lower device limit or pairing time
  applies at once; replaced tokens and revoked sessions leave the log mask (Charge 10 review L3,
  L5; RB-080-05).
- Pop-out windows closed at once all keep their bounds, and the bounds file is no longer read on
  the UI thread (Charge 10 review L6; RB-080-14).
- A capture cancelled midway still puts the page back: animations, motion and banners (Charge 10
  review L7; RB-080-12).
- Focus mode no longer quits rBrowser, and closing docked DevTools cannot either: views and the
  dock inside the main window answer CEF's `DoClose` themselves and a dropped view is released,
  not closed through the main window (owner, build 22481e2; RB-010-12).
- After moving the split divider every layout control works as before: the divider, the canvas
  positions and the strip's parameters of the top-level layout are kept in `layout.modes`
  (`config.local.json`) instead of presets named `split`/`canvas` that became current; focus mode
  off returns to the mode it came from, and the divider moves the views without a rebuild
  (RB-080-04).
- The toolbar hides the layout preset select when the project has no presets (RB-080-04).
- Focus mode's toolbar icon no longer looks like zoom to fit's (RB-010-12).
- A click into a view made it the active view in a loop: the focus the host queued for the new
  leader reached the window after the next view's and took the focus back, so the views took the
  lead from each other about 9000 times a second - the canvas labels flashed, sync stuttered and
  the window froze. The view that gets the focus is no longer focused again, a queued focus for a
  view that is no longer the leader is dropped, and only a click on a view's label makes it the
  leader again unless `sync.activateOnFocus` is on (RB-020-02).

- The leader chord works with the focus outside every browser (the window, a panel, an overlay
  without a page): the main window takes a key no browser handled as the shell's (RB-010-17).
- ffmpeg download: a 60 s idle timeout per read instead of a 60 s total budget, so slow lines no
  longer fail; installing the installed version does nothing, one install runs across processes,
  stale staging is cleaned and a version is activated by rename (Charge 9 review H1, L2; RB-080-02).
- A hung ffmpeg no longer blocks recording (write and stop timeouts), and the video folder never
  follows a link or junction (Charge 9 review M1, L1; RB-080-02).
- Environment comparison runs can be cancelled (`Leader E X`, MCP `env_compare_cancel`), and its
  report is read off the UI thread (Charge 9 review M2, L6; RB-080-03).
- Screenshot editor: an image's chunks are cut once instead of re-read per chunk, mosaic blocks are
  at least 8 px, and oversized edits are compressed harder or refused with a clear error (Charge 9
  review M3, L3, L4; RB-080-01).
- `console.clearOnNavigate` works (the `$console` feed carries `navigatedAt`); plugin panels can
  copy to the clipboard; a disabled or untrusted plugin's colour-vision and JS-off emulation is
  undone; the form-autofill seed setting takes the tool's range.
- `wsl.restart_helper` (`Leader R W`) restarts the WSL helper, also after polling took over
  (RB-030-04).
- The pop-out window can be resized, maximised and minimised like the main window.
- The main window, the settings window and the page windows can be resized, maximised and minimised:
  the cef crate answers 0 where CEF's own defaults are true; a repository lint holds every window
  delegate to it (owner report on ed2fd51).
- The toolbar strip (and with it the which-key hint bar) has its height again: CEF took the strip's
  size `{0, 44}` for empty and laid the shell out at height 0; the docked DevTools, their splitter
  and the cell labels had the same flaw (owner acceptance 0.1).
- Fonts never collects what was typed into a form field: placeholder or nothing, and no text at all
  for password, payment or one-time-code fields (Charge 8 review M2, RB-070-13).
- A plugin's `Page.captureScreenshot` is held to a small clip of the viewport (at most 1 Mpx in CSS
  px, scale at most 4, no `captureBeyondViewport`) (Charge 8 review M1).
- Image overlay reads and writes its state file only in a trusted workspace, writes a design into
  chunks once per version instead of reading the whole file per chunk, and caps designs at 16
  megapixels (Charge 8 review L1, M3).
- Network rules: a restart whose `Fetch.enable` fails no longer reports the view as intercepting
  (Charge 8 review L2); sidecar params from a plugin that are not a JSON object are refused (L4).
- Network rules of a project plugin follow workspace trust, and a project plugin can no longer label
  its rules as settings rules (Charge 7 review M2).
- Storage-panel pins are read and applied only in a trusted workspace (Charge 7 review M3).
- Network rules never leave a request paused: an event channel of their own, recovery after lost
  events, fallback answers and `Fetch.disable` on shutdown (Charge 7 review M1).
- Outline and overlays no longer re-request the DOM document on the shared CDP session; Lighthouse
  jobs belong to the calling plugin (Charge 7 review L2, L4).
- The rulers' page host is a custom element, so sync no longer replays guide drags (RB-060-05).
- Plugins' `cdp` is limited to a method allowlist: own isolated world or main world only, the view's
  own cookies, no file, download, `*.disable`, `Security`, `Fetch`, `Target`, `Browser` or `Input`;
  `project-read` hides `config.local.json` and the trust-protected folders under any case and
  refuses Windows short names; further IPv6 special-purpose ranges are not public for `http`;
  sidecar namespaces for plugins are an allowlist; peek no longer switches CDP domains on or drops
  other users' node ids (Charge 6 review, RB-060-12, RB-060-04).
- Project plugins stop entirely (commands, MCP tools, panels, checks, services) the moment workspace
  trust is revoked or goes stale, and a project's own origins pass the plugin HTTP guard only once
  it is trusted; the trust dialog shows each project plugin's CDP domains, globs, sidecar methods,
  HTTP rate and evaluate, risky ones highlighted; console, error and request logs (`$console`, MCP
  `console_logs`) mask OAuth codes, signatures and tokens; plugin settings honour project and local
  scope (Charge 6 review, RB-060-12, RB-010-09).
- The sync agent no longer captures clicks and keys inside a plugin's own UI (its `rbrowser-*`
  host), so plugin panels drawn in a page are not replayed in the other views (RB-060-12).
- Charge 5 review: `settings_get` masks header values (RB-030-03); `rbrowser mcp` announces
  `tools/list_changed` and negotiates the protocol version offline (RB-030-02); an MCP shortcut
  batch is all or nothing (RB-030-03); the sitemap render views are hidden from CDP clients from the
  start (RB-050-05); the memory sample walks the process tree once and skips reused parent ids
  (RB-020-09).
- Plugin shortcut notice (RB-010-17); the bundled-plugin shortcut lint now reads manifest.toml; the
  plugin HTTP transport resolves names itself and refuses private addresses, settings changes no
  longer hash the project on the UI thread, page and panel answers are capped at 64 KiB, and a page
  script switched off and on keeps its contexts (Charge 5 review, RB-060-12, RB-050-05).
- The interval reload of an environment is `environments[].reloadIntervalSec` as documented; the old
  `reload_interval_sec` is still read (RB-030-05).
- MCP serves `flow_list`/`flow_run` in the coverage reference and adds `engine_restart`,
  `recorder_start`, `recorder_stop`, `recorder_export`, `visual_compare` and
  `visual_baseline_update`, which charge-4 commands named but no server registered (RB-030-01,
  RB-040-01, RB-050-01, RB-050-02, RB-050-04).
- `config_set` refuses the recorder's folders and secret field list (RB-050-01, RB-050-02).
- WebKit views on the display's pixel ratio open again; time zone and position apply to WebKit, and
  views list what an engine does not emulate (RB-040-04).
- Quitting closes the WebKit windows before the sidecar stops, and the sidecar gets 3 s to end
  before it is killed (RB-040-03, RB-040-04).
- A view whose breakpoint changes engine moves to the new engine; closed external views no longer
  leak a task (RB-040-01).
- Only a WebKit build installed from the pinned archive counts as installed (RB-040-04).
- Recorder: a password field's keys are never steps (named keys are kept in clear), tokens in URLs
  are recorded masked, a start with sync off or the view excluded is refused (`notCaptured` names
  switched-off kinds), a failed save no longer loses the recording, `target` is applied only after
  the checks, Windows device names are refused, and a stale replay outcome no longer drops a view
  from `onlyIn` (RB-050-01).
- `recorder-export`: only a fill the recorder marked secret reads `RB_SECRET_<NAME>`, and the code
  stops with the variable's name when it is missing (RB-050-02).
- `startUrl` and environment URLs must be http(s); bookmarks resolve and environments switch only
  to http(s) URLs, and `rbrowser init` refuses a `file:` URL (RB-010-01, RB-010-12, RB-070-06).
- MCP can no longer change the recorder's secret fields (RB-050-01).
- The geolocation grant is scoped to the view's browser context and retried when refused (RB-080-10).
- Packaging no longer passes signing secrets to build steps (RB-090-03).
- Only the switched program proves an update (RB-090-02).
- The analyser's static source reads files through one handle, within its limits (RB-030-07).
- Baselines for page slugs of 65 to 80 characters and for breakpoint ids that are not file names;
  Windows device names are refused; machine and OS are recorded, not compared; sizes come from the
  PNG headers before any decode; the LFS line covers only the PNGs, and nested `.gitattributes`
  count (RB-050-04).
- Sitemap discovery follows redirects within the origin only, keeps only the site's own pages and
  reads no local or private sitemap for a public site; the environment rewrite leaves no page on
  production; a cancelled batch leaves no hidden render view open (RB-050-05).
- Firefox: the profile lives in rBrowser's local folder per project, never in the project tree;
  links are refused; Firefox ends with the app on Windows (RB-040-02).
- Firefox follows emulation and workspace-trust changes, the sync switches and modifier keys, and
  replays through the agent's commands (RB-040-02).
- The Windows package ships the flow worker and `@playwright/test`. A flow run takes its views out
  of sync and gives them back however it ends. `flows.cancel` also stops `flows.run_all`. `rbrowser
  flow run` ends at once when a run cannot start. A stopped flow worker keeps the views it finished.
  A flow's page is always its own view, never a popup or a new view (RB-040-07).
- Owner acceptance of the 0.1 test package (4a3d56f): the shell view keeps its 44 DIP in the
  window's layout (its height for the width and its minimum, not only its preferred size), and
  its console and a failed load reach `rbrowser.log`; a start CEF refuses (as administrator,
  the profile still in use, the sandbox) shows the reason in a message box and the log and
  exits with CEF's code instead of silently; a watch error that polling repeats on every pass
  (a dangling symlink in a WSL project) is logged and rescanned once, not every two seconds;
  the main, pages and page windows carry the rBrowser icon instead of Chromium's.
- Batch-3 review (RB-020-02, RB-020-03, RB-020-04, RB-020-06, RB-020-08, RB-030-01): a view
  the hub gave a URL (resync, follow, navigation sync) takes events again once that load ends -
  also when it reloads the URL it shows, never starts or fails to start - instead of never; a
  form posted to its own page has the followers load it; a late replay outcome no longer
  desyncs a view that navigated meanwhile, and a view that became the leader gets no queued
  replay. An agent configuration that did not reach the page is sent again, and a new
  document's agent ends with the newest one. `reload.all` reaches every view (decoupled ones
  reload their own page) and takes the address field's `view`; `navigate`, `back` and
  `forward` with `"all"` name the decoupled views they skip, go on past a view that fails and
  answer `{ views, skipped, failed }`, and the MCP tools wait only on the views reached. MCP's
  `click`, `type`, `hover` and `press_key` give each view `mcp.navigationTimeoutMs` and say on
  a timeout that input may have gone out; `sync_set` answers once the agents are configured
  and resynced views have loaded. A page that does not answer no longer holds up the page
  API's `ready` and bus messages for the others.

- Batch-2 review (RB-060-01, RB-060-11, RB-020-07, RB-030-05, RB-050-03): the findings panel
  opens in a window of its own (`panels.findings` opens or focuses it; it shows the last run
  and names the rules and ignores the project turned off), and the command palette in a
  frameless popup right below the toolbar that closes on Escape, on run and when it loses the
  focus, the focus going back to the active view - the shell strip keeps its 44 px; the session
  bar gets the keyboard focus. Moving views into a session is atomic and leaves no sync-agent
  session behind. Hot reload restarts its watcher only when its own settings change, drops a
  stale polling source, swaps stylesheets in an isolated world and never waits on the window's
  thread; the watcher never walks a folder link made at runtime. Captures run one at a time,
  read the views after the delay and always restore the metrics. Session cookies end with the
  app unless `sessions.persistSessionCookies` is on. `capture.dir` is protected from MCP, a
  user folder may not be a network share or device path, and a project folder may not lead
  through a link or into `.git/` or `.rbrowser/` (but `.rbrowser/out/`).

- `rbrowser init` and every other command now answer in PowerShell and cmd (RB-010-11):
  `rbrowser.exe` is a GUI program the shell neither waits for nor shows the output of, so the
  package ships a console twin, `rbrowser.com`, which a bare `rbrowser` finds first. It hands
  `rbrowser.exe` the terminal, shows its output and ends with its exit code; a call that opens the
  window frees the terminal at once and the app runs on. Started straight from a shell,
  `rbrowser.exe` writes to that console too. Without `--yes`, `init` on a terminal lists the
  proposed views (id, width x height, analyser or default) and asks `Write
  .rbrowser/config.json? [y/N]`.
- `package-windows.sh` and the npm stages of `check.sh` build and test the front ends with the
  Node `.nvmrc` pins, not whatever `node` is first on PATH: another version is switched to the pin
  with fnm or nvm, and without it the package stops (the check's npm stage fails) with the install
  command (RB-010-15).
- MCP hardening (batch-3 security review): `settings_set` refuses everything `config_set`
  refuses, from one shared list, and the network keys, request headers, breakpoints, extensions,
  the screenshot folder, the recent projects and `logs.level` are protected from MCP; rmcp no
  longer logs whole tool calls and answers at debug (held at warn unless `RUST_LOG` names it);
  `navigate` over MCP loads only http(s) and files inside the open project; `screenshot` with
  `save: false` keeps all inline PNGs together within `mcp.maxAnswerBytes`; `rbrowser mcp`
  answers a request whose answer is too large or cut off with a JSON-RPC error instead of
  dropping it. The threat model's T8 now says what leader-only submit does not cover.
- The app no longer closes right after its window opens: DevTools started with `Handle::current()`
  on CEF's UI thread, which is outside the Tokio runtime, and panicked. Every feature now starts
  from the core's runtime handle; a test starts all features outside the runtime (RB-010-06).
- `run-windows.ps1 -Package` unpacks a renamed package ZIP: the folder it empties and starts is the
  archive's own top folder, not the ZIP's file name, so an older unpacked package of the same
  version no longer stops the unpacking (RB-010-15).
- Shell UI (RB-010-07): exact npm overrides for esbuild 0.28.2, glob 14.0.4 and minimatch 11.0.5
  clear the six dev-only `npm audit` advisories in `ui/`.
- `diagnostics.create` from the shell or MCP writes only into the diagnostics folder (`out` is a
  file name ending in `.zip`), and no diagnostics ZIP, `--out` included, overwrites an existing
  file (RB-010-10).
- Loading a project no longer rewrites the checked-in `.rbrowser/config.json` (or leaves a
  backup) when its `version` is older: the migration happens in memory; a save writes the new
  version and backs the old file up as `*.bak-v<n>`, which the `.rbrowser/.gitignore` rBrowser
  writes now ignores (RB-010-01).
- A project folder can no longer switch off certificate checks, set a proxy or rewrite host
  resolution for the grid: `network.ignoreCertificateErrors`, `network.proxy` and
  `network.hostResolverRules` are ignored in `config.json` and in a committed
  `config.local.json` alike, with a warning per key, unless the workspace is trusted (RB-010-09)
  (RB-010-01).
- A user setting outside its schema range or choices (`shortcuts.leaderTimeoutMs: 0`,
  `logs.crashDumpsInBundle: 99`, an unknown theme) falls back to its default with a warning in
  the log naming the key; the rest of `settings.json` still applies (RB-010-01).
- A command run from the shell (`runCommand`) no longer freezes the
  window while it works (`diagnostics.create` writing its ZIP): it runs off the UI thread and
  the shell gets its answer when it is done, and key presses no longer wait for a running
  command (RB-010-17).
- The shell takes its theme from the user settings again: `getSettings` is answered with every
  user setting and its origin instead of an error in the status line (RB-010-07).
- Changing a shortcut no longer drops the `shortcuts.bindings` entries rBrowser did not apply
  (a plugin command not loaded yet, a refused binding), and never overwrites a `settings.json`
  that could not be read: the change then holds for the session, with a notice (RB-010-17).
- A popup a grid page opens (`window.open`, `target=_blank`) is no longer reported as its
  opener: it opens as a window of its own whose address, title and loads leave the grid view
  alone, and it cannot load a shell page (RB-010-04).
