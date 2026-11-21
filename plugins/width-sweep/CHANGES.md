# Changes

## Unreleased

- Changed (RB-094-10): a finding's path is the element path every check plugin shares
  (`web-common/src/element-path.ts`): a step is numbered only when its parent holds more than one
  child of that tag (`html > body > main > div > img`, formerly `main:nth-of-type(1) > ...`), so
  a finding of this plugin and one of `a11y-axe`, `outline` or `seo` at the same element carry the
  same path. Every finding is re-identified once.
- Added (RB-093-01): the command and tool `show` (`plugin.width-sweep.show`, MCP `sweep_show`)
  holds one view at a finding's width - the findings panel's "Show at N px" - through the same
  width lease as a sweep, until Escape in the view, the cancel, another width or
  `width-sweep.showSeconds` (120 s, 10 to 240). The sweep's progress and a held width show in
  the activity bar with Cancel.

## 0.1.0

The first version (RB-093-01): one view swept through a range of widths - `width-sweep.from` to
`width-sweep.to` (320 to 1920 px) in steps of `width-sweep.step` (10 px), then pixel by pixel
between two widths whose issues differ - and the layout breaks reported as width ranges in the
findings panel ("700-740 px: horizontal overflow caused by `.hero img`"), each with the element's
selector and the width in its middle. Kinds (`width-sweep.kinds`): `overflow` (the document wider
than the viewport, its outermost causes, `100vw` elements beside a classic scrollbar left out),
`text-clipped` (text cut off by `overflow: hidden`/`clip` or `text-overflow`), `overlap` (visible
in-flow siblings overlapping). The width is the host's width lease: the view's own metrics at that
width, the view out of sync while it runs; the end, Escape in the view, the cancel and the end of
the plugin's call give its own width and sync back. Commands `plugin.width-sweep.run`
(`Ctrl+Shift+Space B W`) and `plugin.width-sweep.cancel` (`Ctrl+Shift+Space B Q`), both in the
context menu's Checks; MCP `sweep_run` and `sweep_cancel`. Settings in Checks > Width sweep.
