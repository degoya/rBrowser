// The colours of the tab order overlay. Each fill carries a white number at WCAG AA (4.5:1) and
// shows against light pages; the white ring around each marker and line, with a dark edge, shows
// against dark ones (warnings.test.ts holds them to it).

/** A stop. */
export const MARKER = '#1d4ed8';
/** A stop with a warning (positive tabindex, focusable but unseen). */
export const WARNED = '#b91c1c';
/** A stop reached by a jump against the reading order, and the line to it. */
export const REVERSE = '#c2410c';
/** The ring around markers and lines. */
export const RING = '#ffffff';
/** The live highlight: the stop focused in one of the views. */
export const LIVE = '#ff6a00';
