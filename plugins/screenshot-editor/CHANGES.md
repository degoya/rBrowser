# Changes

## Unreleased

- Text size (RB-090-39): the panel follows the shell's text size - Windows' *Accessibility >
  Text size* while `ui.followWindowsTextSize` is on -, live; its bundle is rebuilt with
  panel-ui's root font size from `ui.textScale`.
- Changed (RB-080-86): the panel is built on Nuxt UI - undo, redo, discard, copy and Save as
  header actions, the tools as a field group whose tooltips show their keys, the line width,
  text and block size as sliders, the colour as swatches and a colour picker in a popover, the
  images grouped by folder; the canvas, the crop frame and every key stay as they were.
- The panel's layout (RB-080-63): the image actions (file, undo and redo, discard, copy, save) in
  the top row; the tools as icons in the second, and beside them only the chosen tool's options;
  every tooltip names its shortcut. The crop tool drags a frame with handles and its size, and
  Apply crop (Enter) and Cancel (Escape) appear at the frame; outside the crop tool the image
  shows cropped (the keyboard cursor starts in its middle), and Undo takes the crop back. The
  crop as applied, applied again, changes nothing; Escape in the crop tool drops a changed frame,
  else discards unsaved changes as its tooltip says, else leaves the tool. A zoom the person chose
  stays after a crop. The canvas paints once per frame.
- Fixed: an image over 47 KiB with saved annotations did not open (`Failed:
  plugins.error.too_large`) - `load`'s first answer held a whole chunk and the annotations, more
  than one panel message (64 KiB). The chunk now waits for the next call when they do not fit
  together, and annotations larger than a message are left out with `annotations_too_large`. A
  host error reads as a sentence, never as its key.
- Fixed: the annotation font did not load in the sandboxed panel (blocked by CORS); it is
  embedded as a data URL (`panel/annotation-font.css`), the font file itself moved to `font/` and
  no longer ships.

## 0.1.0

The first version (RB-080-01): a dialog panel that opens a PNG of `.rbrowser/out/` - the newest
screenshot with `Ctrl+Shift+Space C E` - and annotates it with nine tools: rectangle (outline or
filled bar), arrow, line, freehand, text, numbered marker, highlight, redaction (pixelate, blur or
black bar) and crop. Undo and redo (100 steps), zoom, every tool and action by keyboard (tool keys
R A L F T M H B C, arrow keys and Enter to draw, Escape to cancel). Saving renders the file in the
component: `<name>.edited.png` (or `.jpg`) and `<name>.annotations.json` below
`.rbrowser/out/screenshot-editor/`, mirroring the original's path; opening the original again
loads its annotations. Redactions are rendered into the pixels (the blur is computed from the
mosaic only), the export carries no text chunk or other metadata. Copy to the clipboard from the
panel or with `Ctrl+Shift+Space C K`. MCP `shot_annotate` renders without UI, `shot_load` and
`shot_list` read. Settings > Capture > Screenshot editor: colour, line width, text size,
redaction block, export format, JPEG quality. Text is drawn with an embedded subset of Noto Sans
Mono Bold (SIL OFL 1.1).
