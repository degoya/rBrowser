# Writing an rBrowser plugin

The SDK - these templates, the WIT contract and the CI workflow - is licensed **MIT**
([`LICENSE`](LICENSE)); rBrowser and its first-party plugins stay GPL-3.0-or-later.

A plugin is a signed WebAssembly component (`docs/plugins.md`). It runs in a sandbox with no file
system, no processes, no network and no WASI of its own, and reaches exactly the host interfaces
its manifest's capabilities grant - nothing is granted implicitly, nothing can be asked for at run
time. Copied and adapted from rCommander's SDK.

| Template | World | What it does |
| --- | --- | --- |
| [`templates/feature`](templates/feature) | `feature-plugin` | Commands and MCP tools (`<mcp_prefix>_<tool>`), panels (with a Nuxt UI panel), page scripts |
| [`templates/check`](templates/check) | `check-plugin` | Findings in the shared format (`checks_run`) |
| [`templates/analyzer`](templates/analyzer) | `analyzer-plugin` | Proposes config changes a person applies |
| [`templates/export`](templates/export) | `export-plugin` | Turns a recording into code |

A plugin of several kinds targets a world of its own that `include`s them and lists every kind in
its manifest; commands and MCP tools are always served by the `feature` export.

Each template is a small working plugin: `cargo test` runs its logic natively, `cargo build
--target wasm32-unknown-unknown` and `wasm-tools component new` (1.261.0) make the component,
`rb-plugin-pack package` signs it and `rb-plugin-pack conformance` asks it what rBrowser will.
[`ci/plugin.yml`](ci/plugin.yml) does all of it in a CI job. `scripts/check-sdk-templates.sh` (in
the rBrowser repository) holds every template to its world and to the contract; with `--build` it
builds each one on its own.

## Panels with Nuxt UI

A plugin's panel looks like rBrowser's own when it uses the same parts: the feature template's
panel (`templates/feature/web/`) is a Vue 3 app on Nuxt UI built with **panel-ui**, the panel
runtime of rBrowser's first-party panels, carried as a copy in `templates/feature/web/web-common/`.

| | |
| --- | --- |
| Version | panel-ui 0.1.0 with Vue 3.5.45, Nuxt UI 4.12.0, Tailwind CSS 4.3.5, Lucide icons (`@iconify-json/lucide` 1.2.154), built with Vite 8.3.8 on Node 26.13.1; the template's `web/package.json` pins them and `scripts/check-sdk-templates.sh` holds the copy and the pins to rBrowser's source |
| Licence | panel-ui and `texts.js` are MIT like the SDK (`web/web-common/panel-ui/LICENSE`). What the build bundles is MIT (Vue, Nuxt UI and the packages it uses, Tailwind CSS) and ISC (Lucide; MIT for the icons from Feather); the build writes every package's name, version and licence text into `panel/dist/THIRD-PARTY-LICENSES.txt` - ship it with the panel |
| How to include it | Keep `web/web-common/` as it is (replace it with a newer SDK's copy, never edit it), name your plugin in `web/vite.config.ts` (`id`, `license`) and `web/src/panel/main.ts` (the text prefix `plugin.<id>.`), then `npm ci && npm run build && npm test` in `web/`; `panel/index.html` loads `dist/panel.css` and `dist/panel.js` |
| Size | The example: `panel.js` about 350 KB (110 KB gzip), `panel.css` 206 KB (26 KB gzip); rBrowser holds its own panels to 640 KiB script and 256 KiB styles |

The panel runs in a sandboxed frame without network: no web fonts, no icon downloads, no
`localStorage` (panel-ui puts an in-memory stand-in first). `templates/feature/README.md` says
what each part does.

Keys: `rb-plugin-pack keygen --out <folder outside the repository>` makes the pair. The private
half belongs in your secret store, never in version control; a person installing your plugin
confirms the key's fingerprint once (trust on first use).
