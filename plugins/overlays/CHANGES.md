# Changes

## Unreleased

- Changed (RB-080-86): the panel is built on Nuxt UI - the overlays as switches in cards by kind
  (layout, elements by selector, accessibility, page content), the selector with Find and the
  match steps and the forced state as a choice beside the switches that use them, the tab order
  comparison with the accessibility overlays, "all off" as a header action, one status line.
- Fixed (RB-090-17): switching a checking overlay (stacking contexts, scrollbars, text spacing,
  target size, small text) or the tab order on for a big page in several views no longer fails
  with "larger than the plugin may handle (65536)": the hits, stops and differences one answer
  lists share a byte budget under the 64 KiB message limit, every text cut to 300 characters, each
  view's entry with its whole `count` and how many it lists (`shown`). The panel names the hits as
  "1500 of 2345 shown" when a page has more than the overlay draws.

## 0.3.0

The tab order overlay (RB-070-12, `O T`): every stop of the tab order numbered as the browser walks
it (positive tabindex first, open shadow roots and slots in place, same-origin iframes, inert,
disabled and unrendered elements out, one radio button per group; a cross-origin iframe is one
stop), joined by lines (`tabOrder.showLines`); a jump against the reading order in orange, a stop
with a positive tabindex or one nobody sees (no size, opacity 0, before the page, clipped by a
closed menu) in red. Live mode (`tabOrder.live`): the stop focused in one view is ringed in every
view, without moving the focus. `tabOrder.includeIframes` walks same-origin iframes. The new tool
`overlay_tab_order` answers each view's ordered list (index, selector, path, tabindex, rect,
warnings) and the differences between the views (elements only some views reach, elements at
another place); `overlay_set` with `tab-order` answers the same and reports the findings
`tab-positive-tabindex` and `tab-invisible-focusable`. The panel compares the views. Force-state
keeps to the document someone requested and switches the DOM and CSS domains on only when the
force is refused (review finding L2 of Charge 7; the `Accessibility` domain names the document).

## 0.2.0

The further debug overlays (RB-070-07): stacking contexts labelled with z-index and reason
(`O Z`), unneeded scrollbars (`O S`, `scrollbarTolerancePx`), WCAG 1.4.12 text spacing with the
clipped text marked (`O E`), target size with the 2.5.8 spacing exception (`O A`,
`targetSizePx`), small text (`O M`, `smallTextPx`), focus tracking (`O K`), forced
`:hover`/`:focus`/`:active`/`:focus-visible` over CDP `CSS.forcePseudoState` (`O V`, Chromium),
content chaos (`O L`, `chaos.factor`), the placeholdifier (`O P`), CSS off (`D 1`,
`cssOff.includeInline`), JavaScript off over CDP `Emulation.setScriptExecutionDisabled` with a
reload (`D 2`, Chromium) and images off (`D 3`). `overlay_set` takes the twelve new ids and
`target: "active"`, answers the hits of the checking overlays per view (also reported as findings
while `reportFindings` is on), the forced states and the script switch per view. Page changes go
through constructed style sheets of the plugin's own or remembered text nodes and are undone when
an overlay is switched off. The panel switches every overlay (Shift-click: the active view only).

## 0.1.0

The debug overlays (RB-060-03): the overflow detector (the viewport framed in yellow while the
page scrolls sideways, the causing elements in red with their overhang, `100vw` elements in
blue), the element highlight (a CSS selector searched in every view, the current match
emphasised) and the layout outlines (every element, or the flex and grid containers, framed by
depth). Commands `plugin.overlays.outlines` (`Ctrl+Shift+Space D O`), `.overflow` (`O F`),
`.highlight` (`O H`) and `.off` (`O X`); MCP tools `overlay_set`, `overlay_overflow` and
`overlay_state`; the panel "Overlays"; settings under DevTools > Overlays. The state is kept per
project in `.rbrowser/out/overlays/state.json`.
