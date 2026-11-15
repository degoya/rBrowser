# Changes

## Unreleased

- Fixed (RB-090-19): the pipette read nothing on colourful pages (diag11: "Cannot read ... without
  flicker" in every view, three calibrations per pick failing), and where it did read, it could
  read the wrong place - a box of the page taken for the bottom marker measured 0.5538 px per CSS
  px for an image of exactly 1.5. An image that is the viewport at its DPR (every view at 100 %)
  is now mapped by its size, without markers; a view shown scaled by any two of four markers, one
  in each corner, framed in black so a page's colour beside one never runs into it, a pair kept
  only near the scale the image's size gives and at a marker's size. A colourful page no longer
  fills the marker search before the bottom markers (it kept the first 16 boxes of the image and
  no later one; a closed box without a marker's shape now makes room). A window image
  of more than 8 megapixels (view 7 of diag11, "failed: plugins.error.invalid") is named with its
  size in the log, and the status bar says why a view cannot be read: no markers, an image too
  large, no image.
- Changed (RB-080-86): the panel is built on Nuxt UI - Pick is the header's main action, the
  colours, the palette and the text measure are cards, the levels and the measure are tables
  with their verdicts as badges, copy, mark and use are icon buttons named in full; the counts
  are plural-aware. The pick and calibration logic is unchanged.
- The views no longer flicker on Windows while the pipette is on (RB-080-77). The owner's log
  (diag9) had every read of RB-080-64's window image refused - "does not show the marker where
  the viewport maps it: [Some([231, 70, 251]), Some([124, 248, 59])]; a clip instead", 196 times
  - and fall back to the clip, which makes the view render a picture of the clip's size for a
  frame. The window image is in the display's colours (a colour profile turns `#ff00ff` into
  `(231, 70, 251)`), and the viewport is not always where the image's size puts it. Now the page
  script shows two markers in the viewport's corners once per pipette (and after a resize or a
  DPR change), the component finds them anywhere in the image by which channels dominate and
  measures the viewport's scale and offset in it (`calibrate`, `src/calibrate.rs`); every read
  sends that map back. A window image without the markers is asked again with more frames to
  wait (the image lags the page), then the status bar says the view cannot be read without
  flicker - never a clip by itself. The new setting **Pipette reads** (`capture`: `window`, the
  default, or `clip`) takes the page's own colours with flicker where a person wants them; a
  colour taken from a window image in the display's colours says so in the panel. Firefox and
  WebKit views get no pipette (diag9: `plugins.error.not_available` 33 times); a request of such
  a view is refused before any CDP call. The loupe is cut around the pointer by the measured
  map, its middle the pixel taken, at any DPR and offset; the tiles' math is a page script of its
  own, `page/tile.js`. The status bar's texts no longer name the plugin themselves - the core
  does (RB-080-76: "Colour picker: Colour picker: picked ..."). A failure in the panel shows its
  code's sentence, or "The colour picker could not finish this.", never the code.


- The views no longer flicker when the loupe's tile is taken where the pointer comes to rest
  (RB-080-64). A clipped `Page.captureScreenshot` - and, under the device emulation of a view
  scaled into its cell, an unclipped one from the surface - makes Chromium render the page again,
  and the view shows that picture for a frame. The component now reads the view as its window
  shows it (`fromSurface: false`, no clip; measured in Chromium under rBrowser's emulation: 5 of
  5 shots with a clip drew a foreign frame, 0 of 5 from the window) and keeps only the pixels
  around the point - the tile, the loupe of a click, `color_sample` -, decoding the PNG row by row.
  Where the window's image fails or is not the viewport, a clip as before (logged; the first tile
  of each pipette names at info how the view was read). A tile older than two seconds is replaced
  at the next rest. `color_sample`'s `devicePixel` is the pixel of the viewport's image.
- The pipette shows that it is on (RB-080-64): the panel's button is pressed at once and reads
  "Picking… Esc to stop", a second click or Esc (in the panel or a view) ends it in every view,
  the status bar names start, the colour taken and the end - also when the pipette was started by
  its shortcut. `pick` takes `{"stop": true}`; a page script's `stopped` ends it everywhere.
- Review of RB-080-64: the window image is a permission of its own (`cdp.window_image`); the
  status bar shows "Colour picker: ..." (capability `status`); a window image above 8
  megapixels is read as a clip; the read that shows the marker is never kept; the panel's
  "Measure in every view" works (no form in the sandboxed panel; Enter in the field measures).
- The marks (long lines, palette uses) are a page script of their own, `page/marks.js`
  (`rbrowser-color-picker-marks`), split off `page/picker.js`.

- The views no longer flicker while the pipette is on (RB-080-56). Chromium renders a clipped
  `Page.captureScreenshot` through the device emulation, and the view draws a different picture
  for a frame, and the pipette took one per pointer move - and one in every view at its start and
  at each synced scroll. Now only the view the pointer (or the keyboard) is in takes a
  screenshot: one tile of about 117 x 117 device pixels where the pointer rests (120 ms), from
  which the loupe is drawn while the pointer moves inside it. At the start the active view shows
  the loupe at its centre (one screenshot); no other view takes one before the pointer comes. A
  scroll, a resize or a change of the device pixel ratio drops the tile; a failed one is tried
  once more. The loupe shows the page as it was when the pointer came to rest - a hover effect or
  an animation under a resting pointer is not followed. The label comes from the new request
  `value`, without a screenshot; the per-position request `sample` is gone. Every screenshot (and
  every failed one) writes one debug line to the log (`color-picker: screenshot of view`).

## 0.1.0

First release (RB-070-10): a pipette on the rendered pixels of every view - a screenshot of the
pixels around the cursor (CDP `Page.captureScreenshot`), DPR-aware, an 11 x 11 loupe
(`color-picker.loupeSize`); click takes the colour, Shift+click the contrast's background,
arrows move by 1 px (Shift: 10), Enter takes, Esc ends (`Ctrl+Shift+Space D I`). The panel
(`Ctrl+Shift+Space P I`) shows both colours as HEX, `rgb()`, `hsl()` and `oklch()` to copy (the
default format first, `color-picker.defaultFormat`), their WCAG 2.x contrast with AA and AAA for
normal and large text, the active view's palette - computed text, background and border colours
grouped in OKLab and sorted by use, their uses marked in every view - and the characters per line
of `color-picker.textMeasure.selector` (or an element chosen in a view) in every view: lines,
average, longest, rated against 45-75 with lines above 80 marked (`Ctrl+Shift+Space D M`). MCP:
`color_sample`, `color_contrast`, `color_palette`, `color_measure_text`. Chromium views only.
