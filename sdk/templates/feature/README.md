# rBrowser feature plugin template

A working `feature-plugin` (MIT): the command and MCP tool `count_views` answers how many views the grid has and their widths, and a panel built with Nuxt UI shows them. Copy the folder, rename the package in `Cargo.toml`, the `id`, the
`key_id` and the texts in `manifest.toml` and `locales/en.json` (and the `id` and prefix in
`web/vite.config.ts` and `web/src/panel/main.ts`), then change the logic.

```bash
(cd web && npm ci && npm run build && npm test)       # the panel into panel/dist (Node 26.13.1)
cargo test                                            # the logic, natively
cargo build --release --target wasm32-unknown-unknown # the core module
wasm-tools component new target/wasm32-unknown-unknown/release/rbrowser_plugin_feature_example.wasm \
  -o target/component.wasm                            # the component, no WASI adapter
rb-plugin-pack package . --component target/component.wasm --key <private.pem> \
  --out target/example.rbplugin                       # signed; without --key: development only
rb-plugin-pack conformance target/example.rbplugin --trust <key_id>=<public key>
```

- `src/lib.rs` holds the logic and says what the host promises this kind; `src/lib_tests.rs`
  tests it without a WebAssembly toolchain.
- `src/guest.rs` translates it into `wit/rbrowser-plugin.wit` and exists on `wasm32` only.
- `wit/rbrowser-plugin.wit` is the contract `rbrowser:plugin@0.1.0`, byte for byte.
- The manifest grants `views`; a component that imports anything else is refused when it is
  installed.
- `CHANGES.md` carries one `## <version>` section per release.

**The panel - Nuxt UI** (RB-080-87): `web/` is a Vue 3 app on [Nuxt UI](https://ui.nuxt.com),
built with rBrowser's panel runtime `panel-ui`, the one rBrowser's own panels use. `npm run build`
writes `panel/dist/` - `panel.js` (one classic script: the frame has an opaque origin),
`panel.css` (Tailwind and Nuxt UI compiled, only the classes you name) and
`THIRD-PARTY-LICENSES.txt` (name, version and licence text of every bundled package) - which
`panel/index.html` loads and the package carries; build it before `rb-plugin-pack package`.

- `web/web-common/` is the copy of rBrowser's `plugins/web-common/panel-ui/` and
  `plugins/web-common/panel/texts.js`, under the MIT licence like the rest of the SDK
  (`web/web-common/panel-ui/LICENSE`). Replace it with a newer SDK's copy, do not edit it; its
  `web/package.json` pins (Vue 3.5.45, Nuxt UI 4.12.0, Tailwind CSS 4.3.5, Vite 8.3.8) are the
  ones the copy is tested with.
- `src/panel/main.ts` mounts `App.vue` with `mountPanel(App, { prefix: 'plugin.<id>.', name })`:
  an in-memory stand-in for the storage the sandbox refuses, the texts from the panels window,
  the bundled Lucide icons only (the frame has no network), the app's light or dark theme.
  `usePanel()` gives a component `t(key, params)` (ICU plurals), `error(failure)` (a failed call
  as one sentence), `call({ tool } | { command }, args)`, `post(message)`, `settings`, `ready`
  and `onSettings`; `onPanelEvent` (`web-common/panel-ui/src/events.ts`) the host's events (a
  page script's `emit`, an answer's `$post` to the panel).
- `App.vue` starts with `PanelFrame` (title, icon, header actions as `IconAction`, one status
  line), then Nuxt UI components - your own element only where Nuxt UI has none.
- Tabs go in `PanelTabs` (`web-common/panel-ui/src/components/PanelTabs.vue`), `UTabs` in one
  row: it scrolls sideways without a scroll bar, a back and a forward button show when tabs are
  cut off, and the chosen tab is scrolled into sight; attributes and content slots are `UTabs`'.
- Size: the example's `panel.js` is about 350 KB (110 KB gzip) and `panel.css` 206 KB (26 KB
  gzip); each Nuxt UI component you use adds to the script. Licences of the bundled code: MIT
  (Vue, Nuxt UI and the packages it uses, Tailwind CSS), ISC (Lucide's icons; MIT for those from
  Feather) - all in `THIRD-PARTY-LICENSES.txt`; ship that file with the panel.

**Page scripts and panels** (rBrowser 0.6, RB-060-12): add `[[page_scripts]]` (files under
`page/`) and `[[panels]]` (files under `panel/`) to the manifest. A page script runs in an
isolated world of the plugin's own in every view and talks to the component through
`rbPlugin.send(message)`, which arrives as `call_tool("@page", {"view", "message"})`; a panel runs
in a sandboxed frame without network and calls the plugin's own commands and tools through
`postMessage`. Any answer may carry `"$post": [{"target": "all" | <view id> | "panel",
"message": ...}]`, which the host delivers to the page scripts (`rbPlugin.on`) or the panels. The
contract stays `rbrowser:plugin@0.1.0`; rBrowser's `plugins/hello-panel` uses every path.

The full guide is `docs/plugins.md` ("Page scripts and panels") and the wiki page *Plugin
development*.
