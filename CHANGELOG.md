# Changelog

All notable changes to rBrowser are documented in this file. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). `scripts/set-version.sh` cuts
`[Unreleased]` into a version section at every version bump.

## [Unreleased]

### Added

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
