// The viewer's style sheet, adopted by its closed shadow root as a constructed sheet: a page's
// CSP (`style-src`) governs <style> elements and style attributes, not a sheet built through
// the CSSOM, and the page's own CSS cannot reach into the shadow root. Two palettes (GitHub's
// Primer colours: every text colour is at least 4.5:1 on the background and on the selected row,
// the borders of fields and buttons at least 3:1); `auto` follows the view's
// `prefers-color-scheme`.

export const ROW_HEIGHT = 20;

const LIGHT = `
  --bg: #ffffff; --fg: #1f2328; --muted: #59636e; --line: #d1d9e0; --edge: #818b98;
  --bar: #f6f8fa; --key: #8250df; --string: #0a3069; --number: #0550ae; --literal: #953800;
  --tag: #116329; --attr: #953800; --selected: #ddf4ff; --match: #fff8c5; --focus: #0969da;
  --error: #d1242f; --button: #f6f8fa; --pressed: #ddf4ff;
`;

const DARK = `
  --bg: #0d1117; --fg: #e6edf3; --muted: #9198a1; --line: #3d444d; --edge: #768390;
  --bar: #151b23; --key: #d2a8ff; --string: #a5d6ff; --number: #79c0ff; --literal: #ffa657;
  --tag: #7ee787; --attr: #ffa657; --selected: #1f2d3d; --match: #4d3800; --focus: #4493f8;
  --error: #ff7b72; --button: #212830; --pressed: #1f2d3d;
`;

export const CSS = `
.app { ${LIGHT} color-scheme: light; }
.app[data-theme="dark"] { ${DARK} color-scheme: dark; }
@media (prefers-color-scheme: dark) { .app[data-theme="auto"] { ${DARK} color-scheme: dark; } }
.app {
  position: absolute; inset: 0; display: flex; flex-direction: column;
  background: var(--bg); color: var(--fg);
  font: 13px/1.4 system-ui, -apple-system, "Segoe UI", sans-serif;
}
.bar {
  display: flex; flex-wrap: wrap; align-items: center; gap: 6px; padding: 6px 8px;
  background: var(--bar); border-bottom: 1px solid var(--line);
}
.group { display: inline-flex; gap: 2px; }
button {
  font: inherit; color: var(--fg); background: var(--button); border: 1px solid var(--edge);
  border-radius: 6px; padding: 2px 10px; min-height: 26px; cursor: pointer;
}
button[aria-pressed="true"] { background: var(--pressed); border-color: var(--focus); }
button:disabled { cursor: default; opacity: .6; }
input {
  font: inherit; color: var(--fg); background: var(--bg); border: 1px solid var(--edge);
  border-radius: 6px; padding: 2px 8px; min-height: 26px; flex: 1 1 220px; min-width: 120px;
}
input::placeholder { color: var(--muted); opacity: 1; }
:focus-visible { outline: 2px solid var(--focus); outline-offset: 1px; }
.notice { padding: 4px 10px; color: var(--fg); background: var(--match); }
.notice:empty { display: none; }
.error { padding: 4px 10px; color: var(--error); font-weight: 600; border-bottom: 1px solid var(--line); }
.error:empty { display: none; }
.body { position: relative; flex: 1 1 auto; min-height: 0; }
.scroller {
  position: absolute; inset: 0; overflow: auto; overscroll-behavior: contain;
  font: 12.5px/${ROW_HEIGHT}px ui-monospace, "Cascadia Mono", Consolas, "Liberation Mono", monospace;
}
.scroller:focus-visible { outline-offset: -2px; }
.sizer { width: 1px; }
.row {
  position: absolute; left: 0; right: 0; height: ${ROW_HEIGHT}px; white-space: pre;
  overflow: hidden; text-overflow: ellipsis; padding-left: calc(var(--level, 0) * 16px + 4px);
  padding-right: 8px; box-sizing: border-box; cursor: default;
}
.row[aria-selected="true"] { background: var(--selected); }
.scroller:focus .row[aria-selected="true"] { box-shadow: inset 0 0 0 2px var(--focus); }
.twisty { display: inline-block; width: 14px; color: var(--muted); cursor: pointer; }
.row[aria-expanded="false"] .twisty::before { content: "\\25B8"; }
.row[aria-expanded="true"] .twisty::before { content: "\\25BE"; }
.k { color: var(--key); }
.s { color: var(--string); }
.n { color: var(--number); }
.l { color: var(--literal); }
.t { color: var(--tag); }
.a { color: var(--attr); }
.m, .c { color: var(--muted); }
.p { color: var(--muted); margin-right: 8px; }
.raw {
  position: absolute; inset: 0; margin: 0; padding: 8px; overflow: auto; overscroll-behavior: contain;
  white-space: pre-wrap; overflow-wrap: anywhere;
  font: 12.5px/1.5 ui-monospace, "Cascadia Mono", Consolas, "Liberation Mono", monospace;
}
mark { background: var(--error); color: var(--bg); }
.status {
  padding: 3px 10px; border-top: 1px solid var(--line); background: var(--bar); color: var(--muted);
  font: 12px/1.5 ui-monospace, "Cascadia Mono", Consolas, monospace;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-height: 18px;
}
.banner {
  position: fixed; top: 8px; right: 8px; max-width: min(520px, calc(100vw - 16px));
  display: flex; gap: 10px; align-items: center; padding: 8px 10px; border-radius: 6px;
  background: var(--bar); color: var(--fg); border: 1px solid var(--edge);
  box-shadow: 0 4px 16px rgba(0, 0, 0, .25);
}
.banner.app { position: fixed; inset: 8px 8px auto auto; flex-direction: row; }
[hidden] { display: none !important; }
`;
