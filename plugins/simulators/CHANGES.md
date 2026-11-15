# Changes

## Unreleased

- Changed (RB-080-86): the picker is built on Nuxt UI - Off and the simulations as radio groups
  under their group names with each one's description, the strength as a slider with its value,
  "Apply to every view" as a switch that still acts at once (RB-080-75), the views with the
  active one marked, one status line; nothing shows before its texts. Added: a header action
  that reads every view's simulation again.

## 0.1.0

The first version (RB-070-08): fifteen simulations chosen per view - protanopia, protanomaly,
deuteranopia, deuteranomaly, tritanopia, tritanomaly, achromatopsia, achromatomaly, blurred
vision (myopia), cataract, glaucoma (tunnel vision), macular degeneration, low contrast, direct
sunlight and a dimmed screen - so two views can show the same page as two people see it. Chromium
views render protanopia, deuteranopia, tritanopia, achromatopsia, blurred vision and low contrast
themselves (CDP `Emulation.setEmulatedVisionDeficiency`) while no strength is chosen; the page
script draws everything else as a CSS filter on the root element (a `position: fixed` header
stays put) and a layer clicks pass through. A view keeps its simulation across reloads; without
the setting `persist` a new start begins with the `default` simulation. The picker
(`Ctrl+Shift+Space O I`) chooses for the active view, Shift+click for every view, with a strength
slider; MCP `simulator_set`. Settings > DevTools & Debugging > Simulators: `default`, `blurPx`,
`preferNative`, `persist`.
