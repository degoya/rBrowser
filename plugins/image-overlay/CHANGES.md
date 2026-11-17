# Changes

## Unreleased

- Text size (RB-090-39): the panel follows the shell's text size - Windows' *Accessibility >
  Text size* while `ui.followWindowsTextSize` is on -, live; its bundle is rebuilt with
  panel-ui's root font size from `ui.textScale`.
- Changed (RB-080-86): the panel is built on Nuxt UI - switches, sliders, a choice of blend mode
  and position, number fields for the offset, the designs as a grid of previews with name and
  size; a failed change is said as a sentence, never as its code, and the "only the first
  designs" note is plural-aware.
- *Add design...* (RB-080-74): the panel's empty state says what a design is and where it goes,
  and its button - also the command `add_design`, `Ctrl+Shift+Space O N`, palette - asks the
  application to open the native file dialog. The application checks the image as the overlay
  does (PNG, JPEG or WebP by content, 8 MiB, 16384 px a side, 16 megapixels), copies it into
  `.rbrowser/designs/` - only with an open project in a trusted workspace; never over a file: the
  same bytes are taken as they are, other bytes under the name become `name (2).png`, before the
  width and scale hints (`home (2)-1440@2x.png`) - and shows it for the active view's breakpoint
  through `set`; the same bytes already there are said to be there. A name Windows reserves for a
  device (`CON`, `NUL`, `COM1`, ...) gets a `_`. A cancelled dialog says so and ends the panel's
  busy button; right after a cancel the dialog does not open again for 2 s. The outcome is in the
  status bar and the panel. No MCP tool: a dialog is a person's. A file dropped on the panel is not taken (the panel cannot write into the project);
  the panel says to use the button.
- The design per breakpoint is a choice of the designs by name and size, *Nearest width* first;
  a chosen design that is gone stays in the choice, marked missing, an unusable one is marked.
  The panel lists the designs with a small preview: drawn once per design version from the bytes
  the component checked and kept in `.rbrowser/out/image-overlay/thumbs/`, so later openings read
  the small file; a preview that cannot be drawn is left out. The designs are listed again only
  when their names change or after Add design.
- MCP `imageoverlay_designs`: the designs in `.rbrowser/designs/` with size, file size, export
  scale and fingerprint (or why one is unusable), whether a design can be added, a design's bytes
  in chunks, and the kept previews (`thumb`, a PNG of at most 256 px a side and 32 KiB).

## 0.1.0

The first version (RB-070-09): each view's design image from `.rbrowser/designs/` over the page -
the design mapped to the view's breakpoint (`image-overlay.designs`), else the one nearest to its
width (the width in the file name, `home-1440.png`, or the image's width over its export scale);
`@2x`/`@3x` exports shown at their CSS size and preferred for the matching pixel ratio. Opacity,
blend modes (normal, difference, multiply, screen, overlay, exclusion), invert, an offset in CSS px
(arrow keys in the panel, Shift for 10 px), pinned to the page or fixed in the view, and a split
view with a handle in the page whose position every view takes. One state for every view, each
showing its own design; the shared defaults are the plugin's settings (Settings > DevTools &
Debugging > Image overlay), the person's own changes are kept in
`.rbrowser/out/image-overlay/state.json`. Designs are checked before a byte reaches a view: below
`.rbrowser/designs/` only, PNG, JPEG or WebP by content, at most 8 MiB, 16384 px a side and 16
megapixels. A sidebar panel sets everything by keyboard; MCP `imageoverlay_set`; command
`Ctrl+Shift+Space O D`.
