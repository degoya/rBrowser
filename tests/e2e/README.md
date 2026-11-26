# End-to-end criteria

`scripts/e2e-windows.sh` checks the acceptance criteria K1 to K12 of
`docs/roadmap.md` ("Acceptance criteria per milestone") (K0 is an ADR review),
the scroll sync fixture and the milestone smokes against the Windows package, and writes a report
with every measured value and its threshold (RB-099-02). It is required before a milestone closes
(`docs/dod.md`).

```bash
scripts/e2e-windows.sh                         # from WSL: the version's package, every criterion
scripts/e2e-windows.sh --only K4,K10           # only these
scripts/e2e-windows.sh --list                  # the plan; nothing starts
scripts/e2e-windows.sh --package <zip>         # another package; --app <dir> an app folder
scripts/e2e-windows.sh --native                # Git Bash on Windows (the windows-2025 runner)
scripts/e2e-windows.sh --allow-missing firefox # no Firefox: K11 skipped, not failed
scripts/e2e-windows.sh --keep                  # the last app stays open for a look
```

Exit 0: every criterion passed (or was allowed to skip); 1: one failed; 2: the run could not
start. A missing prerequisite - Firefox not installed, WebKit not available - fails its criterion
unless `--allow-missing` names it. The report is `artifacts/e2e/report.json` and `report.md`
(`--report <path>`), each criterion's log `artifacts/e2e/logs/<id>.log`, the app start's
`logs/start-<project>.log`; the work folder `artifacts/e2e/work/` holds the projects' copies.
An app left open with `--keep` has MCP and the CDP proxy on (127.0.0.1, token-protected): close
it before the next run, which unpacks over its folder.

## How a run goes

