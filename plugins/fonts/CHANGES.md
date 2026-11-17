# Changes

## Unreleased

- Text size (RB-090-39): the panel follows the shell's text size - Windows' *Accessibility >
  Text size* while `ui.followWindowsTextSize` is on -, live; its bundle is rebuilt with
  panel-ui's root font size from `ui.textScale`.
- Changed (RB-090-64): the page bundle is rebuilt with web-common's `layers.ts`, which marks
  each layer `aria-hidden` instead of the shared `<rbrowser-layers>` host.
- Added (RB-090-63): "Check the fonts of every view" in the page's context menu under Checks
  (`context_menu`).
- Changed (RB-080-98): a check run over one Firefox or WebKit view answers that the view's engine
  cannot be checked (`plugins.error.not_available` with `engine`), so the findings panel shows
  the view skipped for its engine instead of "Nothing found"; a run over every view leaves such
  views out as before.
- Changed (RB-080-83): the panel is built on Nuxt UI - one status line, tabs Fonts and Sizes per
  breakpoint, the families with a line in each font and their facts and use per view as badges,
  the detail in sections that keep their open state, "not scaled" and "jumps" in a column of
  their own. Added: a preview per face in a sample text and size of one's choice, the font file
  read again in a view by the new tool `preview` (MCP `fonts_preview`) and loaded from its bytes;
  a missing preview says why. Settings `previewMaxKb`, `previewText`, `previewSizePx`.
- Changed (RB-080-62): the panel is laid out with the panel kit - the families with source and
  counts as badges, the detail in sections, the Rendered column only where a view knows how the
  family was drawn, a face's states grouped over the views, "not scaled" as a badge beside its
  role, no breakpoint column for a view without sizes, icon buttons with tooltips.

## 0.1.0

The first version (RB-070-13): the fonts of every view - each family with its source (Google
Fonts, Adobe Fonts, the page's own server, another host, inline, installed), formats, faces with
their `document.fonts` status per view, file sizes (Resource Timing) and preloads, and where it is
used per view - what Chromium really drew against what the page declares
(`CSS.getPlatformFontsForNode`, without dropping other clients' node ids), and the font sizes of
the text roles across the breakpoints. Findings for `checks_run`: `external-host` (privacy;
`fonts.allowedHosts`), `font-display-missing`, `format-not-woff2`, `preload-missing`,
`face-unused`, `too-many-weights`, `large-file` (performance), `text-too-small`,
`input-too-small`, `line-height-tight` (readability), `rendered-fallback`, `glyph-fallback`
(Chromium) and `fluid-not-scaled` (across the views). A click in the panel marks where a family
is used in every view. MCP `fonts_list`, `fonts_check`, `fonts_mark`, `fonts_export` (JSON and CSV
into `.rbrowser/out/fonts/`); commands `plugin.fonts.panel` (`Ctrl+Shift+Space P Y`) and
`plugin.fonts.check` (`Ctrl+Shift+Space K F`). Settings in Checks > Fonts.
