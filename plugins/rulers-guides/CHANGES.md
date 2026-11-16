# Changes

## Unreleased

- Changed (RB-080-86): the panel is built on Nuxt UI - a switch per layer, the guides as rows
  with their unit as a choice and a remove button with its name, the template, alignment and
  breakpoint as choices; the panel's texts are the shared ones bundled into it, no copy of
  texts.js any more.
- RB-080-66: the page script keeps one request in flight per view (what comes meanwhile makes
  one more), asks nothing on a resize or post while nothing is shown and tries a failed start
  again; a failure goes to rBrowser's log and console, not the page's. With seven views the
  component got ten requests for ten posts and eight per second of grid reflow, behind one call
  at a time - `plugins.error.busy` and `timed_out` dozens of times in every view's console.
- RB-080-66: the dialog's column overlay and baseline grid act: there was no effect because
  Apply was a form the sandboxed frame never submits. No form now; every change of a field applies
  at once, without an Apply button (Enter too), setting a layer up switches it on, empty fields show the
  template's values as placeholders (`rulers_set` answers `templates`), and a failure reads as a
  sentence, never as a code.

## 0.1.0

The first version (RB-060-05): rulers at the top and left edge of every view in page CSS px,
with the pointer marked; guides dragged out of a ruler - horizontal ones in px from the top,
vertical ones in px from the left, px from the right or per cent of the page's width - moved by
their handle in the ruler, snapped with Shift, removed by double-click; a column overlay per
breakpoint (columns, gutter, margin, max width, align; Bootstrap and Tailwind templates with
their container widths) and a baseline grid. Every change shows in every view at once. The
shared setups are the plugin's settings (Settings > DevTools & Debugging > Rulers, at project
scope for the team), the person's own changes are kept in `.rbrowser/out/rulers-guides/state.json`.
A dialog sets everything by keyboard; MCP `rulers_set`; commands `Ctrl+Shift+Space O R`
(rulers), `O G` (baseline grid), `O C` (column overlay).