1. The script stages the package (`scripts/package-windows.sh` makes it; the script builds
   nothing and takes no lock) in `%LOCALAPPDATA%\rbrowser-e2e\` - never the person's rBrowser -
   and marks the folder (`.rbrowser-e2e`).
2. `tests/e2e/run.ts` (the Node of `.nvmrc`, which runs TypeScript as it is) switches the staged
   app to the portable mode with known settings (`lib/app.ts`: MCP and the CDP proxy on, the
   timing counters on, updates and the first-run wizard off, no trust), serves
   `tests/fixtures/site` on 127.0.0.1:18766 and :18765 (`lib/server.ts`) and copies the projects
   into the work folder.
3. Per project one app, in turn (`lib/plan.ts`): the cases without an app (K5, K12), the e2e
   project `tests/fixtures/e2e-project` (three Chromium views), the untrusted project (K9), the
   perf project last and alone (K3: `scripts/perf.sh` sums every `rbrowser.exe`'s memory).
4. A case drives the app as a client: MCP through `rbrowser.com mcp --project <dir>` (from WSL
   across the interop, which is K7), CDP through the token proxy (`cdp_endpoint`), and the
   fixture server's own counts and logs. No case reads a token into a log or the report
   (`lib/report.ts` `redact`).

From WSL the fixture server and the projects stay in WSL: K6 watches a WSL project (the
`rbrowser-wsl` watcher), K7 speaks MCP across the interop. With `--native` everything is on
Windows: K6 uses the local watcher, K7 `rbrowser.com mcp` directly; the report notes it.

## The criteria

| K | Case | What is checked | Threshold | Jobs |
| --- | --- | --- | --- | --- |
| K1 | `k01-click-latency.ts` | `scripts/perf.sh --runs 0 --clicks 100`: 100 clicks on `latency.html`, each view's timestamp log | p95 <= 150 ms, no click missed | RB-020-03 |
| K2 | `k02-desync-badge.ts` | A click on "Pricing" of `mobile-menu.html` in the desktop view; the mobile view lacks it | `desync` in `views_list` <= 1 s; the tablet view clicked | RB-020-02 |
| K3 | `k03-scroll-perf.ts` | `scripts/perf.sh --views 8 --dpr3 2` on `tests/fixtures/project-perf` | >= 50 fps (min of 3 runs), < 3 GB, runs within 10 % | RB-020-09 |
| K4 | `k04-leader-submit.ts` | A real click on the submit of `form.html`; the fixture server counts POSTs | exactly 1 POST; every view on `/form.html?posts=1` | RB-020-02 |
| K5 | `k05-init-widths.ts` | `rbrowser.com init --yes --url` on copies of the Tailwind v3/v4 and Bootstrap fixtures | `expected-static.json` widths exactly; `schema/config.v1.json` valid | RB-010-13, RB-030-07 |
| K6 | `k06-hot-reload.ts` | `www/reload.css`, then `reload.js` written in the project's copy; the page reports each reload to `/log` | every view <= 1 s after the save; CSS without a full reload, the JS state kept | RB-030-05 |
| K7 | `k07-mcp.ts` | `navigate` with `target: all` over `rbrowser.com mcp`; a project without an instance | each view's final URL; an error naming the cause and `rbrowser open` | RB-030-01, RB-030-02 |
| K8 | `k08-hreflang.ts` | `seo_check` (`scope: hreflang`) on `hreflang-broken` and `hreflang-ok`, served with their headers and robots.txt | the snapshot `plugins/seo/tests/snapshots/hreflang-broken.txt` exactly; ok: 0 errors | RB-060-09 |
| K9 | `k09-untrusted.ts` | A fresh copy of `tests/fixtures/untrusted-project` in an empty trust store | `trusted: false`; `flow_run` refused; its plugin `blocked`; pages load without the proxy, host rule or header; no page script or snippet ran | RB-010-09 |
| K10 | `k10-cdp-proxy.ts` | The proxy without a token, with a wrong one, with the right one | 401/403 and refused sockets; no `rbrowser:`, `devtools:` or internal target; one page target per Chromium view | RB-000-05, RB-030-06 |
| K11 | `k11-engines.ts` | The tablet view on the installed Firefox, the mobile view on WebKit; the desktop view leads; `app_perf` into `scripts/perf.sh --counters` | replay p95 <= 300 ms per engine; the mobile view's `unsupported` marks | RB-040-02, RB-040-04 |
| K12 | `k12-recorder-spec.ts` | `tests/e2e/recorder/run.sh`: the recorder's export of a prepared recording under `npx playwright test` | green in every breakpoint project | RB-050-02 |
| SYNC | `sync-scroll.ts` | Wheel input in the desktop view over the page, the sidebar and the wide table of `scroll.html` | the same section under the bar, the sidebar and the table scrolled in every view | RB-020-04 |

K1, K3 and K11 are the measurements of `docs/perf-budget.md`: the
cases call `scripts/perf.sh` with the session's endpoint and read its result, they do not measure
a second way. Write the numbers of a reference-machine run into that page's table.

## The milestone criteria

From 0.7 on a milestone's criteria are its jobs' acceptance criteria; `docs/milestones/` does not
exist. The items of the milestone sections of `docs/roadmap.md` (0.7, 0.8, 0.9), each to a case
or a documented measurement:

| Milestone | Item | Proven by |
| --- | --- | --- |
| 0.7 | Plugins storage-panel, network-panel, outline, snippets, json-xml-viewer | M7 (`m07-panels.ts`): loaded; what each does: the Windows acceptance of RB-070-01 to RB-070-05 |
| 0.7 | Project bookmarks | M7: `bookmarks_list`; RB-070-06 |
| 0.7 | More debug overlays, plugin simulators | M7: `simulators` loaded; the overlays: RB-070-07's acceptance |
| 0.7 | image-overlay, color-picker, form-autofill, tab order, fonts | M7: loaded; tab order: RB-070-12's acceptance |
| 0.8 | Screenshot editor, video recording (ffmpeg on demand), environment comparison | M9 (`m09-capture.ts`): the editor loaded, `video_list`; a recording and the comparison: measurements of RB-080-02 and RB-080-03 (ffmpeg download, two environments) |
| 0.8 | Layout modes horizontal, split and canvas | M10 (`m10-layouts.ts`) |
| 0.8 | Portal: real devices on the LAN, pairing with QR code | M11 (`m11-portal-extensions.ts`): its state, off by default; the pairing: RB-080-05's measurement with a phone |
| 0.8 | Chrome extensions (Web Store, unpacked, recommended), Vue and React DevTools | M11: `extension_list`; installing: RB-080-06, RB-080-23, RB-080-24 (a person's decision, no MCP tool) |
| 0.8 | coverage, pwa-check, error-report; emulation extras; SEO extension | M11: the plugins loaded; K8 for the hreflang set; the extras: RB-080-10, RB-080-11 |
| 0.8 | Screenshots without cookie banners and animations | M9: `screenshot` with `hideBanners` and `freezeAnimations` in every view |
| 0.8 | Fixes from the owner's Windows tests (RB-080-16 to RB-080-22) | Their acceptance on Windows |
| 0.9 | Final Windows package: ZIP, optional MSI, notices | M12 (`m12-package.ts`): the package under test is the checkout's version, no bundled plugin in error; the content: RB-099-01 |
| 0.9 | GitHub mirror, public wiki, channels, signing prepared, macOS test build | Release steps, not the app: RB-090-12, RB-090-13, RB-090-03, RB-090-15 (`scripts/export-public.sh`, `scripts/package-managers.sh --check`) |
| 0.9 | Self-update on, signed manifests | M12: `update_status` answers; the update itself: `scripts/self-update-smoke.sh`, RB-090-14 |
| 0.9 | `e2e-windows.sh` covers every K criterion | This harness (RB-099-02) |
| 0.9 | Reviews: security, performance, accessibility | RB-090-06, RB-090-07 (K1, K3, K11 here), RB-090-08 |
| 0.9 | Wiki complete with generated references | RB-090-09; `scripts/check.sh --preflight` holds the generated pages |

## The counter-check

Each criterion must fail on exactly the defect it guards (the job's acceptance criterion). The
procedure, once per criterion, on a branch that is thrown away afterwards:

1. Build the package with the defect below (`scripts/package-windows.sh --allow-dirty`).
2. `scripts/e2e-windows.sh --only <K>` must fail with the named measurement; then
   `scripts/e2e-windows.sh` (all) must fail that criterion and no other.
3. Record the run's `report.md` row in the job's verification note.

| K | Defect to build in | Must fail |
| --- | --- | --- |
| K1 | A 200 ms sleep before each click replay in the sync hub | `p95` |
| K2 | `desync` never set for `not_found` | `badge after` (the wait gives up) |
| K3 | Hidden-view throttling off and a 30 ms busy loop per frame in the agent | `fps (min)` |
| K4 | `sync.leaderOnlySubmit` ignored (every view submits) | `POSTs at the server` |
| K5 | The analyser rounds every width down to a multiple of 100 | `<fixture> widths` |
| K6 | The CSS-only path reloads the whole page | `full reloads on the CSS change` |
| K7 | `navigate` answers the requested URL instead of each view's final one (navigate to a redirect) | `final URL per view` |
| K8 | The hreflang check skips the return-link rule | `broken: missing findings` |
| K9 | The trust check answers trusted for every project | `project_info trusted` and the rest of K9 |
| K10 | The proxy's token check off | `HTTP without a token`, `WebSocket without a token` |
| K11 | A 400 ms delay in the Firefox replay | `Firefox replay p95` |
| K12 | The exporter writes a wrong selector for the login button | `tests failed` |

K12 needs no package: break `tests/e2e/recorder/generated/login.spec.ts` by hand and run
`--only K12`.

## The harness itself

`tests/e2e/` is an npm package of its own: `npm test` runs the Vitest tests of the helpers
(`lib/*.test.ts`: the MCP client against a fake server, the fixture server for real on a free
port, the report, the plan, the instance file, the staged folder), `npm run build` type-checks
everything; `scripts/check.sh --preflight` runs both. `scripts/tests/e2e-windows.sh` checks the
script without Windows. A case is one file exporting a `Case` (`lib/case.ts`), listed in
`lib/registry.ts`.
